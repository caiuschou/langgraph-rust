//! HTTP server exposing POST /v1/chat/completions with OpenAI-compatible SSE streaming.
//!
//! Configure via env: OPENAI_API_KEY, OPENAI_MODEL, OPENAI_BASE_URL, DB_PATH, THREAD_ID, etc.
//! See langgraph's ReactBuildConfig::from_env(). Load .env with dotenv.

use std::io::{self, Write};
use std::sync::Arc;

use axum::{
    body::{to_bytes, Body},
    extract::{Path, State},
    http::{Request, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use bytes::Bytes;
use langgraph::{
    build_react_run_context, parse_chat_request, ChunkMeta, ParseError, ReactBuildConfig,
    ReactRunner, StreamToSse,
};
use tokio::sync::mpsc;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, info_span};

/// Shared state for all routes: runner for chat completions, and config for /v1/models proxy.
struct AppState {
    runner: Arc<ReactRunner>,
    openai_base_url: Option<String>,
    openai_api_key: String,
    http_client: reqwest::Client,
}

/// Max request body size to buffer for logging (bytes). Requests larger than this return 413.
const LOG_BODY_LIMIT: usize = 2 * 1024 * 1024;

/// Middleware that logs method and URI at debug, then forwards the request.
async fn log_request_body(request: Request<Body>, next: Next) -> Result<Response, Response> {
    let (parts, body) = request.into_parts();
    let method = parts.method.clone();
    let uri = parts.uri.clone();
    let bytes = to_bytes(body, LOG_BODY_LIMIT)
        .await
        .map_err(|e| (axum::http::StatusCode::PAYLOAD_TOO_LARGE, e.to_string()).into_response())?;
    tracing::debug!(method = %method, uri = %uri, "request");
    let body = Body::from(bytes);
    let request = Request::from_parts(parts, body);
    Ok(next.run(request).await)
}

/// Load .env from current directory; if not found, try parent (workspace root when run from crate dir).
fn load_dotenv() {
    if dotenv::dotenv().is_ok() {
        return;
    }
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(parent) = cwd.parent() {
            let env_path = parent.join(".env");
            if env_path.is_file() {
                let _ = dotenv::from_path(env_path);
            }
        }
    }
}

/// Writer that strips ANSI escape sequences (e.g. `ESC [ 0 m`) so file logs are plain text.
struct StripAnsiWriter<W> {
    inner: W,
    /// Incomplete escape sequence: ESC or ESC [ ... (waiting for final letter).
    state: Vec<u8>,
}

impl<W: Write> StripAnsiWriter<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            state: Vec::with_capacity(16),
        }
    }

    fn is_csi_parameter(b: u8) -> bool {
        b == b'[' || b == b'?' || b == b';' || (b >= b'0' && b <= b'9')
    }

    fn is_csi_final(b: u8) -> bool {
        b >= 0x40 && b <= 0x7e
    }
}

impl<W: Write> Write for StripAnsiWriter<W> {
    fn write(&mut self, mut buf: &[u8]) -> io::Result<usize> {
        let len = buf.len();
        while !buf.is_empty() {
            if self.state.is_empty() {
                if let Some(i) = buf.iter().position(|&b| b == 0x1b) {
                    self.inner.write_all(&buf[..i])?;
                    buf = &buf[i..];
                    self.state.push(buf[0]);
                    buf = &buf[1..];
                } else {
                    self.inner.write_all(buf)?;
                    break;
                }
            } else if self.state.len() == 1 {
                self.state.push(buf[0]);
                buf = &buf[1..];
                if self.state[1] != b'[' {
                    self.inner.write_all(&self.state)?;
                    self.state.clear();
                }
            } else {
                let b = buf[0];
                buf = &buf[1..];
                if Self::is_csi_final(b) {
                    self.state.clear();
                } else if Self::is_csi_parameter(b) || b == b':' {
                    self.state.push(b);
                    if self.state.len() > 64 {
                        self.inner.write_all(&self.state)?;
                        self.state.clear();
                    }
                } else {
                    self.inner.write_all(&self.state)?;
                    self.state.clear();
                    self.state.push(b);
                }
            }
        }
        Ok(len)
    }

    fn flush(&mut self) -> io::Result<()> {
        if !self.state.is_empty() {
            self.inner.write_all(&self.state)?;
            self.state.clear();
        }
        self.inner.flush()
    }
}

/// Initializes tracing: always to stdout; if env `LOG_FILE` is set, also to that file (append).
/// File output is plain text (ANSI stripped) and uses a compact, readable format.
fn init_tracing() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::Layer;

    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("info,langgraph_server=debug")
    });

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout)
        .with_filter(filter.clone());

    let registry = tracing_subscriber::registry().with(stdout_layer);

    if let Ok(path) = std::env::var("LOG_FILE") {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let plain_writer = std::sync::Mutex::new(StripAnsiWriter::new(file));
        let file_layer = tracing_subscriber::fmt::layer()
            .with_writer(plain_writer)
            .with_ansi(false)
            .with_target(true)
            .with_level(true)
            .with_thread_ids(false)
            .with_file(false)
            .with_line_number(false)
            .with_filter(filter);
        registry.with(file_layer).init();
        tracing::info!(path = %path, "logging to file");
    } else {
        registry.init();
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    load_dotenv();

    // Log file is only used when LOG_FILE is set (e.g. in .env). Use absolute path if relative path doesn't create file.
    if std::env::var("LOG_FILE").is_err() {
        eprintln!("langgraph-server: LOG_FILE not set, logs only to stdout. Set LOG_FILE=./langgraph-server.log in .env or env to also write to a file.");
    }

    init_tracing()?;

    let mut build_config = ReactBuildConfig::from_env();
    // Prefer OPENAI_API_BASE (langgraph-cli / common .env) if OPENAI_BASE_URL not set.
    if build_config.openai_base_url.is_none() {
        if let Ok(base) = std::env::var("OPENAI_API_BASE") {
            build_config.openai_base_url = Some(base);
        }
    }
    if build_config.thread_id.is_none() {
        build_config.thread_id = Some("default".to_string());
    }
    if build_config.openai_api_key.is_none() || build_config.openai_api_key.as_deref() == Some("") {
        return Err("OPENAI_API_KEY must be set".into());
    }

    let model = build_config
        .model
        .clone()
        .unwrap_or_else(|| "gpt-4o-mini".to_string());
    let db_path = build_config
        .db_path
        .as_deref()
        .unwrap_or("memory.db");
    info!(
        model = %model,
        base_url = ?build_config.openai_base_url,
        thread_id = ?build_config.thread_id,
        user_id = ?build_config.user_id,
        db_path = %db_path,
        "LLM and runtime config loaded"
    );

    let ctx = build_react_run_context(&build_config).await.map_err(|e| e.to_string())?;
    let mut openai_config = async_openai::config::OpenAIConfig::new()
        .with_api_key(build_config.openai_api_key.clone().unwrap_or_default());
    if let Some(ref base) = build_config.openai_base_url {
        // Strip trailing slash so async_openai's url(base + "/chat/completions") does not become .../v4//chat/completions (some backends reject double slash).
        let base = base.trim_end_matches('/');
        openai_config = openai_config.with_api_base(base);
    }
    let llm = langgraph::ChatOpenAI::new_with_tool_source(
        openai_config,
        model.clone(),
        ctx.tool_source.as_ref(),
    )
    .await?;
    let llm: Box<dyn langgraph::LlmClient> = Box::new(llm);

    let runner = ReactRunner::new(
        llm,
        ctx.tool_source,
        ctx.checkpointer,
        ctx.store,
        None,
        None,
        false,
    )?;

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let state = Arc::new(AppState {
        runner: Arc::new(runner),
        openai_base_url: build_config.openai_base_url.clone(),
        openai_api_key: build_config.openai_api_key.clone().unwrap_or_default(),
        http_client,
    });
    let app = Router::new()
        .route("/v1/models", get(models_list))
        .route("/v1/models/:model_id", get(model_retrieve))
        .route("/v1/chat/completions", post(chat_completions))
        .layer(middleware::from_fn(log_request_body))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(|req: &axum::http::Request<axum::body::Body>| {
                    info_span!("request", method = %req.method(), uri = %req.uri())
                }),
        )
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listen = std::env::var("LISTEN").unwrap_or_else(|_| "0.0.0.0:8123".to_string());
    info!("listening on http://{}", listen);
    let listener = tokio::net::TcpListener::bind(&listen).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// Proxies GET /v1/models to the configured OpenAI-compatible base URL.
/// Returns 503 if OPENAI_BASE_URL (or OPENAI_API_BASE) is not set.
async fn models_list(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, ModelsProxyError> {
    let base = state
        .openai_base_url
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or(ModelsProxyError::BaseUrlNotConfigured)?;
    // Same base as chat: base already includes /v1 (e.g. https://api.openai.com/v1), append path only.
    let url = format!("{}/models", base.trim_end_matches('/'));
    let res = state
        .http_client
        .get(&url)
        .header(
            "Authorization",
            format!("Bearer {}", state.openai_api_key),
        )
        .send()
        .await
        .map_err(ModelsProxyError::Upstream)?;
    let status = res.status();
    let content_type = res.headers().get("content-type").cloned();
    let body = res.bytes().await.map_err(ModelsProxyError::Upstream)?;
    let mut response = (status, body).into_response();
    if let Some(ct) = content_type {
        response.headers_mut().insert("content-type", ct);
    }
    Ok(response)
}

/// Proxies GET /v1/models/{model_id} to the configured OpenAI-compatible base URL.
async fn model_retrieve(
    State(state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
) -> Result<impl IntoResponse, ModelsProxyError> {
    let base = state
        .openai_base_url
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or(ModelsProxyError::BaseUrlNotConfigured)?;
    // Same base as chat: base already includes /v1, append path only.
    let url = format!("{}/models/{}", base.trim_end_matches('/'), model_id);
    let res = state
        .http_client
        .get(&url)
        .header(
            "Authorization",
            format!("Bearer {}", state.openai_api_key),
        )
        .send()
        .await
        .map_err(ModelsProxyError::Upstream)?;
    let status = res.status();
    let content_type = res.headers().get("content-type").cloned();
    let body = res.bytes().await.map_err(ModelsProxyError::Upstream)?;
    let mut response = (status, body).into_response();
    if let Some(ct) = content_type {
        response.headers_mut().insert("content-type", ct);
    }
    Ok(response)
}

async fn chat_completions(
    State(state): State<Arc<AppState>>,
    Json(req): Json<langgraph::ChatCompletionRequest>,
) -> Result<impl IntoResponse, ServerError> {
    let runner = Arc::clone(&state.runner);
    if !req.stream {
        return Err(ServerError::BadRequest("only stream: true is supported".into()));
    }

    let parsed = parse_chat_request(&req).map_err(ServerError::from)?;

    // Use a large buffer so content chunks are not dropped when client reads slowly.
    let (tx, rx) = mpsc::channel::<String>(2048);
    let id = format!(
        "chatcmpl-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );
    tracing::debug!(request_id = %id, model = %req.model, "chat completions stream");
    let meta = ChunkMeta {
        id: id.clone(),
        model: req.model.clone(),
        created: None,
    };
    let mut adapter = StreamToSse::new_with_sink(meta, parsed.include_usage, tx);

    let user_message = parsed.user_message.clone();
    let runnable_config = Some(parsed.runnable_config);
    tokio::spawn(async move {
        let res = runner
            .stream_with_config(&user_message, runnable_config, Some(|ev| adapter.feed(ev)))
            .await;
        adapter.finish();
        drop(adapter);
        if let Err(e) = res {
            tracing::error!("stream error: {}", e);
        }
    });

    let stream = ReceiverStream::new(rx).map(|s| Ok::<_, std::io::Error>(Bytes::from(s)));
    let body = Body::from_stream(stream);
    let mut res = (axum::http::StatusCode::OK).into_response();
    res.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("text/event-stream"),
    );
    res.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-cache"),
    );
    *res.body_mut() = body;
    Ok(res)
}

/// Error when proxying /v1/models to upstream. Returns 503 if base URL is not set, 502 on upstream failure.
#[derive(Debug, thiserror::Error)]
pub enum ModelsProxyError {
    #[error("OPENAI_BASE_URL or OPENAI_API_BASE must be set to proxy /v1/models")]
    BaseUrlNotConfigured,
    #[error("upstream request failed: {0}")]
    Upstream(#[from] reqwest::Error),
}

impl IntoResponse for ModelsProxyError {
    fn into_response(self) -> axum::response::Response {
        let (status, msg) = match &self {
            ModelsProxyError::BaseUrlNotConfigured => {
                (StatusCode::SERVICE_UNAVAILABLE, self.to_string())
            }
            ModelsProxyError::Upstream(e) => {
                let status = if e.is_timeout() {
                    StatusCode::GATEWAY_TIMEOUT
                } else {
                    StatusCode::BAD_GATEWAY
                };
                (status, e.to_string())
            }
        };
        (status, Json(serde_json::json!({ "error": { "message": msg } }))).into_response()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),
    #[error("not found: {0}")]
    NotFound(String),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> axum::response::Response {
        let (status, msg) = match &self {
            ServerError::BadRequest(m) => (axum::http::StatusCode::BAD_REQUEST, m.clone()),
            ServerError::Parse(e) => (axum::http::StatusCode::BAD_REQUEST, e.to_string()),
            ServerError::NotFound(m) => (axum::http::StatusCode::NOT_FOUND, m.clone()),
        };
        (status, Json(serde_json::json!({ "error": { "message": msg } }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use langgraph::{MockLlm, MockToolSource, ReactRunner};
    use tower::ServiceExt;

    /// **Scenario**: When OPENAI_BASE_URL is not set, GET /v1/models returns 503.
    #[tokio::test]
    async fn models_list_returns_503_when_base_url_not_configured() {
        let runner = ReactRunner::new(
            Box::new(MockLlm::with_no_tool_calls("ok")),
            Box::new(MockToolSource::get_time_example()),
            None,
            None,
            None,
            None,
            false,
        )
        .expect("compile");
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(1))
            .build()
            .expect("client");
        let state = Arc::new(AppState {
            runner: Arc::new(runner),
            openai_base_url: None,
            openai_api_key: "sk-test".to_string(),
            http_client,
        });
        let app = Router::new()
            .route("/v1/models", get(models_list))
            .route("/v1/models/:model_id", get(model_retrieve))
            .with_state(state);
        let res = app
            .oneshot(Request::get("/v1/models").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}

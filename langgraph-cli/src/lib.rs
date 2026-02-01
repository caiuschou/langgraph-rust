//! langgraph-cli library: reusable ReAct run logic for other crates.
//!
//! Reads OpenAI config from .env, builds think → act → observe graph and runs it, returns final state.
//!
//! ## Usage
//!
//! ```rust,no_run,ignore
//! let state = langgraph_cli::run("user message").await?;
//! for m in &state.messages {
//!     // handle System / User / Assistant messages
//! }
//! ```

use std::sync::Arc;

use async_openai::config::OpenAIConfig;
use langgraph::{
    ActNode, ChatOpenAI, CompiledStateGraph, MockToolSource, Namespace, ObserveNode,
    RunnableConfig, SqliteSaver, SqliteStore, Store, StateGraph, ThinkNode,
    ToolChoiceMode, ToolSource, ToolSourceError, ToolSpec, END, REACT_SYSTEM_PROMPT, START,
};
use serde_json::{json, Value};

mod logging_middleware;
use logging_middleware::LoggingMiddleware;

/// Memory tool source for long-term memory (save, retrieve, list).
struct MemoryToolSource {
    store: Arc<dyn Store>,
    namespace: Namespace,
}

impl MemoryToolSource {
    fn new(store: Arc<dyn Store>, namespace: Namespace) -> Self {
        Self { store, namespace }
    }
}

#[async_trait::async_trait]
impl ToolSource for MemoryToolSource {
    async fn list_tools(&self) -> Result<Vec<ToolSpec>, ToolSourceError> {
        Ok(vec![
            ToolSpec {
                name: "save_memory".to_string(),
                description: Some(
                    "Save information to long-term memory. Use when user says 'remember' or shares preferences."
                        .to_string(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "info": {
                            "type": "string",
                            "description": "Information to remember (e.g., 'name is Alice', 'likes coffee')"
                        }
                    },
                    "required": ["info"]
                }),
            },
            ToolSpec {
                name: "retrieve_memory".to_string(),
                description: Some("Retrieve specific memory by key. Use for questions like 'what's my name'.".to_string()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "key": {
                            "type": "string",
                            "description": "Key to retrieve (e.g., 'name', 'preferences')"
                        }
                    },
                    "required": ["key"]
                }),
            },
            ToolSpec {
                name: "list_memories".to_string(),
                description: Some(
                    "List all stored memories for the user. Use for 'what do you know about me'."
                        .to_string(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                }),
            },
        ])
    }

    async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<langgraph::ToolCallContent, ToolSourceError> {
        match name {
            "save_memory" => {
                let info = arguments["info"].as_str().unwrap_or("").to_string();
                let timestamp = chrono::Utc::now().to_rfc3339();
                let key = format!(
                    "memory_{}",
                    chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
                );
                let value = json!({
                    "info": info,
                    "timestamp": timestamp
                });
                self.store
                    .put(&self.namespace, &key, &value)
                    .await
                    .map_err(|e| ToolSourceError::Transport(e.to_string()))?;
                Ok(langgraph::ToolCallContent {
                    text: format!("Saved to memory: {}", info),
                })
            }
            "retrieve_memory" => {
                let key = arguments["key"].as_str().unwrap_or("");
                let hits = self
                    .store
                    .search(&self.namespace, Some(key), Some(5))
                    .await
                    .map_err(|e| ToolSourceError::Transport(e.to_string()))?;
                if hits.is_empty() {
                    Ok(langgraph::ToolCallContent {
                        text: format!("No memories found for '{}'", key),
                    })
                } else {
                    let memories: Vec<String> = hits
                        .iter()
                        .map(|h| h.value["info"].as_str().unwrap_or("").to_string())
                        .collect();
                    Ok(langgraph::ToolCallContent {
                        text: format!("Found memories: {}", memories.join(", ")),
                    })
                }
            }
            "list_memories" => {
                let keys = self
                    .store
                    .list(&self.namespace)
                    .await
                    .map_err(|e| ToolSourceError::Transport(e.to_string()))?;
                let mut memories = Vec::new();
                for key in keys {
                    if let Some(value) = self
                        .store
                        .get(&self.namespace, &key)
                        .await
                        .map_err(|e| ToolSourceError::Transport(e.to_string()))?
                    {
                        if let Some(info) = value["info"].as_str() {
                            memories.push(info.to_string());
                        }
                    }
                }
                if memories.is_empty() {
                    Ok(langgraph::ToolCallContent {
                        text: "No memories stored yet. Tell me something to remember!".to_string(),
                    })
                } else {
                    Ok(langgraph::ToolCallContent {
                        text: format!("I remember: {}", memories.join("; ")),
                    })
                }
            }
            _ => Err(ToolSourceError::NotFound(format!("Unknown tool: {}", name))),
        }
    }
}

/// Extension trait for fluent API: attach node logging middleware then compile.
/// Example of extending the build chain from outside langgraph; see idea/NODE_MIDDLEWARE_OPTIONS.md.
pub trait WithNodeLogging {
    /// Returns the same graph with `LoggingMiddleware` attached. Chain with `.compile()?`.
    fn with_node_logging(self) -> Self;
}

impl WithNodeLogging for StateGraph<ReActState> {
    fn with_node_logging(self) -> Self {
        self.with_middleware(Arc::new(LoggingMiddleware))
    }
}

/// Public types for callers to handle `run` return value.
pub use langgraph::{Message, ReActState};

/// Error type used internally.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// Run config: API base, key, model, temperature, tool_choice. Can be filled from env / .env.
#[derive(Clone, Debug)]
pub struct RunConfig {
    /// OpenAI API base URL, e.g. `https://api.openai.com/v1`.
    pub api_base: String,
    /// OpenAI API key.
    pub api_key: String,
    /// Model name, e.g. `gpt-4o-mini`.
    pub model: String,
    /// Sampling temperature 0–2, lower is more deterministic. Default: unset (use API default).
    pub temperature: Option<f32>,
    /// Tool choice mode: auto (model chooses), none (no tools), required (must use tools).
    pub tool_choice: Option<ToolChoiceMode>,
    /// Embeddings API key. If not set, uses OPENAI_API_KEY.
    pub embedding_api_key: Option<String>,
    /// Embeddings API base URL. If not set, uses OPENAI_API_BASE.
    pub embedding_api_base: Option<String>,
    /// Embeddings model name, e.g. `text-embedding-3-small`.
    pub embedding_model: Option<String>,
    /// Thread ID for short-term memory (checkpointer). Required for persistence.
    pub thread_id: Option<String>,
    /// User ID for long-term memory (store). Used for multi-tenant isolation.
    pub user_id: Option<String>,
    /// SQLite database path for persistence. Defaults to "memory.db".
    pub db_path: Option<String>,
    /// Use Exa MCP for web search. Enables Exa's remote MCP server.
    pub use_exa_mcp: bool,
    /// Exa API key for MCP authentication.
    pub exa_api_key: Option<String>,
    /// Exa MCP server URL. Default: `https://mcp.exa.ai/mcp`.
    pub mcp_exa_url: String,
    /// Command for mcp-remote (stdio→HTTP bridge). Default: `npx`.
    pub mcp_remote_cmd: String,
    /// Args for mcp-remote, e.g. `-y mcp-remote`. Default: `-y mcp-remote`.
    pub mcp_remote_args: String,
}

impl RunConfig {
    /// Get the effective embedding API key (falls back to OPENAI_API_KEY if not set).
    pub fn embedding_api_key(&self) -> &str {
        self.embedding_api_key.as_deref().unwrap_or(&self.api_key)
    }

    /// Get the effective embedding API base URL (falls back to OPENAI_API_BASE if not set).
    pub fn embedding_api_base(&self) -> &str {
        self.embedding_api_base.as_deref().unwrap_or(&self.api_base)
    }

    /// Get the embedding model name (defaults to text-embedding-3-small).
    pub fn embedding_model(&self) -> &str {
        self.embedding_model
            .as_deref()
            .unwrap_or("text-embedding-3-small")
    }

    #[cfg(all(feature = "embedding", feature = "openai"))]
    /// Create an OpenAIEmbedder from this configuration.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use langgraph_cli::RunConfig;
    /// use langgraph::OpenAIEmbedder;
    ///
    /// let config = RunConfig::from_env()?;
    /// let embedder = config.create_embedder();
    /// ```
    pub fn create_embedder(&self) -> langgraph::OpenAIEmbedder {
        use async_openai::config::OpenAIConfig;
        let openai_config = OpenAIConfig::new()
            .with_api_key(self.embedding_api_key())
            .with_api_base(self.embedding_api_base());
        langgraph::OpenAIEmbedder::with_config(openai_config, self.embedding_model())
    }
}

impl RunConfig {
    /// Fill config from env vars (and .env). Requires `dotenv::dotenv().ok()` or load inside `run()`.
    ///
    /// `OPENAI_API_KEY` required; `OPENAI_API_BASE`, `OPENAI_MODEL` have defaults.
    /// `OPENAI_TEMPERATURE`, `OPENAI_TOOL_CHOICE` (auto|none|required) optional.
    /// For embeddings: `EMBEDDING_API_KEY`, `EMBEDDING_API_BASE`, `EMBEDDING_MODEL` optional.
    /// For memory: `THREAD_ID`, `USER_ID`, `DB_PATH` optional.
    /// For Exa MCP: `USE_EXA_MCP`, `EXA_API_KEY`, `MCP_EXA_URL`, `MCP_REMOTE_CMD`, `MCP_REMOTE_ARGS` optional.
    pub fn from_env() -> Result<Self, Error> {
        let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "OPENAI_API_KEY is not set; please configure it in .env",
            )
        })?;
        let api_base = std::env::var("OPENAI_API_BASE")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
        let temperature = std::env::var("OPENAI_TEMPERATURE")
            .ok()
            .and_then(|s| s.parse().ok());
        let tool_choice = std::env::var("OPENAI_TOOL_CHOICE")
            .ok()
            .and_then(|s| s.parse().ok());
        let embedding_api_key = std::env::var("EMBEDDING_API_KEY").ok();
        let embedding_api_base = std::env::var("EMBEDDING_API_BASE").ok();
        let embedding_model = std::env::var("EMBEDDING_MODEL")
            .ok()
            .or_else(|| Some("text-embedding-3-small".to_string()));
        let thread_id = std::env::var("THREAD_ID").ok();
        let user_id = std::env::var("USER_ID").ok();
        let db_path = std::env::var("DB_PATH").ok();
        let exa_api_key = std::env::var("EXA_API_KEY").ok();
        let use_exa_mcp = std::env::var("USE_EXA_MCP")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| exa_api_key.is_some());
        let mcp_exa_url = std::env::var("MCP_EXA_URL")
            .unwrap_or_else(|_| "https://mcp.exa.ai/mcp".to_string());
        let mcp_remote_cmd = std::env::var("MCP_REMOTE_CMD")
            .unwrap_or_else(|_| "npx".to_string());
        let mcp_remote_args = std::env::var("MCP_REMOTE_ARGS")
            .unwrap_or_else(|_| "-y mcp-remote".to_string());
        Ok(Self {
            api_base,
            api_key,
            model,
            temperature,
            tool_choice,
            embedding_api_key,
            embedding_api_base,
            embedding_model,
            thread_id,
            user_id,
            db_path,
            use_exa_mcp,
            exa_api_key,
            mcp_exa_url,
            mcp_remote_cmd,
            mcp_remote_args,
        })
    }
}

/// Run ReAct graph with default config (from .env), returns final state.
///
/// Loads `.env` internally, then calls `run_with_config`.
pub async fn run(user_message: &str) -> Result<ReActState, Error> {
    dotenv::dotenv().ok();
    let config = RunConfig::from_env()?;
    run_with_config(&config, user_message).await
}

/// Run ReAct graph with given config; does not read .env, returns final state.
#[cfg(feature = "sqlite")]
pub async fn run_with_config(config: &RunConfig, user_message: &str) -> Result<ReActState, Error> {
    let openai_config = OpenAIConfig::new()
        .with_api_base(&config.api_base)
        .with_api_key(config.api_key.clone());

    let db_path = config.db_path.as_deref().unwrap_or("memory.db");

    let checkpointer = if config.thread_id.is_some() {
        let serializer = Arc::new(langgraph::JsonSerializer);
        Some(Arc::new(SqliteSaver::new(db_path, serializer)?) as Arc<dyn langgraph::Checkpointer<ReActState>>)
    } else {
        None
    };

    let store = if config.user_id.is_some() {
        Some(Arc::new(SqliteStore::new(db_path)?) as Arc<dyn langgraph::Store>)
    } else {
        None
    };

    let tool_source: Box<dyn ToolSource> = if config.use_exa_mcp {
        #[cfg(feature = "mcp")]
        {
            let args: Vec<String> = config.mcp_remote_args.split_whitespace().map(String::from).collect();
            let mut args = args;
            if !args.iter().any(|a| a == &config.mcp_exa_url || a.contains("mcp.exa.ai")) {
                args.push(config.mcp_exa_url.clone());
            }
            if let Some(ref key) = config.exa_api_key {
                let mut env = vec![("EXA_API_KEY".to_string(), key.clone())];
                if let Ok(home) = std::env::var("HOME") {
                    env.push(("HOME".to_string(), home));
                }
                Box::new(langgraph::McpToolSource::new_with_env(
                    config.mcp_remote_cmd.clone(),
                    args,
                    env,
                )?)
            } else {
                Box::new(langgraph::McpToolSource::new(config.mcp_remote_cmd.clone(), args)?)
            }
        }
        #[cfg(not(feature = "mcp"))]
        {
            return Err("MCP feature is not enabled. Build with --features mcp".into());
        }
    } else if let Some(user_id) = &config.user_id {
        if let Some(s) = &store {
            let namespace = vec![user_id.clone(), "memories".to_string()];
            Box::new(MemoryToolSource::new(s.clone(), namespace))
        } else {
            Box::new(MockToolSource::get_time_example())
        }
    } else {
        Box::new(MockToolSource::get_time_example())
    };

    let tools = tool_source.list_tools().await?;
    let mut llm = ChatOpenAI::with_config(openai_config, config.model.clone()).with_tools(tools);
    if let Some(t) = config.temperature {
        llm = llm.with_temperature(t);
    }
    if let Some(tc) = config.tool_choice {
        llm = llm.with_tool_choice(tc);
    }
    let think = ThinkNode::new(Box::new(llm));
    let act = ActNode::new(tool_source);
    let observe = ObserveNode::new();

    let mut graph = StateGraph::<ReActState>::new();

    if let Some(s) = store {
        graph = graph.with_store(s);
    }

    graph
        .add_node("think", Arc::new(think))
        .add_node("act", Arc::new(act))
        .add_node("observe", Arc::new(observe))
        .add_edge(START, "think")
        .add_edge("think", "act")
        .add_edge("act", "observe")
        .add_edge("observe", END);

    let compiled: CompiledStateGraph<ReActState> = if let Some(cp) = checkpointer {
        graph.with_node_logging().compile_with_checkpointer(cp)?
    } else {
        graph.with_node_logging().compile()?
    };

    let runnable_config = if config.thread_id.is_some() || config.user_id.is_some() {
        Some(RunnableConfig {
            thread_id: config.thread_id.clone(),
            checkpoint_id: None,
            checkpoint_ns: String::new(),
            user_id: config.user_id.clone(),
        })
    } else {
        None
    };

    let state = ReActState {
        messages: vec![
            Message::system(REACT_SYSTEM_PROMPT),
            Message::user(user_message.to_string()),
        ],
        tool_calls: vec![],
        tool_results: vec![],
    };

    let final_state = compiled.invoke(state, runnable_config).await?;
    Ok(final_state)
}

/// Run ReAct graph with given config; does not read .env, returns final state.
#[cfg(not(feature = "sqlite"))]
pub async fn run_with_config(config: &RunConfig, user_message: &str) -> Result<ReActState, Error> {
    let openai_config = OpenAIConfig::new()
        .with_api_base(&config.api_base)
        .with_api_key(config.api_key.clone());

    let tool_source: Box<dyn ToolSource> = if config.use_exa_mcp {
        #[cfg(feature = "mcp")]
        {
            let args: Vec<String> = config.mcp_remote_args.split_whitespace().map(String::from).collect();
            let mut args = args;
            if !args.iter().any(|a| a == &config.mcp_exa_url || a.contains("mcp.exa.ai")) {
                args.push(config.mcp_exa_url.clone());
            }
            if let Some(ref key) = config.exa_api_key {
                let mut env = vec![("EXA_API_KEY".to_string(), key.clone())];
                if let Ok(home) = std::env::var("HOME") {
                    env.push(("HOME".to_string(), home));
                }
                Box::new(langgraph::McpToolSource::new_with_env(
                    config.mcp_remote_cmd.clone(),
                    args,
                    env,
                )?)
            } else {
                Box::new(langgraph::McpToolSource::new(config.mcp_remote_cmd.clone(), args)?)
            }
        }
        #[cfg(not(feature = "mcp"))]
        {
            return Err("MCP feature is not enabled. Build with --features mcp".into());
        }
    } else {
        Box::new(MockToolSource::get_time_example())
    };

    let tools = tool_source.list_tools().await?;
    let mut llm = ChatOpenAI::with_config(openai_config, config.model.clone()).with_tools(tools);
    if let Some(t) = config.temperature {
        llm = llm.with_temperature(t);
    }
    if let Some(tc) = config.tool_choice {
        llm = llm.with_tool_choice(tc);
    }
    let think = ThinkNode::new(Box::new(llm));
    let act = ActNode::new(Box::new(tool_source));
    let observe = ObserveNode::new();

    let mut graph = StateGraph::<ReActState>::new();
    graph
        .add_node("think", Arc::new(think))
        .add_node("act", Arc::new(act))
        .add_node("observe", Arc::new(observe))
        .add_edge(START, "think")
        .add_edge("think", "act")
        .add_edge("act", "observe")
        .add_edge("observe", END);

    let compiled: CompiledStateGraph<ReActState> = graph.with_node_logging().compile()?;

    let state = ReActState {
        messages: vec![
            Message::system(REACT_SYSTEM_PROMPT),
            Message::user(user_message.to_string()),
        ],
        tool_calls: vec![],
        tool_results: vec![],
    };

    let final_state = compiled.invoke(state, None).await?;
    Ok(final_state)
}

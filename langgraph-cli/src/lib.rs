//! langgraph-cli library: reusable ReAct run logic for other crates.
//!
//! Reads OpenAI config from .env, builds think → act → observe graph and runs it, returns final state.
//!
//! ## Usage
//!
//! ```rust,no_run
//! let state = langgraph_cli::run("user message").await?;
//! for m in &state.messages {
//!     // handle System / User / Assistant messages
//! }
//! ```

use std::sync::Arc;

use async_openai::config::OpenAIConfig;
use langgraph::{
    ActNode, ChatOpenAI, CompiledStateGraph, MockToolSource, NodeMiddleware, ObserveNode,
    REACT_SYSTEM_PROMPT, StateGraph, ThinkNode, ToolChoiceMode, ToolSource,
};

mod logging_middleware;
use logging_middleware::LoggingMiddleware;

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
}

impl RunConfig {
    /// Fill config from env vars (and .env). Requires `dotenv::dotenv().ok()` or load inside `run()`.
    ///
    /// `OPENAI_API_KEY` required; `OPENAI_API_BASE`, `OPENAI_MODEL` have defaults.
    /// `OPENAI_TEMPERATURE`, `OPENAI_TOOL_CHOICE` (auto|none|required) optional.
    pub fn from_env() -> Result<Self, Error> {
        let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "OPENAI_API_KEY is not set; please configure it in .env",
            )
        })?;
        let api_base = std::env::var("OPENAI_API_BASE")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        let model =
            std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
        let temperature = std::env::var("OPENAI_TEMPERATURE")
            .ok()
            .and_then(|s| s.parse().ok());
        let tool_choice = std::env::var("OPENAI_TOOL_CHOICE")
            .ok()
            .and_then(|s| s.parse().ok());
        Ok(Self {
            api_base,
            api_key,
            model,
            temperature,
            tool_choice,
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
pub async fn run_with_config(config: &RunConfig, user_message: &str) -> Result<ReActState, Error> {
    let openai_config = OpenAIConfig::new()
        .with_api_base(&config.api_base)
        .with_api_key(config.api_key.clone());

    let tool_source = MockToolSource::get_time_example();
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
        .add_edge("think")
        .add_edge("act")
        .add_edge("observe");

    let middleware: Arc<dyn NodeMiddleware<ReActState>> = Arc::new(LoggingMiddleware);
    let compiled: CompiledStateGraph<ReActState> = graph.compile_with_middleware(middleware)?;

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

//! Builds checkpointer, store, runnable_config and tool_source from [`ReactBuildConfig`](super::config::ReactBuildConfig).
//!
//! Used by CLI or other callers that hold a [`ReactBuildConfig`](super::config::ReactBuildConfig).
//! Requires `sqlite` and `mcp` features (SqliteSaver, SqliteStore, McpToolSource).

mod context;
mod error;
mod llm;
mod store;
mod tool_source;

use std::sync::Arc;

use crate::error::AgentError;
use crate::memory::{JsonSerializer, RunnableConfig, SqliteSaver};
use crate::react::ReactRunner;
use crate::state::ReActState;
use crate::LlmClient;

use super::config::ReactBuildConfig;
use llm::build_default_llm;
use store::build_store;
use tool_source::build_tool_source;

pub use context::ReactRunContext;
pub use error::BuildRunnerError;

fn to_agent_error(e: impl std::fmt::Display) -> AgentError {
    AgentError::ExecutionFailed(e.to_string())
}

/// Builds checkpointer when thread_id is set; otherwise returns None.
fn build_checkpointer(
    config: &ReactBuildConfig,
    db_path: &str,
) -> Result<Option<Arc<dyn crate::memory::Checkpointer<ReActState>>>, AgentError> {
    if config.thread_id.is_none() {
        return Ok(None);
    }
    let serializer = Arc::new(JsonSerializer);
    let saver = SqliteSaver::new(db_path, serializer).map_err(to_agent_error)?;
    Ok(Some(
        Arc::new(saver) as Arc<dyn crate::memory::Checkpointer<ReActState>>
    ))
}

/// Builds runnable_config when thread_id or user_id is set; otherwise returns None.
fn build_runnable_config(config: &ReactBuildConfig) -> Option<RunnableConfig> {
    if config.thread_id.is_none() && config.user_id.is_none() {
        return None;
    }
    Some(RunnableConfig {
        thread_id: config.thread_id.clone(),
        checkpoint_id: None,
        checkpoint_ns: String::new(),
        user_id: config.user_id.clone(),
    })
}

/// Builds checkpointer, store, runnable_config and tool_source from the given config.
///
/// Requires `sqlite` and `mcp` features. Callers (e.g. langgraph-cli) build [`ReactBuildConfig`](super::config::ReactBuildConfig)
/// from their own config and pass it here.
pub async fn build_react_run_context(
    config: &ReactBuildConfig,
) -> Result<ReactRunContext, AgentError> {
    let db_path = config.db_path.as_deref().unwrap_or("memory.db");

    let checkpointer = build_checkpointer(config, db_path)?;
    let store = build_store(config, db_path)?;
    let runnable_config = build_runnable_config(config);
    let tool_source = build_tool_source(config, &store).await?;

    Ok(ReactRunContext {
        checkpointer,
        store,
        runnable_config,
        tool_source,
    })
}

/// Builds a [`ReactRunner`](crate::react::ReactRunner) from config and optional LLM.
///
/// When `llm` is `Some`, that client is used. When `llm` is `None`, the library builds a default
/// LLM from config if `openai_api_key` and `model` (or env) are set (requires `openai` feature);
/// otherwise returns [`BuildRunnerError::NoLlm`].
///
/// Uses [`build_react_run_context`](build_react_run_context) for persistence and tool source,
/// then compiles the ReAct graph with optional checkpointer and passes `config.system_prompt`
/// into the runner for initial state.
pub async fn build_react_runner(
    config: &ReactBuildConfig,
    llm: Option<Box<dyn LlmClient>>,
    verbose: bool,
) -> Result<ReactRunner, BuildRunnerError> {
    let ctx = build_react_run_context(config).await?;
    let llm = match llm {
        Some(l) => l,
        None => build_default_llm(config)?,
    };
    let runner = ReactRunner::new(
        llm,
        ctx.tool_source,
        ctx.checkpointer,
        ctx.store,
        ctx.runnable_config,
        config.system_prompt.clone(),
        verbose,
    )?;
    Ok(runner)
}

/// Builds a [`ReactRunner`](crate::react::ReactRunner) with an OpenAI client from explicit config and model.
///
/// Convenience when you already have an [`OpenAIConfig`](async_openai::config::OpenAIConfig).
pub async fn build_react_runner_with_openai(
    config: &ReactBuildConfig,
    openai_config: async_openai::config::OpenAIConfig,
    model: impl Into<String>,
    verbose: bool,
) -> Result<ReactRunner, BuildRunnerError> {
    use crate::llm::ChatOpenAI;
    let client = ChatOpenAI::with_config(openai_config, model);
    build_react_runner(config, Some(Box::new(client)), verbose).await
}

//! Builds checkpointer, store, runnable_config and tool_source from [`ReactBuildConfig`](super::config::ReactBuildConfig).
//!
//! Used by CLI or other callers that hold a [`ReactBuildConfig`](super::config::ReactBuildConfig).
//! Requires `sqlite` and `mcp` features (SqliteSaver, SqliteStore, McpToolSource).

use std::sync::Arc;

use crate::error::AgentError;
use crate::graph::CompilationError;
use crate::memory::{JsonSerializer, RunnableConfig, SqliteSaver};
use crate::react::ReactRunner;
use crate::state::ReActState;
use crate::tool_source::McpToolSource;
use crate::tool_source::{MemoryToolsSource, MockToolSource, ToolSource};
use crate::tools::{register_mcp_tools, AggregateToolSource};
use crate::LlmClient;

use super::config::ReactBuildConfig;

/// Error when building a [`ReactRunner`](crate::react::ReactRunner) from config.
#[derive(Debug, thiserror::Error)]
pub enum BuildRunnerError {
    #[error("failed to build run context: {0}")]
    Context(#[from] AgentError),
    #[error("compilation failed: {0}")]
    Compilation(#[from] CompilationError),
    #[error("no LLM provided and config has no openai_api_key/model; pass Some(llm) or set OPENAI_API_KEY and OPENAI_MODEL")]
    NoLlm,
}

/// Context for running the ReAct graph: persistence (checkpointer, store, runnable_config)
/// and tool source built from config.
pub struct ReactRunContext {
    /// Checkpointer when thread_id is set; None otherwise.
    pub checkpointer: Option<Arc<dyn crate::memory::Checkpointer<ReActState>>>,
    /// Store when user_id is set; None otherwise.
    pub store: Option<Arc<dyn crate::memory::Store>>,
    /// RunnableConfig when thread_id or user_id is set; None otherwise.
    pub runnable_config: Option<RunnableConfig>,
    /// Tool source (Mock or Aggregate with optional Memory + MCP Exa).
    pub tool_source: Box<dyn ToolSource>,
}

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

/// Builds store when user_id is set and embedder config is available; otherwise returns None.
/// When embedding is configured (and `in-memory-vector` + `openai` features), uses
/// InMemoryVectorStore for semantic long-term memory. When embedding is not available,
/// long-term memory is disabled (no store, no memory tools) per design.
fn build_store(
    config: &ReactBuildConfig,
    _db_path: &str,
) -> Result<Option<Arc<dyn crate::memory::Store>>, AgentError> {
    if config.user_id.is_none() {
        return Ok(None);
    }
    match build_vector_store(config) {
        Ok(store) => Ok(Some(store)),
        Err(_) => Ok(None),
    }
}

fn build_vector_store(
    config: &ReactBuildConfig,
) -> Result<Arc<dyn crate::memory::Store>, AgentError> {
    use async_openai::config::OpenAIConfig;
    use crate::memory::{InMemoryVectorStore, OpenAIEmbedder};

    let api_key = config
        .embedding_api_key
        .as_deref()
        .or(config.openai_api_key.as_deref())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AgentError::ExecutionFailed("embedding requires EMBEDDING_API_KEY or OPENAI_API_KEY".into()))?;
    let model = config
        .embedding_model
        .as_deref()
        .or(config.model.as_deref())
        .filter(|s| !s.is_empty())
        .unwrap_or("text-embedding-3-small");
    let mut openai_config = OpenAIConfig::new().with_api_key(api_key);
    let base = config
        .embedding_base_url
        .as_deref()
        .or(config.openai_base_url.as_deref());
    if let Some(b) = base.filter(|s| !s.is_empty()) {
        openai_config = openai_config.with_api_base(b);
    }
    let embedder = OpenAIEmbedder::with_config(openai_config, model);
    let store = InMemoryVectorStore::new(Arc::new(embedder));
    Ok(Arc::new(store) as Arc<dyn crate::memory::Store>)
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

/// Registers MCP Exa tools on the aggregate when exa_api_key is set.
async fn register_exa_mcp(
    config: &ReactBuildConfig,
    aggregate: &AggregateToolSource,
) -> Result<(), AgentError> {
    let key = match config.exa_api_key.as_ref() {
        Some(k) => k,
        None => return Ok(()),
    };
    let args: Vec<String> = config
        .mcp_remote_args
        .split_whitespace()
        .map(String::from)
        .collect();
    let mut args = args;
    if !args
        .iter()
        .any(|a| a == &config.mcp_exa_url || a.contains("mcp.exa.ai"))
    {
        args.push(config.mcp_exa_url.clone());
    }
    let mut env = vec![("EXA_API_KEY".to_string(), key.clone())];
    if let Ok(home) = std::env::var("HOME") {
        env.push(("HOME".to_string(), home));
    }
    let mcp = McpToolSource::new_with_env(
        config.mcp_remote_cmd.clone(),
        args,
        env,
        config.mcp_verbose,
    )
    .map_err(to_agent_error)?;
    register_mcp_tools(aggregate, Arc::new(mcp))
        .await
        .map_err(to_agent_error)?;
    Ok(())
}

/// Builds tool source: MockToolSource when no memory and no Exa; otherwise AggregateToolSource
/// with optional MemoryToolsSource and optional MCP Exa.
async fn build_tool_source(
    config: &ReactBuildConfig,
    store: &Option<Arc<dyn crate::memory::Store>>,
) -> Result<Box<dyn ToolSource>, AgentError> {
    let has_memory = config.user_id.is_some() && store.is_some();
    let has_exa = config.exa_api_key.is_some();

    if !has_memory && !has_exa {
        return Ok(Box::new(MockToolSource::get_time_example()));
    }

    let aggregate = if has_memory {
        let user_id = config.user_id.as_ref().unwrap();
        let s = store.as_ref().unwrap();
        let namespace = vec![user_id.clone(), "memories".to_string()];
        MemoryToolsSource::new(s.clone(), namespace).await
    } else {
        AggregateToolSource::new()
    };

    register_exa_mcp(config, &aggregate).await?;

    Ok(Box::new(aggregate))
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

/// Builds default OpenAI LLM from config when `openai_api_key` and `model` are set.
fn build_default_llm(config: &ReactBuildConfig) -> Result<Box<dyn LlmClient>, BuildRunnerError> {
    use async_openai::config::OpenAIConfig;
    use crate::llm::ChatOpenAI;

    let api_key = config
        .openai_api_key
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or(BuildRunnerError::NoLlm)?;
    let model = config
        .model
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("gpt-4o-mini");
    let mut openai_config = OpenAIConfig::new().with_api_key(api_key);
    if let Some(ref base) = config.openai_base_url {
        if !base.is_empty() {
            openai_config = openai_config.with_api_base(base);
        }
    }
    let client = ChatOpenAI::with_config(openai_config, model);
    Ok(Box::new(client))
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

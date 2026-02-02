//! Builds checkpointer, store, runnable_config and tool_source from [`ReactBuildConfig`](super::config::ReactBuildConfig).
//!
//! Used by CLI or other callers that hold a [`ReactBuildConfig`](super::config::ReactBuildConfig).
//! Requires `sqlite` and `mcp` features (SqliteSaver, SqliteStore, McpToolSource).

use std::sync::Arc;

use crate::error::AgentError;
use crate::memory::{JsonSerializer, RunnableConfig, SqliteSaver, SqliteStore};
use crate::state::ReActState;
use crate::tool_source::McpToolSource;
use crate::tool_source::{MemoryToolsSource, MockToolSource, ToolSource};
use crate::tools::{register_mcp_tools, AggregateToolSource};

use super::config::ReactBuildConfig;

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

/// Builds store when user_id is set; otherwise returns None.
fn build_store(
    config: &ReactBuildConfig,
    db_path: &str,
) -> Result<Option<Arc<dyn crate::memory::Store>>, AgentError> {
    if config.user_id.is_none() {
        return Ok(None);
    }
    let store = SqliteStore::new(db_path).map_err(to_agent_error)?;
    Ok(Some(Arc::new(store) as Arc<dyn crate::memory::Store>))
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
    let mcp = McpToolSource::new_with_env(config.mcp_remote_cmd.clone(), args, env)
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

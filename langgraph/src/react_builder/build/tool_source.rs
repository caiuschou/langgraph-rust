//! Builds tool source from [`ReactBuildConfig`](super::super::config::ReactBuildConfig).
//!
//! Always includes web_fetcher (WebToolsSource). When no memory and no Exa, returns
//! an `AggregateToolSource` with only web_fetcher; otherwise `AggregateToolSource`
//! with optional `MemoryToolsSource`, optional MCP Exa, and web_fetcher.

use std::sync::Arc;

use crate::error::AgentError;
use crate::tool_source::{MemoryToolsSource, ToolSource, WebToolsSource};
use crate::tools::{register_mcp_tools, AggregateToolSource, WebFetcherTool};

use crate::tool_source::McpToolSource;

use super::super::config::ReactBuildConfig;

fn to_agent_error(e: impl std::fmt::Display) -> AgentError {
    AgentError::ExecutionFailed(e.to_string())
}

/// Default namespace for long-term memory when no user_id is set (default-on behavior).
const DEFAULT_MEMORY_NAMESPACE: &[&str] = &["default", "memories"];

/// Registers MCP Exa tools on the aggregate when exa_api_key is set.
/// Prefers HTTP when `mcp_exa_url` is http(s); otherwise uses mcp-remote (stdio).
async fn register_exa_mcp(
    config: &ReactBuildConfig,
    aggregate: &AggregateToolSource,
) -> Result<(), AgentError> {
    let key = match config.exa_api_key.as_ref() {
        Some(k) => k,
        None => return Ok(()),
    };
    let url = config.mcp_exa_url.trim();
    let use_http = url.starts_with("http://") || url.starts_with("https://");

    let mcp = if use_http {
        McpToolSource::new_http(url, [("EXA_API_KEY", key.as_str())])
            .await
            .map_err(to_agent_error)?
    } else {
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
        McpToolSource::new_with_env(
            config.mcp_remote_cmd.clone(),
            args,
            env,
            config.mcp_verbose,
        )
        .map_err(to_agent_error)?
    };
    register_mcp_tools(aggregate, Arc::new(mcp))
        .await
        .map_err(to_agent_error)?;
    Ok(())
}

/// Builds tool source: MockToolSource when no memory and no Exa; otherwise AggregateToolSource
/// with optional MemoryToolsSource and optional MCP Exa.
/// Long-term memory is enabled by default when store is available; namespace is
/// `[user_id, "memories"]` when config.user_id is set, else `["default", "memories"]`.
pub(crate) async fn build_tool_source(
    config: &ReactBuildConfig,
    store: &Option<Arc<dyn crate::memory::Store>>,
) -> Result<Box<dyn ToolSource>, AgentError> {
    let has_memory = store.is_some();
    let has_exa = config.exa_api_key.is_some();

    if !has_memory && !has_exa {
        return Ok(Box::new(WebToolsSource::new().await));
    }

    let aggregate = if has_memory {
        let s = store.as_ref().unwrap();
        let namespace: Vec<String> = config
            .user_id
            .as_ref()
            .map(|u| vec![u.clone(), "memories".to_string()])
            .unwrap_or_else(|| DEFAULT_MEMORY_NAMESPACE.iter().map(|s| (*s).to_string()).collect());
        MemoryToolsSource::new(s.clone(), namespace).await
    } else {
        AggregateToolSource::new()
    };

    aggregate.register_sync(Box::new(WebFetcherTool::new()));
    register_exa_mcp(config, &aggregate).await?;

    Ok(Box::new(aggregate))
}

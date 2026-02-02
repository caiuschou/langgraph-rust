//! Run ReAct graph with given config (no SQLite); does not read .env, returns final state.
//!
//! No checkpointer or store. Single AggregateToolSource when use_exa_mcp and exa_api_key
//! are set; otherwise MockToolSource. See docs/rust-langgraph/tools-refactor/architecture/common-interface-mcp.md.

use std::sync::Arc;

use async_openai::config::OpenAIConfig;
use langgraph::{ChatOpenAI, MockToolSource, ToolSource};

use crate::config::RunConfig;

use super::common::run_react_graph;
use super::Error;

/// Run ReAct graph with given config; does not read .env, returns final state.
pub async fn run_with_config(
    config: &RunConfig,
    user_message: &str,
) -> Result<langgraph::ReActState, Error> {
    let openai_config = OpenAIConfig::new()
        .with_api_base(&config.api_base)
        .with_api_key(config.api_key.clone());

    let has_exa = config.use_exa_mcp && config.exa_api_key.is_some();

    let tool_source: Box<dyn ToolSource> = if !has_exa {
        Box::new(MockToolSource::get_time_example())
    } else {
        #[cfg(feature = "mcp")]
        {
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
            let key = config.exa_api_key.as_ref().unwrap();
            let mut env = vec![("EXA_API_KEY".to_string(), key.clone())];
            if let Ok(home) = std::env::var("HOME") {
                env.push(("HOME".to_string(), home));
            }
            let mcp = langgraph::McpToolSource::new_with_env(
                config.mcp_remote_cmd.clone(),
                args,
                env,
            )?;
            let aggregate = langgraph::tools::AggregateToolSource::new();
            langgraph::register_mcp_tools(&aggregate, Arc::new(mcp)).await?;
            Box::new(aggregate)
        }
        #[cfg(not(feature = "mcp"))]
        {
            return Err("MCP feature is not enabled. Build with --features mcp".into());
        }
    };

    let mut llm =
        ChatOpenAI::new_with_tool_source(openai_config, config.model.clone(), tool_source.as_ref())
            .await?;
    if let Some(t) = config.temperature {
        llm = llm.with_temperature(t);
    }
    if let Some(tc) = config.tool_choice {
        llm = llm.with_tool_choice(tc);
    }
    let llm: Box<dyn langgraph::LlmClient> = Box::new(llm);
    run_react_graph(
        user_message,
        llm,
        tool_source,
        None,
        None,
        None,
    )
    .await
}

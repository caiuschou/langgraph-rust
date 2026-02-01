//! Run ReAct graph with given config (no SQLite); does not read .env, returns final state.
//!
//! No checkpointer or store. Interacts with [`RunConfig`](crate::config::RunConfig),
//! [`WithNodeLogging`](crate::middleware::WithNodeLogging).

use std::sync::Arc;

use async_openai::config::OpenAIConfig;
use langgraph::{
    ActNode, ChatOpenAI, CompiledStateGraph, MockToolSource, ObserveNode, StateGraph, ThinkNode,
    ToolSource, END, REACT_SYSTEM_PROMPT, START,
};
use langgraph::{Message, ReActState};

use crate::config::RunConfig;
use crate::middleware::WithNodeLogging;

use super::Error;

/// Run ReAct graph with given config; does not read .env, returns final state.
pub async fn run_with_config(config: &RunConfig, user_message: &str) -> Result<ReActState, Error> {
    let openai_config = OpenAIConfig::new()
        .with_api_base(&config.api_base)
        .with_api_key(config.api_key.clone());

    let tool_source: Box<dyn ToolSource> = if config.use_exa_mcp {
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
                Box::new(langgraph::McpToolSource::new(
                    config.mcp_remote_cmd.clone(),
                    args,
                )?)
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

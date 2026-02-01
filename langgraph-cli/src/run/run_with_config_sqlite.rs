//! Run ReAct graph with given config (SQLite feature enabled); does not read .env, returns final state.
//!
//! Uses checkpointer and store when thread_id/user_id are set.
//! Interacts with [`RunConfig`](crate::config::RunConfig),
//! [`StoreToolSource`](langgraph::StoreToolSource),
//! [`WithNodeLogging`](crate::middleware::WithNodeLogging).

use std::sync::Arc;

use async_openai::config::OpenAIConfig;
use langgraph::{
    ActNode, ChatOpenAI, CompiledStateGraph, MockToolSource, ObserveNode, RunnableConfig,
    SqliteSaver, SqliteStore, StateGraph, StoreToolSource, ThinkNode, ToolSource, END,
    REACT_SYSTEM_PROMPT, START,
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

    let db_path = config.db_path.as_deref().unwrap_or("memory.db");

    let checkpointer = if config.thread_id().is_some() {
        let serializer = Arc::new(langgraph::JsonSerializer);
        Some(Arc::new(SqliteSaver::new(db_path, serializer)?)
            as Arc<dyn langgraph::Checkpointer<ReActState>>)
    } else {
        None
    };

    let store = if config.user_id().is_some() {
        Some(Arc::new(SqliteStore::new(db_path)?) as Arc<dyn langgraph::Store>)
    } else {
        None
    };

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
    } else if let Some(user_id) = config.user_id() {
        if let Some(s) = &store {
            let namespace = vec![user_id.to_string(), "memories".to_string()];
            Box::new(StoreToolSource::new(s.clone(), namespace))
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

    let runnable_config = if config.thread_id().is_some() || config.user_id().is_some() {
        Some(RunnableConfig {
            thread_id: config.thread_id().map(ToString::to_string),
            checkpoint_id: None,
            checkpoint_ns: String::new(),
            user_id: config.user_id().map(ToString::to_string),
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

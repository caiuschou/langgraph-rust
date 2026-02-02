//! Run ReAct graph with given config (SQLite feature enabled); does not read .env, returns final state.
//!
//! Uses checkpointer and store when thread_id/user_id are set.
//! Builds a single AggregateToolSource: memory when user_id+store, MCP (Exa) when
//! use_exa_mcp and exa_api_key are set. See docs/rust-langgraph/tools-refactor/architecture/common-interface-mcp.md.

use std::sync::Arc;

use async_openai::config::OpenAIConfig;
use langgraph::{
    ChatOpenAI, MemoryToolsSource, MockToolSource, RunnableConfig, SqliteSaver, SqliteStore,
    ToolSource,
};

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

    let db_path = config.db_path.as_deref().unwrap_or("memory.db");

    let checkpointer = if config.thread_id().is_some() {
        let serializer = Arc::new(langgraph::JsonSerializer);
        Some(Arc::new(SqliteSaver::new(db_path, serializer)?)
            as Arc<dyn langgraph::Checkpointer<langgraph::ReActState>>)
    } else {
        None
    };

    let store = if config.user_id().is_some() {
        Some(Arc::new(SqliteStore::new(db_path)?) as Arc<dyn langgraph::Store>)
    } else {
        None
    };

    let has_memory = config.user_id().is_some() && store.is_some();
    let has_exa = config.use_exa_mcp && config.exa_api_key.is_some();

    let tool_source: Box<dyn ToolSource> = if !has_memory && !has_exa {
        Box::new(MockToolSource::get_time_example())
    } else {
        let aggregate = if has_memory {
            let user_id = config.user_id().unwrap();
            let s = store.as_ref().unwrap();
            let namespace = vec![user_id.to_string(), "memories".to_string()];
            MemoryToolsSource::new(s.clone(), namespace).await
        } else {
            langgraph::tools::AggregateToolSource::new()
        };
        if has_exa {
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
                langgraph::register_mcp_tools(&aggregate, Arc::new(mcp)).await?;
            }
            #[cfg(not(feature = "mcp"))]
            {
                return Err("MCP feature is not enabled. Build with --features mcp".into());
            }
        }
        Box::new(aggregate)
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

    run_react_graph(
        user_message,
        llm,
        tool_source,
        checkpointer,
        store,
        runnable_config,
    )
    .await
}

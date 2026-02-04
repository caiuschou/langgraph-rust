//! Run ReAct graph with given config; does not read .env, returns final state.
//!
//! Uses [`langgraph::build_react_run_context`](langgraph::build_react_run_context) to build
//! checkpointer, store, runnable_config and tool_source from config; then builds LLM and calls
//! [`run_react_graph`](super::common::run_react_graph).
//!
//! See docs/rust-langgraph/tools-refactor/architecture/common-interface-mcp.md.

use async_openai::config::OpenAIConfig;
use langgraph::ChatOpenAI;

use crate::config::RunConfig;

use super::common::{run_react_graph, run_react_graph_stream};
use super::Error;

/// Run ReAct graph with given config; does not read .env, returns final state.
pub async fn run_with_config(
    config: &RunConfig,
    user_message: &str,
) -> Result<langgraph::ReActState, Error> {
    let build_config = config.to_react_build_config();
    let ctx = langgraph::build_react_run_context(&build_config)
        .await
        .map_err(|e| Box::new(e) as Error)?;

    let openai_config = OpenAIConfig::new()
        .with_api_base(&config.api_base)
        .with_api_key(config.api_key.clone());

    let mut llm = ChatOpenAI::new_with_tool_source(
        openai_config,
        config.model.clone(),
        ctx.tool_source.as_ref(),
    )
    .await?;
    if let Some(t) = config.temperature {
        llm = llm.with_temperature(t);
    }
    if let Some(tc) = config.tool_choice {
        llm = llm.with_tool_choice(tc);
    }
    let llm: Box<dyn langgraph::LlmClient> = Box::new(llm);

    if config.stream {
        run_react_graph_stream(
            user_message,
            llm,
            ctx.tool_source,
            ctx.checkpointer,
            ctx.store,
            ctx.runnable_config,
        )
        .await
    } else {
        run_react_graph(
            user_message,
            llm,
            ctx.tool_source,
            ctx.checkpointer,
            ctx.store,
            ctx.runnable_config,
        )
        .await
    }
}

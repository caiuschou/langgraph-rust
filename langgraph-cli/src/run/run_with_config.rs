//! Run ReAct graph with given config; does not read .env, returns final state.
//!
//! Uses [`langgraph::build_react_run_context`](langgraph::build_react_run_context) to build
//! checkpointer, store, runnable_config and tool_source from config; then builds LLM and calls
//! [`langgraph::run_react_graph`](langgraph::run_react_graph) or
//! [`langgraph::run_react_graph_stream`](langgraph::run_react_graph_stream).
//!
//! See docs/rust-langgraph/tools-refactor/architecture/common-interface-mcp.md.

use async_openai::config::OpenAIConfig;
use langgraph::ChatOpenAI;

use crate::config::RunConfig;

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
        let mut last_tool_calls: Vec<langgraph::ToolCall> = vec![];
        langgraph::run_react_graph_stream(
            user_message,
            llm,
            ctx.tool_source,
            ctx.checkpointer,
            ctx.store,
            ctx.runnable_config,
            config.verbose,
            Some(|event: langgraph::StreamEvent<langgraph::ReActState>| {
                use langgraph::StreamEvent;
                use std::io::Write;
                match &event {
                    StreamEvent::TaskStart { node_id } => {
                        if node_id == "think" {
                            let _ = writeln!(std::io::stdout(), "Thinking...");
                            let _ = std::io::stdout().flush();
                        } else if node_id == "act" {
                            let name = last_tool_calls
                                .first()
                                .map(|tc| tc.name.as_str())
                                .unwrap_or("...");
                            let _ = writeln!(std::io::stdout());
                            let _ = writeln!(std::io::stdout(), "[Calling tool: {}]", name);
                            let _ = std::io::stdout().flush();
                        }
                    }
                    StreamEvent::TaskEnd { node_id, .. } => {
                        if node_id == "act" {
                            let _ = writeln!(std::io::stdout(), "[Tool result received]");
                            let _ = std::io::stdout().flush();
                        }
                    }
                    StreamEvent::Messages { chunk, .. } => {
                        let _ = write!(std::io::stdout(), "{}", chunk.content);
                        let _ = std::io::stdout().flush();
                    }
                    StreamEvent::Updates { state, .. } => {
                        last_tool_calls = state.tool_calls.clone();
                    }
                    _ => {}
                }
            }),
        )
        .await
        .map_err(|e| Box::new(e) as Error)
    } else {
        langgraph::run_react_graph(
            user_message,
            llm,
            ctx.tool_source,
            ctx.checkpointer,
            ctx.store,
            ctx.runnable_config,
            config.verbose,
        )
        .await
        .map_err(|e| Box::new(e) as Error)
    }
}

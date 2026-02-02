//! Shared run logic: build ReAct graph and invoke.
//!
//! Used by [`run_with_config`](super::run_with_config) and by tests that inject
//! MockLlm/MockToolSource. Interacts with [`StateGraph`](langgraph::StateGraph),
//! [`ThinkNode`](langgraph::ThinkNode), [`ActNode`](langgraph::ActNode), [`ObserveNode`](langgraph::ObserveNode).

use std::sync::Arc;

use langgraph::{
    ActNode, CompiledStateGraph, ObserveNode, ReActState, StateGraph, ThinkNode, ToolSource, END,
    REACT_SYSTEM_PROMPT, START,
};
use langgraph::{LlmClient, Message};

use crate::middleware::WithNodeLogging;

use super::Error;

/// Runs the ReAct graph with the given LLM and tool source.
///
/// When `checkpointer` / `store` / `runnable_config` are set, compiles with
/// checkpointer and invokes with config; otherwise compiles without and invokes with `None`.
/// Used by run_with_config (both sqlite and no_sqlite) and by tests.
pub(crate) async fn run_react_graph(
    user_message: &str,
    llm: Box<dyn LlmClient>,
    tool_source: Box<dyn ToolSource>,
    checkpointer: Option<Arc<dyn langgraph::Checkpointer<ReActState>>>,
    store: Option<Arc<dyn langgraph::Store>>,
    runnable_config: Option<langgraph::RunnableConfig>,
) -> Result<ReActState, Error> {
    let think = ThinkNode::new(llm);
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

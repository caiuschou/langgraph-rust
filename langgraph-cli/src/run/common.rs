//! Shared run logic: build ReAct graph and invoke or stream.
//!
//! Used by [`run_with_config`](super::run_with_config) and by tests that inject
//! MockLlm/MockToolSource. Interacts with [`StateGraph`](langgraph::StateGraph),
//! [`ThinkNode`](langgraph::ThinkNode), [`ActNode`](langgraph::ActNode), [`ObserveNode`](langgraph::ObserveNode).
//! When `runnable_config.thread_id` is set and a checkpointer is present, loads the previous
//! checkpoint via [`Checkpointer::get_tuple`](langgraph::Checkpointer::get_tuple) and appends
//! the new user message for multi-turn conversation.

use std::collections::HashSet;
use std::io::{self, Write};
use std::sync::Arc;

use langgraph::{
    ActNode, CheckpointError, CompiledStateGraph, ObserveNode, ReActState, StateGraph, ThinkNode,
    ToolSource, END, REACT_SYSTEM_PROMPT, START,
};
use langgraph::{LlmClient, Message, StreamEvent, StreamMode};
use tokio_stream::StreamExt;

use crate::middleware::WithNodeLogging;

use super::Error;

/// Runs the ReAct graph with the given LLM and tool source.
///
/// When `checkpointer` / `store` / `runnable_config` are set, compiles with
/// checkpointer and invokes with config; otherwise compiles without and invokes with `None`.
/// If `runnable_config.thread_id` is present and checkpointer is set, loads the latest checkpoint
/// and appends the new user message so that multi-turn conversation continues across runs.
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
    let observe = ObserveNode::with_loop();

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

    let compiled: CompiledStateGraph<ReActState> = if let Some(cp) = &checkpointer {
        graph.with_node_logging().compile_with_checkpointer(Arc::clone(cp))?
    } else {
        graph.with_node_logging().compile()?
    };

    let state = build_initial_state(
        user_message,
        &checkpointer,
        &runnable_config,
    )
    .await?;

    let final_state = compiled.invoke(state, runnable_config).await?;
    Ok(final_state)
}

/// Runs the ReAct graph in streaming mode: emits Thinking... / Calling tool / LLM tokens to stdout.
///
/// Same graph build as [`run_react_graph`]; uses [`CompiledStateGraph::stream`] and consumes
/// `StreamEvent` to print human-readable progress. Returns the final state from the last
/// `StreamEvent::Values` in the stream.
pub(crate) async fn run_react_graph_stream(
    user_message: &str,
    llm: Box<dyn LlmClient>,
    tool_source: Box<dyn ToolSource>,
    checkpointer: Option<Arc<dyn langgraph::Checkpointer<ReActState>>>,
    store: Option<Arc<dyn langgraph::Store>>,
    runnable_config: Option<langgraph::RunnableConfig>,
) -> Result<ReActState, Error> {
    let think = ThinkNode::new(llm);
    let act = ActNode::new(tool_source);
    let observe = ObserveNode::with_loop();

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

    let compiled: CompiledStateGraph<ReActState> = if let Some(cp) = &checkpointer {
        graph.with_node_logging().compile_with_checkpointer(Arc::clone(cp))?
    } else {
        graph.with_node_logging().compile()?
    };

    let state = build_initial_state(
        user_message,
        &checkpointer,
        &runnable_config,
    )
    .await?;

    let modes = HashSet::from([
        StreamMode::Messages,
        StreamMode::Tasks,
        StreamMode::Updates,
    ]);
    let mut stream = compiled.stream(state, runnable_config, modes);

    let mut final_state: Option<ReActState> = None;
    let mut last_tool_calls: Vec<langgraph::ToolCall> = vec![];

    while let Some(event) = stream.next().await {
        match event {
            StreamEvent::TaskStart { node_id } => {
                if node_id == "think" {
                    let _ = writeln!(io::stdout(), "Thinking...");
                    let _ = io::stdout().flush();
                } else if node_id == "act" {
                    let name = last_tool_calls
                        .first()
                        .map(|tc| tc.name.as_str())
                        .unwrap_or("...");
                    let _ = writeln!(io::stdout());
                    let _ = writeln!(io::stdout(), "[Calling tool: {}]", name);
                    let _ = io::stdout().flush();
                }
            }
            StreamEvent::TaskEnd { node_id, result: _ } => {
                if node_id == "act" {
                    let _ = writeln!(io::stdout(), "[Tool result received]");
                    let _ = io::stdout().flush();
                }
            }
            StreamEvent::Messages { chunk, metadata: _ } => {
                let _ = write!(io::stdout(), "{}", chunk.content);
                let _ = io::stdout().flush();
            }
            StreamEvent::Updates { node_id: _, state } => {
                last_tool_calls = state.tool_calls.clone();
            }
            StreamEvent::Values(s) => {
                final_state = Some(s);
            }
            _ => {}
        }
    }

    final_state.ok_or_else(|| {
        Box::new(std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "stream ended without final state",
        )) as Error
    })
}

/// Builds the initial ReActState for this run: either from the latest checkpoint for the thread
/// (when checkpointer and runnable_config with thread_id are present) or a fresh state with
/// system prompt and the given user message.
async fn build_initial_state(
    user_message: &str,
    checkpointer: &Option<Arc<dyn langgraph::Checkpointer<ReActState>>>,
    runnable_config: &Option<langgraph::RunnableConfig>,
) -> Result<ReActState, Error> {
    let load_from_checkpoint = checkpointer.is_some()
        && runnable_config
            .as_ref()
            .and_then(|c| c.thread_id.as_ref())
            .is_some();

    if load_from_checkpoint {
        let cp = checkpointer.as_ref().expect("checkpointer is Some");
        let config = runnable_config.as_ref().expect("runnable_config is Some");
        let tuple = cp
            .get_tuple(config)
            .await
            .map_err(|e: CheckpointError| Box::new(e) as Error)?;
        if let Some((checkpoint, _)) = tuple {
            let mut state = checkpoint.channel_values.clone();
            state.messages.push(Message::user(user_message.to_string()));
            state.tool_calls = vec![];
            state.tool_results = vec![];
            return Ok(state);
        }
    }

    Ok(ReActState {
        messages: vec![
            Message::system(REACT_SYSTEM_PROMPT),
            Message::user(user_message.to_string()),
        ],
        tool_calls: vec![],
        tool_results: vec![],
        turn_count: 0,
    })
}

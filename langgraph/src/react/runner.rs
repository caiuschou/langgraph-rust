//! ReAct graph runner: encapsulates graph build, initial state, invoke and stream.
//!
//! Used by langgraph-cli, langgraph-bot, and other callers that need to run the
//! ReAct graph without manually building Think → Act → Observe. Interacts with
//! [`StateGraph`](crate::graph::StateGraph), [`ThinkNode`](super::ThinkNode),
//! [`ActNode`](super::ActNode), [`ObserveNode`](super::ObserveNode), and
//! [`build_react_initial_state`](super::build_react_initial_state).

use std::collections::HashSet;
use std::sync::Arc;

use tokio_stream::StreamExt;

use crate::error::AgentError;
use crate::graph::{CompilationError, CompiledStateGraph, LoggingNodeMiddleware};
use crate::memory::{CheckpointError, Checkpointer, RunnableConfig, Store};
use crate::message::Message;
use crate::state::ReActState;
use crate::stream::{StreamEvent, StreamMode};
use crate::tool_source::ToolSource;
use crate::LlmClient;
use crate::{ActNode, ObserveNode, ThinkNode, StateGraph, END, REACT_SYSTEM_PROMPT, START};

use super::with_node_logging::WithNodeLogging;

/// Builds the initial ReActState for a run: either from the latest checkpoint for the thread
/// (when checkpointer and runnable_config with thread_id are present) or a fresh state with
/// system prompt and the given user message.
///
/// When `system_prompt` is `None`, uses [`REACT_SYSTEM_PROMPT`].
///
/// # Errors
///
/// Returns `CheckpointError` if loading from checkpoint fails.
pub async fn build_react_initial_state(
    user_message: &str,
    checkpointer: Option<&dyn Checkpointer<ReActState>>,
    runnable_config: Option<&RunnableConfig>,
    system_prompt: Option<&str>,
) -> Result<ReActState, CheckpointError> {
    let load_from_checkpoint = checkpointer.is_some()
        && runnable_config
            .and_then(|c| c.thread_id.as_ref())
            .is_some();

    if load_from_checkpoint {
        let cp = checkpointer.expect("checkpointer is Some");
        let config = runnable_config.expect("runnable_config is Some");
        let tuple = cp.get_tuple(config).await?;
        if let Some((checkpoint, _)) = tuple {
            let mut state = checkpoint.channel_values.clone();
            state.messages.push(Message::user(user_message.to_string()));
            state.tool_calls = vec![];
            state.tool_results = vec![];
            return Ok(state);
        }
    }

    let prompt = system_prompt.unwrap_or(REACT_SYSTEM_PROMPT);
    Ok(ReActState {
        messages: vec![
            Message::system(prompt),
            Message::user(user_message.to_string()),
        ],
        tool_calls: vec![],
        tool_results: vec![],
        turn_count: 0,
    })
}

/// Runs the ReAct graph with the given LLM and tool source.
///
/// When `checkpointer` / `store` / `runnable_config` are set, compiles with
/// checkpointer and invokes with config; otherwise compiles without and invokes with `None`.
/// If `runnable_config.thread_id` is present and checkpointer is set, loads the latest checkpoint
/// and appends the new user message so that multi-turn conversation continues across runs.
/// When `verbose` is true, attaches node logging middleware (enter/exit).
///
/// # Errors
///
/// Returns `CompilationError` if graph compilation fails.
/// Returns `AgentError` or `CheckpointError` if invoke or initial state build fails.
pub async fn run_react_graph(
    user_message: &str,
    llm: Box<dyn LlmClient>,
    tool_source: Box<dyn ToolSource>,
    checkpointer: Option<Arc<dyn Checkpointer<ReActState>>>,
    store: Option<Arc<dyn Store>>,
    runnable_config: Option<RunnableConfig>,
    verbose: bool,
) -> Result<ReActState, RunError> {
    let runner = ReactRunner::new(
        llm,
        tool_source,
        checkpointer,
        store,
        runnable_config,
        verbose,
    )?;
    runner.invoke(user_message).await
}

/// Runs the ReAct graph in streaming mode.
///
/// Same graph build as [`run_react_graph`]; uses [`CompiledStateGraph::stream`].
/// Returns the final state from the last `StreamEvent::Values` in the stream.
/// When `on_event` is provided, invokes it for each `StreamEvent` so the caller
/// can implement custom UX (e.g. print "Thinking...", "Calling tool", token chunks).
///
/// # Errors
///
/// Returns `CompilationError` if graph compilation fails.
/// Returns `RunError` if stream ends without final state or other failure.
pub async fn run_react_graph_stream<F>(
    user_message: &str,
    llm: Box<dyn LlmClient>,
    tool_source: Box<dyn ToolSource>,
    checkpointer: Option<Arc<dyn Checkpointer<ReActState>>>,
    store: Option<Arc<dyn Store>>,
    runnable_config: Option<RunnableConfig>,
    verbose: bool,
    on_event: Option<F>,
) -> Result<ReActState, RunError>
where
    F: FnMut(StreamEvent<ReActState>),
{
    let runner = ReactRunner::new(
        llm,
        tool_source,
        checkpointer,
        store,
        runnable_config,
        verbose,
    )?;
    runner.stream_with_callback(user_message, on_event).await
}

/// Error type for ReactRunner invoke/stream operations.
#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("compilation failed: {0}")]
    Compilation(#[from] CompilationError),
    #[error("checkpoint error: {0}")]
    Checkpoint(#[from] CheckpointError),
    #[error("execution failed: {0}")]
    Execution(#[from] AgentError),
    #[error("stream ended without final state")]
    StreamEndedWithoutState,
}

impl From<std::io::Error> for RunError {
    fn from(e: std::io::Error) -> Self {
        RunError::Execution(AgentError::ExecutionFailed(e.to_string()))
    }
}

/// ReAct graph runner: encapsulates compiled graph and persistence config.
///
/// Built from LLM, tool source, and optional checkpointer/store/config.
/// Supports `invoke` (non-streaming) and `stream` (streaming with StreamEvent).
///
/// # Example
///
/// ```ignore
/// let runner = ReactRunner::new(llm, tool_source, checkpointer, store, config, verbose)?;
/// let state = runner.invoke("Hello").await?;
/// ```
pub struct ReactRunner {
    compiled: CompiledStateGraph<ReActState>,
    checkpointer: Option<Arc<dyn Checkpointer<ReActState>>>,
    runnable_config: Option<RunnableConfig>,
}

impl ReactRunner {
    /// Creates a runner with the given LLM, tool source, and optional persistence.
    ///
    /// When `verbose` is true, attaches node logging middleware. When both
    /// checkpointer and verbose are set, compiles with both.
    pub fn new(
        llm: Box<dyn LlmClient>,
        tool_source: Box<dyn ToolSource>,
        checkpointer: Option<Arc<dyn Checkpointer<ReActState>>>,
        store: Option<Arc<dyn Store>>,
        runnable_config: Option<RunnableConfig>,
        verbose: bool,
    ) -> Result<Self, CompilationError> {
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

        let graph = if verbose {
            graph.with_node_logging()
        } else {
            graph
        };

        let compiled = match (&checkpointer, verbose) {
            (Some(cp), true) => {
                let mw = Arc::new(LoggingNodeMiddleware::<ReActState>::default());
                graph.compile_with_checkpointer_and_middleware(Arc::clone(cp), mw)?
            }
            (Some(cp), false) => graph.compile_with_checkpointer(Arc::clone(cp))?,
            (None, _) => graph.compile()?,
        };

        Ok(Self {
            compiled,
            checkpointer,
            runnable_config,
        })
    }

    /// Invokes the graph with the given user message.
    pub async fn invoke(&self, user_message: &str) -> Result<ReActState, RunError> {
        let state = build_react_initial_state(
            user_message,
            self.checkpointer.as_deref(),
            self.runnable_config.as_ref(),
            None,
        )
        .await?;
        let final_state = self.compiled.invoke(state, self.runnable_config.clone()).await?;
        Ok(final_state)
    }

    /// Streams the graph execution; returns the final state from the last StreamEvent::Values.
    ///
    /// Emits `StreamEvent` for TaskStart, TaskEnd, Messages, Updates, Values.
    /// When `on_event` is provided, invokes it for each event so the caller can implement
    /// custom UX (e.g. print "Thinking...", "Calling tool", token chunks).
    pub async fn stream_with_callback<F>(
        &self,
        user_message: &str,
        mut on_event: Option<F>,
    ) -> Result<ReActState, RunError>
    where
        F: FnMut(StreamEvent<ReActState>),
    {
        let state = build_react_initial_state(
            user_message,
            self.checkpointer.as_deref(),
            self.runnable_config.as_ref(),
            None,
        )
        .await?;

        let modes = HashSet::from([
            StreamMode::Messages,
            StreamMode::Tasks,
            StreamMode::Updates,
            StreamMode::Values,
        ]);
        let mut stream = self
            .compiled
            .stream(state, self.runnable_config.clone(), modes);

        let mut final_state: Option<ReActState> = None;
        while let Some(event) = stream.next().await {
            if let Some(ref mut f) = on_event {
                f(event.clone());
            }
            if let StreamEvent::Values(s) = event {
                final_state = Some(s);
            }
        }

        final_state.ok_or(RunError::StreamEndedWithoutState)
    }
}

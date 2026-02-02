//! Compiled state graph: immutable, supports invoke only.
//!
//! Built by `StateGraph::compile` or `compile_with_checkpointer`. Holds nodes and
//! edge order (derived from explicit edges at compile time), optional checkpointer.
//! When checkpointer is set and config.thread_id is provided, final state is saved after invoke. See docs/rust-langgraph/16-memory-design.md §4.1.

use std::collections::{HashMap, HashSet};
use std::fmt::Debug;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::error::AgentError;
use crate::memory::{Checkpoint, CheckpointSource, Checkpointer, RunnableConfig, Store};
use crate::stream::{StreamEvent, StreamMode};

use super::node_middleware::NodeMiddleware;
use super::{Next, Node, RunContext};

/// Compiled graph: immutable structure, supports invoke only.
///
/// Created by `StateGraph::compile()` or `compile_with_checkpointer()`. Runs from first node;
/// uses each node's returned `Next` to choose next node. When checkpointer is set, invoke(state, config)
/// saves the final state for config.thread_id. When store is set (via `with_store` before compile),
/// nodes can use it for long-term memory (e.g. namespace from config.user_id). See docs/rust-langgraph/16-memory-design.md §5.2.
#[derive(Clone)]
pub struct CompiledStateGraph<S> {
    pub(super) nodes: HashMap<String, Arc<dyn Node<S>>>,
    pub(super) edge_order: Vec<String>,
    pub(super) checkpointer: Option<Arc<dyn Checkpointer<S>>>,
    /// Optional long-term store; set when graph was built with `with_store`. Nodes use it via config or construction. See docs/rust-langgraph/16-memory-design.md §5.2.
    pub(super) store: Option<Arc<dyn Store>>,
    /// Optional node middleware; set when built with `compile_with_middleware` or `compile_with_checkpointer_and_middleware`.
    pub(super) middleware: Option<Arc<dyn NodeMiddleware<S>>>,
}

impl<S> CompiledStateGraph<S>
where
    S: Clone + Send + Sync + Debug + 'static,
{
    /// Shared run loop used by invoke() and stream(): steps through nodes until completion.
    async fn run_loop_inner(
        &self,
        state: &mut S,
        config: &Option<RunnableConfig>,
        current_id: &mut String,
        run_ctx: Option<&RunContext<S>>,
    ) -> Result<(), AgentError> {
        loop {
            let node = self
                .nodes
                .get(current_id)
                .expect("compiled graph has all nodes")
                .clone();
            let current_state = state.clone();

            let (new_state, next) = if let Some(middleware) = &self.middleware {
                let node_id = current_id.clone();
                let run_ctx_owned = run_ctx.cloned();
                middleware
                    .around_run(
                        &node_id,
                        current_state,
                        Box::new(move |s| {
                            let node = node.clone();
                            let run_ctx_inner = run_ctx_owned.clone();
                            Box::pin(async move {
                                if let Some(ctx) = run_ctx_inner.as_ref() {
                                    node.run_with_context(s, ctx).await
                                } else {
                                    node.run(s).await
                                }
                            })
                        }),
                    )
                    .await?
            } else if let Some(ctx) = run_ctx {
                node.run_with_context(current_state, ctx).await?
            } else {
                node.run(current_state).await?
            };

            *state = new_state;

            if let Some(ctx) = run_ctx {
                if let Some(tx) = &ctx.stream_tx {
                    if ctx.stream_mode.contains(&StreamMode::Values) {
                        let _ = tx.send(StreamEvent::Values(state.clone())).await;
                    }
                    if ctx.stream_mode.contains(&StreamMode::Updates) {
                        let _ = tx
                            .send(StreamEvent::Updates {
                                node_id: current_id.clone(),
                                state: state.clone(),
                            })
                            .await;
                    }
                }
            }

            match next {
                Next::End => {
                    if let (Some(cp), Some(cfg)) = (&self.checkpointer, config) {
                        if cfg.thread_id.is_some() {
                            let checkpoint =
                                Checkpoint::from_state(state.clone(), CheckpointSource::Update, 0);
                            let _ = cp.put(cfg, &checkpoint).await;
                        }
                    }
                    return Ok(());
                }
                Next::Node(id) => *current_id = id,
                Next::Continue => {
                    let pos = self
                        .edge_order
                        .iter()
                        .position(|x| x == current_id)
                        .expect("current node in edge_order");
                    let next_pos = pos + 1;
                    if next_pos >= self.edge_order.len() {
                        if let (Some(cp), Some(cfg)) = (&self.checkpointer, config) {
                            if cfg.thread_id.is_some() {
                                let checkpoint = Checkpoint::from_state(
                                    state.clone(),
                                    CheckpointSource::Update,
                                    0,
                                );
                                let _ = cp.put(cfg, &checkpoint).await;
                            }
                        }
                        return Ok(());
                    }
                    *current_id = self.edge_order[next_pos].clone();
                }
            }
        }
    }

    /// Runs the graph with the given state. Starts at the first node in edge order;
    /// after each node, uses returned `Next` to continue linear order, jump to a node, or end.
    ///
    /// When `config` has `thread_id` and the graph was compiled with a checkpointer,
    /// the final state is saved after the run. Pass `None` for config to keep current behavior (no persistence).
    ///
    /// - `Next::Continue`: run the next node in edge_order, or end if last.
    /// - `Next::Node(id)`: run the node with that id next.
    /// - `Next::End`: stop and return current state.
    pub async fn invoke(&self, state: S, config: Option<RunnableConfig>) -> Result<S, AgentError> {
        let mut state = state;
        let mut current_id = self
            .edge_order
            .first()
            .cloned()
            .ok_or_else(|| AgentError::ExecutionFailed("empty graph".into()))?;

        self.run_loop_inner(&mut state, &config, &mut current_id, None)
            .await?;

        Ok(state)
    }

    /// Streams graph execution, emitting events via channel-backed Stream.
    pub fn stream(
        &self,
        state: S,
        config: Option<RunnableConfig>,
        stream_mode: impl Into<HashSet<StreamMode>>,
    ) -> ReceiverStream<StreamEvent<S>> {
        let (tx, rx) = mpsc::channel(128);
        let graph = self.clone();
        let mode_set: HashSet<StreamMode> = stream_mode.into();

        tokio::spawn(async move {
            let mut state = state;
            let mut current_id = match graph.edge_order.first().cloned() {
                Some(id) => id,
                None => return,
            };
            let run_ctx = RunContext {
                config: config.clone().unwrap_or_default(),
                stream_tx: Some(tx),
                stream_mode: mode_set,
            };

            let _ = graph
                .run_loop_inner(&mut state, &config, &mut current_id, Some(&run_ctx))
                .await;
        });

        ReceiverStream::new(rx)
    }

    /// Returns the long-term store if the graph was compiled with `with_store(store)`.
    ///
    /// Nodes can use it for cross-thread memory (e.g. namespace from `config.user_id`). See docs/rust-langgraph/16-memory-design.md §5.
    pub fn store(&self) -> Option<&Arc<dyn Store>> {
        self.store.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};
    use std::sync::Arc;

    use async_trait::async_trait;
    use tokio_stream::StreamExt;

    use crate::graph::{Next, Node, StateGraph, END, START};
    use crate::stream::{StreamEvent, StreamMode};

    /// **Scenario**: When edge_order is empty, invoke returns ExecutionFailed("empty graph").
    #[tokio::test]
    async fn invoke_empty_graph_returns_execution_failed() {
        let graph = CompiledStateGraph::<crate::state::ReActState> {
            nodes: HashMap::new(),
            edge_order: vec![],
            checkpointer: None,
            store: None,
            middleware: None,
        };
        let state = crate::state::ReActState::default();
        let result = graph.invoke(state, None).await;
        match &result {
            Err(AgentError::ExecutionFailed(msg)) => {
                assert!(msg.contains("empty graph"), "{}", msg)
            }
            _ => panic!(
                "expected ExecutionFailed(\"empty graph\"), got {:?}",
                result
            ),
        }
    }

    #[derive(Clone)]
    struct AddNode {
        id: &'static str,
        delta: i32,
    }

    #[async_trait]
    impl Node<i32> for AddNode {
        fn id(&self) -> &str {
            self.id
        }

        async fn run(&self, state: i32) -> Result<(i32, Next), AgentError> {
            Ok((state + self.delta, Next::Continue))
        }
    }

    fn build_two_step_graph() -> CompiledStateGraph<i32> {
        let mut graph = StateGraph::<i32>::new();
        graph.add_node(
            "first",
            Arc::new(AddNode {
                id: "first",
                delta: 1,
            }),
        );
        graph.add_node(
            "second",
            Arc::new(AddNode {
                id: "second",
                delta: 2,
            }),
        );
        graph.add_edge(START, "first");
        graph.add_edge("first", "second");
        graph.add_edge("second", END);
        graph.compile().expect("graph compiles")
    }

    /// **Scenario**: stream(values) emits state snapshots per node and ends with final state.
    #[tokio::test]
    async fn stream_values_emits_states() {
        let graph = build_two_step_graph();
        let stream = graph.stream(0, None, HashSet::from_iter([StreamMode::Values]));
        let events: Vec<_> = stream.collect().await;
        assert!(!events.is_empty(), "expected at least one Values event");
        assert!(
            matches!(events.last(), Some(StreamEvent::Values(v)) if *v == 3),
            "last event should be final state 3"
        );
    }

    /// **Scenario**: stream(updates) emits Updates with node ids in order.
    #[tokio::test]
    async fn stream_updates_emit_node_ids_in_order() {
        let graph = build_two_step_graph();
        let stream = graph.stream(0, None, HashSet::from_iter([StreamMode::Updates]));
        let events: Vec<_> = stream.collect().await;
        let ids: Vec<_> = events
            .iter()
            .map(|e| match e {
                StreamEvent::Updates { node_id, state } => {
                    assert!(
                        *state == 1 || *state == 3,
                        "unexpected state value {}",
                        state
                    );
                    node_id.clone()
                }
                other => panic!("unexpected event {:?}", other),
            })
            .collect();
        assert_eq!(ids, vec!["first".to_string(), "second".to_string()]);
    }
}

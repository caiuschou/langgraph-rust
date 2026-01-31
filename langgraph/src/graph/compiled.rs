//! Compiled state graph: immutable, supports invoke only.
//!
//! Built by `StateGraph::compile` or `compile_with_checkpointer`. Holds nodes, edge order, optional checkpointer.
//! When checkpointer is set and config.thread_id is provided, final state is saved after invoke. See docs/rust-langgraph/16-memory-design.md §4.1.

use std::collections::HashMap;
use std::sync::Arc;

use crate::error::AgentError;
use crate::memory::{Checkpoint, CheckpointSource, Checkpointer, RunnableConfig, Store};

use super::Next;
use super::Node;

/// Compiled graph: immutable structure, supports invoke only.
///
/// Created by `StateGraph::compile()` or `compile_with_checkpointer()`. Runs from first node;
/// uses each node's returned `Next` to choose next node. When checkpointer is set, invoke(state, config)
/// saves the final state for config.thread_id. When store is set (via `with_store` before compile),
/// nodes can use it for long-term memory (e.g. namespace from config.user_id). See docs/rust-langgraph/16-memory-design.md §5.2.
pub struct CompiledStateGraph<S> {
    pub(super) nodes: HashMap<String, Box<dyn Node<S>>>,
    pub(super) edge_order: Vec<String>,
    pub(super) checkpointer: Option<Arc<dyn Checkpointer<S>>>,
    /// Optional long-term store; set when graph was built with `with_store`. Nodes use it via config or construction. See docs/rust-langgraph/16-memory-design.md §5.2.
    pub(super) store: Option<Arc<dyn Store>>,
}

impl<S> CompiledStateGraph<S>
where
    S: Clone + Send + Sync + 'static,
{
    /// Runs the graph with the given state. Starts at the first node in edge order;
    /// after each node, uses returned `Next` to continue linear order, jump to a node, or end.
    ///
    /// When `config` has `thread_id` and the graph was compiled with a checkpointer,
    /// the final state is saved after the run. Pass `None` for config to keep current behavior (no persistence).
    ///
    /// - `Next::Continue`: run the next node in edge_order, or end if last.
    /// - `Next::Node(id)`: run the node with that id next.
    /// - `Next::End`: stop and return current state.
    pub async fn invoke(
        &self,
        state: S,
        config: Option<RunnableConfig>,
    ) -> Result<S, AgentError> {
        let mut state = state;
        let mut current_id = self
            .edge_order
            .first()
            .cloned()
            .ok_or_else(|| AgentError::ExecutionFailed("empty graph".into()))?;

        loop {
            let node = self
                .nodes
                .get(&current_id)
                .expect("compiled graph has all nodes");
            let (new_state, next) = node.run(state).await?;
            state = new_state;

            match next {
                Next::End => {
                    if let (Some(cp), Some(cfg)) = (&self.checkpointer, &config) {
                        if cfg.thread_id.is_some() {
                            let checkpoint =
                                Checkpoint::from_state(state.clone(), CheckpointSource::Update, 0);
                            let _ = cp.put(cfg, &checkpoint).await;
                        }
                    }
                    return Ok(state);
                }
                Next::Node(id) => current_id = id,
                Next::Continue => {
                    let pos = self
                        .edge_order
                        .iter()
                        .position(|x| x == &current_id)
                        .expect("current node in edge_order");
                    let next_pos = pos + 1;
                    if next_pos >= self.edge_order.len() {
                        if let (Some(cp), Some(cfg)) = (&self.checkpointer, &config) {
                            if cfg.thread_id.is_some() {
                                let checkpoint = Checkpoint::from_state(
                                    state.clone(),
                                    CheckpointSource::Update,
                                    0,
                                );
                                let _ = cp.put(cfg, &checkpoint).await;
                            }
                        }
                        return Ok(state);
                    }
                    current_id = self.edge_order[next_pos].clone();
                }
            }
        }
    }

    /// Returns the long-term store if the graph was compiled with `with_store(store)`.
    ///
    /// Nodes can use it for cross-thread memory (e.g. namespace from `config.user_id`). See docs/rust-langgraph/16-memory-design.md §5.
    pub fn store(&self) -> Option<&Arc<dyn Store>> {
        self.store.as_ref()
    }
}

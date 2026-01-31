//! State graph: nodes + explicit edges (from → to).
//!
//! Add nodes with `add_node`, define the chain with `add_edge(from, to)` using
//! `START` and `END` for graph entry/exit, then `compile` or `compile_with_checkpointer`
//! to get a `CompiledStateGraph`. Design: docs/rust-langgraph/11-state-graph-design.md.
//! Checkpointer/store: docs/rust-langgraph/16-memory-design.md.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::graph::compile_error::CompilationError;
use crate::graph::compiled::CompiledStateGraph;
use crate::graph::node::Node;
use crate::graph::node_middleware::NodeMiddleware;
use crate::memory::{Checkpointer, Store};

/// Sentinel for graph entry: use as `from_id` in `add_edge(START, first_node_id)`.
pub const START: &str = "__start__";

/// Sentinel for graph exit: use as `to_id` in `add_edge(last_node_id, END)`.
pub const END: &str = "__end__";

/// State graph: nodes plus explicit edges. No conditional edges in minimal version.
///
/// Generic over state type `S`. Build with `add_node` / `add_edge(from, to)` (use
/// `START` and `END` for entry/exit), then `compile()` or `compile_with_middleware()`
/// to obtain an executable graph.
///
/// **Interaction**: Accepts `Arc<dyn Node<S>>`; produces `CompiledStateGraph<S>`.
/// Middleware is optional and passed at compile time only (not stored in StateGraph).
pub struct StateGraph<S> {
    nodes: HashMap<String, Arc<dyn Node<S>>>,
    /// Edges (from_id, to_id). Compiled graph derives linear execution order from these.
    edges: Vec<(String, String)>,
    /// Optional long-term store; when set, compiled graph holds it for nodes (e.g. via config or node construction). See docs/rust-langgraph/16-memory-design.md §5.2.
    store: Option<Arc<dyn Store>>,
}

impl<S> Default for StateGraph<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S> StateGraph<S>
where
    S: Clone + Send + Sync + 'static,
{
    /// Creates an empty graph.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            store: None,
        }
    }

    /// Attaches a long-term store to the graph. When compiled, the graph holds `Option<Arc<dyn Store>>`;
    /// nodes can use it for cross-thread memory (e.g. namespace from `RunnableConfig::user_id`). See docs/rust-langgraph/16-memory-design.md §5.2.
    pub fn with_store(self, store: Arc<dyn Store>) -> Self {
        Self {
            store: Some(store),
            ..self
        }
    }

    /// Adds a node; id must be unique. Replaces if same id.
    ///
    /// Returns `&mut Self` for method chaining. The node is stored as
    /// `Arc<dyn Node<S>>`; use `add_edge` to include it in the chain.
    pub fn add_node(&mut self, id: impl Into<String>, node: Arc<dyn Node<S>>) -> &mut Self {
        self.nodes.insert(id.into(), node);
        self
    }

    /// Adds an edge from `from_id` to `to_id`.
    ///
    /// Use `START` for graph entry and `END` for graph exit. Both ids (except
    /// START/END) must be registered via `add_node` before `compile()`.
    /// Edges must form a single linear chain: one edge from START, one edge to END.
    pub fn add_edge(
        &mut self,
        from_id: impl Into<String>,
        to_id: impl Into<String>,
    ) -> &mut Self {
        self.edges.push((from_id.into(), to_id.into()));
        self
    }

    /// Builds the executable graph: validates that all edge node ids exist and
    /// edges form a single linear chain from START to END.
    ///
    /// Returns `CompilationError` if any edge references an unknown node or
    /// the chain is invalid. On success, the graph is immutable and ready for `invoke`.
    pub fn compile(self) -> Result<CompiledStateGraph<S>, CompilationError> {
        self.compile_internal(None, None)
    }

    /// Builds the executable graph with a checkpointer for persistence (thread_id in config).
    ///
    /// Aligns with LangGraph `graph.compile(checkpointer=checkpointer)`. When `invoke(state, config)`
    /// is called with `config.thread_id`, the final state is saved after the run. See docs/rust-langgraph/16-memory-design.md §4.1.
    pub fn compile_with_checkpointer(
        self,
        checkpointer: Arc<dyn Checkpointer<S>>,
    ) -> Result<CompiledStateGraph<S>, CompilationError> {
        self.compile_internal(Some(checkpointer), None)
    }

    /// Builds the executable graph with node middleware. The middleware wraps each node.run in invoke.
    pub fn compile_with_middleware(
        self,
        middleware: Arc<dyn NodeMiddleware<S>>,
    ) -> Result<CompiledStateGraph<S>, CompilationError> {
        self.compile_internal(None, Some(middleware))
    }

    /// Builds the executable graph with both checkpointer and node middleware.
    pub fn compile_with_checkpointer_and_middleware(
        self,
        checkpointer: Arc<dyn Checkpointer<S>>,
        middleware: Arc<dyn NodeMiddleware<S>>,
    ) -> Result<CompiledStateGraph<S>, CompilationError> {
        self.compile_internal(Some(checkpointer), Some(middleware))
    }

    fn compile_internal(
        self,
        checkpointer: Option<Arc<dyn Checkpointer<S>>>,
        middleware: Option<Arc<dyn NodeMiddleware<S>>>,
    ) -> Result<CompiledStateGraph<S>, CompilationError> {
        for (from, to) in &self.edges {
            if from != START && !self.nodes.contains_key(from) {
                return Err(CompilationError::NodeNotFound(from.clone()));
            }
            if to != END && !self.nodes.contains_key(to) {
                return Err(CompilationError::NodeNotFound(to.clone()));
            }
        }

        let start_edges: Vec<_> = self
            .edges
            .iter()
            .filter(|(f, _)| f == START)
            .map(|(_, t)| t.clone())
            .collect();
        if start_edges.len() != 1 {
            return Err(CompilationError::MissingStart);
        }
        let first = start_edges.into_iter().next().unwrap();

        let end_edges: Vec<_> = self
            .edges
            .iter()
            .filter(|(_, t)| t == END)
            .map(|(f, _)| f.clone())
            .collect();
        if end_edges.len() != 1 {
            return Err(CompilationError::MissingEnd);
        }
        let expected_last = end_edges.into_iter().next().unwrap();

        let froms: Vec<_> = self
            .edges
            .iter()
            .filter(|(f, _)| f.as_str() != START)
            .map(|(f, _)| f.clone())
            .collect();
        let tos: Vec<_> = self
            .edges
            .iter()
            .filter(|(_, t)| t.as_str() != END)
            .map(|(_, t)| t.clone())
            .collect();
        let unique_froms: std::collections::HashSet<_> = froms.iter().cloned().collect();
        let unique_tos: std::collections::HashSet<_> = tos.iter().cloned().collect();
        if unique_froms.len() != froms.len() {
            return Err(CompilationError::InvalidChain(
                "duplicate from (branch)".into(),
            ));
        }
        if unique_tos.len() != tos.len() {
            return Err(CompilationError::InvalidChain(
                "duplicate to (merge or branch)".into(),
            ));
        }

        let next_map: HashMap<String, String> = self
            .edges
            .iter()
            .filter(|(f, _)| f.as_str() != START)
            .map(|(f, t)| (f.clone(), t.clone()))
            .collect();

        let mut edge_order = vec![first.clone()];
        let mut current = first;
        let mut visited = HashSet::new();
        visited.insert(current.clone());
        loop {
            let next = match next_map.get(&current) {
                Some(n) => n.clone(),
                None => break,
            };
            if next == END {
                if current != expected_last {
                    return Err(CompilationError::InvalidChain(
                        "chain tail does not match the single edge to END".into(),
                    ));
                }
                break;
            }
            if visited.contains(&next) {
                return Err(CompilationError::InvalidChain("cycle detected".into()));
            }
            visited.insert(next.clone());
            edge_order.push(next.clone());
            current = next;
        }

        Ok(CompiledStateGraph {
            nodes: self.nodes,
            edge_order,
            checkpointer,
            store: self.store,
            middleware,
        })
    }
}

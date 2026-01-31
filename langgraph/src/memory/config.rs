//! Invoke config: thread_id, checkpoint_id, checkpoint_ns, user_id.
//!
//! Aligns with LangGraph's config["configurable"]. Used by CompiledStateGraph::invoke
//! and Checkpointer. See docs/rust-langgraph/16-memory-design.md §3.1.

/// Config for a single invoke. Identifies the thread and optional checkpoint.
///
/// Aligns with LangGraph's config["configurable"] (thread_id, checkpoint_id, checkpoint_ns).
/// When using a checkpointer, invoke must provide at least thread_id.
///
/// **Interaction**: Passed to `CompiledStateGraph::invoke(state, config)` and
/// `Checkpointer::put` / `get_tuple` / `list`.
#[derive(Debug, Clone, Default)]
pub struct RunnableConfig {
    /// Unique id for this conversation/thread. Required when using a checkpointer.
    pub thread_id: Option<String>,
    /// If set, load state from this checkpoint instead of the latest (time travel / branch).
    pub checkpoint_id: Option<String>,
    /// Optional namespace for checkpoints (e.g. subgraph). Default is empty.
    pub checkpoint_ns: String,
    /// Optional user id; used by Store for cross-thread memory (namespace).
    pub user_id: Option<String>,
}

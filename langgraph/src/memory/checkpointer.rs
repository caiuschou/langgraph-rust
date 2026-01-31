//! Checkpointer trait and CheckpointError.
//!
//! Saves and loads checkpoints by (thread_id, checkpoint_ns, checkpoint_id).
//! Aligns with LangGraph BaseCheckpointSaver. See docs/rust-langgraph/16-memory-design.md §3.3.

use async_trait::async_trait;

use crate::memory::checkpoint::{Checkpoint, CheckpointListItem, CheckpointMetadata};
use crate::memory::config::RunnableConfig;

/// Error type for checkpoint operations.
///
/// Used by Checkpointer::put, get_tuple, list and by Serializer.
#[derive(Debug, thiserror::Error)]
pub enum CheckpointError {
    #[error("thread_id required")]
    ThreadIdRequired,
    #[error("serialization: {0}")]
    Serialization(String),
    #[error("storage: {0}")]
    Storage(String),
    #[error("not found: {0}")]
    NotFound(String),
}

/// Saves and loads checkpoints by (thread_id, checkpoint_ns, checkpoint_id).
///
/// Aligns with LangGraph BaseCheckpointSaver (put, get_tuple, list).
/// Implementations: MemorySaver (in-memory). Future: SqliteSaver, PostgresSaver.
///
/// **Interaction**: Injected at compile via StateGraph::compile_with_checkpointer;
/// CompiledStateGraph::invoke uses it when config.thread_id is set.
#[async_trait]
pub trait Checkpointer<S>: Send + Sync
where
    S: Clone + Send + Sync + 'static,
{
    /// Persist a checkpoint for the thread and config. Returns the checkpoint id used.
    async fn put(
        &self,
        config: &RunnableConfig,
        checkpoint: &Checkpoint<S>,
    ) -> Result<String, CheckpointError>;

    /// Load the latest checkpoint for the thread (or the one given by config.checkpoint_id).
    async fn get_tuple(
        &self,
        config: &RunnableConfig,
    ) -> Result<Option<(Checkpoint<S>, CheckpointMetadata)>, CheckpointError>;

    /// List checkpoint ids for the thread (e.g. for get_state_history, time travel).
    async fn list(
        &self,
        config: &RunnableConfig,
        limit: Option<usize>,
        before: Option<&str>,
        after: Option<&str>,
    ) -> Result<Vec<CheckpointListItem>, CheckpointError>;
}

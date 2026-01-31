//! Checkpoint and metadata types.
//!
//! Aligns with LangGraph checkpoint (id, ts, channel_values, channel_versions, metadata).
//! See docs/rust-langgraph/16-memory-design.md §3.2.

use std::collections::HashMap;
use std::time::SystemTime;

/// Metadata for a single checkpoint (source, step, created_at).
///
/// Aligns with LangGraph checkpoint metadata. Used by Checkpointer implementations
/// and by list() for time-travel UI.
#[derive(Debug, Clone)]
pub struct CheckpointMetadata {
    pub source: CheckpointSource,
    pub step: u64,
    pub created_at: Option<std::time::SystemTime>,
}

/// Source of the checkpoint (input, loop, update, fork).
///
/// Aligns with LangGraph checkpoint metadata.source.
#[derive(Debug, Clone)]
pub enum CheckpointSource {
    Input,
    Loop,
    Update,
    Fork,
}

/// One checkpoint: state snapshot + channel versions + id/ts.
///
/// Stored by Checkpointer keyed by (thread_id, checkpoint_ns, checkpoint_id).
/// channel_values is the graph state S; channel_versions used for reducer/merge.
///
/// **Interaction**: Produced by graph execution; consumed by Checkpointer::put,
/// returned by get_tuple.
#[derive(Debug, Clone)]
pub struct Checkpoint<S> {
    pub id: String,
    pub ts: String,
    pub channel_values: S,
    pub channel_versions: HashMap<String, u64>,
    pub metadata: CheckpointMetadata,
}

/// Item returned by Checkpointer::list for history / time-travel.
#[derive(Debug, Clone)]
pub struct CheckpointListItem {
    pub checkpoint_id: String,
    pub metadata: CheckpointMetadata,
}

impl<S> Checkpoint<S> {
    /// Creates a checkpoint from current state for saving after invoke. Uses current time for id/ts.
    pub fn from_state(state: S, source: CheckpointSource, step: u64) -> Self {
        let now = SystemTime::now();
        let ts = format!(
            "{}",
            now.duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0)
        );
        let id = format!("{}-{}", ts, step);
        Self {
            id: id.clone(),
            ts,
            channel_values: state,
            channel_versions: HashMap::new(),
            metadata: CheckpointMetadata {
                source,
                step,
                created_at: Some(now),
            },
        }
    }
}

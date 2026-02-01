//! Serializer for checkpoint state (state <-> bytes).
//!
//! Aligns with LangGraph SerializerProtocol / JsonPlusSerializer. Used by persistent
//! Checkpointer implementations. See docs/rust-langgraph/16-memory-design.md §3.5.

use crate::memory::checkpointer::CheckpointError;

/// Serializes and deserializes state for checkpoint storage.
///
/// Used by persistent Checkpointer implementations (e.g. SqliteSaver). MemorySaver
/// stores `Checkpoint<S>` in memory and does not use a Serializer.
pub trait Serializer<S>: Send + Sync
where
    S: Clone + Send + Sync + 'static,
{
    fn serialize(&self, state: &S) -> Result<Vec<u8>, CheckpointError>;
    fn deserialize(&self, bytes: &[u8]) -> Result<S, CheckpointError>;
}

/// JSON-based serializer. Requires S: Serialize + serde::de::DeserializeOwned.
///
/// Use for persistent checkpoint storage when state is JSON-serializable.
pub struct JsonSerializer;

impl<S> Serializer<S> for JsonSerializer
where
    S: Clone + Send + Sync + 'static + serde::Serialize + serde::de::DeserializeOwned,
{
    fn serialize(&self, state: &S) -> Result<Vec<u8>, CheckpointError> {
        serde_json::to_vec(state).map_err(|e| CheckpointError::Serialization(e.to_string()))
    }

    fn deserialize(&self, bytes: &[u8]) -> Result<S, CheckpointError> {
        serde_json::from_slice(bytes).map_err(|e| CheckpointError::Serialization(e.to_string()))
    }
}

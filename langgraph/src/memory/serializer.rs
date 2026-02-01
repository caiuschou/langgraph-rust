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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
    struct TestState {
        value: String,
    }

    /// **Scenario**: Serialize then deserialize yields the same value.
    #[test]
    fn json_serializer_roundtrip() {
        let ser = JsonSerializer;
        let state = TestState {
            value: "hello".into(),
        };
        let bytes = ser.serialize(&state).unwrap();
        let restored: TestState = ser.deserialize(&bytes).unwrap();
        assert_eq!(state, restored);
    }

    /// **Scenario**: Invalid JSON on deserialize returns CheckpointError::Serialization.
    #[test]
    fn json_serializer_invalid_json_deserialize_returns_checkpoint_error() {
        let ser = JsonSerializer;
        let invalid = b"{ not valid json ]";
        let result: Result<TestState, _> = ser.deserialize(invalid);
        assert!(result.is_err());
        let err = result.unwrap_err();
        match &err {
            CheckpointError::Serialization(s) => assert!(!s.is_empty()),
            _ => panic!("expected Serialization variant: {:?}", err),
        }
    }
}

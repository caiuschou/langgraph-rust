//! Streaming types for LangGraph runs.
//!
//! Defines stream modes and events for value, update, message, and custom
//! streaming. Used by `CompiledStateGraph::stream` and nodes that emit
//! incremental results.

use serde_json::Value;
use std::fmt::Debug;

/// Stream mode selector: which kinds of events to emit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StreamMode {
    /// Emit full state after each node completes.
    Values,
    /// Emit incremental updates with node id and state.
    Updates,
    /// Emit message chunks (LLM streaming).
    Messages,
    /// Emit custom JSON payloads from nodes or tools.
    Custom,
}

/// Metadata attached to streamed messages.
#[derive(Clone, Debug)]
pub struct StreamMetadata {
    /// LangGraph node id that produced the message.
    pub langgraph_node: String,
}

/// One chunk of streamed message content.
#[derive(Clone, Debug)]
pub struct MessageChunk {
    pub content: String,
}

/// Streamed event emitted while running a graph.
#[derive(Clone, Debug)]
pub enum StreamEvent<S>
where
    S: Clone + Send + Sync + Debug + 'static,
{
    /// Full state snapshot after a node finishes.
    Values(S),
    /// Incremental update with the node id and state after that node.
    Updates { node_id: String, state: S },
    /// Message chunk emitted by a node (e.g. ThinkNode streaming LLM output).
    Messages {
        chunk: MessageChunk,
        metadata: StreamMetadata,
    },
    /// Custom JSON payload for arbitrary streaming data.
    Custom(Value),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[derive(Clone, Debug, PartialEq)]
    struct DummyState(i32);

    /// **Scenario**: StreamMode four variants are distinct, Eq, and usable in HashSet.
    #[test]
    fn stream_mode_four_variants_hashset_equality() {
        let v = StreamMode::Values;
        let u = StreamMode::Updates;
        let m = StreamMode::Messages;
        let c = StreamMode::Custom;
        assert_eq!(v, StreamMode::Values);
        assert_ne!(v, u);
        assert_ne!(u, m);
        assert_ne!(m, c);
        assert_ne!(c, v);
        let set: HashSet<StreamMode> = [v, u, m, c].into_iter().collect();
        assert_eq!(set.len(), 4, "all four modes distinct in HashSet");
        assert!(set.contains(&StreamMode::Values));
        assert!(set.contains(&StreamMode::Custom));
    }

    /// **Scenario**: StreamEvent variants carry expected data.
    #[test]
    fn stream_event_variants_hold_data() {
        let values = StreamEvent::Values(DummyState(1));
        match values {
            StreamEvent::Values(DummyState(v)) => assert_eq!(v, 1),
            _ => panic!("expected Values variant"),
        }

        let updates = StreamEvent::Updates {
            node_id: "n1".into(),
            state: DummyState(2),
        };
        match updates {
            StreamEvent::Updates { node_id, state } => {
                assert_eq!(node_id, "n1");
                assert_eq!(state, DummyState(2));
            }
            _ => panic!("expected Updates variant"),
        }

        let messages: StreamEvent<DummyState> = StreamEvent::Messages {
            chunk: MessageChunk {
                content: "chunk".into(),
            },
            metadata: StreamMetadata {
                langgraph_node: "think".into(),
            },
        };
        match messages {
            StreamEvent::Messages { chunk, metadata } => {
                assert_eq!(chunk.content, "chunk");
                assert_eq!(metadata.langgraph_node, "think");
            }
            _ => panic!("expected Messages variant"),
        }

        let custom: StreamEvent<DummyState> = StreamEvent::Custom(serde_json::json!({"k": "v"}));
        match custom {
            StreamEvent::Custom(v) => assert_eq!(v["k"], "v"),
            _ => panic!("expected Custom variant"),
        }
    }
}

//! State types for graph-based agents.
//!
//! Contains ReAct state and tool types used by the minimal ReAct agent (Think/Act/Observe).
//! Design: docs/rust-langgraph/13-react-agent-design.md.

pub mod react_state;

pub use react_state::{ReActState, ToolCall, ToolResult};

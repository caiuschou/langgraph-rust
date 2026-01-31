//! State graph: nodes + linear edges, compile and invoke.
//!
//! Aligns with LangGraph `StateGraph`: add nodes and edges, compile, then
//! invoke with state. Design: docs/rust-langgraph/11-state-graph-design.md.

mod compile_error;
mod compiled;
mod next;
mod node;
mod state_graph;

pub use compile_error::CompilationError;
pub use compiled::CompiledStateGraph;
pub use next::Next;
pub use node::Node;
pub use state_graph::StateGraph;

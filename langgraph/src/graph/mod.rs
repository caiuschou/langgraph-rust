//! State graph: nodes + linear edges, compile and invoke.
//!
//! Aligns with LangGraph `StateGraph`: add nodes and edges, compile, then
//! invoke with state. Design: docs/rust-langgraph/11-state-graph-design.md.

mod compile_error;
mod compiled;
mod name_node;
mod next;
mod node;
mod node_middleware;
mod run_context;
mod state_graph;

pub use compile_error::CompilationError;
pub use compiled::CompiledStateGraph;
pub use name_node::NameNode;
pub use next::Next;
pub use node::Node;
pub use node_middleware::NodeMiddleware;
pub use run_context::RunContext;
pub use state_graph::{StateGraph, END, START};

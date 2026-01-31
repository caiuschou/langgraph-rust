//! Graph compilation error.
//!
//! Returned by `StateGraph::compile` when edge targets are not registered nodes.
//! Used only by the graph module.

use thiserror::Error;

/// Error when compiling a state graph (e.g. edge references unknown node).
///
/// Returned by `StateGraph::compile()`. Validation ensures every id in
/// `edge_order` exists in the node map.
#[derive(Debug, Error)]
pub enum CompilationError {
    /// A node id in the edge chain was not registered via `add_node`.
    #[error("node not found: {0}")]
    NodeNotFound(String),
}

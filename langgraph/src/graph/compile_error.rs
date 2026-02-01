//! Graph compilation error.
//!
//! Returned by `StateGraph::compile` when edges reference unknown nodes or
//! do not form a single linear chain from START to END.

use thiserror::Error;

/// Error when compiling a state graph (e.g. edge references unknown node, invalid chain).
///
/// Returned by `StateGraph::compile()`. Validation ensures every id in
/// edges (except START/END) exists in the node map and edges form exactly one
/// linear chain from START to END.
#[derive(Debug, Error)]
pub enum CompilationError {
    /// A node id in an edge was not registered via `add_node` (and is not START/END).
    #[error("node not found: {0}")]
    NodeNotFound(String),

    /// No edge has from_id == START, or more than one such edge.
    #[error("graph must have exactly one edge from START")]
    MissingStart,

    /// No edge has to_id == END, or more than one such edge.
    #[error("graph must have exactly one edge to END")]
    MissingEnd,

    /// Edges do not form a single linear chain (e.g. branch, cycle, disconnected).
    #[error("edges must form a single linear chain from START to END: {0}")]
    InvalidChain(String),
}

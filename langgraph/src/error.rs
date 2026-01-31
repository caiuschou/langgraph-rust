//! Agent execution error types.
//!
//! Used by `Agent::run` and all agents that implement the minimal Agent trait.

use thiserror::Error;

/// Agent execution error.
///
/// Returned by `Agent::run` when a step fails. Aligns with LangGraph-style
/// single-node execution; no separate error types for tools or LLM in this minimal API.
#[derive(Debug, Error)]
pub enum AgentError {
    /// Execution failed with a message (e.g. LLM call failed, tool error).
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
}

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

#[cfg(test)]
mod tests {
    use super::*;

    /// **Scenario**: Display format of ExecutionFailed contains "execution failed" and the message.
    #[test]
    fn agent_error_display_execution_failed() {
        let err = AgentError::ExecutionFailed("msg".to_string());
        let s = err.to_string();
        assert!(
            s.contains("execution failed"),
            "Display should contain 'execution failed': {}",
            s
        );
        assert!(s.contains("msg"), "Display should contain message: {}", s);
    }

    /// **Scenario**: Debug format includes variant name and message.
    #[test]
    fn agent_error_debug_format() {
        let err = AgentError::ExecutionFailed("test".to_string());
        let s = format!("{:?}", err);
        assert!(
            s.contains("ExecutionFailed"),
            "Debug should contain variant name: {}",
            s
        );
        assert!(s.contains("test"), "Debug should contain message: {}", s);
    }
}

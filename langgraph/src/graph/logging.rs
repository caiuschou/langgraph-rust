//! Logging utilities for graph execution.
//!
//! Provides structured logging for graph execution events, node execution,
//! state updates, and other important events.

/// Log node execution start.
///
/// This should be called when a node starts executing.
pub fn log_node_start(node_id: &str) {
    #[cfg(feature = "tracing")]
    tracing::debug!(node_id = node_id, "Starting node execution");

    #[cfg(not(feature = "tracing"))]
    eprintln!("[DEBUG] Starting node execution: {}", node_id);
}

/// Log node execution completion.
///
/// This should be called when a node completes execution.
pub fn log_node_complete(node_id: &str, next: &crate::graph::Next) {
    #[cfg(feature = "tracing")]
    tracing::debug!(node_id = node_id, ?next, "Node execution complete");

    #[cfg(not(feature = "tracing"))]
    eprintln!("[DEBUG] Node execution complete: {} -> {:?}", node_id, next);
}

/// Log state update.
///
/// This should be called when state is updated after node execution.
pub fn log_state_update(node_id: &str) {
    #[cfg(feature = "tracing")]
    tracing::debug!(node_id = node_id, "State updated");

    #[cfg(not(feature = "tracing"))]
    eprintln!("[DEBUG] State updated after node: {}", node_id);
}

/// Log graph execution start.
pub fn log_graph_start() {
    #[cfg(feature = "tracing")]
    tracing::info!("Starting graph execution");

    #[cfg(not(feature = "tracing"))]
    eprintln!("[INFO] Starting graph execution");
}

/// Log graph execution completion.
pub fn log_graph_complete() {
    #[cfg(feature = "tracing")]
    tracing::info!("Graph execution complete");

    #[cfg(not(feature = "tracing"))]
    eprintln!("[INFO] Graph execution complete");
}

/// Log graph execution error.
pub fn log_graph_error(error: &crate::error::AgentError) {
    #[cfg(feature = "tracing")]
    tracing::error!(?error, "Graph execution error");

    #[cfg(not(feature = "tracing"))]
    eprintln!("[ERROR] Graph execution error: {:?}", error);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logging_functions() {
        // These should not panic
        log_node_start("test_node");
        log_node_complete("test_node", &crate::graph::Next::End);
        log_state_update("test_node");
        log_graph_start();
        log_graph_complete();
        log_graph_error(&crate::error::AgentError::ExecutionFailed(
            "test".to_string(),
        ));
    }
}

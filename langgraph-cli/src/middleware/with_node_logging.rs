//! Extension trait for fluent API: attach node logging middleware then compile.
//!
//! Example of extending the build chain from outside langgraph.
//! Interacts with [`StateGraph`](langgraph::StateGraph), [`ReActState`](langgraph::ReActState),
//! and [`LoggingMiddleware`](super::logging::LoggingMiddleware).

use std::sync::Arc;

use langgraph::{ReActState, StateGraph};

use super::logging::LoggingMiddleware;

/// Extension trait for fluent API: attach node logging middleware then compile.
///
/// Returns the same graph with `LoggingMiddleware` attached. Chain with `.compile()?`.
pub trait WithNodeLogging {
    /// Returns the same graph with `LoggingMiddleware` attached. Chain with `.compile()?`.
    fn with_node_logging(self) -> Self;
}

impl WithNodeLogging for StateGraph<ReActState> {
    fn with_node_logging(self) -> Self {
        self.with_middleware(Arc::new(LoggingMiddleware))
    }
}

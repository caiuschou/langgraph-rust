//! Logging middleware that prints node enter/exit around each node.run call.

use async_trait::async_trait;
use std::pin::Pin;

use langgraph::{AgentError, NodeMiddleware, Next, ReActState};

/// Middleware that logs node enter/exit around each node.run call.
///
/// Logs to stderr so that normal output (Assistant messages) can be redirected separately.
pub struct LoggingMiddleware;

#[async_trait]
impl NodeMiddleware<ReActState> for LoggingMiddleware {
    async fn around_run(
        &self,
        node_id: &str,
        state: ReActState,
        inner: Box<
            dyn FnOnce(ReActState)
                -> Pin<
                    Box<
                        dyn std::future::Future<Output = Result<(ReActState, Next), AgentError>>
                            + Send,
                    >,
                > + Send,
        >,
    ) -> Result<(ReActState, Next), AgentError> {
        eprintln!("[node] enter node={}", node_id);
        let result = inner(state).await;
        match &result {
            Ok((_, ref next)) => eprintln!("[node] exit node={} next={:?}", node_id, next),
            Err(e) => eprintln!("[node] exit node={} error={}", node_id, e),
        }
        result
    }
}

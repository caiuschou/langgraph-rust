//! Act node: read tool_calls, call ToolSource for each, write tool_results.
//!
//! Design: docs/rust-langgraph/13-react-agent-design.md §8.3 stage 3.3–3.4.
//! ActNode holds a ToolSource (e.g. `Box<dyn ToolSource>`), implements `Node<ReActState>`;
//! run reads state.tool_calls, calls call_tool(name, args) for each, writes state.tool_results.
//!
//! # Error Handling
//!
//! By default, tool errors propagate and short-circuit the graph. Use `with_handle_tool_errors`
//! to configure error handling:
//!
//! - `HandleToolErrors::Never` - Errors propagate (default)
//! - `HandleToolErrors::Always` - Errors are caught and returned as error messages
//! - `HandleToolErrors::Custom(handler)` - Custom error handler function
//!
//! # Streaming Support
//!
//! `ActNode` supports custom streaming through `run_with_context`. When called with
//! a `RunContext` that has `StreamMode::Custom` enabled, it creates a `ToolStreamWriter`
//! and passes it to tools via `ToolCallContext`. Tools can then emit progress updates
//! or intermediate results during execution.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, trace, warn};

use crate::error::AgentError;
use crate::graph::{Next, Node, RunContext};
use crate::state::{ReActState, ToolResult};
use crate::stream::{StreamEvent, StreamMode, ToolStreamWriter};
use crate::tool_source::{ToolCallContext, ToolSource, ToolSourceError};

/// Truncates a string for logging, appending "..." if longer than max_len.
/// Used for tool result preview in tracing to avoid huge log lines.
fn truncate_for_log(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", s.chars().take(max_len).collect::<String>())
    }
}

/// Default error message template for tool errors.
pub const DEFAULT_TOOL_ERROR_TEMPLATE: &str = "Error: {error}\n Please fix your mistakes.";

/// Default execution error message template with tool name and kwargs.
pub const DEFAULT_EXECUTION_ERROR_TEMPLATE: &str =
    "Error executing tool '{tool_name}' with kwargs {tool_kwargs} with error:\n {error}\n Please fix the error and try again.";

/// Error handler function type.
///
/// Takes the error, tool name, and tool arguments, returns an error message string.
pub type ErrorHandlerFn =
    Arc<dyn Fn(&ToolSourceError, &str, &Value) -> String + Send + Sync + 'static>;

/// Configuration for how ActNode handles tool errors.
///
/// Aligns with Python's `handle_tool_errors` parameter in ToolNode.
#[derive(Clone)]
pub enum HandleToolErrors {
    /// Errors propagate and short-circuit the graph (default behavior).
    Never,
    /// Errors are caught and returned as ToolResult with error message.
    /// Uses the default error template if None.
    Always(Option<String>),
    /// Custom error handler function.
    Custom(ErrorHandlerFn),
}

impl Default for HandleToolErrors {
    fn default() -> Self {
        Self::Never
    }
}

impl std::fmt::Debug for HandleToolErrors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Never => write!(f, "HandleToolErrors::Never"),
            Self::Always(msg) => write!(f, "HandleToolErrors::Always({:?})", msg),
            Self::Custom(_) => write!(f, "HandleToolErrors::Custom(<fn>)"),
        }
    }
}

/// Act node: one ReAct step that executes tool_calls and produces tool_results.
///
/// Reads `state.tool_calls`, calls `ToolSource::call_tool(name, arguments)` for each
/// (parsing arguments from JSON string); appends one ToolResult per call.
///
/// # Error Handling
///
/// By default (HandleToolErrors::Never), a single call failure returns Err and
/// short-circuits the graph. Use `with_handle_tool_errors` to configure error handling:
///
/// ```rust,ignore
/// let act = ActNode::new(tools)
///     .with_handle_tool_errors(HandleToolErrors::Always(None));
/// ```
///
/// **Interaction**: Implements `Node<ReActState>`; used by StateGraph. Consumes
/// `ToolSource` (e.g. MockToolSource); reads ReActState.tool_calls, writes
/// ReActState.tool_results. See docs/rust-langgraph/mcp-integration/README.md.
pub struct ActNode {
    /// Tool source used to execute each tool call.
    tools: Box<dyn ToolSource>,
    /// Error handling configuration.
    handle_tool_errors: HandleToolErrors,
}

impl ActNode {
    /// Creates an Act node with the given tool source.
    ///
    /// By default, tool errors propagate (HandleToolErrors::Never).
    pub fn new(tools: Box<dyn ToolSource>) -> Self {
        Self {
            tools,
            handle_tool_errors: HandleToolErrors::Never,
        }
    }

    /// Sets the error handling configuration.
    ///
    /// # Arguments
    ///
    /// * `handle_tool_errors` - How to handle tool errors:
    ///   - `Never` - Errors propagate (default)
    ///   - `Always(None)` - Catch errors with default message
    ///   - `Always(Some(msg))` - Catch errors with custom message
    ///   - `Custom(handler)` - Use custom error handler
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Catch all errors with default message
    /// let act = ActNode::new(tools)
    ///     .with_handle_tool_errors(HandleToolErrors::Always(None));
    ///
    /// // Catch errors with custom message
    /// let act = ActNode::new(tools)
    ///     .with_handle_tool_errors(HandleToolErrors::Always(Some("Custom error".into())));
    ///
    /// // Custom error handler
    /// let act = ActNode::new(tools)
    ///     .with_handle_tool_errors(HandleToolErrors::Custom(Arc::new(|e, name, _args| {
    ///         format!("Tool {} failed: {}", name, e)
    ///     })));
    /// ```
    pub fn with_handle_tool_errors(mut self, handle_tool_errors: HandleToolErrors) -> Self {
        self.handle_tool_errors = handle_tool_errors;
        self
    }

    /// Handles a tool error according to the configured error handling mode.
    ///
    /// Returns Some(error_message) if the error should be caught and returned as a result,
    /// or None if the error should propagate.
    fn handle_error(
        &self,
        error: &ToolSourceError,
        tool_name: &str,
        tool_args: &Value,
    ) -> Option<String> {
        match &self.handle_tool_errors {
            HandleToolErrors::Never => None,
            HandleToolErrors::Always(custom_msg) => {
                let msg = custom_msg.clone().unwrap_or_else(|| {
                    DEFAULT_EXECUTION_ERROR_TEMPLATE
                        .replace("{tool_name}", tool_name)
                        .replace("{tool_kwargs}", &tool_args.to_string())
                        .replace("{error}", &error.to_string())
                });
                Some(msg)
            }
            HandleToolErrors::Custom(handler) => Some(handler(error, tool_name, tool_args)),
        }
    }
}

#[async_trait]
impl Node<ReActState> for ActNode {
    fn id(&self) -> &str {
        "act"
    }

    /// Reads state.tool_calls, calls call_tool_with_context for each, writes tool_results.
    /// Passes ToolCallContext (recent_messages) explicitly so tools like get_recent_messages
    /// receive current conversation without internal state. Also calls set_call_context for
    /// backward compatibility. Returns Next::Continue.
    ///
    /// # Error Handling
    ///
    /// If `handle_tool_errors` is:
    /// - `Never` (default): Errors propagate and short-circuit the graph
    /// - `Always`: Errors are caught and returned as error messages in ToolResult
    /// - `Custom(handler)`: Custom handler is called to generate error message
    ///
    /// This is the basic version without streaming support. For streaming support,
    /// use `run_with_context` which passes a `ToolStreamWriter` to tools.
    async fn run(&self, state: ReActState) -> Result<(ReActState, Next), AgentError> {
        let ctx = ToolCallContext::new(state.messages.clone());
        self.tools.set_call_context(Some(ctx.clone()));
        let mut tool_results = Vec::with_capacity(state.tool_calls.len());

        for tc in &state.tool_calls {
            let args: Value = if tc.arguments.trim().is_empty() {
                serde_json::json!({})
            } else {
                serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!({}))
            };

            debug!(tool = %tc.name, args = ?args, "Calling tool");

            let result = self
                .tools
                .call_tool_with_context(&tc.name, args.clone(), Some(&ctx))
                .await;

            match result {
                Ok(content) => {
                    trace!(
                        tool = %tc.name,
                        result_len = content.text.len(),
                        result_preview = %truncate_for_log(&content.text, 200),
                        "Tool returned"
                    );
                    tool_results.push(ToolResult {
                        call_id: tc.id.clone(),
                        name: Some(tc.name.clone()),
                        content: content.text,
                    });
                }
                Err(e) => {
                    warn!(tool = %tc.name, error = %e, "Tool call failed");
                    if let Some(error_msg) = self.handle_error(&e, &tc.name, &args) {
                        // Error is handled - add as error result
                        tool_results.push(ToolResult {
                            call_id: tc.id.clone(),
                            name: Some(tc.name.clone()),
                            content: error_msg,
                        });
                    } else {
                        // Error propagates
                        self.tools.set_call_context(None);
                        return Err(AgentError::ExecutionFailed(e.to_string()));
                    }
                }
            }
        }

        self.tools.set_call_context(None);
        let new_state = ReActState {
            messages: state.messages,
            tool_calls: state.tool_calls,
            tool_results,
            turn_count: state.turn_count,
        };
        Ok((new_state, Next::Continue))
    }

    /// Reads state.tool_calls, calls call_tool_with_context for each, writes tool_results.
    ///
    /// This version supports custom streaming: when `StreamMode::Custom` is enabled in the
    /// run context, it creates a `ToolStreamWriter` and passes it to tools via `ToolCallContext`.
    /// Tools can then emit progress updates or intermediate results during execution.
    ///
    /// # Error Handling
    ///
    /// Same as `run`: respects `handle_tool_errors` configuration.
    ///
    /// # Streaming
    ///
    /// When `run_ctx.stream_mode` contains `StreamMode::Custom`:
    /// - A `ToolStreamWriter` is created from the run context's stream sender
    /// - The writer is passed to each tool via `ToolCallContext::stream_writer`
    /// - Tools can call `ctx.emit_custom(json!({"progress": 50}))` to emit events
    /// - Events are sent as `StreamEvent::Custom(Value)` to the stream consumer
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // In a tool implementation:
    /// async fn call(&self, args: Value, ctx: Option<&ToolCallContext>) -> Result<ToolCallContent, ToolSourceError> {
    ///     if let Some(ctx) = ctx {
    ///         ctx.emit_custom(serde_json::json!({"status": "starting"}));
    ///     }
    ///     // Do work...
    ///     Ok(ToolCallContent { text: "Done".to_string() })
    /// }
    /// ```
    async fn run_with_context(
        &self,
        state: ReActState,
        run_ctx: &RunContext<ReActState>,
    ) -> Result<(ReActState, Next), AgentError> {
        // Create ToolStreamWriter if Custom streaming is enabled
        let tool_writer = if run_ctx.stream_mode.contains(&StreamMode::Custom) {
            if let Some(tx) = &run_ctx.stream_tx {
                let tx = tx.clone();
                ToolStreamWriter::new(move |value| tx.try_send(StreamEvent::Custom(value)).is_ok())
            } else {
                ToolStreamWriter::noop()
            }
        } else {
            ToolStreamWriter::noop()
        };

        // Create ToolCallContext with stream writer
        let ctx = ToolCallContext::with_stream_writer(state.messages.clone(), tool_writer);
        self.tools.set_call_context(Some(ctx.clone()));

        let mut tool_results = Vec::with_capacity(state.tool_calls.len());

        for tc in &state.tool_calls {
            let args: Value = if tc.arguments.trim().is_empty() {
                serde_json::json!({})
            } else {
                serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!({}))
            };

            debug!(tool = %tc.name, args = ?args, "Calling tool");

            let result = self
                .tools
                .call_tool_with_context(&tc.name, args.clone(), Some(&ctx))
                .await;

            match result {
                Ok(content) => {
                    trace!(
                        tool = %tc.name,
                        result_len = content.text.len(),
                        result_preview = %truncate_for_log(&content.text, 200),
                        "Tool returned"
                    );
                    tool_results.push(ToolResult {
                        call_id: tc.id.clone(),
                        name: Some(tc.name.clone()),
                        content: content.text,
                    });
                }
                Err(e) => {
                    warn!(tool = %tc.name, error = %e, "Tool call failed");
                    if let Some(error_msg) = self.handle_error(&e, &tc.name, &args) {
                        tool_results.push(ToolResult {
                            call_id: tc.id.clone(),
                            name: Some(tc.name.clone()),
                            content: error_msg,
                        });
                    } else {
                        self.tools.set_call_context(None);
                        return Err(AgentError::ExecutionFailed(e.to_string()));
                    }
                }
            }
        }

        self.tools.set_call_context(None);

        let new_state = ReActState {
            messages: state.messages,
            tool_calls: state.tool_calls,
            tool_results,
            turn_count: state.turn_count,
        };
        Ok((new_state, Next::Continue))
    }
}

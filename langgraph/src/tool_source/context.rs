//! Context passed into tool calls for the current step.
//!
//! Used by short-term memory tools (e.g. `get_recent_messages`) that need access to
//! the current conversation. ActNode sets this via `ToolSource::set_call_context` before
//! executing tool calls. See `docs/rust-langgraph/tools-refactor/overview.md` §3.2.
//!
//! # Streaming Support
//!
//! `ToolCallContext` includes an optional `stream_writer` field that enables tools
//! to emit custom streaming events (e.g., progress updates, intermediate results)
//! during execution. The writer is provided by `ActNode` when streaming is enabled.
//!
//! ```rust,ignore
//! use langgraph::tool_source::ToolCallContext;
//! use serde_json::json;
//!
//! async fn my_tool(ctx: Option<&ToolCallContext>) -> String {
//!     if let Some(ctx) = ctx {
//!         if let Some(writer) = &ctx.stream_writer {
//!             writer.emit_custom(json!({"status": "starting"}));
//!         }
//!     }
//!     // Do work...
//!     "Result".to_string()
//! }
//! ```

use crate::message::Message;
use crate::stream::ToolStreamWriter;

/// Per-step context available to tools during execution.
///
/// Injected by ActNode before calling tools; implementations that need current
/// messages (e.g. ShortTermMemoryToolSource) read it in `call_tool`. Other
/// ToolSource implementations ignore it (default `set_call_context` is no-op).
///
/// # Fields
///
/// - `recent_messages`: Current conversation messages from state
/// - `stream_writer`: Optional writer for emitting custom streaming events
///
/// # Streaming
///
/// When streaming is enabled and `StreamMode::Custom` is active, `ActNode` provides
/// a `ToolStreamWriter` that tools can use to emit progress updates or intermediate
/// results. This enables real-time feedback during long-running tool operations.
///
/// **Interaction**: Set by ActNode via `ToolSource::set_call_context`; read by
/// `ShortTermMemoryToolSource::call_tool` to return recent messages; `stream_writer`
/// used by any tool that wants to emit custom streaming events.
#[derive(Debug, Clone, Default)]
pub struct ToolCallContext {
    /// Recent messages in the current conversation (current step's state.messages).
    pub recent_messages: Vec<Message>,

    /// Optional writer for emitting custom streaming events.
    ///
    /// This is provided by `ActNode` when streaming is enabled with `StreamMode::Custom`.
    /// Tools can use this to emit progress updates, intermediate results, or any
    /// custom JSON data during execution.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// if let Some(writer) = &ctx.stream_writer {
    ///     writer.emit_custom(serde_json::json!({"progress": 50}));
    /// }
    /// ```
    pub stream_writer: Option<ToolStreamWriter>,
}

impl ToolCallContext {
    /// Creates a new ToolCallContext with the given messages.
    pub fn new(recent_messages: Vec<Message>) -> Self {
        Self {
            recent_messages,
            stream_writer: None,
        }
    }

    /// Creates a new ToolCallContext with messages and a stream writer.
    pub fn with_stream_writer(recent_messages: Vec<Message>, stream_writer: ToolStreamWriter) -> Self {
        Self {
            recent_messages,
            stream_writer: Some(stream_writer),
        }
    }

    /// Emits a custom streaming event if a writer is available.
    ///
    /// This is a convenience method that checks if `stream_writer` is present
    /// and calls `emit_custom` on it. Returns `true` if the event was sent,
    /// `false` if no writer is available or sending failed.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let sent = ctx.emit_custom(serde_json::json!({"status": "processing"}));
    /// ```
    pub fn emit_custom(&self, value: serde_json::Value) -> bool {
        self.stream_writer
            .as_ref()
            .map(|w| w.emit_custom(value))
            .unwrap_or(false)
    }
}

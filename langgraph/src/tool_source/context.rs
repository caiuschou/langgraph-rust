//! Context passed into tool calls for the current step.
//!
//! Used by short-term memory tools (e.g. `get_recent_messages`) that need access to
//! the current conversation. ActNode sets this via `ToolSource::set_call_context` before
//! executing tool calls. See `docs/rust-langgraph/tools-refactor/overview.md` §3.2.

use crate::message::Message;

/// Per-step context available to tools during execution.
///
/// Injected by ActNode before calling tools; implementations that need current
/// messages (e.g. ShortTermMemoryToolSource) read it in `call_tool`. Other
/// ToolSource implementations ignore it (default `set_call_context` is no-op).
///
/// **Interaction**: Set by ActNode via `ToolSource::set_call_context`; read by
/// `ShortTermMemoryToolSource::call_tool` to return recent messages.
#[derive(Debug, Clone, Default)]
pub struct ToolCallContext {
    /// Recent messages in the current conversation (current step's state.messages).
    pub recent_messages: Vec<Message>,
}

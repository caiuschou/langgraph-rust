//! LLM client abstraction for ReAct Think node.
//!
//! Design: docs/rust-langgraph/13-react-agent-design.md §8.2 stage 2.1–2.2.
//! ThinkNode depends on a callable that returns assistant text and optional
//! tool_calls; this module defines the trait and a mock implementation.

mod mock;

#[cfg(feature = "zhipu")]
mod openai;
#[cfg(feature = "zhipu")]
mod zhipu;

pub use mock::MockLlm;

#[cfg(feature = "zhipu")]
pub use openai::ChatOpenAI;
#[cfg(feature = "zhipu")]
pub use zhipu::ChatZhipu;

use async_trait::async_trait;

use crate::error::AgentError;
use crate::message::Message;
use crate::state::ToolCall;

/// Response from an LLM completion: assistant message text and optional tool calls.
///
/// **Interaction**: Returned by `LlmClient::invoke()`; ThinkNode writes
/// `content` into a new assistant message and `tool_calls` into `ReActState::tool_calls`.
pub struct LlmResponse {
    /// Assistant message content (plain text).
    pub content: String,
    /// Tool calls from this turn; empty means no tools, observe → END.
    pub tool_calls: Vec<ToolCall>,
}

/// LLM client: given messages, returns assistant text and optional tool_calls.
///
/// ThinkNode calls this to produce the next assistant message and any tool
/// invocations. Implementations: `MockLlm` (fixed response), `ChatOpenAI` / `ChatZhipu` (real API, feature `zhipu`).
///
/// **Interaction**: Used by ThinkNode; see docs/rust-langgraph/13-react-agent-design.md §4 and §8.2.
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Invoke one turn: read messages, return assistant content and optional tool_calls.
    /// Aligns with LangChain's `invoke` / `ainvoke` (single-call API).
    async fn invoke(&self, messages: &[Message]) -> Result<LlmResponse, AgentError>;
}

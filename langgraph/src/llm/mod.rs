//! LLM client abstraction for ReAct Think node.
//!
//! Design: docs/rust-langgraph/13-react-agent-design.md §8.2 stage 2.1–2.2.
//! ThinkNode depends on a callable that returns assistant text and optional
//! tool_calls; this module defines the trait and a mock implementation.

mod mock;

/// Tool choice mode for chat completions: when tools are present, controls whether
/// the model may choose (auto), must not use (none), or must use (required).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ToolChoiceMode {
    /// Model can pick between message or tool calls. Default when tools are present.
    #[default]
    Auto,
    /// Model will not call any tool.
    None,
    /// Model must call one or more tools.
    Required,
}

impl std::str::FromStr for ToolChoiceMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "none" => Ok(Self::None),
            "required" => Ok(Self::Required),
            _ => Err(format!(
                "unknown tool_choice: {} (use auto, none, or required)",
                s
            )),
        }
    }
}

#[cfg(feature = "openai")]
mod openai;

pub use mock::MockLlm;

#[cfg(feature = "openai")]
pub use openai::ChatOpenAI;

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
/// invocations. Implementations: `MockLlm` (fixed response), `ChatOpenAI` (real API, feature `openai`).
///
/// **Interaction**: Used by ThinkNode; see docs/rust-langgraph/13-react-agent-design.md §4 and §8.2.
#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Invoke one turn: read messages, return assistant content and optional tool_calls.
    /// Aligns with LangChain's `invoke` / `ainvoke` (single-call API).
    async fn invoke(&self, messages: &[Message]) -> Result<LlmResponse, AgentError>;
}

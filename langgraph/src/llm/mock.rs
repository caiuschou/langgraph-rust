//! Mock LLM for tests and examples.
//!
//! Design: docs/rust-langgraph/13-react-agent-design.md §8.2 stage 2.2.
//! Returns fixed assistant message and optional fixed ToolCall (e.g. get_time);
//! configurable "no tool_calls" to test END path. Optional stateful mode for multi-round.

use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;

use crate::error::AgentError;
use crate::llm::{LlmClient, LlmResponse};
use crate::message::Message;
use crate::state::ToolCall;

/// Mock LLM: fixed assistant text and optional tool_calls.
///
/// Configurable to return one fixed ToolCall (e.g. get_time) or no tool_calls,
/// so the graph can run one round (think → act → observe → END) or test END
/// after think. Used by ThinkNode in tests and ReAct linear example.
/// Optional stateful mode: first call returns tool_calls, second returns no tool_calls (multi-round).
///
/// **Interaction**: Implements `LlmClient`; used by ThinkNode.
pub struct MockLlm {
    /// Assistant message content to return (or first call when stateful).
    content: String,
    /// Tool calls to return (or first call when stateful).
    tool_calls: Vec<ToolCall>,
    /// When Some, first invoke() returns (content, tool_calls), later returns (second_content, []).
    call_count: Option<AtomicUsize>,
    /// Second response content (stateful mode).
    second_content: Option<String>,
}

impl MockLlm {
    /// Creates a mock that returns one assistant message and one tool call (get_time).
    ///
    /// Aligns with docs/rust-langgraph/13-react-agent-design.md §8.2: fixed single assistant message and single ToolCall (e.g. get_time).
    pub fn with_get_time_call() -> Self {
        Self {
            content: "I'll check the time.".to_string(),
            tool_calls: vec![ToolCall {
                name: "get_time".to_string(),
                arguments: "{}".to_string(),
                id: Some("call-1".to_string()),
            }],
            call_count: None,
            second_content: None,
        }
    }

    /// Creates a mock that returns assistant text and no tool_calls (END path).
    pub fn with_no_tool_calls(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            tool_calls: vec![],
            call_count: None,
            second_content: None,
        }
    }

    /// Creates a mock with custom content and tool_calls.
    pub fn new(content: impl Into<String>, tool_calls: Vec<ToolCall>) -> Self {
        Self {
            content: content.into(),
            tool_calls,
            call_count: None,
            second_content: None,
        }
    }

    /// Creates a stateful mock: first invoke() returns get_time tool_call, second returns no tool_calls.
    /// Used for multi-round ReAct tests (phase 5).
    pub fn first_tools_then_end() -> Self {
        Self {
            content: "I'll check the time.".to_string(),
            tool_calls: vec![ToolCall {
                name: "get_time".to_string(),
                arguments: "{}".to_string(),
                id: Some("call-1".to_string()),
            }],
            call_count: Some(AtomicUsize::new(0)),
            second_content: Some("The time is as above.".to_string()),
        }
    }

    /// Set content (builder).
    pub fn with_content(mut self, content: impl Into<String>) -> Self {
        self.content = content.into();
        self
    }

    /// Set tool_calls (builder).
    pub fn with_tool_calls(mut self, tool_calls: Vec<ToolCall>) -> Self {
        self.tool_calls = tool_calls;
        self
    }
}

#[async_trait]
impl LlmClient for MockLlm {
    async fn invoke(&self, _messages: &[Message]) -> Result<LlmResponse, AgentError> {
        let (content, tool_calls) = match &self.call_count {
            Some(c) => {
                let n = c.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    (self.content.clone(), self.tool_calls.clone())
                } else {
                    (
                        self.second_content
                            .as_deref()
                            .unwrap_or(&self.content)
                            .to_string(),
                        vec![],
                    )
                }
            }
            None => (self.content.clone(), self.tool_calls.clone()),
        };
        Ok(LlmResponse { content, tool_calls })
    }
}

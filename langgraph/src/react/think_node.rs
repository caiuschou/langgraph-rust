//! Think node: read messages, call LLM, write assistant message and optional tool_calls.
//!
//! Design: docs/rust-langgraph/13-react-agent-design.md §8.3 stage 3.1–3.2.
//! ThinkNode holds an LLM client (e.g. MockLlm or `Box<dyn LlmClient>`), implements
//! `Node<ReActState>`; run reads state.messages, calls LLM, appends one assistant message
//! and sets state.tool_calls from the response (empty when no tools).

use async_trait::async_trait;

use crate::error::AgentError;
use crate::graph::Next;
use crate::Node;
use crate::llm::LlmClient;
use crate::message::Message;
use crate::state::ReActState;

/// Think node: one ReAct step that produces assistant message and optional tool_calls.
///
/// Reads `state.messages`, calls the LLM, appends one assistant message and sets
/// `state.tool_calls` from the response. When the LLM returns no tool_calls, the
/// graph can end after observe. Does not call ToolSource::list_tools in this minimal
/// version (prompt can be fixed).
///
/// **Interaction**: Implements `Node<ReActState>`; used by StateGraph. Consumes
/// `LlmClient` (e.g. MockLlm); writes to ReActState.messages and ReActState.tool_calls.
pub struct ThinkNode {
    /// LLM client used to produce assistant message and optional tool_calls.
    llm: Box<dyn LlmClient>,
}

impl ThinkNode {
    /// Creates a Think node with the given LLM client.
    pub fn new(llm: Box<dyn LlmClient>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl Node<ReActState> for ThinkNode {
    fn id(&self) -> &str {
        "think"
    }

    /// Reads state.messages, calls LLM, appends assistant message and sets tool_calls.
    /// Returns Next::Continue to follow linear edge order (e.g. think → act).
    async fn run(&self, state: ReActState) -> Result<(ReActState, Next), AgentError> {
        let response = self.llm.invoke(&state.messages).await?;
        let mut messages = state.messages;
        messages.push(Message::Assistant(response.content));
        let new_state = ReActState {
            messages,
            tool_calls: response.tool_calls,
            tool_results: state.tool_results,
        };
        Ok((new_state, Next::Continue))
    }
}

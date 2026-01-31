//! Minimal message types for agent state.
//!
//! Aligns with LangGraph/LangChain: System (usually first in the list), User, Assistant.
//! Used by `AgentState::messages` and by agents that read/append messages in `Agent::run`.

/// A single message in the conversation.
///
/// Roles match LangGraph: system prompt, user input, assistant reply.
/// No separate Tool role in this minimal design; extend in later Sprints.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Message {
    /// System prompt; typically placed first in the message list.
    System(String),
    /// User input.
    User(String),
    /// Model/agent reply.
    Assistant(String),
}

impl Message {
    /// Creates a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self::System(content.into())
    }

    /// Creates a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self::User(content.into())
    }

    /// Creates an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::Assistant(content.into())
    }
}

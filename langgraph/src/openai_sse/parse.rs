//! Parse OpenAI-style chat request into ReAct runner inputs.
//!
//! Used by HTTP handlers to build `user_message`, `system_prompt`, and
//! [`RunnableConfig`](crate::memory::RunnableConfig) from [`ChatCompletionRequest`].

use crate::memory::RunnableConfig;
use crate::react::REACT_SYSTEM_PROMPT;
use super::request::ChatCompletionRequest;
use thiserror::Error;

/// Result of parsing a chat completion request for the ReAct runner.
#[derive(Debug, Clone)]
pub struct ParsedChatRequest {
    /// Last user message content (input for this turn).
    pub user_message: String,
    /// System prompt; use with `build_react_initial_state(..., system_prompt, ...)`.
    pub system_prompt: String,
    /// Config for checkpointer (thread_id etc.); use with invoke/stream.
    pub runnable_config: RunnableConfig,
    /// Whether to include usage in the final SSE chunk.
    pub include_usage: bool,
}

/// Errors while parsing a chat completion request.
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("no user message in messages")]
    NoUserMessage,
}

/// Parses an OpenAI-style request into ReAct runner inputs.
///
/// - **user_message**: Last message with `role == "user"`; its `content` (or empty string if null).
/// - **system_prompt**: First message with `role == "system"` content, or [`REACT_SYSTEM_PROMPT`].
/// - **runnable_config**: `thread_id` from request if present; otherwise default.
/// - **include_usage**: From `stream_options.include_usage` (default false).
///
/// # Errors
///
/// Returns `ParseError::NoUserMessage` if no message has `role == "user"`.
pub fn parse_chat_request(req: &ChatCompletionRequest) -> Result<ParsedChatRequest, ParseError> {
    let user_message = req
        .messages
        .iter()
        .rev()
        .find(|m| m.role.eq_ignore_ascii_case("user"))
        .and_then(|m| m.content.as_ref().map(|c| c.as_text()))
        .unwrap_or_default();

    let has_user = req.messages.iter().any(|m| m.role.eq_ignore_ascii_case("user"));
    if !has_user {
        return Err(ParseError::NoUserMessage);
    }

    let system_prompt = req
        .messages
        .iter()
        .find(|m| m.role.eq_ignore_ascii_case("system"))
        .and_then(|m| m.content.as_ref().map(|c| c.as_text()))
        .unwrap_or_else(|| REACT_SYSTEM_PROMPT.to_string());

    let runnable_config = RunnableConfig {
        thread_id: req.thread_id.clone(),
        checkpoint_id: None,
        checkpoint_ns: String::new(),
        user_id: None,
    };

    let include_usage = req
        .stream_options
        .as_ref()
        .map(|o| o.include_usage)
        .unwrap_or(false);

    Ok(ParsedChatRequest {
        user_message,
        system_prompt,
        runnable_config,
        include_usage,
    })
}

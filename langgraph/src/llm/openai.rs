//! OpenAI Chat Completions client implementing `LlmClient` (ChatOpenAI).
//!
//! Uses the real OpenAI Chat Completions API. Requires `OPENAI_API_KEY` (or
//! explicit config). Optional tools can be set for function/tool calling;
//! when present, the API may return `tool_calls` in the response.
//!
//! **Interaction**: Implements `LlmClient`; used by ThinkNode like `MockLlm`.
//! Depends on `async_openai` (feature `zhipu`).

use async_trait::async_trait;

use crate::error::AgentError;
use crate::llm::{LlmClient, LlmResponse};
use crate::message::Message;
use crate::state::ToolCall;
use crate::tool_source::ToolSpec;

use async_openai::{
    config::OpenAIConfig,
    types::chat::{
        ChatCompletionMessageToolCalls, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessage,
        ChatCompletionTool, ChatCompletionTools, CreateChatCompletionRequestArgs,
        FunctionObject,
    },
    Client,
};

/// OpenAI Chat Completions client implementing `LlmClient` (aligns with LangChain ChatOpenAI).
///
/// Uses `OPENAI_API_KEY` from the environment by default; or provide
/// config via `ChatOpenAI::with_config`. Optionally set tools (e.g. from
/// `ToolSource::list_tools()`) to enable tool_calls in the response.
///
/// **Interaction**: Implements `LlmClient`; used by ThinkNode.
pub struct ChatOpenAI {
    client: Client<OpenAIConfig>,
    model: String,
    tools: Option<Vec<ToolSpec>>,
}

impl ChatOpenAI {
    /// Build client with default config (API key from `OPENAI_API_KEY` env).
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            model: model.into(),
            tools: None,
        }
    }

    /// Build client with custom config (e.g. custom API key or base URL).
    pub fn with_config(config: OpenAIConfig, model: impl Into<String>) -> Self {
        Self {
            client: Client::with_config(config),
            model: model.into(),
            tools: None,
        }
    }

    /// Set tools for this completion (enables tool_calls in response).
    pub fn with_tools(mut self, tools: Vec<ToolSpec>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Convert our `Message` list to OpenAI request messages (system/user/assistant text only).
    fn messages_to_request(messages: &[Message]) -> Vec<ChatCompletionRequestMessage> {
        messages
            .iter()
            .map(|m| match m {
                Message::System(s) => {
                    ChatCompletionRequestMessage::System(ChatCompletionRequestSystemMessage::from(
                        s.as_str(),
                    ))
                }
                Message::User(s) => {
                    ChatCompletionRequestMessage::User(ChatCompletionRequestUserMessage::from(s.as_str()))
                }
                Message::Assistant(s) => ChatCompletionRequestMessage::Assistant(
                    (s.as_str()).into(),
                ),
            })
            .collect()
    }
}

#[async_trait]
impl LlmClient for ChatOpenAI {
    async fn invoke(&self, messages: &[Message]) -> Result<LlmResponse, AgentError> {
        let openai_messages = Self::messages_to_request(messages);
        let mut args = CreateChatCompletionRequestArgs::default();
        args.model(self.model.clone());
        args.messages(openai_messages);

        if let Some(ref tools) = self.tools {
            let chat_tools: Vec<ChatCompletionTools> = tools
                .iter()
                .map(|t| {
                    ChatCompletionTools::Function(ChatCompletionTool {
                        function: FunctionObject {
                            name: t.name.clone(),
                            description: t.description.clone(),
                            parameters: Some(t.input_schema.clone()),
                            ..Default::default()
                        },
                    })
                })
                .collect();
            args.tools(chat_tools);
        }

        let request = args.build().map_err(|e| {
            AgentError::ExecutionFailed(format!("OpenAI request build failed: {}", e))
        })?;

        let response = self.client.chat().create(request).await.map_err(|e| {
            AgentError::ExecutionFailed(format!("OpenAI API error: {}", e))
        })?;

        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| AgentError::ExecutionFailed("OpenAI returned no choices".to_string()))?;

        let msg = choice.message;
        let content = msg.content.unwrap_or_default();
        let tool_calls: Vec<ToolCall> = msg
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .filter_map(|tc| {
                if let ChatCompletionMessageToolCalls::Function(f) = tc {
                    Some(ToolCall {
                        name: f.function.name,
                        arguments: f.function.arguments,
                        id: Some(f.id),
                    })
                } else {
                    None
                }
            })
            .collect();

        Ok(LlmResponse { content, tool_calls })
    }
}

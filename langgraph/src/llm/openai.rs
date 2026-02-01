//! OpenAI Chat Completions client implementing `LlmClient` (ChatOpenAI).
//!
//! Uses the real OpenAI Chat Completions API. Requires `OPENAI_API_KEY` (or
//! explicit config). Optional tools can be set for function/tool calling;
//! when present, API may return `tool_calls` in the response.
//!
//! **Interaction**: Implements `LlmClient`; used by ThinkNode like `MockLlm`.
//! Depends on `async_openai` (feature `openai`).

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
        ChatCompletionRequestSystemMessage, ChatCompletionRequestUserMessage, ChatCompletionTool,
        ChatCompletionToolChoiceOption, ChatCompletionTools, CreateChatCompletionRequestArgs,
        FunctionObject, ToolChoiceOptions,
    },
    Client,
};

use super::ToolChoiceMode;

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
    temperature: Option<f32>,
    tool_choice: Option<ToolChoiceMode>,
}

impl ChatOpenAI {
    /// Build client with default config (API key from `OPENAI_API_KEY` env).
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            model: model.into(),
            tools: None,
            temperature: None,
            tool_choice: None,
        }
    }

    /// Build client with custom config (e.g. custom API key or base URL).
    pub fn with_config(config: OpenAIConfig, model: impl Into<String>) -> Self {
        Self {
            client: Client::with_config(config),
            model: model.into(),
            tools: None,
            temperature: None,
            tool_choice: None,
        }
    }

    /// Set tools for this completion (enables tool_calls in response).
    pub fn with_tools(mut self, tools: Vec<ToolSpec>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set temperature (0–2). Lower values are more deterministic.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set tool choice mode (auto, none, required). Overrides API default when tools are present.
    pub fn with_tool_choice(mut self, mode: ToolChoiceMode) -> Self {
        self.tool_choice = Some(mode);
        self
    }

    /// Convert our `Message` list to OpenAI request messages (system/user/assistant text only).
    fn messages_to_request(messages: &[Message]) -> Vec<ChatCompletionRequestMessage> {
        messages
            .iter()
            .map(|m| match m {
                Message::System(s) => ChatCompletionRequestMessage::System(
                    ChatCompletionRequestSystemMessage::from(s.as_str()),
                ),
                Message::User(s) => ChatCompletionRequestMessage::User(
                    ChatCompletionRequestUserMessage::from(s.as_str()),
                ),
                Message::Assistant(s) => {
                    ChatCompletionRequestMessage::Assistant((s.as_str()).into())
                }
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

        if let Some(t) = self.temperature {
            args.temperature(t);
        }

        if let Some(mode) = self.tool_choice {
            let opt = match mode {
                ToolChoiceMode::Auto => ToolChoiceOptions::Auto,
                ToolChoiceMode::None => ToolChoiceOptions::None,
                ToolChoiceMode::Required => ToolChoiceOptions::Required,
            };
            args.tool_choice(ChatCompletionToolChoiceOption::Mode(opt));
        }

        let request = args.build().map_err(|e| {
            AgentError::ExecutionFailed(format!("OpenAI request build failed: {}", e))
        })?;

        let response = self
            .client
            .chat()
            .create(request)
            .await
            .map_err(|e| AgentError::ExecutionFailed(format!("OpenAI API error: {}", e)))?;

        let choice =
            response.choices.into_iter().next().ok_or_else(|| {
                AgentError::ExecutionFailed("OpenAI returned no choices".to_string())
            })?;

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

        Ok(LlmResponse {
            content,
            tool_calls,
        })
    }
}

#[cfg(all(test, feature = "openai"))]
mod tests {
    use super::*;

    /// **Scenario**: ChatOpenAI::new sets model; tools and temperature are None.
    #[test]
    fn chat_openai_new_creates_client() {
        let _ = ChatOpenAI::new("gpt-4");
        let _ = ChatOpenAI::new("gpt-4o-mini");
    }

    /// **Scenario**: ChatOpenAI::with_config uses custom config and model.
    #[test]
    fn chat_openai_with_config_creates_client() {
        let config = OpenAIConfig::new().with_api_key("test-key");
        let _ = ChatOpenAI::with_config(config, "gpt-4");
    }

    /// **Scenario**: Builder chain with_tools and with_temperature builds without panic.
    #[test]
    fn chat_openai_with_tools_and_temperature_builder() {
        let tools = vec![ToolSpec {
            name: "get_time".into(),
            description: None,
            input_schema: serde_json::json!({}),
        }];
        let _ = ChatOpenAI::new("gpt-4")
            .with_tools(tools)
            .with_temperature(0.5f32);
    }
}

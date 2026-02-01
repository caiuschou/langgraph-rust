//! Short-term memory tool source: get_recent_messages from current step context.
//!
//! Uses `ToolCallContext` (injected by ActNode via `set_call_context`) to return
//! the last N messages. See `idea/memory-tools-design.md` §3.3.

use std::sync::RwLock;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::message::Message;
use crate::tool_source::{ToolCallContent, ToolCallContext, ToolSource, ToolSourceError, ToolSpec};

/// Tool name: get recent messages from the current conversation.
pub const TOOL_GET_RECENT_MESSAGES: &str = "get_recent_messages";

fn get_recent_messages_spec() -> ToolSpec {
    ToolSpec {
        name: TOOL_GET_RECENT_MESSAGES.to_string(),
        description: Some(
            "(Optional) Get the last N messages from the current conversation. Use only when you need \
             to explicitly re-read or summarize recent turns (e.g. when prompt does not include full history). \
             Most ReAct flows can omit this tool.".to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "limit": { "type": "integer", "description": "Max number of messages to return (optional)" }
            }
        }),
    }
}

/// Tool source that exposes current-step messages as one tool: get_recent_messages.
///
/// Holds `RwLock<Option<ToolCallContext>>`; ActNode calls `set_call_context` before
/// tool execution so that `call_tool("get_recent_messages", args)` can read
/// `recent_messages` and return the last `limit` as JSON (role + content).
/// See `idea/memory-tools-design.md` §3.3.
pub struct ShortTermMemoryToolSource {
    context: RwLock<Option<ToolCallContext>>,
}

impl ShortTermMemoryToolSource {
    /// Creates a short-term memory tool source.
    pub fn new() -> Self {
        Self {
            context: RwLock::new(None),
        }
    }

    fn message_to_json(m: &Message) -> Value {
        let (role, content) = match m {
            Message::System(s) => ("system", s.as_str()),
            Message::User(s) => ("user", s.as_str()),
            Message::Assistant(s) => ("assistant", s.as_str()),
        };
        json!({ "role": role, "content": content })
    }
}

impl Default for ShortTermMemoryToolSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolSource for ShortTermMemoryToolSource {
    async fn list_tools(&self) -> Result<Vec<ToolSpec>, ToolSourceError> {
        Ok(vec![get_recent_messages_spec()])
    }

    async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolCallContent, ToolSourceError> {
        self.call_tool_with_context(name, arguments, None).await
    }

    async fn call_tool_with_context(
        &self,
        name: &str,
        arguments: Value,
        ctx: Option<&ToolCallContext>,
    ) -> Result<ToolCallContent, ToolSourceError> {
        if name != TOOL_GET_RECENT_MESSAGES {
            return Err(ToolSourceError::NotFound(name.to_string()));
        }
        let limit = arguments
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);
        let messages_vec: Vec<Message> = match ctx {
            Some(c) => c.recent_messages.clone(),
            None => {
                let guard = self
                    .context
                    .read()
                    .map_err(|e| ToolSourceError::Transport(e.to_string()))?;
                guard.as_ref().map(|c| c.recent_messages.clone()).unwrap_or_default()
            }
        };
        let messages = messages_vec.as_slice();
        let take = limit.unwrap_or(messages.len());
        let start = messages.len().saturating_sub(take);
        let slice = &messages[start..];
        let arr: Vec<Value> = slice.iter().map(Self::message_to_json).collect();
        let text = serde_json::to_string(&arr).map_err(|e| ToolSourceError::InvalidInput(e.to_string()))?;
        Ok(ToolCallContent { text })
    }

    fn set_call_context(&self, ctx: Option<ToolCallContext>) {
        if let Ok(mut g) = self.context.write() {
            *g = ctx;
        }
    }
}

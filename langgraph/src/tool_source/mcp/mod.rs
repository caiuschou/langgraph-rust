//! MCP ToolSource: connects to an MCP server via stdio, implements ToolSource.
//!
//! Design: docs/rust-langgraph/mcp-integration/mcp-tool-devplan.md.
//! Uses `McpSession` for transport; maps MCP tools/list and tools/call to
//! `ToolSpec` and `ToolCallContent`.

mod session;

use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::Value;
use tokio::task;

use crate::tool_source::{ToolCallContent, ToolSource, ToolSourceError, ToolSpec};

pub use session::{McpSession, McpSessionError};

/// Tool source backed by an MCP server over stdio.
///
/// Spawns the MCP server process (e.g. mcp-filesystem-server), performs
/// initialize handshake, and implements `ToolSource` via `tools/list` and
/// `tools/call`. Used by ReAct's ActNode and by LLM `with_tools`.
///
/// **Interaction**: Implements `ToolSource`; used by ActNode and by examples
/// that pass tools to ChatOpenAI. Holds `McpSession` behind Mutex (feature `openai`).
/// for interior mutability (ToolSource uses `&self`).
pub struct McpToolSource {
    session: Mutex<McpSession>,
}

impl McpToolSource {
    /// Creates a new McpToolSource by spawning the MCP server and initializing.
    /// Returns `Err` if spawn or initialize fails. Child process inherits only
    /// default env (HOME, PATH, etc.); no extra vars.
    ///
    /// **Interaction**: Caller provides `command` (e.g. `cargo`) and `args`
    /// (e.g. `["run", "-p", "mcp-filesystem-server", "--quiet"]`).
    pub fn new(command: impl Into<String>, args: Vec<String>) -> Result<Self, McpSessionError> {
        let session = McpSession::new(command, args, None::<Vec<(String, String)>>)?;
        Ok(Self {
            session: Mutex::new(session),
        })
    }

    /// Like `new`, but passes the given env vars to the MCP server process.
    /// Use for servers that need credentials (e.g. GITLAB_TOKEN for GitLab MCP).
    ///
    /// **Interaction**: Caller provides `command`, `args`, and env key-value pairs
    /// to be set in the child process environment.
    pub fn new_with_env(
        command: impl Into<String>,
        args: Vec<String>,
        env: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Result<Self, McpSessionError> {
        let session = McpSession::new(command, args, Some(env))?;
        Ok(Self {
            session: Mutex::new(session),
        })
    }

    /// Lists tools by sending `tools/list` and mapping result to `Vec<ToolSpec>`.
    fn list_tools_sync(&self) -> Result<Vec<ToolSpec>, ToolSourceError> {
        let mut session = self
            .session
            .lock()
            .map_err(|e| ToolSourceError::Transport(e.to_string()))?;
        let id = "langgraph-tools-list";
        session
            .send_request(id, "tools/list", Value::Object(serde_json::Map::new()))
            .map_err(|e| ToolSourceError::Transport(e.to_string()))?;

        let result = session
            .wait_for_result(id, std::time::Duration::from_secs(10))
            .map_err(|e| ToolSourceError::Transport(e.to_string()))?
            .ok_or_else(|| ToolSourceError::Transport("timeout waiting for tools/list".into()))?;

        if let Some(err) = result.error {
            return Err(ToolSourceError::JsonRpc(err.message));
        }

        let tools_value = result
            .result
            .and_then(|r| r.get("tools").cloned())
            .ok_or_else(|| ToolSourceError::Transport("no tools in response".into()))?;

        let tools_array = tools_value
            .as_array()
            .ok_or_else(|| ToolSourceError::Transport("tools not an array".into()))?;

        let mut specs = Vec::with_capacity(tools_array.len());
        for t in tools_array {
            let obj = t
                .as_object()
                .ok_or_else(|| ToolSourceError::Transport("tool item not an object".into()))?;
            let name = obj
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let description = obj
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from);
            let input_schema = obj
                .get("inputSchema")
                .cloned()
                .unwrap_or(Value::Object(serde_json::Map::new()));
            specs.push(ToolSpec {
                name,
                description,
                input_schema,
            });
        }
        Ok(specs)
    }

    /// Calls a tool by sending `tools/call` and extracting text from content.
    fn call_tool_sync(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<ToolCallContent, ToolSourceError> {
        let mut session = self
            .session
            .lock()
            .map_err(|e| ToolSourceError::Transport(e.to_string()))?;
        let id = format!("langgraph-call-{}", name);
        let params = serde_json::json!({ "name": name, "arguments": arguments });
        session
            .send_request(&id, "tools/call", params)
            .map_err(|e| ToolSourceError::Transport(e.to_string()))?;

        let result = session
            .wait_for_result(&id, std::time::Duration::from_secs(30))
            .map_err(|e| ToolSourceError::Transport(e.to_string()))?
            .ok_or_else(|| ToolSourceError::Transport("timeout waiting for tools/call".into()))?;

        if let Some(err) = result.error {
            return Err(ToolSourceError::JsonRpc(err.message));
        }

        let result_value = result
            .result
            .ok_or_else(|| ToolSourceError::Transport("no result in tools/call response".into()))?;

        if result_value
            .get("isError")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            let msg = result_value
                .get("content")
                .and_then(|c| c.as_array())
                .and_then(|a| a.first())
                .and_then(|b| b.get("text").and_then(|t| t.as_str()))
                .unwrap_or("tool returned error")
                .to_string();
            return Err(ToolSourceError::Transport(msg));
        }

        let mut text_parts = Vec::new();
        if let Some(content_array) = result_value.get("content").and_then(|c| c.as_array()) {
            for block in content_array {
                if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                        text_parts.push(t);
                    }
                }
            }
        }
        let mut text = text_parts.join("\n").trim().to_string();
        if text.is_empty() {
            if let Some(structured) = result_value.get("structuredContent") {
                text = serde_json::to_string(structured).unwrap_or_default();
            }
        }
        if text.is_empty() {
            return Err(ToolSourceError::Transport(
                "no text or structuredContent in tools/call response".into(),
            ));
        }

        Ok(ToolCallContent { text })
    }
}

#[async_trait]
impl ToolSource for McpToolSource {
    async fn list_tools(&self) -> Result<Vec<ToolSpec>, ToolSourceError> {
        task::block_in_place(|| self.list_tools_sync())
    }

    async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<ToolCallContent, ToolSourceError> {
        task::block_in_place(|| self.call_tool_sync(name, arguments))
    }
}

#[cfg(all(test, feature = "mcp"))]
mod tests {
    use super::*;

    /// **Scenario**: When command does not exist, McpToolSource::new returns an error.
    #[test]
    fn mcp_tool_source_new_invalid_command_returns_error() {
        let result = McpToolSource::new("_nonexistent_command_that_does_not_exist_xyz_", vec![]);
        assert!(result.is_err(), "expected Err for nonexistent command");
    }
}

//! Tool source abstraction: list tools and call a tool.
//!
//! Design: docs/rust-langgraph/mcp-integration/implementation.md.
//! ReAct/Agent depends on `ToolSource` instead of a concrete tool registry;
//! implementations include `MockToolSource` (tests) and `McpToolSource` (feature mcp).

mod mock;

#[cfg(feature = "mcp")]
mod mcp;

pub use mock::MockToolSource;

#[cfg(feature = "mcp")]
pub use mcp::{McpSession, McpSessionError, McpToolSource};

use async_trait::async_trait;
use serde_json::Value;
use thiserror::Error;

/// Tool specification, aligned with MCP `tools/list` result item.
///
/// Used by ReAct/Think to build tool descriptions for the LLM.
///
/// **Interaction**: Returned by `ToolSource::list_tools()`; consumed by ThinkNode
/// to build prompts (future). See docs/rust-langgraph/mcp-integration/implementation.md §1.1.
#[derive(Debug, Clone)]
pub struct ToolSpec {
    /// Tool name (e.g. used in MCP tools/call).
    pub name: String,
    /// Human-readable description for the LLM.
    pub description: Option<String>,
    /// JSON Schema for arguments (MCP inputSchema).
    pub input_schema: Value,
}

/// Result of a single tool call; aligns with MCP `tools/call` content.
///
/// **Interaction**: Returned by `ToolSource::call_tool()`; ActNode maps this to
/// `ToolResult` and writes into `ReActState::tool_results`. See docs/rust-langgraph/mcp-integration/implementation.md §1.1.
#[derive(Debug, Clone)]
pub struct ToolCallContent {
    /// Result text (e.g. from MCP result.content[].text).
    pub text: String,
}

/// Errors from listing or calling tools (ToolSource or MCP).
///
/// **Interaction**: Returned by `ToolSource::list_tools()` and `call_tool()`;
/// nodes may map to `AgentError` when running the graph.
#[derive(Debug, Error)]
pub enum ToolSourceError {
    #[error("tool not found: {0}")]
    NotFound(String),
    #[error("invalid arguments: {0}")]
    InvalidInput(String),
    #[error("MCP/transport error: {0}")]
    Transport(String),
    #[error("JSON-RPC error: {0}")]
    JsonRpc(String),
}

/// Tool source: list tools and call a tool.
///
/// ReAct/Agent depends on this instead of a concrete ToolRegistry. Think node
/// uses `list_tools()` to build prompts; Act node uses `call_tool(name, args)`.
/// Implementations: `MockToolSource` (tests), future `McpToolSource`.
///
/// **Interaction**: Used by ThinkNode (list_tools) and ActNode (call_tool).
/// See docs/rust-langgraph/mcp-integration/README.md §3.1 and docs/rust-langgraph/mcp-integration/implementation.md §1.1.
#[async_trait]
pub trait ToolSource: Send + Sync {
    /// List available tools (e.g. MCP tools/list).
    async fn list_tools(&self) -> Result<Vec<ToolSpec>, ToolSourceError>;

    /// Call a tool by name with JSON arguments (e.g. MCP tools/call).
    async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<ToolCallContent, ToolSourceError>;
}

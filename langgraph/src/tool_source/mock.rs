//! Mock ToolSource for tests and examples.
//!
//! Design: docs/rust-langgraph/13-react-agent-design.md §8.2 stage 2.3.
//! Returns fixed tool list and fixed call results; no MCP Server required.

use async_trait::async_trait;
use serde_json::{json, Value};

use super::{ToolCallContent, ToolSource, ToolSourceError, ToolSpec};

/// Mock tool source: fixed tool list and fixed call result.
///
/// `list_tools()` returns a configurable list; `call_tool(name, _)` returns
/// a configurable text (same for all tools by default). Used by ActNode tests
/// and ReAct linear-chain example.
///
/// **Interaction**: Implements `ToolSource`; used by ActNode in tests and examples.
pub struct MockToolSource {
    /// Tools returned by list_tools().
    tools: Vec<ToolSpec>,
    /// Text returned by call_tool for any name (or use per-name map later).
    call_result: String,
}

impl MockToolSource {
    /// Creates a mock that lists one tool `get_time` and returns fixed time string on call.
    ///
    /// Aligns with docs/rust-langgraph/13-react-agent-design.md §8.2: fixed tool list and call_tool returns fixed text.
    pub fn get_time_example() -> Self {
        Self {
            tools: vec![ToolSpec {
                name: "get_time".to_string(),
                description: Some("Get current time.".to_string()),
                input_schema: json!({ "type": "object", "properties": {} }),
            }],
            call_result: "2025-01-29 12:00:00".to_string(),
        }
    }

    /// Creates a mock with custom tool list and fixed call result.
    pub fn new(tools: Vec<ToolSpec>, call_result: String) -> Self {
        Self { tools, call_result }
    }

    /// Set the text returned by call_tool (builder style).
    pub fn with_call_result(mut self, text: String) -> Self {
        self.call_result = text;
        self
    }
}

impl Default for MockToolSource {
    fn default() -> Self {
        Self::get_time_example()
    }
}

#[async_trait]
impl ToolSource for MockToolSource {
    async fn list_tools(&self) -> Result<Vec<ToolSpec>, ToolSourceError> {
        Ok(self.tools.clone())
    }

    async fn call_tool(&self, _name: &str, _arguments: Value) -> Result<ToolCallContent, ToolSourceError> {
        Ok(ToolCallContent {
            text: self.call_result.clone(),
        })
    }
}

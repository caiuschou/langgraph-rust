//! Act node: read tool_calls, call ToolSource for each, write tool_results.
//!
//! Design: docs/rust-langgraph/13-react-agent-design.md §8.3 stage 3.3–3.4.
//! ActNode holds a ToolSource (e.g. `Box<dyn ToolSource>`), implements `Node<ReActState>`;
//! run reads state.tool_calls, calls call_tool(name, args) for each, writes state.tool_results.
//! On single-call failure the whole step fails (short-circuit).

use async_trait::async_trait;
use serde_json::Value;

use crate::error::AgentError;
use crate::graph::Next;
use crate::Node;
use crate::state::{ReActState, ToolResult};
use crate::tool_source::{ToolCallContext, ToolSource};

/// Act node: one ReAct step that executes tool_calls and produces tool_results.
///
/// Reads `state.tool_calls`, calls `ToolSource::call_tool(name, arguments)` for each
/// (parsing arguments from JSON string); appends one ToolResult per call. When
/// tool_calls is empty, leaves tool_results empty. Single call failure returns
/// Err and short-circuits the graph.
///
/// **Interaction**: Implements `Node<ReActState>`; used by StateGraph. Consumes
/// `ToolSource` (e.g. MockToolSource); reads ReActState.tool_calls, writes
/// ReActState.tool_results. See docs/rust-langgraph/mcp-integration/README.md.
pub struct ActNode {
    /// Tool source used to execute each tool call.
    tools: Box<dyn ToolSource>,
}

impl ActNode {
    /// Creates an Act node with the given tool source.
    pub fn new(tools: Box<dyn ToolSource>) -> Self {
        Self { tools }
    }
}

#[async_trait]
impl Node<ReActState> for ActNode {
    fn id(&self) -> &str {
        "act"
    }

    /// Reads state.tool_calls, calls call_tool_with_context for each, writes tool_results.
    /// Passes ToolCallContext (recent_messages) explicitly so tools like get_recent_messages
    /// receive current conversation without internal state. Also calls set_call_context for
    /// backward compatibility. Returns Next::Continue.
    async fn run(&self, state: ReActState) -> Result<(ReActState, Next), AgentError> {
        let ctx = ToolCallContext {
            recent_messages: state.messages.clone(),
        };
        self.tools.set_call_context(Some(ctx.clone()));
        let mut tool_results = Vec::with_capacity(state.tool_calls.len());
        for tc in &state.tool_calls {
            let args: Value = if tc.arguments.trim().is_empty() {
                serde_json::json!({})
            } else {
                serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!({}))
            };
            let content = self
                .tools
                .call_tool_with_context(&tc.name, args, Some(&ctx))
                .await
                .map_err(|e| AgentError::ExecutionFailed(e.to_string()))?;
            tool_results.push(ToolResult {
                call_id: tc.id.clone(),
                name: Some(tc.name.clone()),
                content: content.text,
            });
        }
        self.tools.set_call_context(None);
        let new_state = ReActState {
            messages: state.messages,
            tool_calls: state.tool_calls,
            tool_results,
        };
        Ok((new_state, Next::Continue))
    }
}

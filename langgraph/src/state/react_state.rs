//! ReAct state and tool types for the minimal ReAct agent.
//!
//! Design: docs/rust-langgraph/13-react-agent-design.md.
//! ReActState holds messages plus per-round tool_calls and tool_results; Think/Act/Observe
//! nodes read and write these fields. ToolCall and ToolResult align with MCP `tools/call`
//! and result content.

use crate::message::Message;

/// A single tool invocation produced by the LLM (Think node) and consumed by Act.
///
/// Aligns with MCP `tools/call`: `name` and `arguments` (JSON string or object).
/// Optional `id` can be used to correlate with `ToolResult::call_id` in Observe.
///
/// **Interaction**: Written by ThinkNode from LLM output; read by ActNode to call
/// `ToolSource::call_tool(name, arguments)`.
#[derive(Debug, Clone, Default)]
pub struct ToolCall {
    /// Tool name as registered in ToolSource (e.g. MCP tools/list).
    pub name: String,
    /// Arguments as JSON string; parse in Act when calling the tool.
    pub arguments: String,
    /// Optional id to match with ToolResult; useful when merging results in Observe.
    pub id: Option<String>,
}

/// Result of executing one tool call (Act node output, Observe node input).
///
/// Aligns with MCP result `content[].text`. Use `call_id` or `name` to associate
/// with the corresponding `ToolCall` when merging into state in Observe.
///
/// **Interaction**: Written by ActNode from `ToolSource::call_tool` result; read by
/// ObserveNode to append to messages or internal state and then clear.
#[derive(Debug, Clone, Default)]
pub struct ToolResult {
    /// Id of the tool call this result belongs to (if ToolCall had `id`).
    pub call_id: Option<String>,
    /// Tool name; alternative to call_id for matching.
    pub name: Option<String>,
    /// Result content (e.g. text from MCP result.content[].text).
    pub content: String,
}

/// State for the minimal ReAct graph: Think → Act → Observe.
///
/// Extends conversation history (`messages`) with per-round tool data: LLM outputs
/// `tool_calls`, Act fills `tool_results`, Observe merges results and clears both.
/// Satisfies `Clone + Send + Sync + 'static` for use with `Node<ReActState>` and
/// `StateGraph<ReActState>`.
///
/// **Interaction**: Consumed and produced by ThinkNode, ActNode, ObserveNode; passed
/// through `StateGraph::invoke`. See docs/rust-langgraph/13-react-agent-design.md §2.1.
#[derive(Debug, Clone, Default)]
pub struct ReActState {
    /// Conversation history (System, User, Assistant). Used by Think and extended by Observe.
    pub messages: Vec<Message>,
    /// Current round tool calls from the LLM (Think writes, Act reads).
    pub tool_calls: Vec<ToolCall>,
    /// Current round tool execution results (Act writes, Observe reads and merges).
    pub tool_results: Vec<ToolResult>,
}

// ReActState, ToolCall, ToolResult: fields are standard types (String, Vec<Message>, Option<String>, etc.),
// so they satisfy Clone + Send + Sync + 'static required by Node<S> and StateGraph<S>.

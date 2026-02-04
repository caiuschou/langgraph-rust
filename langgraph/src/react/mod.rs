//! ReAct graph nodes: Think, Act, Observe, and routing utilities.
//!
//! Design: docs/rust-langgraph/13-react-agent-design.md §8.3 stage 3.
//! Three nodes implementing `Node<ReActState>` for the minimal ReAct chain
//! think → act → observe (linear, then conditional edge in stage 5).
//!
//! # Routing
//!
//! Use [`tools_condition`] for conditional routing based on tool calls:
//!
//! ```rust,ignore
//! graph.add_conditional_edges(
//!     "think",
//!     tools_condition,
//!     [("tools", "act"), ("__end__", "__end__")].into(),
//! );
//! ```

mod act_node;
mod observe_node;
mod think_node;

pub use act_node::{
    ActNode, ErrorHandlerFn, HandleToolErrors, DEFAULT_EXECUTION_ERROR_TEMPLATE,
    DEFAULT_TOOL_ERROR_TEMPLATE,
};
pub use observe_node::ObserveNode;
pub use think_node::ThinkNode;

use crate::state::ReActState;

/// Output of the tools_condition function.
///
/// - `Tools` - Route to the tools/act node
/// - `End` - Route to the end node
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolsConditionResult {
    /// Route to the tools execution node ("tools" or "act").
    Tools,
    /// Route to the end node ("__end__").
    End,
}

impl ToolsConditionResult {
    /// Returns the node ID string for this routing result.
    ///
    /// - `Tools` -> `"tools"`
    /// - `End` -> `"__end__"`
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Tools => "tools",
            Self::End => "__end__",
        }
    }
}

/// Conditional routing function for ReAct-style tool-calling workflows.
///
/// This utility function implements the standard conditional logic for ReAct-style
/// agents: if the state contains tool calls, route to the tool execution node;
/// otherwise, end the workflow.
///
/// # Arguments
///
/// * `state` - The current ReActState to examine for tool calls
///
/// # Returns
///
/// * `ToolsConditionResult::Tools` - If `state.tool_calls` is not empty
/// * `ToolsConditionResult::End` - If `state.tool_calls` is empty
///
/// # Example
///
/// ```rust,ignore
/// use langgraph::react::tools_condition;
/// use langgraph::graph::StateGraph;
///
/// let mut graph = StateGraph::new();
/// graph.add_node("think", think_node);
/// graph.add_node("act", act_node);
///
/// // Route based on whether there are tool calls
/// graph.add_conditional_edges(
///     "think",
///     |state| tools_condition(state).as_str().to_string(),
///     [("tools", "act"), ("__end__", "__end__")].into(),
/// );
/// ```
///
/// # Notes
///
/// - This function only examines `state.tool_calls`, not the messages
/// - Tool calls are typically populated by `ThinkNode` when the LLM decides to call tools
/// - If your state structure differs, you may need to implement a custom condition function
pub fn tools_condition(state: &ReActState) -> ToolsConditionResult {
    if state.tool_calls.is_empty() {
        ToolsConditionResult::End
    } else {
        ToolsConditionResult::Tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ToolCall;
    use crate::Message;

    /// **Scenario**: tools_condition returns End when no tool calls.
    #[test]
    fn tools_condition_returns_end_when_no_tool_calls() {
        let state = ReActState {
            messages: vec![Message::User("hello".into())],
            tool_calls: vec![],
            tool_results: vec![],
            turn_count: 0,
        };

        let result = tools_condition(&state);
        assert_eq!(result, ToolsConditionResult::End);
        assert_eq!(result.as_str(), "__end__");
    }

    /// **Scenario**: tools_condition returns Tools when tool calls present.
    #[test]
    fn tools_condition_returns_tools_when_tool_calls_present() {
        let state = ReActState {
            messages: vec![Message::User("search".into())],
            tool_calls: vec![ToolCall {
                id: Some("tc1".into()),
                name: "search".into(),
                arguments: "{}".into(),
            }],
            tool_results: vec![],
            turn_count: 0,
        };

        let result = tools_condition(&state);
        assert_eq!(result, ToolsConditionResult::Tools);
        assert_eq!(result.as_str(), "tools");
    }

    /// **Scenario**: ToolsConditionResult as_str returns correct values.
    #[test]
    fn tools_condition_result_as_str() {
        assert_eq!(ToolsConditionResult::Tools.as_str(), "tools");
        assert_eq!(ToolsConditionResult::End.as_str(), "__end__");
    }
}

/// Default system prompt for ReAct agents.
///
/// Follows the Thought → Action → Observation pattern. Prepend as the first
/// message in `ReActState::messages` when building state so the LLM reasons
/// before acting and analyzes tool results. Callers can use a custom system
/// message instead; ThinkNode does not inject this automatically.
///
/// See docs/rust-langgraph/17-react-prompt-practices.md for prompt design
/// and alternatives (e.g. domain-specific or thought/action/observation splits).
pub const REACT_SYSTEM_PROMPT: &str = r#"You are an agent that follows the ReAct pattern (Reasoning + Acting).

RULES:
1. THOUGHT first: Before any action, reason "Do I need external information?"
   - If the question can be answered with your knowledge (math, general knowledge, reasoning) → give FINAL_ANSWER directly. Do NOT call tools.
   - Only call tools when the user explicitly needs data you cannot know: current time, weather, search, etc.
2. Use ACTION: call tools only when truly needed, or give FINAL_ANSWER when you have enough.
3. After each tool result (OBSERVATION), reason about what you learned and decide the next step.
4. Be thorough but concise in your reasoning.
5. When using tool data, cite or summarize it clearly in your final answer.

PHASES:
- THOUGHT: Reason about what the user needs, what you already have, and whether any tool would help.
- ACTION: Execute one tool at a time, or give FINAL_ANSWER with your complete response.
- OBSERVATION: After seeing tool output, analyze it and either call another tool or answer.

Explain your reasoning clearly. Use tools only when they can help; for simple questions, answer directly. Do not make up facts; use tool results when available."#;

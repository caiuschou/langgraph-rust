//! ReAct graph nodes: Think, Act, Observe.
//!
//! Design: docs/rust-langgraph/13-react-agent-design.md §8.3 stage 3.
//! Three nodes implementing `Node<ReActState>` for the minimal ReAct chain
//! think → act → observe (linear, then conditional edge in stage 5).

mod act_node;
mod observe_node;
mod think_node;

pub use act_node::ActNode;
pub use observe_node::ObserveNode;
pub use think_node::ThinkNode;

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

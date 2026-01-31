//! Graph node trait: one step in a StateGraph.
//!
//! Receives state `S`, returns updated `S` and `Next` (continue, jump, or end).
//! Used by `StateGraph` and `CompiledStateGraph`. Aligns with LangGraph node
//! `(state) -> partial`. Agents can implement `Node<S>` when `Agent::State == S`.
//! Conditional edges: see `Next` and docs/rust-langgraph/13-react-agent-design.md §8.5.

use async_trait::async_trait;

use crate::error::AgentError;

use super::Next;

/// One step in a graph: state in, (state out, next step).
///
/// Used by `StateGraph` to run a single step. The graph runner uses `Next` to
/// choose the next node (Continue = linear order, Node(id) = jump, End = stop).
/// Aligns with LangGraph node `(state) -> partial`; returns full `S` and routing.
///
/// **Interaction**: Implemented by graph nodes and by agents via blanket impl
/// when `Agent::State == S`. See `StateGraph::add_node` and `CompiledStateGraph::invoke`.
#[async_trait]
pub trait Node<S>: Send + Sync
where
    S: Clone + Send + Sync + 'static,
{
    /// Node id (e.g. `"chat"`, `"tool"`). Must be unique within a graph.
    fn id(&self) -> &str;

    /// One step: state in, (state out, next step).
    ///
    /// Return `Next::Continue` to follow the linear edge order; `Next::Node(id)` to
    /// jump to a node; `Next::End` to stop. The runner uses this for conditional edges.
    async fn run(&self, state: S) -> Result<(S, Next), AgentError>;
}

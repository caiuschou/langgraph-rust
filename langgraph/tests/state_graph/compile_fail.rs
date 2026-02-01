//! StateGraph compile failure cases: unknown node, invalid chain, etc.

use std::sync::Arc;

use langgraph::{CompilationError, StateGraph, START};

use crate::common::{AgentState, EchoAgent};

#[tokio::test]
async fn compile_fails_when_edge_refers_to_unknown_node() {
    let mut graph = StateGraph::<AgentState>::new();
    graph.add_node("echo", Arc::new(EchoAgent::new()));
    graph.add_edge(START, "echo");
    graph.add_edge("echo", "missing");

    match graph.compile() {
        Err(CompilationError::NodeNotFound(id)) => assert_eq!(id, "missing"),
        _ => panic!("expected NodeNotFound"),
    }
}

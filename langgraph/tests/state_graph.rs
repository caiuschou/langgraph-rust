//! Integration tests for StateGraph: compile validation, invoke, and with_store (P5.2).

use std::sync::Arc;

use async_trait::async_trait;
use langgraph::{
    Agent, AgentError, CompilationError, InMemoryStore, Message, StateGraph, Store,
};

#[derive(Debug, Clone, Default)]
struct AgentState {
    pub messages: Vec<Message>,
}

struct EchoAgent;

impl EchoAgent {
    fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Agent for EchoAgent {
    fn name(&self) -> &str {
        "echo"
    }
    type State = AgentState;
    async fn run(&self, state: Self::State) -> Result<Self::State, AgentError> {
        let mut messages = state.messages;
        if let Some(Message::User(s)) = messages.last() {
            messages.push(Message::Assistant(s.clone()));
        }
        Ok(AgentState { messages })
    }
}

#[tokio::test]
async fn compile_fails_when_edge_refers_to_unknown_node() {
    let mut graph = StateGraph::<AgentState>::new();
    graph.add_node("echo", Box::new(EchoAgent::new()));
    graph.add_edge("echo");
    graph.add_edge("missing");

    match graph.compile() {
        Err(CompilationError::NodeNotFound(id)) => assert_eq!(id, "missing"),
        _ => panic!("expected NodeNotFound"),
    }
}

#[tokio::test]
async fn invoke_single_node_chain() {
    let mut graph = StateGraph::<AgentState>::new();
    graph
        .add_node("echo", Box::new(EchoAgent::new()))
        .add_edge("echo");

    let compiled = graph.compile().unwrap();
    let mut state = AgentState::default();
    state.messages.push(Message::User("hi".into()));

    let state = compiled.invoke(state, None).await.unwrap();
    let last = state.messages.last().unwrap();
    assert!(matches!(last, Message::Assistant(s) if s == "hi"));
}

/// Compiled graph without `with_store` has no store (P5.2: do not break existing usage).
#[tokio::test]
async fn compile_without_store_has_no_store() {
    let mut graph = StateGraph::<AgentState>::new();
    graph
        .add_node("echo", Box::new(EchoAgent::new()))
        .add_edge("echo");

    let compiled = graph.compile().unwrap();
    assert!(compiled.store().is_none());
}

/// Compiled graph with `with_store(store)` holds the store; `store()` returns Some (P5.2).
#[tokio::test]
async fn compile_with_store_holds_store() {
    let store: Arc<dyn Store> = Arc::new(InMemoryStore::new());
    let mut graph = StateGraph::<AgentState>::new();
    graph
        .add_node("echo", Box::new(EchoAgent::new()))
        .add_edge("echo");

    let compiled = graph.with_store(store).compile().unwrap();
    assert!(compiled.store().is_some());
    // Same store reference
    let graph_store = compiled.store().unwrap().clone();
    let ns = vec!["u1".to_string(), "memories".to_string()];
    graph_store
        .put(&ns, "k1", &serde_json::json!("v1"))
        .await
        .unwrap();
    let v = graph_store.get(&ns, "k1").await.unwrap();
    assert_eq!(v.as_ref().and_then(|x| x.as_str()), Some("v1"));
}

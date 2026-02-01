//! Unit tests for [`LoggingMiddleware`](crate::middleware::LoggingMiddleware) and
//! [`WithNodeLogging`](crate::middleware::WithNodeLogging).
//!
//! Scenarios: around_run calls inner and returns the same result; with_node_logging attaches middleware.

use std::pin::Pin;
use std::sync::Arc;

use langgraph::{AgentError, Message, NameNode, Next, NodeMiddleware, ReActState, StateGraph, END, START};

use crate::middleware::{LoggingMiddleware, WithNodeLogging};

/// **Scenario**: LoggingMiddleware::around_run calls inner with the given state and returns inner's result.
#[tokio::test]
async fn logging_middleware_around_run_calls_inner_and_returns_result() {
    let m = LoggingMiddleware;
    let state = ReActState {
        messages: vec![Message::user("hi")],
        ..Default::default()
    };
    let next = Next::Continue;
    let inner_called = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let inner_called_clone = inner_called.clone();
    let inner = Box::new(move |s: ReActState| {
        inner_called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        Box::pin(async move { Ok((s, next.clone())) }) as Pin<Box<dyn std::future::Future<Output = Result<(ReActState, Next), AgentError>> + Send>>
    });
    let result = m.around_run("test_node", state.clone(), inner).await;
    assert!(inner_called.load(std::sync::atomic::Ordering::SeqCst));
    match &result {
        Ok((s, n)) => {
            assert_eq!(s.messages.len(), 1);
            assert!(matches!(n, Next::Continue));
        }
        Err(_) => panic!("expected Ok"),
    }
}

/// **Scenario**: When inner returns Err, around_run propagates the error.
#[tokio::test]
async fn logging_middleware_around_run_propagates_error() {
    let m = LoggingMiddleware;
    let state = ReActState::default();
    let inner = Box::new(|_s: ReActState| {
        Box::pin(async {
            Err(AgentError::ExecutionFailed("fail".into()))
        }) as Pin<Box<dyn std::future::Future<Output = Result<(ReActState, Next), AgentError>> + Send>>
    });
    let result = m.around_run("test_node", state, inner).await;
    match &result {
        Err(AgentError::ExecutionFailed(msg)) => assert_eq!(msg, "fail"),
        _ => panic!("expected ExecutionFailed"),
    }
}

/// **Scenario**: Multiple around_run calls (enter/exit per node) complete without panic and return inner result.
#[tokio::test]
async fn logging_middleware_around_run_enter_exit_twice() {
    let m = LoggingMiddleware;
    let state0 = ReActState {
        messages: vec![Message::user("a")],
        ..Default::default()
    };
    let inner1 = Box::new(|s: ReActState| {
        Box::pin(async move { Ok((s, Next::Continue)) })
            as Pin<Box<dyn std::future::Future<Output = Result<(ReActState, Next), AgentError>> + Send>>
    });
    let r1: (ReActState, Next) = m.around_run("node1", state0.clone(), inner1).await.unwrap();
    let inner2 = Box::new(|s: ReActState| {
        Box::pin(async move { Ok((s, Next::End)) })
            as Pin<Box<dyn std::future::Future<Output = Result<(ReActState, Next), AgentError>> + Send>>
    });
    let r2: (ReActState, Next) = m.around_run("node2", r1.0.clone(), inner2).await.unwrap();
    assert_eq!(r2.0.messages.len(), 1);
    assert!(matches!(r2.1, Next::End));
}

/// **Scenario**: StateGraph::new().with_node_logging().compile() produces a graph that runs through
/// LoggingMiddleware on invoke (invoke succeeds with expected state).
#[tokio::test]
async fn with_node_logging_compile_invoke_succeeds() {
    let mut graph = StateGraph::<ReActState>::new();
    graph
        .add_node("n", Arc::new(NameNode::new("n")))
        .add_edge(START, "n")
        .add_edge("n", END);
    let compiled = graph.with_node_logging().compile().expect("compile");
    let state = ReActState {
        messages: vec![Message::user("hi")],
        ..Default::default()
    };
    let result = compiled.invoke(state, None).await;
    let final_state = result.expect("invoke should succeed");
    assert_eq!(final_state.messages.len(), 1);
    assert!(matches!(final_state.messages.first(), Some(Message::User(s)) if s == "hi"));
}

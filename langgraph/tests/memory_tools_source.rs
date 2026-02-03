//! Unit tests for MemoryToolsSource (composite long-term + short-term).
//!
//! Verifies list_tools returns 5 tools; call_tool dispatches to store/short-term;
//! set_call_context is forwarded so get_recent_messages sees context.

use langgraph::memory::{InMemoryStore, Store};
use langgraph::message::Message;
use langgraph::tool_source::{
    MemoryToolsSource, ToolCallContext, ToolSource, TOOL_GET_RECENT_MESSAGES, TOOL_LIST_MEMORIES,
    TOOL_RECALL, TOOL_REMEMBER,
};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn memory_tools_source_list_tools_returns_five_tools() {
    let store: Arc<dyn Store> = Arc::new(InMemoryStore::new());
    let ns = vec!["memories".to_string()];
    let source = MemoryToolsSource::new(store, ns).await;
    let tools = source.list_tools().await.unwrap();
    assert_eq!(tools.len(), 5);
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&TOOL_REMEMBER));
    assert!(names.contains(&TOOL_RECALL));
    assert!(names.contains(&TOOL_LIST_MEMORIES));
    assert!(names.contains(&TOOL_GET_RECENT_MESSAGES));
}

#[tokio::test]
async fn memory_tools_source_call_tool_dispatches_to_store() {
    let store: Arc<dyn Store> = Arc::new(InMemoryStore::new());
    let ns = vec!["memories".to_string()];
    let source = MemoryToolsSource::new(store, ns).await;

    let r = source
        .call_tool(TOOL_REMEMBER, json!({ "key": "k", "value": "v" }))
        .await
        .unwrap();
    assert_eq!(r.text, "ok");

    let r = source
        .call_tool(TOOL_RECALL, json!({ "key": "k" }))
        .await
        .unwrap();
    assert_eq!(r.text, "\"v\"");
}

#[tokio::test]
async fn memory_tools_source_set_call_context_forwarded_get_recent_messages() {
    let store: Arc<dyn Store> = Arc::new(InMemoryStore::new());
    let ns = vec!["memories".to_string()];
    let source = MemoryToolsSource::new(store, ns).await;

    source.set_call_context(Some(ToolCallContext::new(
        vec![Message::user("hi"), Message::assistant("hello")],
    )));

    let r = source
        .call_tool(TOOL_GET_RECENT_MESSAGES, json!({}))
        .await
        .unwrap();
    let arr: Vec<serde_json::Value> = serde_json::from_str(&r.text).unwrap();
    assert_eq!(arr.len(), 2);
    assert_eq!(arr[0].get("content").and_then(|v| v.as_str()), Some("hi"));
    assert_eq!(
        arr[1].get("content").and_then(|v| v.as_str()),
        Some("hello")
    );
}

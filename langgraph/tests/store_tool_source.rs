//! Unit tests for StoreToolSource.
//!
//! Verifies list_tools returns 4 tools; remember → recall consistent; recall missing key
//! returns not found; list_memories / search_memories behavior. See docs/rust-langgraph/tools-refactor §6.

use langgraph::memory::{InMemoryStore, Store};
use langgraph::tool_source::{
    StoreToolSource, ToolSource, TOOL_LIST_MEMORIES, TOOL_RECALL, TOOL_REMEMBER,
    TOOL_SEARCH_MEMORIES,
};
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn store_tool_source_list_tools_returns_four_tools() {
    let store: Arc<dyn Store> = Arc::new(InMemoryStore::new());
    let ns = vec!["memories".to_string()];
    let source = StoreToolSource::new(store, ns).await;
    let tools = source.list_tools().await.unwrap();
    assert_eq!(tools.len(), 4);
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(names.contains(&TOOL_REMEMBER));
    assert!(names.contains(&TOOL_RECALL));
    assert!(names.contains(&TOOL_SEARCH_MEMORIES));
    assert!(names.contains(&TOOL_LIST_MEMORIES));
}

#[tokio::test]
async fn store_tool_source_remember_recall_consistent() {
    let store: Arc<dyn Store> = Arc::new(InMemoryStore::new());
    let ns = vec!["memories".to_string()];
    let source = StoreToolSource::new(store, ns).await;

    let r = source
        .call_tool(
            TOOL_REMEMBER,
            json!({ "key": "pref", "value": "dark mode" }),
        )
        .await
        .unwrap();
    assert_eq!(r.text, "ok");

    let r = source
        .call_tool(TOOL_RECALL, json!({ "key": "pref" }))
        .await
        .unwrap();
    assert_eq!(r.text, "\"dark mode\"");
}

#[tokio::test]
async fn store_tool_source_recall_missing_key_returns_not_found() {
    let store: Arc<dyn Store> = Arc::new(InMemoryStore::new());
    let ns = vec!["memories".to_string()];
    let source = StoreToolSource::new(store, ns).await;

    let err = source
        .call_tool(TOOL_RECALL, json!({ "key": "nonexistent" }))
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("not found") || msg.contains("NotFound"));
}

#[tokio::test]
async fn store_tool_source_list_memories_returns_keys() {
    let store: Arc<dyn Store> = Arc::new(InMemoryStore::new());
    let ns = vec!["memories".to_string()];
    let source = StoreToolSource::new(store, ns).await;

    source
        .call_tool(TOOL_REMEMBER, json!({ "key": "a", "value": 1 }))
        .await
        .unwrap();
    source
        .call_tool(TOOL_REMEMBER, json!({ "key": "b", "value": 2 }))
        .await
        .unwrap();

    let r = source
        .call_tool(TOOL_LIST_MEMORIES, json!({}))
        .await
        .unwrap();
    let keys: Vec<String> = serde_json::from_str(&r.text).unwrap();
    assert!(keys.contains(&"a".to_string()));
    assert!(keys.contains(&"b".to_string()));
}

#[tokio::test]
async fn store_tool_source_search_memories_returns_hits() {
    let store: Arc<dyn Store> = Arc::new(InMemoryStore::new());
    let ns = vec!["memories".to_string()];
    let source = StoreToolSource::new(store, ns).await;

    source
        .call_tool(TOOL_REMEMBER, json!({ "key": "apple", "value": "fruit" }))
        .await
        .unwrap();
    source
        .call_tool(TOOL_REMEMBER, json!({ "key": "car", "value": "vehicle" }))
        .await
        .unwrap();

    let r = source
        .call_tool(
            TOOL_SEARCH_MEMORIES,
            json!({ "query": "fruit", "limit": 5 }),
        )
        .await
        .unwrap();
    let hits: Vec<serde_json::Value> = serde_json::from_str(&r.text).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].get("key").and_then(|v| v.as_str()), Some("apple"));
}

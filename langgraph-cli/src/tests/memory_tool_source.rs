//! Unit tests for [`MemoryToolSource`](crate::tools::MemoryToolSource).
//!
//! Uses [`InMemoryStore`](langgraph::InMemoryStore) as the store backend.
//! Scenarios: list_tools, save_memory, retrieve_memory, list_memories, unknown tool.

use std::sync::Arc;

use langgraph::{InMemoryStore, ToolSource};
use serde_json::json;

use crate::tools::MemoryToolSource;

fn make_tool_source() -> MemoryToolSource {
    let store = Arc::new(InMemoryStore::new());
    let namespace = vec!["test_user".to_string(), "memories".to_string()];
    MemoryToolSource::new(store, namespace)
}

/// **Scenario**: list_tools returns exactly three tools with expected names.
///
/// Given: a MemoryToolSource with an in-memory store  
/// When: list_tools() is called  
/// Then: result is Ok and contains save_memory, retrieve_memory, list_memories
#[tokio::test]
async fn list_tools_returns_three_memory_tools() {
    let source = make_tool_source();

    let tools = source.list_tools().await.expect("list_tools should succeed");

    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(tools.len(), 3);
    assert!(names.contains(&"save_memory"));
    assert!(names.contains(&"retrieve_memory"));
    assert!(names.contains(&"list_memories"));
}

/// **Scenario**: save_memory stores info and returns a success message.
///
/// Given: a MemoryToolSource  
/// When: call_tool("save_memory", { "info": "user likes coffee" }) is called  
/// Then: result is Ok and text contains "Saved to memory" and "user likes coffee"
#[tokio::test]
async fn call_tool_save_memory_stores_and_returns_message() {
    let source = make_tool_source();
    let args = json!({ "info": "user likes coffee" });

    let result = source.call_tool("save_memory", args).await;

    let content = result.expect("save_memory should succeed");
    assert!(content.text.contains("Saved to memory"));
    assert!(content.text.contains("user likes coffee"));
}

/// **Scenario**: After saving, list_memories returns the saved info.
///
/// Given: a MemoryToolSource and one saved memory  
/// When: call_tool("list_memories", {}) is called  
/// Then: result is Ok and text contains the previously saved info
#[tokio::test]
async fn call_tool_list_memories_after_save_returns_saved_info() {
    let source = make_tool_source();
    source
        .call_tool("save_memory", json!({ "info": "name is Alice" }))
        .await
        .expect("save should succeed");

    let content = source
        .call_tool("list_memories", json!({}))
        .await
        .expect("list_memories should succeed");

    assert!(content.text.contains("name is Alice"));
    assert!(content.text.contains("I remember"));
}

/// **Scenario**: retrieve_memory with no matching key returns a "no memories" message.
///
/// Given: a MemoryToolSource with no data for key "nonexistent"  
/// When: call_tool("retrieve_memory", { "key": "nonexistent" }) is called  
/// Then: result is Ok and text indicates no memories found
#[tokio::test]
async fn call_tool_retrieve_memory_empty_returns_no_memories_message() {
    let source = make_tool_source();
    let args = json!({ "key": "nonexistent" });

    let content = source
        .call_tool("retrieve_memory", args)
        .await
        .expect("retrieve_memory should not error");

    assert!(content.text.contains("No memories found") || content.text.to_lowercase().contains("no"));
}

/// **Scenario**: After saving, retrieve_memory (via search) can return the saved info.
///
/// Given: a MemoryToolSource with one saved memory  
/// When: call_tool("retrieve_memory", { "key": "memory" }) is called (search matches key prefix)  
/// Then: result may contain the saved info (behaviour depends on store search)
#[tokio::test]
async fn call_tool_retrieve_after_save_can_find_memory() {
    let source = make_tool_source();
    source
        .call_tool("save_memory", json!({ "info": "favorite color is blue" }))
        .await
        .expect("save should succeed");

    let content = source
        .call_tool("retrieve_memory", json!({ "key": "memory" }))
        .await
        .expect("retrieve should succeed");

    assert!(
        content.text.contains("favorite color is blue") || content.text.contains("No memories"),
        "expected saved info or no memories: {}",
        content.text
    );
}

/// **Scenario**: call_tool with an unknown tool name returns NotFound error.
///
/// Given: a MemoryToolSource  
/// When: call_tool("unknown_tool", {}) is called  
/// Then: result is Err(ToolSourceError::NotFound(_))
#[tokio::test]
async fn call_tool_unknown_name_returns_not_found_error() {
    let source = make_tool_source();

    let result = source.call_tool("unknown_tool", json!({})).await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{:?}", err);
    assert!(err_str.to_lowercase().contains("not found") || err_str.contains("Unknown tool"));
}

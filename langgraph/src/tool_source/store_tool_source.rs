//! Store-backed tool source: long-term memory as tools (remember, recall, search_memories, list_memories).
//!
//! Wraps `Store` with a fixed namespace and exposes put/get/list/search as tools for the LLM.
//! See `idea/memory-tools-design.md` §2.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::memory::{Namespace, Store, StoreError, StoreSearchHit};
use crate::tool_source::{ToolCallContent, ToolSource, ToolSourceError, ToolSpec};

/// Tool name: write a key-value pair to long-term memory.
pub const TOOL_REMEMBER: &str = "remember";
/// Tool name: read a value by key from long-term memory.
pub const TOOL_RECALL: &str = "recall";
/// Tool name: search memories by query (and optional limit).
pub const TOOL_SEARCH_MEMORIES: &str = "search_memories";
/// Tool name: list all keys in the current namespace.
pub const TOOL_LIST_MEMORIES: &str = "list_memories";

fn remember_spec() -> ToolSpec {
    ToolSpec {
        name: TOOL_REMEMBER.to_string(),
        description: Some(
            "Write a key-value pair to long-term memory. Call when: the user expresses a preference, \
             the user explicitly asks to remember something, or existing memory should be updated.".to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "Memory key" },
                "value": { "description": "Value (any JSON)" }
            },
            "required": ["key", "value"]
        }),
    }
}

fn recall_spec() -> ToolSpec {
    ToolSpec {
        name: TOOL_RECALL.to_string(),
        description: Some(
            "Read a value by key from long-term memory. Call when you need to retrieve something \
             previously stored with remember.".to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "key": { "type": "string", "description": "Memory key" }
            },
            "required": ["key"]
        }),
    }
}

fn search_memories_spec() -> ToolSpec {
    ToolSpec {
        name: TOOL_SEARCH_MEMORIES.to_string(),
        description: Some(
            "Search long-term memories by query (optional) and limit (optional). Call when you need \
             to find relevant past information before answering or acting.".to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query (optional)" },
                "limit": { "type": "integer", "description": "Max results (optional)" }
            }
        }),
    }
}

fn list_memories_spec() -> ToolSpec {
    ToolSpec {
        name: TOOL_LIST_MEMORIES.to_string(),
        description: Some(
            "List all memory keys in the current namespace. Call when you need to see what \
             has been stored before recalling or searching.".to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
    }
}

fn map_store_error(e: StoreError) -> ToolSourceError {
    match e {
        StoreError::NotFound => ToolSourceError::NotFound("key not found".to_string()),
        StoreError::Serialization(s) => ToolSourceError::InvalidInput(s),
        StoreError::Storage(s) => ToolSourceError::Transport(s),
        StoreError::EmbeddingError(s) => ToolSourceError::Transport(s),
    }
}

/// Tool source that exposes Store operations as tools (remember, recall, search_memories, list_memories).
///
/// Holds `Arc<dyn Store>` and a fixed namespace (e.g. `[user_id, "memories"]`). Use with ActNode
/// or composite ToolSource for long-term memory. See `idea/memory-tools-design.md` §2.
pub struct StoreToolSource {
    store: Arc<dyn Store>,
    namespace: Namespace,
}

impl StoreToolSource {
    /// Creates a store tool source with the given store and namespace.
    pub fn new(store: Arc<dyn Store>, namespace: Namespace) -> Self {
        Self { store, namespace }
    }

    async fn do_remember(&self, key: &str, value: Value) -> Result<ToolCallContent, ToolSourceError> {
        self.store
            .put(&self.namespace, key, &value)
            .await
            .map_err(map_store_error)?;
        Ok(ToolCallContent {
            text: "ok".to_string(),
        })
    }

    async fn do_recall(&self, key: &str) -> Result<ToolCallContent, ToolSourceError> {
        let opt = self
            .store
            .get(&self.namespace, key)
            .await
            .map_err(map_store_error)?;
        let text = match opt {
            Some(v) => v.to_string(),
            None => return Err(ToolSourceError::NotFound("key not found".to_string())),
        };
        Ok(ToolCallContent { text })
    }

    async fn do_search_memories(
        &self,
        query: Option<&str>,
        limit: Option<usize>,
    ) -> Result<ToolCallContent, ToolSourceError> {
        let hits = self
            .store
            .search(&self.namespace, query, limit)
            .await
            .map_err(map_store_error)?;
        let arr: Vec<serde_json::Value> = hits
            .into_iter()
            .map(|h: StoreSearchHit| {
                json!({
                    "key": h.key,
                    "value": h.value,
                    "score": h.score
                })
            })
            .collect();
        Ok(ToolCallContent {
            text: serde_json::to_string(&arr).map_err(|e| ToolSourceError::InvalidInput(e.to_string()))?,
        })
    }

    async fn do_list_memories(&self) -> Result<ToolCallContent, ToolSourceError> {
        let keys = self
            .store
            .list(&self.namespace)
            .await
            .map_err(map_store_error)?;
        Ok(ToolCallContent {
            text: serde_json::to_string(&keys).map_err(|e| ToolSourceError::InvalidInput(e.to_string()))?,
        })
    }
}

#[async_trait]
impl ToolSource for StoreToolSource {
    async fn list_tools(&self) -> Result<Vec<ToolSpec>, ToolSourceError> {
        Ok(vec![
            remember_spec(),
            recall_spec(),
            search_memories_spec(),
            list_memories_spec(),
        ])
    }

    async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolCallContent, ToolSourceError> {
        match name {
            TOOL_REMEMBER => {
                let key = arguments
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolSourceError::InvalidInput("missing key".to_string()))?;
                let value = arguments
                    .get("value")
                    .cloned()
                    .unwrap_or(Value::Null);
                self.do_remember(key, value).await
            }
            TOOL_RECALL => {
                let key = arguments
                    .get("key")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ToolSourceError::InvalidInput("missing key".to_string()))?;
                self.do_recall(key).await
            }
            TOOL_SEARCH_MEMORIES => {
                let query = arguments.get("query").and_then(|v| v.as_str()).map(String::from);
                let limit = arguments.get("limit").and_then(|v| v.as_u64()).map(|n| n as usize);
                self.do_search_memories(query.as_deref(), limit).await
            }
            TOOL_LIST_MEMORIES => self.do_list_memories().await,
            _ => Err(ToolSourceError::NotFound(name.to_string())),
        }
    }
}

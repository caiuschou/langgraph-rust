//! Memory tool source for long-term memory (save, retrieve, list).
//!
//! Implements [`ToolSource`](langgraph::ToolSource); used by [`run_with_config`](crate::run) when
//! long-term memory is enabled. Interacts with [`Store`](langgraph::Store) and [`Namespace`](langgraph::Namespace).
//!
//! Tools are defined in [`memory_tools::specs`]; each tool is handled by a dedicated handler
//! (e.g. [`MemoryToolSource::handle_save_memory`]) and dispatched from [`ToolSource::call_tool`].

use std::sync::Arc;

use langgraph::{Namespace, Store, ToolCallContent, ToolSource, ToolSourceError, ToolSpec};
use serde_json::Value;

mod memory_tools {
    use serde_json::{json, Value};

    use langgraph::ToolSpec;

    /// Tool names exposed by the memory tool source.
    pub(super) const SAVE_MEMORY: &str = "save_memory";
    pub(super) const RETRIEVE_MEMORY: &str = "retrieve_memory";
    pub(super) const LIST_MEMORIES: &str = "list_memories";

    /// JSON keys for stored memory value shape (used by handlers and serialization).
    pub(super) const MEMORY_KEY_INFO: &str = "info";
    pub(super) const MEMORY_KEY_TIMESTAMP: &str = "timestamp";

    /// Returns the list of tool specs for long-term memory (save, retrieve, list).
    /// Used by [`MemoryToolSource::list_tools`](super::MemoryToolSource::list_tools).
    pub(super) fn specs() -> Vec<ToolSpec> {
        vec![
            ToolSpec {
                name: SAVE_MEMORY.to_string(),
                description: Some(
                    "Save information to long-term memory. Use when user says 'remember' or shares preferences."
                        .to_string(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "info": {
                            "type": "string",
                            "description": "Information to remember (e.g. 'name is Alice', 'likes coffee')"
                        }
                    },
                    "required": ["info"]
                }),
            },
            ToolSpec {
                name: RETRIEVE_MEMORY.to_string(),
                description: Some(
                    "Retrieve specific memory by key. Use for questions like 'what's my name'."
                        .to_string(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "key": {
                            "type": "string",
                            "description": "Key to retrieve (e.g. 'name', 'preferences')"
                        }
                    },
                    "required": ["key"]
                }),
            },
            ToolSpec {
                name: LIST_MEMORIES.to_string(),
                description: Some(
                    "List all stored memories for the user. Use for 'what do you know about me'."
                        .to_string(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                }),
            },
        ]
    }

    /// Builds the JSON value stored for a memory entry. Caller provides `info`; timestamp is set to now.
    pub(super) fn memory_value(info: &str) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert(MEMORY_KEY_INFO.to_string(), Value::String(info.to_string()));
        obj.insert(
            MEMORY_KEY_TIMESTAMP.to_string(),
            Value::String(chrono::Utc::now().to_rfc3339()),
        );
        Value::Object(obj)
    }

    /// Reads the "info" string from a stored memory value (from search/get).
    pub(super) fn info_from_value(value: &Value) -> Option<String> {
        value[MEMORY_KEY_INFO].as_str().map(String::from)
    }
}

/// Memory tool source for long-term memory (save, retrieve, list).
///
/// Holds a [`Store`](langgraph::Store) and [`Namespace`](langgraph::Namespace); implements
/// [`ToolSource`](langgraph::ToolSource) by delegating to internal handlers per tool name.
pub struct MemoryToolSource {
    store: Arc<dyn Store>,
    namespace: Namespace,
}

impl MemoryToolSource {
    /// Creates a new memory tool source with the given store and namespace.
    pub fn new(store: Arc<dyn Store>, namespace: Namespace) -> Self {
        Self { store, namespace }
    }

    /// Maps a store/transport error into [`ToolSourceError::Transport`].
    fn map_store_error(e: impl std::fmt::Display) -> ToolSourceError {
        ToolSourceError::Transport(e.to_string())
    }

    /// Handles `save_memory`: writes one memory entry and returns a success message.
    async fn handle_save_memory(
        &self,
        arguments: &Value,
    ) -> Result<ToolCallContent, ToolSourceError> {
        let info = arguments["info"].as_str().unwrap_or("").to_string();
        let key = format!(
            "memory_{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
        );
        let value = memory_tools::memory_value(&info);
        self.store
            .put(&self.namespace, &key, &value)
            .await
            .map_err(Self::map_store_error)?;
        Ok(ToolCallContent {
            text: format!("Saved to memory: {}", info),
        })
    }

    /// Handles `retrieve_memory`: searches store by key and returns found memories or a "no memories" message.
    async fn handle_retrieve_memory(
        &self,
        arguments: &Value,
    ) -> Result<ToolCallContent, ToolSourceError> {
        let key = arguments["key"].as_str().unwrap_or("");
        let hits = self
            .store
            .search(&self.namespace, Some(key), Some(5))
            .await
            .map_err(Self::map_store_error)?;
        if hits.is_empty() {
            return Ok(ToolCallContent {
                text: format!("No memories found for '{}'", key),
            });
        }
        let memories: Vec<String> = hits
            .iter()
            .filter_map(|h| memory_tools::info_from_value(&h.value))
            .collect();
        Ok(ToolCallContent {
            text: format!("Found memories: {}", memories.join(", ")),
        })
    }

    /// Handles `list_memories`: lists all keys, loads each value, and returns concatenated "info" or empty message.
    async fn handle_list_memories(&self) -> Result<ToolCallContent, ToolSourceError> {
        let keys = self
            .store
            .list(&self.namespace)
            .await
            .map_err(Self::map_store_error)?;
        let mut memories = Vec::new();
        for key in keys {
            if let Some(value) = self
                .store
                .get(&self.namespace, &key)
                .await
                .map_err(Self::map_store_error)?
            {
                if let Some(info) = memory_tools::info_from_value(&value) {
                    memories.push(info);
                }
            }
        }
        let text = if memories.is_empty() {
            "No memories stored yet. Tell me something to remember!".to_string()
        } else {
            format!("I remember: {}", memories.join("; "))
        };
        Ok(ToolCallContent { text })
    }
}

#[async_trait::async_trait]
impl ToolSource for MemoryToolSource {
    async fn list_tools(&self) -> Result<Vec<ToolSpec>, ToolSourceError> {
        Ok(memory_tools::specs())
    }

    async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<ToolCallContent, ToolSourceError> {
        match name {
            memory_tools::SAVE_MEMORY => self.handle_save_memory(&arguments).await,
            memory_tools::RETRIEVE_MEMORY => self.handle_retrieve_memory(&arguments).await,
            memory_tools::LIST_MEMORIES => self.handle_list_memories().await,
            _ => Err(ToolSourceError::NotFound(format!("Unknown tool: {}", name))),
        }
    }
}

//! Memory tool source for long-term memory (save, retrieve, list).
//!
//! Implements [`ToolSource`](langgraph::ToolSource); used by [`run_with_config`](crate::run) when
//! long-term memory is enabled. Interacts with [`Store`](langgraph::Store) and [`Namespace`](langgraph::Namespace).

use std::sync::Arc;

use langgraph::{
    Namespace, Store, ToolCallContent, ToolSource, ToolSourceError, ToolSpec,
};
use serde_json::{json, Value};

/// Memory tool source for long-term memory (save, retrieve, list).
pub struct MemoryToolSource {
    store: Arc<dyn Store>,
    namespace: Namespace,
}

impl MemoryToolSource {
    /// Creates a new memory tool source with the given store and namespace.
    pub fn new(store: Arc<dyn Store>, namespace: Namespace) -> Self {
        Self { store, namespace }
    }
}

#[async_trait::async_trait]
impl ToolSource for MemoryToolSource {
    async fn list_tools(&self) -> Result<Vec<ToolSpec>, ToolSourceError> {
        Ok(vec![
            ToolSpec {
                name: "save_memory".to_string(),
                description: Some(
                    "Save information to long-term memory. Use when user says 'remember' or shares preferences."
                        .to_string(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "info": {
                            "type": "string",
                            "description": "Information to remember (e.g., 'name is Alice', 'likes coffee')"
                        }
                    },
                    "required": ["info"]
                }),
            },
            ToolSpec {
                name: "retrieve_memory".to_string(),
                description: Some("Retrieve specific memory by key. Use for questions like 'what's my name'.".to_string()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "key": {
                            "type": "string",
                            "description": "Key to retrieve (e.g., 'name', 'preferences')"
                        }
                    },
                    "required": ["key"]
                }),
            },
            ToolSpec {
                name: "list_memories".to_string(),
                description: Some(
                    "List all stored memories for the user. Use for 'what do you know about me'."
                        .to_string(),
                ),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                }),
            },
        ])
    }

    async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<ToolCallContent, ToolSourceError> {
        match name {
            "save_memory" => {
                let info = arguments["info"].as_str().unwrap_or("").to_string();
                let timestamp = chrono::Utc::now().to_rfc3339();
                let key = format!(
                    "memory_{}",
                    chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
                );
                let value = json!({
                    "info": info,
                    "timestamp": timestamp
                });
                self.store
                    .put(&self.namespace, &key, &value)
                    .await
                    .map_err(|e| ToolSourceError::Transport(e.to_string()))?;
                Ok(ToolCallContent {
                    text: format!("Saved to memory: {}", info),
                })
            }
            "retrieve_memory" => {
                let key = arguments["key"].as_str().unwrap_or("");
                let hits = self
                    .store
                    .search(&self.namespace, Some(key), Some(5))
                    .await
                    .map_err(|e| ToolSourceError::Transport(e.to_string()))?;
                if hits.is_empty() {
                    Ok(ToolCallContent {
                        text: format!("No memories found for '{}'", key),
                    })
                } else {
                    let memories: Vec<String> = hits
                        .iter()
                        .map(|h| h.value["info"].as_str().unwrap_or("").to_string())
                        .collect();
                    Ok(ToolCallContent {
                        text: format!("Found memories: {}", memories.join(", ")),
                    })
                }
            }
            "list_memories" => {
                let keys = self
                    .store
                    .list(&self.namespace)
                    .await
                    .map_err(|e| ToolSourceError::Transport(e.to_string()))?;
                let mut memories = Vec::new();
                for key in keys {
                    if let Some(value) = self
                        .store
                        .get(&self.namespace, &key)
                        .await
                        .map_err(|e| ToolSourceError::Transport(e.to_string()))?
                    {
                        if let Some(info) = value["info"].as_str() {
                            memories.push(info.to_string());
                        }
                    }
                }
                if memories.is_empty() {
                    Ok(ToolCallContent {
                        text: "No memories stored yet. Tell me something to remember!".to_string(),
                    })
                } else {
                    Ok(ToolCallContent {
                        text: format!("I remember: {}", memories.join("; ")),
                    })
                }
            }
            _ => Err(ToolSourceError::NotFound(format!("Unknown tool: {}", name))),
        }
    }
}

//! In-memory Store. Aligns with LangGraph InMemoryStore. Not persistent.
//!
//! Semantic search uses in-memory vector store (see 16-memory-design §5.2.1). This implementation does key/list and optional query filter only.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::memory::store::{Namespace, Store, StoreError, StoreSearchHit};

/// Key for the inner map: namespace joined by "\0", then key. Enables list by namespace prefix.
fn map_key(namespace: &Namespace, key: &str) -> String {
    let ns = namespace.join("\0");
    format!("{}\0{}", ns, key)
}

/// In-memory Store. Aligns with LangGraph InMemoryStore. Not persistent.
///
/// **Interaction**: Used as `Arc<dyn Store>` when graph is compiled with store; nodes use it for cross-thread memory.
pub struct InMemoryStore {
    inner: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

impl InMemoryStore {
    /// Creates a new in-memory store.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn namespace_prefix(namespace: &Namespace) -> String {
        format!("{}\0", namespace.join("\0"))
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Store for InMemoryStore {
    async fn put(
        &self,
        namespace: &Namespace,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<(), StoreError> {
        let k = map_key(namespace, key);
        self.inner.write().await.insert(k, value.clone());
        Ok(())
    }

    async fn get(
        &self,
        namespace: &Namespace,
        key: &str,
    ) -> Result<Option<serde_json::Value>, StoreError> {
        let k = map_key(namespace, key);
        Ok(self.inner.read().await.get(&k).cloned())
    }

    async fn list(&self, namespace: &Namespace) -> Result<Vec<String>, StoreError> {
        let prefix = Self::namespace_prefix(namespace);
        let guard = self.inner.read().await;
        let mut keys: Vec<String> = guard
            .keys()
            .filter(|k| k.starts_with(&prefix))
            .map(|k| {
                k.strip_prefix(&prefix).unwrap_or(k).to_string()
            })
            .collect();
        keys.sort();
        keys.dedup();
        Ok(keys)
    }

    async fn search(
        &self,
        namespace: &Namespace,
        query: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<StoreSearchHit>, StoreError> {
        let prefix = Self::namespace_prefix(namespace);
        let guard = self.inner.read().await;
        let mut hits: Vec<StoreSearchHit> = guard
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .map(|(k, v)| {
                let key = k.strip_prefix(&prefix).unwrap_or(k).to_string();
                StoreSearchHit {
                    key,
                    value: v.clone(),
                    score: None,
                }
            })
            .collect();
        if let Some(q) = query {
            if !q.is_empty() {
                hits.retain(|h| {
                    h.key.contains(q)
                        || h.value.to_string().to_lowercase().contains(&q.to_lowercase())
                });
            }
        }
        if let Some(n) = limit {
            hits.truncate(n);
        }
        Ok(hits)
    }
}

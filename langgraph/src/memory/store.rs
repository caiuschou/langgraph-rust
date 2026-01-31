//! Store trait and StoreError for cross-thread memory.
//!
//! Aligns with LangGraph BaseStore (namespace, put, get, list, search). See docs/rust-langgraph/16-memory-design.md §5.1.

use async_trait::async_trait;

/// Namespace for Store: e.g. (user_id, "memories") or (user_id, "preferences"). Aligns with LangGraph namespace tuple.
pub type Namespace = Vec<String>;

/// Error for store operations.
///
/// Callers do not depend on underlying backend errors (e.g. rusqlite, lancedb).
/// Use `?` with `serde_json::Error` via the `From` impl for serialization failures.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// JSON or namespace serialization/deserialization failed.
    #[error("serialization: {0}")]
    Serialization(String),

    /// Backend storage error (e.g. DB I/O). Message is opaque to avoid leaking backend types.
    #[error("storage: {0}")]
    Storage(String),

    /// Key not found in the given namespace (optional; get may use `Ok(None)` instead).
    #[error("not found")]
    NotFound,
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Serialization(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_error_from_serde_json_error() {
        let invalid = "not valid json {{{";
        let err: StoreError = serde_json::from_str::<serde_json::Value>(invalid).unwrap_err().into();
        match &err {
            StoreError::Serialization(s) => assert!(s.contains("expected value") || s.len() > 0),
            _ => panic!("expected Serialization variant"),
        }
    }
}

/// A single hit returned by [`Store::search`].
///
/// For key-value or string-filter search (e.g. [`crate::memory::InMemoryStore`], [`crate::memory::SqliteStore`]),
/// `score` is `None`. For semantic/vector search (e.g. LanceStore), `score` is the similarity (e.g. cosine or L2).
#[derive(Debug, Clone)]
pub struct StoreSearchHit {
    /// The key of the matched entry within the namespace.
    pub key: String,
    /// The stored value (JSON).
    pub value: serde_json::Value,
    /// Similarity score when using vector search; `None` for string-filter-only stores.
    pub score: Option<f64>,
}

/// Long-term cross-session store: namespace-isolated key-value with optional search.
///
/// Used for user preferences, long-term memories, and retrievable facts. Not tied to a single
/// thread; use [`Namespace`] (e.g. `[user_id, "memories"]`) for multi-tenant isolation. Differs
/// from [`crate::memory::Checkpointer`], which is per-thread checkpoint state.
///
/// - **Namespace**: Keys are unique per `(namespace, key)`. Same key in different namespaces
///   are separate entries.
/// - **Put**: Overwrites any existing value for that `(namespace, key)`.
/// - **Get**: Returns `Ok(None)` when the key does not exist in the namespace.
/// - **Search**: When `query` is `None` or empty, implementations may degenerate to list-like
///   behavior (return entries up to `limit`). When `query` is set, behavior is
///   implementation-defined (string filter or semantic similarity).
#[async_trait]
pub trait Store: Send + Sync {
    /// Stores `value` under `namespace` and `key`. Replaces any existing value for that key.
    async fn put(
        &self,
        namespace: &Namespace,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<(), StoreError>;

    /// Returns the value for `(namespace, key)`, or `None` if not found.
    async fn get(
        &self,
        namespace: &Namespace,
        key: &str,
    ) -> Result<Option<serde_json::Value>, StoreError>;

    /// Returns all keys in the given namespace (order is implementation-defined).
    async fn list(&self, namespace: &Namespace) -> Result<Vec<String>, StoreError>;

    /// Searches within the namespace. If `query` is `None` or empty, may return entries
    /// up to `limit` (list-like). If `query` is set, filters by string match or semantic
    /// similarity; `limit` caps the number of results.
    async fn search(
        &self,
        namespace: &Namespace,
        query: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<StoreSearchHit>, StoreError>;
}

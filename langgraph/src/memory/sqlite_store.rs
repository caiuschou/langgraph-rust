//! SQLite-backed Store (SqliteStore). Persistent across process restarts.
//!
//! Aligns with 16-memory-design §5.2.2. put/get/list; search is key/value filter (no semantic index).

use std::path::Path;

use async_trait::async_trait;
use rusqlite::params;

use crate::memory::store::{Namespace, Store, StoreError, StoreSearchHit};

fn ns_to_key(ns: &Namespace) -> String {
    serde_json::to_string(ns).unwrap_or_else(|_| "[]".to_string())
}

/// SQLite-backed Store. Key: (namespace, key). Value stored as JSON text.
///
/// Persistent; for single-node and dev. Uses spawn_blocking for async.
///
/// **Interaction**: Used as `Arc<dyn Store>` when graph is compiled with store; nodes use it for cross-thread memory.
pub struct SqliteStore {
    db_path: std::path::PathBuf,
}

impl SqliteStore {
    /// Creates a new SQLite store and ensures the table exists.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let db_path = path.as_ref().to_path_buf();
        let conn = rusqlite::Connection::open(&db_path)
            .map_err(|e| StoreError::Storage(e.to_string()))?;
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS store_kv (
                ns TEXT NOT NULL,
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                PRIMARY KEY (ns, key)
            )
            "#,
            [],
        )
        .map_err(|e| StoreError::Storage(e.to_string()))?;
        Ok(Self { db_path })
    }
}

#[async_trait]
impl Store for SqliteStore {
    async fn put(
        &self,
        namespace: &Namespace,
        key: &str,
        value: &serde_json::Value,
    ) -> Result<(), StoreError> {
        let ns = ns_to_key(namespace);
        let key = key.to_string();
        let value_str = serde_json::to_string(value)?;
        let db_path = self.db_path.clone();

        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)
                .map_err(|e| StoreError::Storage(e.to_string()))?;
            conn.execute(
                "INSERT OR REPLACE INTO store_kv (ns, key, value) VALUES (?1, ?2, ?3)",
                params![ns, key, value_str],
            )
            .map_err(|e| StoreError::Storage(e.to_string()))?;
            Ok::<(), StoreError>(())
        })
        .await
        .map_err(|e| StoreError::Storage(e.to_string()))?
    }

    async fn get(
        &self,
        namespace: &Namespace,
        key: &str,
    ) -> Result<Option<serde_json::Value>, StoreError> {
        let ns = ns_to_key(namespace);
        let key = key.to_string();
        let db_path = self.db_path.clone();

        let value_str_opt = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)
                .map_err(|e| StoreError::Storage(e.to_string()))?;
            let mut stmt = conn
                .prepare("SELECT value FROM store_kv WHERE ns = ?1 AND key = ?2")
                .map_err(|e| StoreError::Storage(e.to_string()))?;
            let mut rows = stmt
                .query(params![ns, key])
                .map_err(|e| StoreError::Storage(e.to_string()))?;
            let row = match rows.next().map_err(|e| StoreError::Storage(e.to_string()))? {
                Some(r) => r,
                None => return Ok::<_, StoreError>(None),
            };
            let value_str: String = row.get(0).map_err(|e| StoreError::Storage(e.to_string()))?;
            Ok(Some(value_str))
        })
        .await
        .map_err(|e| StoreError::Storage(e.to_string()))??;

        let value_str = match value_str_opt {
            Some(s) => s,
            None => return Ok(None),
        };
        let value = serde_json::from_str(&value_str)?;
        Ok(Some(value))
    }

    async fn list(&self, namespace: &Namespace) -> Result<Vec<String>, StoreError> {
        let ns = ns_to_key(namespace);
        let db_path = self.db_path.clone();

        let keys = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)
                .map_err(|e| StoreError::Storage(e.to_string()))?;
            let mut stmt = conn
                .prepare("SELECT key FROM store_kv WHERE ns = ?1 ORDER BY key")
                .map_err(|e| StoreError::Storage(e.to_string()))?;
            let rows = stmt
                .query_map(params![ns], |row| row.get(0))
                .map_err(|e| StoreError::Storage(e.to_string()))?;
            let keys: Vec<String> = rows
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| StoreError::Storage(e.to_string()))?;
            Ok::<Vec<String>, StoreError>(keys)
        })
        .await
        .map_err(|e| StoreError::Storage(e.to_string()))??;

        Ok(keys)
    }

    async fn search(
        &self,
        namespace: &Namespace,
        query: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<StoreSearchHit>, StoreError> {
        let ns = ns_to_key(namespace);
        let query = query.map(String::from);
        let db_path = self.db_path.clone();

        let mut hits = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)
                .map_err(|e| StoreError::Storage(e.to_string()))?;
            let mut stmt = conn
                .prepare("SELECT key, value FROM store_kv WHERE ns = ?1")
                .map_err(|e| StoreError::Storage(e.to_string()))?;
            let rows = stmt
                .query_map(params![ns], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                    ))
                })
                .map_err(|e| StoreError::Storage(e.to_string()))?;
            let mut hits: Vec<StoreSearchHit> = Vec::new();
            for row in rows {
                let (key, value_str) = row.map_err(|e| StoreError::Storage(e.to_string()))?;
                let value =
                    serde_json::from_str(&value_str)?;
                hits.push(StoreSearchHit {
                    key,
                    value,
                    score: None,
                });
            }
            Ok::<Vec<StoreSearchHit>, StoreError>(hits)
        })
        .await
        .map_err(|e| StoreError::Storage(e.to_string()))??;

        if let Some(q) = &query {
            if !q.is_empty() {
                let q_lower = q.to_lowercase();
                hits.retain(|h| {
                    h.key.to_lowercase().contains(&q_lower)
                        || h.value.to_string().to_lowercase().contains(&q_lower)
                });
            }
        }
        if let Some(n) = limit {
            hits.truncate(n);
        }
        Ok(hits)
    }
}

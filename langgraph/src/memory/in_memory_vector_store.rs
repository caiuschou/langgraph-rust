use std::sync::Arc;
use async_trait::async_trait;
use dashmap::DashMap;
use serde_json::Value as JsonValue;

use crate::memory::store::{Namespace, Store, StoreError, StoreSearchHit};
use crate::memory::embedder::Embedder;

/// Pure in-memory vector store for semantic search.
///
/// **Interaction**: Used as `Arc<dyn Store>`; nodes use it for cross-thread
/// memory with semantic search.
///
/// **In-Memory**: All data stored in memory, lost when store is dropped.
pub struct InMemoryVectorStore {
    data: DashMap<String, VectorEntry>,
    embedder: Arc<dyn Embedder>,
}

/// Entry in the vector store.
#[derive(Clone)]
struct VectorEntry {
    vector: Vec<f32>,
    value: JsonValue,
}

impl InMemoryVectorStore {
    /// Creates a new in-memory vector store.
    ///
    /// # Arguments
    ///
    /// * `embedder` - Embedder for vector generation
    ///
    /// # Example
    ///
    /// ```ignore
    /// let embedder = Arc::new(OpenAIEmbedder::new("text-embedding-3-small"));
    /// let store = InMemoryVectorStore::new(embedder);
    /// ```
    pub fn new(embedder: Arc<dyn Embedder>) -> Self {
        Self {
            data: DashMap::new(),
            embedder,
        }
    }

    /// Extracts embeddable text from a JSON value.
    fn text_from_value(value: &JsonValue) -> String {
        value.get("text")
            .and_then(|v| v.as_str())
            .map(String::from)
            .unwrap_or_else(|| value.to_string())
    }

    /// Computes cosine similarity between two vectors.
    ///
    /// Returns 0.0 if either vector has zero magnitude.
    fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot_product / (norm_a * norm_b)
        }
    }

    /// Creates a compound key from namespace and key.
    fn make_key(namespace: &Namespace, key: &str) -> String {
        format!("{}:{}", serde_json::to_string(namespace).unwrap_or_default(), key)
    }
}

#[async_trait]
impl Store for InMemoryVectorStore {
    async fn put(
        &self,
        namespace: &Namespace,
        key: &str,
        value: &JsonValue,
    ) -> Result<(), StoreError> {
        let text = Self::text_from_value(value);

        let vectors = self.embedder.embed(&[&text])?;
        let vector = vectors
            .into_iter()
            .next()
            .ok_or_else(|| StoreError::EmbeddingError("No vector returned".into()))?;

        let compound_key = Self::make_key(namespace, key);
        let entry = VectorEntry {
            vector,
            value: value.clone(),
        };

        self.data.insert(compound_key, entry);

        Ok(())
    }

    async fn get(
        &self,
        namespace: &Namespace,
        key: &str,
    ) -> Result<Option<JsonValue>, StoreError> {
        let compound_key = Self::make_key(namespace, key);

        Ok(self.data.get(&compound_key).map(|entry| entry.value.clone()))
    }

    async fn list(&self, namespace: &Namespace) -> Result<Vec<String>, StoreError> {
        let ns_str = serde_json::to_string(namespace).unwrap_or_default();
        let ns_prefix = format!("{}:", ns_str);

        let mut keys = Vec::new();
        for entry in self.data.iter() {
            if entry.key().starts_with(&ns_prefix) {
                let key = entry.key().strip_prefix(&ns_prefix).unwrap_or("");
                keys.push(key.to_string());
            }
        }

        Ok(keys)
    }

    async fn search(
        &self,
        namespace: &Namespace,
        query: Option<&str>,
        limit: Option<usize>,
    ) -> Result<Vec<StoreSearchHit>, StoreError> {
        let limit = limit.unwrap_or(10).min(1000);
        let ns_str = serde_json::to_string(namespace).unwrap_or_default();

        if let Some(q) = query {
            if !q.is_empty() {
                let vectors = self.embedder.embed(&[q])?;
                let query_vec = vectors
                    .into_iter()
                    .next()
                    .ok_or_else(|| StoreError::EmbeddingError("No vector returned".into()))?;

                let mut scores: Vec<(String, f32)> = Vec::new();
                let ns_prefix = format!("{}:", ns_str);

                for entry in self.data.iter() {
                    if entry.key().starts_with(&ns_prefix) {
                        let score = Self::cosine_similarity(&query_vec, &entry.vector);
                        scores.push((entry.key().clone(), score));
                    }
                }

                scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

                let hits: Vec<StoreSearchHit> = scores
                    .into_iter()
                    .take(limit)
                    .map(|(key, score)| StoreSearchHit {
                        key: key.strip_prefix(&ns_prefix).unwrap_or(&key).to_string(),
                        value: self
                            .data
                            .get(&key)
                            .map(|e| e.value.clone())
                            .unwrap_or(JsonValue::Null),
                        score: Some(score as f64),
                    })
                    .collect();

                return Ok(hits);
            }
        }

        let ns_prefix = format!("{}:", ns_str);
        let keys: Vec<String> = self
            .data
            .iter()
            .filter(|e| e.key().starts_with(&ns_prefix))
            .map(|e| e.key().clone())
            .take(limit)
            .collect();

        let hits: Vec<StoreSearchHit> = keys
            .into_iter()
            .map(|key| StoreSearchHit {
                key: key.strip_prefix(&ns_prefix).unwrap_or(&key).to_string(),
                value: self
                    .data
                    .get(&key)
                    .map(|e| e.value.clone())
                    .unwrap_or(JsonValue::Null),
                score: None,
            })
            .collect();

        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::embedder::Embedder;

    struct MockEmbedder {
        dimension: usize,
    }

    impl MockEmbedder {
        fn new(dimension: usize) -> Self {
            Self { dimension }
        }
    }

    impl Embedder for MockEmbedder {
        fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, StoreError> {
            Ok(texts
                .iter()
                .map(|t| {
                    let mut v = vec![0f32; self.dimension];
                    for (i, b) in t.bytes().enumerate() {
                        v[i % self.dimension] += b as f32 / 256.0;
                    }
                    v
                })
                .collect())
        }

        fn dimension(&self) -> usize {
            self.dimension
        }
    }

    /// **Scenario**: Store can put and search entries with semantic similarity.
    #[tokio::test]
    async fn test_put_search() {
        let embedder = Arc::new(MockEmbedder::new(1536));
        let store = InMemoryVectorStore::new(embedder);

        let ns = vec!["test".into()];
        store
            .put(
                &ns,
                "key1",
                &serde_json::json!({"text": "hello world"}),
            )
            .await
            .unwrap();
        store
            .put(
                &ns,
                "key2",
                &serde_json::json!({"text": "rust programming"}),
            )
            .await
            .unwrap();

        let hits = store.search(&ns, Some("rust"), Some(10)).await.unwrap();

        assert!(!hits.is_empty());
        assert!(hits.iter().any(|h| h.key == "key2"));
        for hit in &hits {
            assert!(hit.score.is_some());
        }
    }

    /// **Scenario**: Store can get values by key.
    #[tokio::test]
    async fn test_get() {
        let embedder = Arc::new(MockEmbedder::new(1536));
        let store = InMemoryVectorStore::new(embedder);

        let ns = vec!["test".into()];
        store
            .put(
                &ns,
                "key1",
                &serde_json::json!({"text": "hello", "data": 123}),
            )
            .await
            .unwrap();

        let value = store.get(&ns, "key1").await.unwrap();
        assert_eq!(value, Some(serde_json::json!({"text": "hello", "data": 123})));

        let not_found = store.get(&ns, "non_existent").await.unwrap();
        assert_eq!(not_found, None);
    }

    /// **Scenario**: Store can list all keys in a namespace.
    #[tokio::test]
    async fn test_list() {
        let embedder = Arc::new(MockEmbedder::new(1536));
        let store = InMemoryVectorStore::new(embedder);

        let ns = vec!["test".into()];
        store
            .put(&ns, "key1", &serde_json::json!("v1"))
            .await
            .unwrap();
        store
            .put(&ns, "key2", &serde_json::json!("v2"))
            .await
            .unwrap();

        let keys = store.list(&ns).await.unwrap();
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
    }

    /// **Scenario**: Different namespaces are isolated.
    #[tokio::test]
    async fn test_namespace_isolation() {
        let embedder = Arc::new(MockEmbedder::new(1536));
        let store = InMemoryVectorStore::new(embedder);

        let ns1 = vec!["user1".into()];
        let ns2 = vec!["user2".into()];

        store
            .put(&ns1, "key", &serde_json::json!("v1"))
            .await
            .unwrap();
        store
            .put(&ns2, "key", &serde_json::json!("v2"))
            .await
            .unwrap();

        let v1 = store.get(&ns1, "key").await.unwrap();
        let v2 = store.get(&ns2, "key").await.unwrap();

        assert_eq!(v1, Some(serde_json::json!("v1")));
        assert_eq!(v2, Some(serde_json::json!("v2")));
    }

    /// **Scenario**: Cosine similarity returns 0.0 for zero vectors.
    #[test]
    fn test_cosine_similarity_zero_vectors() {
        let a: Vec<f32> = vec![0.0, 0.0, 0.0];
        let b: Vec<f32> = vec![1.0, 2.0, 3.0];
        assert_eq!(InMemoryVectorStore::cosine_similarity(&a, &b), 0.0);
        assert_eq!(InMemoryVectorStore::cosine_similarity(&b, &a), 0.0);
    }

    /// **Scenario**: Cosine similarity returns 1.0 for identical vectors.
    #[test]
    fn test_cosine_similarity_identical() {
        let a: Vec<f32> = vec![1.0, 2.0, 3.0];
        let b: Vec<f32> = vec![1.0, 2.0, 3.0];
        let sim = InMemoryVectorStore::cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6, "Expected ~1.0, got {}", sim);
    }

    /// **Scenario**: Search without query returns entries up to limit.
    #[tokio::test]
    async fn test_search_no_query() {
        let embedder = Arc::new(MockEmbedder::new(1536));
        let store = InMemoryVectorStore::new(embedder);

        let ns = vec!["test".into()];
        store
            .put(&ns, "key1", &serde_json::json!({"text": "first"}))
            .await
            .unwrap();
        store
            .put(&ns, "key2", &serde_json::json!({"text": "second"}))
            .await
            .unwrap();

        let hits = store.search(&ns, None, Some(10)).await.unwrap();
        assert_eq!(hits.len(), 2);
        for hit in &hits {
            assert!(hit.score.is_none());
        }
    }

    /// **Scenario**: Search with empty query returns entries up to limit.
    #[tokio::test]
    async fn test_search_empty_query() {
        let embedder = Arc::new(MockEmbedder::new(1536));
        let store = InMemoryVectorStore::new(embedder);

        let ns = vec!["test".into()];
        store
            .put(&ns, "key1", &serde_json::json!({"text": "first"}))
            .await
            .unwrap();

        let hits = store.search(&ns, Some(""), Some(10)).await.unwrap();
        assert_eq!(hits.len(), 1);
    }
}

//! # Memory: Checkpointing and Long-term Store
//!
//! Aligns with LangGraph's [Checkpointer] + [Store]. See `docs/rust-langgraph/16-memory-design.md`.
//!
//! ## Overview
//!
//! The memory module provides two distinct capabilities:
//!
//! 1. **Checkpointer** — Per-thread state snapshots for time-travel, branching, and resumable
//!    conversations. Keys checkpoints by `(thread_id, checkpoint_ns, checkpoint_id)`.
//! 2. **Store** — Cross-session key-value storage for long-term memory (preferences, facts, etc.).
//!    Isolated by [`Namespace`] (e.g. `[user_id, "memories"]`). Optional vector search via LanceDB.
//!
//! ## Config
//!
//! [`RunnableConfig`] is passed to `CompiledStateGraph::invoke`. When using a checkpointer:
//! - `thread_id`: Required. Identifies the conversation/thread.
//! - `checkpoint_id`: Optional. Load a specific checkpoint (time-travel / branch).
//! - `checkpoint_ns`: Optional namespace for subgraphs.
//! - `user_id`: Used by Store for multi-tenant isolation.
//!
//! ## Checkpointer Implementations
//!
//! | Type         | Persistence | Use case                    | Feature  |
//! |--------------|-------------|-----------------------------|----------|
//! | [`MemorySaver`]  | In-memory   | Dev, tests                  | —        |
//! | [`SqliteSaver`]  | SQLite file | Single-node, production     | `sqlite` |
//!
//! Use with [`StateGraph::compile_with_checkpointer`](crate::graph::StateGraph::compile_with_checkpointer).
//! [`JsonSerializer`] is required for `SqliteSaver` (state must be `Serialize + DeserializeOwned`).
//!
//! ## Store Implementations
//!
//! | Type             | Persistence | Search                      | Feature  |
//! |------------------|-------------|-----------------------------|----------|
//! | [`InMemoryStore`] | In-memory   | String filter (key/value)   | —        |
//! | [`SqliteStore`]   | SQLite file | String filter               | `sqlite` |
//! | `LanceStore`      | LanceDB     | Vector similarity (semantic)| `lance`  |
//!
//! `LanceStore` (feature `lance`) requires an `Embedder` for vector indexing; search with `query` uses semantic similarity.

mod checkpoint;
mod checkpointer;
mod config;
#[cfg(feature = "lance")]
mod embedder;
mod in_memory_store;
mod memory_saver;
mod serializer;
mod store;

#[cfg(feature = "lance")]
mod lance_store;
#[cfg(feature = "sqlite")]
mod sqlite_saver;
#[cfg(feature = "sqlite")]
mod sqlite_store;

pub use checkpoint::{Checkpoint, CheckpointListItem, CheckpointMetadata, CheckpointSource};
pub use checkpointer::{CheckpointError, Checkpointer};
pub use config::RunnableConfig;
pub use in_memory_store::InMemoryStore;
pub use memory_saver::MemorySaver;
pub use serializer::{JsonSerializer, Serializer};
pub use store::{Namespace, Store, StoreError, StoreSearchHit};

#[cfg(feature = "lance")]
pub use embedder::Embedder;
#[cfg(feature = "lance")]
pub use lance_store::LanceStore;
#[cfg(feature = "sqlite")]
pub use sqlite_saver::SqliteSaver;
#[cfg(feature = "sqlite")]
pub use sqlite_store::SqliteStore;

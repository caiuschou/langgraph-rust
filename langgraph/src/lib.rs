//! # LangGraph for Rust
//!
//! A minimal, LangGraph-inspired agent framework in Rust. Build stateful agents and graphs
//! with a simple **state-in, state-out** design: one shared state type flows through nodes,
//! with no separate Input/Output types.
//!
//! ## Design Principles
//!
//! - **Single state type**: Each graph uses one state struct (e.g. `AgentState`) that all
//!   nodes read from and write to.
//! - **One node per `Agent::run`**: Each agent implements a single step: receive state,
//!   return updated state. No streaming or complex I/O in the core API.
//! - **State graphs**: Compose agents into `StateGraph` with conditional edges. Design docs:
//!   `docs/rust-langgraph/09-minimal-agent-design.md`, `docs/rust-langgraph/11-state-graph-design.md`.
//!
//! ## Main Modules
//!
//! - [`graph`]: `StateGraph`, `CompiledStateGraph`, `Node`, `Next` — build and run state graphs.
//! - [`react`]: ReAct-style nodes (`ThinkNode`, `ActNode`, `ObserveNode`) for reasoning + tool use.
//! - [`llm`]: `LlmClient` trait, `MockLlm`, and optional `ChatZhipu` / `ChatOpenAI` via features.
//! - [`memory`]: Checkpointing, stores, and optional SQLite/LanceDB persistence.
//! - [`tool_source`]: Tool specs and execution; optional MCP integration.
//! - [`traits`]: Core `Agent` trait — implement for custom agents.
//!
//! ## Features
//!
//! - `mcp` (default): MCP tool source for external tools.
//! - `sqlite` (default): Persistent checkpointer and store.
//! - `zhipu`: OpenAI-compatible chat (e.g. GLM) via `async-openai`.
//! - `lance`: LanceDB vector store for long-term memory.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use async_trait::async_trait;
//! use langgraph::{Agent, AgentError, Message};
//!
//! #[derive(Clone, Default)]
//! struct MyState { messages: Vec<Message> }
//!
//! struct EchoAgent;
//!
//! #[async_trait]
//! impl Agent for EchoAgent {
//!     fn name(&self) -> &str { "echo" }
//!     type State = MyState;
//!     async fn run(&self, state: Self::State) -> Result<Self::State, AgentError> {
//!         let mut m = state.messages;
//!         if let Some(Message::User(s)) = m.last() {
//!             m.push(Message::Assistant(s.clone()));
//!         }
//!         Ok(MyState { messages: m })
//!     }
//! }
//!
//! # #[tokio::main]
//! # async fn main() {
//! let mut state = MyState::default();
//! state.messages.push(Message::User("hello".into()));
//! let out = EchoAgent.run(state).await.unwrap();
//! # }
//! ```
//!
//! Run the full echo example: `cargo run -p langgraph-examples --example echo -- "hello"`
//!
//! ## Examples
//!
//! Concrete agents and state types (e.g. `EchoAgent`, `AgentState`) live in `langgraph-examples`,
//! not in this framework crate.

pub mod error;
pub mod graph;
pub mod llm;
pub mod message;
pub mod memory;
pub mod react;
pub mod state;
pub mod tool_source;
pub mod traits;

pub use error::AgentError;
pub use graph::{CompilationError, CompiledStateGraph, Next, Node, NodeMiddleware, StateGraph};
pub use llm::{LlmClient, LlmResponse, MockLlm, ToolChoiceMode};
#[cfg(feature = "zhipu")]
pub use llm::{ChatOpenAI, ChatZhipu};
pub use message::Message;
pub use state::{ReActState, ToolCall, ToolResult};
pub use react::{ActNode, ObserveNode, ThinkNode, REACT_SYSTEM_PROMPT};
pub use tool_source::{MockToolSource, ToolCallContent, ToolSource, ToolSourceError, ToolSpec};
#[cfg(feature = "mcp")]
pub use tool_source::McpToolSource;
pub use memory::{
    Checkpoint, CheckpointError, CheckpointListItem, CheckpointMetadata, CheckpointSource,
    Checkpointer, InMemoryStore, JsonSerializer, MemorySaver, Namespace, RunnableConfig, Store,
    StoreError, StoreSearchHit,
};
#[cfg(feature = "lance")]
pub use memory::{Embedder, LanceStore};
#[cfg(feature = "sqlite")]
pub use memory::{SqliteSaver, SqliteStore};
pub use traits::Agent;

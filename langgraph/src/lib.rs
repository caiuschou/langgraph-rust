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
//! - [`llm`]: `LlmClient` trait, `MockLlm`, and optional `ChatOpenAI` via features.
//! - [`memory`]: Checkpointing, stores, and optional SQLite/LanceDB persistence.
//! - [`tool_source`]: Tool specs and execution; optional MCP integration.
//! - [`traits`]: Core `Agent` trait — implement for custom agents.
//!
//! ## Features
//!
//! - `mcp` (default): MCP tool source for external tools.
//! - `sqlite` (default): Persistent checkpointer and store.
//! - `openai`: OpenAI-compatible chat (e.g., OpenAI) via `async-openai`.
//! - `lance`: LanceDB vector store for long-term memory.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use async_trait::async_trait;
//! use langgraph::{Agent, AgentError, Message};
//!
//! #[derive(Clone, Debug, Default)]
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

pub mod cache;
pub mod channels;
pub mod error;
pub mod graph;
pub mod llm;
pub mod managed;
pub mod memory;
pub mod message;
pub mod react;
#[cfg(all(feature = "sqlite", feature = "mcp"))]
pub mod react_builder;
pub mod state;
pub mod stream;
pub mod tool_source;
pub mod tools;
pub mod traits;

pub use cache::{Cache, CacheError, InMemoryCache};
pub use channels::{
    BinaryOperatorAggregate, Channel, ChannelError, EphemeralValue, LastValue, NamedBarrierValue,
    Topic,
};
pub use error::AgentError;
pub use graph::{
    generate_dot, generate_text, log_graph_complete, log_graph_error, log_graph_start,
    log_node_complete, log_node_start, log_state_update, CompilationError, CompiledStateGraph,
    DefaultInterruptHandler, GraphInterrupt, Interrupt, InterruptHandler, NameNode, Next, Node,
    NodeMiddleware, RetryPolicy, RunContext, Runtime, StateGraph, END, START,
};
#[cfg(feature = "openai")]
pub use llm::ChatOpenAI;
pub use llm::{LlmClient, LlmResponse, MockLlm, ToolChoiceMode};
pub use managed::{IsLastStep, ManagedValue};
#[cfg(all(feature = "lance", feature = "openai"))]
pub use memory::OpenAIEmbedder;
pub use memory::{
    Checkpoint, CheckpointError, CheckpointListItem, CheckpointMetadata, CheckpointSource,
    Checkpointer, InMemoryStore, JsonSerializer, MemorySaver, Namespace, RunnableConfig, Store,
    StoreError, StoreSearchHit,
};
#[cfg(feature = "lance")]
pub use memory::{Embedder, LanceStore};
#[cfg(feature = "sqlite")]
pub use memory::{SqliteSaver, SqliteStore};
pub use message::Message;
pub use react::{
    tools_condition, ActNode, ErrorHandlerFn, HandleToolErrors, ObserveNode, ThinkNode,
    ToolsConditionResult, DEFAULT_EXECUTION_ERROR_TEMPLATE, DEFAULT_TOOL_ERROR_TEMPLATE,
    REACT_SYSTEM_PROMPT,
};
#[cfg(all(feature = "sqlite", feature = "mcp"))]
pub use react_builder::{build_react_run_context, ReactBuildConfig, ReactRunContext};
pub use state::{ReActState, ToolCall, ToolResult};
pub use stream::{
    CheckpointEvent, MessageChunk, StreamEvent, StreamMetadata, StreamMode, StreamWriter,
    ToolStreamWriter,
};
#[cfg(feature = "mcp")]
pub use tool_source::McpToolSource;
pub use tool_source::{
    MemoryToolsSource, MockToolSource, ShortTermMemoryToolSource, StoreToolSource, ToolCallContent,
    ToolCallContext, ToolSource, ToolSourceError, ToolSpec, TOOL_GET_RECENT_MESSAGES,
    TOOL_LIST_MEMORIES, TOOL_RECALL, TOOL_REMEMBER, TOOL_SEARCH_MEMORIES,
};
#[cfg(feature = "mcp")]
pub use tools::{register_mcp_tools, McpToolAdapter};
pub use traits::Agent;

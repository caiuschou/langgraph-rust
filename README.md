# LangGraph for Rust

A minimal, LangGraph-inspired agent framework in Rust. Build stateful agents and graphs with a simple **state-in, state-out** design.

## Overview

LangGraph-rust provides a lightweight framework for building stateful AI agents in Rust. It follows the design principles of [LangGraph](https://github.com/langchain-ai/langgraph), bringing the power of stateful agent graphs to Rust's async ecosystem.

### Key Design Principles

- **Single state type**: Each graph uses one shared state struct that all nodes read from and write to
- **One step per run**: Each agent implements a single step—receive state, return updated state
- **State graphs**: Compose agents into graphs with conditional edges for complex workflows
- **Minimal core API with optional streaming**: `invoke` stays state-in/state-out; use `stream` for incremental output when you need it

## Features

- **State Graphs**: Build and run stateful agent graphs with conditional routing
- **ReAct Pattern**: Built-in support for reasoning + acting loops (Think → Act → Observe); **ReactRunner** and **build_react_runner** for config-driven ReAct (optional persistence, MCP, memory tools)
- **LLM Integration**: Flexible `LlmClient` trait with mock and OpenAI-compatible implementations
- **Memory & Checkpointing**: In-memory and persistent storage for agent state
- **Tool Integration**: Extensible tool system with MCP (Model Context Protocol) support
- **Persistence**: Optional SQLite and LanceDB backends for long-term memory
- **Middleware**: Wrap node execution with custom async logic (logging, monitoring, retry, etc.)
- **Streaming**: Stream per-step states or node updates via `CompiledStateGraph::stream` with selectable modes
- **Channels**: Flexible state update strategies (LastValue, EphemeralValue, BinaryOperatorAggregate, Topic, NamedBarrierValue)
- **Runtime Context**: Custom runtime context, store access, and managed values support
- **Cache System**: In-memory caching with TTL support for node results
- **Retry Mechanism**: Configurable retry policies (fixed interval, exponential backoff)
- **Interrupt Handling**: Human-in-the-loop support with interrupt handlers
- **Graph Visualization**: Generate DOT and text representations of graphs
- **Managed Values**: Access to step metadata and graph execution context

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
langgraph = "0.1"
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"
```

### Feature Flags

- `lance`: Enable LanceDB vector store for long-term memory (optional; heavy dependency).  
  MCP, SQLite checkpointing/storage, in-memory vector store, and OpenAI-compatible chat are included by default (no feature gate).

## Configuration

### Environment Variables

The CLI loads configuration from a `.env` file in the project root. Copy `.env.example` to `.env` and fill in your values:

```bash
cp .env.example .env
```

#### Chat Configuration (LLM)

| Variable | Description | Default | Example |
|----------|-------------|----------|----------|
| `OPENAI_API_KEY` | OpenAI API key (required) | - | `sk-...` |
| `OPENAI_API_BASE` | API base URL | `https://api.openai.com/v1` | `https://api.openai.com/v1` |
| `OPENAI_MODEL` | Model name | `gpt-4o-mini` | `gpt-4o-mini` |
| `OPENAI_TEMPERATURE` | Sampling temperature (0-2) | - | `0.2` |
| `OPENAI_TOOL_CHOICE` | Tool choice mode | - | `auto\|none\|required` |

#### Embeddings Configuration (Vector Search)

| Variable | Description | Default | Example |
|----------|-------------|----------|----------|
| `EMBEDDING_API_KEY` | Embeddings API key (optional, uses OPENAI_API_KEY if not set) | `OPENAI_API_KEY` | `sk-...` |
| `EMBEDDING_API_BASE` | Embeddings API base URL (optional, uses OPENAI_API_BASE if not set) | `OPENAI_API_BASE` | `https://api.openai.com/v1` |
| `EMBEDDING_MODEL` | Embeddings model name | `text-embedding-3-small` | `text-embedding-3-small` |

#### ReAct / ReactRunner (ReactBuildConfig::from_env())

When using the config-driven ReAct API (`ReactBuildConfig::from_env()`, `build_react_runner`), the following are read in addition to the above:

| Variable | Description | Default |
|----------|-------------|---------|
| `THREAD_ID` | Thread ID for short-term memory (checkpointer); enables multi-turn per thread | - |
| `USER_ID` | User ID for long-term memory (store); with embedding config enables semantic memory | - |
| `DB_PATH` | SQLite path for checkpointer/store | `memory.db` at build time |
| `REACT_SYSTEM_PROMPT` | Override default ReAct system prompt | built-in `REACT_SYSTEM_PROMPT` |
| `EXA_API_KEY` | When set, enables Exa MCP for web search | - |
| `MCP_EXA_URL` | Exa MCP server URL | `https://mcp.exa.ai/mcp` |
| `MCP_REMOTE_CMD` | Command for mcp-remote (stdio→HTTP bridge) | `npx` |
| `MCP_REMOTE_ARGS` | Args for mcp-remote | `-y mcp-remote` |
| `MCP_VERBOSE` / `VERBOSE` | Inherit MCP subprocess stderr for debug logs | `false` |
| `OPENAI_BASE_URL` | Used by default LLM when `build_react_runner(config, None, _)` | - |

#### Using Different Providers

**Other OpenAI-compatible providers:**
Just set the appropriate `OPENAI_API_BASE` and `EMBEDDING_API_BASE` URLs.

#### Programmatic Usage

You can also configure embeddings programmatically:

```rust
use langgraph_cli::RunConfig;
use langgraph::OpenAIEmbedder;

// Load config from .env
let config = RunConfig::from_env()?;

// Create embedder from config
let embedder = config.create_embedder();

// Embed text
let vectors = embedder.embed(&["Hello, world!"])?;
```

## Quick Start

```rust
use async_trait::async_trait;
use langgraph::{Agent, AgentError, Message};

#[derive(Clone, Debug, Default)]
struct MyState {
    messages: Vec<Message>,
}

struct EchoAgent;

#[async_trait]
impl Agent for EchoAgent {
    fn name(&self) -> &str {
        "echo"
    }

    type State = MyState;

    async fn run(&self, state: Self::State) -> Result<Self::State, AgentError> {
        let mut messages = state.messages;
        if let Some(Message::User(s)) = messages.last() {
            messages.push(Message::Assistant(s.clone()));
        }
        Ok(MyState { messages })
    }
}

#[tokio::main]
async fn main() {
    let mut state = MyState::default();
    state.messages.push(Message::User("hello, world!".to_string()));

    let agent = EchoAgent;
    match agent.run(state).await {
        Ok(s) => {
            if let Some(Message::Assistant(content)) = s.messages.last() {
                println!("{}", content);
            }
        }
        Err(e) => eprintln!("error: {}", e),
    }
}
```

Run the echo example:
```bash
cargo run -p langgraph-examples --example echo -- "hello, world!"
```

## Streaming

Stream graph execution instead of waiting for `invoke` to finish. Choose one or more modes with `StreamMode`:

- `Values`: full state after each node
- `Updates`: node id + state after each node
- `Messages`: LLM token streaming from ThinkNode
- `Custom`: custom events from nodes via `StreamWriter`
- `Checkpoints`: checkpoint save events
- `Tasks`: task start/end events
- `Debug`: debug information during execution

```rust,no_run
use std::collections::HashSet;
use std::sync::Arc;
use async_trait::async_trait;
use tokio_stream::StreamExt;
use langgraph::{AgentError, Next, Node, StateGraph, StreamEvent, StreamMode, START, END};

struct Add(&'static str, i32);

#[async_trait]
impl Node<i32> for Add {
    fn id(&self) -> &str { self.0 }
    async fn run(&self, state: i32) -> Result<(i32, Next), AgentError> {
        Ok((state + self.1, Next::Continue))
    }
}

#[tokio::main]
async fn main() {
    let mut g = StateGraph::<i32>::new();
    g.add_node("first", Arc::new(Add("first", 1)));
    g.add_node("second", Arc::new(Add("second", 2)));
    g.add_edge(START, "first");
    g.add_edge("first", "second");
    g.add_edge("second", END);

    let compiled = g.compile().unwrap();
    let modes = HashSet::from_iter([StreamMode::Updates]);
    let mut stream = compiled.stream(0, None, modes);

    while let Some(event) = stream.next().await {
        match event {
            StreamEvent::Updates { node_id, state } => {
                println!("{node_id} -> {state}");
            }
            StreamEvent::Values { state } => {
                println!("State: {state}");
            }
            StreamEvent::Messages { chunk } => {
                println!("Message chunk: {:?}", chunk);
            }
            StreamEvent::Custom { name, data } => {
                println!("Custom event: {} = {:?}", name, data);
            }
            _ => {}
        }
    }
}
```

### Custom Streaming from Nodes

Nodes can emit custom events using `StreamWriter`:

```rust
use langgraph::graph::{RunContext, StreamWriter};
use langgraph::stream::StreamEvent;

async fn my_node(state: MyState, ctx: &RunContext<MyState>) -> Result<(MyState, Next), AgentError> {
    // Emit custom events
    if let Some(writer) = ctx.stream_writer() {
        writer.emit_custom("progress", serde_json::json!({"step": 1})).await?;
    }
    
    // ... node logic ...
    
    Ok((updated_state, Next::Continue))
}
```

## ReAct Agent Example

The ReAct (Reasoning + Acting) pattern enables agents to reason about problems, take actions using tools, and observe results before continuing. This is implemented as a three-node loop:

### How ReAct Works

```
User Query → Think → Act → Observe → Think → ...
                   ↓                    ↓
              (use tools)         (analyze results)
```

1. **Think**: LLM reasons about the current state and decides what to do
2. **Act**: Execute tool calls (if any) and gather results
3. **Observe**: LLM observes the results and decides whether to answer or continue

### Recommended: config-driven ReAct with ReactRunner

For most use cases, use **ReactBuildConfig** and **build_react_runner** so the library builds the graph, checkpointer, store, and tool source from config. Set `OPENAI_API_KEY` and `OPENAI_MODEL` (and optionally `THREAD_ID`, `USER_ID`, `EXA_API_KEY`, embedding vars) in `.env`, then:

```rust
use langgraph::{build_react_runner, ReactBuildConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();

    let config = ReactBuildConfig::from_env();
    // llm: None => use config's OPENAI_* to build default LLM
    let runner = build_react_runner(&config, None, false).await?;

    let result = runner.invoke("What time is it?").await?;

    if let Some(reply) = result.last_assistant_reply() {
        println!("{}", reply);
    }
    Ok(())
}
```

- **Multi-turn**: set `THREAD_ID` so the runner uses a checkpointer and resumes from the last checkpoint.
- **Long-term memory**: set `USER_ID` and embedding-related env vars to enable semantic memory (and memory tools).
- **Custom system prompt**: set `config.system_prompt` (or `REACT_SYSTEM_PROMPT` in env) before calling `build_react_runner`.
- **Streaming**: use `runner.stream_with_callback(user_message, Some(|ev| { ... })).await` and handle `StreamEvent` (e.g. `TaskStart`, `Messages`, `Updates`).
- **Custom invoke flow**: use `build_react_initial_state(user_message, checkpointer, runnable_config, system_prompt)` to build initial state and pass it to your own compiled graph.

### Basic ReAct Agent (manual graph)

Alternatively, build the Think → Act → Observe graph by hand. Here's a complete example using a mock LLM and tool source:

```rust
use std::sync::Arc;
use langgraph::{
    ActNode, CompiledStateGraph, Message, MockLlm, MockToolSource,
    ObserveNode, ReActState, REACT_SYSTEM_PROMPT, StateGraph, ThinkNode, START, END,
};

#[tokio::main]
async fn main() {
    // Create the ReAct graph nodes
    let think = ThinkNode::new(Box::new(MockLlm::with_get_time_call()));
    let act = ActNode::new(Box::new(MockToolSource::get_time_example()));
    let observe = ObserveNode::new();

    // Build the graph: START → think → act → observe → END
    let mut graph = StateGraph::<ReActState>::new();
    graph
        .add_node("think", Arc::new(think))
        .add_node("act", Arc::new(act))
        .add_node("observe", Arc::new(observe))
        .add_edge(START, "think")
        .add_edge("think", "act")
        .add_edge("act", "observe")
        .add_edge("observe", END);

    let compiled = graph.compile().expect("valid graph");

    // Initialize state with system prompt and user message
    let state = ReActState {
        messages: vec![
            Message::system(REACT_SYSTEM_PROMPT),
            Message::user("What time is it?"),
        ],
        tool_calls: vec![],
        tool_results: vec![],
        turn_count: 0,
    };

    // Run the agent
    let result = compiled.invoke(state, None).await.unwrap();

    if let Some(reply) = result.last_assistant_reply() {
        println!("{}", reply);
    }
}
```

### ReAct Agent with Real LLM (manual graph)

For production use, replace the mock components with real implementations. You can still use the recommended `build_react_runner` flow above; this example shows the same with a manually built graph:

```rust
use std::sync::Arc;
use langgraph::{
    ActNode, ChatOpenAI, CompiledStateGraph, Message, MockToolSource,
    ObserveNode, ReActState, REACT_SYSTEM_PROMPT, StateGraph, ThinkNode, ToolSource, START, END,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();  // Load OPENAI_API_KEY from .env

    // Create tool source and register tools with LLM
    let tool_source = MockToolSource::get_time_example();
    let tools = tool_source.list_tools().await?;

    // Initialize LLM with tool capabilities
    let llm = ChatOpenAI::new("gpt-4o-mini").with_tools(tools);
    let think = ThinkNode::new(Box::new(llm));
    let act = ActNode::new(Box::new(tool_source));
    let observe = ObserveNode::new();

    // Build and compile graph: START → think → act → observe → END
    let mut graph = StateGraph::<ReActState>::new();
    graph
        .add_node("think", Arc::new(think))
        .add_node("act", Arc::new(act))
        .add_node("observe", Arc::new(observe))
        .add_edge(START, "think")
        .add_edge("think", "act")
        .add_edge("act", "observe")
        .add_edge("observe", END);

    let compiled = graph.compile()?;

    // Run with user query
    let state = ReActState {
        messages: vec![
            Message::system(REACT_SYSTEM_PROMPT),
            Message::user("What time is it?"),
        ],
        tool_calls: vec![],
        tool_results: vec![],
        turn_count: 0,
    };

    let result = compiled.invoke(state, None).await?;
    if let Some(reply) = result.last_assistant_reply() {
        println!("{}", reply);
    }
    Ok(())
}
```

### Custom Tools

Create custom tools by implementing the `ToolSource` trait:

```rust
use async_trait::async_trait;
use langgraph::{ToolSource, ToolSourceError, ToolSpec, ToolCallContent};
use serde_json::json;

struct MyTools;

#[async_trait]
impl ToolSource for MyTools {
    async fn list_tools(&self) -> Result<Vec<ToolSpec>, ToolSourceError> {
        Ok(vec![
            ToolSpec {
                name: "calculator".to_string(),
                description: Some("Perform mathematical calculations".to_string()),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "expression": {"type": "string"}
                    },
                    "required": ["expression"]
                }),
            },
        ])
    }

    async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<ToolCallContent, ToolSourceError> {
        match name {
            "calculator" => {
                // Parse and calculate
                let result = "42";
                Ok(ToolCallContent { text: result.to_string() })
            }
            _ => Err(ToolSourceError::NotFound(name.to_string())),
        }
    }
}
```

### Running ReAct Examples

```bash
# Mock ReAct agent
cargo run -p langgraph-examples --example react_linear -- "What time is it?"

# ReAct with MCP tools
cargo run -p langgraph-examples --example react_mcp -- "Search for Rust news"
```

## Examples

The `langgraph-examples` crate contains various examples:

| Example | Description |
|---------|-------------|
| `echo` | Simple echo agent |
| `react_linear` | Linear ReAct loop with reasoning |
| `react_mcp` | ReAct agent with MCP tools |
| `react_exa` | ReAct agent with web search |
| `react_mcp_gitlab` | ReAct agent with GitLab MCP tools |
| `react_memory` | ReAct agent with memory tools |
| `memory_checkpoint` | Checkpointing and state recovery |
| `memory_persistence` | Persistent storage with SQLite |
| `openai_embedding` | OpenAI embeddings for vector search |
| `state_graph_echo` | State graph with conditional routing |

Run examples:
```bash
cargo run -p langgraph-examples --example <name>
```

## Core Concepts

### State

All agents operate on a single state type that flows through the graph:

```rust
#[derive(Clone, Default)]
struct AgentState {
    messages: Vec<Message>,
    tool_calls: Vec<ToolCall>,
    tool_results: Vec<ToolResult>,
}
```

### Channels

Channels control how state updates are aggregated when multiple nodes write to the same state field:

```rust
use langgraph::channels::{LastValue, EphemeralValue, Topic, BinaryOperatorAggregate, NamedBarrierValue};
use std::collections::HashSet;

// LastValue: Keep only the most recent value (default)
let channel = LastValue::new();

// EphemeralValue: Clear after reading (for one-time signals)
let channel = EphemeralValue::new();

// Topic: Accumulate values into a list (for message history)
let channel = Topic::<String>::new();

// BinaryOperatorAggregate: Custom aggregation (e.g., sum, max)
let channel = BinaryOperatorAggregate::new(0, |a, b| a + b);

// NamedBarrierValue: Wait until all named values are received (for synchronization)
let names: HashSet<String> = ["step1".to_string(), "step2".to_string()].into_iter().collect();
let channel = NamedBarrierValue::new(names);
```

### State Updaters

Customize how node outputs merge into graph state:

```rust
use langgraph::channels::{FieldBasedUpdater, StateUpdater};
use std::sync::Arc;

#[derive(Clone, Debug)]
struct MyState {
    messages: Vec<String>,
    count: i32,
}

// Custom updater: append messages, accumulate count
let updater = FieldBasedUpdater::new(|current: &mut MyState, update: &MyState| {
    current.messages.extend(update.messages.iter().cloned());
    current.count += update.count;
});

let graph = StateGraph::<MyState>::new()
    .with_state_updater(Arc::new(updater));
```

### Agents

Agents implement the `Agent` trait:

```rust
#[async_trait]
pub trait Agent {
    fn name(&self) -> &str;
    type State;
    async fn run(&self, state: Self::State) -> Result<Self::State, AgentError>;
}
```

### State Graphs

Build a **linear chain** with `add_node` and `add_edge` (use `START` and `END`). At runtime, each node returns `Next` (Continue, Node(id), or End); the runner follows the chain or jumps. For example, ReAct uses think → act → observe → (observe returns `Next::Node("think")` to loop or `Next::End` to stop).

```rust
use langgraph::{StateGraph, START, END};
use std::sync::Arc;

let mut graph = StateGraph::new();
graph.add_node("think", Arc::new(think_node));
graph.add_node("act", Arc::new(act_node));
graph.add_node("observe", Arc::new(observe_node));
graph.add_edge(START, "think");
graph.add_edge("think", "act");
graph.add_edge("act", "observe");
graph.add_edge("observe", END);

let compiled = graph.compile()?;
let result = compiled.invoke(state, None).await?;
```

### Runtime Context

Access runtime context, stores, and managed values in nodes:

```rust
use langgraph::graph::{RunContext, Runtime};
use langgraph::memory::{InMemoryStore, Namespace};
use std::sync::Arc;

let store = Arc::new(InMemoryStore::new());
let ctx = RunContext::<MyState>::new(config)
    .with_store(store.clone())
    .with_runtime_context(serde_json::json!({"user_id": "123"}));

let result = graph.invoke_with_context(initial_state, ctx).await?;

// In your node, access runtime context:
// let store = run_context.store();
// let runtime_ctx = run_context.runtime_context();
```

### Managed Values

Access step metadata and execution context:

```rust
use langgraph::managed::{ManagedValue, IsLastStep};

// Check if this is the last step
let is_last = run_context.get_managed_value::<IsLastStep>();
if let Some(IsLastStep(true)) = is_last {
    // Final cleanup logic
}
```

### Middleware

Middleware allows you to wrap node execution with custom async logic (around pattern):

```rust
use std::sync::Arc;
use langgraph::{AgentError, NodeMiddleware, Next, StateGraph};
use std::pin::Pin;

// Custom middleware that logs node execution
struct LoggingMiddleware;

#[async_trait]
impl NodeMiddleware<MyState> for LoggingMiddleware {
    async fn around_run(
        &self,
        node_id: &str,
        state: MyState,
        inner: Box<dyn FnOnce(MyState) -> Pin<Box<dyn std::future::Future<Output = Result<(MyState, Next), AgentError>> + Send>> + Send>,
    ) -> Result<(MyState, Next), AgentError> {
        eprintln!("[node] enter node={}", node_id);
        let result = inner(state).await;
        match &result {
            Ok((_, ref next)) => eprintln!("[node] exit node={} next={:?}", node_id, next),
            Err(e) => eprintln!("[node] exit node={} error={}", node_id, e),
        }
        result
    }
}

// Use middleware via fluent API
let middleware = Arc::new(LoggingMiddleware);
let compiled = graph.with_middleware(middleware).compile()?;

// Or pass to compile method
let compiled = graph.compile_with_middleware(Arc::new(LoggingMiddleware))?;

// Or combine with checkpointer
let compiled = graph.compile_with_checkpointer_and_middleware(
    checkpointer,
    Arc::new(LoggingMiddleware)
)?;
```

### Retry Mechanism

Configure retry policies for node execution:

```rust
use langgraph::graph::RetryPolicy;
use std::time::Duration;

// Fixed interval retry
let policy = RetryPolicy::fixed(3, Duration::from_secs(1));

// Exponential backoff retry
let policy = RetryPolicy::exponential(
    3,
    Duration::from_secs(1),
    Duration::from_secs(10),
    2.0,
);

// Use in graph compilation (if supported)
```

### Interrupt Handling

Handle interrupts for human-in-the-loop workflows:

```rust
use langgraph::graph::{Interrupt, InterruptHandler, DefaultInterruptHandler};

let interrupt = Interrupt::new(serde_json::json!({"action": "approve"}));
let handler = DefaultInterruptHandler;
let result = handler.handle_interrupt(&interrupt)?;
```

### Cache System

In-memory caching with TTL support:

```rust
use langgraph::cache::{Cache, InMemoryCache};
use std::time::Duration;

let cache = InMemoryCache::new();
cache.set("key".to_string(), "value".to_string(), Some(Duration::from_secs(60))).await?;
let value = cache.get(&"key".to_string()).await;
```

### Graph Visualization

Generate visual representations of your graphs:

```rust
use langgraph::graph::{generate_dot, generate_text};

let dot = generate_dot(&compiled_graph);
let text = generate_text(&compiled_graph);
println!("{}", dot); // Graphviz DOT format
println!("{}", text); // Text representation
```

### ReAct Pattern

Built-in ReAct nodes for reasoning + tool use:

```rust
use langgraph::{StateGraph, ThinkNode, ActNode, ObserveNode};
use std::sync::Arc;

let mut graph = StateGraph::new();
graph.add_node("think", Arc::new(ThinkNode::new(Box::new(llm))));
graph.add_node("act", Arc::new(ActNode::new(Box::new(tools))));
graph.add_node("observe", Arc::new(ObserveNode::new()));
```

For a config-driven run without building the graph yourself, use `ReactBuildConfig::from_env()` and `build_react_runner`. To get the final assistant reply from `ReActState`, use `state.last_assistant_reply()` (returns the last Assistant message content, or `None` if there is none).

### Memory: Short-term & Long-term

LangGraph-rust provides two types of memory for different use cases:

#### Short-term Memory (Checkpointer)

**Purpose**: Save and restore conversation state within a single session/thread

- Per-thread state snapshots for resumable conversations
- Time-travel: load any historical checkpoint
- Branching: create alternate conversation paths

**Implementations**:
- `MemorySaver` - In-memory (dev/tests)
- `SqliteSaver` - Persistent SQLite file (production)

```rust
use langgraph::memory::{MemorySaver, RunnableConfig};
use std::sync::Arc;

let checkpointer = Arc::new(MemorySaver::new());
let compiled = graph.compile_with_checkpointer(checkpointer)?;

let config = RunnableConfig {
    thread_id: Some("conversation-1"),
    checkpoint_id: None,
    checkpoint_ns: String::new(),
    user_id: None,
};

// First invoke - saves checkpoint
let result = compiled.invoke(state, Some(config)).await?;

// Resume from last checkpoint
let result2 = compiled.invoke(result, Some(config)).await?;
```

#### Long-term Memory (Store)

**Purpose**: Cross-session key-value storage for persistent knowledge

- Store preferences, facts, documents across sessions
- Namespace isolation (e.g., by user_id)
- Optional semantic search via LanceDB or sqlite-vec vector store

**Implementations**:
- `InMemoryStore` - In-memory (dev/tests)
- `SqliteStore` - Persistent SQLite file (key-value search)
- `SqliteVecStore` - Persistent SQLite file with vector search (semantic search)
- `LanceStore` - Persistent LanceDB vector store (semantic search, feature: `lance`)
- `InMemoryVectorStore` - In-memory vector store with semantic search (feature: `in-memory-vector`)

```rust
use langgraph::memory::{InMemoryStore, Namespace, Store, StoreError};
use std::sync::Arc;

let store = Arc::new(InMemoryStore::new());

let ns = Namespace::new(&["user-123", "preferences"]);
store.put(&ns, "theme", "dark").await?;

// Retrieve
let theme = store.get(&ns, "theme").await?;
assert_eq!(theme, Some("dark".to_string()));

// List all keys
let items = store.list(&ns).await?;
```

#### Semantic Search

Vector-based semantic search for retrieving relevant memories.

**With InMemoryVectorStore (dev/tests):**

```rust
use langgraph::memory::{InMemoryVectorStore, Namespace, Store, Embedder};
use std::sync::Arc;

let embedder = Arc::new(MockEmbedder::new(1536));
let store = InMemoryVectorStore::new(embedder);
let ns = Namespace::new(&["user-123", "memories"]);

// Store memories (automatically embedded)
store.put(&ns, "doc-1", "User likes Rust and enjoys hiking").await?;

// Semantic search
let results = store.search(
    &ns,
    Some("outdoor activities"),  // semantic query
    Some(10),                    // limit
).await?;
```

**With LanceDB (production):**

```rust
use langgraph::memory::{LanceStore, Namespace, Store, Embedder};
use std::sync::Arc;

// Custom embedder
struct MyEmbedder;
impl Embedder for MyEmbedder {
    fn dimension(&self) -> usize { 768 }
    fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, Box<dyn std::error::Error>> {
        // Use your embedding model (e.g., OpenAI, sentence-transformers)
        Ok(texts.iter().map(|_| vec![0.0; 768]).collect())
    }
}

let store = Arc::new(LanceStore::new("data/memories.lance", MyEmbedder)?);
let ns = Namespace::new(&["user-123", "memories"]);

// Store memories (automatically embedded)
store.put(&ns, "doc-1", "User likes Rust and enjoys hiking").await?;

// Semantic search
let results = store.search(
    &ns,
    Some("outdoor activities"),  // semantic query
    5,                            // limit
    None,                         // optional filter
).await?;
```

#### When to Use Which

| Use Case | Recommended |
|----------|-------------|
| Multi-turn conversation state | Short-term (Checkpointer) |
| Resume interrupted conversations | Short-term (Checkpointer) |
| Time-travel / branching | Short-term (Checkpointer) |
| User preferences | Long-term (Store) |
| Facts / documents | Long-term (Store) |
| Semantic search (dev/tests) | In-memory (InMemoryVectorStore) |
| Semantic search (production) | Long-term (LanceStore) |

#### Combining Both

Use checkpointer for conversation flow and Store for persistent knowledge:

```rust
use langgraph::{
    Checkpointer, InMemoryStore, JsonSerializer, Message, RunnableConfig, SqliteSaver,
};
use std::sync::Arc;

let serializer = Arc::new(JsonSerializer);
let checkpointer: Arc<dyn Checkpointer<YourState>> =
    Arc::new(SqliteSaver::new("data/checkpoints.db", serializer)?);
let store = Arc::new(InMemoryStore::new());

let compiled = graph.compile_with_checkpointer(checkpointer)?;

// In your node, use store to read/write persistent memories
let user_pref = store.get(&ns, "theme").await?;
```

### LLM Integration

Use the `LlmClient` trait with various backends:

```rust
use langgraph::{LlmClient, MockLlm, Message};

let llm = MockLlm::new();
let messages = vec![
    Message::system("You are a helpful assistant."),
    Message::user("Hello, world!"),
];
let response = llm.invoke(&messages).await?;

// Or use OpenAI-compatible (with feature "openai", API key from OPENAI_API_KEY env)
use langgraph::ChatOpenAI;

let llm = ChatOpenAI::new("gpt-4o-mini");
```

### Tools

Define and execute tools via `ToolSource`:

```rust
use langgraph::{ToolSpec, ToolSource};
use serde_json::json;

let tools = vec![
    ToolSpec {
        name: "search".to_string(),
        description: Some("Search the web".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": {"type": "string"}
            },
            "required": ["query"]
        }),
    },
    ToolSpec {
        name: "calculate".to_string(),
        description: Some("Perform calculations".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "expression": {"type": "string"}
            },
            "required": ["expression"]
        }),
    },
];
```

## Project Structure

```
langgraph-rust/
├── langgraph/           # Main library crate
│   ├── src/
│   │   ├── cache/       # Cache system (InMemoryCache)
│   │   ├── channels/    # State channels (LastValue, EphemeralValue, Topic, etc.)
│   │   ├── graph/       # State graph implementation
│   │   │   ├── runtime.rs      # Runtime context system
│   │   │   ├── retry.rs         # Retry policies
│   │   │   ├── interrupt.rs     # Interrupt handling
│   │   │   ├── logging.rs       # Structured logging
│   │   │   └── visualization.rs # Graph visualization
│   │   ├── managed/     # Managed values (IsLastStep, etc.)
│   │   ├── memory/      # Checkpointing and storage
│   │   ├── react/       # ReAct pattern nodes
│   │   ├── llm/         # LLM client trait & implementations
│   │   ├── stream/      # Stream modes and events
│   │   └── tool_source/ # Tool execution & MCP
│   └── Cargo.toml
├── langgraph-cli/       # CLI to run ReAct agents (see langgraph-cli/README.md)
│   └── src/
└── langgraph-examples/  # Example agents and usage
    └── examples/
```

## Testing

The project includes comprehensive test coverage across core modules: channels, runtime, cache, retry, interrupt, logging, visualization, streaming, and graph execution.

Run tests:
```bash
cargo test
```

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Acknowledgments

Inspired by [LangGraph](https://github.com/langchain-ai/langgraph) by LangChain.

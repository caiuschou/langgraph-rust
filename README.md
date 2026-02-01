# LangGraph for Rust

A minimal, LangGraph-inspired agent framework in Rust. Build stateful agents and graphs with a simple **state-in, state-out** design.

## Overview

LangGraph-rust provides a lightweight framework for building stateful AI agents in Rust. It follows the design principles of [LangGraph](https://github.com/langchain-ai/langgraph), bringing the power of stateful agent graphs to Rust's async ecosystem.

### Key Design Principles

- **Single state type**: Each graph uses one shared state struct that all nodes read from and write to
- **One step per run**: Each agent implements a single step—receive state, return updated state
- **State graphs**: Compose agents into graphs with conditional edges for complex workflows
- **Minimal core API**: No streaming or complex I/O in the core—keep it simple

## Features

- **State Graphs**: Build and run stateful agent graphs with conditional routing
- **ReAct Pattern**: Built-in support for reasoning + acting loops (Think → Act → Observe)
- **LLM Integration**: Flexible `LlmClient` trait with mock and OpenAI-compatible implementations
- **Memory & Checkpointing**: In-memory and persistent storage for agent state
- **Tool Integration**: Extensible tool system with MCP (Model Context Protocol) support
- **Persistence**: Optional SQLite and LanceDB backends for long-term memory
- **Middleware**: Wrap node execution with custom async logic (logging, monitoring, retry, etc.)

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
langgraph = "0.1"
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"
```

### Feature Flags

- `mcp` (default): Enable MCP tool source for external tools
- `sqlite` (default): Enable SQLite-based checkpointing and storage
- `openai`: Enable OpenAI-compatible chat (e.g., OpenAI) via `async-openai`
- `lance`: Enable LanceDB vector store for long-term memory

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

#[derive(Clone, Default)]
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

### Basic ReAct Agent

Here's a complete example using a mock LLM and tool source:

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
    };

    // Run the agent
    let result = compiled.invoke(state, None).await.unwrap();

    // Print the conversation
    for msg in &result.messages {
        match msg {
            Message::System(s) => println!("[System] {}", s),
            Message::User(s) => println!("[User] {}", s),
            Message::Assistant(s) => println!("[Assistant] {}", s),
        }
    }
}
```

### ReAct Agent with Real LLM

For production use, replace the mock components with real implementations:

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
    };

    let result = compiled.invoke(state, None).await?;
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
| `memory_checkpoint` | Checkpointing and state recovery |
| `memory_persistence` | Persistent storage with SQLite |
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

Compose agents into graphs with conditional routing:

```rust
use langgraph::{StateGraph, Next};
use std::sync::Arc;

let mut graph = StateGraph::new();
graph.add_node("agent", Arc::new(agent));
graph.add_node("tools", Arc::new(tools));

// Conditional routing
graph.add_conditional_edges(
    "agent",
    |state| {
        if state.tool_calls.is_empty() {
            Next::End
        } else {
            Next::Node("tools")
        }
    },
);

graph.add_edge("tools", "agent");

let compiled = graph.compile()?;
let result = compiled.invoke(state, None).await?;
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
- Optional semantic search via LanceDB vector store

**Implementations**:
- `InMemoryStore` - In-memory (dev/tests)
- `SqliteStore` - Persistent SQLite file (key-value search)
- `LanceStore` - Persistent LanceDB vector store (semantic search)

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

#### Semantic Search with LanceDB

Vector-based semantic search for retrieving relevant memories:

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
| Semantic search | Long-term (LanceStore) |

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

// Or use OpenAI-compatible (with feature "openai")
use langgraph::ChatOpenAI;

let llm = ChatOpenAI::new("your-api-key");
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
│   │   ├── graph/       # State graph implementation
│   │   ├── react/       # ReAct pattern nodes
│   │   ├── llm/         # LLM client trait & implementations
│   │   ├── memory/      # Checkpointing and storage
│   │   └── tool_source/ # Tool execution & MCP
│   └── Cargo.toml
└── langgraph-examples/  # Example agents and usage
    └── examples/
```

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Acknowledgments

Inspired by [LangGraph](https://github.com/langchain-ai/langgraph) by LangChain.

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
- `zhipu`: Enable OpenAI-compatible chat (e.g., GLM) via `async-openai`
- `lance`: Enable LanceDB vector store for long-term memory

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
use langgraph::{
    ActNode, CompiledStateGraph, Message, MockLlm, MockToolSource,
    ObserveNode, ReActState, REACT_SYSTEM_PROMPT, StateGraph, ThinkNode,
};

#[tokio::main]
async fn main() {
    // Create the ReAct graph nodes
    let think = ThinkNode::new(Box::new(MockLlm::with_get_time_call()));
    let act = ActNode::new(Box::new(MockToolSource::get_time_example()));
    let observe = ObserveNode::new();

    // Build the graph: think → act → observe → END
    let mut graph = StateGraph::<ReActState>::new();
    graph
        .add_node("think", Box::new(think))
        .add_node("act", Box::new(act))
        .add_node("observe", Box::new(observe))
        .add_edge("think")      // think → act
        .add_edge("act")        // act → observe
        .add_edge("observe");   // observe → END

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
use langgraph::{
    ActNode, ChatZhipu, CompiledStateGraph, Message, MockToolSource,
    ObserveNode, ReActState, REACT_SYSTEM_PROMPT, StateGraph, ThinkNode, ToolSource,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();  // Load ZHIPU_API_KEY from .env

    // Create tool source and register tools with LLM
    let tool_source = MockToolSource::get_time_example();
    let tools = tool_source.list_tools().await?;

    // Initialize LLM with tool capabilities
    let llm = ChatZhipu::new("glm-4-flash").with_tools(tools);
    let think = ThinkNode::new(Box::new(llm));
    let act = ActNode::new(Box::new(tool_source));
    let observe = ObserveNode::new();

    // Build and compile graph
    let mut graph = StateGraph::<ReActState>::new();
    graph
        .add_node("think", Box::new(think))
        .add_node("act", Box::new(act))
        .add_node("observe", Box::new(observe))
        .add_edge("think")
        .add_edge("act")
        .add_edge("observe");

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
use langgraph::{ToolSource, ToolSourceError, ToolSpec};
use async_trait::async_trait;

struct MyTools;

#[async_trait]
impl ToolSource for MyTools {
    async fn list_tools(&self) -> Result<Vec<ToolSpec>, ToolSourceError> {
        Ok(vec![
            ToolSpec::new(
                "calculator",
                "Perform mathematical calculations",
                "{ \"expression\": \"string\" }",
            ),
        ])
    }

    async fn call_tool(&self, name: &str, input: serde_json::Value)
        -> Result<String, ToolSourceError>
    {
        match name {
            "calculator" => {
                // Parse and calculate
                Ok("42".to_string())
            }
            _ => Err(ToolSourceError::ToolNotFound(name.to_string())),
        }
    }
}
```

### Running ReAct Examples

```bash
# Mock ReAct agent
cargo run -p langgraph-examples --example react_linear -- "What time is it?"

# ReAct with real LLM (requires ZHIPU_API_KEY)
cargo run -p langgraph-examples --example react_zhipu -- "3+5 equals?"

# ReAct with MCP tools
cargo run -p langgraph-examples --example react_mcp -- "Search for Rust news"
```

## Examples

The `langgraph-examples` crate contains various examples:

| Example | Description |
|---------|-------------|
| `echo` | Simple echo agent |
| `react_linear` | Linear ReAct loop with reasoning |
| `react_zhipu` | ReAct agent with GLM LLM |
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
use langgraph::{StateGraph, Node, Next};

let mut graph = StateGraph::new();
graph.add_node(Node::new("agent", agent));
graph.add_node(Node::new("tools", tools));
graph.set_entry_point("agent");

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
let result = compiled.invoke(state).await?;
```

### ReAct Pattern

Built-in ReAct nodes for reasoning + tool use:

```rust
use langgraph::react::{ThinkNode, ActNode, ObserveNode};

let mut graph = StateGraph::new();
graph.add_node(Node::new("think", ThinkNode::new(llm)));
graph.add_node(Node::new("act", ActNode::new(tools)));
graph.add_node(Node::new("observe", ObserveNode::new(llm)));
```

### Memory & Checkpointing

Save and restore agent state:

```rust
use langgraph::memory::MemorySaver;

let checkpointer = MemorySaver::new();
let compiled = graph.compile().with_checkpointer(checkpointer)?;

let config = RunnableConfig {
    thread_id: "thread-1",
    checkpoint_id: None,
};

// Run with checkpointing
let result = compiled.invoke(state, Some(config)).await?;

// Resume from checkpoint
let result2 = compiled.invoke(state, Some(config)).await?;
```

### LLM Integration

Use the `LlmClient` trait with various backends:

```rust
use langgraph::llm::{LlmClient, MockLlm};

let llm = MockLlm::new();
let response = llm.chat("Hello, world!").await?;

// Or use OpenAI-compatible (with feature "zhipu")
use langgraph::ChatZhipu;

let llm = ChatZhipu::new("your-api-key");
```

### Tools

Define and execute tools:

```rust
use langgraph::{ToolSpec, ToolSource};

let tools = vec![
    ToolSpec::new("search", "Search the web", search_tool),
    ToolSpec::new("calculate", "Perform calculations", calc_tool),
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

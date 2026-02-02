# langgraph-cli

ReAct agent built on [langgraph](https://github.com/your-org/langgraph): reads OpenAI config from `.env`, usable as a **CLI** or a **library**.

Graph: think → act → observe → END. Uses OpenAI Chat Completions and mock tools (e.g. `get_time`) by default.

## Configuration

Create a `.env` in the project root or current working directory (see `.env.example`):

```bash
OPENAI_API_BASE=https://api.openai.com/v1
OPENAI_API_KEY=sk-...
OPENAI_MODEL=gpt-4o-mini
```

- `OPENAI_API_KEY`: required  
- `OPENAI_API_BASE`: optional, default `https://api.openai.com/v1`  
- `OPENAI_MODEL`: optional, default `gpt-4o-mini`  
- `OPENAI_TEMPERATURE`: optional, 0–2, lower = more deterministic (e.g. 0.2)

## Using as CLI

From the workspace:

```bash
# Positional argument
cargo run -p langgraph-cli -- "What time is it?"

# Or -m / --message
cargo run -p langgraph-cli -- -m "3+5 equals?"

# Lower temperature for more deterministic tool use (e.g. fewer false tool calls)
cargo run -p langgraph-cli -- -t 0.2 -m "3+5 equals?"

# Force tool choice: --tool-choice auto|none|required
cargo run -p langgraph-cli -- --tool-choice auto -m "What time is it?"
```

After installing the binary:

```bash
cargo install --path langgraph-cli
langgraph "Your message"
```

## Using as a library

Add to `Cargo.toml`:

```toml
[dependencies]
langgraph-cli = { path = "../langgraph-cli" }  # or git / crates.io
tokio = { version = "1", features = ["rt-multi-thread"] }
```

### Basic usage

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = langgraph_cli::run("What time is it?").await?;
    for m in &state.messages {
        match m {
            langgraph_cli::Message::System(s) => println!("System: {}", s),
            langgraph_cli::Message::User(s) => println!("User: {}", s),
            langgraph_cli::Message::Assistant(s) => println!("Assistant: {}", s),
        }
    }
    Ok(())
}
```

### Custom configuration

Override API base, key, or model:

```rust
use langgraph_cli::RunConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = RunConfig {
        api_base: "https://api.openai.com/v1".to_string(),
        api_key: "sk-...".to_string(),
        model: "gpt-4o-mini".to_string(),
        temperature: Some(0.2),
        ..RunConfig::from_env()?
    };
    let state = langgraph_cli::run_with_config(&config, "Your message").await?;
    Ok(())
}
```

### Load config from environment variables

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    let config = RunConfig::from_env()?;
    let state = langgraph_cli::run_with_config(&config, "Your message").await?;
    Ok(())
}
```

### Quick enable short-term memory

Enable conversation memory across turns:

```rust
use langgraph_cli::RunConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = RunConfig::from_env()?.with_short_term_memory("user123");
    
    // First message
    let state = langgraph_cli::run_with_config(&config, "My name is Alice").await?;
    
    // Second message - remembers the first
    let state = langgraph_cli::run_with_config(&config, "What's my name?").await?;
    Ok(())
}
```

### Quick enable long-term memory

Persistent memory storage for facts and preferences:

```rust
use langgraph_cli::RunConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = RunConfig::from_env()?.with_long_term_memory("user123");
    
    // Save to memory
    let state = langgraph_cli::run_with_config(&config, "Remember that I like coffee").await?;
    
    // Retrieve from memory
    let state = langgraph_cli::run_with_config(&config, "What do you know about me?").await?;
    Ok(())
}
```

### Quick enable both short-term and long-term memory

```rust
use langgraph_cli::RunConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = RunConfig::from_env()?.with_memory("thread123", "user123");
    
    // Uses both checkpointer and store
    let state = langgraph_cli::run_with_config(&config, "My name is Alice").await?;
    let state = langgraph_cli::run_with_config(&config, "Remember that I like coffee").await?;
    Ok(())
}
```

### Quick disable memory

```rust
use langgraph_cli::RunConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = RunConfig::from_env()?.without_memory();
    
    // Runs without any memory
    let state = langgraph_cli::run_with_config(&config, "What time is it?").await?;
    Ok(())
}
```

### Using web search with Exa MCP

Enable web search capabilities:

```rust
use langgraph_cli::RunConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = RunConfig::from_env()?;
    config.tool_source.exa_api_key = Some("your-exa-api-key".to_string());
    
    let state = langgraph_cli::run_with_config(&config, "Search for latest Rust news").await?;
    Ok(())
}
```

## Dependencies and features

- Default `openai` feature: uses the real OpenAI API (requires the `.env` above).
- Depends on `langgraph` with the `openai` feature (OpenAI-compatible client).

## License

Same as the workspace (e.g. MIT).

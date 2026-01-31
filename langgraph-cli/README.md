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
- `OPENAI_TOOL_CHOICE`: optional, `auto`|`none`|`required`

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

In code:

```rust
let state = langgraph_cli::run("Your message").await?;
for m in &state.messages {
    // Handle langgraph_cli::Message (System / User / Assistant)
}
```

To override API base, key, or model, use `RunConfig` and `run_with_config`:

```rust
dotenv::dotenv().ok();
let config = langgraph_cli::RunConfig::from_env()?;
// Or build config manually
let state = langgraph_cli::run_with_config(&config, "Your message").await?;
```

More on the library API and design: [docs/AS_LIBRARY.md](docs/AS_LIBRARY.md).

## Dependencies and features

- Default `openai` feature: uses the real OpenAI API (requires the `.env` above).
- Depends on `langgraph` with the `zhipu` feature (OpenAI-compatible client).

## License

Same as the workspace (e.g. MIT).

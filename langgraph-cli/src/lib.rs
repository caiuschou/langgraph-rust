//! langgraph-cli library: reusable ReAct run logic for other crates.
//!
//! Reads OpenAI config from .env, builds think → act → observe graph and runs it, returns final state.
//!
//! ## Usage
//!
//! Default run (config from env):
//!
//! ```rust,no_run,ignore
//! let state = langgraph_cli::run("user message").await?;
//! for m in &state.messages {
//!     // handle System / User / Assistant messages
//! }
//! ```
//!
//! Run with overrides (e.g. temperature, memory, Exa MCP) without parsing CLI:
//!
//! ```rust,no_run,ignore
//! use langgraph_cli::{run_with_options, RunOptions};
//!
//! let options = RunOptions {
//!     temperature: Some(0.2),
//!     thread_id: Some("thread-1".into()),
//!     ..Default::default()
//! };
//! let state = run_with_options("user message", &options).await?;
//! ```

mod config;
mod middleware;
mod run;

pub use config::{Error, MemoryConfig, RunConfig, RunOptions, ToolSourceConfig};
pub use langgraph::{Message, ReActState};
pub use run::{run, run_with_config, run_with_options};

#[cfg(test)]
mod tests;

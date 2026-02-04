//! # langgraph-cli
//!
//! Reusable ReAct run logic for the LangGraph Rust ecosystem. Reads config from env (or overrides),
//! builds a think → act → observe graph, and returns the final [`ReActState`].
//!
//! ## Main modules
//!
//! - **Config**: [`RunConfig`], [`RunOptions`], [`MemoryConfig`], [`ToolSourceConfig`] — build
//!   run configuration from env or programmatic overrides.
//! - **Run**: [`run`], [`run_with_options`], [`run_with_config`] — execute the ReAct graph and
//!   get back state; [`build_config_summary`] for human-readable config summary.
//!
//! ## Quick start
//!
//! Default run (config from `.env` and environment):
//!
//! ```rust,no_run
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let state = langgraph_cli::run("user message").await?;
//! for m in &state.messages {
//!     // handle System / User / Assistant messages
//! }
//! # Ok(()) }
//! ```
//!
//! Run with overrides (e.g. temperature, memory, Exa MCP) without parsing CLI:
//!
//! ```rust,no_run
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use langgraph_cli::{run_with_options, RunOptions};
//!
//! let options = RunOptions {
//!     temperature: Some(0.2),
//!     thread_id: Some("thread-1".into()),
//!     ..Default::default()
//! };
//! let state = run_with_options("user message", &options).await?;
//! # Ok(()) }
//! ```
//!
//! ## Binary
//!
//! The `langgraph-cli` binary parses CLI args into [`RunOptions`] and calls [`run_with_options`].
//! Run: `cargo run -p langgraph-cli -- "your message"`.

mod config;
mod run;

pub use config::{Error, MemoryConfig, RunConfig, RunOptions, ToolSourceConfig};
pub use langgraph::{Message, ReActState};
pub use run::{build_config_summary, run, run_with_config, run_with_options};

#[cfg(test)]
mod tests;

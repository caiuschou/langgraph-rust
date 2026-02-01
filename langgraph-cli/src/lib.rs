//! langgraph-cli library: reusable ReAct run logic for other crates.
//!
//! Reads OpenAI config from .env, builds think → act → observe graph and runs it, returns final state.
//!
//! ## Usage
//!
//! ```rust,no_run,ignore
//! let state = langgraph_cli::run("user message").await?;
//! for m in &state.messages {
//!     // handle System / User / Assistant messages
//! }
//! ```

mod config;
mod middleware;
mod run;

pub use config::{Error, MemoryConfig, RunConfig};
pub use langgraph::{Message, ReActState};
pub use run::{run, run_with_config};

#[cfg(test)]
mod tests;

//! Run entry points: run with default config, run_with_config, or run_with_options.
//!
//! Re-exports [`run`], [`run_with_config`], [`run_with_options`] and [`Error`].

pub use crate::config::Error;

mod run_with_config;

use langgraph::ReActState;

use crate::config::{RunConfig, RunOptions};

pub use run_with_config::run_with_config;

/// Run ReAct graph with default config (from .env), returns final state.
///
/// Loads `.env` internally, then calls `run_with_config`.
pub async fn run(user_message: &str) -> Result<ReActState, Error> {
    dotenv::dotenv().ok();
    let config = RunConfig::from_env()?;
    run_with_config(&config, user_message).await
}

/// Run ReAct graph with config from env and optional overrides (e.g. from CLI or programmatic).
///
/// Loads `.env`, builds `RunConfig` from env, applies `options`, then runs the graph.
/// Use this when you have overrides (temperature, tool_choice, memory, db_path, Exa MCP)
/// without parsing CLI. Interacts with [`RunConfig::apply_options`](crate::RunConfig::apply_options)
/// and [`run_with_config`](run_with_config).
pub async fn run_with_options(
    user_message: &str,
    options: &RunOptions,
) -> Result<ReActState, Error> {
    dotenv::dotenv().ok();
    let mut config = RunConfig::from_env()?;
    config.apply_options(options);
    run_with_config(&config, user_message).await
}

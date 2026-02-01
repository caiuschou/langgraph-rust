//! Run entry points: run with default config or run_with_config.
//!
//! Re-exports [`run`], [`run_with_config`] and [`Error`].

pub use crate::config::Error;

#[cfg(not(feature = "sqlite"))]
mod run_with_config_no_sqlite;
#[cfg(feature = "sqlite")]
mod run_with_config_sqlite;

use langgraph::ReActState;

use crate::config::RunConfig;

#[cfg(not(feature = "sqlite"))]
pub use run_with_config_no_sqlite::run_with_config;
#[cfg(feature = "sqlite")]
pub use run_with_config_sqlite::run_with_config;

/// Run ReAct graph with default config (from .env), returns final state.
///
/// Loads `.env` internally, then calls `run_with_config`.
pub async fn run(user_message: &str) -> Result<ReActState, Error> {
    dotenv::dotenv().ok();
    let config = RunConfig::from_env()?;
    run_with_config(&config, user_message).await
}

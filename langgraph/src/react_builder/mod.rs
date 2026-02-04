//! ReAct run context builder: builds checkpointer, store, runnable_config and tool source from config.
//!
//! Callers (e.g. langgraph-cli) hold a [`ReactBuildConfig`] and use [`build_react_run_context`]
//! to obtain a [`ReactRunContext`] (checkpointer, store, runnable_config, tool source). Then
//! [`build_react_runner`] or [`build_react_runner_with_openai`] builds a
//! [`ReactRunner`](crate::ReactRunner) for [`run_react_graph`](crate::run_react_graph).
//!
//! Requires `sqlite` and `mcp` features for full persistence and MCP tool support.
//!
//! # Main types
//!
//! - [`ReactBuildConfig`]: Configuration for DB path, thread_id, user_id, system prompt,
//!   MCP/Exa settings, OpenAI and embedding keys. Use [`ReactBuildConfig::from_env`] to load from env.
//! - [`ReactRunContext`]: Built run resources (checkpointer, store, config, tool source).
//! - [`BuildRunnerError`]: Error when building the runner (e.g. missing API key).
//!
//! # Example
//!
//! ```rust,no_run
//! use langgraph::react_builder::{ReactBuildConfig, build_react_run_context, build_react_runner_with_openai};
//!
//! let config = ReactBuildConfig::from_env();
//! let ctx = build_react_run_context(&config).await?;
//! let runner = build_react_runner_with_openai(&ctx, None).await?;
//! let state = langgraph::run_react_graph(&runner, &ctx, "Hello", None).await?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod build;
mod config;

pub use build::{
    build_react_run_context, build_react_runner, build_react_runner_with_openai, BuildRunnerError,
    ReactRunContext,
};
pub use config::ReactBuildConfig;

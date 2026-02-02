//! ReAct run context builder: builds checkpointer, store, runnable_config and tool_source from config.
//!
//! Used by CLI or other callers that hold a [`ReactBuildConfig`]. Requires `sqlite` and `mcp` features.
//!
//! [`ReactBuildConfig`]: config::ReactBuildConfig

mod config;
mod build;

pub use config::ReactBuildConfig;
pub use build::{build_react_run_context, ReactRunContext};

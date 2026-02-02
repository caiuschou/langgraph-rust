//! ReAct run context builder: builds checkpointer, store, runnable_config and tool_source from config.
//!
//! Used by CLI or other callers that hold a [`ReactBuildConfig`]. Requires `sqlite` and `mcp` features.
//!
//! [`ReactBuildConfig`]: config::ReactBuildConfig

mod build;
mod config;

pub use build::{build_react_run_context, ReactRunContext};
pub use config::ReactBuildConfig;

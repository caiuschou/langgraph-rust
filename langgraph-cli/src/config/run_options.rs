//! Optional overrides for running the ReAct graph (CLI args or programmatic).
//!
//! Used by [`RunConfig::apply_options`](super::RunConfig::apply_options) and
//! [`run_with_options`](crate::run_with_options). Callers (e.g. binary or tests) build
//! a `RunOptions` and pass it to get env-based config with overrides applied.

use langgraph::ToolChoiceMode;

/// Optional overrides for a run: temperature, tool choice, memory, DB path, Exa MCP.
///
/// Used with [`run_with_options`](crate::run_with_options) or
/// [`RunConfig::apply_options`](super::RunConfig::apply_options). All fields are optional;
/// only set fields override the base config (from env).
#[derive(Clone, Debug, Default)]
pub struct RunOptions {
    /// Override sampling temperature (0–2).
    pub temperature: Option<f32>,
    /// Override tool choice mode (auto, none, required).
    pub tool_choice: Option<ToolChoiceMode>,
    /// Thread ID for short-term memory (checkpointer). When set with `user_id`, enables both.
    pub thread_id: Option<String>,
    /// User ID for long-term memory (store). When set with `thread_id`, enables both.
    pub user_id: Option<String>,
    /// Override SQLite database path for persistence.
    pub db_path: Option<String>,
    /// Enable Exa MCP; when true, uses `exa_api_key` or env `EXA_API_KEY` if key not set.
    pub mcp_exa: bool,
    /// Exa API key (overrides env when set).
    pub exa_api_key: Option<String>,
    /// Exa MCP server URL override.
    pub mcp_exa_url: Option<String>,
}

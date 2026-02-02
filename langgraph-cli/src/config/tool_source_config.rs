//! Tool source configuration. Exa MCP is enabled only when `exa_api_key` is set; otherwise off by default.
//!
//! Used by [`RunConfig`](super::RunConfig) and by run logic that builds [`ToolSource`](langgraph::ToolSource).

/// Tool source configuration. Only Exa MCP is configurable via API key; when key is None, Exa is off.
#[derive(Clone, Debug, Default)]
pub struct ToolSourceConfig {
    /// Exa API key. When set, Exa MCP is enabled; when None, Exa is off by default.
    pub exa_api_key: Option<String>,
}

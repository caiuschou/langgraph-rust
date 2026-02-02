//! Configuration for building a ReAct run context (checkpointer, store, runnable_config, tool_source).
//!
//! Used by [`build_react_run_context`](super::build::build_react_run_context). CLI or other
//! callers build this from their own config (e.g. env, CLI args) and pass it to the builder.

/// Configuration for building ReAct run context. Holds only persistence and tool-source fields.
///
/// Callers (e.g. langgraph-cli) build this from `RunConfig` or env; langgraph uses it to build
/// checkpointer, store, runnable_config and tool_source.
#[derive(Clone, Debug)]
pub struct ReactBuildConfig {
    /// SQLite database path. Defaults to "memory.db" when None at build time.
    pub db_path: Option<String>,
    /// Thread ID for short-term memory (checkpointer). When set, checkpointer is created.
    pub thread_id: Option<String>,
    /// User ID for long-term memory (store). When set, store is created.
    pub user_id: Option<String>,
    /// Exa API key. When set, Exa MCP is enabled; when None, Exa is off.
    pub exa_api_key: Option<String>,
    /// Exa MCP server URL.
    pub mcp_exa_url: String,
    /// Command for mcp-remote (stdio→HTTP bridge).
    pub mcp_remote_cmd: String,
    /// Args for mcp-remote, e.g. "-y mcp-remote".
    pub mcp_remote_args: String,
}

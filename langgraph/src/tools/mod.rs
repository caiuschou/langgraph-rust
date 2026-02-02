mod aggregate_source;
mod conversation;
#[cfg(feature = "mcp")]
mod mcp_adapter;
pub mod memory;
mod registry;
mod r#trait;

pub use aggregate_source::AggregateToolSource;
pub use conversation::{GetRecentMessagesTool, TOOL_GET_RECENT_MESSAGES};
pub use memory::{
    ListMemoriesTool, RecallTool, RememberTool, SearchMemoriesTool, TOOL_LIST_MEMORIES,
    TOOL_RECALL, TOOL_REMEMBER, TOOL_SEARCH_MEMORIES,
};
pub use r#trait::Tool;
pub use registry::{ToolRegistry, ToolRegistryLocked};

#[cfg(feature = "mcp")]
pub use mcp_adapter::{register_mcp_tools, McpToolAdapter};

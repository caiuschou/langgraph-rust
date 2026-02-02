mod aggregate_source;
mod conversation;
pub mod memory;
#[cfg(feature = "mcp")]
mod mcp_adapter;
mod registry;
mod r#trait;

pub use aggregate_source::AggregateToolSource;
pub use conversation::{GetRecentMessagesTool, TOOL_GET_RECENT_MESSAGES};
pub use memory::{
    ListMemoriesTool, RecallTool, RememberTool, SearchMemoriesTool,
    TOOL_LIST_MEMORIES, TOOL_RECALL, TOOL_REMEMBER, TOOL_SEARCH_MEMORIES,
};
pub use registry::{ToolRegistry, ToolRegistryLocked};
pub use r#trait::Tool;

#[cfg(feature = "mcp")]
pub use mcp_adapter::{register_mcp_tools, McpToolAdapter};

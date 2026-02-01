//! Composite tool source: long-term (Store) + short-term (get_recent_messages) in one.
//!
//! Merges `list_tools` from both; dispatches `call_tool` by name; forwards
//! `set_call_context` to the short-term source. See `idea/memory-tools-design.md` §7.5.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;

use crate::memory::{Namespace, Store};
use crate::tool_source::{
    ShortTermMemoryToolSource, StoreToolSource, ToolCallContent, ToolCallContext, ToolSource,
    ToolSourceError, ToolSpec, TOOL_GET_RECENT_MESSAGES,
};

/// Composite tool source that exposes both long-term (Store) and short-term (recent messages) memory tools.
///
/// Holds `StoreToolSource` and `ShortTermMemoryToolSource`; `list_tools` returns all 5 tools;
/// `call_tool` dispatches by name; `set_call_context` is forwarded only to the short-term source
/// so that `get_recent_messages` receives current-step messages from ActNode.
///
/// **Interaction**: Use with `ActNode::new(Box::new(MemoryToolsSource::new(store, namespace)))`
/// when you want both remember/recall/search_memories/list_memories and get_recent_messages.
pub struct MemoryToolsSource {
    store_tools: StoreToolSource,
    short_term: ShortTermMemoryToolSource,
}

impl MemoryToolsSource {
    /// Creates a composite with both long-term (store + namespace) and short-term memory tools.
    pub fn new(store: Arc<dyn Store>, namespace: Namespace) -> Self {
        Self {
            store_tools: StoreToolSource::new(store, namespace),
            short_term: ShortTermMemoryToolSource::new(),
        }
    }

    /// Returns which tool source owns the given tool name (store vs short-term).
    fn which(&self, name: &str) -> bool {
        name == TOOL_GET_RECENT_MESSAGES
    }
}

#[async_trait]
impl ToolSource for MemoryToolsSource {
    async fn list_tools(&self) -> Result<Vec<ToolSpec>, ToolSourceError> {
        let mut tools = self.store_tools.list_tools().await?;
        let short = self.short_term.list_tools().await?;
        tools.extend(short);
        Ok(tools)
    }

    async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolCallContent, ToolSourceError> {
        if self.which(name) {
            self.short_term.call_tool(name, arguments).await
        } else {
            self.store_tools.call_tool(name, arguments).await
        }
    }

    async fn call_tool_with_context(
        &self,
        name: &str,
        arguments: Value,
        ctx: Option<&ToolCallContext>,
    ) -> Result<ToolCallContent, ToolSourceError> {
        if self.which(name) {
            self.short_term.call_tool_with_context(name, arguments, ctx).await
        } else {
            self.store_tools.call_tool_with_context(name, arguments, ctx).await
        }
    }

    fn set_call_context(&self, ctx: Option<ToolCallContext>) {
        self.short_term.set_call_context(ctx);
    }
}

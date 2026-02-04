//! Think node: read messages, call LLM, write assistant message and optional tool_calls.
//!
//! Design: docs/rust-langgraph/13-react-agent-design.md §8.3 stage 3.1–3.2.
//! ThinkNode holds an LLM client (e.g. MockLlm or `Box<dyn LlmClient>`), implements
//! `Node<ReActState>`; run reads state.messages, calls LLM, appends one assistant message
//! and sets state.tool_calls from the response (empty when no tools).
//!
//! # Streaming Support
//!
//! ThinkNode implements `run_with_context` to support Messages streaming. When
//! `stream_mode` contains `StreamMode::Messages`, it uses `LlmClient::invoke_stream()`
//! and forwards `MessageChunk` tokens to the stream channel as `StreamEvent::Messages`.

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::AgentError;
use crate::graph::{Next, RunContext};
use crate::llm::LlmClient;
use crate::message::Message;
use crate::state::ReActState;
use crate::stream::{MessageChunk, StreamEvent, StreamMetadata, StreamMode};
use crate::Node;

/// Think node: one ReAct step that produces assistant message and optional tool_calls.
///
/// Reads `state.messages`, calls the LLM, appends one assistant message and sets
/// `state.tool_calls` from the response. When the LLM returns no tool_calls, the
/// graph can end after observe. Does not call ToolSource::list_tools in this minimal
/// version (prompt can be fixed).
///
/// **Interaction**: Implements `Node<ReActState>`; used by StateGraph. Consumes
/// `LlmClient` (e.g. MockLlm); writes to ReActState.messages and ReActState.tool_calls.
pub struct ThinkNode {
    /// LLM client used to produce assistant message and optional tool_calls.
    llm: Box<dyn LlmClient>,
}

impl ThinkNode {
    /// Creates a Think node with the given LLM client.
    pub fn new(llm: Box<dyn LlmClient>) -> Self {
        Self { llm }
    }
}

#[async_trait]
impl Node<ReActState> for ThinkNode {
    fn id(&self) -> &str {
        "think"
    }

    /// Reads state.messages, calls LLM, appends assistant message and sets tool_calls.
    /// Returns Next::Continue to follow linear edge order (e.g. think → act).
    async fn run(&self, state: ReActState) -> Result<(ReActState, Next), AgentError> {
        let response = self.llm.invoke(&state.messages).await?;
        let mut messages = state.messages;
        messages.push(Message::Assistant(response.content));
        let new_state = ReActState {
            messages,
            tool_calls: response.tool_calls,
            tool_results: state.tool_results,
            turn_count: state.turn_count,
        };
        Ok((new_state, Next::Continue))
    }

    /// Streaming-aware variant: when `stream_mode` contains `Messages`, uses
    /// `invoke_stream()` and forwards chunks to the stream channel.
    ///
    /// Token chunks are sent as `StreamEvent::Messages` with metadata containing
    /// the node id ("think"). This enables real-time LLM output display (typewriter effect).
    async fn run_with_context(
        &self,
        state: ReActState,
        ctx: &RunContext<ReActState>,
    ) -> Result<(ReActState, Next), AgentError> {
        let should_stream =
            ctx.stream_mode.contains(&StreamMode::Messages) && ctx.stream_tx.is_some();

        let response = if should_stream {
            // Create internal channel for message chunks
            let (chunk_tx, mut chunk_rx) = mpsc::channel::<MessageChunk>(128);

            // Get a clone of the stream sender for the forwarding task
            let stream_tx = ctx.stream_tx.clone().unwrap();
            let node_id = self.id().to_string();

            // Spawn task to forward chunks as StreamEvent::Messages
            let forward_task = tokio::spawn(async move {
                while let Some(chunk) = chunk_rx.recv().await {
                    let event = StreamEvent::Messages {
                        chunk,
                        metadata: StreamMetadata {
                            langgraph_node: node_id.clone(),
                        },
                    };
                    // Ignore send errors (consumer may have dropped)
                    let _ = stream_tx.send(event).await;
                }
            });

            // Call LLM with streaming
            let result = self
                .llm
                .invoke_stream(&state.messages, Some(chunk_tx))
                .await;

            // Wait for forwarding task to complete (chunk_tx is dropped after invoke_stream)
            let _ = forward_task.await;

            result?
        } else {
            // Non-streaming path: use regular invoke
            self.llm.invoke(&state.messages).await?
        };

        let mut messages = state.messages;
        messages.push(Message::Assistant(response.content));
        let new_state = ReActState {
            messages,
            tool_calls: response.tool_calls,
            tool_results: state.tool_results,
            turn_count: state.turn_count,
        };
        Ok((new_state, Next::Continue))
    }
}

//! Integration test: parse request → run React stream with mock → collect SSE lines.
//!
//! **Scenario**: Full flow without real HTTP: ChatCompletionRequest → parse_chat_request →
//! ReactRunner (MockLlm + MockToolSource) → stream_with_config with StreamToSse sink →
//! assert we get initial chunk, content delta, and final stop chunk.

use langgraph::{
    parse_chat_request, ChatCompletionRequest, ChatMessage, ChunkMeta, MessageContent, MockLlm,
    MockToolSource, ReactRunner, StreamToSse,
};
use tokio::sync::mpsc;

#[tokio::test]
async fn stream_flow_produces_openai_sse_lines() {
    let req = ChatCompletionRequest {
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: Some(MessageContent::String("Hello".to_string())),
        }],
        model: "gpt-4o-mini".to_string(),
        stream: true,
        stream_options: None,
        thread_id: None,
    };
    let parsed = parse_chat_request(&req).expect("parse");

    let llm = MockLlm::with_no_tool_calls("Hi there.");
    let runner = ReactRunner::new(
        Box::new(llm),
        Box::new(MockToolSource::get_time_example()),
        None,
        None,
        None,
        None,
        false,
    )
    .expect("compile");

    let (tx, mut rx) = mpsc::channel::<String>(64);
    let meta = ChunkMeta {
        id: "chatcmpl-test".to_string(),
        model: req.model.clone(),
        created: None,
    };
    let mut adapter = StreamToSse::new_with_sink(meta, false, tx);

    let _ = runner
        .stream_with_config(&parsed.user_message, Some(parsed.runnable_config), Some(|ev| {
            adapter.feed(ev);
        }))
        .await
        .expect("stream");
    adapter.finish();
    drop(adapter);

    let mut lines = Vec::new();
    while let Some(s) = rx.recv().await {
        lines.push(s);
    }

    assert!(!lines.is_empty(), "at least one SSE line");
    assert!(
        lines[0].contains(r#""role":"assistant""#),
        "first chunk has role"
    );
    let has_content = lines.iter().any(|s| s.contains("Hi"));
    assert!(has_content, "some chunk has assistant content");
    let has_stop = lines.iter().any(|s| s.contains(r#""finish_reason":"stop""#));
    assert!(has_stop, "final chunk has finish_reason stop");
}

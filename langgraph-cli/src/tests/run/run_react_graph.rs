//! Unit tests for [`run_react_graph`](langgraph::run_react_graph).
//!
//! BDD-style scenarios: no checkpointer/store path, with store path, single round with tool call,
//! empty user message, multi-turn from checkpoint. Each test documents Given/When/Then and the
//! behaviour under test.

use std::sync::Arc;

use langgraph::{
    run_react_graph, Checkpoint, CheckpointSource, Checkpointer, Message, MemorySaver, MockLlm,
    MockToolSource, ReActState, ToolSource, REACT_SYSTEM_PROMPT,
};

/// **Scenario**: When no checkpointer, no store, and no runnable_config, run_react_graph compiles
/// without checkpointer and returns Ok with state containing system prompt, user message, and
/// assistant reply.
///
/// Given: MockLlm that returns no tool_calls, MockToolSource, and None for checkpointer, store,
/// runnable_config  
/// When: run_react_graph("hi", llm, tool_source, None, None, None) is called  
/// Then: result is Ok; state.messages has at least system + user + assistant; user message is "hi";
/// assistant message matches the mock content.
#[tokio::test]
async fn run_react_graph_without_checkpointer_or_store_returns_ok_and_state_has_messages() {
    let llm = MockLlm::with_no_tool_calls("Hello from mock.");
    let tool_source = MockToolSource::get_time_example();
    let llm: Box<dyn langgraph::LlmClient> = Box::new(llm);
    let tool_source: Box<dyn ToolSource> = Box::new(tool_source);

    let result = run_react_graph("hi", llm, tool_source, None, None, None, false).await;

    let state = result.expect("run_react_graph with mock should succeed");
    assert!(
        state.messages.len() >= 2,
        "expected at least system + user + assistant: {}",
        state.messages.len()
    );
    let has_user = state
        .messages
        .iter()
        .any(|m| matches!(m, Message::User(s) if s == "hi"));
    assert!(has_user, "state should contain user message 'hi'");
    let has_assistant = state
        .messages
        .iter()
        .any(|m| matches!(m, Message::Assistant(s) if s == "Hello from mock."));
    assert!(
        has_assistant,
        "state should contain assistant message from mock"
    );
}

/// **Scenario**: When user_message is empty string, run_react_graph still returns Ok and state
/// contains the empty user message.
///
/// Given: MockLlm with no tool_calls, MockToolSource, no checkpointer/store/config  
/// When: run_react_graph("", ...) is called  
/// Then: result is Ok; state.messages contains Message::User("").
#[tokio::test]
async fn run_react_graph_with_empty_user_message_returns_ok_and_state_has_empty_user() {
    let llm = MockLlm::with_no_tool_calls("Acknowledged.");
    let tool_source = MockToolSource::get_time_example();
    let llm: Box<dyn langgraph::LlmClient> = Box::new(llm);
    let tool_source: Box<dyn ToolSource> = Box::new(tool_source);

    let result = run_react_graph("", llm, tool_source, None, None, None, false).await;

    let state = result.expect("run_react_graph should succeed");
    let has_empty_user = state
        .messages
        .iter()
        .any(|m| matches!(m, Message::User(s) if s.is_empty()));
    assert!(has_empty_user, "state should contain empty user message");
}

/// **Scenario**: When store is Some(InMemoryStore) and checkpointer is None, run_react_graph
/// compiles the graph with store and returns Ok (tests the with_store code path).
///
/// Given: MockLlm, MockToolSource, None checkpointer, Some(Arc&lt;InMemoryStore&gt;), None config  
/// When: run_react_graph("hello", ...) is called  
/// Then: result is Ok; state contains user message "hello" and assistant reply.
#[tokio::test]
async fn run_react_graph_with_store_and_no_checkpointer_returns_ok() {
    let llm = MockLlm::with_no_tool_calls("Reply with store.");
    let tool_source = MockToolSource::get_time_example();
    let store = Arc::new(langgraph::InMemoryStore::new());
    let llm: Box<dyn langgraph::LlmClient> = Box::new(llm);
    let tool_source: Box<dyn ToolSource> = Box::new(tool_source);

    let result = run_react_graph(
        "hello",
        llm,
        tool_source,
        None,
        Some(store as Arc<dyn langgraph::Store>),
        None,
        false,
    )
    .await;

    let state = result.expect("run_react_graph with store should succeed");
    let has_user = state
        .messages
        .iter()
        .any(|m| matches!(m, Message::User(s) if s == "hello"));
    assert!(has_user, "state should contain user message 'hello'");
}

/// **Scenario**: When MockLlm returns one tool_call (get_time) and MockToolSource handles it,
/// run_react_graph runs one round (think → act → observe → END) and returns Ok with tool result
/// merged into messages.
///
/// Given: MockLlm::with_get_time_call(), MockToolSource::get_time_example(), no checkpointer/store  
/// When: run_react_graph("What time is it?", ...) is called  
/// Then: result is Ok; state.messages has at least 3 entries; at least one User message contains
/// tool result (e.g. "Tool" and a date); tool_calls and tool_results are cleared.
#[tokio::test]
async fn run_react_graph_one_round_with_tool_call_returns_ok_and_tool_result_in_messages() {
    let llm = MockLlm::with_get_time_call();
    let tool_source = MockToolSource::get_time_example();
    let llm: Box<dyn langgraph::LlmClient> = Box::new(llm);
    let tool_source: Box<dyn ToolSource> = Box::new(tool_source);

    let result = run_react_graph("What time is it?", llm, tool_source, None, None, None, false).await;

    let state = result.expect("run_react_graph with tool call should succeed");
    assert!(
        state.messages.len() >= 3,
        "expected at least user + assistant + tool result user: {}",
        state.messages.len()
    );
    let has_tool_result = state
        .messages
        .iter()
        .any(|m| matches!(m, Message::User(s) if s.contains("Tool") && s.contains("2025")));
    assert!(
        has_tool_result,
        "state should contain a User message with tool result (Tool + date)"
    );
    assert!(state.tool_calls.is_empty(), "tool_calls should be cleared");
    assert!(
        state.tool_results.is_empty(),
        "tool_results should be cleared"
    );
}

/// **Scenario**: Initial state built inside run_react_graph starts with REACT_SYSTEM_PROMPT and
/// the given user message; after one round with no tool_calls the first message is still system.
///
/// Given: MockLlm with no tool_calls  
/// When: run_react_graph("hi", ...) is called  
/// Then: state.messages[0] is Message::System with content equal to REACT_SYSTEM_PROMPT.
#[tokio::test]
async fn run_react_graph_state_starts_with_system_prompt() {
    let llm = MockLlm::with_no_tool_calls("Hi back.");
    let tool_source = MockToolSource::get_time_example();
    let llm: Box<dyn langgraph::LlmClient> = Box::new(llm);
    let tool_source: Box<dyn ToolSource> = Box::new(tool_source);

    let result = run_react_graph("hi", llm, tool_source, None, None, None, false).await;

    let state = result.expect("run_react_graph should succeed");
    let first = state
        .messages
        .first()
        .expect("state should have at least one message");
    match first {
        Message::System(s) => assert_eq!(s, REACT_SYSTEM_PROMPT),
        _ => panic!("first message should be System, got {:?}", first),
    }
}

/// **Scenario**: When checkpointer and runnable_config with thread_id are set and the checkpointer
/// has a previous checkpoint for that thread, run_react_graph loads that state, appends the new
/// user message, and runs one round; the final state contains the history plus the new turn.
///
/// Given: MemorySaver with a checkpoint for thread "t1" containing messages [system, user("first"),
/// assistant("Reply to first")]; MockLlm that returns no tool_calls with "Reply to second"
/// When: run_react_graph("second", llm, tool_source, Some(checkpointer), None, Some(config with thread_id "t1")) is called
/// Then: result is Ok; state.messages contains Message::User("first"), Message::Assistant("Reply to first"),
/// Message::User("second"), and Message::Assistant("Reply to second").
#[tokio::test]
async fn run_react_graph_with_checkpoint_loads_history_and_appends_new_turn() {
    let history_state = ReActState {
        messages: vec![
            Message::system(REACT_SYSTEM_PROMPT),
            Message::user("first".to_string()),
            Message::Assistant("Reply to first".to_string()),
        ],
        tool_calls: vec![],
        tool_results: vec![],
        turn_count: 0,
    };
    let checkpoint = Checkpoint::from_state(history_state, CheckpointSource::Update, 0);
    let saver: MemorySaver<ReActState> = MemorySaver::new();
    let config = langgraph::RunnableConfig {
        thread_id: Some("t1".into()),
        checkpoint_id: None,
        checkpoint_ns: String::new(),
        user_id: None,
    };
    saver.put(&config, &checkpoint).await.unwrap();

    let llm = MockLlm::with_no_tool_calls("Reply to second");
    let tool_source = MockToolSource::get_time_example();
    let llm: Box<dyn langgraph::LlmClient> = Box::new(llm);
    let tool_source: Box<dyn ToolSource> = Box::new(tool_source);
    let cp: Arc<dyn Checkpointer<ReActState>> = Arc::new(saver);

    let result = run_react_graph(
        "second",
        llm,
        tool_source,
        Some(cp),
        None,
        Some(config),
        false,
    )
    .await;

    let state = result.expect("run_react_graph with checkpoint should succeed");
    let has_first_user = state
        .messages
        .iter()
        .any(|m| matches!(m, Message::User(s) if s == "first"));
    assert!(has_first_user, "state should contain history user message 'first'");
    let has_first_assistant = state
        .messages
        .iter()
        .any(|m| matches!(m, Message::Assistant(s) if s == "Reply to first"));
    assert!(
        has_first_assistant,
        "state should contain history assistant message 'Reply to first'"
    );
    let has_second_user = state
        .messages
        .iter()
        .any(|m| matches!(m, Message::User(s) if s == "second"));
    assert!(has_second_user, "state should contain new user message 'second'");
    let has_second_assistant = state
        .messages
        .iter()
        .any(|m| matches!(m, Message::Assistant(s) if s == "Reply to second"));
    assert!(
        has_second_assistant,
        "state should contain new assistant message 'Reply to second'"
    );
}

//! Unit tests for ReAct nodes: ThinkNode, ActNode, ObserveNode.
//!
//! Design: [docs/rust-langgraph/13-react-agent-design.md](https://github.com/.../13-react-agent-design.md) §8.3 stage 3.7.
//! Each node is fed ReActState and we assert output state shape and content;
//! uses MockLlm and MockToolSource.

use langgraph::{
    ActNode, Message, MockLlm, MockToolSource, Next, Node, ObserveNode, ReActState, ThinkNode,
    ToolCall, ToolResult,
};

// --- ThinkNode ---

#[tokio::test]
async fn think_node_id_is_think() {
    let llm = MockLlm::with_get_time_call();
    let node = ThinkNode::new(Box::new(llm));
    assert_eq!(node.id(), "think");
}

#[tokio::test]
async fn think_node_appends_assistant_message_and_sets_tool_calls() {
    let llm = MockLlm::with_get_time_call();
    let node = ThinkNode::new(Box::new(llm));
    let state = ReActState {
        messages: vec![Message::user("What time is it?")],
        tool_calls: vec![],
        tool_results: vec![],
    };
    let (out, _) = node.run(state).await.unwrap();
    assert_eq!(out.messages.len(), 2);
    assert!(matches!(&out.messages[1], Message::Assistant(s) if s == "I'll check the time."));
    assert_eq!(out.tool_calls.len(), 1);
    assert_eq!(out.tool_calls[0].name, "get_time");
    assert_eq!(out.tool_calls[0].arguments, "{}");
    assert_eq!(out.tool_results.len(), 0);
}

#[tokio::test]
async fn think_node_with_no_tool_calls_sets_empty_tool_calls() {
    let llm = MockLlm::with_no_tool_calls("Hello.");
    let node = ThinkNode::new(Box::new(llm));
    let state = ReActState {
        messages: vec![Message::user("Hi")],
        tool_calls: vec![],
        tool_results: vec![],
    };
    let (out, _) = node.run(state).await.unwrap();
    assert_eq!(out.messages.len(), 2);
    assert!(matches!(&out.messages[1], Message::Assistant(s) if s == "Hello."));
    assert!(out.tool_calls.is_empty());
    assert!(out.tool_results.is_empty());
}

#[tokio::test]
async fn think_node_preserves_tool_results_from_input_state() {
    let llm = MockLlm::with_no_tool_calls("Done.");
    let node = ThinkNode::new(Box::new(llm));
    let state = ReActState {
        messages: vec![Message::user("Hi")],
        tool_calls: vec![],
        tool_results: vec![ToolResult {
            call_id: Some("c1".into()),
            name: Some("get_time".into()),
            content: "12:00".into(),
        }],
    };
    let (out, _) = node.run(state).await.unwrap();
    assert_eq!(out.tool_results.len(), 1);
    assert_eq!(out.tool_results[0].content, "12:00");
}

// --- ActNode ---

#[tokio::test]
async fn act_node_id_is_act() {
    let tools = MockToolSource::get_time_example();
    let node = ActNode::new(Box::new(tools));
    assert_eq!(node.id(), "act");
}

#[tokio::test]
async fn act_node_executes_tool_calls_and_writes_tool_results() {
    let tools = MockToolSource::get_time_example();
    let node = ActNode::new(Box::new(tools));
    let state = ReActState {
        messages: vec![Message::user("What time?")],
        tool_calls: vec![ToolCall {
            name: "get_time".into(),
            arguments: "{}".into(),
            id: Some("call-1".into()),
        }],
        tool_results: vec![],
    };
    let (out, _) = node.run(state).await.unwrap();
    assert_eq!(out.messages.len(), 1);
    assert_eq!(out.tool_calls.len(), 1);
    assert_eq!(out.tool_results.len(), 1);
    assert_eq!(out.tool_results[0].call_id.as_deref(), Some("call-1"));
    assert_eq!(out.tool_results[0].name.as_deref(), Some("get_time"));
    assert_eq!(out.tool_results[0].content, "2025-01-29 12:00:00");
}

#[tokio::test]
async fn act_node_empty_tool_calls_leaves_tool_results_empty() {
    let tools = MockToolSource::get_time_example();
    let node = ActNode::new(Box::new(tools));
    let state = ReActState {
        messages: vec![Message::user("Hi")],
        tool_calls: vec![],
        tool_results: vec![],
    };
    let (out, _) = node.run(state).await.unwrap();
    assert!(out.tool_results.is_empty());
    assert!(out.tool_calls.is_empty());
}

#[tokio::test]
async fn act_node_multiple_tool_calls_produces_multiple_results() {
    let tools = MockToolSource::get_time_example();
    let node = ActNode::new(Box::new(tools));
    let state = ReActState {
        messages: vec![],
        tool_calls: vec![
            ToolCall {
                name: "get_time".into(),
                arguments: "{}".into(),
                id: Some("c1".into()),
            },
            ToolCall {
                name: "get_time".into(),
                arguments: r#"{}"#.into(),
                id: Some("c2".into()),
            },
        ],
        tool_results: vec![],
    };
    let (out, _) = node.run(state).await.unwrap();
    assert_eq!(out.tool_results.len(), 2);
    assert_eq!(out.tool_results[0].content, "2025-01-29 12:00:00");
    assert_eq!(out.tool_results[1].content, "2025-01-29 12:00:00");
}

// --- ObserveNode ---

#[tokio::test]
async fn observe_node_id_is_observe() {
    let node = ObserveNode::new();
    assert_eq!(node.id(), "observe");
}

#[tokio::test]
async fn observe_node_appends_tool_results_as_user_messages_and_clears_tool_fields() {
    let node = ObserveNode::new();
    let state = ReActState {
        messages: vec![
            Message::user("What time?"),
            Message::Assistant("I'll check.".into()),
        ],
        tool_calls: vec![ToolCall {
            name: "get_time".into(),
            arguments: "{}".into(),
            id: Some("call-1".into()),
        }],
        tool_results: vec![ToolResult {
            call_id: Some("call-1".into()),
            name: Some("get_time".into()),
            content: "2025-01-29 12:00:00".into(),
        }],
    };
    let (out, _) = node.run(state).await.unwrap();
    assert_eq!(out.messages.len(), 3);
    assert!(matches!(&out.messages[2], Message::User(s) if s.contains("Tool") && s.contains("2025-01-29 12:00:00")));
    assert!(out.tool_calls.is_empty());
    assert!(out.tool_results.is_empty());
}

#[tokio::test]
async fn observe_node_empty_tool_results_clears_tool_fields_only() {
    let node = ObserveNode::new();
    let state = ReActState {
        messages: vec![Message::user("Hi"), Message::Assistant("Hello.".into())],
        tool_calls: vec![ToolCall {
            name: "x".into(),
            arguments: "{}".into(),
            id: None,
        }],
        tool_results: vec![],
    };
    let (out, _) = node.run(state).await.unwrap();
    assert_eq!(out.messages.len(), 2);
    assert!(out.tool_calls.is_empty());
    assert!(out.tool_results.is_empty());
}

#[tokio::test]
async fn observe_node_default_constructible() {
    let node = ObserveNode::default();
    assert_eq!(node.id(), "observe");
}

#[tokio::test]
async fn observe_node_with_loop_returns_node_think_when_had_tool_calls() {
    let node = ObserveNode::with_loop();
    let state = ReActState {
        messages: vec![Message::user("Hi"), Message::Assistant("I'll check.".into())],
        tool_calls: vec![ToolCall {
            name: "get_time".into(),
            arguments: "{}".into(),
            id: Some("c1".into()),
        }],
        tool_results: vec![ToolResult {
            call_id: Some("c1".into()),
            name: Some("get_time".into()),
            content: "12:00".into(),
        }],
    };
    let (out, next) = node.run(state).await.unwrap();
    assert_eq!(out.messages.len(), 3);
    assert!(matches!(next, Next::Node(id) if id == "think"));
}

#[tokio::test]
async fn observe_node_with_loop_returns_end_when_no_tool_calls() {
    let node = ObserveNode::with_loop();
    let state = ReActState {
        messages: vec![Message::user("Hi"), Message::Assistant("Hello.".into())],
        tool_calls: vec![],
        tool_results: vec![],
    };
    let (out, next) = node.run(state).await.unwrap();
    assert_eq!(out.messages.len(), 2);
    assert!(matches!(next, Next::End));
}

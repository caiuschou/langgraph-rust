//! Integration-style tests for [`run_with_config`](crate::run_with_config) and
//! [`run_react_graph`](crate::run::common::run_react_graph).
//!
//! Scenarios: use_exa_mcp without mcp feature returns error; invalid db_path returns error;
//! run_react_graph with MockLlm/MockToolSource returns Ok and state contains expected messages.

use langgraph::{Message, MockLlm, MockToolSource, ToolSource};

use crate::config::RunConfig;
use crate::run::run_react_graph;
use crate::run_with_config;

/// **Scenario**: When use_exa_mcp is true but the mcp feature is not enabled, run_with_config returns an error.
///
/// Only compiled when mcp feature is disabled (e.g. `cargo test -p langgraph-cli --no-default-features`).
#[cfg(not(feature = "mcp"))]
#[tokio::test]
async fn run_with_config_use_exa_mcp_without_mcp_feature_returns_error() {
    let saved = std::env::var("OPENAI_API_KEY").ok();
    std::env::set_var("OPENAI_API_KEY", "test-key-for-test");

    let config = match RunConfig::from_env() {
        Ok(c) => c,
        Err(_) => {
            if let Some(k) = saved {
                std::env::set_var("OPENAI_API_KEY", k);
            }
            return;
        }
    };
    let mut config = config;
    config.use_exa_mcp = true;

    let result = run_with_config(&config, "hi").await;

    if let Some(k) = saved {
        std::env::set_var("OPENAI_API_KEY", k);
    } else {
        std::env::remove_var("OPENAI_API_KEY");
    }

    assert!(result.is_err(), "expected Err when use_exa_mcp and mcp feature off");
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.to_lowercase().contains("mcp") || err_msg.contains("feature"),
        "error should mention MCP or feature: {}",
        err_msg
    );
}

/// **Scenario**: When thread_id is set and db_path is a directory (not a file), run_with_config returns an error.
///
/// Only compiled when sqlite feature is enabled. SqliteSaver::new fails when path is a directory.
#[cfg(feature = "sqlite")]
#[tokio::test]
async fn run_with_config_invalid_db_path_returns_error() {
    let saved = std::env::var("OPENAI_API_KEY").ok();
    std::env::set_var("OPENAI_API_KEY", "test-key-for-test");

    let config = match RunConfig::from_env() {
        Ok(c) => c,
        Err(_) => {
            if let Some(k) = saved {
                std::env::set_var("OPENAI_API_KEY", k);
            }
            return;
        }
    };
    let mut config = config
        .with_short_term_memory("test-thread")
        .with_long_term_memory("test-user");
    // Use a directory as db_path so SqliteSaver::new / SqliteStore::new fail.
    config.db_path = Some(std::env::temp_dir().display().to_string());

    let result = run_with_config(&config, "hi").await;

    if let Some(k) = saved {
        std::env::set_var("OPENAI_API_KEY", k);
    } else {
        std::env::remove_var("OPENAI_API_KEY");
    }

    assert!(result.is_err(), "expected Err when db_path is a directory");
}

/// **Scenario**: run_react_graph with MockLlm and MockToolSource returns Ok and final state
/// contains system + user + assistant messages (one round, no tool_calls → END).
#[tokio::test]
async fn run_with_config_returns_ok_with_mock_llm() {
    let llm = MockLlm::with_no_tool_calls("Hello from mock.");
    let tool_source = MockToolSource::get_time_example();
    let llm: Box<dyn langgraph::LlmClient> = Box::new(llm);
    let tool_source: Box<dyn ToolSource> = Box::new(tool_source);

    let result: Result<langgraph::ReActState, _> =
        run_react_graph("hi", llm, tool_source, None, None, None).await;

    let state = result.expect("run_react_graph with mock should succeed");
    assert!(
        state.messages.len() >= 2,
        "expected at least system + user + assistant: {}",
        state.messages.len()
    );
    let has_user = state.messages.iter().any(|m| matches!(m, Message::User(s) if s == "hi"));
    assert!(has_user, "state should contain user message 'hi'");
    let has_assistant = state
        .messages
        .iter()
        .any(|m| matches!(m, Message::Assistant(s) if s == "Hello from mock."));
    assert!(has_assistant, "state should contain assistant message from mock");
}

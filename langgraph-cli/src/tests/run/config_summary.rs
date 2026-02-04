//! Integration tests for [`build_config_summary`](crate::build_config_summary).
//!
//! Asserts that for typical RunConfig scenarios (LLM-only, short-term only, both memories,
//! Exa on/off, embedding on/off), the summary sections have entries() matching the scheme format.

use std::collections::HashMap;

use crate::build_config_summary;
use crate::config::{MemoryConfig, RunConfig, ToolSourceConfig};

/// Builds a minimal RunConfig with the given overrides for summary tests.
/// Uses default api_base, model, api_key (non-empty so embedding_available when needed).
fn minimal_config(
    memory: MemoryConfig,
    db_path: Option<String>,
    exa_api_key: Option<String>,
    embedding_api_key: Option<String>,
) -> RunConfig {
    RunConfig {
        api_base: "https://api.openai.com/v1".to_string(),
        api_key: "test-key".to_string(),
        model: "gpt-4o-mini".to_string(),
        temperature: None,
        tool_choice: None,
        embedding_api_key,
        embedding_api_base: None,
        embedding_model: None,
        memory,
        db_path,
        tool_source: ToolSourceConfig {
            exa_api_key,
        },
        mcp_exa_url: "https://mcp.exa.ai/mcp".to_string(),
        mcp_remote_cmd: "npx".to_string(),
        mcp_remote_args: "-y mcp-remote".to_string(),
        stream: true,
        verbose: false,
    }
}

fn entries_map(entries: &[(&'static str, String)]) -> HashMap<&'static str, String> {
    entries.iter().cloned().collect()
}

/// **Scenario**: When config has no memory and no Exa, summary has four sections:
/// LLM, Memory (mode=none), Tools (tools=), Embedding.
#[test]
fn build_config_summary_no_memory_no_exa_has_four_sections_and_memory_mode_none() {
    let config = minimal_config(
        MemoryConfig::NoMemory,
        None,
        None,
        None,
    );
    let summary = build_config_summary(&config);
    let sections = summary.sections();
    assert_eq!(sections.len(), 4, "always four sections");
    assert_eq!(sections[0].section_name(), "LLM config");
    assert_eq!(sections[1].section_name(), "Memory config");
    assert_eq!(sections[2].section_name(), "Tools");
    assert_eq!(sections[3].section_name(), "Embedding");

    let mem_entries = entries_map(&sections[1].entries());
    assert_eq!(mem_entries.get("mode").map(|s| s.as_str()), Some("none"));

    let tools_entries = entries_map(&sections[2].entries());
    assert_eq!(tools_entries.get("tools").map(|s| s.as_str()), Some(""));
}

/// **Scenario**: When config has only short-term memory (thread_id), Memory section
/// has mode=short_term, short_term=sqlite, thread_id, db_path (effective memory.db when None).
#[test]
fn build_config_summary_short_term_only_memory_section_has_sqlite_and_db_path() {
    let config = minimal_config(
        MemoryConfig::ShortTerm {
            thread_id: "t1".to_string(),
        },
        None,
        None,
        None,
    );
    let summary = build_config_summary(&config);
    let sections = summary.sections();
    let mem = entries_map(&sections[1].entries());
    assert_eq!(mem.get("mode").map(|s| s.as_str()), Some("short_term"));
    assert_eq!(mem.get("short_term").map(|s| s.as_str()), Some("sqlite"));
    assert_eq!(mem.get("thread_id").map(|s| s.as_str()), Some("t1"));
    assert_eq!(mem.get("db_path").map(|s| s.as_str()), Some("memory.db"));
    assert!(mem.get("long_term").is_none());
    assert!(mem.get("store").is_none());
}

/// **Scenario**: When config has both short- and long-term memory and embedding key is set,
/// Memory section has mode=both, short_term=sqlite, long_term=vector, store=in_memory_vector.
#[test]
fn build_config_summary_both_memory_with_embedding_has_vector_store() {
    let config = minimal_config(
        MemoryConfig::Both {
            thread_id: "t1".to_string(),
            user_id: "u1".to_string(),
        },
        Some("memory.db".to_string()),
        None,
        Some("embed-key".to_string()),
    );
    let summary = build_config_summary(&config);
    let sections = summary.sections();
    let mem = entries_map(&sections[1].entries());
    assert_eq!(mem.get("mode").map(|s| s.as_str()), Some("both"));
    assert_eq!(mem.get("short_term").map(|s| s.as_str()), Some("sqlite"));
    assert_eq!(mem.get("long_term").map(|s| s.as_str()), Some("vector"));
    assert_eq!(mem.get("store").map(|s| s.as_str()), Some("in_memory_vector"));
}

/// **Scenario**: When config has long-term memory but no embedding key (api_key empty so
/// embedding_api_key() is empty), long_term=none and no store.
#[test]
fn build_config_summary_long_term_without_embedding_has_long_term_none() {
    let mut config = minimal_config(
        MemoryConfig::LongTerm {
            user_id: "u1".to_string(),
        },
        None,
        None,
        None,
    );
    config.api_key = String::new();
    config.embedding_api_key = None;
    let summary = build_config_summary(&config);
    let sections = summary.sections();
    let mem = entries_map(&sections[1].entries());
    assert_eq!(mem.get("mode").map(|s| s.as_str()), Some("long_term"));
    assert_eq!(mem.get("long_term").map(|s| s.as_str()), Some("none"));
    assert!(mem.get("store").is_none());
}

/// **Scenario**: When Exa is enabled (exa_api_key set), Tools section has exa and exa_url.
#[test]
fn build_config_summary_with_exa_tools_section_has_exa_and_exa_url() {
    let config = minimal_config(
        MemoryConfig::NoMemory,
        None,
        Some("exa-key".to_string()),
        None,
    );
    let summary = build_config_summary(&config);
    let sections = summary.sections();
    let tools = entries_map(&sections[2].entries());
    assert_eq!(tools.get("tools").map(|s| s.as_str()), Some("exa"));
    assert_eq!(
        tools.get("exa_url").map(|s| s.as_str()),
        Some("https://mcp.exa.ai/mcp")
    );
}

/// **Scenario**: When both memory (with embedding) and Exa are enabled, Tools section is tools=memory,exa.
#[test]
fn build_config_summary_both_memory_and_exa_tools_has_memory_exa() {
    let config = minimal_config(
        MemoryConfig::Both {
            thread_id: "t1".to_string(),
            user_id: "u1".to_string(),
        },
        None,
        Some("exa-key".to_string()),
        Some("emb-key".to_string()),
    );
    let summary = build_config_summary(&config);
    let sections = summary.sections();
    let tools = entries_map(&sections[2].entries());
    assert_eq!(tools.get("tools").map(|s| s.as_str()), Some("memory,exa"));
}

/// **Scenario**: LLM section always has model, api_base, temperature, tool_choice; temperature (default) when None.
#[test]
fn build_config_summary_llm_section_has_expected_keys_and_default_temperature() {
    let config = minimal_config(MemoryConfig::NoMemory, None, None, None);
    let summary = build_config_summary(&config);
    let llm = entries_map(&summary.sections()[0].entries());
    assert_eq!(llm.get("model").map(|s| s.as_str()), Some("gpt-4o-mini"));
    assert_eq!(llm.get("api_base").map(|s| s.as_str()), Some("https://api.openai.com/v1"));
    assert_eq!(llm.get("temperature").map(|s| s.as_str()), Some("(default)"));
    assert_eq!(llm.get("tool_choice").map(|s| s.as_str()), Some("auto"));
}

/// **Scenario**: Embedding section always has model and api_base (effective values).
#[test]
fn build_config_summary_embedding_section_has_model_and_api_base() {
    let config = minimal_config(MemoryConfig::NoMemory, None, None, None);
    let summary = build_config_summary(&config);
    let emb = entries_map(&summary.sections()[3].entries());
    assert_eq!(emb.get("model").map(|s| s.as_str()), Some("text-embedding-3-small"));
    assert_eq!(emb.get("api_base").map(|s| s.as_str()), Some("https://api.openai.com/v1"));
}

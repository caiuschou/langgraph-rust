//! Build run config summary from [`RunConfig`](crate::config::RunConfig) for verbose output.
//!
//! Used by [`run_with_config`](super::run_with_config) when `config.verbose` is true.
//! Memory section infers short_term/long_term/store from the same logic as
//! [`build_react_run_context`](langgraph::build_react_run_context).

use langgraph::{
    EmbeddingConfigSummary, LlmConfigSummary, MemoryConfigSummary, RunConfigSummary,
    ToolConfigSummary,
};

use crate::config::{MemoryConfig, RunConfig};

/// Builds a run config summary from the given config (LLM, memory, tools, embedding).
///
/// Memory short_term/long_term/store are inferred to match current
/// `build_react_run_context` behaviour: short_term=sqlite when thread_id is set;
/// long_term=vector and store=in_memory_vector when user_id is set and embedding
/// is available. db_path is shown as effective value (default "memory.db").
pub fn build_config_summary(config: &RunConfig) -> RunConfigSummary {
    let llm = LlmConfigSummary {
        model: config.model.clone(),
        api_base: config.api_base.clone(),
        temperature: config.temperature,
        tool_choice: config
            .tool_choice
            .as_ref()
            .map(|tc| format!("{:?}", tc).to_lowercase())
            .unwrap_or_else(|| "auto".to_string()),
    };

    let (mode, short_term, thread_id, db_path, long_term, long_term_store) =
        memory_summary_fields(config);
    let memory = MemoryConfigSummary {
        mode,
        short_term,
        thread_id,
        db_path,
        long_term,
        long_term_store,
    };

    let (sources, exa_url) = tool_summary_fields(config);
    let tools = ToolConfigSummary {
        sources,
        exa_url,
    };

    let embedding = EmbeddingConfigSummary {
        model: config.embedding_model().to_string(),
        api_base: config.embedding_api_base().to_string(),
    };

    RunConfigSummary::new()
        .with_section(Box::new(llm))
        .with_section(Box::new(memory))
        .with_section(Box::new(tools))
        .with_section(Box::new(embedding))
}

fn memory_summary_fields(
    config: &RunConfig,
) -> (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
) {
    let mode = match &config.memory {
        MemoryConfig::NoMemory => "none",
        MemoryConfig::ShortTerm { .. } => "short_term",
        MemoryConfig::LongTerm { .. } => "long_term",
        MemoryConfig::Both { .. } => "both",
    };
    let mode = mode.to_string();

    let thread_id = config.thread_id().map(ToString::to_string);
    let short_term = thread_id.as_ref().map(|_| "sqlite".to_string());
    let db_path = thread_id.as_ref().map(|_| {
        config
            .db_path
            .clone()
            .unwrap_or_else(|| "memory.db".to_string())
    });

    let has_long_term = config.user_id().is_some();
    let embedding_available = !config.embedding_api_key().is_empty();
    let (long_term, long_term_store) = if has_long_term && embedding_available {
        (
            Some("vector".to_string()),
            Some("in_memory_vector".to_string()),
        )
    } else if has_long_term {
        (Some("none".to_string()), None)
    } else {
        (None, None)
    };

    (mode, short_term, thread_id, db_path, long_term, long_term_store)
}

fn tool_summary_fields(config: &RunConfig) -> (Vec<String>, Option<String>) {
    let has_memory = config.user_id().is_some() && !config.embedding_api_key().is_empty();
    let has_exa = config.tool_source.exa_api_key.is_some();

    let mut sources = Vec::new();
    if has_memory {
        sources.push("memory".to_string());
    }
    if has_exa {
        sources.push("exa".to_string());
    }
    if sources.is_empty() {
        sources.push("".to_string()); // always show tools= for stable format
    }

    let exa_url = has_exa.then(|| config.mcp_exa_url.clone());

    (sources, exa_url)
}

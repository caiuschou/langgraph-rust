//! Integration-style tests for [`run_with_config`](crate::run_with_config).
//!
//! Scenarios: invalid db_path returns error. BDD-style with Given/When/Then in doc comments.

use crate::config::RunConfig;
use crate::run_with_config;

/// **Scenario**: When thread_id is set and db_path is a directory (not a file), run_with_config
/// returns an error (SqliteSaver::new / SqliteStore::new fail on directory path).
///
/// Given: RunConfig with short-term and long-term memory set, db_path set to temp directory  
/// When: run_with_config(&config, "hi") is called  
/// Then: result is Err.
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
    config.db_path = Some(std::env::temp_dir().display().to_string());

    let result = run_with_config(&config, "hi").await;

    if let Some(k) = saved {
        std::env::set_var("OPENAI_API_KEY", k);
    } else {
        std::env::remove_var("OPENAI_API_KEY");
    }

    assert!(result.is_err(), "expected Err when db_path is a directory");
}

//! Unit tests for [`RunConfig`](crate::config::RunConfig) and [`MemoryConfig`](crate::config::MemoryConfig).
//!
//! Scenarios: from_env with/without OPENAI_API_KEY, builder methods, accessors.
//! Tests that touch OPENAI_API_KEY use a static lock so they do not run in parallel
//! and overwrite each other's environment.

use std::sync::Mutex;

use crate::config::{MemoryConfig, RunConfig};

/// Lock used by tests that set/remove OPENAI_API_KEY so they run serially and do not race.
static ENV_API_KEY_LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();

fn env_api_key_lock() -> std::sync::MutexGuard<'static, ()> {
    ENV_API_KEY_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap()
}

/// **Scenario**: When OPENAI_API_KEY is not set, from_env returns an error.
///
/// Given: no OPENAI_API_KEY in the environment  
/// When: RunConfig::from_env() is called  
/// Then: result is Err and the error message mentions OPENAI_API_KEY
#[test]
fn from_env_fails_when_api_key_is_missing() {
    let _guard = env_api_key_lock();
    let saved = std::env::var("OPENAI_API_KEY").ok();
    std::env::remove_var("OPENAI_API_KEY");

    let result = RunConfig::from_env();

    if let Some(ref key) = saved {
        std::env::set_var("OPENAI_API_KEY", key);
    } else {
        std::env::remove_var("OPENAI_API_KEY");
    }

    assert!(result.is_err());
    let err_msg = format!("{:?}", result.unwrap_err());
    assert!(
        err_msg.contains("OPENAI_API_KEY") || err_msg.to_lowercase().contains("api_key"),
        "error should mention API key: {}",
        err_msg
    );
}

/// **Scenario**: When OPENAI_API_KEY is set, from_env returns Ok with default api_base and model.
///
/// Given: OPENAI_API_KEY is set to a non-empty value  
/// When: RunConfig::from_env() is called  
/// Then: result is Ok, api_base defaults to OpenAI URL, model defaults to gpt-4o-mini
#[test]
fn from_env_succeeds_with_defaults_when_api_key_is_set() {
    let _guard = env_api_key_lock();
    let saved_base = std::env::var("OPENAI_API_BASE").ok();
    let saved_model = std::env::var("OPENAI_MODEL").ok();
    let saved_key = std::env::var("OPENAI_API_KEY").ok();

    std::env::set_var("OPENAI_API_KEY", "test-key-for-unit-test");
    std::env::remove_var("OPENAI_API_BASE");
    std::env::remove_var("OPENAI_MODEL");

    let result = RunConfig::from_env();

    if let Some(ref k) = saved_key {
        std::env::set_var("OPENAI_API_KEY", k);
    } else {
        std::env::remove_var("OPENAI_API_KEY");
    }
    if let Some(ref b) = saved_base {
        std::env::set_var("OPENAI_API_BASE", b);
    } else {
        std::env::remove_var("OPENAI_API_BASE");
    }
    if let Some(ref m) = saved_model {
        std::env::set_var("OPENAI_MODEL", m);
    } else {
        std::env::remove_var("OPENAI_MODEL");
    }

    let config = result.expect("from_env should succeed when OPENAI_API_KEY is set");
    assert_eq!(config.api_base, "https://api.openai.com/v1");
    assert_eq!(config.model, "gpt-4o-mini");
    assert_eq!(config.api_key, "test-key-for-unit-test");
}

/// **Scenario**: with_short_term_memory sets memory to ShortTerm and thread_id() returns the id.
///
/// Given: a config obtained from from_env  
/// When: with_short_term_memory("thread-1") is called  
/// Then: memory is ShortTerm and thread_id() is Some("thread-1")
#[test]
fn builder_with_short_term_memory_sets_thread_id() {
    let _guard = env_api_key_lock();
    let saved_key = std::env::var("OPENAI_API_KEY").ok();
    std::env::set_var("OPENAI_API_KEY", "key");
    let config = RunConfig::from_env().expect("need key");
    if let Some(ref k) = saved_key {
        std::env::set_var("OPENAI_API_KEY", k);
    } else {
        std::env::remove_var("OPENAI_API_KEY");
    }

    let config = config.with_short_term_memory("thread-1");

    assert!(matches!(config.memory, MemoryConfig::ShortTerm { .. }));
    assert_eq!(config.thread_id(), Some("thread-1"));
    assert_eq!(config.user_id(), None);
}

/// **Scenario**: with_long_term_memory sets memory to LongTerm and user_id() returns the id.
///
/// Given: a config from from_env  
/// When: with_long_term_memory("user-1") is called  
/// Then: memory is LongTerm and user_id() is Some("user-1")
#[test]
fn builder_with_long_term_memory_sets_user_id() {
    let _guard = env_api_key_lock();
    let saved_key = std::env::var("OPENAI_API_KEY").ok();
    std::env::set_var("OPENAI_API_KEY", "key");
    let config = RunConfig::from_env().expect("need key");
    if let Some(ref k) = saved_key {
        std::env::set_var("OPENAI_API_KEY", k);
    } else {
        std::env::remove_var("OPENAI_API_KEY");
    }

    let config = config.with_long_term_memory("user-1");

    assert!(matches!(config.memory, MemoryConfig::LongTerm { .. }));
    assert_eq!(config.user_id(), Some("user-1"));
    assert_eq!(config.thread_id(), None);
}

/// **Scenario**: with_memory sets both thread_id and user_id.
///
/// Given: a config from from_env  
/// When: with_memory("thread-1", "user-1") is called  
/// Then: thread_id() is Some("thread-1") and user_id() is Some("user-1")
#[test]
fn builder_with_memory_sets_both_thread_and_user_id() {
    let _guard = env_api_key_lock();
    let saved_key = std::env::var("OPENAI_API_KEY").ok();
    std::env::set_var("OPENAI_API_KEY", "key");
    let config = RunConfig::from_env().expect("need key");
    if let Some(ref k) = saved_key {
        std::env::set_var("OPENAI_API_KEY", k);
    } else {
        std::env::remove_var("OPENAI_API_KEY");
    }

    let config = config.with_memory("thread-1", "user-1");

    assert!(matches!(config.memory, MemoryConfig::Both { .. }));
    assert_eq!(config.thread_id(), Some("thread-1"));
    assert_eq!(config.user_id(), Some("user-1"));
}

/// **Scenario**: without_memory resets memory to NoMemory.
///
/// Given: a config with short-term memory set  
/// When: without_memory() is called  
/// Then: memory is NoMemory and thread_id() and user_id() are None
#[test]
fn builder_without_memory_clears_memory() {
    let _guard = env_api_key_lock();
    let saved_key = std::env::var("OPENAI_API_KEY").ok();
    std::env::set_var("OPENAI_API_KEY", "key");
    let config = RunConfig::from_env()
        .expect("need key")
        .with_short_term_memory("t1");
    if let Some(ref k) = saved_key {
        std::env::set_var("OPENAI_API_KEY", k);
    } else {
        std::env::remove_var("OPENAI_API_KEY");
    }

    let config = config.without_memory();

    assert!(matches!(config.memory, MemoryConfig::NoMemory));
    assert_eq!(config.thread_id(), None);
    assert_eq!(config.user_id(), None);
}

/// **Scenario**: embedding_api_key() falls back to api_key when embedding_api_key is None.
///
/// Given: a config with api_key set and no explicit embedding_api_key  
/// When: embedding_api_key() is called  
/// Then: it returns the same value as api_key
#[test]
fn embedding_api_key_falls_back_to_api_key() {
    let _guard = env_api_key_lock();
    let saved_key = std::env::var("OPENAI_API_KEY").ok();
    std::env::set_var("OPENAI_API_KEY", "main-key");
    std::env::remove_var("EMBEDDING_API_KEY");
    let config = RunConfig::from_env().expect("need key");
    if let Some(ref k) = saved_key {
        std::env::set_var("OPENAI_API_KEY", k);
    } else {
        std::env::remove_var("OPENAI_API_KEY");
    }

    assert_eq!(config.embedding_api_key(), "main-key");
    assert_eq!(config.embedding_api_key(), config.api_key.as_str());
}

/// **Scenario**: MemoryConfig default is NoMemory.
///
/// Given: no explicit memory configuration  
/// When: MemoryConfig::default() is used  
/// Then: it is NoMemory
#[test]
fn memory_config_default_is_no_memory() {
    let memory = MemoryConfig::default();
    assert!(matches!(memory, MemoryConfig::NoMemory));
}

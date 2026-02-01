//! Memory configuration for enabling short-term and/or long-term memory.
//!
//! Used by [`RunConfig`](super::RunConfig) and interacts with checkpointer/store setup in run.

/// Memory configuration for enabling short-term and/or long-term memory.
#[derive(Clone, Debug, Default)]
pub enum MemoryConfig {
    #[default]
    NoMemory,
    ShortTerm {
        thread_id: String,
    },
    LongTerm {
        user_id: String,
    },
    Both {
        thread_id: String,
        user_id: String,
    },
}

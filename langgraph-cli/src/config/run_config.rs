//! Run config: API base, key, model, temperature, tool_choice. Can be filled from env / .env.
//!
//! Interacts with [`MemoryConfig`](super::MemoryConfig), [`run_with_config`](crate::run) and
//! langgraph's `ToolChoiceMode`, `OpenAIEmbedder`.

use super::{MemoryConfig, ToolSourceConfig};
use langgraph::ToolChoiceMode;

/// Error type used for config loading.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// Run config: API base, key, model, temperature, tool_choice. Can be filled from env / .env.
#[derive(Clone, Debug)]
pub struct RunConfig {
    /// OpenAI API base URL, e.g. `https://api.openai.com/v1`.
    pub api_base: String,
    /// OpenAI API key.
    pub api_key: String,
    /// Model name, e.g. `gpt-4o-mini`.
    pub model: String,
    /// Sampling temperature 0–2, lower is more deterministic. Default: unset (use API default).
    pub temperature: Option<f32>,
    /// Tool choice mode: auto (model chooses), none (no tools), required (must use tools).
    pub tool_choice: Option<ToolChoiceMode>,
    /// Embeddings API key. If not set, uses OPENAI_API_KEY.
    pub embedding_api_key: Option<String>,
    /// Embeddings API base URL. If not set, uses OPENAI_API_BASE.
    pub embedding_api_base: Option<String>,
    /// Embeddings model name, e.g. `text-embedding-3-small`.
    pub embedding_model: Option<String>,
    /// Memory configuration for short-term and/or long-term memory. Defaults to NoMemory when THREAD_ID/USER_ID not set.
    pub memory: MemoryConfig,
    /// SQLite database path for persistence. Defaults to "memory.db" when DB_PATH not set.
    pub db_path: Option<String>,
    /// Tool source configuration (e.g. Exa MCP). When exa_api_key is None, Exa is off by default.
    pub tool_source: ToolSourceConfig,
    /// Exa MCP server URL. Default: `https://mcp.exa.ai/mcp`.
    pub mcp_exa_url: String,
    /// Command for mcp-remote (stdio→HTTP bridge). Default: `npx`.
    pub mcp_remote_cmd: String,
    /// Args for mcp-remote, e.g. `-y mcp-remote`. Default: `-y mcp-remote`.
    pub mcp_remote_args: String,
    /// When true, run with streaming: show Thinking... / Calling tool / LLM tokens on stdout.
    pub stream: bool,
    /// When true, show debug logs (node enter/exit, graph execution). Requires --verbose.
    pub verbose: bool,
}

impl RunConfig {
    /// Apply optional overrides from `RunOptions` to this config.
    ///
    /// Only set fields in `options` override; memory is set when `thread_id` and/or
    /// `user_id` are present. Exa is enabled when `mcp_exa` is true and a key is
    /// available (from options or env).
    pub fn apply_options(&mut self, options: &super::RunOptions) {
        if let Some(t) = options.temperature {
            self.temperature = Some(t);
        }
        if let Some(tc) = options.tool_choice {
            self.tool_choice = Some(tc);
        }
        if options.thread_id.is_some() || options.user_id.is_some() {
            self.memory = match (&options.thread_id, &options.user_id) {
                (Some(tid), Some(uid)) => MemoryConfig::Both {
                    thread_id: tid.clone(),
                    user_id: uid.clone(),
                },
                (Some(tid), None) => MemoryConfig::ShortTerm {
                    thread_id: tid.clone(),
                },
                (None, Some(uid)) => MemoryConfig::LongTerm {
                    user_id: uid.clone(),
                },
                (None, None) => MemoryConfig::NoMemory,
            };
        }
        if options.db_path.is_some() {
            self.db_path = options.db_path.clone();
        }
        if let Some(key) = &options.exa_api_key {
            self.tool_source.exa_api_key = Some(key.clone());
        }
        if options.mcp_exa && self.tool_source.exa_api_key.is_none() {
            if let Ok(key) = std::env::var("EXA_API_KEY") {
                self.tool_source.exa_api_key = Some(key);
            }
        }
        if let Some(url) = &options.mcp_exa_url {
            self.mcp_exa_url = url.clone();
        }
        if options.stream {
            self.stream = true;
        }
        self.verbose = options.verbose;
    }

    /// Enable short-term memory (checkpointer) for conversation history.
    pub fn with_short_term_memory(mut self, thread_id: &str) -> Self {
        self.memory = MemoryConfig::ShortTerm {
            thread_id: thread_id.to_string(),
        };
        self
    }

    /// Enable long-term memory (store) for persistent facts and preferences.
    pub fn with_long_term_memory(mut self, user_id: &str) -> Self {
        self.memory = MemoryConfig::LongTerm {
            user_id: user_id.to_string(),
        };
        self
    }

    /// Enable both short-term and long-term memory.
    pub fn with_memory(mut self, thread_id: &str, user_id: &str) -> Self {
        self.memory = MemoryConfig::Both {
            thread_id: thread_id.to_string(),
            user_id: user_id.to_string(),
        };
        self
    }

    /// Disable memory (both short-term and long-term).
    pub fn without_memory(mut self) -> Self {
        self.memory = MemoryConfig::NoMemory;
        self
    }

    /// Get thread ID for short-term memory (checkpointer).
    pub fn thread_id(&self) -> Option<&str> {
        match &self.memory {
            MemoryConfig::ShortTerm { thread_id } => Some(thread_id),
            MemoryConfig::Both { thread_id, .. } => Some(thread_id),
            _ => None,
        }
    }

    /// Get user ID for long-term memory (store).
    pub fn user_id(&self) -> Option<&str> {
        match &self.memory {
            MemoryConfig::LongTerm { user_id } => Some(user_id),
            MemoryConfig::Both { user_id, .. } => Some(user_id),
            _ => None,
        }
    }

    /// Get the effective embedding API key (falls back to OPENAI_API_KEY if not set).
    pub fn embedding_api_key(&self) -> &str {
        self.embedding_api_key.as_deref().unwrap_or(&self.api_key)
    }

    /// Get the effective embedding API base URL (falls back to OPENAI_API_BASE if not set).
    pub fn embedding_api_base(&self) -> &str {
        self.embedding_api_base.as_deref().unwrap_or(&self.api_base)
    }

    /// Get the embedding model name (defaults to text-embedding-3-small).
    pub fn embedding_model(&self) -> &str {
        self.embedding_model
            .as_deref()
            .unwrap_or("text-embedding-3-small")
    }

    /// Builds a langgraph [`ReactBuildConfig`](langgraph::ReactBuildConfig) for use with
    /// [`build_react_run_context`](langgraph::build_react_run_context). CLI-specific RunConfig
    /// is converted to the minimal config required by the builder.
    pub fn to_react_build_config(&self) -> langgraph::ReactBuildConfig {
        langgraph::ReactBuildConfig {
            db_path: self.db_path.clone(),
            thread_id: self.thread_id().map(ToString::to_string),
            user_id: self.user_id().map(ToString::to_string),
            exa_api_key: self.tool_source.exa_api_key.clone(),
            mcp_exa_url: self.mcp_exa_url.clone(),
            mcp_remote_cmd: self.mcp_remote_cmd.clone(),
            mcp_remote_args: self.mcp_remote_args.clone(),
            mcp_verbose: self.verbose,
        }
    }

    #[cfg(all(feature = "embedding", feature = "openai"))]
    /// Create an OpenAIEmbedder from this configuration.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use langgraph_cli::RunConfig;
    /// use langgraph::OpenAIEmbedder;
    ///
    /// let config = RunConfig::from_env()?;
    /// let embedder = config.create_embedder();
    /// ```
    pub fn create_embedder(&self) -> langgraph::OpenAIEmbedder {
        use async_openai::config::OpenAIConfig;
        let openai_config = async_openai::config::OpenAIConfig::new()
            .with_api_key(self.embedding_api_key())
            .with_api_base(self.embedding_api_base());
        langgraph::OpenAIEmbedder::with_config(openai_config, self.embedding_model())
    }
}

impl RunConfig {
    /// Fill config from env vars (and .env). Requires `dotenv::dotenv().ok()` or load inside `run()`.
    ///
    /// `OPENAI_API_KEY` required; `OPENAI_API_BASE`, `OPENAI_MODEL` have defaults.
    /// `OPENAI_TEMPERATURE`, `OPENAI_TOOL_CHOICE` (auto|none|required) optional.
    /// For embeddings: `EMBEDDING_API_KEY`, `EMBEDDING_API_BASE`, `EMBEDDING_MODEL` optional.
    /// For memory: `THREAD_ID`, `USER_ID`, `DB_PATH` optional.
    /// For Exa MCP: `USE_EXA_MCP`, `EXA_API_KEY`, `MCP_EXA_URL`, `MCP_REMOTE_CMD`, `MCP_REMOTE_ARGS` optional.
    pub fn from_env() -> Result<Self, Error> {
        let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "OPENAI_API_KEY is not set; please configure it in .env",
            )
        })?;
        let api_base = std::env::var("OPENAI_API_BASE")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
        let temperature = std::env::var("OPENAI_TEMPERATURE")
            .ok()
            .and_then(|s| s.parse().ok());
        let tool_choice = std::env::var("OPENAI_TOOL_CHOICE")
            .ok()
            .and_then(|s| s.parse().ok());
        let embedding_api_key = std::env::var("EMBEDDING_API_KEY").ok();
        let embedding_api_base = std::env::var("EMBEDDING_API_BASE").ok();
        let embedding_model = std::env::var("EMBEDDING_MODEL")
            .ok()
            .or_else(|| Some("text-embedding-3-small".to_string()));
        let thread_id = std::env::var("THREAD_ID").ok();
        let user_id = std::env::var("USER_ID").ok();
        let db_path = std::env::var("DB_PATH")
            .ok()
            .or_else(|| Some("memory.db".to_string()));
        let tool_source = ToolSourceConfig {
            exa_api_key: std::env::var("EXA_API_KEY").ok(),
        };
        let mcp_exa_url =
            std::env::var("MCP_EXA_URL").unwrap_or_else(|_| "https://mcp.exa.ai/mcp".to_string());
        let mcp_remote_cmd = std::env::var("MCP_REMOTE_CMD").unwrap_or_else(|_| "npx".to_string());
        let mcp_remote_args =
            std::env::var("MCP_REMOTE_ARGS").unwrap_or_else(|_| "-y mcp-remote".to_string());
        let memory = match (thread_id, user_id) {
            (Some(tid), Some(uid)) => MemoryConfig::Both {
                thread_id: tid,
                user_id: uid,
            },
            (Some(tid), None) => MemoryConfig::ShortTerm { thread_id: tid },
            (None, Some(uid)) => MemoryConfig::LongTerm { user_id: uid },
            (None, None) => MemoryConfig::NoMemory,
        };
        Ok(Self {
            api_base,
            api_key,
            model,
            temperature,
            tool_choice,
            embedding_api_key,
            embedding_api_base,
            embedding_model,
            memory,
            db_path,
            tool_source,
            mcp_exa_url,
            mcp_remote_cmd,
            mcp_remote_args,
            stream: true,
            verbose: false,
        })
    }
}

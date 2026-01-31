//! langgraph-cli 库：可被其他 crate 复用的 ReAct 运行逻辑。
//!
//! 从 .env 读取 OpenAI 配置，构建 think → act → observe 图并执行，返回最终状态。
//!
//! ## 用法
//!
//! ```rust,no_run
//! let state = langgraph_cli::run("用户消息").await?;
//! for m in &state.messages {
//!     // 处理 System / User / Assistant 消息
//! }
//! ```

use async_openai::config::OpenAIConfig;
use langgraph::{
    ActNode, ChatOpenAI, CompiledStateGraph, MockToolSource, ObserveNode, REACT_SYSTEM_PROMPT,
    StateGraph, ThinkNode, ToolSource,
};

/// 对外暴露的类型，便于调用方处理 `run` 的返回值。
pub use langgraph::{Message, ReActState};

/// 库内使用的错误类型。
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// 运行配置：API 地址、密钥、模型。可从环境 / .env 填充。
#[derive(Clone, Debug)]
pub struct RunConfig {
    /// OpenAI API base URL，如 `https://api.openai.com/v1`。
    pub api_base: String,
    /// OpenAI API key。
    pub api_key: String,
    /// 模型名，如 `gpt-4o-mini`。
    pub model: String,
}

impl RunConfig {
    /// 从环境变量（及 .env）填充配置。需已调用 `dotenv::dotenv().ok()` 或由 `run()` 内部加载。
    ///
    /// `OPENAI_API_KEY` 必填；`OPENAI_API_BASE`、`OPENAI_MODEL` 有默认值。
    pub fn from_env() -> Result<Self, Error> {
        let api_key = std::env::var("OPENAI_API_KEY").map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "OPENAI_API_KEY 未设置，请在 .env 中配置",
            )
        })?;
        let api_base = std::env::var("OPENAI_API_BASE")
            .unwrap_or_else(|_| "https://api.openai.com/v1".to_string());
        let model =
            std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());
        Ok(Self {
            api_base,
            api_key,
            model,
        })
    }
}

/// 使用默认配置（从 .env 读取）运行 ReAct 图，返回最终状态。
///
/// 内部会加载 `.env`，再调用 `run_with_config`。
pub async fn run(user_message: &str) -> Result<ReActState, Error> {
    dotenv::dotenv().ok();
    let config = RunConfig::from_env()?;
    run_with_config(&config, user_message).await
}

/// 使用指定配置运行 ReAct 图，不读 .env，返回最终状态。
pub async fn run_with_config(config: &RunConfig, user_message: &str) -> Result<ReActState, Error> {
    let openai_config = OpenAIConfig::new()
        .with_api_base(&config.api_base)
        .with_api_key(config.api_key.clone());

    let tool_source = MockToolSource::get_time_example();
    let tools = tool_source.list_tools().await?;
    let llm = ChatOpenAI::with_config(openai_config, config.model.clone()).with_tools(tools);
    let think = ThinkNode::new(Box::new(llm));
    let act = ActNode::new(Box::new(tool_source));
    let observe = ObserveNode::new();

    let mut graph = StateGraph::<ReActState>::new();
    graph
        .add_node("think", Box::new(think))
        .add_node("act", Box::new(act))
        .add_node("observe", Box::new(observe))
        .add_edge("think")
        .add_edge("act")
        .add_edge("observe");

    let compiled: CompiledStateGraph<ReActState> = graph.compile()?;

    let state = ReActState {
        messages: vec![
            Message::system(REACT_SYSTEM_PROMPT),
            Message::user(user_message.to_string()),
        ],
        tool_calls: vec![],
        tool_results: vec![],
    };

    let final_state = compiled.invoke(state, None).await?;
    Ok(final_state)
}

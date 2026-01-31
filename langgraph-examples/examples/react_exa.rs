//! ReAct agent with Exa MCP: web search via Exa's MCP server (think → act → observe).
//!
//! Connects to Exa's hosted MCP at `https://mcp.exa.ai/mcp` using `mcp-remote` as a
//! stdio→HTTP bridge so `McpToolSource` can talk to the remote server. Uses a real
//! LLM (ChatZhipu) so the model can choose tools like `web_search_exa`, `get_code_context_exa`,
//! or `company_research_exa`.
//!
//! Design: [docs/rust-langgraph/mcp-integration/overview.md].
//!
//! ## Prerequisites
//!
//! - Node.js/npx (for `mcp-remote`). Install: `npm install -g mcp-remote` or use `npx -y mcp-remote`.
//! - Zhipu API key for the LLM: set `ZHIPU_API_KEY` in `.env` or environment.
//!
//! ## Usage
//!
//! ```bash
//! cargo run -p langgraph-examples --example react_exa
//! cargo run -p langgraph-examples --example react_exa -- "Search the web for latest Rust 2024 news"
//! ```
//!
//! ## Environment
//!
//! - `ZHIPU_API_KEY`: Required for ChatZhipu (do NOT commit).
//! - `EXA_API_KEY`: Optional. Exa hosted endpoint may work without it (rate-limited).
//!   If you use the npm Exa MCP server locally, set this and point `MCP_EXA_URL` to your server.
//! - `MCP_EXA_URL`: Optional. Default `https://mcp.exa.ai/mcp`. Use another URL if self-hosting.
//! - `MCP_REMOTE_CMD`: Optional. Default `npx`. Use full path if npx is not in PATH.
//! - `MCP_REMOTE_ARGS`: Optional. Default `-y mcp-remote $MCP_EXA_URL`.

use langgraph::{
    ActNode, ChatZhipu, CompiledStateGraph, Message, McpToolSource, ObserveNode, ReActState,
    REACT_SYSTEM_PROMPT, StateGraph, ThinkNode, ToolSource,
};

#[cfg(not(feature = "zhipu"))]
fn main() {
    eprintln!("Build with --features zhipu to run this example (mcp is default).");
    std::process::exit(1);
}

#[cfg(feature = "zhipu")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();

    let user_input = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Search the web for the latest news about Rust programming language in 2024.".to_string());

    let exa_url = std::env::var("MCP_EXA_URL").unwrap_or_else(|_| "https://mcp.exa.ai/mcp".to_string());
    let cmd = std::env::var("MCP_REMOTE_CMD").unwrap_or_else(|_| "npx".to_string());
    let args_str = std::env::var("MCP_REMOTE_ARGS").unwrap_or_else(|_| "-y mcp-remote".to_string());
    let mut args: Vec<String> = args_str
        .split_whitespace()
        .map(String::from)
        .collect();
    if !args.iter().any(|a| a.as_str() == exa_url.as_str() || a.contains("mcp.exa.ai")) {
        args.push(exa_url.clone());
    }

    let tool_source = if let Ok(key) = std::env::var("EXA_API_KEY") {
        let mut env: Vec<(String, String)> = vec![("EXA_API_KEY".to_string(), key)];
        if let Ok(home) = std::env::var("HOME") {
            env.push(("HOME".to_string(), home));
        }
        McpToolSource::new_with_env(cmd, args, env)?
    } else {
        McpToolSource::new(cmd, args)?
    };

    let tools = tool_source.list_tools().await?;
    let llm = ChatZhipu::new("glm-4-flash").with_tools(tools);
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
            Message::user(user_input.clone()),
        ],
        tool_calls: vec![],
        tool_results: vec![],
    };

    println!("User: {}", user_input);
    println!("---");

    match compiled.invoke(state, None).await {
        Ok(s) => {
            for m in &s.messages {
                match m {
                    Message::System(x) => println!("[System] {}", x),
                    Message::User(x) => println!("[User] {}", x),
                    Message::Assistant(x) => println!("[Assistant] {}", x),
                }
            }
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("\nEnsure ZHIPU_API_KEY is set and npx/mcp-remote can reach {}", exa_url);
            std::process::exit(1);
        }
    }

    Ok(())
}

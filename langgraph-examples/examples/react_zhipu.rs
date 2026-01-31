//! ReAct with ChatZhipu (智谱): think → act → observe → END.
//!
//! Design: [docs/rust-langgraph/15-llm-react-agent.md](https://github.com/.../15-llm-react-agent.md).
//! Uses Zhipu GLM Chat Completions (ChatZhipu) instead of MockLlm; ToolSource::list_tools
//! is passed to ChatZhipu::with_tools so the model can return tool_calls.
//!
//! ## Prerequisites
//!
//! Create a `.env` file in the workspace root with:
//!
//! ```bash
//! ZHIPU_API_KEY=...
//! ```
//!
//! Or set the `ZHIPU_API_KEY` environment variable.
//!
//! ## Usage
//!
//! ```bash
//! cargo run -p langgraph-examples --example react_zhipu -- "What time is it?"
//! cargo run -p langgraph-examples --example react_zhipu -- "3+5 equals?"
//! ```

use langgraph::{
    ActNode, ChatZhipu, CompiledStateGraph, Message, MockToolSource, ObserveNode, ReActState,
    REACT_SYSTEM_PROMPT, StateGraph, ThinkNode, ToolSource,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env (ZHIPU_API_KEY) before creating client
    dotenv::dotenv().ok();

    let user_input = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "What time is it?".to_string());

    // ToolSource provides tools for both: list_tools (→ LLM) and call_tool (→ Act)
    let tool_source = MockToolSource::get_time_example();
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
            eprintln!("\nMake sure ZHIPU_API_KEY is set:");
            eprintln!("  export ZHIPU_API_KEY=\"...\"");
            std::process::exit(1);
        }
    }

    Ok(())
}

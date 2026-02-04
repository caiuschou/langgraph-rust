//! ReAct Agent binary: parses CLI message, invokes the library and prints the result.

use clap::Parser;
use langgraph_cli::{run_with_options, Message, RunOptions};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "langgraph")]
#[command(about = "ReAct agent — input a message, run think → act → observe chain")]
struct Args {
    /// User message (or pass as first positional argument)
    #[arg(short, long, value_name = "TEXT")]
    message: Option<String>,

    /// Positional args: user message (when -m/--message is not used)
    #[arg(trailing_var_arg = true)]
    rest: Vec<String>,

    /// Sampling temperature 0–2, lower is more deterministic (e.g. 0.2)
    #[arg(short, long, value_name = "FLOAT")]
    temperature: Option<f32>,

    /// Tool choice: auto (default), none, required
    #[arg(long, value_name = "MODE")]
    tool_choice: Option<String>,

    /// Thread ID for short-term memory (checkpointer)
    #[arg(long, value_name = "ID")]
    thread_id: Option<String>,

    /// User ID for long-term memory (store)
    #[arg(long, value_name = "ID")]
    user_id: Option<String>,

    /// SQLite database path for persistence
    #[arg(long, value_name = "PATH")]
    db_path: Option<String>,

    /// Enable Exa MCP for web search
    #[arg(long)]
    mcp_exa: bool,

    /// Exa API key (optional, for authenticated requests)
    #[arg(long, value_name = "KEY")]
    exa_api_key: Option<String>,

    /// Exa MCP server URL
    #[arg(long, value_name = "URL")]
    mcp_exa_url: Option<String>,

    /// Stream output: show Thinking... / Calling tool / LLM tokens as they arrive (default: on)
    #[arg(long, default_value_t = true)]
    stream: bool,

    /// Disable streaming (use when piping or scripting)
    #[arg(long = "no-stream", action = clap::ArgAction::SetTrue)]
    no_stream: bool,

    /// Show debug logs (node enter/exit, graph execution)
    #[arg(short, long)]
    verbose: bool,
}

fn get_message(args: &Args) -> String {
    if let Some(ref m) = args.message {
        return m.clone();
    }
    if args.rest.is_empty() {
        return "What time is it?".to_string();
    }
    args.rest.join(" ").trim().to_string()
}

fn args_to_run_options(args: &Args) -> Result<RunOptions, String> {
    let tool_choice = match &args.tool_choice {
        None => None,
        Some(tc) => Some(tc.parse().map_err(|e: String| e)?),
    };
    Ok(RunOptions {
        temperature: args.temperature,
        tool_choice,
        thread_id: args.thread_id.clone(),
        user_id: args.user_id.clone(),
        db_path: args.db_path.clone(),
        mcp_exa: args.mcp_exa,
        exa_api_key: args.exa_api_key.clone(),
        mcp_exa_url: args.mcp_exa_url.clone(),
        stream: args.stream && !args.no_stream,
        verbose: args.verbose,
        ..Default::default()
    })
}

/// Initializes tracing so that langgraph's debug/info/error logs are shown only when verbose.
fn init_tracing(verbose: bool) {
    let level = if verbose { "debug" } else { "off" };
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(level))
        .with_writer(std::io::stderr)
        .try_init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    let args = Args::parse();
    init_tracing(args.verbose);
    let input = get_message(&args);

    let options = match args_to_run_options(&args) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
    };

    println!("User: {}", input);
    println!("---");

    let state = match run_with_options(&input, &options).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
    };

    if options.stream {
        // Ensure the final reply is visible: use last non-empty Assistant (last one can be empty when the turn only had tool_calls).
        println!();
        match state
            .messages
            .iter()
            .rev()
            .find(|m| matches!(m, Message::Assistant(c) if !c.is_empty()))
        {
            Some(Message::Assistant(content)) => println!("[Assistant] {}", content),
            _ => eprintln!(
                "[No assistant text in state ({} messages); last turn may have been tool-only]",
                state.messages.len()
            ),
        }
    } else {
        for m in &state.messages {
            match m {
                Message::System(x) => println!("[System] {}", x),
                Message::User(x) => println!("[User] {}", x),
                Message::Assistant(x) => println!("[Assistant] {}", x),
            }
        }
    }
    if state.messages.is_empty() {
        eprintln!("no messages");
        std::process::exit(1);
    }

    Ok(())
}

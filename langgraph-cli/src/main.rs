//! ReAct Agent binary: parses CLI message, invokes the library and prints the result.

use clap::Parser;
use langgraph_cli::{run_with_config, Message, RunConfig};

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    let args = Args::parse();
    let input = get_message(&args);

    let mut config = match RunConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
    };
    if let Some(t) = args.temperature {
        config.temperature = Some(t);
    }
    if let Some(ref tc) = args.tool_choice {
        config.tool_choice = Some(match tc.parse() {
            Ok(m) => m,
            Err(e) => {
                eprintln!("error: {}", e);
                std::process::exit(1);
            }
        });
    }

    println!("User: {}", input);
    println!("---");

    let state = match run_with_config(&config, &input).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
    };

    for m in &state.messages {
        match m {
            Message::System(x) => println!("[System] {}", x),
            Message::User(x) => println!("[User] {}", x),
            Message::Assistant(x) => println!("[Assistant] {}", x),
        }
    }
    if state.messages.is_empty() {
        eprintln!("no messages");
        std::process::exit(1);
    }

    Ok(())
}

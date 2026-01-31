//! ReAct Agent 二进制：解析命令行消息，调用库运行并打印结果。

use clap::Parser;
use langgraph_cli::{run, Message};

#[derive(Parser, Debug)]
#[command(name = "langgraph")]
#[command(about = "ReAct agent — 输入消息，运行 think → act → observe 链")]
struct Args {
    /// 用户消息（也可直接作为第一个位置参数）
    #[arg(short, long, value_name = "TEXT")]
    message: Option<String>,

    /// 位置参数：用户消息（当未使用 -m/--message 时）
    #[arg(trailing_var_arg = true)]
    rest: Vec<String>,
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
    let args = Args::parse();
    let input = get_message(&args);

    println!("User: {}", input);
    println!("---");

    let state = match run(&input).await {
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

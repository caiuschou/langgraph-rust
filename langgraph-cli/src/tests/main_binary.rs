//! Integration-style tests for the `langgraph` binary (main).
//!
//! Scenarios: --help exits with code 0; invalid args yield non-zero or error output.

use std::process::Command;

/// **Scenario**: Running the binary with --help exits with code 0.
///
/// Runs `cargo run -p langgraph-cli --bin langgraph -- --help` and asserts success.
#[test]
fn main_exits_zero_with_help() {
    let status = Command::new("cargo")
        .args([
            "run",
            "-p",
            "langgraph-cli",
            "--bin",
            "langgraph",
            "--",
            "--help",
        ])
        .status();
    let status = status.expect("failed to run cargo");
    assert!(
        status.success(),
        "expected --help to exit 0, got {}",
        status
    );
}

/// **Scenario**: Running the binary with an invalid argument yields non-zero exit or error output.
#[test]
fn main_with_invalid_args_returns_error() {
    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "langgraph-cli",
            "--bin",
            "langgraph",
            "--",
            "--invalid-flag-xyz",
        ])
        .output();
    let output = output.expect("failed to run cargo");
    assert!(
        !output.status.success(),
        "expected invalid args to exit non-zero, got {}",
        output.status
    );
}

/// **Scenario**: When --verbose is passed, stderr contains the four config summary lines
/// (LLM config, Memory config, Tools, Embedding) before graph execution.
///
/// Sets OPENAI_API_KEY so config loads; summary is printed after build_react_run_context.
#[test]
fn main_with_verbose_prints_config_summary_to_stderr() {
    let output = Command::new("cargo")
        .args([
            "run",
            "-p",
            "langgraph-cli",
            "--bin",
            "langgraph",
            "--",
            "--verbose",
            "-m",
            "hi",
        ])
        .env("OPENAI_API_KEY", "test-key-for-verbose-test")
        .output();
    let output = output.expect("failed to run cargo");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("[LLM config]"),
        "stderr should contain [LLM config], got: {}",
        stderr
    );
    assert!(
        stderr.contains("[Memory config]"),
        "stderr should contain [Memory config], got: {}",
        stderr
    );
    assert!(
        stderr.contains("[Tools]"),
        "stderr should contain [Tools], got: {}",
        stderr
    );
    assert!(
        stderr.contains("[Embedding]"),
        "stderr should contain [Embedding], got: {}",
        stderr
    );
}

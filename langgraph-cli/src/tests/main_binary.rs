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
    assert!(status.success(), "expected --help to exit 0, got {}", status);
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

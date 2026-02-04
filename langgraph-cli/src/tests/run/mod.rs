//! Run module tests: unit tests for [`run_react_graph`](langgraph::run_react_graph) and
//! integration tests for [`run_with_config`](crate::run_with_config).
//!
//! Tests are BDD-style with clear Scenario/Given/When/Then in doc comments.
//! Unit tests live in `run_react_graph`; integration tests in `run_with_config`.

mod config_summary;
mod run_react_graph;
mod run_with_config;

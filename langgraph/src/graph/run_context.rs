//! Run context passed into nodes for streaming-aware execution.
//!
//! Holds runnable config and optional stream sender plus selected stream modes.

use std::collections::HashSet;
use std::fmt::Debug;

use tokio::sync::mpsc;

use crate::memory::RunnableConfig;
use crate::stream::{StreamEvent, StreamMode};

#[derive(Clone)]
pub struct RunContext<S>
where
    S: Clone + Send + Sync + Debug + 'static,
{
    /// Config for the current run (thread_id, checkpoint, user_id, etc.).
    pub config: RunnableConfig,
    /// Optional sender for streaming events.
    pub stream_tx: Option<mpsc::Sender<StreamEvent<S>>>,
    /// Enabled stream modes (Values, Updates, Messages, Custom).
    pub stream_mode: HashSet<StreamMode>,
}

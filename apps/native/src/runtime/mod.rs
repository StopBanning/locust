//! Runtime orchestration has no window or Electron dependency.
pub mod acp;
pub(crate) mod acp_events;
pub mod claude;
pub(crate) mod claude_events;
pub mod codex;
mod discovery;
pub(crate) mod events;
pub mod kilo;
pub mod opencode;
pub(crate) mod opencode_config;
pub(crate) mod opencode_events;
pub(crate) mod opencode_process;
pub(crate) mod opencode_serve;
use crate::approval::{Channel, Mode};
use serde_json::Value;
use std::{io, path::PathBuf, sync::Arc};
pub struct Turn {
    pub prompt: String,
    pub folder: PathBuf,
    pub model: Option<String>,
    /// Reasoning effort for this turn only: a plain lowercase word, as codex-app-server-run.ts passes it.
    pub effort: Option<String>,
    /// A prior Codex thread to reopen with `thread/resume` instead of `thread/start` (the follow-up path).
    pub resume: Option<String>,
    /// Teammate mode; drives Codex sandbox / approvalPolicy (`codexAppServerPolicy`).
    pub mode: Mode,
    /// When set, approval requests raise a card instead of being auto-declined.
    pub approvals: Option<Arc<Channel>>,
    pub mission_id: String,
    pub run_id: String,
}
pub struct Event {
    pub kind: String,
    pub payload: Value,
    pub thread: Option<String>,
}
pub trait Runtime {
    fn start(&mut self, turn: &Turn) -> io::Result<()>;
    /// A timeout returns None; terminal events are explicit.
    fn next_event(&mut self) -> io::Result<Option<Event>>;
    fn stop(&mut self) -> io::Result<()>;
}

#[cfg(test)]
mod tests;

pub mod permission_bridge;

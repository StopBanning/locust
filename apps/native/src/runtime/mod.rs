//! Runtime orchestration has no window or Electron dependency.
pub mod codex;
mod discovery;
pub(crate) mod events;
use serde_json::Value;
use std::{io, path::PathBuf};
pub struct Turn {
    pub prompt: String,
    pub folder: PathBuf,
    pub model: Option<String>,
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

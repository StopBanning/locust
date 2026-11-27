//! Runtime orchestration has no window or Electron dependency.
pub mod acp;
pub mod acp_elicitation;
pub mod acp_events;
pub mod agy_events;
pub mod antigravity;
pub mod apple;
pub mod antigravity_events;
pub mod antigravity_projects;
pub mod claude;
pub mod claude_commands;
pub mod claude_events;
pub mod codex;
pub mod codex_voice;
pub mod copilot;
pub mod copilot_events;
pub mod cursor;
pub mod cursor_events;
pub mod discovery;
pub mod events;
pub mod goal;
pub mod held_text;
pub mod kilo;
pub mod muse;
pub mod muse_events;
pub mod opencode;
pub mod opencode_config;
pub mod opencode_events;
pub mod opencode_serve;
pub mod opencode_commands;
#[cfg(test)]
mod opencode_serve_queue_tests;
pub mod process;
pub mod records;
pub mod safety;
pub mod version;
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
    fn can_steer(&self) -> bool {
        false
    }
    /// The runtime's own slash commands, when it announced a list during the run (Claude Code, at its start).
    fn learned_commands(&mut self) -> Option<Vec<crate::runtime_commands::CommandInfo>> {
        None
    }
    fn steer(&mut self, _text: &str) -> bool {
        false
    }
    /// A standing goal on the runtime's thread (Codex app-server `thread/goal/*`). A runtime without them says so; it
    /// is never a turn-ending error.
    fn goal(&mut self, _ask: goal::GoalAsk) -> goal::GoalAnswer {
        goal::GoalAnswer::NoStandingGoals
    }
}

#[cfg(test)]
mod antigravity_tests;
#[cfg(test)]
mod apple_tests;
#[cfg(test)]
mod compaction_tests;
#[cfg(test)]
mod copilot_tests;
#[cfg(test)]
mod cursor_tests;
#[cfg(test)]
mod muse_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod command_grid_tests;
#[cfg(test)]
mod event_goldens_tests;
#[cfg(test)]
mod oversized_cursor_edit_tests;
#[cfg(test)]
mod copilot_quiet_records_tests;
#[cfg(test)]
mod denied_read_and_file_row_tests;
#[cfg(test)]
mod message_tail_tests;
#[cfg(test)]
mod claude_background_calls_tests;
#[cfg(test)]
mod claude_thinking_tests;
#[cfg(test)]
mod claude_helper_children_tests;
#[cfg(test)]
mod codex_config_warning_tests;
#[cfg(test)]
mod large_cursor_result_tests;
#[cfg(test)]
mod claude_command_connectors_tests;
#[cfg(test)]
mod opencode_sealed_run_tests;
#[cfg(test)]
mod side_question_fork_tests;
#[cfg(test)]
mod opencode_2_step_tests;
#[cfg(test)]
mod opencode_parent_git_tests;
#[cfg(test)]
mod acp_rename_tests;
#[cfg(test)]
mod run_skills_tests;
#[cfg(test)]
mod runtime_adapters_case_tests;

pub mod permission_bridge;

#[cfg(test)]
mod test_support;

pub use discovery::find_named as find_executable;

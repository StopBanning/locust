//! Composer & input — M4b. Pure logic ports Electron; the window wires them in `main.rs`.

pub mod attachments;
pub mod drafts;
pub mod file_mentions;
pub mod keys;
pub mod placeholder;
pub mod queue;
pub mod queue_store;
pub mod send_blocked;
pub mod slash;
pub mod workspace_files;

pub use attachments::{MAX_PASTED_BYTES, attach_paths, attachment_label, with_attachments};
pub use drafts::{Drafts, draft_key};
pub use file_mentions::{at_query, file_matches, without_at_query};
pub use keys::{ComposerKey, KeyAction, KeyMods, handle_key, shift_tab_mode};
pub use placeholder::{PlaceholderInput, placeholder};
pub use queue::{
    Origin, QueuedRow, QueuedVerdict, VerdictInput, queue_note, queued_in, queued_verdict,
    take_next, without_queue_of,
};
pub use queue_store::{new_queue_id, read_queue, write_queue};
pub use send_blocked::{SEND_LIVE_TITLE, SendBlocked, send_blocked_reason};
pub use slash::{
    AvailableInput, SlashAction, SlashCommand, available_commands, matching_commands, slash_query,
};

/// Electron composer textarea max height (shell.css `.lc-composer textarea`).
pub const COMPOSER_MAX_HEIGHT_PX: f32 = 180.0;
/// ~11 lines at Electron's 16px line-height before the 180px cap.
pub const COMPOSER_MAX_LINES: usize = 11;
pub const COMPOSER_MIN_LINES: usize = 1;

/// Files heading when the host listed every path it knows.
pub const FILES_HEADING: &str = "Files in this folder";
/// Teammates group in the `@` popup.
pub const TEAMMATES_HEADING: &str = "Teammates";
/// Empty `@` match in a partial listing (Composer.tsx).
pub const NO_MATCH_PARTIAL: &str =
    "No match among these. This folder has more files than @ lists; + attaches any of them.";
/// Teammate `@` detail line.
pub fn teammate_detail(role: &str) -> String {
    format!("{role}: sent this message too")
}
/// Queued strip action labels.
pub const EDIT_LABEL: &str = "Edit";
pub const DISCARD_LABEL: &str = "Discard";
pub const SEND_NOW_LABEL: &str = "Send now";
pub const STOP_AND_SEND_NOW_LABEL: &str = "Stop and send now";
pub const QUEUE_SEND_TITLE: &str = "Send this when the reply finishes";

//! A TURN'S TO-DOS, AND HOW EACH IS GOING (Electron `renderer/src/callTodos.ts`, and `HandedOut` in
//! `components/PeerThread.tsx`). Under a lead's turn, who has each ask it handed out and where it stands now, one line
//! each, read from the same run and record the sidebar reads: a to-do says Done when its teammate has finished and
//! Needs you while it waits on the person.

/// One to-do's state (`CallTodoState`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TodoState {
    Starting,
    Working,
    NeedsYou,
    Done,
    Failed,
    Stopped,
}

/// The chip's tone (`StatusTone`), drawn as `.lc-status--{tone}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Quiet,
    Blue,
    Amber,
    Green,
    Red,
}

impl Tone {
    pub fn name(self) -> &'static str {
        match self {
            Self::Quiet => "quiet",
            Self::Blue => "blue",
            Self::Amber => "amber",
            Self::Green => "green",
            Self::Red => "red",
        }
    }
}

impl TodoState {
    /// The chip's word and tone (`CALL_TODO_WORDS`).
    pub fn words(self) -> (&'static str, Tone) {
        match self {
            Self::Starting => ("Starting", Tone::Quiet),
            Self::Working => ("Working", Tone::Blue),
            Self::NeedsYou => ("Needs you", Tone::Amber),
            Self::Done => ("Done", Tone::Green),
            Self::Failed => ("Failed", Tone::Red),
            Self::Stopped => ("Stopped", Tone::Quiet),
        }
    }
}

/// One to-do's state: the run while the window holds it, else the record. Waiting on the person outranks both. A
/// to-do the window has heard of but not yet seen run is starting.
pub fn todo_state(run_phase: Option<&str>, record_phase: Option<&str>, needs_you: bool) -> TodoState {
    if needs_you {
        return TodoState::NeedsYou;
    }
    match run_phase.or(record_phase) {
        Some("completed") => TodoState::Done,
        Some("failed") => TodoState::Failed,
        Some("cancelled" | "interrupted") => TodoState::Stopped,
        Some("running" | "cancelling") => TodoState::Working,
        _ => TodoState::Starting,
    }
}

/// "3 to-dos", "1 to-do".
pub fn todo_count(count: usize) -> String {
    format!("{count} {}", if count == 1 { "to-do" } else { "to-dos" })
}

/// One line of the card: the posted message it came from, the turn it was delivered into (None until the window has
/// seen its delivery: then it is Starting, with nothing to open), who has it, and what was asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handed {
    pub message_id: String,
    pub mission_id: Option<String>,
    pub name: String,
    pub task: String,
}

/// What a message no longer in the workroom is shown as.
pub const GONE: &str = "This message is no longer in the workroom.";

/// The card's lines for one turn's peer messages (`HandedOut`): the messages it POSTED, each its own line, even two
/// delivered into the same turn. `received_in` names the receiving turn of a posted message, never the sender's.
pub fn handed_out<'a>(
    messages: impl IntoIterator<Item = (&'a str, &'a str, &'a str, Option<&'a str>)>,
    received_in: impl Fn(&str) -> Option<String>,
) -> Vec<Handed> {
    messages
        .into_iter()
        .filter(|(_, direction, _, _)| *direction == "posted")
        .map(|(message_id, _, to_name, text)| Handed {
            message_id: message_id.to_owned(),
            mission_id: received_in(message_id),
            name: to_name.to_owned(),
            task: text.map_or_else(|| GONE.to_owned(), str::to_owned),
        })
        .collect()
}

/// The card's label: "Handed out · 2 to-dos". None when the turn handed nothing out.
pub fn handed_out_label(handed: &[Handed]) -> Option<String> {
    (!handed.is_empty()).then(|| format!("Handed out \u{b7} {}", todo_count(handed.len())))
}

/// The turn a posted message was delivered into (missionView.ts `peerRunFor`): the mission that RECEIVED it, read
/// from whichever turns the window holds -- a live one before the history has caught up -- and never the sender's own.
pub fn peer_run_for<'a>(message_id: &str, missions: impl IntoIterator<Item = (&'a str, &'a [crate::ledger::PeerLink])>) -> Option<&'a str> {
    if message_id.is_empty() {
        return None;
    }
    missions
        .into_iter()
        .find(|(_, links)| crate::thread_view::received_message(message_id, links).is_some())
        .map(|(mission_id, _)| mission_id)
}

#[cfg(test)]
#[path = "call_todos_tests.rs"]
mod tests;

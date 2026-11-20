//! What a Home team card says (Electron `homeTeamState.ts`, 0.606/0.609/0.610): the word beside the name, in which
//! colour and how it is read aloud; what an idle teammate last did, for the hover; and what a waiting card says
//! under the name in place of the role.

use crate::bot_rig::{FaceActivity, face_label};
use crate::teammate_reach::StatusView;

/// The word beside a card's name, its tone ("live", "amber" or "red"), and what a screen reader hears.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamCardState {
    pub word: String,
    pub tone: &'static str,
    pub spoken: String,
}

/// "Sign-in needed" reads "sign-in needed" mid-line; "AI agent not installed" keeps its capitals.
fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(first), Some(second)) if first.is_ascii_uppercase() && second.is_ascii_lowercase() => {
            format!("{}{}", first.to_ascii_lowercase(), &text[1..])
        }
        _ => text.to_owned(),
    }
}

/// The same fact as the sidebar face, worded for the card: the live word while working, "waiting on you" in amber,
/// what blocks the teammate in red. Idle says nothing (the card is calm when they are), and so does an agent that is
/// still being checked: a moment, not a state -- every card once went amber for the seconds after launch.
pub fn team_card_state(view: Option<&StatusView>, activity: FaceActivity) -> Option<TeamCardState> {
    let view = view?;
    match view.status {
        "blocked" => {
            if view.tone == "amber" {
                return None;
            }
            let word = lower_first(&view.label);
            Some(TeamCardState { spoken: word.clone(), word, tone: "red" })
        }
        "approval-needed" => Some(TeamCardState { word: "waiting on you".into(), tone: "amber", spoken: "waiting on you".into() }),
        "working" => {
            let word = face_label(activity).to_owned();
            Some(TeamCardState { spoken: format!("{word} now"), word, tone: "live" })
        }
        _ => None,
    }
}

/// Electron's agoLabel: "just now", "5 minutes ago", "2 hours ago", "1 week ago"; `None` for a time it cannot read.
pub fn ago_label(iso: &str, now: chrono::DateTime<chrono::Utc>) -> Option<String> {
    let at = chrono::DateTime::parse_from_rfc3339(iso).ok()?;
    let seconds = (now - at.with_timezone(&chrono::Utc)).num_seconds();
    if seconds < 60 {
        return Some("just now".into());
    }
    for (size, name) in [(604_800, "week"), (86_400, "day"), (3_600, "hour"), (60, "minute")] {
        let count = seconds / size;
        if count >= 1 {
            return Some(format!("{count} {name}{} ago", if count == 1 { "" } else { "s" }));
        }
    }
    None
}

/// What an idle teammate last did, for the card's hover (0.609): "last worked 3 hours ago", or "no work yet".
pub fn last_worked_phrase(last_at: Option<&str>, now: chrono::DateTime<chrono::Utc>) -> String {
    match last_at.and_then(|at| ago_label(at, now)) {
        Some(ago) => format!("last worked {ago}"),
        None => "no work yet".into(),
    }
}

/// Something on Home that waits on the person, as far as a team card needs to know.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Waiting<'a> {
    /// An approval card: `asking` when it is a question, `command` when it wants to run something.
    Approval { what: &'a str, asking: bool, command: bool },
    Decision { what: &'a str },
    Memory,
    Scout,
    Routine,
}

/// What a waiting card says under the name (0.610): the question as asked, or the action it wants to take.
pub fn waiting_line(item: Waiting<'_>) -> Option<String> {
    match item {
        Waiting::Decision { what } => Some(what.to_owned()),
        Waiting::Approval { what, asking: true, .. } => Some(what.to_owned()),
        Waiting::Approval { what, command: true, .. } => Some(format!("Wants to run {what}")),
        Waiting::Approval { what, .. } => Some(format!("Wants your approval: {what}")),
        Waiting::Memory | Waiting::Scout | Waiting::Routine => None,
    }
}

#[cfg(test)]
#[path = "home_team_state_tests.rs"]
mod tests;

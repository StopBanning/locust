//! What a CLI says about its own setup, not about the work (missionView's SETUP_NOTES, 0.308). Codex opens every turn
//! with remarks on how it is configured ("Skill descriptions were shortened to fit the skills context budget", "Codex
//! is ignoring 1 unrecognized configuration setting ... (the path to your config.toml)"). They are about the CLI, so
//! no conversation carries them: its row in Settings says them, from its newest run that said any.
use serde_json::Value;
use std::collections::BTreeMap;

/// Whether a remark is one of these (missionView's `isSetupNote`).
pub fn is_setup_note(message: &str) -> bool {
    crate::invariants::regex(r"(?i)^(Codex is ignoring \d+ unrecognized configuration settings?\b|Skill descriptions were shortened to fit\b)")
        .is_match(message.trim())
}

/// A run's setup notes, each once, in the order said (missionView's `setupNotesOf`).
pub fn setup_notes_of<'a>(events: impl IntoIterator<Item = &'a Value>) -> Vec<String> {
    let mut notes: Vec<String> = Vec::new();
    for event in events {
        if event["type"] != "adapter.diagnostic" {
            continue;
        }
        let message = event["payload"]["message"].as_str().unwrap_or("").trim();
        if is_setup_note(message) && !notes.iter().any(|n| n == message) {
            notes.push(message.to_owned());
        }
    }
    notes
}

/// One run as the list holds it: its runtime, when it was made, and its events.
pub struct Run<'a> {
    pub runtime: &'a str,
    pub created_at: &'a str,
    pub events: &'a [Value],
}

/// Each runtime's setup notes from its newest run that said any (missionView's `latestSetupNotes`): what its row in
/// Settings shows.
pub fn latest_setup_notes<'a>(runs: impl IntoIterator<Item = Run<'a>>) -> BTreeMap<String, Vec<String>> {
    let mut newest: BTreeMap<String, (&str, Vec<String>)> = BTreeMap::new();
    for run in runs {
        let notes = setup_notes_of(run.events);
        if notes.is_empty() {
            continue;
        }
        if newest.get(run.runtime).is_none_or(|(at, _)| run.created_at > *at) {
            newest.insert(run.runtime.to_owned(), (run.created_at, notes));
        }
    }
    newest.into_iter().map(|(runtime, (_, notes))| (runtime, notes)).collect()
}

/// The words a runtime's row says, at most three (Screens.tsx: "<name> says: <note>").
pub fn row_lines(display_name: &str, notes: &[String]) -> Vec<String> {
    notes.iter().take(3).map(|note| format!("{display_name} says: {note}")).collect()
}

#[cfg(test)]
#[path = "setup_notes_tests.rs"]
mod tests;

//! SAVING A CONVERSATION AS A ROUTINE (Electron `renderer/src/routines.ts`: `routineDraft`, `draftName`).
//!
//! A routine replays what a person asked, so a draft takes the words a person typed on each turn, in order, with the
//! turn each came from. Host-written turns (a relayed reply, a resume, a routine step, a tag, a turn typed in the
//! runtime's own terminal) carry a briefing or a command, not something to replay here, and are left out. Turns the
//! person stopped are left out too; where none worked, any not stopped are kept, so a conversation whose every turn
//! failed on a quota can still be saved and run later.
use crate::conversation_turns::{conversation_turns, typed_prompt};
use crate::ledger::Metadata;
use std::collections::HashMap;

/// `MAX_ROUTINE_STEPS`: a routine holds at most this many steps.
pub const MAX_ROUTINE_STEPS: usize = 12;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutineDraft {
    pub name: String,
    pub steps: Vec<String>,
    pub learned_from: Vec<String>,
    /// The conversation had more turns than a routine may hold, and the draft was cut.
    pub truncated: bool,
}

/// A name from the first step: enough to recognise, short enough to sit in a list. Electron counts UTF-16 units;
/// this counts characters, so a name of emoji or accented letters is cut where a person would count, not earlier.
pub fn draft_name(step: &str) -> String {
    let said = step.split_whitespace().collect::<Vec<_>>().join(" ");
    if said.chars().count() <= 60 {
        return said;
    }
    let cut: String = said.chars().take(60).collect();
    let kept = match cut.rfind(' ') {
        Some(space) if cut[..space].chars().count() > 24 => &cut[..space],
        _ => cut.as_str(),
    };
    format!("{}...", kept.trim_end())
}

/// The draft for the conversation `latest` ends, or None when nobody typed anything in it. `phase_of` says how each
/// turn ended ("completed", "failed", "cancelled", ...), by its mission id; a turn it does not know is kept as not
/// stopped.
pub fn routine_draft(latest: &Metadata, by_id: &HashMap<String, Metadata>, phase_of: impl Fn(&str) -> Option<String>) -> Option<RoutineDraft> {
    let mut said: Vec<(String, String, Option<String>)> = Vec::new();
    for turn in conversation_turns(latest, by_id) {
        let held = by_id.get(&turn.mission_id);
        // Started by the host (or in the runtime's own terminal): not the person's words to replay.
        if held.is_some_and(|m| m.started_by_kind.is_some()) {
            continue;
        }
        // A turn on another runtime was started with the host's briefing; the person's words are the part they typed.
        let prompt = match held {
            Some(m) => typed_prompt(m, by_id).trim().to_owned(),
            None => turn.prompt.trim().to_owned(),
        };
        if prompt.is_empty() {
            continue;
        }
        let phase = phase_of(&turn.mission_id);
        said.push((turn.mission_id, prompt, phase));
    }
    let completed: Vec<_> = said.iter().filter(|t| t.2.as_deref() == Some("completed")).cloned().collect();
    let typed = if completed.is_empty() {
        said.into_iter().filter(|t| t.2.as_deref() != Some("cancelled")).collect::<Vec<_>>()
    } else {
        completed
    };
    let first = typed.first()?;
    // Named by what the person typed, not by the host's attachment line (L17); the step keeps its files.
    let name = draft_name(&crate::attachments::split_attachments(&first.1).0);
    let kept: Vec<_> = typed.iter().take(MAX_ROUTINE_STEPS).collect();
    Some(RoutineDraft {
        name,
        steps: kept.iter().map(|t| t.1.clone()).collect(),
        learned_from: kept.iter().map(|t| t.0.clone()).collect(),
        truncated: typed.len() > kept.len(),
    })
}

#[cfg(test)]
#[path = "routine_draft_tests.rs"]
mod tests;

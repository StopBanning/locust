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

// ---- THE SHELF'S FINISHED CONVERSATIONS (Electron `renderer/src/savableConversations.ts`) ---------------------------

/// `SAVABLE_SHOWN`: how many finished conversations an empty shelf offers. Four fills the space without scrolling.
pub const SAVABLE_SHOWN: usize = 4;

/// `NOTHING_TO_SAVE_YET`: the line when a workspace has finished nothing and there are no templates to offer instead.
pub const NOTHING_TO_SAVE_YET: &str = "Press New routine to write one, or finish a conversation and save it here.";

/// The newest finished conversations, newest first, at most `limit`. `completed` only: a failed or stopped run is not
/// turns worth repeating, and offering one would be the screen recommending it. A row with no time sorts last rather
/// than going missing.
pub fn savable_conversations(rows: &[crate::engine::ConversationRow], limit: usize) -> Vec<&crate::engine::ConversationRow> {
    let at = |row: &crate::engine::ConversationRow| chrono::DateTime::parse_from_rfc3339(&row.updated).map_or(i64::MIN, |d| d.timestamp_millis());
    let mut kept: Vec<_> = rows.iter().filter(|row| row.state == "completed").collect();
    kept.sort_by_key(|row| std::cmp::Reverse(at(row)));
    kept.truncate(limit);
    kept
}

/// `turnsLabel`: "4 turns", and "1 turn" rather than "1 turns"; none counts as one.
pub fn turns_label(turns: usize) -> String {
    let count = turns.max(1);
    format!("{count} turn{}", if count == 1 { "" } else { "s" })
}

/// A finished conversation as the shelf offers it: how many turns it had, and its draft (None when nobody typed in it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub turns: usize,
    pub draft: Option<RoutineDraft>,
    /// Every turn read, the newest first and back through each parent: where its owner is looked for.
    pub chain: Vec<String>,
}

/// How far back a conversation is walked: well past any routine's 12 steps, and a guard against a parent loop.
const MAX_WALK: usize = 256;

/// The conversation whose newest turn is `tip`, read turn by turn through `read` (a turn's record and how it ended),
/// following each turn's parent. None when the tip itself cannot be read.
pub fn saved_from(tip: &str, read: impl Fn(&str) -> Option<(Metadata, String)>) -> Option<Saved> {
    let (latest, tip_phase) = read(tip)?;
    let mut by_id = HashMap::new();
    let mut phases = HashMap::new();
    phases.insert(latest.mission_id.clone(), tip_phase);
    let mut next = latest.parent.clone();
    let mut chain = vec![latest.mission_id.clone()];
    by_id.insert(latest.mission_id.clone(), latest.clone());
    while let Some(id) = next.take() {
        if by_id.contains_key(&id) || by_id.len() >= MAX_WALK {
            break;
        }
        let Some((meta, phase)) = read(&id) else { break };
        next = meta.parent.clone();
        phases.insert(id.clone(), phase);
        chain.push(id.clone());
        by_id.insert(id, meta);
    }
    let turns = conversation_turns(&latest, &by_id).len();
    let draft = routine_draft(&latest, &by_id, |id| phases.get(id).cloned());
    Some(Saved { turns, draft, chain })
}

/// `saved_from` over the ledger folders (native's, then Electron's, read-only).
pub fn read_saved(tip: &str) -> Option<Saved> {
    saved_from(tip, |id| {
        let path = crate::native::find_ledger(id)?;
        let records = crate::ledger::Tail::new(&path, 0).poll();
        let meta = records.iter().find_map(crate::ledger::metadata)?;
        let phase = crate::door_tools::Turn::from_records(&records).map_or("interrupted", |t| t.phase()).to_owned();
        Some((meta, phase))
    })
}

#[cfg(test)]
#[path = "routine_draft_tests.rs"]
mod tests;

/// Whether the finished conversation on screen has the person's steps to replay.
pub fn savable_mission_id<'a>(id: Option<&'a str>, phase: Option<&str>, running: bool, has_draft: bool) -> Option<&'a str> {
    id.filter(|id| !id.starts_with("pending:") && !running && phase == Some("completed") && has_draft)
}

#[cfg(test)]
#[path = "savable_conversations_tests.rs"]
mod savable_conversations_tests;

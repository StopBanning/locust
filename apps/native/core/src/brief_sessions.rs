//! What each CLI session has been told (Electron `main/brief-sessions.ts` and the A2.5 half of
//! `workroom-briefing.ts`): a resumed turn is sent only the paragraphs of the standing brief its session does not
//! already hold, after one line that says the rest still holds.
//!
//! The runtime's own session is the truth and cannot be read back, so this is the host's account of it: per mission,
//! the keys of every brief paragraph that mission's session has been sent, and how many turns ago it was last briefed
//! in full. Every doubt resolves to a full brief -- no record, an unreadable file, a cold start, a session that
//! compacted, or `FULL_BRIEF_EVERY` turns gone by. A turn told too much costs characters; one told too little can lose
//! the reply formats, so the record only ever takes paragraphs away when it is sure.

use crate::briefing::{PeerEntry, SECTION_GAP, describe_peer};
use crate::store::Store;
use serde_json::{Value, json};
use std::collections::HashSet;

/// A session is briefed in full at least this often, whatever the record says: a runtime that compacts without
/// saying so (Cursor reports nothing) still gets the whole brief back within a few turns.
pub const FULL_BRIEF_EVERY: usize = 8;
/// Missions remembered; the oldest go first. A forgotten one is briefed in full.
const MAX_SESSIONS: usize = 500;
/// Keys per session; beyond this the ones no brief has stood on longest go first.
const MAX_KEYS: usize = 400;
/// Machine-local: what this computer's CLI sessions were sent. Never backed up (profile_backup::LEFT_OUT).
pub const BRIEF_SESSIONS_FILE: &str = "brief-sessions.json";

/// Said above a memory list sent again to a session that holds an earlier one (0.418).
pub const MEMORY_LIST_REPLACES: &str = "The list of what is remembered below replaces the one you were given earlier in this conversation. A memory on the earlier list that is not on this one was changed or forgotten by the person: do not use it or quote it.";
pub const ABOUT_YOU_REPLACES: &str = "The note below replaces the one about the person you were given earlier in this conversation.";
const ABOUT_YOU_HEADING: &str = "About the person you work for, in their own words:";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BriefRecord {
    /// `paragraph_key`s of the brief paragraphs the session holds.
    pub given: Vec<String>,
    /// Turns since the session was last briefed in full: 0 on that turn.
    pub turns: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BriefPlan {
    pub already_given: Option<HashSet<String>>,
    pub turns: usize,
}

/// A paragraph's key: the same words give the same key.
pub fn paragraph_key(paragraph: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(paragraph.as_bytes()))[..16].to_owned()
}

/// Whether the runtime compacted its conversation during a run: every adapter that can tell raises
/// `<runtime>.context_compacted`. A compacted session keeps a summary, not the brief.
pub fn compacted_during(events: &[Value]) -> bool {
    events
        .iter()
        .any(|e| e["type"] == "adapter.diagnostic" && e["payload"]["code"].as_str().is_some_and(|c| c.ends_with(".context_compacted")))
}

/// What a turn may take as already told, and the turn count it records. `resumes` is whether the turn runs in the
/// earlier turn's CLI session -- a cold start (no session, a changed mode, another runtime) holds nothing.
pub fn brief_plan(resumes: bool, compacted: bool, earlier: Option<&BriefRecord>) -> BriefPlan {
    let Some(earlier) = earlier.filter(|_| resumes && !compacted) else {
        return BriefPlan { already_given: None, turns: 0 };
    };
    let turns = earlier.turns + 1;
    if turns >= FULL_BRIEF_EVERY {
        return BriefPlan { already_given: None, turns: 0 };
    }
    BriefPlan { already_given: Some(earlier.given.iter().cloned().collect()), turns }
}

/// What the session holds after this turn: what it held, and what this brief stands on, newest last, so the bound
/// drops what no brief has used longest. `held` is in no order of its own, so it is sorted to keep the file stable.
pub fn brief_held(already_given: Option<&HashSet<String>>, given: &[String]) -> Vec<String> {
    let mut now: Vec<String> = Vec::new();
    for key in given {
        if !now.contains(key) {
            now.push(key.clone());
        }
    }
    let mut kept: Vec<String> = already_given.into_iter().flatten().filter(|k| !now.contains(k)).cloned().collect();
    kept.sort();
    kept.extend(now);
    let skip = kept.len().saturating_sub(MAX_KEYS);
    kept.split_off(skip)
}

/// The line a resumed turn gets in place of what its session already holds. It names the block tags the brief
/// teaches, because a runtime that quietly lost the session still has the tags to answer with.
pub fn still_holds_line(standing: &str, identity: Option<(&PeerEntry, &[PeerEntry])>) -> String {
    let mut tags: Vec<String> = Vec::new();
    if standing.contains("<locust-share ") {
        tags.push("<locust-share to=\"Name\">".into());
    }
    for tag in ["locust-ask", "locust-file", "locust-memory"] {
        if standing.contains(&format!("<{tag}>")) {
            tags.push(format!("<{tag}>"));
        }
    }
    let blocks = if tags.is_empty() { String::new() } else { format!(" and how to write the reply blocks ({})", tags.join(", ")) };
    let who = match identity {
        None => String::new(),
        Some((me, others)) if others.is_empty() => format!("You are {}. ", me.name),
        Some((me, others)) => format!(
            "You are {}; your teammates here are {}. ",
            me.name,
            others.iter().map(describe_peer).collect::<Vec<_>>().join(", ")
        ),
    };
    format!("{who}You were given standing instructions earlier in this conversation -- how to work here{blocks}. They still hold; only what changed since is repeated here.")
}

/// The standing brief for a turn: whole when the session holds none of it, and otherwise the paragraphs it does not
/// hold, after the line that says so. Answers the sections and the key of every paragraph the brief stands on.
pub fn standing_for(
    standing: &[String],
    already_given: Option<&HashSet<String>>,
    identity: Option<(&PeerEntry, &[PeerEntry])>,
) -> (Vec<String>, Vec<String>) {
    let joined = standing.join(SECTION_GAP);
    let paragraphs: Vec<&str> = joined.split(SECTION_GAP).filter(|p| !p.trim().is_empty()).collect();
    let given: Vec<String> = paragraphs.iter().map(|p| paragraph_key(p)).collect();
    let Some(held) = already_given else {
        return (standing.to_vec(), given);
    };
    let fresh: Vec<&str> = paragraphs.iter().zip(&given).filter(|(_, k)| !held.contains(*k)).map(|(p, _)| *p).collect();
    // Nothing held yet is a first brief, said whole rather than behind a line claiming an earlier one.
    if fresh.len() == paragraphs.len() {
        return (standing.to_vec(), given);
    }
    let mut sections = vec![still_holds_line(&joined, identity)];
    sections.extend(fresh.into_iter().map(|p| {
        if p.starts_with(crate::people::MEMORY_HEADING) {
            format!("{MEMORY_LIST_REPLACES}\n{p}")
        } else if p.starts_with(ABOUT_YOU_HEADING) {
            format!("{ABOUT_YOU_REPLACES}\n{p}")
        } else {
            p.to_owned()
        }
    }));
    (sections, given)
}

fn key_reads(key: &Value) -> bool {
    key.as_str().is_some_and(|k| k.len() == 16 && k.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
}

/// The record on disk, in the native data folder. Unreadable reads as empty: the only cost of forgetting is a full
/// brief, and a turn must never be refused over it.
pub struct BriefSessions<'a> {
    store: &'a Store,
}

impl<'a> BriefSessions<'a> {
    pub fn for_store(store: &'a Store) -> Self {
        Self { store }
    }

    fn read(&self) -> Value {
        let path = self.store.root().join(BRIEF_SESSIONS_FILE);
        std::fs::read(&path)
            .ok()
            .filter(|b| b.len() <= 4 * 1024 * 1024)
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
            .filter(|v| v["version"] == 1 && v["sessions"].is_object())
            .unwrap_or_else(|| json!({"version": 1, "sessions": {}}))
    }

    /// What the session of `mission` holds; `None` when there is nothing to trust.
    pub fn after(&self, mission: &str) -> Option<BriefRecord> {
        let file = self.read();
        let held = &file["sessions"][mission];
        let given = held["given"].as_array().filter(|keys| keys.iter().all(key_reads))?;
        let turns = held["turns"].as_u64()?;
        held["at"].as_str()?;
        Some(BriefRecord { given: given.iter().filter_map(|k| k.as_str().map(str::to_owned)).collect(), turns: turns as usize })
    }

    /// What the session of `mission` holds once its turn is sent. Oldest missions out first.
    pub fn record(&self, mission: &str, record: &BriefRecord) -> Result<(), String> {
        let file = self.read();
        let mut sessions: Vec<(String, Value)> = file["sessions"]
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(id, _)| id.as_str() != mission)
            .map(|(id, v)| (id.clone(), v.clone()))
            .collect();
        // serde_json keeps keys sorted, so order is by when each was recorded.
        sessions.sort_by(|a, b| a.1["at"].as_str().unwrap_or("").cmp(b.1["at"].as_str().unwrap_or("")));
        let skip = record.given.len().saturating_sub(MAX_KEYS);
        let at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        sessions.push((mission.to_owned(), json!({"given": &record.given[skip..], "turns": record.turns, "at": at})));
        let drop = sessions.len().saturating_sub(MAX_SESSIONS);
        let kept: serde_json::Map<String, Value> = sessions.into_iter().skip(drop).collect();
        self.store.save_json(BRIEF_SESSIONS_FILE, &json!({"version": 1, "sessions": kept}))
    }
}

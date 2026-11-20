//! a-step-change-stands-in-for-an-unread-file.test.ts (3 cases): a file the host saw change but could not read is
//! shown as the one step that changed it (its diff and counts), after edits that failed too; when two steps changed it,
//! neither is the whole change, so the host's own word stands.
use super::*;
use crate::ledger::Transcript;
use serde_json::{Value, json};

const FOLDER: &str = "C:\\work\\shop";
const NOTES: &str = "C:\\work\\shop\\notes.txt";

struct Turn(Vec<Value>);
impl Turn {
    fn new() -> Self {
        let mut t = Turn(vec![]);
        t.push("run.started", json!({}));
        t
    }
    fn push(&mut self, kind: &str, mut payload: Value) -> &mut Self {
        payload["evidence"] = json!({"redacted": true});
        let n = self.0.len() + 1;
        self.0.push(json!({"recordType": "mission.event", "event": {"id": format!("e{n}"), "runId": "r", "missionId": "m", "sequence": n,
            "occurredAt": "2026-10-06T07:13:00.000Z", "sourceAdapter": "claude", "type": kind, "payload": payload}}));
        self
    }
    fn edit(&mut self, id: &str, to: &str) -> &mut Self {
        let patch = json!({"text": format!("--- {NOTES}\n+++ {NOTES}\n@@ -1,3 +1,3 @@\n line one\n-line two\n+{to}\n line three\n"), "truncated": false, "added": 1, "removed": 1});
        self.push("tool.started", json!({"itemId": id, "toolKind": "tool_use", "name": "Edit", "phase": "started"}));
        self.push("tool.started", json!({"itemId": id, "toolKind": "tool_use", "name": "Edit", "command": NOTES, "phase": "started"}));
        self.push("tool.completed", json!({"itemId": id, "toolKind": "tool_use", "name": "Edit", "command": NOTES, "phase": "completed", "patch": patch}))
    }
    fn failed_edit(&mut self, id: &str) -> &mut Self {
        self.push("tool.started", json!({"itemId": id, "toolKind": "tool_use", "name": "Edit", "command": NOTES, "phase": "started"}));
        self.push("tool.failed", json!({"itemId": id, "toolKind": "tool_use", "name": "Edit", "command": NOTES, "phase": "completed", "status": "error"}))
    }
    fn ended_then_unread(&mut self) -> &mut Self {
        self.push("run.completed", json!({}));
        let status = "reported by the runtime, changed on disk";
        self.push("tool.started", json!({"itemId": "o1", "toolKind": "observed_edit", "name": "edit", "command": NOTES, "status": status, "phase": "started"}));
        self.push("tool.completed", json!({"itemId": "o1", "toolKind": "observed_edit", "name": "edit", "command": NOTES, "status": status, "phase": "completed"}))
    }
    fn items(&self) -> Vec<Item> {
        let mut t = Transcript::default();
        t.apply(self.0.clone());
        t.items
    }
}

/// The foot's rows as Electron's test prints them: a file with its counts, or a file with no diff and the host's word.
fn files(items: &[Item]) -> Vec<String> {
    turn_foot(items, Some(FOLDER)).map(|f| f.rows).unwrap_or_default().iter().map(|row| match row {
        Row::File { entry, .. } => format!("{} +{}/-{}", entry.file.path, entry.counts.0, entry.counts.1),
        Row::Unreported { path, observed, .. } => format!("{path} {}", if *observed { "seen on disk" } else { "unreported" }),
    }).collect()
}

fn is_notes(row: &str, tail: &str) -> bool {
    row.replace('\\', "/").ends_with(&format!("notes.txt {tail}"))
}

// case: reads as the one step that changed it
#[test]
fn reads_as_the_one_step_that_changed_it() {
    let rows = files(Turn::new().edit("t1", "line 2").ended_then_unread().items().as_slice());
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert!(is_notes(&rows[0], "+1/-1"), "{rows:?}");
}

// case: reads as the step that landed, after edits that failed (Haiku, 2026-10-06: two Edits before reading)
#[test]
fn reads_as_the_step_that_landed_after_edits_that_failed() {
    let items = Turn::new().failed_edit("f1").failed_edit("f2").edit("t1", "line 2").ended_then_unread().items();
    let rows = files(&items);
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert!(is_notes(&rows[0], "+1/-1"), "{rows:?}");
    let edits: Vec<bool> = items.iter().filter_map(|i| match i {
        Item::Tool { name, status, .. } if name == "Edit" => Some(*status == Status::Failed),
        _ => None,
    }).collect();
    let seen: Vec<String> = items.iter().filter_map(|i| match i { Item::Tool { name, status, detail, .. } => Some(format!("{name} {} {status:?} {}", detail.id, detail.runtime_status)), _ => None }).collect();
    assert_eq!(edits, [true, true, false], "{seen:?}");
}

// case: keeps the host's word when two steps changed it: neither is the whole change
#[test]
fn keeps_the_hosts_word_when_two_steps_changed_it() {
    let rows = files(Turn::new().edit("t1", "line 2").edit("t2", "line II").ended_then_unread().items().as_slice());
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert!(is_notes(&rows[0], "seen on disk"), "{rows:?}");
}

/// netFileEntries' last pass, beyond the three cases: two steps that changed a file the host never looked at are still
/// one row on the card, the first step's.
#[test]
fn two_steps_on_one_file_the_host_never_read_are_one_row() {
    let mut turn = Turn::new();
    turn.edit("t1", "line 2").edit("t2", "line II").push("run.completed", json!({}));
    let rows = files(&turn.items());
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert!(is_notes(&rows[0], "+1/-1"), "{rows:?}");
}

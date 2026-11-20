//! `two-files-with-one-name-stay-two.test.ts` (H10, the review's scenario): the host's observation of a changed file
//! was matched to the runtime's row by basename, and every other row of that basename was then removed -- so
//! packages/a/package.json and packages/b/package.json were one row showing b's diff, "1 file".
use crate::activity_entries::{FileEntry, entries};
use crate::activity_summary::activity_summary;
use crate::ledger::Transcript;
use serde_json::{Value, json};

fn ev(seq: u64, kind: &str, payload: Value) -> Value {
    json!({"id": format!("e{seq}"), "runId": "run_1", "missionId": "mission_1", "sequence": seq,
        "occurredAt": "2026-09-24T12:00:00.000Z", "sourceAdapter": "claude", "type": kind, "payload": payload})
}

fn patch(path: &str) -> Value {
    json!({"text": format!("--- a/{path}\n+++ b/{path}\n@@ -2 +2 @@\n-  \"version\": \"1.0.0\"\n+  \"version\": \"1.0.1\"\n"),
        "added": 1, "removed": 1, "truncated": false})
}

const REPORTED: &str = "reported by the runtime, read from disk";

fn run() -> Vec<Value> {
    let mut events = Vec::new();
    let mut seq = 0;
    for (id, pkg) in [("a", "packages/a/package.json"), ("b", "packages/b/package.json")] {
        let absolute = format!("C:\\work\\repo\\{}", pkg.replace('/', "\\"));
        seq += 1;
        events.push(ev(seq, "tool.started", json!({"itemId": format!("edit_{id}"), "toolKind": "edit", "name": "edit", "command": absolute, "phase": "started"})));
        seq += 1;
        events.push(ev(seq, "tool.completed", json!({"itemId": format!("edit_{id}"), "toolKind": "edit", "name": "edit", "command": absolute, "phase": "completed", "patch": patch(pkg)})));
    }
    for (id, pkg) in [("a", "packages/a/package.json"), ("b", "packages/b/package.json")] {
        seq += 1;
        events.push(ev(seq, "tool.started", json!({"itemId": format!("obs_{id}"), "toolKind": "observed_edit", "name": "edit", "command": pkg, "status": REPORTED, "phase": "started"})));
        seq += 1;
        events.push(ev(seq, "tool.completed", json!({"itemId": format!("obs_{id}"), "toolKind": "observed_edit", "name": "edit", "command": pkg, "status": REPORTED, "phase": "completed", "patch": patch(pkg)})));
    }
    events
}

fn transcript(events: &[Value]) -> Transcript {
    let mut t = Transcript::default();
    t.apply(events.iter().map(|e| json!({"recordType": "mission.event", "event": e})).collect());
    t
}

fn check(workspace: Option<&str>) {
    let t = transcript(&run());
    let card: Vec<(usize, FileEntry)> = entries(&t.items, workspace);
    let mut paths: Vec<String> = card.iter().map(|(_, f)| f.file.path.replace('\\', "/").to_lowercase()).collect();
    paths.sort();
    assert_eq!(paths, vec!["packages/a/package.json".to_owned(), "packages/b/package.json".to_owned()], "two files, two rows");
    let lines: usize = card.iter().map(|(_, f)| f.counts.0 + f.counts.1).sum();
    assert_eq!(lines, 4, "each file's own change is counted");
    assert_eq!(activity_summary(&t.items), "Edited 2 files");
}

#[test]
fn are_two_rows_each_with_its_own_change() {
    check(Some("C:\\work\\repo"));
}

#[test]
fn are_two_rows_each_with_its_own_change_no_workspace_known() {
    check(None);
}

//! Electron's `main/a-usage-reading-outlives-the-newest-twenty.test.ts` (0.571), a case to a test with its values. Colin,
//! 2026-10-03: "claude code on home stopped showing usage". His newest Claude conversation was the fortieth most recent,
//! behind an evening of Codex runs, and the reading was read off the newest twenty only. Each agent's newest reading is
//! now kept in `usage-readings.json`, and a newer one takes its place.

use crate::store::Store;
use crate::usage_ledger::usage_windows;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn temp() -> (Temp, Store) {
    let p = std::env::temp_dir().join(format!("locust-usage-kept-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    let store = Store::new(p.join("native"), p.join("electron")).unwrap();
    (Temp(p), store)
}

/// Electron's `mission()`: a finished mission on `runtime`'s account default, saying `usage` when there is one.
fn mission(store: &Store, id: &str, runtime: &str, at: &str, usage: Option<&str>) {
    let created = json!({"schemaVersion": 27, "recordType": "mission.created", "metadata": {
        "missionId": id, "runId": format!("run_{id}"), "prompt": "p", "runtime": runtime, "model": "account-default",
        "requestedRouteId": runtime, "resolvedRouteId": format!("{runtime}-account:default"), "cliVersion": "1",
        "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": at}});
    let event = match usage {
        None => json!({"type": "noise"}),
        Some(said) => json!({"id": format!("e_{id}"), "type": "adapter.diagnostic", "sourceAdapter": runtime, "occurredAt": at,
            "payload": {"level": "info", "code": format!("{runtime}.usage_window"), "message": said}}),
    };
    let records = [created, json!({"schemaVersion": 27, "recordType": "mission.event", "event": event})];
    let dir = store.root().join("mission-ledger");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(format!("{id}.jsonl")), records.iter().map(|r| format!("{r}\n")).collect::<String>()).unwrap();
}

/// Thirty-nine newer Codex turns, then the Claude run that last said its window.
fn an_evening_of_codex(store: &Store) {
    for index in 0..39 {
        mission(store, &format!("mission_codex_{index}"), "codex", "2026-10-03T21:00:00.000Z", None);
    }
}

fn kept(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn a_reading_past_the_newest_twenty_conversations_is_shown_and_kept() {
    let (_temp, store) = temp();
    an_evening_of_codex(&store);
    mission(&store, "mission_claude", "claude", "2026-10-03T17:08:00.000Z", Some("5-hour window 41% used · resets 2026-10-03T22:00:00.000Z"));
    let windows = usage_windows(&store);
    assert!(windows["claude"].starts_with("5-hour window 41% used"), "{windows:?}");
    let file = store.root().join("usage-readings.json");
    assert!(kept(&file)["claude"].as_str().unwrap().contains("41% used"));
    // Gone even from the ledger, the kept reading still answers.
    fs::remove_file(store.root().join("mission-ledger/mission_claude.jsonl")).unwrap();
    assert!(usage_windows(&store)["claude"].contains("41% used"));
}

#[test]
fn a_kept_reading_gives_way_to_a_newer_one() {
    let (_temp, store) = temp();
    an_evening_of_codex(&store);
    mission(&store, "mission_claude", "claude", "2026-10-03T17:08:00.000Z", Some("5-hour window 41% used · resets 2026-10-03T22:00:00.000Z"));
    usage_windows(&store);
    fs::remove_file(store.root().join("mission-ledger/mission_claude.jsonl")).unwrap();
    mission(&store, "mission_claude_2", "claude", "2026-10-03T21:30:00.000Z", Some("5-hour window 63% used · resets 2026-10-03T22:00:00.000Z"));
    assert!(usage_windows(&store)["claude"].contains("63% used"));
}

// Carries, where Rust has the behaviour, packages/mission-store/test/mission-store.test.ts (Electron 4ce2f37a): a
// mission's ledger written by `LedgerWriter`, read back by `mission_export::recover_mission_from_path` (Electron's
// `getMission`) and listed by `ledger::try_list_recent` (`listMissions`), and deleted by `trash::delete_mission`.
// Electron makes its events with its Codex `exec --json` reader, which Rust does not have; the four here are the kinds
// it emits for that run (a start, the answer, the turn's end, the run's end), written out.
use crate::{
    ledger::try_list_recent, ledger_write::LedgerWriter, mission_export::recover_mission_from_path, store::Store,
    trash::delete_mission,
};
use serde_json::{Value, json};
use std::{io::Write, path::PathBuf};

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn temp() -> (Temp, Store) {
    let root = std::env::temp_dir().join(format!("locust-mission-ledger-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    (Temp(root), store)
}
fn create(store: &Store, id: &str) -> LedgerWriter {
    LedgerWriter::create_runtime(store, id, "Inspect the workspace without changing it.", "account-default", "codex", "codex", &[], None).unwrap()
}
fn successful() -> Vec<(&'static str, Value)> {
    vec![
        ("run.started", json!({ "runtimeThreadId": "thread_1" })),
        ("message.delta", json!({ "itemId": "answer", "operation": "replace", "text": "Durable result", "final": true })),
        ("step.completed", json!({ "stepKind": "turn", "itemId": "turn" })),
        ("run.completed", json!({ "runtimeThreadId": "thread_1" })),
    ]
}
fn append(writer: &mut LedgerWriter, events: &[(&str, Value)]) {
    for (kind, payload) in events {
        writer.append(kind, payload.clone(), Some("thread_1")).unwrap();
    }
}
fn issues(mission: &Value) -> Vec<&str> {
    mission["issues"].as_array().unwrap().iter().filter_map(Value::as_str).collect()
}

#[test]
fn a_mission_is_kept_versioned_and_append_only_and_recovers_its_terminal_phase() {
    let (_t, store) = temp();
    let mut writer = create(&store, "mission_1");
    let events = successful();
    append(&mut writer, &events[..2]);
    append(&mut writer, &events[2..]);
    let path = writer.path.clone();
    drop(writer);
    let mission = recover_mission_from_path(&path).unwrap();
    assert!(issues(&mission).is_empty(), "{mission}");
    assert_eq!(mission["phase"], "completed");
    assert_eq!(mission["ledgerSequence"], events.len() + 1);
    assert_eq!(mission["events"].as_array().unwrap().len(), events.len());
    let listed = try_list_recent(&store.ledger_dir().unwrap(), 100).unwrap();
    assert_eq!(listed.len(), 1);
    let text = std::fs::read_to_string(&path).unwrap();
    let lines: Vec<Value> = text.trim_end().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(), events.len() + 1);
    assert_eq!((&lines[0]["schemaVersion"], &lines[0]["recordType"], &lines[0]["ledgerSequence"]), (&json!(27), &json!("mission.created"), &json!(1)));
    let sequences: Vec<u64> = lines.iter().map(|l| l["ledgerSequence"].as_u64().unwrap()).collect();
    assert_eq!(sequences, (1..=events.len() as u64 + 1).collect::<Vec<_>>());
}

// The reading half of "ignores an incomplete crash tail and reports the recovered prefix".
#[test]
fn an_incomplete_crash_tail_is_left_out_and_the_prefix_reported() {
    let (_t, store) = temp();
    let mut writer = create(&store, "mission_1");
    append(&mut writer, &successful()[..2]);
    let path = writer.path.clone();
    drop(writer);
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(br#"{"schemaVersion":1,"recordType":"mission.event""#).unwrap();
    let mission = recover_mission_from_path(&path).unwrap();
    assert_eq!(mission["events"].as_array().unwrap().len(), 2);
    assert_eq!(mission["phase"], "interrupted");
    assert!(issues(&mission).contains(&"truncated-tail"), "{mission}");
}

// The listing half of "does not trust unsupported or malformed ledger files".
#[test]
fn a_ledger_of_an_unsupported_version_or_malformed_lists_no_mission() {
    let (_t, store) = temp();
    let dir = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    let future = json!({ "schemaVersion": 99, "recordType": "mission.created", "ledgerSequence": 1, "metadata": {
        "missionId": "mission_future", "runId": "run_future", "prompt": "Inspect the workspace without changing it.", "runtime": "codex",
        "model": "account-default", "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default", "cliVersion": "0.151.0-alpha.7.2",
        "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": "2026-08-31T16:00:00.000Z" } });
    std::fs::write(dir.join("mission_future.jsonl"), format!("{future}\n")).unwrap();
    std::fs::write(dir.join("mission_malformed.jsonl"), "not-json\n").unwrap();
    assert!(try_list_recent(&dir, 100).unwrap().is_empty());
}

#[test]
fn a_ledger_changed_outside_its_writer_is_refused_and_stays_refused() {
    let (_t, store) = temp();
    let mut writer = create(&store, "mission_1");
    let events = successful();
    append(&mut writer, &events[..2]);
    std::fs::OpenOptions::new().append(true).open(&writer.path).unwrap().write_all(b"{\"intruder\":true}\n").unwrap();
    let (kind, payload) = &events[2];
    assert_eq!(writer.append(kind, payload.clone(), None).unwrap_err().to_string(), "Mission ledger changed outside the active writer");
    assert_eq!(writer.append(kind, payload.clone(), None).unwrap_err().to_string(), "Mission ledger is unavailable");
    let path = writer.path.clone();
    drop(writer);
    let mission = recover_mission_from_path(&path).unwrap();
    assert_eq!(mission["events"].as_array().unwrap().len(), 2);
    assert!(issues(&mission).contains(&"invalid-record"), "{mission}");
}

// The first half of "reads the most recently updated ledgers rather than whichever sort first by name": 2,001 files,
// the newest last by name. Rust stats every file and has no cap, so there is no file-limit issue to say.
#[test]
fn the_most_recently_updated_ledger_is_listed_though_it_sorts_last_by_name() {
    let (_t, store) = temp();
    let dir = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    let header = |id: &str| json!({ "schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": "2026-08-31T16:00:00.000Z",
        "metadata": { "missionId": id, "runId": "run_1", "prompt": "Inspect the workspace without changing it.", "runtime": "codex",
            "model": "account-default", "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default", "cliVersion": "0.151.0-alpha.7.2",
            "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": "2026-08-31T16:00:00.000Z" } });
    let now = std::time::SystemTime::now();
    let write = |id: &str, at: std::time::SystemTime| {
        let path = dir.join(format!("{id}.jsonl"));
        std::fs::write(&path, format!("{}\n", header(id))).unwrap();
        std::fs::OpenOptions::new().write(true).open(&path).unwrap().set_modified(at).unwrap();
    };
    for i in 0..2_000 {
        write(&format!("mission_a{i:04}"), now - std::time::Duration::from_secs(10_000));
    }
    write("mission_zzz_newest", now);
    let listed = try_list_recent(&dir, 5).unwrap();
    assert!(listed.iter().any(|m| m.mission_id == "mission_zzz_newest"));
}

#[test]
fn deleting_a_mission_takes_it_out_and_says_whether_there_was_one() {
    let (_t, store) = temp();
    let path = create(&store, "mission_1").path.clone();
    let dir = store.ledger_dir().unwrap();
    assert_eq!(delete_mission(&dir, "mission_1", &[]), Ok(true));
    assert!(recover_mission_from_path(&path).is_none());
    assert!(try_list_recent(&dir, 100).unwrap().is_empty());
    assert_eq!(delete_mission(&dir, "mission_1", &[]), Ok(false));
}

// The second half of "refuses to append to a mission that was deleted": the same id, created again, is fresh.
#[test]
fn a_mission_created_again_under_a_deleted_id_is_fresh() {
    let (_t, store) = temp();
    drop(create(&store, "mission_1"));
    delete_mission(&store.ledger_dir().unwrap(), "mission_1", &[]).unwrap();
    let mut again = create(&store, "mission_1");
    append(&mut again, &successful()[..1]);
    let path = again.path.clone();
    drop(again);
    let mission = recover_mission_from_path(&path).unwrap();
    assert!(issues(&mission).is_empty(), "{mission}");
    assert_eq!(mission["events"].as_array().unwrap().len(), 1);
}

#[test]
fn deleting_a_mission_leaves_every_other_mission_untouched() {
    let (_t, store) = temp();
    drop(create(&store, "mission_1"));
    drop(create(&store, "mission_2"));
    let dir = store.ledger_dir().unwrap();
    delete_mission(&dir, "mission_1", &[]).unwrap();
    let listed: Vec<String> = try_list_recent(&dir, 100).unwrap().into_iter().map(|m| m.mission_id).collect();
    assert_eq!(listed, ["mission_2"]);
}

#[test]
fn deleting_refuses_an_id_that_could_name_any_other_file() {
    let (_t, store) = temp();
    assert!(delete_mission(&store.ledger_dir().unwrap(), "../escape", &[]).is_err());
}

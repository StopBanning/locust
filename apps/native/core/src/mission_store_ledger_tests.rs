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

// case: persists a versioned append-only mission and recovers its terminal phase
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

// case: ignores an incomplete crash tail and reports the recovered prefix
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
    let err = crate::ledger_write::append_events(&path, |run, mission, ad, seq, at| {
        vec![json!({"id":format!("{run}:{seq}"),"runId":run,"missionId":mission,"sequence":seq,"occurredAt":at,"sourceAdapter":ad,"type":"run.completed","payload":{}})]
    }).unwrap_err();
    assert!(err.to_string().contains("Mission ledger is unavailable"), "{err}");
}

// case: does not trust unsupported or malformed ledger files
#[test]
fn a_ledger_of_an_unsupported_version_or_malformed_lists_no_mission() {
    let (_t, store) = temp();
    let dir = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    let future = json!({ "schemaVersion": 99, "recordType": "mission.created", "ledgerSequence": 1, "metadata": {
        "missionId": "mission_future", "runId": "run_future", "prompt": "Inspect the workspace without changing it.", "runtime": "codex",
        "model": "account-default", "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default", "cliVersion": "0.151.0-alpha.7.2",
        "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": "2026-08-31T16:00:00.000Z" } });
    let future_path = dir.join("mission_future.jsonl");
    let malformed_path = dir.join("mission_malformed.jsonl");
    std::fs::write(&future_path, format!("{future}\n")).unwrap();
    std::fs::write(&malformed_path, "not-json\n").unwrap();
    assert!(try_list_recent(&dir, 100).unwrap().is_empty());
    let fut = crate::ledger::read_ledger(&future_path).unwrap();
    assert!(fut.unreadable);
    assert_eq!(fut.issues, vec![crate::ledger::Issue::UnsupportedSchema]);
    let mal = crate::ledger::read_ledger(&malformed_path).unwrap();
    assert!(mal.unreadable);
    assert_eq!(mal.issues, vec![crate::ledger::Issue::InvalidRecord]);
}

// case: detects external modification on append and fails closed afterwards
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

// case: reads the most recently updated ledgers rather than whichever sort first by name
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

// case: removes the record for good and reports whether there was one
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

// case: refuses to append to a mission that was deleted, rather than resurrecting it
#[test]
fn a_mission_created_again_under_a_deleted_id_is_fresh() {
    let (_t, store) = temp();
    let path = create(&store, "mission_1").path.clone();
    delete_mission(&store.ledger_dir().unwrap(), "mission_1", &[]).unwrap();
    assert!(crate::ledger_write::append_host_failure(&path, "runtime-transport-failed", "late", "2026-08-31T16:00:00.000Z").is_err());
    assert!(recover_mission_from_path(&path).is_none());
    let mut again = create(&store, "mission_1");
    append(&mut again, &successful()[..1]);
    let path = again.path.clone();
    drop(again);
    let mission = recover_mission_from_path(&path).unwrap();
    assert!(issues(&mission).is_empty(), "{mission}");
    assert_eq!(mission["events"].as_array().unwrap().len(), 1);
}

// case: leaves every other mission untouched
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

// case: refuses an id that could name any other file
#[test]
fn deleting_refuses_an_id_that_could_name_any_other_file() {
    let (_t, store) = temp();
    assert!(delete_mission(&store.ledger_dir().unwrap(), "../escape", &[]).is_err());
}

// case: serializes concurrent appends without losing ledger or event order
#[test]
fn serializes_concurrent_appends_without_losing_ledger_or_event_order() {
    let (_t, store) = temp();
    let path = create(&store, "mission_1").path.clone();
    let p1 = path.clone();
    let p2 = path.clone();
    let t1 = std::thread::spawn(move || {
        crate::ledger_write::append_events(&p1, |run, mission, ad, seq, at| {
            vec![
                json!({"id":format!("{run}:{seq}"),"runId":run,"missionId":mission,"sequence":seq,"occurredAt":at,"sourceAdapter":ad,"type":"run.started","payload":{"runtimeThreadId":"thread_1"}}),
                json!({"id":format!("{run}:{}", seq + 1),"runId":run,"missionId":mission,"sequence":seq + 1,"occurredAt":at,"sourceAdapter":ad,"type":"message.delta","payload":{"itemId":"answer","operation":"replace","text":"Durable result","final":true}}),
            ]
        }).unwrap();
    });
    let t2 = std::thread::spawn(move || {
        crate::ledger_write::append_events(&p2, |run, mission, ad, seq, at| {
            vec![
                json!({"id":format!("{run}:{seq}"),"runId":run,"missionId":mission,"sequence":seq,"occurredAt":at,"sourceAdapter":ad,"type":"step.completed","payload":{"stepKind":"turn","itemId":"turn"}}),
                json!({"id":format!("{run}:{}", seq + 1),"runId":run,"missionId":mission,"sequence":seq + 1,"occurredAt":at,"sourceAdapter":ad,"type":"run.completed","payload":{"runtimeThreadId":"thread_1"}}),
            ]
        }).unwrap();
    });
    t1.join().unwrap();
    t2.join().unwrap();
    let read = crate::ledger::read_ledger(&path).unwrap();
    assert!(!read.unreadable && read.issues.is_empty(), "{read:?}");
    let seqs: Vec<u64> = read.records[1..].iter().map(|r| r["event"]["sequence"].as_u64().unwrap()).collect();
    assert_eq!(seqs, [1, 2, 3, 4]);
    let lseqs: Vec<u64> = read.records.iter().map(|r| r["ledgerSequence"].as_u64().unwrap()).collect();
    assert_eq!(lseqs, [1, 2, 3, 4, 5]);
}

// case: rejects a mismatched or noncontiguous event before writing it
#[test]
fn rejects_a_mismatched_or_noncontiguous_event_before_writing_it() {
    let (_t, store) = temp();
    let path = create(&store, "mission_1").path.clone();
    let bad_seq = crate::ledger_write::append_events(&path, |run, mission, ad, _seq, at| {
        vec![json!({"id":format!("{run}:2"),"runId":run,"missionId":mission,"sequence":2,"occurredAt":at,"sourceAdapter":ad,"type":"run.started","payload":{}})]
    }).unwrap_err();
    assert!(bad_seq.to_string().contains("Mission event sequence is invalid"), "{bad_seq}");
    let bad_corr = crate::ledger_write::append_events(&path, |run, _mission, ad, seq, at| {
        vec![json!({"id":format!("{run}:{seq}"),"runId":run,"missionId":"mission_other","sequence":seq,"occurredAt":at,"sourceAdapter":ad,"type":"run.started","payload":{}})]
    }).unwrap_err();
    assert!(bad_corr.to_string().contains("correlation"), "{bad_corr}");
    assert_eq!(recover_mission_from_path(&path).unwrap()["events"], json!([]));
}

// case: records a generic host launch failure as a failed recovered mission
#[test]
fn records_a_generic_host_launch_failure_as_a_failed_recovered_mission() {
    let (_t, store) = temp();
    let path = create(&store, "mission_1").path.clone();
    crate::ledger_write::append_host_failure(
        &path,
        "runtime-start-failed",
        "The Codex process could not be started safely.",
        "2026-08-31T16:00:00.000Z",
    ).unwrap();
    let recovered = recover_mission_from_path(&path).unwrap();
    assert_eq!(recovered["phase"], "failed");
    assert_eq!(recovered["hostFailures"], json!([{
        "code": "runtime-start-failed",
        "message": "The Codex process could not be started safely.",
        "occurredAt": "2026-08-31T16:00:00.000Z",
    }]));
}

// case: stops recovery at a noncontiguous body record and refuses to extend the corrupt ledger
#[test]
fn stops_recovery_at_a_noncontiguous_body_record_and_refuses_to_extend_the_corrupt_ledger() {
    let (_t, store) = temp();
    let mut writer = create(&store, "mission_1");
    append(&mut writer, &successful()[..2]);
    let path = writer.path.clone();
    drop(writer);
    let jump = json!({
        "schemaVersion": 27, "recordType": "mission.event", "ledgerSequence": 5,
        "occurredAt": "2026-08-31T16:00:00.000Z",
        "event": {"id":"e3","runId":"run_mission_1","missionId":"mission_1","sequence":3,"occurredAt":"2026-08-31T16:00:00.000Z","sourceAdapter":"codex","type":"step.completed","payload":{"stepKind":"turn"}}
    });
    std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(format!("{jump}\n").as_bytes()).unwrap();
    let read = crate::ledger::read_ledger(&path).unwrap();
    assert_eq!(read.records.len(), 3); // header + 2 valid events
    assert_eq!(read.records.last().unwrap()["ledgerSequence"], 3);
    assert!(read.issues.contains(&crate::ledger::Issue::InvalidRecord));
    let err = crate::ledger_write::append_events(&path, |run, mission, ad, seq, at| {
        vec![json!({"id":format!("{run}:{seq}"),"runId":run,"missionId":mission,"sequence":seq,"occurredAt":at,"sourceAdapter":ad,"type":"run.completed","payload":{}})]
    }).unwrap_err();
    assert!(err.to_string().contains("Mission ledger is unavailable"), "{err}");
}

// case: orders history by recency and honors the list limit
#[test]
fn orders_history_by_recency_and_honors_the_list_limit() {
    let (_t, store) = temp();
    let dir = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    let base = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_788_000_000);
    for (idx, (id, at)) in [
        ("mission_a", "2026-08-31T10:00:00.000Z"),
        ("mission_b", "2026-08-31T12:00:00.000Z"),
        ("mission_c", "2026-08-31T11:00:00.000Z"),
    ].into_iter().enumerate() {
        let line = json!({
            "schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": at,
            "metadata": {
                "missionId": id, "runId": format!("run_{id}"), "prompt": "Inspect.", "runtime": "codex",
                "model": "account-default", "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default",
                "cliVersion": "0.151.0", "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1,
                "createdAt": at
            }
        });
        let p = dir.join(format!("{id}.jsonl"));
        std::fs::write(&p, format!("{line}\n")).unwrap();
        std::fs::OpenOptions::new().write(true).open(&p).unwrap().set_modified(base + std::time::Duration::from_secs(idx as u64)).unwrap();
    }
    let limited: Vec<String> = try_list_recent(&dir, 2).unwrap().into_iter().map(|m| m.mission_id).collect();
    assert_eq!(limited, ["mission_b", "mission_c"]);
    let all: Vec<String> = try_list_recent(&dir, 10).unwrap().into_iter().map(|m| m.mission_id).collect();
    assert_eq!(all, ["mission_b", "mission_c", "mission_a"]);
}

// case: refuses an event its own reader would reject, rather than truncating recovery later
#[test]
fn refuses_an_event_its_own_reader_would_reject_rather_than_truncating_recovery_later() {
    let (_t, store) = temp();
    let mut writer = create(&store, "mission_1");
    let err1 = writer.append("message.delta", json!({"itemId":"   ","operation":"replace","text":"hi","final":true}), None).unwrap_err();
    assert!(err1.to_string().contains("Mission event is not readable by the ledger reader"), "{err1}");
    let err2 = writer.append("message.delta", json!({"itemId":"answer","operation":"prepend","text":"hi","final":true}), None).unwrap_err();
    assert!(err2.to_string().contains("Mission event is not readable by the ledger reader"), "{err2}");
    let path = writer.path.clone();
    assert_eq!(recover_mission_from_path(&path).unwrap()["events"], json!([]));
    let events = successful();
    append(&mut writer, &events);
    drop(writer);
    let recovered = recover_mission_from_path(&path).unwrap();
    assert_eq!(recovered["events"].as_array().unwrap().len(), events.len());
    assert_eq!(recovered["phase"], "completed");
    assert!(issues(&recovered).is_empty());
}

// A conversation's place in the list is when it was last updated, not when its file was last touched on disk.
#[test]
fn the_mission_updated_last_is_listed_first_even_when_its_file_is_older() {
    let (_t, store) = temp();
    let dir = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    let base = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_788_000_000);
    // mission_old's file is the oldest on disk, but it holds the newest conversation.
    for (idx, (id, at)) in [("mission_old", "2026-08-31T23:00:00.000Z"), ("mission_mid", "2026-08-31T10:00:00.000Z"), ("mission_new", "2026-08-31T11:00:00.000Z")]
        .into_iter()
        .enumerate()
    {
        let line = json!({
            "schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": at,
            "metadata": {
                "missionId": id, "runId": format!("run_{id}"), "prompt": "Inspect.", "runtime": "codex",
                "model": "account-default", "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default",
                "cliVersion": "0.151.0", "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1,
                "createdAt": at
            }
        });
        let p = dir.join(format!("{id}.jsonl"));
        std::fs::write(&p, format!("{line}\n")).unwrap();
        std::fs::OpenOptions::new().write(true).open(&p).unwrap().set_modified(base + std::time::Duration::from_secs(idx as u64)).unwrap();
    }
    let first: Vec<String> = try_list_recent(&dir, 1).unwrap().into_iter().map(|m| m.mission_id).collect();
    assert_eq!(first, ["mission_old"]);
}

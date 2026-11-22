// Carries, where Rust has the behaviour, packages/mission-store/test/schema-compat.test.ts (Electron 4ce2f37a): which
// ledger versions Rust reads and writes. Rust's readers (`ledger::Tail`, `ledger::metadata`,
// `mission_export::recover_mission_from_path`) take every version from 1 to 28, and refuse a mission.created record that
// claims what no writer of its version could write (`ledger::written_by_its_version`, mission-store `parsedMetadata`).
// Rust writes 28 for an Apple Intelligence run and 27 for every other, as Electron's `schemaVersionWritten`.
use crate::{
    ledger::{Tail, metadata},
    ledger_write::{LedgerWriter, append_note, append_peer_links},
    mission_export::recover_mission_from_path,
    store::Store,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const NOW: &str = "2026-08-31T16:00:00.000Z";

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn temp() -> (Temp, Store) {
    let root = std::env::temp_dir().join(format!("locust-schema-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    std::fs::create_dir_all(store.ledger_dir().unwrap()).unwrap();
    (Temp(root), store)
}
fn v1_metadata(extra: Value) -> Value {
    let mut m = json!({ "missionId": "mission_1", "runId": "run_1", "prompt": "Inspect the workspace without changing it.", "runtime": "codex",
        "model": "account-default", "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default", "cliVersion": "0.151.0-alpha.7.2",
        "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": NOW });
    for (k, v) in extra.as_object().unwrap() {
        m[k] = v.clone();
    }
    m
}
/// The events of Electron's `codexEvents()` (its Codex `exec --json` reader's), written out.
fn codex_events() -> Vec<Value> {
    let e = |sequence: u64, kind: &str, payload: Value| json!({ "id": format!("run_1:codex:{sequence}"), "runId": "run_1", "missionId": "mission_1",
        "sequence": sequence, "occurredAt": NOW, "sourceAdapter": "codex", "runtimeThreadId": "thread_1", "type": kind, "payload": payload });
    vec![
        e(1, "run.started", json!({ "runtimeThreadId": "thread_1" })),
        e(2, "message.delta", json!({ "itemId": "answer", "operation": "replace", "text": "Durable result", "final": true })),
        e(3, "step.completed", json!({ "stepKind": "turn", "itemId": "turn" })),
        e(4, "run.completed", json!({ "runtimeThreadId": "thread_1" })),
    ]
}
/// A ledger file exactly as version `schema` of the package wrote them.
fn ledger_at(store: &Store, schema: u64, events: &[Value], extra: Value) -> PathBuf {
    let path = store.ledger_dir().unwrap().join("mission_1.jsonl");
    let mut lines = vec![json!({ "schemaVersion": schema, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": NOW, "metadata": v1_metadata(extra) })];
    lines.extend(events.iter().enumerate().map(|(i, event)| json!({ "schemaVersion": schema, "recordType": "mission.event", "ledgerSequence": i + 2, "occurredAt": NOW, "event": event })));
    std::fs::write(&path, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
    path
}
fn written(store: &Store, extra: Value) -> PathBuf {
    let writer = LedgerWriter::create_runtime_with(store, "mission_1", "Inspect the workspace without changing it.", "account-default", "codex", "codex", &[], None, &extra).unwrap();
    writer.path.clone()
}
fn header(path: &Path) -> crate::ledger::Metadata {
    Tail::new(path, 0).poll().iter().find_map(metadata).expect("the header reads")
}

#[test]
fn new_missions_are_written_at_27_and_every_version_from_1_to_27_is_read() {
    let (_t, store) = temp();
    let path = written(&store, Value::Null);
    let first: Value = serde_json::from_str(std::fs::read_to_string(&path).unwrap().lines().next().unwrap()).unwrap();
    assert_eq!(first["schemaVersion"], 27);
    // The pair that drifted, checked as a pair: every version the writer has written reads back through a real file.
    for version in 1..=28 {
        let path = ledger_at(&store, version, &[], json!({}));
        assert_eq!(Tail::new(&path, 0).poll().len(), 1, "version {version}");
    }
}

#[test]
fn a_file_whose_version_the_reader_does_not_know_is_not_read() {
    let (_t, store) = temp();
    let path = ledger_at(&store, 29, &[], json!({}));
    assert!(Tail::new(&path, 0).poll().is_empty());
}

#[test]
fn a_mission_recorded_before_the_version_bump_still_recovers() {
    let (_t, store) = temp();
    let path = ledger_at(&store, 1, &codex_events(), json!({}));
    let mission = recover_mission_from_path(&path).unwrap();
    assert_eq!(mission["issues"], json!([]));
    assert_eq!(mission["phase"], "completed");
    assert_eq!(mission["events"].as_array().unwrap().len(), codex_events().len());
    assert_eq!(mission["metadata"]["runtime"], "codex");
}

// Rust appends to a finished turn's file through its after-turn writers; each writes at the file's own version.
#[test]
fn an_append_to_a_version_1_mission_is_written_in_version_1() {
    let (_t, store) = temp();
    let path = ledger_at(&store, 1, &codex_events()[..2], json!({}));
    append_note(&path, "host.carry_on", "Carried on.").unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let versions: Vec<u64> = text.trim_end().lines().map(|l| serde_json::from_str::<Value>(l).unwrap()["schemaVersion"].as_u64().unwrap()).collect();
    assert_eq!(versions, vec![1; versions.len()]);
    let mission = recover_mission_from_path(&path).unwrap();
    assert_eq!(mission["issues"], json!([]));
    assert_eq!(mission["events"].as_array().unwrap().len(), 3);
}

#[test]
fn a_continuation_round_trips_through_the_durable_record() {
    let (_t, store) = temp();
    let path = written(&store, json!({ "continuesFrom": { "missionId": "mission_before", "checkpointEpoch": 2, "reason": "route-switch" } }));
    assert_eq!(recover_mission_from_path(&path).unwrap()["metadata"]["continuesFrom"], json!({ "missionId": "mission_before", "checkpointEpoch": 2, "reason": "route-switch" }));
    let m = header(&path);
    assert_eq!((m.parent.as_deref(), m.checkpoint_epoch, m.continues_reason.as_deref()), (Some("mission_before"), Some(2), Some("route-switch")));
}

#[test]
fn what_a_handoff_left_out_round_trips() {
    let (_t, store) = temp();
    let path = written(&store, json!({ "continuesFrom": { "missionId": "mission_before", "checkpointEpoch": 2, "reason": "route-switch", "leftOut": ["summary", "earlier"] } }));
    assert_eq!(header(&path).omitted_briefing, ["summary", "earlier"]);
}

#[test]
fn a_peer_link_round_trips_through_the_durable_record() {
    let (_t, store) = temp();
    let path = written(&store, Value::Null);
    append_peer_links(&path, &[("received", "wm_1", "tm_atlas", NOW), ("posted", "wm_2", "tm_atlas", NOW)]).unwrap();
    let mission = recover_mission_from_path(&path).unwrap();
    assert_eq!(mission["issues"], json!([]));
    assert_eq!(mission["peerLinks"], json!([
        { "direction": "received", "messageId": "wm_1", "peerTeammateId": "tm_atlas", "occurredAt": NOW },
        { "direction": "posted", "messageId": "wm_2", "peerTeammateId": "tm_atlas", "occurredAt": NOW },
    ]));
    assert_eq!(mission["ledgerSequence"], 3);
}

#[test]
fn a_peer_link_is_refused_on_a_mission_written_before_version_5() {
    let (_t, store) = temp();
    let path = ledger_at(&store, 4, &[], json!({}));
    let error = append_peer_links(&path, &[("received", "wm_1", "tm_atlas", NOW)]).unwrap_err();
    assert!(error.to_string().contains("cannot hold peer links"), "{error}");
}

#[test]
fn a_peer_link_with_an_unsafe_id_or_an_unknown_direction_is_refused() {
    let (_t, store) = temp();
    let path = written(&store, Value::Null);
    assert!(append_peer_links(&path, &[("received", "../escape", "tm_atlas", NOW)]).is_err());
    assert!(append_peer_links(&path, &[("broadcast", "wm_1", "tm_atlas", NOW)]).is_err());
    assert_eq!(recover_mission_from_path(&path).unwrap()["peerLinks"], json!([]));
}

#[test]
fn the_mode_a_run_was_asked_for_round_trips_beside_what_it_was_allowed() {
    let (_t, store) = temp();
    let path = written(&store, json!({ "sandbox": "read-only", "mode": "plan" }));
    let m = header(&path);
    assert_eq!((m.mode.as_deref(), m.sandbox.as_str()), (Some("plan"), "read-only"));
}

#[test]
fn a_mission_with_no_mode_reads_as_one_with_none() {
    let (_t, store) = temp();
    let path = ledger_at(&store, 27, &[], json!({}));
    assert_eq!(header(&path).mode, None);
}

#[test]
fn a_workspace_write_mission_is_recorded_at_the_current_version() {
    let (_t, store) = temp();
    let path = written(&store, json!({ "sandbox": "workspace-write" }));
    assert_eq!(header(&path).sandbox, "workspace-write");
}

#[test]
fn v1_and_v2_read_only_missions_still_read() {
    let (_t, store) = temp();
    for version in [1, 2] {
        let path = ledger_at(&store, version, &[], json!({}));
        assert_eq!(header(&path).sandbox, "read-only", "version {version}");
    }
}

// Rust's own: the rest of mission-store's validatePeerLink, which no Electron case reaches.
#[test]
fn a_peer_link_with_an_unsafe_teammate_id_or_a_time_that_is_not_one_is_refused() {
    let (_t, store) = temp();
    let path = written(&store, Value::Null);
    assert_eq!(append_peer_links(&path, &[("received", "wm_1", "../tm", NOW)]).unwrap_err().to_string(), "peerTeammateId is invalid");
    assert_eq!(append_peer_links(&path, &[("received", "wm_1", "tm_atlas", "later")]).unwrap_err().to_string(), "Peer link timestamp is invalid");
    assert_eq!(recover_mission_from_path(&path).unwrap()["peerLinks"], json!([]));
}

/// No header reads from this file: the record was refused, so the mission is not there.
fn refused(path: &Path) -> bool {
    Tail::new(path, 0).poll().iter().find_map(metadata).is_none()
}

// case: writes new missions at the current version
#[test]
fn writes_new_missions_at_the_current_version() {
    let (_t, store) = temp();
    let first = |path: &Path| -> Value { serde_json::from_str(std::fs::read_to_string(path).unwrap().lines().next().unwrap()).unwrap() };
    assert_eq!(crate::ledger_write::schema_written("codex"), 27);
    assert_eq!(crate::ledger_write::schema_written("apple"), 28);
    assert_eq!(first(&written(&store, Value::Null))["schemaVersion"], 27);
}

// case: keeps a mission on Apple Intelligence, and refuses one in a file written before version 28
#[test]
fn keeps_a_mission_on_apple_intelligence_and_refuses_one_in_a_file_written_before_version_28() {
    let (_t, store) = temp();
    let mut writer = LedgerWriter::create_runtime_with(&store, "mission_1", "Summarise the notes.", "apple-on-device", "apple", "apple", &[], None, &json!({"sandbox":"read-only","mode":"ask"})).unwrap();
    // Every line the run writes is at its own version, so Electron's reader walks the file to its end.
    writer.append("run.started", json!({}), None).unwrap();
    let text = std::fs::read_to_string(&writer.path).unwrap();
    let lines: Vec<Value> = text.lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert!(lines.len() >= 2);
    assert!(lines.iter().all(|l| l["schemaVersion"] == 28), "{text}");
    assert_eq!(header(&writer.path).runtime, "apple");
    // A 27 file naming Apple Intelligence was hand-edited: 27 could not write it.
    let (_t2, older) = temp();
    let path = ledger_at(&older, 27, &[], json!({"runtime":"apple","model":"apple-on-device","mode":"ask"}));
    assert!(refused(&path));
    assert!(!refused(&ledger_at(&older, 28, &[], json!({"runtime":"apple","model":"apple-on-device","mode":"ask"}))));
}

// case: refuses a version-1 file describing a runtime version 1 could not write
#[test]
fn refuses_a_version_1_file_describing_a_runtime_version_1_could_not_write() {
    let (_t, store) = temp();
    assert!(refused(&ledger_at(&store, 1, &[], json!({"runtime":"claude"}))));
    assert!(refused(&ledger_at(&store, 1, &[], json!({"model":"gpt-5.5"}))));
    assert!(!refused(&ledger_at(&store, 1, &[], json!({}))));
}

// case: refuses a pre-v3 file claiming a mission was allowed to write
#[test]
fn refuses_a_pre_v3_file_claiming_a_mission_was_allowed_to_write() {
    let (_t, store) = temp();
    assert!(refused(&ledger_at(&store, 2, &[], json!({"sandbox":"workspace-write"}))));
    assert!(!refused(&ledger_at(&store, 3, &[], json!({"sandbox":"workspace-write"}))));
}

// case: refuses a pre-v4 file claiming to continue another mission
#[test]
fn refuses_a_pre_v4_file_claiming_to_continue_another_mission() {
    let (_t, store) = temp();
    let link = json!({"continuesFrom":{"missionId":"mission_other","checkpointEpoch":1,"reason":"route-switch"}});
    assert!(refused(&ledger_at(&store, 1, &[], link.clone())));
    assert!(refused(&ledger_at(&store, 3, &[], link.clone())));
    assert!(!refused(&ledger_at(&store, 4, &[], link)));
}

// case: refuses a follow-up in a file written before version 6
#[test]
fn refuses_a_follow_up_in_a_file_written_before_version_6() {
    let (_t, store) = temp();
    let link = json!({"continuesFrom":{"missionId":"mission_other","checkpointEpoch":1,"reason":"follow-up"}});
    assert!(refused(&ledger_at(&store, 5, &[], link.clone())));
    assert!(!refused(&ledger_at(&store, 6, &[], link)));
}

// case: refuses a Gemini mission in a file written before version 7
#[test]
fn refuses_a_gemini_mission_in_a_file_written_before_version_7() {
    let (_t, store) = temp();
    let gemini = json!({"runtime":"gemini","model":"gemini-3-pro"});
    assert!(refused(&ledger_at(&store, 6, &[], gemini.clone())));
    assert!(!refused(&ledger_at(&store, 7, &[], gemini)));
    // Claude was known before 7.
    assert!(!refused(&ledger_at(&store, 6, &[], json!({"runtime":"claude","model":"sonnet"}))));
}

// case: refuses a host-started run in a file written before version 10
#[test]
fn refuses_a_host_started_run_in_a_file_written_before_version_10() {
    let (_t, store) = temp();
    let relay = json!({"startedBy":{"kind":"relay","hop":1}});
    assert!(refused(&ledger_at(&store, 9, &[], relay.clone())));
    assert!(!refused(&ledger_at(&store, 10, &[], relay)));
}

// case: refuses a resume starter in a file written before version 11
#[test]
fn refuses_a_resume_starter_in_a_file_written_before_version_11() {
    let (_t, store) = temp();
    let resume = json!({"startedBy":{"kind":"resume","epoch":1}});
    assert!(refused(&ledger_at(&store, 10, &[], resume.clone())));
    assert!(!refused(&ledger_at(&store, 11, &[], resume)));
}

// case: refuses a command in a file written before version 12
#[test]
fn refuses_a_command_in_a_file_written_before_version_12() {
    let (_t, store) = temp();
    let command = json!({"command":{"name":"review"}});
    assert!(refused(&ledger_at(&store, 11, &[], command.clone())));
    assert!(!refused(&ledger_at(&store, 12, &[], command)));
}

// case: refuses a routine starter in a file written before version 13
#[test]
fn refuses_a_routine_starter_in_a_file_written_before_version_13() {
    let (_t, store) = temp();
    let routine = json!({"startedBy":{"kind":"routine","routineId":"routine_1","step":1}});
    assert!(refused(&ledger_at(&store, 12, &[], routine.clone())));
    assert!(!refused(&ledger_at(&store, 13, &[], routine)));
}

// case: refuses a full-access mission in a file written before version 14
#[test]
fn refuses_a_full_access_mission_in_a_file_written_before_version_14() {
    let (_t, store) = temp();
    let full = json!({"sandbox":"full-access"});
    assert!(refused(&ledger_at(&store, 13, &[], full.clone())));
    assert!(!refused(&ledger_at(&store, 14, &[], full)));
}

// case: refuses a mode in a file written before version 15
#[test]
fn refuses_a_mode_in_a_file_written_before_version_15() {
    let (_t, store) = temp();
    let mode = json!({"mode":"ask"});
    assert!(refused(&ledger_at(&store, 14, &[], mode.clone())));
    assert!(!refused(&ledger_at(&store, 15, &[], mode)));
}

// case: refuses a terminal starter in a file written before version 18
#[test]
fn refuses_a_terminal_starter_in_a_file_written_before_version_18() {
    let (_t, store) = temp();
    let terminal = json!({"startedBy":{"kind":"terminal","exchange":1}});
    assert!(refused(&ledger_at(&store, 17, &[], terminal.clone())));
    assert!(!refused(&ledger_at(&store, 18, &[], terminal)));
}

// case: refuses a side starter in a file written before version 19
#[test]
fn refuses_a_side_starter_in_a_file_written_before_version_19() {
    let (_t, store) = temp();
    let side = json!({"startedBy":{"kind":"side","conversation":"conv_1","question":1}});
    assert!(refused(&ledger_at(&store, 18, &[], side.clone())));
    assert!(!refused(&ledger_at(&store, 19, &[], side)));
}

// case: keeps a mission on an agent the person added, and refuses one in a file written before version 25
#[test]
fn keeps_a_mission_on_an_agent_the_person_added_and_refuses_one_in_a_file_written_before_version_25() {
    let (_t, store) = temp();
    let acp = json!({"runtime":"acp","model":"agent-default"});
    assert!(refused(&ledger_at(&store, 24, &[], acp.clone())));
    assert!(!refused(&ledger_at(&store, 25, &[], acp)));
}

// case: keeps a turn Locust sent back to fix a failing check, and refuses one in a file written before version 26
#[test]
fn keeps_a_turn_locust_sent_back_to_fix_a_failing_check_and_refuses_one_in_a_file_written_before_version_26() {
    let (_t, store) = temp();
    let fix = json!({"sandbox":"workspace-write","startedBy":{"kind":"check-fix","fix":1,"of":2,"command":"npm test"}});
    assert!(refused(&ledger_at(&store, 25, &[], fix.clone())));
    assert!(!refused(&ledger_at(&store, 26, &[], fix)));
}

// case: keeps a mission on Kilo, and refuses one in a file written before version 27
#[test]
fn keeps_a_mission_on_kilo_and_refuses_one_in_a_file_written_before_version_27() {
    let (_t, store) = temp();
    let kilo = json!({"runtime":"kilo","model":"kilo-default"});
    assert!(refused(&ledger_at(&store, 26, &[], kilo.clone())));
    assert!(!refused(&ledger_at(&store, 27, &[], kilo)));
}

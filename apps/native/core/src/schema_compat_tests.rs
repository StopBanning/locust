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

// case: rejects a file whose version this reader does not know
#[test]
fn a_file_whose_version_the_reader_does_not_know_is_not_read() {
    let (_t, store) = temp();
    let path = ledger_at(&store, 29, &[], json!({}));
    assert!(Tail::new(&path, 0).poll().is_empty());
}

// case: still recovers a mission recorded before the version bump
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

// case: appends to a version-1 mission in version 1, keeping the file walkable
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

// case: round-trips a continuation through the durable record
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

// case: round-trips a peer link through the durable record
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

// case: refuses to append a peer link to a mission written before version 5
#[test]
fn a_peer_link_is_refused_on_a_mission_written_before_version_5() {
    let (_t, store) = temp();
    let path = ledger_at(&store, 4, &[], json!({}));
    let error = append_peer_links(&path, &[("received", "wm_1", "tm_atlas", NOW)]).unwrap_err();
    assert!(error.to_string().contains("cannot hold peer links"), "{error}");
}

// case: refuses a peer link with an unsafe id or an unknown direction
#[test]
fn a_peer_link_with_an_unsafe_id_or_an_unknown_direction_is_refused() {
    let (_t, store) = temp();
    let path = written(&store, Value::Null);
    assert!(append_peer_links(&path, &[("received", "../escape", "tm_atlas", NOW)]).is_err());
    assert!(append_peer_links(&path, &[("broadcast", "wm_1", "tm_atlas", NOW)]).is_err());
    assert_eq!(recover_mission_from_path(&path).unwrap()["peerLinks"], json!([]));
}

// case: round-trips the mode a run was asked for, beside what it was allowed
#[test]
fn the_mode_a_run_was_asked_for_round_trips_beside_what_it_was_allowed() {
    let (_t, store) = temp();
    let path = written(&store, json!({ "sandbox": "read-only", "mode": "plan" }));
    let m = header(&path);
    assert_eq!((m.mode.as_deref(), m.sandbox.as_str()), (Some("plan"), "read-only"));
}

// case: reads a mission with no mode as one, rather than inventing a default
#[test]
fn a_mission_with_no_mode_reads_as_one_with_none() {
    let (_t, store) = temp();
    let path = ledger_at(&store, 27, &[], json!({}));
    assert_eq!(header(&path).mode, None);
}

// case: records a workspace-write mission at the current version
#[test]
fn a_workspace_write_mission_is_recorded_at_the_current_version() {
    let (_t, store) = temp();
    let path = written(&store, json!({ "sandbox": "workspace-write" }));
    assert_eq!(header(&path).sandbox, "workspace-write");
}

// case: still reads v1 and v2 read-only missions
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
    let side = json!({"startedBy":{"kind":"side","of":"mission_main","question":1}});
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

// case: stops recovery when a record disagrees with its file version
#[test]
fn stops_recovery_when_a_record_disagrees_with_its_file_version() {
    let (_t, store) = temp();
    let events = codex_events();
    let path = ledger_at(&store, 1, &events, json!({}));
    let mut lines: Vec<String> = std::fs::read_to_string(&path).unwrap().trim_end().lines().map(str::to_owned).collect();
    let mut third: Value = serde_json::from_str(&lines[2]).unwrap();
    third["schemaVersion"] = json!(2);
    lines[2] = third.to_string();
    std::fs::write(&path, format!("{}\n", lines.join("\n"))).unwrap();

    let recovered = crate::ledger::read_ledger(&path).unwrap();
    assert!(!recovered.unreadable);
    assert_eq!(recovered.records.len(), 2); // header + 1 event before mismatch
    assert!(recovered.issues.contains(&crate::ledger::Issue::InvalidRecord));
}

// case: round-trips what a handoff left out, and refuses a malformed list
#[test]
fn round_trips_what_a_handoff_left_out_and_refuses_a_malformed_list() {
    let (_t, store) = temp();
    let path = written(&store, json!({"continuesFrom":{"missionId":"mission_before","checkpointEpoch":2,"reason":"route-switch","leftOut":["summary","earlier"]}}));
    assert_eq!(header(&path).omitted_briefing, ["summary", "earlier"]);

    let (_t2, other) = temp();
    let too_long = "x".repeat(41);
    let err = LedgerWriter::create_runtime_with(
        &other, "mission_1", "Inspect.", "account-default", "codex", "codex", &[], None,
        &json!({"continuesFrom":{"missionId":"mission_before","checkpointEpoch":2,"reason":"route-switch","leftOut":[too_long]}}),
    );
    assert!(err.is_err());
}

// case: refuses a continuation that names an unsafe mission id
#[test]
fn refuses_a_continuation_that_names_an_unsafe_mission_id() {
    let (_t, store) = temp();
    let err = LedgerWriter::create_runtime_with(
        &store, "mission_1", "Inspect.", "account-default", "codex", "codex", &[], None,
        &json!({"continuesFrom":{"missionId":"../escape","checkpointEpoch":1,"reason":"route-switch"}}),
    );
    assert!(err.is_err());
    assert!(refused(&ledger_at(&store, 27, &[], json!({"continuesFrom":{"missionId":"../escape","checkpointEpoch":1,"reason":"route-switch"}}))));
}

// case: refuses a continuation with an impossible checkpoint epoch
#[test]
fn refuses_a_continuation_with_an_impossible_checkpoint_epoch() {
    let (_t, store) = temp();
    let err = LedgerWriter::create_runtime_with(
        &store, "mission_1", "Inspect.", "account-default", "codex", "codex", &[], None,
        &json!({"continuesFrom":{"missionId":"mission_before","checkpointEpoch":0,"reason":"route-switch"}}),
    );
    assert!(err.is_err());
    assert!(refused(&ledger_at(&store, 27, &[], json!({"continuesFrom":{"missionId":"mission_before","checkpointEpoch":0,"reason":"route-switch"}}))));
}

// case: refuses a peer link in a file written before version 5
#[test]
fn refuses_a_peer_link_in_a_file_written_before_version_5() {
    let (_t, store) = temp();
    let path = ledger_at(&store, 1, &[], json!({}));
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(&format!("{}\n", json!({
        "schemaVersion": 1,
        "recordType": "mission.peer",
        "ledgerSequence": 2,
        "occurredAt": NOW,
        "link": {"direction":"received","messageId":"wm_1","peerTeammateId":"tm_atlas","occurredAt":NOW}
    })));
    std::fs::write(&path, text).unwrap();

    let recovered = crate::ledger::read_ledger(&path).unwrap();
    assert_eq!(recovered.records.len(), 1);
    assert_eq!(recovered.issues, vec![crate::ledger::Issue::InvalidRecord]);
}

// case: round-trips a follow-up continuation, session handle and all
#[test]
fn round_trips_a_follow_up_continuation_session_handle_and_all() {
    let (_t, store) = temp();
    let link = json!({"missionId":"mission_before","checkpointEpoch":1,"reason":"follow-up","runtimeThreadId":"session-abc"});
    let path = written(&store, json!({"continuesFrom": link}));
    let recovered = recover_mission_from_path(&path).unwrap();
    assert_eq!(recovered["issues"], json!([]));
    assert_eq!(recovered["metadata"]["continuesFrom"], link);
}

// case: round-trips who started a run at the current version
#[test]
fn round_trips_who_started_a_run_at_the_current_version() {
    let (_t, store) = temp();
    let path = written(&store, json!({"startedBy":{"kind":"relay","hop":2}}));
    let recovered = recover_mission_from_path(&path).unwrap();
    assert_eq!(recovered["issues"], json!([]));
    assert_eq!(recovered["metadata"]["startedBy"], json!({"kind":"relay","hop":2}));
    let m = header(&path);
    assert_eq!((m.started_by_kind.as_deref(), m.started_by_hop), (Some("relay"), Some(2)));
}

// case: refuses a starter it cannot read rather than dropping the field
#[test]
fn refuses_a_starter_it_cannot_read_rather_than_dropping_the_field() {
    let (_t, store) = temp();
    assert!(refused(&ledger_at(&store, 10, &[], json!({"startedBy":{"kind":"relay","hop":0}}))));
}

// case: round-trips a routine starter at the current version
#[test]
fn round_trips_a_routine_starter_at_the_current_version() {
    let (_t, store) = temp();
    let path = written(&store, json!({"startedBy":{"kind":"routine","routineId":"rt_abc","step":2}}));
    let recovered = recover_mission_from_path(&path).unwrap();
    assert_eq!(recovered["issues"], json!([]));
    assert_eq!(recovered["metadata"]["startedBy"], json!({"kind":"routine","routineId":"rt_abc","step":2}));
}

// case: round-trips a terminal starter at the current version (v18)
#[test]
fn round_trips_a_terminal_starter_at_the_current_version_v18() {
    let (_t, store) = temp();
    let path = written(&store, json!({"startedBy":{"kind":"terminal","exchange":2}}));
    let recovered = recover_mission_from_path(&path).unwrap();
    assert_eq!(recovered["issues"], json!([]));
    assert_eq!(recovered["metadata"]["startedBy"], json!({"kind":"terminal","exchange":2}));
}

// case: refuses a terminal starter whose exchange is below 1
#[test]
fn refuses_a_terminal_starter_whose_exchange_is_below_1() {
    let (_t, store) = temp();
    let err = LedgerWriter::create_runtime_with(
        &store, "mission_1", "Inspect.", "account-default", "codex", "codex", &[], None,
        &json!({"startedBy":{"kind":"terminal","exchange":0}}),
    );
    assert!(err.is_err());
}

// case: round-trips a side starter at the current version (v19)
#[test]
fn round_trips_a_side_starter_at_the_current_version_v19() {
    let (_t, store) = temp();
    let path = written(&store, json!({"startedBy":{"kind":"side","of":"mission_main","question":2}}));
    let recovered = recover_mission_from_path(&path).unwrap();
    assert_eq!(recovered["issues"], json!([]));
    assert_eq!(recovered["metadata"]["startedBy"], json!({"kind":"side","of":"mission_main","question":2}));
    assert!(header(&path).side);
}

// case: refuses a side starter with no conversation, or a question below 1
#[test]
fn refuses_a_side_starter_with_no_conversation_or_a_question_below_1() {
    let (_t, store) = temp();
    assert!(LedgerWriter::create_runtime_with(
        &store, "mission_1", "Inspect.", "account-default", "codex", "codex", &[], None,
        &json!({"startedBy":{"kind":"side","of":"","question":1}}),
    ).is_err());
    assert!(LedgerWriter::create_runtime_with(
        &store, "mission_2", "Inspect.", "account-default", "codex", "codex", &[], None,
        &json!({"startedBy":{"kind":"side","of":"mission_main","question":0}}),
    ).is_err());
}

// case: refuses a routine starter with no routine id or a step below 1
#[test]
fn refuses_a_routine_starter_with_no_routine_id_or_a_step_below_1() {
    let (_t, store) = temp();
    assert!(LedgerWriter::create_runtime_with(
        &store, "mission_1", "Inspect.", "account-default", "codex", "codex", &[], None,
        &json!({"startedBy":{"kind":"routine","routineId":"","step":1}}),
    ).is_err());
    assert!(LedgerWriter::create_runtime_with(
        &store, "mission_2", "Inspect.", "account-default", "codex", "codex", &[], None,
        &json!({"startedBy":{"kind":"routine","routineId":"rt_abc","step":0}}),
    ).is_err());
}

// case: refuses a mode that is not one of the five
#[test]
fn refuses_a_mode_that_is_not_one_of_the_five() {
    let (_t, store) = temp();
    let err = LedgerWriter::create_runtime_with(
        &store, "mission_1", "Inspect.", "account-default", "codex", "codex", &[], None,
        &json!({"mode":"whatever"}),
    );
    assert!(err.is_err());
    assert!(err.err().unwrap().to_string().to_lowercase().contains("mode"));
}

// case: round-trips a full-access mission at the current version
#[test]
fn round_trips_a_full_access_mission_at_the_current_version() {
    let (_t, store) = temp();
    let path = written(&store, json!({"sandbox":"full-access"}));
    let recovered = recover_mission_from_path(&path).unwrap();
    assert_eq!(recovered["issues"], json!([]));
    assert_eq!(recovered["metadata"]["sandbox"], "full-access");
    assert_eq!(header(&path).sandbox, "full-access");
}

// case: round-trips a resume starter at the current version
#[test]
fn round_trips_a_resume_starter_at_the_current_version() {
    let (_t, store) = temp();
    let path = written(&store, json!({"startedBy":{"kind":"resume","epoch":3}}));
    let recovered = recover_mission_from_path(&path).unwrap();
    assert_eq!(recovered["issues"], json!([]));
    assert_eq!(recovered["metadata"]["startedBy"], json!({"kind":"resume","epoch":3}));
}

// case: refuses a resume starter whose epoch is not an epoch
#[test]
fn refuses_a_resume_starter_whose_epoch_is_not_an_epoch() {
    let (_t, store) = temp();
    assert!(refused(&ledger_at(&store, 11, &[], json!({"startedBy":{"kind":"resume","epoch":0}}))));
}

// case: round-trips the command the host ran
#[test]
fn round_trips_the_command_the_host_ran() {
    let (_t, store) = temp();
    let path = written(&store, json!({"command":{"executablePath":"/usr/bin/codex","args":["-p","<prompt>","--no-color"]}}));
    let recovered = recover_mission_from_path(&path).unwrap();
    assert_eq!(recovered["issues"], json!([]));
    assert_eq!(recovered["metadata"]["command"]["args"], json!(["-p","<prompt>","--no-color"]));
}

// case: refuses a command carrying an argument it cannot vouch for
#[test]
fn refuses_a_command_carrying_an_argument_it_cannot_vouch_for() {
    let (_t, store) = temp();
    let bad = format!("--flag\0");
    assert!(refused(&ledger_at(&store, 12, &[], json!({"command":{"executablePath":"/usr/bin/codex","args":[bad]}}))));
}

// case: round-trips a Cursor Agent mission at the current version
#[test]
fn round_trips_a_cursor_agent_mission_at_the_current_version() {
    let (_t, store) = temp();
    let writer = LedgerWriter::create_runtime_with(
        &store, "mission_1", "Inspect.", "composer-1", "cursor", "cursor", &[], None, &Value::Null,
    ).unwrap();
    let recovered = recover_mission_from_path(&writer.path).unwrap();
    assert_eq!(recovered["issues"], json!([]));
    assert_eq!(recovered["metadata"]["runtime"], "cursor");
}

// case: refuses a continuation reason it does not know
#[test]
fn refuses_a_continuation_reason_it_does_not_know() {
    let (_t, store) = temp();
    assert!(LedgerWriter::create_runtime_with(
        &store, "mission_1", "Inspect.", "account-default", "codex", "codex", &[], None,
        &json!({"continuesFrom":{"missionId":"mission_before","checkpointEpoch":1,"reason":"guessed"}}),
    ).is_err());
}

// case: accepts a Claude mission and its Claude events
#[test]
fn accepts_a_claude_mission_and_its_claude_events() {
    let (_t, store) = temp();
    let writer = LedgerWriter::create_runtime_with(
        &store, "mission_1", "Inspect.", "claude-sonnet-5", "claude", "claude", &[], None, &Value::Null,
    ).unwrap();
    let path = writer.path.clone();
    drop(writer);
    crate::ledger_write::append_events(&path, |run, mission, _adapter, seq, at| {
        vec![json!({
            "id": format!("{run}:claude:{seq}"), "runId": run, "missionId": mission, "sequence": seq,
            "occurredAt": at, "sourceAdapter": "claude", "type": "run.started",
            "payload": {"runtimeThreadId": "thread_1"}
        })]
    }).unwrap();
    let recovered = crate::ledger::read_ledger(&path).unwrap();
    assert!(!recovered.unreadable);
    assert!(recovered.issues.is_empty());
    assert_eq!(recovered.records.len(), 2);
    assert_eq!(recovered.records[0]["metadata"]["runtime"], "claude");
    assert_eq!(recovered.records[0]["metadata"]["model"], "claude-sonnet-5");
}

// case: refuses an event from a runtime the mission is not running
#[test]
fn refuses_an_event_from_a_runtime_the_mission_is_not_running() {
    let (_t, store) = temp();
    let path = written(&store, Value::Null);
    let err = crate::ledger_write::append_events(&path, |run, mission, _adapter, seq, at| {
        vec![json!({
            "id": format!("{run}:claude:{seq}"), "runId": run, "missionId": mission, "sequence": seq,
            "occurredAt": at, "sourceAdapter": "claude", "type": "run.started",
            "payload": {"runtimeThreadId": "thread_1"}
        })]
    }).unwrap_err();
    assert!(err.to_string().contains("not readable by the ledger reader"), "{err}");
}

// A continuation is refused, on read and on write, when its checkpoint is not one, its reason is not a known one, or it
// lists more than eight things left out (schema-compat's continuation rules).
#[test]
fn a_continuation_with_a_zero_checkpoint_an_unknown_reason_or_too_much_left_out_is_refused() {
    let nine: Vec<String> = (0..9).map(|i| format!("item{i}")).collect();
    for link in [
        json!({ "missionId": "mission_before", "checkpointEpoch": 0, "reason": "route-switch" }),
        json!({ "missionId": "mission_before", "checkpointEpoch": 2, "reason": "because" }),
        json!({ "missionId": "mission_before", "checkpointEpoch": 2, "reason": "route-switch", "leftOut": nine }),
    ] {
        let (_t, store) = temp();
        let path = ledger_at(&store, 27, &codex_events(), json!({ "continuesFrom": link }));
        assert!(refused(&path), "read: {link}");
        let (_t, store) = temp();
        let made = LedgerWriter::create_runtime_with(&store, "mission_1", "Inspect.", "account-default", "codex", "codex", &[], None, &json!({ "continuesFrom": link }));
        assert!(made.is_err(), "write: {link}");
        // Refused before anything is made: no ledger file is left for a mission that was never started.
        assert_eq!(std::fs::read_dir(store.ledger_dir().unwrap()).unwrap().count(), 0, "a file was left: {link}");
    }
    // Eight left out is still fine.
    let (_t, store) = temp();
    let eight: Vec<String> = (0..8).map(|i| format!("item{i}")).collect();
    let path = written(&store, json!({ "continuesFrom": { "missionId": "mission_before", "checkpointEpoch": 1, "reason": "carry-on", "leftOut": eight } }));
    assert!(!refused(&path));
}

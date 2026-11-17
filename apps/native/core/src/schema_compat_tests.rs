// Carries, where Rust has the behaviour, packages/mission-store/test/schema-compat.test.ts (Electron 4ce2f37a): which
// ledger versions Rust reads and writes. Rust's readers (`ledger::Tail`, `ledger::metadata`,
// `mission_export::recover_mission_from_path`) take every version from 1 to 27 and check no field against the version
// that could hold it, so the cases that refuse a hand-edited older file are not built (see the tsv row).
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
    for version in 1..=27 {
        let path = ledger_at(&store, version, &[], json!({}));
        assert_eq!(Tail::new(&path, 0).poll().len(), 1, "version {version}");
    }
}

#[test]
fn a_file_whose_version_the_reader_does_not_know_is_not_read() {
    let (_t, store) = temp();
    let path = ledger_at(&store, 28, &[], json!({}));
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

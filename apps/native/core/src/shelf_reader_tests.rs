//! What the list reader hands the window for the shelf rows (23 Nov merge check): the amber "receipt incomplete" tag
//! (`missions_screen` reads `Summary::integrity_issues`) and the limit offer (`free_offer` reads `RunEnd::Failed`'s
//! `at_limit`) must come from the record on disk, not only from a summary a test builds by hand.
use super::*;
use crate::ledger::torn_ledger_tests::Folder;
use crate::ledger::RunEnd;
use serde_json::{Value, json};

fn record(id: &str, end: Value) -> String {
    let at = "2026-11-23T10:00:00.000Z";
    let header = json!({ "schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": at,
        "metadata": { "missionId": id, "runId": format!("run_{id}"), "prompt": "do the thing", "runtime": "opencode",
            "model": "opencode/some-free-model", "requestedRouteId": "opencode:account-default",
            "resolvedRouteId": "opencode:some-free-model", "cliVersion": null, "workspaceId": "ws_test",
            "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": at } });
    let mut event = json!({ "schemaVersion": 27, "recordType": "mission.event", "ledgerSequence": 2, "occurredAt": at,
        "event": { "id": "e1", "runId": format!("run_{id}"), "missionId": id, "sequence": 1, "occurredAt": at,
            "sourceAdapter": "opencode", "type": "run.completed", "payload": {} } });
    event["event"]["type"] = end["type"].clone();
    event["event"]["payload"] = end["payload"].clone();
    format!("{header}\n{event}\n")
}

fn summary(folder: &Folder, id: &str) -> Summary {
    HistoryCache::new()
        .list_recent_summaries(&folder.0, 10)
        .expect("the folder is read")
        .into_iter()
        .find(|s| s.mission_id == id)
        .expect("the mission is listed")
}

#[test]
fn a_record_with_damage_after_its_header_is_listed_with_integrity_issues() {
    let folder = Folder::new();
    folder.write("m_clean", &record("m_clean", json!({"type": "run.completed", "payload": {}})), false);
    let mut damaged = record("m_damaged", json!({"type": "run.completed", "payload": {}}));
    damaged.push_str("not a record at all\n");
    folder.write("m_damaged", &damaged, false);
    assert!(summary(&folder, "m_damaged").integrity_issues, "the amber tag reads this flag");
    assert!(!summary(&folder, "m_clean").integrity_issues, "a clean record is not amber");
}

#[test]
fn a_run_that_failed_on_its_quota_is_listed_as_failed_at_its_limit() {
    let folder = Folder::new();
    let quota = json!({"type": "run.failed", "payload": {"kind": "quota-exhausted", "message": "Out of free requests"}});
    folder.write("m_quota", &record("m_quota", quota), false);
    let plain = json!({"type": "run.failed", "payload": {"kind": "process-exit", "message": "It stopped"}});
    folder.write("m_plain", &record("m_plain", plain), false);
    assert_eq!(
        summary(&folder, "m_quota").run_end,
        Some(RunEnd::Failed { message: "Out of free requests".into(), at_limit: true })
    );
    assert_eq!(summary(&folder, "m_plain").run_end, Some(RunEnd::Failed { message: "It stopped".into(), at_limit: false }));
}

#[test]
fn an_open_conversation_whose_tip_failed_on_its_quota_carries_the_run_end_the_limit_offer_reads() {
    let folder = Folder::new();
    let quota = json!({"type": "run.failed", "payload": {"kind": "quota-exhausted", "message": "Out of free requests"}});
    folder.write("m_quota", &record("m_quota", quota), false);
    let snapshot = crate::ledger::Session::new(folder.0.clone(), "m_quota".into()).poll().expect("the record is read");
    assert_eq!(snapshot.run_end, Some(RunEnd::Failed { message: "Out of free requests".into(), at_limit: true }));
    let completed = Folder::new();
    completed.write("m_done", &record("m_done", json!({"type": "run.completed", "payload": {}})), false);
    let snapshot = crate::ledger::Session::new(completed.0.clone(), "m_done".into()).poll().expect("the record is read");
    assert_eq!(snapshot.run_end, Some(RunEnd::Completed));
}

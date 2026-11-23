//! Electron's `renderer/src/last-activity.test.ts` (B4 of PLAN-2026-09-13-INTERACTION): a conversation is listed by
//! when it last did something, not by when it was started. `ledger::last_activity_at` is the rule; both readers that
//! list conversations (`ledger::try_list_recent` and the window's `history_cache`, through `native::list_recent`) take
//! their `updated` from it, and `native::list_recent` sorts on that.
use super::*;
use crate::ledger::torn_ledger_tests::Folder;
use serde_json::{Value, json};
use std::fs;

const CREATED: &str = "2026-09-14T10:00:00.000Z";

fn at(occurred_at: &str) -> Value {
    json!({ "occurredAt": occurred_at })
}

/// A record written as the host writes one: its header, then an event per time.
fn record(id: &str, created_at: &str, times: &[&str]) -> String {
    let mut out = format!(
        "{}\n",
        json!({ "schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": created_at,
            "metadata": { "missionId": id, "runId": format!("run_{id}"), "prompt": format!("Ask {id}"), "runtime": "codex",
                "model": "account-default", "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default",
                "cliVersion": "test", "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1,
                "createdAt": created_at } })
    );
    for (index, time) in times.iter().enumerate() {
        let sequence = index as u64 + 1;
        out.push_str(&format!(
            "{}\n",
            json!({ "schemaVersion": 27, "recordType": "mission.event", "ledgerSequence": sequence + 1, "occurredAt": time,
                "event": { "id": format!("e{sequence}"), "runId": format!("run_{id}"), "missionId": id, "sequence": sequence,
                    "occurredAt": time, "sourceAdapter": "codex", "type": "run.completed", "payload": {} } })
        ));
    }
    out
}

/// The conversations the window lists, newest first, from records in Electron's folder.
fn listed(files: &[(&str, &str, &[&str])]) -> Vec<(String, String)> {
    let folder = Folder::new();
    let ledger = folder.0.join("electron/mission-ledger");
    fs::create_dir_all(&ledger).unwrap();
    for (id, created_at, times) in files {
        fs::write(ledger.join(format!("{id}.jsonl")), record(id, created_at, times)).unwrap();
    }
    let store = Store::new(folder.0.join("native"), folder.0.join("electron")).unwrap();
    let _g = hooks::install(store, None);
    let listed = list_recent(MAX_LISTED_MISSIONS, true).expect("the records are listed");
    listed.summaries.into_iter().map(|s| (s.mission_id, s.updated)).collect()
}

// case: is the last event, when there is one
#[test]
fn is_the_last_event_when_there_is_one() {
    let events = [at("2026-09-14T10:05:00.000Z"), at("2026-09-14T11:30:00.000Z")];
    assert_eq!(ledger::last_activity_at(CREATED, events.iter()), "2026-09-14T11:30:00.000Z");
    // And the list says the same of a record holding them.
    let rows = listed(&[("m_said", CREATED, &["2026-09-14T10:05:00.000Z", "2026-09-14T11:30:00.000Z"])]);
    assert_eq!(rows, [("m_said".to_owned(), "2026-09-14T11:30:00.000Z".to_owned())]);
    // The other reader of the folder says the same.
    let folder = Folder::new();
    folder.write("m_said", &record("m_said", CREATED, &["2026-09-14T10:05:00.000Z", "2026-09-14T11:30:00.000Z"]), false);
    assert_eq!(ledger::try_list_recent(&folder.0, 10).unwrap()[0].updated, "2026-09-14T11:30:00.000Z");
}

// case: falls back to when it was made, for a run that has said nothing
#[test]
fn falls_back_to_when_it_was_made_for_a_run_that_has_said_nothing() {
    assert_eq!(ledger::last_activity_at(CREATED, std::iter::empty()), CREATED);
    let rows = listed(&[("m_quiet", CREATED, &[])]);
    assert_eq!(rows, [("m_quiet".to_owned(), CREATED.to_owned())]);
}

// case: never goes BACKWARDS from the creation time
#[test]
fn never_goes_backwards_from_the_creation_time() {
    // A runtime clock behind the host's would otherwise sort a live run below where it was a moment ago.
    assert_eq!(ledger::last_activity_at(CREATED, [at("2026-09-14T09:00:00.000Z")].iter()), CREATED);
    let rows = listed(&[("m_behind", CREATED, &["2026-09-14T09:00:00.000Z"])]);
    assert_eq!(rows, [("m_behind".to_owned(), CREATED.to_owned())]);
    // The other reader of the folder says the same.
    let folder = Folder::new();
    folder.write("m_behind", &record("m_behind", CREATED, &["2026-09-14T09:00:00.000Z"]), false);
    let summaries = ledger::try_list_recent(&folder.0, 10).unwrap();
    assert_eq!(summaries[0].updated, CREATED);
}

// case: puts a long working conversation above a newer idle one
#[test]
fn puts_a_long_working_conversation_above_a_newer_idle_one() {
    let rows = listed(&[
        ("m_idle", "2026-09-14T11:00:00.000Z", &[]),
        ("m_working", "2026-09-14T09:00:00.000Z", &["2026-09-14T11:59:00.000Z"]),
    ]);
    let order: Vec<&str> = rows.iter().map(|(id, _)| id.as_str()).collect();
    assert_eq!(order, ["m_working", "m_idle"]);
}

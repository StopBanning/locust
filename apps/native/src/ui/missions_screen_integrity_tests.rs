use super::*;
use crate::tests::TempDir;
use rune::{Env, Runtime};
use serde_json::json;
use std::io::Write;

#[test]
fn the_missions_list_draws_an_incomplete_completed_receipt_in_amber() {
    let dir = TempDir::new();
    let at = "2026-11-23T10:00:00Z";
    for id in ["m_complete", "m_damaged"] {
        dir.write(id, &[
            json!({
                "schemaVersion": 27, "recordType": "mission.created", "occurredAt": at,
                "metadata": {
                    "missionId": id, "runId": format!("run_{id}"), "prompt": format!("Review {id}"),
                    "runtime": "codex", "model": "account-default", "createdAt": at,
                    "workspaceId": "ws_test", "sandbox": "read-only",
                    "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default",
                    "executionPolicyVersion": 1
                }
            }),
            json!({
                "schemaVersion": 27, "recordType": "mission.event", "occurredAt": at,
                "event": {
                    "id": format!("end_{id}"), "missionId": id, "runId": format!("run_{id}"),
                    "sequence": 1, "type": "run.completed", "sourceAdapter": "codex",
                    "occurredAt": at, "payload": {}
                }
            }),
        ]);
    }
    std::fs::OpenOptions::new()
        .append(true)
        .open(dir.0.join("m_damaged.jsonl"))
        .unwrap()
        .write_all(b"{\"schemaVersion\":")
        .unwrap();

    let mut s = State::default();
    let summaries = locust_core::history_cache::global()
        .list_recent_summaries(&dir.0, 20)
        .unwrap();
    for summary in summaries {
        let id = summary.mission_id.clone();
        s.recent.push(ConversationRow {
            id: id.clone(),
            teammate: "Codex".into(),
            title: summary.title.clone(),
            started_by: "native".into(),
            state: "completed".into(),
            updated: at.to_owned(),
        });
        s.ledger_summaries.insert(id, summary);
    }
    let mut rt = Runtime::new(
        s,
        |s, _| view(s, &Palette::new(true), false),
        (1160.0, 720.0),
        Env::new(true, 1.0),
    );
    for _ in 0..8 {
        rt.frame(0.05);
    }
    let elements = rt.elements();
    let tag = |key: &str| {
        elements
            .iter()
            .find(|el| el.key.as_deref() == Some(key))
            .unwrap()
    };
    let incomplete = tag("missions-tag-m_damaged");
    let complete = tag("missions-tag-m_complete");
    let l = crate::look::controls::Look::new(true);
    assert_eq!(
        incomplete.text.as_deref(),
        Some("Completed · receipt incomplete")
    );
    assert_eq!(incomplete.text_color, Some(l.c("amber")));
    assert_eq!(complete.text.as_deref(), Some("Completed"));
    assert_eq!(complete.text_color, Some(l.c("text-muted")));
}

use super::*;
use crate::tests::TempDir;
use rune::{Env, Runtime};
use serde_json::json;
use std::io::Write;

// The two Electron renderer cases already have named unit tests. This adds the real durable-history input,
// including the unreadable suffix, rather than setting Snapshot.damaged by hand.
#[test]
fn messages_draws_a_damaged_tip_from_its_saved_receipt_but_not_another_turn() {
    let dir = TempDir::new();
    let at = "2026-11-23T10:00:00Z";
    for id in ["m_whole", "m_torn"] {
        dir.write(id, &[
            json!({"schemaVersion":27,"recordType":"mission.created","occurredAt":at,"metadata":{
                "missionId":id,"runId":format!("run_{id}"),"prompt":"Read the receipt","runtime":"codex",
                "model":"account-default","createdAt":at,"workspaceId":"ws_test","sandbox":"read-only",
                "requestedRouteId":"codex","resolvedRouteId":"codex-account:default","executionPolicyVersion":1
            }}),
            json!({"schemaVersion":27,"recordType":"mission.event","occurredAt":at,"event":{
                "id":format!("end_{id}"),"missionId":id,"runId":format!("run_{id}"),"sequence":1,
                "type":"run.completed","sourceAdapter":"codex","occurredAt":at,"payload":{}
            }}),
        ]);
    }
    let torn = dir.0.join("m_torn.jsonl");
    std::fs::OpenOptions::new()
        .append(true)
        .open(&torn)
        .unwrap()
        .write_all(b"{broken\n")
        .unwrap();
    let original = std::fs::read(&torn).unwrap();
    let saved = locust_core::history_cache::global()
        .list_recent_summaries(&dir.0, 20)
        .unwrap();
    assert_eq!(saved.len(), 2);
    let mut s = tests_live_state();
    for summary in saved {
        s.ledger_summaries
            .insert(summary.mission_id.clone(), summary);
    }
    let pane = s.ledger.as_mut().unwrap();
    pane.snapshot.running = false;
    pane.snapshot.live = None;
    pane.snapshot.damaged = false;
    // On an initial live-poll failure the renderer still knows the chosen local receipt.
    pane.conversation.conversation_id = Some("m_torn".into());
    let mut rt = Runtime::new(
        s,
        |s, _| message_column(&[], "Codec", &Palette::new(true), s),
        (800.0, 600.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.05);
    let said = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("damaged-record-note"))
        .and_then(|e| e.text);
    assert_eq!(
        said.as_deref(),
        Some(
            "Part of this turn's record could not be read, so what came after it is missing here. The file is kept as it is."
        )
    );
    rt.update(|s, _| {
        s.ledger.as_mut().unwrap().snapshot.tip = "m_whole".into();
    });
    for _ in 0..8 {
        rt.frame(0.05);
    }
    assert!(
        rt.find("damaged-record-note").is_none(),
        "an intact tip must not inherit the anchor's note"
    );
    assert_eq!(std::fs::read(torn).unwrap(), original);
}

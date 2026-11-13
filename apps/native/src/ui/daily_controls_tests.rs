use super::*;
use serde_json::json;

#[test]
fn model_pick_is_consumed_for_one_send_only_and_does_not_leak() {
    let mut s = State::default();
    team_scene(&mut s);
    s.demo = false;

    let pick = TurnPick {
        model: "sample-model".into(),
        effort: Some("high".into()),
        runtime: None,
        account: None,
    };
    s.picks.insert("codec".into(), pick.clone());
    assert_eq!(
        s.picks.get("argon"),
        None,
        "pick on one teammate never leaks to another"
    );
    assert_eq!(take_turn_pick(&mut s, "codec"), Some(pick.clone()));
    assert_eq!(take_turn_pick(&mut s, "codec"), None);

    // A pick made on a Recent ledger (@ledger) is cleared when leaving or switching Recent ledgers.
    s.ledger_summaries.insert(
        "recent-1".into(),
        ledger::Summary {
            parent: None,
            words: String::new(),
            mission_id: "recent-1".into(),
            title: "First".into(),
            runtime: "codex".into(),
            model: "gpt-6.1-sol".into(),
            created_at: "2026-11-08T00:00:00Z".into(),
            running: false,
            failed: false,
            updated: "2026-11-08T00:00:00Z".into(),
            ..Default::default()
        },
    );
    s.ledger_summaries.insert(
        "recent-2".into(),
        ledger::Summary {
            parent: None,
            words: String::new(),
            mission_id: "recent-2".into(),
            title: "Second".into(),
            runtime: "claude".into(),
            model: "sonnet".into(),
            created_at: "2026-11-08T00:00:00Z".into(),
            running: false,
            failed: false,
            updated: "2026-11-08T00:00:00Z".into(),
            ..Default::default()
        },
    );
    let mut tx = Tx::default();
    open_recent(
        &mut s,
        ConversationRow {
            id: "recent-1".into(),
            teammate: "Codex".into(),
            title: "First".into(),
            started_by: "ledger".into(),
            state: "completed".into(),
            updated: "2026-11-08T00:00:00Z".into(),
        },
        &mut tx,
    );
    s.picks.insert(LEDGER_TARGET.into(), pick.clone());
    open_recent(
        &mut s,
        ConversationRow {
            id: "recent-2".into(),
            teammate: "Claude Code".into(),
            title: "Second".into(),
            started_by: "ledger".into(),
            state: "completed".into(),
            updated: "2026-11-08T00:00:00Z".into(),
        },
        &mut tx,
    );
    assert!(
        s.picks.get(LEDGER_TARGET).is_none(),
        "Recent ledger pick must not leak to another Recent conversation"
    );

    // Starting a fresh conversation (Cmd+N) or changing a teammate's runtime clears its unconsumed pick.
    select_teammate(&mut s, "codec", &mut tx);
    s.picks.insert("codec".into(), pick.clone());
    fresh_conversation(&mut s);
    assert!(s.picks.get("codec").is_none());

    s.picks.insert("codec".into(), pick);
    let mut updated_team = s.teammates.clone();
    updated_team[0].runtime = Some("claude".into());
    apply_team(&mut s, updated_team);
    assert!(
        s.picks.get("codec").is_none(),
        "runtime change clears stale model pick"
    );
}

#[test]
fn stop_requires_our_accepted_turn_and_exact_ledger_tip() {
    let mut s = State::default();
    ledger_live_scene(&mut s);
    s.demo = false;
    assert!(
        !can_stop(&s),
        "watching someone else's working turn is never ownership"
    );
    s.own_turn = Some(OwnTurn {
        conversation: "synthetic-ledger".into(),
        tip: Some("synthetic-ledger".into()),
        before: None,
    });
    assert!(can_stop(&s));
    s.ledger.as_mut().expect("scene").snapshot.tip = "another-turn".into();
    assert!(!can_stop(&s));
    s.ledger.as_mut().expect("scene").snapshot.tip = "synthetic-ledger".into();
    s.ledger.as_mut().expect("scene").snapshot.running = false;
    assert!(!can_stop(&s));
}

#[test]
fn stop_never_latches_onto_external_turn_after_fast_completion_or_detach() {
    let temp = tests::TempDir::new();
    temp.write(
        "root",
        &[
            json!({
                "schemaVersion":27,
                "recordType":"mission.created",
                "metadata":{
                    "missionId":"root",
                    "prompt":"Initial",
                    "runtime":"codex",
                    "model":"sample",
                    "createdAt":"2026-01-01T00:00:00Z"
                }
            }),
            json!({
                "schemaVersion":27,
                "recordType":"mission.event",
                "event":{
                    "sequence":1,
                    "occurredAt":"2026-01-01T00:00:01Z",
                    "type":"run.completed",
                    "payload":{}
                }
            }),
        ],
    );
    // Our follow-up child_1 already finished before the first ledger poll saw it running.
    temp.write(
        "child_1",
        &[
            json!({
                "schemaVersion":27,
                "recordType":"mission.created",
                "metadata":{
                    "missionId":"child_1",
                    "prompt":"Our follow-up",
                    "runtime":"codex",
                    "model":"sample",
                    "createdAt":"2026-01-01T00:00:02Z",
                    "continuesFrom":{"missionId":"root"}
                }
            }),
            json!({
                "schemaVersion":27,
                "recordType":"mission.event",
                "event":{
                    "sequence":1,
                    "occurredAt":"2026-01-01T00:00:03Z",
                    "type":"run.completed",
                    "payload":{}
                }
            }),
        ],
    );

    let mut s = State::default();
    team_scene(&mut s);
    s.demo = false;
    let header = s.teammates[0].clone();
    s.own_turn = Some(OwnTurn {
        conversation: "root".into(),
        tip: None,
        before: Some("root".into()),
    });
    s.ledger = Some(LedgerPane {
        header,
        conversation: Conversation {
            conversation_id: Some("root".into()),
            turns: Vec::new(),
        },
        items: Vec::new(),
        dir: temp.0.clone(),
        worker: Some(ledger::Session::new(temp.0.clone(), "root".into())),
        snapshot: ledger::Snapshot::default(),
        due: 0.0,
        loading: true,
        after_tip: Some("root".into()),
        after_tip_deadline: None,
        backoff: 0.0,
    });

    let mut tx = Tx::default();
    ledger_tick(&mut s, 0.0, &mut tx);
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while s.ledger_job.is_pending() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    ledger_tick(&mut s, 0.1, &mut tx);
    assert!(s.own_turn.is_none(), "completed turn must clear ownership");
    assert!(!can_stop(&s));

    // Now an external turn child_2 starts in Locust; polling it must never grant Stop.
    temp.write(
        "child_2",
        &[json!({
            "schemaVersion":27,
            "recordType":"mission.created",
            "metadata":{
                "missionId":"child_2",
                "prompt":"External turn",
                "runtime":"codex",
                "model":"sample",
                "createdAt":"2026-01-01T00:00:04Z",
                "continuesFrom":{"missionId":"child_1"}
            }
        })],
    );
    s.ledger.as_mut().unwrap().loading = true;
    s.ledger.as_mut().unwrap().due = 1.0;
    ledger_tick(&mut s, 1.0, &mut tx);
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while s.ledger_job.is_pending() && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    ledger_tick(&mut s, 1.1, &mut tx);
    assert!(s.flight.is_some(), "window follows the external turn");
    assert!(!can_stop(&s), "external turn must not expose Stop");
}

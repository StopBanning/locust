//! `what-was-said-while-working-stays-where-it-was-said.test.ts`, drawn: the window draws the turn from the ledger's
//! own reading in the order it happened (the logic's own cases are in core/src/thread_order_tests.rs).
use super::*;
use serde_json::{json, Value};

fn record(sequence: u64, kind: &str, payload: Value) -> Value {
    json!({ "event": { "id": format!("evt_{sequence}"), "runId": "run_1", "missionId": "mission_1", "sequence": sequence,
        "occurredAt": "2026-09-23T12:00:00.000Z", "sourceAdapter": "opencode", "type": kind, "payload": payload } })
}
fn said(sequence: u64, id: &str, text: &str) -> Value {
    record(sequence, "message.delta", json!({ "itemId": id, "operation": "append", "text": text, "final": true }))
}
fn tool(sequence: u64, kind: &str, id: &str, command: &str) -> Value {
    record(sequence, kind, json!({ "itemId": id, "toolKind": "command_execution", "name": "shell", "command": command, "exitCode": 0 }))
}

/// The turn read by the ledger and drawn in a window; what a reader meets, in order: words, steps lines (by the first
/// row's id), and the foot.
fn drawn(records: Vec<Value>) -> Vec<String> {
    let mut transcript = ledger::Transcript::default();
    transcript.apply(records);
    let texts: Vec<String> = transcript.items.iter().filter_map(|i| match i { ledger::Item::Assistant { text, .. } => Some(text.clone()), _ => None }).collect();
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: Teammate { id: "tm_codec".into(), name: "Codec".into(), role: "Engineer".into(), runtime: None, model: None, account: None, busy: false, runs_in: "local".into(), folder: None },
        conversation: Conversation { conversation_id: Some("m_1".into()), turns: Vec::new() },
        items: transcript.items.clone(),
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: ledger::Snapshot { running: transcript.running, turn_metas: vec![ledger::Metadata { mission_id: "m_1".into(), ..Default::default() }], ..Default::default() },
        due: 0.0, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
    });
    let mut rt = rune::Runtime::new(s, |s, _| message_column(&s.ledger.as_ref().unwrap().items, "Codec", &Palette::new(true), s), (800.0, 2400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.05);
    let els = rt.elements();
    if std::env::var("DUMP").is_ok() { for e in &els { if e.key.is_some() || e.text.is_some() { eprintln!("KEY {:?} {:?}", e.key, e.text); } } }
    els.into_iter()
        .filter_map(|e| {
            if let Some(key) = e.key.as_deref() {
                if let Some(id) = key.strip_prefix("m_1/group/") {
                    return Some(format!("steps: {id}"));
                }
                // The turn's foot: its trace line (`{conversation}/trace/{index}`), not one of its parts.
                if key.strip_prefix("m_1/trace/").is_some_and(|rest| !rest.contains('/')) {
                    return Some("foot".into());
                }
            }
            e.text.filter(|t| texts.contains(t)).map(|t| format!("said: {t}"))
        })
        .collect()
}

// case: reads in the order it happened: each thing said, then the steps before the next (drawn)
#[test]
fn the_window_draws_each_thing_said_then_the_steps_before_the_next() {
    let records = vec![
        record(1, "run.started", json!({})),
        said(2, "m1", "Creating your HELLO file and lining up verification."),
        tool(3, "tool.started", "t1", "touch hello.txt"),
        tool(4, "tool.completed", "t1", "touch hello.txt"),
        said(5, "m2", "Then I will print it back to confirm."),
        tool(6, "tool.started", "t2", "cat hello.txt"),
        tool(7, "tool.completed", "t2", "cat hello.txt"),
        said(8, "m3", "DONE"),
        record(9, "run.completed", json!({})),
    ];
    assert_eq!(
        drawn(records),
        [
            "said: Creating your HELLO file and lining up verification.",
            "steps: t1",
            "said: Then I will print it back to confirm.",
            "steps: t2",
            "said: DONE",
            "foot",
        ]
    );
}

// case: does not split the steps on a message with nothing on screen (drawn)
#[test]
fn the_window_draws_one_steps_line_across_a_message_with_nothing_on_screen() {
    let records = vec![
        record(1, "run.started", json!({})),
        tool(2, "tool.started", "t1", "ls"),
        tool(3, "tool.completed", "t1", "ls"),
        said(4, "m0", "   "),
        tool(5, "tool.started", "t2", "pwd"),
        tool(6, "tool.completed", "t2", "pwd"),
        said(7, "m1", "Done."),
        record(8, "run.completed", json!({})),
    ];
    assert_eq!(drawn(records), ["steps: t1", "said: Done.", "foot"]);
}

// case: puts a row the host added after the run ended in no group: it is in the turn’s files at the foot (drawn)
#[test]
fn the_window_draws_a_row_added_after_the_run_ended_only_in_the_foot() {
    let records = vec![
        record(1, "run.started", json!({})),
        said(2, "m1", "All written."),
        record(3, "run.completed", json!({})),
        record(4, "tool.started", json!({ "itemId": "obs", "toolKind": "file_change", "name": "edit", "command": "a.txt" })),
    ];
    assert_eq!(drawn(records), ["said: All written.", "foot"]);
}

// case: draws the same order while it runs, so nothing moves when it ends (drawn)
#[test]
fn the_window_draws_the_same_order_while_the_turn_runs() {
    let going = vec![
        record(1, "run.started", json!({})),
        said(2, "m1", "Creating your HELLO file and lining up verification."),
        tool(3, "tool.started", "t1", "touch hello.txt"),
        tool(4, "tool.completed", "t1", "touch hello.txt"),
        said(5, "m2", "Then I will print it back to confirm."),
        tool(6, "tool.started", "t2", "cat hello.txt"),
        tool(7, "tool.completed", "t2", "cat hello.txt"),
        said(8, "m3", "DONE"),
    ];
    let mut ended = going.clone();
    ended.push(record(9, "run.completed", json!({})));
    let live = drawn(going);
    let mut done = drawn(ended);
    assert_eq!(done.pop().as_deref(), Some("foot"));
    assert_eq!(live, done);
    assert_eq!(live.iter().filter(|d| d.starts_with("steps: ")).count(), 2, "{live:?}");
}

// Electron's `renderer/src/a-notice-is-said-once.test.ts`: a runtime's remark about itself is said by the first turn
// of a conversation that heard it, a usage warning once a run and once a conversation, and a CLI's remark about its
// own setup in no turn at all (its row in Settings says it: `setup_notes`, drawn by agents_section). Read as the
// window reads a conversation: its turns' ledger files through `Session::poll`.
use super::*;

const SKILLS: &str = "Skill descriptions were shortened to fit the skills context budget.";
const REMARK: &str = "The model is answering from a fallback region.";

fn notice(seq: u64, message: &str) -> Value {
    event(seq, "adapter.diagnostic", json!({ "code": "codex.item_notice", "level": "warning", "terminal": false, "message": message }))
}

fn warn(seq: u64, kind: &str, message: &str) -> Value {
    event(seq, "route.limit_detected", json!({ "kind": kind, "message": message, "evidence": { "redacted": false } }))
}

fn usage(percent: u32) -> String {
    format!("You've used {percent}% of your 7-day window · resets 2026-09-28T07:00:00.000Z")
}

fn header(id: &str, after: Option<&str>) -> Value {
    let mut created = created(id, "Synthetic prompt");
    if let Some(parent) = after {
        created["metadata"]["continuesFrom"] = json!({ "missionId": parent });
    }
    created
}

/// A Codex turn that opens with its setup note and a remark, maybe one more, then reads a file.
fn turn(id: &str, after: Option<&str>, extra: Option<&str>) -> Vec<Value> {
    let mut records = vec![header(id, after), event(1, "run.started", json!({})), notice(2, SKILLS), notice(3, REMARK)];
    if let Some(extra) = extra {
        records.push(notice(4, extra));
    }
    records.push(event(5, "tool.started", json!({ "itemId": "a", "toolKind": "read", "name": "read", "command": "README.md", "phase": "started" })));
    records.push(event(6, "tool.completed", json!({ "itemId": "a", "toolKind": "read", "name": "read", "command": "README.md", "phase": "completed" })));
    records.push(event(7, "run.completed", json!({})));
    records
}

/// A turn that answers under a usage warning.
fn answer(id: &str, after: Option<&str>, kind: &str, message: &str) -> Vec<Value> {
    vec![
        header(id, after),
        event(1, "run.started", json!({})),
        warn(2, kind, message),
        event(3, "message.delta", json!({ "itemId": "m", "operation": "replace", "text": "Done.", "final": true })),
        event(4, "run.completed", json!({})),
    ]
}

fn read(turns: &[Vec<Value>]) -> Snapshot {
    let temp = Temp::new();
    let mut last = String::new();
    for records in turns {
        last = records[0]["metadata"]["missionId"].as_str().unwrap().to_owned();
        write(&temp.path(&last), records);
    }
    Session::new(temp.0.clone(), last).poll().unwrap()
}

/// Each turn's fold notices, oldest turn first.
fn notices(snapshot: &Snapshot) -> Vec<Vec<String>> {
    snapshot
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Trace(trace) => Some(trace.notices.iter().map(|n| n.message.clone()).collect()),
            _ => None,
        })
        .collect()
}

fn limits(snapshot: &Snapshot) -> Vec<(String, String)> {
    snapshot
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Limit { kind, text } => Some((kind.clone(), text.clone())),
            _ => None,
        })
        .collect()
}

// case: is carried by the first turn that heard it
#[test]
fn is_carried_by_the_first_turn_that_heard_it() {
    assert_eq!(notices(&read(&[turn("t1", None, None)])), [vec![REMARK.to_owned()]]);
}

// case: is not said again by a later turn, while a new remark still is
#[test]
fn is_not_said_again_by_a_later_turn_while_a_new_remark_still_is() {
    let said = notices(&read(&[turn("t1", None, None), turn("t2", Some("t1"), Some("The sandbox was reset."))]));
    assert_eq!(said, [vec![REMARK.to_owned()], vec!["The sandbox was reset.".to_owned()]]);
    // However its spacing and case came out, and on the turn after that too.
    let said = notices(&read(&[
        turn("t1", None, None),
        turn("t2", Some("t1"), Some("The sandbox was reset.")),
        turn("t3", Some("t2"), Some("the  sandbox was RESET.")),
    ]));
    assert_eq!(said[2], Vec::<String>::new());
}

// case: is one line a run, the newest figure, where the first one stood
#[test]
fn is_one_line_a_run_the_newest_figure_where_the_first_one_stood() {
    let snapshot = read(&[vec![
        header("t1", None),
        event(1, "run.started", json!({})),
        warn(2, "temporary-rate-limit", &usage(55)),
        event(3, "message.delta", json!({ "itemId": "m", "operation": "replace", "text": "A scratch project.", "final": true })),
        warn(4, "temporary-rate-limit", &usage(56)),
        event(5, "run.completed", json!({})),
    ]]);
    let said = limits(&snapshot);
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].1.contains("56%"));
    // Where the first one stood: above the answer.
    let limit = snapshot.items.iter().position(|i| matches!(i, Item::Limit { .. })).unwrap();
    let reply = snapshot.items.iter().position(|i| matches!(i, Item::Assistant { .. })).unwrap();
    assert!(limit < reply);
}

// case: is said once a conversation -- Colin: "Once per convo"
#[test]
fn is_said_once_a_conversation_colin_once_per_convo() {
    assert_eq!(limits(&read(&[answer("t1", None, "temporary-rate-limit", &usage(56))])).len(), 1);
    let said = limits(&read(&[answer("t1", None, "temporary-rate-limit", &usage(56)), answer("t2", Some("t1"), "temporary-rate-limit", &usage(58))]));
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].1.contains("56%"));
}

// case: never hides a limit actually reached
#[test]
fn never_hides_a_limit_actually_reached() {
    let said = limits(&read(&[
        answer("t1", None, "temporary-rate-limit", "You've used 90% of your 5-hour window"),
        answer("t2", Some("t1"), "quota-exhausted", "5-hour window 100% used · resets 2026-09-23T20:00:00.000Z"),
    ]));
    assert_eq!(said.iter().filter(|(kind, _)| kind == "quota-exhausted").count(), 1, "{said:?}");
}

// case: never reaches a turn, not even the first
#[test]
fn never_reaches_a_turn_not_even_the_first() {
    let snapshot = read(&[turn("t1", None, None)]);
    assert!(!notices(&snapshot).concat().iter().any(|n| crate::setup_notes::is_setup_note(n)));
    assert!(!snapshot.items.iter().any(|i| matches!(i, Item::Note { text, .. } if crate::setup_notes::is_setup_note(text))));
    // Nor a turn with no fold to put remarks under, where the others are promoted to the thread.
    let quiet = read(&[vec![header("t1", None), event(1, "run.started", json!({})), notice(2, SKILLS), notice(3, REMARK), event(4, "run.completed", json!({}))]]);
    assert!(quiet.items.iter().any(|i| matches!(i, Item::Note { text, .. } if text == REMARK)));
    assert!(!quiet.items.iter().any(|i| matches!(i, Item::Note { text, .. } if crate::setup_notes::is_setup_note(text))));
    // Nor when it comes after the work began, where a runtime's warning is drawn in the thread.
    let late = read(&[vec![
        header("t1", None),
        event(1, "run.started", json!({})),
        event(2, "tool.started", json!({ "itemId": "a", "toolKind": "read", "name": "read", "command": "README.md", "phase": "started" })),
        event(3, "tool.completed", json!({ "itemId": "a", "toolKind": "read", "name": "read", "command": "README.md", "phase": "completed" })),
        notice(4, SKILLS),
        notice(5, REMARK),
        event(6, "run.completed", json!({})),
    ]]);
    assert!(late.items.iter().any(|i| matches!(i, Item::Note { text, .. } if text == REMARK)), "the control: a remark after work is a line");
    assert!(!late.items.iter().any(|i| matches!(i, Item::Note { text, .. } if crate::setup_notes::is_setup_note(text))));
}

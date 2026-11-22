// Electron's `renderer/src/cost.test.ts`, the conversation half, read as the window reads a conversation: its turns'
// ledger files through `Session::poll`. What it has cost so far (cost.ts `conversationCost`, here
// `relay::conversation_cost` over each turn's receipt) and how full the context is (`latestContext`).
use super::*;

fn completed_with(seq: u64, usage: Value) -> Value {
    event(seq, "run.completed", json!({ "usage": usage, "evidence": { "redacted": true } }))
}

/// A turn of the conversation: its header (continuing `after`), its start, and its receipt when it has one.
fn turn(id: &str, after: Option<&str>, receipt: Option<Value>) -> Vec<Value> {
    let mut created = created(id, "Synthetic prompt");
    if let Some(parent) = after {
        created["metadata"]["continuesFrom"] = json!({ "missionId": parent });
    }
    let mut records = vec![created, event(1, "run.started", json!({}))];
    if let Some(usage) = receipt {
        records.push(completed_with(2, usage));
    }
    records
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

fn spent(input: u64, output: u64) -> Option<Value> {
    Some(json!({ "inputTokens": input, "outputTokens": output }))
}

fn tokens(input: u64, output: u64) -> Option<crate::benchmark::RunCost> {
    Some(crate::benchmark::RunCost { input_tokens: Some(input), output_tokens: Some(output), ..Default::default() })
}

// case: adds every turn that reported a number, including the one still running
#[test]
fn adds_every_turn_that_reported_a_number_including_the_one_still_running() {
    // Cost lived on a receipt, and a receipt belongs to one turn: a five-turn conversation kept its cost in five places.
    let snapshot = read(&[turn("t1", None, spent(100, 10)), turn("t2", Some("t1"), spent(200, 20)), turn("t3", Some("t2"), spent(50, 5))]);
    assert_eq!(snapshot.cost, tokens(350, 35));
    assert_eq!(crate::relay::conversation_cost([tokens(100, 10), tokens(200, 20)], tokens(50, 5)), tokens(350, 35));
}

// case: counts the earlier turns while the current one has reported nothing yet
#[test]
fn counts_the_earlier_turns_while_the_current_one_has_reported_nothing_yet() {
    let snapshot = read(&[turn("t1", None, spent(100, 10)), turn("t2", Some("t1"), None)]);
    assert_eq!(snapshot.cost, tokens(100, 10));
    assert_eq!(crate::relay::conversation_cost([tokens(100, 10)], None), tokens(100, 10));
}

// case: says nothing when no turn reported anything, rather than zero
#[test]
fn says_nothing_when_no_turn_reported_anything_rather_than_zero() {
    // A runtime that reported no usage has not said the work was free.
    assert_eq!(read(&[turn("t1", None, None)]).cost, None);
    assert_eq!(read(&[turn("t1", None, Some(json!({}))), turn("t2", Some("t1"), None)]).cost, None);
    assert_eq!(crate::relay::conversation_cost([], None), None);
    assert_eq!(crate::relay::conversation_cost([None], None), None);
}

// case: takes the newest turn that reported one, never the sum of the turns
#[test]
fn takes_the_newest_turn_that_reported_one_never_the_sum_of_the_turns() {
    // The window holds ONE prompt: summing turns would call a five-turn conversation five times as full as it is.
    let held = |used: u64| Some(json!({ "inputTokens": used, "contextTokens": used, "contextWindow": 200_000 }));
    let used = |s: &Snapshot| s.context.as_ref().map(|r| r.used);
    assert_eq!(used(&read(&[turn("t1", None, held(10_000)), turn("t2", Some("t1"), held(20_000)), turn("t3", Some("t2"), held(30_000))])), Some(30_000));
    // A live turn has reported nothing yet; the one before it still answers.
    assert_eq!(used(&read(&[turn("t1", None, held(10_000)), turn("t2", Some("t1"), held(20_000)), turn("t3", Some("t2"), None)])), Some(20_000));
}

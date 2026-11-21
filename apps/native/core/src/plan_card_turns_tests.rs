// Electron's `renderer/src/a-plan-card-is-its-own-turns.test.ts`: OpenCode sends its whole session's to-do list on
// every turn, so a turn's card showed the steps earlier turns had finished. Each turn's card is built here as the
// window builds it: a conversation of real ledger files read through `Session::poll`.
use super::*;

fn todo(steps: &[(&str, &str)]) -> Value {
    json!(steps.iter().map(|(content, status)| json!({"content": content, "status": status})).collect::<Vec<_>>())
}

fn turn(id: &str, after: Option<&str>, plans: &[Value]) -> Vec<Value> {
    let mut created = created(id, "Synthetic prompt");
    if let Some(parent) = after {
        created["metadata"]["continuesFrom"] = json!({"missionId": parent});
    }
    let mut records = vec![created, event(1, "run.started", json!({}))];
    for (n, plan) in plans.iter().enumerate() {
        records.push(event(2 + n as u64, "plan.updated", json!({"itemId": "todo", "plan": plan})));
    }
    records.push(event(10, "run.completed", json!({})));
    records
}

const ONE: &str = "Create notes/one.txt containing ALPHA";
const TWO: &str = "Create notes/two.txt containing BRAVO";
const THREE: &str = "Create notes/three.txt containing CHARLIE";

fn turn_2() -> Vec<Value> {
    turn("turn_2", None, &[todo(&[(ONE, "completed"), (TWO, "in_progress")]), todo(&[(ONE, "completed"), (TWO, "completed")])])
}

/// The cards of a conversation of these turns, oldest first, as the window draws them.
fn cards(turns: &[Vec<Value>]) -> Vec<Vec<(String, String)>> {
    let temp = Temp::new();
    let mut last = String::new();
    for records in turns {
        last = records[0]["metadata"]["missionId"].as_str().unwrap().to_owned();
        write(&temp.path(&last), records);
    }
    let snapshot = Session::new(temp.0.clone(), last).poll().unwrap();
    snapshot
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Plan { steps, .. } => Some(steps.iter().map(|s| (s.text.clone(), s.state.clone())).collect()),
            _ => None,
        })
        .collect()
}

// case: leaves out the steps earlier turns finished
#[test]
fn leaves_out_the_steps_earlier_turns_finished() {
    let turn_3 = turn("turn_3", Some("turn_2"), &[
        todo(&[(ONE, "completed"), (TWO, "completed"), (THREE, "pending")]),
        todo(&[(ONE, "completed"), (TWO, "completed"), (THREE, "completed")]),
    ]);
    let drawn = cards(&[turn_2(), turn_3]);
    assert_eq!(drawn.len(), 2);
    assert_eq!(drawn[1], [(THREE.to_owned(), "done".to_owned())], "one step, done: 1 of 1");
    assert_eq!(drawn[0].len(), 2, "the first turn's card is its own whole list");
    // A step the turn before left unfinished is this turn's, even when this turn only ever shows it done.
    let left = turn("turn_2", None, &[todo(&[(ONE, "completed"), (TWO, "pending")])]);
    let finished = turn("turn_3", Some("turn_2"), &[todo(&[(ONE, "completed"), (TWO, "completed")])]);
    assert_eq!(cards(&[left, finished])[1], [(TWO.to_owned(), "done".to_owned())]);
}

// case: keeps a step this turn worked on, even one an earlier turn had finished
#[test]
fn keeps_a_step_this_turn_worked_on_even_one_an_earlier_turn_had_finished() {
    // Re-opened and done again, spaced and cased differently: it is this turn's work too.
    let again = turn("turn_3", Some("turn_2"), &[todo(&[("create  notes/two.txt containing bravo", "in_progress")]), todo(&[(TWO, "completed")])]);
    assert_eq!(cards(&[turn_2(), again])[1].len(), 1);
}

// case: draws no card for a turn that only repeated a finished list
#[test]
fn draws_no_card_for_a_turn_that_only_repeated_a_finished_list() {
    let repeated = turn("turn_3", Some("turn_2"), &[todo(&[(ONE, "completed"), (TWO, "completed")])]);
    assert_eq!(cards(&[turn_2(), repeated]).len(), 1, "only the first turn's card");
}

// case: is untouched on the first turn, and where the runtime starts a fresh plan each turn
#[test]
fn is_untouched_on_the_first_turn_and_where_the_runtime_starts_a_fresh_plan_each_turn() {
    assert_eq!(cards(&[turn_2()])[0].len(), 2);
    let fresh = turn("turn_3", Some("turn_2"), &[todo(&[("Read the config", "completed")])]);
    assert_eq!(cards(&[turn_2(), fresh])[1], [("Read the config".to_owned(), "done".to_owned())]);
}

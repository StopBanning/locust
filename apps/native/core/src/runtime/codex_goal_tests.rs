//! A Codex teammate's standing goal through the caller: a Codex turn whose `codex app-server` is the stand-in
//! (codex_stand_in.rs) says `thread/goal/updated` and `thread/goal/cleared`, and the runtime holds what it said.
use super::codex_stand_in::*;
use super::*;
use crate::runtime::goal::{GoalStatus, ThreadGoal};

fn kept_green(status: &str) -> Value {
    json!({"threadId": "thread_9", "objective": "Keep the test suite green", "status": status, "tokenBudget": 200000,
        "tokensUsed": 1200, "timeUsedSeconds": 42, "createdAt": 1764200000, "updatedAt": 1764200042})
}

fn completed() -> (&'static str, Value) {
    ("turn/completed", json!({"threadId": "thread_9", "turn": {"id": "turn_1", "status": "completed"}}))
}

#[test]
fn a_running_codex_turn_holds_the_goal_its_server_says() {
    let (mut codex, _told) = turn_with(&[
        ("thread/goal/updated", json!({"threadId": "thread_9", "goal": kept_green("active")})),
        completed(),
    ]);
    assert_eq!(codex.standing_goal(), None, "nothing is held before the server says so");
    to_the_end(&mut codex);
    let held = codex.standing_goal().expect("the goal the server said");
    assert_eq!(held.objective, "Keep the test suite green");
    assert_eq!(held.status, GoalStatus::Active);
    assert_eq!(held.token_budget, Some(200000));
}

#[test]
fn a_goal_the_server_clears_is_no_longer_held() {
    let (mut codex, _told) = turn_with(&[
        ("thread/goal/updated", json!({"threadId": "thread_9", "goal": kept_green("active")})),
        ("thread/goal/cleared", json!({"threadId": "thread_9"})),
        completed(),
    ]);
    to_the_end(&mut codex);
    assert_eq!(codex.standing_goal(), None);
}

#[test]
fn a_later_update_replaces_the_goal_held_and_an_unknown_status_reads_as_unknown() {
    let (mut codex, _told) = turn_with(&[
        ("thread/goal/updated", json!({"threadId": "thread_9", "goal": kept_green("active")})),
        ("thread/goal/updated", json!({"threadId": "thread_9", "goal": kept_green("hibernating")})),
        completed(),
    ]);
    to_the_end(&mut codex);
    assert_eq!(codex.standing_goal().map(|g| g.status), Some(GoalStatus::Unknown));
}

#[test]
fn another_threads_goal_is_not_this_ones() {
    let mut other = kept_green("active");
    other["threadId"] = json!("thread_other");
    other["objective"] = json!("Someone else's goal");
    let (mut codex, _told) = turn_with(&[
        ("thread/goal/updated", json!({"threadId": "thread_9", "goal": kept_green("paused")})),
        ("thread/goal/updated", json!({"threadId": "thread_other", "goal": other})),
        ("thread/goal/cleared", json!({"threadId": "thread_other"})),
        completed(),
    ]);
    to_the_end(&mut codex);
    let held: Option<&ThreadGoal> = codex.standing_goal();
    assert_eq!(held.map(|g| (g.objective.as_str(), g.status)), Some(("Keep the test suite green", GoalStatus::Paused)));
}

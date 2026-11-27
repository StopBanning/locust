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

// ---- set, get and clear, once the thread exists ---------------------------------------------------------------------

use crate::runtime::goal::{GoalAnswer, GoalAsk, GoalSet};

/// A Codex turn under way on thread_9 whose server answers the goal requests as `plan` says.
fn started(plan: Value) -> (Codex, Told) {
    let mut plan = plan;
    plan["turn/start"] = json!([{"result": {"turn": {"id": "turn_1"}}}]);
    let (mut codex, told) = stand_in(plan);
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    (codex, told)
}

/// The stand-in's step that ends turn_1, completed.
fn turn_ends() -> Value {
    json!({"send": {"jsonrpc": "2.0", "method": "turn/completed", "params": {"threadId": "thread_9", "turn": {"id": "turn_1", "status": "completed"}}}})
}

fn held(goal: Value) -> GoalAnswer {
    GoalAnswer::Held(ThreadGoal::parse(&goal, None))
}

#[test]
fn a_set_asks_thread_goal_set_on_the_thread_as_the_person_and_holds_the_goal_it_answers() {
    let (mut codex, told) = started(json!({"thread/goal/set": [{"result": {"goal": kept_green("active")}}]}));
    let answer = codex.goal(GoalAsk::Set(GoalSet {
        objective: Some("Keep the test suite green".into()),
        status: None,
        token_budget: Some(200000),
    }));
    assert_eq!(
        told.params_of("thread/goal/set").unwrap(),
        json!({"threadId": "thread_9", "objective": "Keep the test suite green", "tokenBudget": 200000, "origin": "user"})
    );
    assert_eq!(answer, held(kept_green("active")));
    assert_eq!(codex.standing_goal().map(|g| g.status), Some(GoalStatus::Active));
}

#[test]
fn a_set_of_the_status_alone_sends_the_status_and_nothing_it_was_not_given() {
    let (mut codex, told) = started(json!({"thread/goal/set": [{"result": {"goal": kept_green("paused")}}]}));
    let answer = codex.goal(GoalAsk::Set(GoalSet { status: Some(GoalStatus::Paused), ..Default::default() }));
    assert_eq!(told.params_of("thread/goal/set").unwrap(), json!({"threadId": "thread_9", "status": "paused", "origin": "user"}));
    assert_eq!(answer, held(kept_green("paused")));
}

#[test]
fn a_get_asks_thread_goal_get_and_holds_what_it_answers_a_goal_or_none() {
    let (mut codex, told) = started(json!({
        "thread/goal/get#1": [{"result": {"goal": kept_green("blocked")}}],
        "thread/goal/get#2": [{"result": {"goal": null}}],
    }));
    assert_eq!(codex.goal(GoalAsk::Get), held(kept_green("blocked")));
    assert_eq!(told.params_of("thread/goal/get").unwrap(), json!({"threadId": "thread_9"}));
    assert_eq!(codex.standing_goal().map(|g| g.status), Some(GoalStatus::Blocked));
    assert_eq!(codex.goal(GoalAsk::Get), GoalAnswer::Held(None));
    assert_eq!(codex.standing_goal(), None, "none held once the server says there is none");
}

#[test]
fn a_clear_asks_thread_goal_clear_on_the_thread_and_holds_nothing() {
    let (mut codex, told) = started(json!({
        "thread/goal/set": [{"result": {"goal": kept_green("active")}}],
        "thread/goal/clear": [{"result": {}}],
    }));
    codex.goal(GoalAsk::Set(GoalSet { objective: Some("Keep the test suite green".into()), ..Default::default() }));
    assert_eq!(codex.goal(GoalAsk::Clear), GoalAnswer::Cleared);
    assert_eq!(told.params_of("thread/goal/clear").unwrap(), json!({"threadId": "thread_9"}));
    assert_eq!(codex.standing_goal(), None);
}

#[test]
fn a_codex_that_does_not_know_the_request_keeps_no_standing_goals_and_the_turn_goes_on() {
    // JSON-RPC's own "method not found", and Codex's words for a request it does not know (as it says of
    // `config/read` when too old, MEASURED on 0.163.0).
    // The turn ends only once both are answered, so a refusal that ended it would show.
    let plan = json!({
        "turn/start": [{"result": {"turn": {"id": "turn_1"}}}],
        "thread/goal/set": [{"error": "Method not found", "code": -32601}],
        "thread/goal/get": [{"error": "unknown variant `thread/goal/get`, expected one of `initialize`, `thread/start`"}, turn_ends()],
    });
    let (mut codex, _told) = stand_in(plan);
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    assert_eq!(codex.goal(GoalAsk::Set(GoalSet { objective: Some("x".into()), ..Default::default() })), GoalAnswer::NoStandingGoals);
    assert_eq!(codex.goal(GoalAsk::Get), GoalAnswer::NoStandingGoals);
    let events = to_the_end(&mut codex);
    assert_eq!(events.last().unwrap().kind, "run.completed", "{:?}", kinds(&events));
    assert!(of(&events, "run.failed").is_empty());
}

#[test]
fn any_other_refusal_is_said_in_codexs_words_and_does_not_end_the_turn() {
    let plan = json!({
        "turn/start": [{"result": {"turn": {"id": "turn_1"}}}],
        "thread/goal/set": [{"error": "objective must not be empty"}, turn_ends()],
    });
    let (mut codex, _told) = stand_in(plan);
    codex.start(&turn_in(Mode::AcceptEdits)).unwrap();
    let answer = codex.goal(GoalAsk::Set(GoalSet { objective: Some(" ".into()), ..Default::default() }));
    assert_eq!(answer, GoalAnswer::Refused("objective must not be empty".into()));
    assert_eq!(to_the_end(&mut codex).last().unwrap().kind, "run.completed");
}

#[test]
fn a_goal_asked_before_the_thread_exists_is_not_sent() {
    let (mut codex, told) = stand_in(json!({}));
    assert_eq!(codex.goal(GoalAsk::Get), GoalAnswer::NoThreadYet);
    assert_eq!(codex.goal(GoalAsk::Clear), GoalAnswer::NoThreadYet);
    std::thread::sleep(Duration::from_millis(100));
    assert!(told.asked().is_empty(), "{:?}", told.asked());
}

#[test]
fn a_runtime_other_than_codex_keeps_no_standing_goals() {
    let mut command = Command::new("python3");
    command.args(["-c", "import time; time.sleep(30)"]);
    let mut claude = crate::runtime::claude::Claude::spawn(&mut command).unwrap();
    let runtime: &mut dyn Runtime = &mut claude;
    assert_eq!(runtime.goal(GoalAsk::Get), GoalAnswer::NoStandingGoals);
    assert_eq!(runtime.goal(GoalAsk::Clear), GoalAnswer::NoStandingGoals);
}

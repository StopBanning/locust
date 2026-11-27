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
        "thread/goal/clear": [{"result": {"cleared": true}}],
    }));
    codex.goal(GoalAsk::Set(GoalSet { objective: Some("Keep the test suite green".into()), ..Default::default() }));
    assert_eq!(codex.goal(GoalAsk::Clear), GoalAnswer::Cleared);
    assert_eq!(told.params_of("thread/goal/clear").unwrap(), json!({"threadId": "thread_9", "origin": "user"}));
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

// ---- one ledger event per set, status change and clear; a reopened thread asks again -------------------------------

/// The goal events a run wrote, by code.
fn goal_codes(events: &[Event]) -> Vec<&str> {
    events
        .iter()
        .filter(|e| e.kind == "adapter.diagnostic")
        .filter_map(|e| e.payload["code"].as_str())
        .filter(|c| c.starts_with("codex.goal_"))
        .collect()
}

fn updated_notice(goal: Value) -> Value {
    json!({"send": {"jsonrpc": "2.0", "method": "thread/goal/updated", "params": {"threadId": "thread_9", "goal": goal}}})
}

fn cleared_notice() -> Value {
    json!({"send": {"jsonrpc": "2.0", "method": "thread/goal/cleared", "params": {"threadId": "thread_9"}}})
}

fn with_tokens(mut goal: Value, used: u64) -> Value {
    goal["tokensUsed"] = json!(used);
    goal
}

fn resumed_turn() -> Turn {
    Turn { resume: Some("thread_9".into()), ..turn_in(Mode::AcceptEdits) }
}

#[test]
fn a_set_writes_one_event_with_the_goal_however_many_times_the_server_says_it() {
    let (mut codex, _told) = started(json!({
        "thread/goal/set": [{"result": {"goal": kept_green("active")}}, updated_notice(kept_green("active")), turn_ends()],
    }));
    codex.goal(GoalAsk::Set(GoalSet { objective: Some("Keep the test suite green".into()), ..Default::default() }));
    let events = to_the_end(&mut codex);
    assert_eq!(goal_codes(&events), ["codex.goal_set"]);
    let set = events.iter().find(|e| e.payload["code"] == "codex.goal_set").unwrap();
    assert_eq!(set.payload["level"], "info");
    assert_eq!(set.payload["terminal"], false);
    assert_eq!(set.payload["message"], "Codex holds a standing goal: Keep the test suite green");
    assert_eq!(set.payload["goal"], kept_green("active"), "the goal as the protocol says it");
    assert_eq!(set.payload["evidence"], json!({"runtimeEventType": "thread/goal/set", "redacted": true}));
}

#[test]
fn a_status_change_writes_one_event_and_the_counts_ticking_write_none() {
    let (mut codex, _told) = turn_with(&[
        ("thread/goal/updated", json!({"threadId": "thread_9", "goal": kept_green("active")})),
        ("thread/goal/updated", json!({"threadId": "thread_9", "goal": with_tokens(kept_green("active"), 5000)})),
        ("thread/goal/updated", json!({"threadId": "thread_9", "goal": with_tokens(kept_green("budgetLimited"), 200000)})),
        ("thread/goal/updated", json!({"threadId": "thread_9", "goal": with_tokens(kept_green("budgetLimited"), 200001)})),
        completed(),
    ]);
    let events = to_the_end(&mut codex);
    assert_eq!(goal_codes(&events), ["codex.goal_set", "codex.goal_status"]);
    let status = events.iter().find(|e| e.payload["code"] == "codex.goal_status").unwrap();
    assert_eq!(status.payload["message"], "Codex's standing goal ran out of its token budget: Keep the test suite green");
    assert_eq!(status.payload["goal"]["status"], "budgetLimited");
    assert_eq!(status.payload["evidence"]["runtimeEventType"], "thread/goal/updated");
}

#[test]
fn a_new_objective_is_a_new_goal_set() {
    let mut other = kept_green("active");
    other["objective"] = json!("Ship the release notes");
    let (mut codex, _told) = turn_with(&[
        ("thread/goal/updated", json!({"threadId": "thread_9", "goal": kept_green("active")})),
        ("thread/goal/updated", json!({"threadId": "thread_9", "goal": other})),
        completed(),
    ]);
    assert_eq!(goal_codes(&to_the_end(&mut codex)), ["codex.goal_set", "codex.goal_set"]);
}

#[test]
fn a_clear_writes_one_event_however_many_times_the_server_says_it() {
    let (mut codex, _told) = started(json!({
        "thread/goal/set": [{"result": {"goal": kept_green("active")}}],
        "thread/goal/clear": [{"result": {"cleared": true}}, cleared_notice(), cleared_notice(), turn_ends()],
    }));
    codex.goal(GoalAsk::Set(GoalSet { objective: Some("Keep the test suite green".into()), ..Default::default() }));
    codex.goal(GoalAsk::Clear);
    let events = to_the_end(&mut codex);
    assert_eq!(goal_codes(&events), ["codex.goal_set", "codex.goal_cleared"]);
    let cleared = events.iter().find(|e| e.payload["code"] == "codex.goal_cleared").unwrap();
    assert_eq!(cleared.payload["message"], "Codex's standing goal was cleared.");
    assert!(cleared.payload.get("goal").is_none());
}

#[test]
fn clear_false_and_malformed_replies_do_not_claim_a_clear_or_discard_a_held_goal() {
    let (mut codex, _told) = started(json!({
        "thread/goal/set": [{"result": {"goal": kept_green("active")}}],
        "thread/goal/clear#1": [{"result": {"cleared": false}}],
        "thread/goal/clear#2": [{"result": {}}],
        "thread/goal/clear#3": [{"result": {"cleared": "true"}}, turn_ends()],
    }));
    codex.goal(GoalAsk::Set(GoalSet { objective: Some("Keep the test suite green".into()), ..Default::default() }));
    assert_eq!(codex.goal(GoalAsk::Clear), GoalAnswer::NotCleared, "false means no clear was performed");
    for _ in 0..2 {
        assert!(matches!(codex.goal(GoalAsk::Clear), GoalAnswer::Refused(_)), "a malformed reply must be refused");
    }
    assert_eq!(codex.standing_goal().map(|goal| goal.status), Some(GoalStatus::Active));
    assert_eq!(goal_codes(&to_the_end(&mut codex)), ["codex.goal_set"]);
}

#[test]
fn a_confirmed_clear_is_written_once_even_if_no_goal_was_cached() {
    let (mut codex, _told) = started(json!({
        "thread/goal/clear": [{"result": {"cleared": true}}, cleared_notice(), turn_ends()],
    }));
    assert!(codex.standing_goal().is_none());
    assert_eq!(codex.goal(GoalAsk::Clear), GoalAnswer::Cleared);
    assert_eq!(goal_codes(&to_the_end(&mut codex)), ["codex.goal_cleared"]);
}

#[test]
fn malformed_get_and_set_replies_do_not_discard_a_held_goal() {
    let mut no_usage = kept_green("paused");
    no_usage.as_object_mut().unwrap().remove("tokensUsed");
    let (mut codex, _told) = started(json!({
        "thread/goal/set#1": [{"result": {"goal": kept_green("active")}}],
        "thread/goal/get#1": [{"result": null}],
        "thread/goal/get#2": [{"result": {"goal": {"status": "paused"}}}],
        "thread/goal/set#2": [{"result": {"goal": null}}],
        "thread/goal/set#3": [{"result": {"goal": no_usage}}, turn_ends()],
    }));
    let set = GoalAsk::Set(GoalSet { objective: Some("Keep the test suite green".into()), ..Default::default() });
    codex.goal(set.clone());
    for ask in [GoalAsk::Get, GoalAsk::Get, set.clone(), set] {
        assert!(matches!(codex.goal(ask), GoalAnswer::Refused(_)), "a malformed reply must be refused");
        assert_eq!(codex.standing_goal().map(|goal| goal.status), Some(GoalStatus::Active));
    }
    assert_eq!(goal_codes(&to_the_end(&mut codex)), ["codex.goal_set"]);
}

#[test]
fn a_clear_of_a_fresh_thread_that_never_held_a_goal_writes_nothing() {
    let (mut codex, _told) = turn_with(&[("thread/goal/cleared", json!({"threadId": "thread_9"})), completed()]);
    assert!(goal_codes(&to_the_end(&mut codex)).is_empty());
}

#[test]
fn an_objective_carrying_a_secret_is_written_scrubbed() {
    let mut goal = kept_green("active");
    goal["objective"] = json!("Rotate the key sk-proj-abcdefghijklmnopqrstuvwxyz0123456789ABCD");
    let (mut codex, _told) = turn_with(&[("thread/goal/updated", json!({"threadId": "thread_9", "goal": goal})), completed()]);
    let events = to_the_end(&mut codex);
    let set = events.iter().find(|e| e.payload["code"] == "codex.goal_set").unwrap();
    let written = set.payload.to_string();
    assert!(!written.contains("abcdefghijklmnopqrstuvwxyz0123456789"), "{written}");
}

#[test]
fn a_reopened_thread_asks_thread_goal_get_before_its_turn_and_learns_the_goal_without_writing_it_again() {
    let (mut codex, told) = stand_in(json!({
        "thread/goal/get": [{"result": {"goal": kept_green("active")}}],
        "turn/start": [
            {"result": {"turn": {"id": "turn_1"}}},
            updated_notice(with_tokens(kept_green("active"), 9000)),
            updated_notice(with_tokens(kept_green("complete"), 9100)),
            turn_ends(),
        ],
    }));
    codex.start(&resumed_turn()).unwrap();
    let asked = told.asked();
    let at = |m: &str| asked.iter().position(|a| a == m).unwrap_or_else(|| panic!("{m} not asked: {asked:?}"));
    assert!(at("thread/resume") < at("thread/goal/get") && at("thread/goal/get") < at("turn/start"), "{asked:?}");
    assert_eq!(told.params_of("thread/goal/get").unwrap(), json!({"threadId": "thread_9"}));
    // The goal it already held is learned, not written again; only what changed since is.
    assert_eq!(goal_codes(&to_the_end(&mut codex)), ["codex.goal_status"]);
}

#[test]
fn a_fresh_thread_and_a_forked_one_do_not_ask() {
    let (_fresh, told) = started(json!({}));
    assert!(told.asked().contains(&"turn/start".to_owned()));
    assert!(!told.asked().contains(&"thread/goal/get".to_owned()), "{:?}", told.asked());
    let (forked, told) = stand_in(json!({}));
    let mut forked = forked.with_fork(true);
    forked.start(&resumed_turn()).unwrap();
    assert!(told.asked().contains(&"thread/fork".to_owned()));
    assert!(!told.asked().contains(&"thread/goal/get".to_owned()), "{:?}", told.asked());
}

#[test]
fn a_reopened_thread_on_a_codex_without_goals_goes_on_with_its_turn() {
    let (mut codex, told) = stand_in(json!({
        "thread/goal/get": [{"error": "Method not found", "code": -32601}],
        "turn/start": [{"result": {"turn": {"id": "turn_1"}}}, turn_ends()],
    }));
    codex.start(&resumed_turn()).unwrap();
    assert!(told.asked().contains(&"thread/goal/get".to_owned()));
    let events = to_the_end(&mut codex);
    assert_eq!(events.last().unwrap().kind, "run.completed");
    assert!(goal_codes(&events).is_empty());
}

// The v0.172.0 ThreadGoalUpdatedNotification has a nullable turnId.
// The resume/fork snapshot has no turn, unlike a change from the new turn.
fn snapshot_notice(goal: Option<Value>) -> Value {
    match goal {
        Some(goal) => json!({"send": {"jsonrpc": "2.0", "method": "thread/goal/updated",
            "params": {"threadId": "thread_9", "turnId": null, "goal": goal}}}),
        None => cleared_notice(),
    }
}

fn live_goal_notice(goal: Value) -> Value {
    json!({"send": {"jsonrpc": "2.0", "method": "thread/goal/updated",
        "params": {"threadId": "thread_9", "turnId": "turn_1", "goal": goal}}})
}

fn live_turn_start() -> Value {
    json!({"send": {"jsonrpc": "2.0", "method": "turn/started",
        "params": {"threadId": "thread_9", "turn": {"id": "turn_1", "status": "inProgress"}}}})
}

fn check_resumed_snapshot(had_goal: bool, notice_before_get: bool, stale_notice: bool) {
    let initial = had_goal.then(|| kept_green("active"));
    let mut snapshot = initial.clone();
    if stale_notice {
        let older = snapshot.as_mut().unwrap();
        older["objective"] = json!("An earlier goal");
        older["updatedAt"] = json!(1764200001);
    }
    let snapshot = snapshot_notice(snapshot);
    let changed = kept_green(if had_goal { "paused" } else { "active" });
    let mut resume = vec![json!({"result": {"thread": {"id": "thread_9"}}})];
    let mut start = vec![json!({"result": {"turn": {"id": "turn_1"}}}), live_turn_start()];
    if notice_before_get {
        resume.push(snapshot);
    } else {
        // Deliberately after both the Get reply and the turn/start reply.
        start.push(snapshot);
    }
    start.extend([live_goal_notice(changed), turn_ends()]);
    let (mut codex, told) = stand_in(json!({
        "thread/resume": resume,
        "thread/goal/get": [{"result": {"goal": initial}}],
        "turn/start": start,
    }));
    codex.start(&resumed_turn()).unwrap();
    let asked = told.asked();
    let at = |method: &str| asked.iter().position(|m| m == method).expect(method);
    assert!(at("thread/resume") < at("thread/goal/get") && at("thread/goal/get") < at("turn/start"));
    assert_eq!(codex.standing_goal().map(|g| (g.objective.as_str(), g.status)),
        had_goal.then_some(("Keep the test suite green", GoalStatus::Active)));
    let events = to_the_end(&mut codex);
    assert_eq!(goal_codes(&events), [if had_goal { "codex.goal_status" } else { "codex.goal_set" }]);
    assert_eq!(codex.standing_goal().unwrap().status, if had_goal { GoalStatus::Paused } else { GoalStatus::Active });
}

#[test]
fn resume_goal_snapshot_before_get_is_not_a_change() {
    check_resumed_snapshot(true, true, false);
}

#[test]
fn resume_goal_snapshot_after_get_is_not_a_change() {
    check_resumed_snapshot(true, false, false);
}

#[test]
fn resume_empty_snapshot_before_get_is_not_a_clear() {
    check_resumed_snapshot(false, true, false);
}

#[test]
fn resume_empty_snapshot_after_get_is_not_a_clear() {
    check_resumed_snapshot(false, false, false);
}

#[test]
fn a_stale_resume_snapshot_never_replaces_get_or_writes_a_change() {
    check_resumed_snapshot(true, true, true);
    check_resumed_snapshot(true, false, true);
}

#[test]
fn a_forked_goal_snapshot_is_not_a_change_but_later_clear_is() {
    let (codex, told) = stand_in(json!({
        "thread/fork": [
            {"result": {"thread": {"id": "thread_9"}}},
            snapshot_notice(Some(kept_green("active"))),
        ],
        "turn/start": [
            {"result": {"turn": {"id": "turn_1"}}},
            live_turn_start(), cleared_notice(), turn_ends(),
        ],
    }));
    let mut codex = codex.with_fork(true);
    codex.start(&resumed_turn()).unwrap();
    assert!(!told.asked().iter().any(|m| m == "thread/goal/get"));
    assert_eq!(codex.standing_goal().map(|g| g.status), Some(GoalStatus::Active));
    assert_eq!(goal_codes(&to_the_end(&mut codex)), ["codex.goal_cleared"]);
    assert!(codex.standing_goal().is_none());
}

#[test]
fn a_fork_without_a_snapshot_records_the_first_live_goal_change() {
    let (codex, _told) = stand_in(json!({
        "turn/start": [
            {"result": {"turn": {"id": "turn_1"}}},
            live_turn_start(), live_goal_notice(kept_green("active")), cleared_notice(), turn_ends(),
        ],
    }));
    let mut codex = codex.with_fork(true);
    codex.start(&resumed_turn()).unwrap();
    assert_eq!(goal_codes(&to_the_end(&mut codex)), ["codex.goal_set", "codex.goal_cleared"]);
}

//! A Codex teammate's standing goal, read off Codex app-server's `thread/goal/updated` and `thread/goal/cleared`
//! notifications. The JSON here is the protocol's: a `ThreadGoal` is {threadId, objective, status, tokenBudget?,
//! tokensUsed, timeUsedSeconds, createdAt, updatedAt}, its status one of active, paused, blocked, usageLimited,
//! budgetLimited or complete.
use super::*;
use serde_json::json;

fn updated(goal: Value) -> Value {
    json!({"threadId": "thread_9", "goal": goal})
}

fn kept_green() -> Value {
    json!({"threadId": "thread_9", "objective": "Keep the test suite green", "status": "active", "tokenBudget": 200000,
        "tokensUsed": 1200, "timeUsedSeconds": 42, "createdAt": 1764200000, "updatedAt": 1764200042})
}

#[test]
fn a_goal_update_reads_every_field_the_protocol_names() {
    let read = from_notification("thread/goal/updated", &updated(kept_green()));
    assert_eq!(
        read,
        Some(GoalEvent::Updated(ThreadGoal {
            thread_id: "thread_9".into(),
            objective: "Keep the test suite green".into(),
            status: GoalStatus::Active,
            token_budget: Some(200000),
            tokens_used: 1200,
            time_used_seconds: 42,
            created_at: 1764200000,
            updated_at: 1764200042,
        }))
    );
}

#[test]
fn a_goal_without_a_budget_has_none_whether_the_field_is_missing_or_null() {
    let mut goal = kept_green();
    goal.as_object_mut().unwrap().remove("tokenBudget");
    let Some(GoalEvent::Updated(read)) = from_notification("thread/goal/updated", &updated(goal.clone())) else { panic!("not read") };
    assert_eq!(read.token_budget, None);
    goal["tokenBudget"] = Value::Null;
    let Some(GoalEvent::Updated(read)) = from_notification("thread/goal/updated", &updated(goal)) else { panic!("not read") };
    assert_eq!(read.token_budget, None);
}

#[test]
fn each_status_the_protocol_names_reads_as_itself_and_one_it_does_not_as_unknown() {
    let named = [
        ("active", GoalStatus::Active),
        ("paused", GoalStatus::Paused),
        ("blocked", GoalStatus::Blocked),
        ("usageLimited", GoalStatus::UsageLimited),
        ("budgetLimited", GoalStatus::BudgetLimited),
        ("complete", GoalStatus::Complete),
        ("wandering", GoalStatus::Unknown),
        ("", GoalStatus::Unknown),
    ];
    for (said, status) in named {
        let mut goal = kept_green();
        goal["status"] = json!(said);
        let Some(GoalEvent::Updated(read)) = from_notification("thread/goal/updated", &updated(goal)) else { panic!("{said} not read") };
        assert_eq!(read.status, status, "{said}");
        assert_eq!(GoalStatus::parse(status.as_str()), status, "{said} reads back as itself");
    }
    // A status that is not text is not one Locust knows either.
    let mut goal = kept_green();
    goal["status"] = json!(3);
    let Some(GoalEvent::Updated(read)) = from_notification("thread/goal/updated", &updated(goal)) else { panic!("not read") };
    assert_eq!(read.status, GoalStatus::Unknown);
}

#[test]
fn a_cleared_notice_names_its_thread() {
    assert_eq!(
        from_notification("thread/goal/cleared", &json!({"threadId": "thread_9"})),
        Some(GoalEvent::Cleared { thread_id: "thread_9".into() })
    );
    assert_eq!(from_notification("thread/goal/cleared", &json!({})), None, "a clear of no thread is nothing");
}

#[test]
fn a_goal_update_without_a_goal_or_an_objective_is_nothing() {
    assert_eq!(from_notification("thread/goal/updated", &json!({"threadId": "thread_9"})), None);
    let mut goal = kept_green();
    goal.as_object_mut().unwrap().remove("objective");
    assert_eq!(from_notification("thread/goal/updated", &updated(goal)), None);
    // Some other notification is not a goal's.
    assert_eq!(from_notification("thread/compacted", &updated(kept_green())), None);
}

#[test]
fn a_goal_takes_its_thread_from_the_notice_when_the_goal_leaves_it_out() {
    let mut goal = kept_green();
    goal.as_object_mut().unwrap().remove("threadId");
    let Some(GoalEvent::Updated(read)) = from_notification("thread/goal/updated", &updated(goal)) else { panic!("not read") };
    assert_eq!(read.thread_id, "thread_9");
}

#[test]
fn a_set_never_sends_a_status_the_protocol_does_not_name() {
    // Codex 0.172.0's ThreadGoalStatus has six words and no "unknown": a status Locust read but could not name is left
    // out of thread/goal/set rather than sent back as a word the server would refuse. A named one is sent as itself.
    let unknown = GoalSet { objective: Some("Keep it green".into()), status: Some(GoalStatus::Unknown), token_budget: None };
    let params = set_params("thread_9", &unknown);
    assert!(params.get("status").is_none(), "{params}");
    assert_eq!(params["objective"], "Keep it green");
    let paused = GoalSet { objective: None, status: Some(GoalStatus::Paused), token_budget: None };
    assert_eq!(set_params("thread_9", &paused)["status"], "paused");
}

#[test]
fn a_reply_about_another_thread_or_missing_a_required_count_is_not_read_as_the_goal() {
    // Lane check of the dot's Goals fixes: a thread/goal/set or get reply must name the thread asked about and carry the
    // 0.172.0 ThreadGoal's required counts (tokensUsed, timeUsedSeconds); otherwise it is a broken reply, not a goal.
    assert!(ThreadGoal::parse_reply(&kept_green(), "thread_9").is_some(), "the full goal on the asked thread reads");
    let mut other = kept_green();
    other["threadId"] = json!("thread_8");
    assert!(ThreadGoal::parse_reply(&other, "thread_9").is_none(), "another thread's goal is not this one's");
    for missing in ["tokensUsed", "timeUsedSeconds"] {
        let mut goal = kept_green();
        goal.as_object_mut().unwrap().remove(missing);
        assert!(ThreadGoal::parse_reply(&goal, "thread_9").is_none(), "a reply without {missing} is broken");
    }
}

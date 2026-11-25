//! Electron `main/a-plan-that-runs-out-is-carried-on.test.ts`, the carry: one press, never twice, never onto the agent
//! that ran out, a start that fails pressed again, and Without asking only with an agent picked. Each stopped turn is a
//! Codex ledger whose plan ran out, as a real run writes it.
use super::provider_carry_on::{carried_from, stopped, stopped_in, team};
use super::*;
use crate::plan_carry_on_tests::{completed, quota};

/// A roster whose one teammate names no model: native refuses to start it ("runs in Locust for now"), the way a start
/// that fails does.
fn team_that_cannot_start(t: &Temp, plan: Value) {
    team(t, plan);
    let mut roster: Value = serde_json::from_str(&fs::read_to_string(t.store().root().join("teammates.json")).unwrap()).unwrap();
    roster["teammates"][0]["route"].as_object_mut().unwrap().remove("model");
    t.store().save_json("teammates.json", &roster).unwrap();
}

#[test]
fn one_press_starts_the_turn_on_the_agent_offered_as_a_reply_to_the_turn_that_ran_out_in_its_mode() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, Value::Null);
    stopped(&t, "mission_plan_press", "codex", &[quota()]);
    let (id, _, mode) = carry("mission_plan_press", "claude", None).unwrap();
    finished(&id);
    assert_eq!(mode, "accept-edits");
    let started = carried_from(&t, "mission_plan_press");
    assert_eq!(started.len(), 1);
    assert_eq!(started[0]["runtime"], "claude");
    assert_eq!(started[0]["mode"], "accept-edits");
    assert_eq!(started[0]["startedBy"], json!({"kind": "carry-on", "from": "codex"}));
    let prompt = started[0]["prompt"].as_str().unwrap_or_default();
    assert!(prompt.contains(&crate::carry_on::carry_on_prompt("codex", crate::carry_on::Reason::Plan)), "{prompt}");
    // Carried: a second press is refused.
    assert_eq!(carry("mission_plan_press", "claude", None).unwrap_err(), "This turn was already carried on.");
}

#[test]
fn never_twice_never_onto_the_agent_that_ran_out_never_a_turn_that_did_not_run_out() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, Value::Null);
    stopped(&t, "mission_plan_twice", "codex", &[quota()]);
    let (id, _, _) = carry("mission_plan_twice", "claude", None).unwrap();
    finished(&id);
    assert_eq!(carry("mission_plan_twice", "claude", None).unwrap_err(), "This turn was already carried on.");
    assert_eq!(carried_from(&t, "mission_plan_twice").len(), 1);
    stopped(&t, "mission_plan_same", "codex", &[quota()]);
    assert_eq!(carry("mission_plan_same", "codex", None).unwrap_err(), "Codex CLI is the agent whose plan ran out.");
    stopped(&t, "mission_plan_finished", "codex", &[completed()]);
    assert!(carry("mission_plan_finished", "claude", None).is_err());
    assert!(carried_from(&t, "mission_plan_same").is_empty() && carried_from(&t, "mission_plan_finished").is_empty());
}

#[test]
fn a_turn_that_asked_before_each_action_is_never_carried_to_an_agent_that_cannot_ask() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, Value::Null);
    stopped_in(&t, "mission_plan_each", "codex", "approve-each", &[quota()]);
    assert_eq!(
        carry("mission_plan_each", "claude", None).unwrap_err(),
        "Claude Code cannot ask before each action, so this turn was not carried on there."
    );
    let (id, _, mode) = carry("mission_plan_each", "opencode", None).unwrap();
    finished(&id);
    assert_eq!(mode, "approve-each");
    assert_eq!(carried_from(&t, "mission_plan_each")[0]["mode"], "approve-each", "the new turn runs in the stopped turn's mode");
}

#[test]
fn a_start_that_fails_can_be_pressed_again() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team_that_cannot_start(&t, Value::Null);
    stopped(&t, "mission_plan_again", "codex", &[quota()]);
    assert_eq!(carry("mission_plan_again", "claude", None).unwrap_err(), "Carry runs in Locust for now.");
    assert!(carried_from(&t, "mission_plan_again").is_empty());
    // The roster is put right (and the turn's owner, kept in the same file, recorded again).
    team(&t, Value::Null);
    crate::people::record_owner(&t.store(), "mission_plan_again", "tm_carry").unwrap();
    let (id, _, _) = carry("mission_plan_again", "claude", None).unwrap();
    finished(&id);
    assert_eq!(carried_from(&t, "mission_plan_again").len(), 1);
}

#[test]
fn without_a_press_only_with_an_agent_picked_and_without_asking_on() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, json!({"runtime": "claude", "automatic": false}));
    stopped(&t, "mission_plan_off", "codex", &[quota()]);
    carry_on_without_asking(&t.store(), "mission_plan_off");
    team(&t, Value::Null);
    stopped(&t, "mission_plan_none", "codex", &[quota()]);
    carry_on_without_asking(&t.store(), "mission_plan_none");
    assert!(carried_from(&t, "mission_plan_off").is_empty() && carried_from(&t, "mission_plan_none").is_empty());

    team(&t, json!({"runtime": "claude", "model": "sonnet", "automatic": true}));
    stopped(&t, "mission_plan_on", "codex", &[quota()]);
    carry_on_without_asking(&t.store(), "mission_plan_on");
    let started = carried_from(&t, "mission_plan_on");
    assert_eq!(started.iter().map(|m| (m["runtime"].clone(), m["model"].clone())).collect::<Vec<_>>(), [(json!("claude"), json!("sonnet"))]);
    for m in &started {
        if let Some(id) = m["missionId"].as_str() {
            finished(id);
        }
    }
    assert!(!fs::read_to_string(path_of(&t, "mission_plan_on")).unwrap().contains("host.carry_on"), "no note when it carried on");
}

#[test]
fn and_without_asking_says_why_when_it_could_not() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, json!({"runtime": "codex", "automatic": true}));
    stopped(&t, "mission_plan_itself", "codex", &[quota()]);
    carry_on_without_asking(&t.store(), "mission_plan_itself");
    assert!(carried_from(&t, "mission_plan_itself").is_empty());
    let text = fs::read_to_string(path_of(&t, "mission_plan_itself")).unwrap();
    assert!(text.contains("Codex CLI is the agent picked to carry on when a plan runs out, and its own plan ran out, so nothing was carried on."), "{text}");

    team_that_cannot_start(&t, json!({"runtime": "claude", "automatic": true}));
    stopped(&t, "mission_plan_refused", "codex", &[quota()]);
    carry_on_without_asking(&t.store(), "mission_plan_refused");
    let text = fs::read_to_string(path_of(&t, "mission_plan_refused")).unwrap();
    assert!(text.contains("Locust could not carry this on with Claude Code: Carry runs in Locust for now."), "{text}");
}

#[test]
fn a_carried_turn_is_recorded_as_locusts_own_at_schema_twenty_four_or_later() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, Value::Null);
    stopped(&t, "mission_plan_record", "codex", &[quota()]);
    let (id, _, _) = carry("mission_plan_record", "claude", None).unwrap();
    finished(&id);
    let path = ledgers(&t).into_iter().find(|p| records(p)[0]["metadata"]["continuesFrom"]["missionId"] == "mission_plan_record").unwrap();
    let header = &records(&path)[0];
    assert!(header["schemaVersion"].as_u64().unwrap_or(0) >= 24, "{header}");
    assert_eq!(header["metadata"]["startedBy"], json!({"kind": "carry-on", "from": "codex"}));
}

#[test]
// case: checks a handoff target before stopping the existing run and sends resumes through start
fn a_carry_on_target_the_organization_refuses_is_refused_before_anything_starts_and_can_be_pressed_again() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, Value::Null);
    stopped(&t, "mission_plan_policy", "codex", &[quota()]);
    let policy = crate::org_policy::parse_policy(&json!({"allowedAgents": ["codex", "opencode"]}), &mut |_| {}).unwrap();
    crate::org_policy::use_policy_on_this_thread(Some(crate::org_policy::PolicyState { path: "/admin/policy.json".into(), present: true, policy, error: None }));
    let refused = carry("mission_plan_policy", "claude", None);
    crate::org_policy::use_policy_on_this_thread(None);
    let why = refused.unwrap_err();
    assert!(why.contains("allowedAgents") && why.contains("Nothing was started"), "{why}");
    assert!(carried_from(&t, "mission_plan_policy").is_empty(), "a refused target records no turn");
    // The stopped turn was left as it was: once the target is allowed, the same press carries it on.
    let (id, _, _) = carry("mission_plan_policy", "claude", None).unwrap();
    finished(&id);
    assert_eq!(carried_from(&t, "mission_plan_policy").len(), 1);
}

// case: does not make the handed-to route the teammate's own
#[test]
fn a_carry_on_leaves_the_teammates_own_route_as_it_was() {
    // App.tsx's handOffMission sets the picker's route only for a teammate that has none; native's start does the same
    // (people::remember_route_of_start), so a teammate with a route keeps it after its turn is carried to another agent.
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, Value::Null);
    stopped(&t, "mission_plan_route", "codex", &[quota()]);
    let before = crate::people::team(&t.store()).unwrap()["teammates"][0]["route"].clone();
    assert_eq!(before["runtime"], "claude");
    let (id, _, _) = carry("mission_plan_route", "opencode", None).unwrap();
    finished(&id);
    assert_eq!(carried_from(&t, "mission_plan_route")[0]["runtime"], "opencode", "the turn ran on the agent handed to");
    assert_eq!(crate::people::team(&t.store()).unwrap()["teammates"][0]["route"], before, "and the teammate's own route is unchanged");
}

//! Electron `main/a-plan-that-runs-out-is-carried-on.test.ts` (0.774): the stopped run's card carries the work on with
//! one press, and Without asking with none, only onto the agent the person chose. The cases with no run in them,
//! Electron's events and values; the carry itself is in `native::tests::plan_carry_on`.

use crate::carry_on::{carry_on_offer, carry_on_prompt, ended_on_plan_limit, parse_plan_runs_out, plan_ran_out, PlanRunsOut, Reason};
use serde_json::{json, Value};

pub(crate) fn quota() -> Value {
    json!({"type": "run.failed", "sourceAdapter": "codex", "payload": {"kind": "quota-exhausted", "message": "You've hit your usage limit."}})
}
fn limit() -> Value {
    json!({"type": "route.limit_detected", "sourceAdapter": "claude", "payload": {"kind": "quota-exhausted", "message": "5-hour window 100% used"}})
}
fn nearly() -> Value {
    json!({"type": "route.limit_detected", "sourceAdapter": "claude", "payload": {"kind": "temporary-rate-limit", "message": "You've used 91% of your 5-hour window"}})
}
pub(crate) fn completed() -> Value {
    json!({"type": "run.completed", "sourceAdapter": "codex", "payload": {}})
}

#[test]
fn a_plan_ran_out_is_known_from_what_the_agent_said_not_from_a_warning_or_a_run_that_went_on() {
    assert!(plan_ran_out(&[quota()]));
    assert!(plan_ran_out(&[limit()]));
    assert!(!plan_ran_out(&[nearly()]));
    assert!(!plan_ran_out(&[limit(), completed()]));
    assert!(ended_on_plan_limit(Some("failed"), &[quota()]));
    // Claude Code's limit ends the run interrupted: still ran out.
    assert!(ended_on_plan_limit(Some("interrupted"), &[limit()]));
    // The person pressed Stop: not the plan running out, whatever was said before it.
    assert!(!ended_on_plan_limit(Some("cancelled"), &[limit()]));
}

#[test]
fn the_carried_turn_is_told_in_locusts_words_what_happened_then_what_to_do() {
    assert_eq!(
        carry_on_prompt("codex", Reason::Plan),
        "Codex CLI ran out of its plan's usage before it finished. Carry on with the task from where it stopped: what it already did is above, so finish what is left, the way the task asks."
    );
}

fn ready(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

fn pick(runtime: &str, model: Option<&str>) -> PlanRunsOut {
    PlanRunsOut { runtime: runtime.into(), model: model.map(str::to_owned), automatic: false }
}

#[test]
fn the_card_offers_the_agent_picked_else_the_first_ready_that_is_not_the_one_that_ran_out() {
    let ready_three = ready(&["claude", "codex", "opencode"]);
    assert_eq!(carry_on_offer("codex", None, &ready_three), Some(("claude".into(), None)));
    assert_eq!(carry_on_offer("claude", None, &ready_three), Some(("codex".into(), None)));
    assert_eq!(
        carry_on_offer("codex", Some(&pick("opencode", Some("opencode/jev-1.13-free"))), &ready_three),
        Some(("opencode".into(), Some("opencode/jev-1.13-free".into())))
    );
    // The pick is the agent that ran out, or not ready: the order instead -- a press is still the person's.
    assert_eq!(carry_on_offer("opencode", Some(&pick("opencode", None)), &ready_three), Some(("claude".into(), None)));
    assert_eq!(carry_on_offer("codex", Some(&pick("cursor", None)), &ready_three), Some(("claude".into(), None)));
    assert_eq!(carry_on_offer("codex", None, &ready(&["codex"])), None);
}

#[test]
fn the_setting_holds_only_an_agent_a_turn_can_run_on() {
    assert_eq!(parse_plan_runs_out(&json!({"runtime": "claude", "automatic": true})), Some(PlanRunsOut { runtime: "claude".into(), model: None, automatic: true }));
    assert_eq!(
        parse_plan_runs_out(&json!({"runtime": "claude", "model": "account-default", "automatic": "yes"})),
        Some(PlanRunsOut { runtime: "claude".into(), model: None, automatic: false })
    );
    assert_eq!(parse_plan_runs_out(&json!({"runtime": "gemini", "automatic": true})), None);
    assert_eq!(parse_plan_runs_out(&json!({"runtime": "omniroute", "automatic": true})), None);
    assert_eq!(parse_plan_runs_out(&Value::Null), None);
}

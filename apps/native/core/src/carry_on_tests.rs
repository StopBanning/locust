//! Carry-on against Electron's own code: `parity/handoff.json` is written by `scripts/handoff-goldens.mjs`, which runs
//! carry-on.ts, checkpoint.ts and handoff.ts over the same stopped turns.
use super::*;

fn goldens() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../parity/handoff.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn reason_word(r: Option<Reason>) -> Value {
    match r {
        Some(Reason::Plan) => json!("plan"),
        Some(Reason::Provider) => json!("provider"),
        Some(Reason::Process) => json!("process"),
        None => Value::Null,
    }
}

#[test]
fn rust_carries_each_stopped_turn_on_as_electron_does() {
    let g = goldens();
    for case in g["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let events = case["events"].as_array().unwrap().clone();
        let reason = carry_on_reason(&events);
        assert_eq!(reason_word(reason), case["reason"], "{name}: reason");
        assert_eq!(reason.map(|r| carry_on_prompt("codex", r)).map(Value::from).unwrap_or(Value::Null), case["instruction"], "{name}: instruction");
        assert_eq!(how_turn_ended(&events).map(Value::from).unwrap_or(Value::Null), case["ended"], "{name}: ended");

        let prompt = case["prompt"].as_str().unwrap();
        let meta = json!({"missionId": "mission_prior", "runId": "run_prior", "prompt": prompt, "runtime": "codex"});
        let checkpoint = reconcile(&meta, &events, false, "route-switch", 1, "2026-11-12T03:00:00.000Z");
        assert_eq!(checkpoint, case["checkpoint"], "{name}: checkpoint");

        let earlier: Vec<EarlierTurn> = case["earlier"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| EarlierTurn { asked: t["asked"].as_str().unwrap().into(), answered: t["answered"].as_str().map(str::to_owned) })
            .collect();
        let briefing = compose_handoff_prompt(prompt, &checkpoint, "Codex CLI", case["next"].as_str(), &earlier, how_turn_ended(&events));
        let want = &case["briefing"];
        match briefing {
            None => assert!(want.is_null(), "{name}: Rust composed no brief"),
            Some(b) => {
                assert_eq!(b.prompt, want["prompt"].as_str().unwrap(), "{name}: brief");
                assert_eq!(json!(b.omitted), want["omitted"], "{name}: omitted");
                assert_eq!(b.task_clipped, want["taskClipped"] == true, "{name}: clipped");
            }
        }
    }
    for r in g["refusals"].as_array().unwrap() {
        let got = carry_on_refusal(r["mode"].as_str().unwrap(), r["to"].as_str().unwrap());
        assert_eq!(got.map(Value::from).unwrap_or(Value::Null), r["refusal"], "{r}");
    }
}

#[test]
fn a_ledger_cut_short_is_never_safe_to_carry_on() {
    let meta = json!({"missionId": "m", "runId": "r"});
    let c = reconcile(&meta, &[], true, "route-switch", 2, "2026-11-12T03:00:00.000Z");
    assert_eq!(c["resumeSafety"], "unsafe");
    assert_eq!(c["epoch"], 2);
}

#[test]
fn a_rescue_with_no_reply_is_refused_rather_than_clipped() {
    let meta = json!({"missionId": "m", "runId": "r"});
    let c = reconcile(&meta, &[], false, "route-switch", 1, "2026-11-12T03:00:00.000Z");
    assert!(compose_handoff_prompt(&"x".repeat(9000), &c, "Codex CLI", None, &[], None).is_none());
}

#[test]
fn the_card_offers_the_persons_pick_when_it_can_run_else_the_first_ready_in_electrons_order() {
    let ready: Vec<String> = ["codex", "opencode"].iter().map(|r| (*r).to_owned()).collect();
    let pick = PlanRunsOut { runtime: "opencode".into(), model: Some("opencode/free".into()), automatic: false };
    assert_eq!(carry_on_offer("codex", Some(&pick), &ready), Some(("opencode".into(), Some("opencode/free".into()))));
    // The pick is the agent that stopped, or not ready: the first ready other agent.
    assert_eq!(carry_on_offer("opencode", Some(&pick), &ready), Some(("codex".into(), None)));
    let away = PlanRunsOut { runtime: "claude".into(), model: None, automatic: true };
    assert_eq!(carry_on_offer("codex", Some(&away), &ready), Some(("opencode".into(), None)));
    assert_eq!(carry_on_offer("codex", None, &["codex".to_owned()]), None);
    assert_eq!(parse_plan_runs_out(&json!({"runtime": "gemini"})), None);
    assert_eq!(parse_plan_runs_out(&json!({"runtime": "claude", "model": "account-default", "automatic": true})), Some(away));
}

#[test]
fn a_cold_follow_up_quotes_the_conversation_oldest_first_and_alone_says_nothing_extra() {
    use crate::carry_on::{compose_cold_follow_up, EarlierTurn};
    let turns = vec![
        EarlierTurn { asked: "Fix the cart".into(), answered: Some("Fixed it.".into()) },
        EarlierTurn { asked: "   ".into(), answered: Some("unquoted".into()) },
        EarlierTurn { asked: "And the tax?".into(), answered: None },
    ];
    assert_eq!(
        compose_cold_follow_up(&turns, "Why?", false),
        "This conversation has earlier turns, but not in your session: it could not be resumed. What was said, oldest first:\n\n- Asked: \"Fix the cart\" -- answered: \"Fixed it.\"\n- Asked: \"And the tax?\" -- no reply was recorded.\n\nThe person now asks:\n\nWhy?"
    );
    assert!(compose_cold_follow_up(&turns, "Why?", true).starts_with("The person went back to an earlier point in this conversation and changed their message there."));
    assert_eq!(compose_cold_follow_up(&[], "Why?", false), "Why?");
}

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

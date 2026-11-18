//! Electron's `a-host-started-run-is-not-retried-as-your-message.test.tsx`, with Electron's values and words. The
//! window's own drawing of it is `src/ui/run_again_tests.rs`.
use super::*;
use serde_json::json;

const PROMPT: &str = "Atlas sent you a message; it is quoted below.";

fn failed_events() -> Vec<Value> {
    vec![json!({"id":"e","runId":"r","sequence":1,"type":"run.failed","sourceAdapter":"codex",
        "occurredAt":"2026-10-06T12:00:00.000Z","payload":{"message":"The CLI refused its arguments."}})]
}

fn turn(kind: Option<&'static str>) -> RunAgainTurn<'static> {
    RunAgainTurn { prompt: PROMPT, started_by_kind: kind, ..Default::default() }
}

fn offer(turn: &RunAgainTurn<'_>) -> Option<RunAgainOffer> {
    run_again_offer(turn, "failed", runtime_never_started(&failed_events()), false)
}

#[test]
fn withholds_run_it_again_for_a_relay_and_never_makes_its_briefing_your_bubble() {
    let relay = turn(Some("relay"));
    assert_eq!(offer(&relay), Some(RunAgainOffer::Note("The message is still waiting. It will be answered on this teammate's next run.")));
    assert!(!person_prompt(relay.started_by_kind));
}

#[test]
fn still_offers_run_it_again_for_your_own_never_started_run() {
    let own = RunAgainTurn { prompt: "Check the migration.", ..Default::default() };
    assert_eq!(offer(&own), Some(RunAgainOffer::RunAgain));
    assert!(person_prompt(own.started_by_kind));
}

#[test]
fn keeps_a_host_started_run_with_its_host_context() {
    for kind in ["routine", "resume", "room", "side", "compare", "judge"] {
        match offer(&turn(Some(kind))) {
            Some(RunAgainOffer::Note(note)) => assert!(note.contains("original control"), "{kind}: {note}"),
            other => panic!("{kind}: {other:?}"),
        }
    }
}

#[test]
fn withholds_a_composer_retry_for_live_and_recovered_handoffs() {
    assert_ne!(offer(&RunAgainTurn { handoff: true, ..turn(None) }), Some(RunAgainOffer::RunAgain));
    assert_ne!(offer(&RunAgainTurn { continues_reason: Some("route-switch"), ..turn(None) }), Some(RunAgainOffer::RunAgain));
}

#[test]
fn keeps_ordinary_follow_ups_and_person_authored_origins_retryable() {
    assert_eq!(offer(&RunAgainTurn { continues_reason: Some("follow-up"), ..turn(None) }), Some(RunAgainOffer::RunAgain));
    assert_eq!(offer(&turn(Some("tag"))), Some(RunAgainOffer::RunAgain));
    assert_eq!(offer(&turn(Some("terminal"))), Some(RunAgainOffer::RunAgain));
}

#[test]
fn keeps_the_existing_failed_only_nonempty_and_never_started_boundaries() {
    let never = runtime_never_started(&failed_events());
    assert_eq!(run_again_offer(&turn(None), "failed", never, true), None);
    assert_eq!(run_again_offer(&turn(None), "completed", never, false), None);
    assert_eq!(run_again_offer(&RunAgainTurn { prompt: " ", ..turn(None) }, "failed", never, false), None);
    let started = [json!({"id":"e","runId":"r","sequence":1,"type":"run.started","sourceAdapter":"codex",
        "occurredAt":"2026-10-06T12:00:00.000Z","payload":{"runtimeThreadId":"thread_1","evidence":{"redacted":true}}})];
    assert_eq!(run_again_offer(&turn(None), "failed", runtime_never_started(&started), false), None);
}

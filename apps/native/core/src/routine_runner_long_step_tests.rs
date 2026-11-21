//! A ROUTINE STEP TOO LONG TO SEND IS REFUSED BEFORE IT STARTS (Electron
//! `a-routine-step-too-long-is-refused-before-it-starts.test.ts`, A5.1): running one saved before the limit. A longer step
//! was written down as dispatching, refused by the mission service, and from step 2 on held as "Dispatch not confirmed
//! ... Review external work": a person sent to look for side effects of a step that never began.
use super::*;
use crate::hand_off::STEP_BUDGET;

fn long() -> String {
    "x".repeat(STEP_BUDGET + 1)
}

fn saved_before(steps: Vec<String>) -> Value {
    with(
        json!({"routineId": "rt_1", "name": "Nightly tidy", "teammateId": "tm_wren",
            "route": {"runtime": "cursor", "model": "composer-2.5", "mode": "ask"},
            "learnedFrom": [], "createdAt": "2026-09-05T00:00:00.000Z", "runs": 0}),
        json!({ "steps": steps }),
    )
}

// case: step 1: refused with the reason, nothing started, nothing recorded
#[test]
fn step_1_refused_with_the_reason_nothing_started_nothing_recorded() {
    let mem = Mem::with(vec![saved_before(vec![long(), "Say done.".into()])]);
    let host = Arc::new(Fake::default());
    let r = runner(&mem, &host);
    let message = refused(r.run("rt_1", None));
    assert!(message.contains("Step 1 is too long to send"), "{message}");
    assert!(message.contains("Nothing was started."), "{message}");
    assert!(host.starts().is_empty());
    assert!(mem.execution("rt_1").is_null());
    assert!(mem.runs().is_empty());
}

// case: step 2: held for what it is -- too long, nothing started -- not as an uncertain dispatch
#[test]
fn step_2_held_for_what_it_is_too_long_nothing_started_not_as_an_uncertain_dispatch() {
    let mem = Mem::with(vec![saved_before(vec!["Read status.ts.".into(), long()])]);
    let host = Arc::new(Fake::default());
    let r = runner(&mem, &host);
    ok(r.run("rt_1", None));
    host.phase("mission_1", Phase::Completed);
    r.on_run_ended("mission_1").unwrap();
    assert_eq!(host.starts().len(), 1);
    let execution = mem.execution("rt_1");
    assert_eq!(execution["status"], "held");
    let reason = execution["reason"].as_str().unwrap_or_default().to_owned();
    assert!(reason.contains("Step 2 is too long to send"), "{reason}");
    assert!(!reason.contains("Dispatch not confirmed"), "{reason}");
    let last = host.notices().last().cloned().unwrap_or_default();
    assert!(last.contains("stopped before step 2 of 2: Step 2 is too long to send"), "{last}");
    // And it STAYS said: a reconcile -- the Routines screen runs one -- must not trade the reason for "The saved step
    // completed" and a Continue that would run into the same refusal (caught by Electron's 0.316 drive).
    r.reconcile().unwrap();
    let after = mem.execution("rt_1");
    assert!(after["reason"].as_str().unwrap_or_default().contains("Step 2 is too long to send"), "{after}");
    assert_eq!(after["canContinue"], false);
    assert_eq!(host.starts().len(), 1);
}

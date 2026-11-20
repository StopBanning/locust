//! Electron's `a-refusal-keeps-the-reason-it-was-refused-for.test.ts` (4 cases): a routine whose start the runtime
//! refused at dispatch said, after a reconcile, that "the app stopped before saving a mission receipt". A hold decided at
//! dispatch is final; the generic sentence is only for a dispatch that never answered.
use super::*;
use super::routine_runner_ts::routine;

const REFUSAL: &str = "Cursor Agent cannot be held read-only on this system: its sandbox needs macOS or Linux, and plan mode alone does not stop it editing files. Choose \"Edit\" if it may change this workspace, or run this on Codex CLI or Claude Code.";

fn refusing() -> (Arc<Mem>, Arc<Fake>) {
    let mem = Mem::with(vec![routine(json!({}))]);
    let host = Arc::new(Fake::default());
    *host.refuse.lock().unwrap() = Some(StartError::Refused { code: "RUNTIME_START_FAILED".into(), message: REFUSAL.into() });
    (mem, host)
}

// case: records the runtime’s own words when the dispatch is refused
#[test]
fn records_the_runtimes_own_words_when_the_dispatch_is_refused() {
    let (mem, host) = refusing();
    let _ = runner(&mem, &host).run("rt_1", None);
    let e = mem.execution("rt_1");
    assert_eq!(e["status"], "held");
    assert!(e["reason"].as_str().unwrap_or("").contains("cannot be held read-only"), "{e}");
    assert_eq!((&e["settledAtDispatch"], &e["canContinue"]), (&json!(true), &json!(false)));
}

// case: does not rewrite that reason on the next reconcile
#[test]
fn does_not_rewrite_that_reason_on_the_next_reconcile() {
    let (mem, host) = refusing();
    let r = runner(&mem, &host);
    let _ = r.run("rt_1", None);
    r.reconcile().unwrap();
    r.reconcile().unwrap();
    let reason = mem.execution("rt_1")["reason"].as_str().unwrap_or("").to_owned();
    assert!(reason.contains("cannot be held read-only") && !reason.contains("the app stopped"), "{reason}");
}

// case: survives an actual restart, which is when the old message appeared
#[test]
fn survives_an_actual_restart() {
    let (mem, host) = refusing();
    let _ = runner(&mem, &host).run("rt_1", None);
    runner(&mem, &host).reconcile().unwrap();
    let e = mem.execution("rt_1");
    assert!(e["reason"].as_str().unwrap_or("").contains("cannot be held read-only"), "{e}");
    assert_eq!(e["settledAtDispatch"], true);
}

// case: still reports a genuinely interrupted dispatch as uncertain
#[test]
fn still_reports_a_genuinely_interrupted_dispatch_as_uncertain() {
    let mem = Mem::with(vec![routine(json!({}))]);
    let host = Arc::new(Fake::default());
    ok(runner(&mem, &host).run("rt_1", None));
    // Back to what a stop between spawn and receipt leaves: a dispatch in flight with no mission recorded.
    mem.edit("rt_1", |r| {
        let e = &mut r["execution"];
        if let Some(o) = e.as_object_mut() {
            o.remove("missionId");
            o.remove("runId");
        }
        e["status"] = json!("dispatching");
    });
    runner(&mem, &host).reconcile().unwrap();
    let reason = mem.execution("rt_1")["reason"].as_str().unwrap_or("").to_owned();
    assert!(reason.contains("the app stopped before saving a mission receipt"), "{reason}");
}

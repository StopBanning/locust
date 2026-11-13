//! routine-recovery.test.ts: durable routine recovery, policy (c), on the native store itself.
use super::*;
use crate::routines::progress::{self, StoreRoutines};
use crate::store::Store;
use std::sync::atomic::AtomicBool;

type SaveFault = Box<dyn FnMut(&Value) -> Option<String> + Send>;

/// The store, with the faults `vi.spyOn(...).mockRejectedValueOnce` gave it.
struct Flaky {
    inner: StoreRoutines,
    fail_save: Mutex<Option<SaveFault>>,
    fail_record_once: AtomicBool,
}

impl Routines for Flaky {
    fn get(&self, id: &str) -> Result<Option<Value>, String> {
        self.inner.get(id)
    }
    fn list(&self) -> Result<Vec<Value>, String> {
        self.inner.list()
    }
    fn record_run(&self, id: &str, attempt: &str, staged: Option<Value>, failed: Option<String>) -> Result<(), String> {
        if self.fail_record_once.swap(false, std::sync::atomic::Ordering::SeqCst) {
            return Err("disk full".into());
        }
        self.inner.record_run(id, attempt, staged, failed)
    }
    fn save_progress(&self, id: &str, execution: &Value, expected: Option<&str>) -> Result<(), String> {
        if let Some(fault) = self.fail_save.lock().unwrap().as_mut()
            && let Some(error) = fault(execution)
        {
            return Err(error);
        }
        self.inner.save_progress(id, execution, expected)
    }
    fn clear_progress(&self, id: &str, attempt: &str) -> Result<(), String> {
        self.inner.clear_progress(id, attempt)
    }
    fn abandon(&self, id: &str, attempt: &str) -> Result<(), String> {
        self.inner.abandon(id, attempt)
    }
    fn keep_schedule(&self, id: &str, attempt: &str, kept_at: &str) -> Result<(), String> {
        self.inner.keep_schedule(id, attempt, kept_at)
    }
    fn record_miss(&self, id: &str, due: &str, recorded: &str) -> Result<(), String> {
        self.inner.record_miss(id, due, recorded)
    }
}

struct F {
    root: PathBuf,
    store: Store,
    id: String,
    created: DateTime<Utc>,
    host: Arc<Fake>,
}

impl Drop for F {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

impl F {
    fn restart(&self) -> RoutineRunner {
        RoutineRunner::new(Arc::new(StoreRoutines(self.store.clone())), self.host.clone())
    }
    fn flaky(&self) -> (Arc<Flaky>, RoutineRunner) {
        let flaky = Arc::new(Flaky { inner: StoreRoutines(self.store.clone()), fail_save: Mutex::new(None), fail_record_once: AtomicBool::new(false) });
        (flaky.clone(), RoutineRunner::new(flaky, self.host.clone()))
    }
    fn read(&self) -> Value {
        progress::get(&self.store, &self.id).unwrap().unwrap()
    }
    fn execution(&self) -> Value {
        self.read()["execution"].clone()
    }
    fn at(&self, after_ms: i64) -> DateTime<Utc> {
        self.created + chrono::Duration::milliseconds(after_ms)
    }
    fn steps(&self, steps: Value) {
        crate::routines::save(&self.store, json!({"routineId": self.id, "name": "Release", "steps": steps})).unwrap();
    }
    fn complete(&self, mission: &str) {
        self.host.phase(mission, Phase::Completed);
    }
    fn started(&self) -> usize {
        self.host.starts().len()
    }
    fn request(&self, decision: Decision) -> RecoveryRequest {
        let e = self.execution();
        decide(&self.id, &e["attemptId"], &e["step"], decision)
    }
    fn write(&self, f: impl FnOnce(&mut Value)) {
        let mut file = crate::routines::list(&self.store).unwrap();
        f(&mut file["routines"][0]);
        self.store.save_json("routines.json", &file).unwrap();
    }
}

fn fixture() -> F {
    let root = std::env::temp_dir().join(format!("locust-recovery-{}", crate::native::uuid_v4().unwrap()));
    let electron = root.join("electron");
    std::fs::create_dir_all(&electron).unwrap();
    std::fs::write(electron.join("routines.json"), json!({"schemaVersion": 1, "routines": []}).to_string()).unwrap();
    std::fs::write(
        electron.join("teammates.json"),
        json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_one", "name": "One", "hue": "blue", "role": "Custom",
            "createdAt": "2026-11-01T00:00:00Z", "folder": root.to_string_lossy(), "route": {"runtime": "codex", "model": "account-default", "mode": "ask"}}]})
        .to_string(),
    )
    .unwrap();
    let store = Store::new(root.join("native"), electron).unwrap();
    crate::routines::save(
        &store,
        json!({"name": "Release", "teammateId": "tm_one", "steps": ["Open the PR.", "Summarise the changes.", "Report the result."], "schedule": {"kind": "every", "hours": 6}}),
    )
    .unwrap();
    let saved = progress::all(&store).unwrap()[0].clone();
    let host = Arc::new(Fake {
        workspace: Mutex::new(text(&saved["workspaceId"]).to_owned()),
        peers: Box::new(|_| Some(peer("tm_one", "One"))),
        ..Fake::default()
    });
    F {
        root,
        store,
        id: text(&saved["routineId"]).to_owned(),
        created: at(text(&saved["createdAt"])),
        host,
    }
}

#[test]
fn restart_proof_step_2_survives_a_fresh_store_and_runner_without_replaying_step_1() {
    let f = fixture();
    let first = f.restart();
    ok(first.run(&f.id, None));
    f.complete("mission_1");
    first.on_run_ended("mission_1").unwrap();
    let e = f.execution();
    assert_eq!((&e["step"], &e["missionId"], &e["status"]), (&json!(2), &json!("mission_2"), &json!("running")));
    let next = f.restart();
    next.tick(f.at(0)).unwrap();
    let e = f.execution();
    assert_eq!((&e["step"], &e["status"], &e["canContinue"]), (&json!(2), &json!("held"), &json!(false)));
    assert_eq!(f.started(), 2);
    assert!(next.run(&f.id, None).unwrap().is_err());
    assert_eq!(f.started(), 2);
}

#[test]
fn completion_accounting_proof_step_1_start_does_not_stamp_last_run_at_or_consume_the_interval() {
    let f = fixture();
    let runner = f.restart();
    ok(runner.run(&f.id, None));
    assert!(f.read().get("lastRunAt").is_none());
    assert_eq!(f.read()["runs"], 0);
    f.complete("mission_1");
    runner.on_run_ended("mission_1").unwrap();
    let next = f.restart();
    next.tick(f.at(0)).unwrap(); // Not due yet: reconciliation must still run.
    assert_eq!(f.execution()["status"], "held");
    f.complete("mission_2"); // The runtime finished while the app was gone.
    next.tick(f.at(60_000)).unwrap();
    assert_eq!(f.execution()["canContinue"], true);
    assert_eq!(f.started(), 2); // Only reconciliation retried, never effects.
    assert!(f.host.updates.lock().unwrap().contains(&Update::RecoveryChanged));
    ok(next.recover(&f.request(Decision::Continue)));
    assert_eq!(f.started(), 3);
    assert!(f.read().get("lastRunAt").is_none());
    f.complete("mission_3");
    next.on_run_ended("mission_3").unwrap();
    let file = crate::routines::list(&f.store).unwrap();
    assert_eq!(file["routines"][0]["runs"], 1);
    assert!(file["routines"][0]["lastRunAt"].is_string());
    assert!(file["routines"][0].get("execution").is_none());
    next.on_run_ended("mission_3").unwrap();
    f.restart().tick(f.at(0)).unwrap();
    assert_eq!(f.read()["runs"], 1);
}

#[test]
fn records_dispatch_intent_before_spawn_and_a_thrown_start_is_uncertain_after_restart() {
    let f = fixture();
    let (store, id) = (f.store.clone(), f.id.clone());
    f.host.once.lock().unwrap().push_back(Box::new(move |_| {
        let e = progress::get(&store, &id).unwrap().unwrap()["execution"].clone();
        assert_eq!((&e["status"], &e["step"]), (&json!("dispatching"), &json!(1)));
        assert!(e.get("missionId").is_none());
        Err(StartError::Threw("crash after spawn".into()))
    }));
    assert_eq!(f.restart().run(&f.id, None).unwrap_err(), "crash after spawn");
    f.restart().tick(f.at(4 * 86_400_000)).unwrap();
    let e = f.execution();
    assert_eq!((&e["status"], &e["canContinue"]), (&json!("held"), &json!(false)));
    assert!(text(&e["reason"]).contains("uncertain"));
    assert_eq!(f.started(), 1);
}

#[test]
fn failure_to_save_intent_prevents_dispatch_and_failure_to_save_the_returned_id_never_permits_replay() {
    let f = fixture();
    let (flaky, runner) = f.flaky();
    let mut once = true;
    *flaky.fail_save.lock().unwrap() = Some(Box::new(move |_| std::mem::take(&mut once).then(|| "disk full".into())));
    assert_eq!(runner.run(&f.id, None).unwrap_err(), "disk full");
    assert_eq!(f.started(), 0);
    *flaky.fail_save.lock().unwrap() = Some(Box::new(|e| (e["status"] == "running").then(|| "receipt write failed".into())));
    let (_, again) = (0, RoutineRunner::new(flaky.clone(), f.host.clone()));
    assert_eq!(again.run(&f.id, None).unwrap_err(), "receipt write failed");
    assert_eq!(f.execution()["status"], "dispatching");
    f.restart().tick(f.at(0)).unwrap();
    assert_eq!(f.execution()["status"], "held");
    assert_eq!(f.started(), 1);
}

#[test]
fn reconciles_final_completion_after_a_crash_exactly_once_without_dispatching_anything() {
    let f = fixture();
    f.steps(json!(["Only step."]));
    ok(f.restart().run(&f.id, None));
    f.complete("mission_1");
    f.restart().tick(f.at(0)).unwrap();
    f.restart().tick(f.at(0)).unwrap();
    assert_eq!(f.read()["runs"], 1);
    assert!(f.read().get("execution").is_none());
    assert_eq!(f.started(), 1);
}

#[test]
fn a_failed_completion_write_leaves_progress_and_retries_accounting_on_next_reconciliation() {
    let f = fixture();
    f.steps(json!(["Only step."]));
    ok(f.restart().run(&f.id, None));
    f.complete("mission_1");
    let (flaky, runner) = f.flaky();
    flaky.fail_record_once.store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(runner.reconcile().unwrap_err(), "disk full");
    assert!(f.read().get("execution").is_some());
    assert_eq!(f.read()["runs"], 0);
    f.restart().reconcile().unwrap();
    assert_eq!(f.read()["runs"], 1);
    assert!(f.read().get("execution").is_none());
}

#[test]
fn refuses_stale_and_duplicate_decisions_and_continuation_uses_the_saved_route_and_steps() {
    let f = fixture();
    ok(f.restart().run(&f.id, None));
    f.complete("mission_1");
    let next = f.restart();
    next.reconcile().unwrap();
    let e = f.execution();
    crate::routines::save(&f.store, json!({"routineId": f.id, "name": "Edited", "steps": ["Different", "Publish everything"]})).unwrap();
    assert!(next.recover(&decide(&f.id, &json!("old_attempt"), &e["step"], Decision::Continue)).unwrap().is_err());
    let request = decide(&f.id, &e["attemptId"], &e["step"], Decision::Continue);
    let answers = [next.recover(&request).unwrap().is_ok(), next.recover(&request).unwrap().is_ok()];
    assert_eq!(answers, [true, false]);
    assert_eq!(f.started(), 2);
    let last = f.host.starts().last().cloned().unwrap();
    assert_eq!((last.prompt.as_str(), last.follow_up_of.as_deref()), ("Summarise the changes.", Some("mission_1")));
    f.complete("mission_2");
    next.on_run_ended("mission_2").unwrap();
    assert_eq!(f.host.starts().last().unwrap().prompt, "Report the result.");
    assert!(next.recover(&RecoveryRequest { decision: Decision::Abandon, ..request }).unwrap().is_err());
}

#[test]
fn keep_an_interrupted_last_step_is_put_down_with_the_schedule_kept_and_the_next_run_comes_when_due_from_step_1() {
    let f = fixture();
    f.steps(json!(["Open the PR.", "Report the result."]));
    let first = f.restart();
    ok(first.run(&f.id, None));
    f.complete("mission_1");
    first.on_run_ended("mission_1").unwrap();
    let next = f.restart();
    next.reconcile().unwrap();
    let e = f.execution();
    assert_eq!((&e["step"], &e["of"], &e["status"], &e["canContinue"]), (&json!(2), &json!(2), &json!("held"), &json!(false)));
    let deciding = Utc::now();
    let request = f.request(Decision::Keep);
    ok(next.recover(&request));
    let kept = f.read();
    assert!(kept.get("execution").is_none());
    assert_eq!(kept["schedule"], json!({"kind": "every", "hours": 6}));
    assert_eq!(kept["runs"], 0);
    // Six hours from the DECISION, not from when the attempt started (0.404).
    let kept_at = at(text(&kept["lastRunAt"]));
    assert!(kept_at >= deciding - chrono::Duration::milliseconds(1) && kept_at <= Utc::now());
    next.tick(kept_at + chrono::Duration::milliseconds(60_000)).unwrap();
    assert_eq!(f.started(), 2);
    next.tick(kept_at + chrono::Duration::milliseconds(6 * 3_600_000 + 60_000)).unwrap();
    assert_eq!(f.started(), 3);
    assert_eq!(f.host.starts().last().unwrap().prompt, "Open the PR.");
    // A decision about an attempt that is gone is stale.
    assert!(next.recover(&request).unwrap().is_err());
}

#[test]
fn keep_a_routine_held_past_its_interval_does_not_start_again_the_moment_it_is_kept() {
    let f = fixture();
    ok(f.restart().run(&f.id, None));
    let next = f.restart();
    next.reconcile().unwrap();
    // Held for seven hours before anyone decided: past its six-hour interval.
    let long_ago = iso(Utc::now() - chrono::Duration::hours(7));
    f.write(|r| r["execution"]["startedAt"] = json!(long_ago));
    assert_eq!(f.execution()["startedAt"], json!(long_ago));
    let calls = f.started();
    ok(next.recover(&f.request(Decision::Keep)));
    let decided = Utc::now();
    next.tick(decided + chrono::Duration::milliseconds(60_000)).unwrap();
    assert_eq!(f.started(), calls);
    next.tick(decided + chrono::Duration::milliseconds(6 * 3_600_000 + 60_000)).unwrap();
    assert_eq!(f.started(), calls + 1);
}

#[test]
fn abandon_is_durable_acknowledgement_removes_the_schedule_and_never_claims_completion() {
    let f = fixture();
    ok(f.restart().run(&f.id, None));
    let next = f.restart();
    next.reconcile().unwrap();
    assert!(next.recover(&f.request(Decision::Continue)).unwrap().is_err());
    ok(next.recover(&f.request(Decision::Abandon)));
    f.restart().tick(f.at(4 * 86_400_000)).unwrap();
    let r = f.read();
    assert_eq!(r["execution"]["status"], "abandoned");
    assert!(r.get("schedule").is_none());
    assert_eq!(r["runs"], 0);
    assert!(r.get("lastRunAt").is_none());
    assert_eq!(f.started(), 1);
}

#[test]
fn question_and_unreadable_question_receipts_hold_even_the_final_step_without_counting() {
    for throws in [false, true] {
        let f = fixture();
        f.steps(json!(["Only step."]));
        ok(f.restart().run(&f.id, None));
        f.complete("mission_1");
        if throws {
            f.host.asked_throws.store(true, std::sync::atomic::Ordering::SeqCst);
        } else {
            f.host.asked.lock().unwrap().as_mut().unwrap().insert("mission_1".into());
        }
        f.restart().reconcile().unwrap();
        let e = f.execution();
        assert_eq!((&e["status"], &e["canContinue"]), (&json!("held"), &json!(false)));
        assert_eq!(f.read()["runs"], 0);
    }
}

#[test]
fn a_different_workspace_cannot_continue_an_otherwise_complete_step() {
    let f = fixture();
    ok(f.restart().run(&f.id, None));
    f.complete("mission_1");
    *f.host.workspace.lock().unwrap() = "ws_other".into();
    let next = f.restart();
    next.reconcile().unwrap();
    assert!(text(&f.execution()["reason"]).contains("original workspace"));
    assert!(next.recover(&f.request(Decision::Continue)).unwrap().is_err());
    assert_eq!(f.started(), 1);
}

#[test]
fn a_malformed_progress_receipt_is_held_instead_of_dropped_and_replayed() {
    let f = fixture();
    f.write(|r| r["execution"] = json!({"nonsense": true}));
    f.restart().tick(f.at(4 * 86_400_000)).unwrap();
    assert_eq!(f.execution()["status"], "held");
    assert_eq!(f.started(), 0);
}

#[test]
fn store_refuses_overwriting_an_active_attempt_and_duplicate_completion() {
    let f = fixture();
    ok(f.restart().run(&f.id, None));
    let e = f.execution();
    let attempt = text(&e["attemptId"]).to_owned();
    assert!(progress::save_progress(&f.store, &f.id, &e, None).unwrap_err().contains("changed"));
    assert!(progress::record_run(&f.store, &f.id, &attempt, None, None).unwrap_err().contains("completion"));
    let mut last = e.clone();
    last["step"] = json!(3);
    progress::save_progress(&f.store, &f.id, &last, Some(&attempt)).unwrap();
    progress::record_run(&f.store, &f.id, &attempt, None, None).unwrap();
    assert!(progress::record_run(&f.store, &f.id, &attempt, None, None).unwrap_err().contains("completion"));
    assert_eq!(f.read()["runs"], 1);
}

#[test]
fn a_returned_spawn_boundary_failure_remains_uncertain_and_does_not_use_routine_blocked_retry_wording() {
    let f = fixture();
    *f.host.refuse.lock().unwrap() = Some(StartError::Refused { code: "RUNTIME_START_FAILED".into(), message: "spawn result unavailable".into() });
    // Seven hours past the routine's own creation, so it is due.
    let later = f.at(7 * 3_600_000);
    let runner = f.restart();
    runner.tick(later).unwrap();
    runner.tick(later + chrono::Duration::milliseconds(60_000)).unwrap();
    assert_eq!(f.started(), 1);
    assert_eq!(f.execution()["status"], "held");
    assert!(f.host.blocked().is_empty());
}

#[test]
fn same_session_completion_persistence_failures_are_reconciled_on_the_next_tick() {
    let f = fixture();
    f.steps(json!(["Only step."]));
    let (flaky, runner) = f.flaky();
    ok(runner.run(&f.id, None));
    f.complete("mission_1");
    flaky.fail_record_once.store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(runner.on_run_ended("mission_1").unwrap_err(), "disk full");
    runner.tick(f.at(0)).unwrap();
    assert_eq!(f.read()["runs"], 1);
    assert!(f.read().get("execution").is_none());
    assert_eq!(f.started(), 1);
}

#[test]
fn an_old_step_acknowledgement_cannot_abandon_a_later_held_step_of_the_same_attempt() {
    let f = fixture();
    ok(f.restart().run(&f.id, None));
    f.complete("mission_1");
    let second = f.restart();
    second.reconcile().unwrap();
    let old = f.execution();
    ok(second.recover(&decide(&f.id, &old["attemptId"], &old["step"], Decision::Continue)));
    let third = f.restart();
    third.reconcile().unwrap();
    assert!(third.recover(&decide(&f.id, &old["attemptId"], &old["step"], Decision::Abandon)).unwrap().is_err());
    let e = f.execution();
    assert_eq!((&e["status"], &e["step"]), (&json!("held"), &json!(2)));
    assert!(f.read().get("schedule").is_some());
}

#[test]
fn repeated_reconciliation_does_not_create_an_event_reload_feedback_loop() {
    let f = fixture();
    ok(f.restart().run(&f.id, None));
    let runner = f.restart();
    runner.reconcile().unwrap();
    let count = f.host.updates.lock().unwrap().len();
    runner.reconcile().unwrap();
    runner.reconcile().unwrap();
    assert_eq!(f.host.updates.lock().unwrap().len(), count);
    assert_eq!(f.execution()["status"], "held");
}

#[test]
fn the_stores_reads_drop_what_does_not_read_and_never_the_routine() {
    // a-routine-works-in-a-copy / keeps-going-until-its-check-passes: parsedRoutine's own cases.
    let f = fixture();
    let staged = json!({"attemptId": "a1", "finishedAt": "2026-10-01T10:00:00.000Z", "folder": "C:/work", "changed": ["receipt.md"], "deleted": []});
    f.write(|r| {
        r["inCopy"] = json!(true);
        r["staged"] = staged.clone();
        r["untilCheck"] = json!({"tries": 3});
        r["lastFailed"] = json!("The check still fails.");
    });
    let read = f.read();
    assert_eq!((&read["inCopy"], &read["staged"], &read["untilCheck"], &read["lastFailed"]), (&json!(true), &staged, &json!({"tries": 3}), &json!("The check still fails.")));
    f.write(|r| {
        r["staged"]["changed"] = json!([42]);
        r["untilCheck"] = json!({"tries": 99});
        r["lastFailed"] = json!("x".repeat(601));
    });
    let read = f.read();
    assert_eq!((&read["name"], &read["inCopy"]), (&json!("Release"), &json!(true)));
    assert!(read.get("staged").is_none() && read.get("untilCheck").is_none() && read.get("lastFailed").is_none());
    f.write(|r| r["lastFailed"] = json!(42));
    assert!(f.read().get("lastFailed").is_none());
    let tries: Vec<bool> = [json!(0), json!(1), json!(5), json!(6), json!(2.5)].iter().map(|t| progress::valid_goal(&json!({"tries": t}))).collect();
    assert_eq!(tries, [false, true, true, false, false]);
}

#[test]
fn settling_a_copy_runs_changes_is_one_decision() {
    let f = fixture();
    f.write(|r| r["staged"] = json!({"attemptId": "a1", "finishedAt": "2026-10-01T10:00:00.000Z", "folder": "/w", "changed": ["a"], "deleted": []}));
    assert_eq!(progress::settle_staged(&f.store, &f.id, "a0").unwrap_err(), "Those changes were already kept or discarded; reload Routines.");
    progress::settle_staged(&f.store, &f.id, "a1").unwrap();
    assert!(f.read().get("staged").is_none());
    assert!(progress::settle_staged(&f.store, &f.id, "a1").is_err());
}

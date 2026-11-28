//! a-routine-at-the-monthly-limit-says-so.test.ts (0.353): a routine step refused at the teammate's monthly limit never
//! started, and the card must say that -- not "Dispatch not confirmed ... Review external work before proceeding",
//! which sends a person to look for side effects of a step the host provably refused before recording anything.
//! Electron's harness keeps the routines in a map; here the runner runs on the native store itself
//! (`progress::StoreRoutines`), with the runner tests' host, whose start answers are queued per step.
use super::*;
use crate::routine_runner::tests::Fake;
use crate::routine_runner::{Begun, Phase, RoutineRunner, StartError};
use std::sync::Arc;

const LIMIT: &str = "Wren has reached this month's limit: $5.02 of $5.00. Raise the limit by editing Wren, or it starts again on October 1.";

#[derive(Clone, Copy)]
enum Says {
    Ok,
    Limit,
}

/// Step N's start answers `answers[N - 1]` (SPEND_LIMIT_REACHED for a limit); the routine is Nightly check, two steps.
fn harness(store: &Store, answers: &[Says]) -> (String, RoutineRunner, Arc<Fake>) {
    save(store, json!({"name": "Nightly check", "teammateId": "codec", "steps": ["Check the build.", "Say done."], "learnedFrom": []})).unwrap();
    let id = list(store).unwrap()["routines"][0]["routineId"].as_str().unwrap().to_owned();
    let host = Arc::new(Fake::default());
    for (n, answer) in answers.iter().copied().enumerate() {
        host.once.lock().unwrap().push_back(Box::new(move |_| match answer {
            Says::Limit => Err(StartError::Refused { code: "SPEND_LIMIT_REACHED".into(), message: LIMIT.into() }),
            Says::Ok => Ok(Begun { mission_id: format!("mission_{}", n + 1), run_id: format!("run_{}", n + 1) }),
        }));
    }
    (id, RoutineRunner::new(Arc::new(progress::StoreRoutines(store.clone())), host.clone()), host)
}

fn execution(store: &Store, id: &str) -> Value {
    list(store).unwrap()["routines"].as_array().unwrap().iter().find(|r| r["routineId"] == id).map_or(Value::Null, |r| r["execution"].clone())
}

// case: step 1: nothing is held and nothing is recorded; the refusal is the answer
#[test]
fn step_1_nothing_is_held_and_nothing_is_recorded_the_refusal_is_the_answer() {
    let (root, store) = fixture();
    let (id, runner, _) = harness(&store, &[Says::Limit]);
    match runner.run(&id, None) {
        Ok(Err(refused)) => assert!(refused.contains("reached this month's limit"), "{refused}"),
        other => panic!("not a refusal: {other:?}"),
    }
    assert!(execution(&store, &id).is_null(), "{}", execution(&store, &id));
    std::fs::remove_dir_all(root).unwrap();
}

// case: a later step: held as NOT STARTED, with the limit in its own words, and able to go on
#[test]
fn a_later_step_held_as_not_started_with_the_limit_in_its_own_words_and_able_to_go_on() {
    let (root, store) = fixture();
    let (id, runner, host) = harness(&store, &[Says::Ok, Says::Limit]);
    assert!(matches!(runner.run(&id, None), Ok(Ok(_))));
    host.phase("mission_1", Phase::Completed);
    runner.on_run_ended("mission_1").unwrap();
    let held = execution(&store, &id);
    assert_eq!(held["status"], "held", "{held}");
    assert_eq!(held["reason"], json!(format!("Step 2 was not started: {LIMIT}")));
    assert!(!held["reason"].as_str().unwrap_or("").contains("Dispatch not confirmed"));
    assert_eq!(held["canContinue"], true);
    // And a reconcile -- the Routines screen runs one -- keeps the reason.
    runner.reconcile().unwrap();
    let after = execution(&store, &id);
    assert_eq!(after["reason"], json!(format!("Step 2 was not started: {LIMIT}")));
    std::fs::remove_dir_all(root).unwrap();
}

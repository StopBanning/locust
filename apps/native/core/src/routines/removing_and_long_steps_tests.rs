//! Two Electron rows on the routines store:
//! - `a-routine-awaiting-review-is-not-removed.test.ts` (H2, second half): a routine whose last attempt is unsettled --
//!   held for the person's review, or still dispatching or running -- is not removed out from under it. Remove used to
//!   delete it and its uncertain-attempt record in one press, skipping the checkbox the review card puts before Abandon.
//! - `a-routine-step-too-long-is-refused-before-it-starts.test.ts` (A5.1), saving: the store took steps up to 20,000
//!   characters while a run's message carries 8,000.
use super::*;
use crate::hand_off::STEP_BUDGET;

fn one(store: &Store) -> Value {
    list(store).unwrap()["routines"][0].clone()
}

/// A routine saved, and its last attempt left at `status` (None: never run). "abandoned" is held, then abandoned.
fn with_attempt(status: Option<&str>) -> (std::path::PathBuf, Store, String) {
    let (root, store) = super::tests::fixture();
    save(&store, json!({"name": "Nightly report", "teammateId": "codec", "steps": ["Summarise the day."]})).unwrap();
    let routine = one(&store);
    let id = routine["routineId"].as_str().unwrap().to_owned();
    if let Some(status) = status {
        let at = routine["createdAt"].clone();
        let progress = json!({"attemptId": "attempt_1", "step": 1, "of": 1,
            "status": if status == "abandoned" { "held" } else { status }, "workspaceId": "ws_test",
            "startedAt": at, "updatedAt": at, "steps": routine["steps"], "route": routine["route"]});
        progress::save_progress(&store, &id, &progress, None).unwrap();
        if status == "abandoned" {
            progress::abandon(&store, &id, "attempt_1").unwrap();
        }
    }
    (root, store, id)
}

// case: is refused while its last attempt is ${status}, and the routine stays
#[test]
fn is_refused_while_its_last_attempt_is_unsettled_and_the_routine_stays() {
    for status in ["dispatching", "running", "held"] {
        let (root, store, id) = with_attempt(Some(status));
        assert_eq!(one(&store)["execution"]["status"], status);
        assert!(awaits_review(&one(&store)), "{status}");
        let refused = remove(&store, &id).unwrap_err();
        assert!(refused.contains("waiting for your review"), "{status}: {refused}");
        assert_eq!(one(&store)["routineId"], id.as_str(), "{status}: the routine stays");
        std::fs::remove_dir_all(root).unwrap();
    }
}

// case: goes once the attempt is abandoned, or when there never was one
#[test]
fn goes_once_the_attempt_is_abandoned_or_when_there_never_was_one() {
    let (root, store, id) = with_attempt(Some("abandoned"));
    assert_eq!(one(&store)["execution"]["status"], "abandoned");
    assert!(!awaits_review(&one(&store)));
    remove(&store, &id).unwrap();
    assert_eq!(list(&store).unwrap()["routines"], json!([]));
    std::fs::remove_dir_all(root).unwrap();

    let (root, store, id) = with_attempt(None);
    assert!(!awaits_review(&one(&store)));
    remove(&store, &id).unwrap();
    assert_eq!(list(&store).unwrap()["routines"], json!([]));
    std::fs::remove_dir_all(root).unwrap();
}

// case: refuses a step longer than a run can send, naming the step and the room
#[test]
fn refuses_a_step_longer_than_a_run_can_send_naming_the_step_and_the_room() {
    let (root, store) = super::tests::fixture();
    let long = "x".repeat(STEP_BUDGET + 1);
    let refused = save(&store, json!({"name": "Nightly tidy", "teammateId": "codec", "steps": ["Fine.", long]})).unwrap_err();
    assert!(refused.starts_with("Step 2 is 8,001 characters; a step can be at most 8,000."), "{refused}");
    assert_eq!(list(&store).unwrap()["routines"], json!([]), "nothing saved");
    save(&store, json!({"name": "Nightly tidy", "teammateId": "codec", "steps": ["Fine."]})).unwrap();
    let id = one(&store)["routineId"].clone();
    let refused = save(&store, json!({"routineId": id, "name": "Nightly tidy", "steps": [long]})).unwrap_err();
    assert!(refused.starts_with("Step 1 is"), "{refused}");
    assert_eq!(one(&store)["steps"], json!(["Fine."]), "the edit is not kept");
    // At the room exactly, it saves.
    save(&store, json!({"routineId": id, "name": "Nightly tidy", "steps": ["x".repeat(STEP_BUDGET)]})).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

// case: still reads a routine saved before, with a step up to 20,000
#[test]
fn still_reads_a_routine_saved_before_with_a_step_up_to_20000() {
    let (root, store) = super::tests::fixture();
    let old = json!({"routineId": "rt_old", "name": "Old", "teammateId": "codec",
        "route": {"runtime": "cursor", "model": "composer-2.5", "mode": "ask"}, "steps": ["x".repeat(12_000)],
        "learnedFrom": [], "createdAt": "2026-09-05T00:00:00.000Z", "runs": 0});
    store.save_json("routines.json", &json!({"schemaVersion": 1, "routines": [old]})).unwrap();
    let ids: Vec<_> = list(&store).unwrap()["routines"].as_array().unwrap().iter().map(|r| r["routineId"].clone()).collect();
    assert_eq!(ids, vec![json!("rt_old")]);
    std::fs::remove_dir_all(root).unwrap();
}

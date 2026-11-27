//! a-routine-due-while-closed-is-missed.test.ts: no catch-up. A routine whose time passed while Locust was closed shows
//! as missed and does not start at launch, and the miss is recorded in the routine's history; a time that arrives
//! while Locust is open still starts. Electron's harness holds the routines in a map whose `recordMiss` writes what the
//! store's does; here the runner runs on the native store itself (`progress::StoreRoutines`), with the runner tests'
//! host (`routine_runner::tests::Fake`, `openedAt` its `opened_at`).
use super::*;
use crate::routine_runner::RoutineRunner;
use crate::routine_runner::tests::Fake;
use chrono::{DateTime, Local, SecondsFormat, TimeZone, Utc};
use std::sync::{Arc, Mutex};

/// Local time in October 2026 (Electron's `new Date(2026, 9, day, hour, minute, second)`).
fn local(day: u32, hour: u32, minute: u32, second: u32) -> DateTime<Utc> {
    Local.with_ymd_and_hms(2026, 10, day, hour, minute, second).earliest().unwrap().with_timezone(&Utc)
}

fn iso(at: DateTime<Utc>) -> String {
    at.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Electron's `saved(over)`: Morning digest, daily at 08:00, created and last run on 2 October at 08:00.
fn saved(store: &Store, schedule: Value, last_run_at: DateTime<Utc>) -> String {
    save(store, json!({"name": "Morning digest", "teammateId": "codec", "steps": ["Say good morning."], "learnedFrom": [],
        "schedule": schedule}))
    .unwrap();
    let id = list(store).unwrap()["routines"][0]["routineId"].as_str().unwrap().to_owned();
    mutate(store, &id, |r| {
        r["createdAt"] = json!(iso(local(2, 8, 0, 0)));
        r["lastRunAt"] = json!(iso(last_run_at));
        Ok(())
    })
    .unwrap();
    id
}

/// The runner as Locust opened at `opened`, in the folder the routine was made in (the store stamps its teammate's).
fn harness(store: &Store, opened: DateTime<Utc>) -> (RoutineRunner, Arc<Fake>) {
    let made_in = held(store)["workspaceId"].as_str().unwrap_or_default().to_owned();
    let host = Arc::new(Fake { opened_at: Some(opened), asked: Mutex::new(None), workspace: Mutex::new(made_in), ..Fake::default() });
    (RoutineRunner::new(Arc::new(progress::StoreRoutines(store.clone())), host.clone()), host)
}

fn held(store: &Store) -> Value {
    list(store).unwrap()["routines"][0].clone()
}

// case: is missed and not started, and one that comes due while Locust is open still starts
#[test]
fn is_missed_and_not_started_and_one_that_comes_due_while_locust_is_open_still_starts() {
    let (root, store) = fixture();
    saved(&store, json!({"kind": "daily", "at": "08:00"}), local(2, 8, 0, 0));
    let (runner, closed) = harness(&store, local(3, 9, 0, 0));
    assert_eq!(runner.tick(local(3, 9, 0, 15)).unwrap(), Vec::<String>::new());
    assert!(closed.starts().is_empty());
    let missed = held(&store);
    assert_eq!(missed["runs"], 0);
    assert_eq!(missed["missedAt"], json!(iso(local(3, 8, 0, 0))));
    assert_eq!(missed["history"], json!([{"kind": "missed", "dueAt": iso(local(3, 8, 0, 0)), "recordedAt": iso(local(3, 9, 0, 15))}]));
    // The next tick does not start it either: the miss moved the clock.
    assert_eq!(runner.tick(local(3, 9, 1, 0)).unwrap(), Vec::<String>::new());
    assert!(closed.starts().is_empty());
    std::fs::remove_dir_all(root).unwrap();

    let (root, store) = fixture();
    let id = saved(&store, json!({"kind": "daily", "at": "08:00"}), local(2, 8, 0, 0));
    let (runner, open) = harness(&store, local(3, 7, 0, 0));
    assert_eq!(runner.tick(local(3, 8, 0, 15)).unwrap(), vec![id]);
    assert_eq!(open.starts().len(), 1);
    std::fs::remove_dir_all(root).unwrap();
}

// case: records one miss for several missed hours, and does not start on the next tick
#[test]
fn records_one_miss_for_several_missed_hours_and_does_not_start_on_the_next_tick() {
    let (root, store) = fixture();
    let opened = local(3, 15, 0, 0);
    saved(&store, json!({"kind": "every", "hours": 1}), local(3, 10, 0, 0));
    let (runner, host) = harness(&store, opened);
    assert_eq!(runner.tick(opened).unwrap(), Vec::<String>::new());
    assert!(host.starts().is_empty());
    let missed = held(&store);
    assert_eq!(missed["history"].as_array().map(Vec::len), Some(1));
    assert_eq!(missed["missedAt"], json!(iso(local(3, 15, 0, 0))));
    assert_eq!(runner.tick(local(3, 15, 1, 0)).unwrap(), Vec::<String>::new());
    assert!(host.starts().is_empty());
    assert_eq!(held(&store)["history"].as_array().map(Vec::len), Some(1));
    std::fs::remove_dir_all(root).unwrap();
}

// case: keeps the miss in the routine file, and a later read still has it
#[test]
fn keeps_the_miss_in_the_routine_file_and_a_later_read_still_has_it() {
    let (root, store) = fixture();
    save(&store, json!({"name": "Morning digest", "teammateId": "codec", "steps": ["Say good morning."], "learnedFrom": [],
        "schedule": {"kind": "daily", "at": "08:00"}}))
    .unwrap();
    let id = held(&store)["routineId"].as_str().unwrap().to_owned();
    let due = iso(local(3, 8, 0, 0));
    let recorded = iso(local(3, 9, 0, 0));
    progress::record_miss(&store, &id, &due, &recorded).unwrap();
    // A store opened afresh on the same folders: what is on disk, not what this one holds.
    let again = Store::new(root.join("native"), root.join("electron")).unwrap();
    let read = list(&again).unwrap()["routines"].as_array().unwrap().iter().find(|r| r["routineId"] == id.as_str()).cloned().unwrap();
    assert_eq!(read["runs"], 0);
    assert_eq!(read["missedAt"], json!(due));
    assert_eq!(read["history"], json!([{"kind": "missed", "dueAt": due, "recordedAt": recorded}]));
    std::fs::remove_dir_all(root).unwrap();
}

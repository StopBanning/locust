//! a-routine-time-passed-while-it-ran-is-missed.test.ts (0.690): a clock time that came while the routine's own copy
//! was still running is recorded as missed when that run ends, the way a time that passed while Locust was closed is.
//! `slotPassedDuring` is `schedule::slot_passed_during`; the store's `recordRun` is `progress::record_run`.
//!
//! Electron fakes `Date` for the two store cases (07:50 start, 08:20 end). The native store reads the real clock, so
//! these put the routine's daily time ten minutes behind now and its run's start before or exactly on it: the same
//! three moments, shifted to today.
use super::*;
use chrono::{DateTime, Duration, Local, SecondsFormat, TimeZone, Timelike, Utc};

/// `at(hour, minute, day = 7)` in October 2026 (2026-10-07 is a Wednesday), local time.
fn at(hour: u32, minute: u32, day: u32) -> DateTime<Local> {
    Local.with_ymd_and_hms(2026, 10, day, hour, minute, 0).earliest().unwrap()
}

fn iso(at: DateTime<Local>) -> String {
    at.with_timezone(&Utc).to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn instant(s: &Value) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s.as_str().unwrap_or_else(|| panic!("not a time: {s}"))).unwrap().with_timezone(&Utc)
}

// case: is the daily time between the start and the end, and nothing when the run began at or after it
#[test]
fn is_the_daily_time_between_the_start_and_the_end_and_nothing_when_the_run_began_at_or_after_it() {
    let daily = json!({"kind": "daily", "at": "08:00"});
    assert_eq!(schedule::slot_passed_during(&daily, at(7, 50, 7), at(8, 20, 7)), Some(at(8, 0, 7)));
    // The run that 08:00 itself started.
    assert_eq!(schedule::slot_passed_during(&daily, at(8, 0, 7), at(8, 40, 7)), None);
    // Ended before the time came.
    assert_eq!(schedule::slot_passed_during(&daily, at(7, 10, 7), at(7, 40, 7)), None);
    // Across midnight: a 23:30 that came while a 23:00 run went past 00:00 (2026-10-10 sweep).
    let late = json!({"kind": "daily", "at": "23:30"});
    assert_eq!(schedule::slot_passed_during(&late, at(23, 0, 7), at(0, 30, 8)), Some(at(23, 30, 7)));
}

// case: is a weekly time on its day, and never an interval or a file
#[test]
fn is_a_weekly_time_on_its_day_and_never_an_interval_or_a_file() {
    let weekly = |day: u32| json!({"kind": "weekly", "days": [day], "at": "09:00"});
    assert_eq!(schedule::slot_passed_during(&weekly(3), at(8, 45, 7), at(9, 30, 7)), Some(at(9, 0, 7)));
    assert_eq!(schedule::slot_passed_during(&weekly(4), at(8, 45, 7), at(9, 30, 7)), None);
    // Every N hours counts from the last run's end: a long run delays the next one, it drops nothing.
    assert_eq!(schedule::slot_passed_during(&json!({"kind": "every", "hours": 1}), at(8, 0, 7), at(10, 30, 7)), None);
    assert_eq!(schedule::slot_passed_during(&json!({"kind": "files", "folder": "C:/inbox"}), at(8, 0, 7), at(10, 30, 7)), None);
}

/// A daily routine at `slot` (HH:MM) with a run of one step going since `started`, on the real store.
fn running_since(store: &Store, slot: DateTime<Local>, started: DateTime<Local>) -> String {
    save(store, json!({"name": "Morning digest", "teammateId": "codec", "steps": ["Say good morning."], "learnedFrom": [],
        "schedule": {"kind": "daily", "at": slot.format("%H:%M").to_string()}}))
    .unwrap();
    let routine = list(store).unwrap()["routines"][0].clone();
    let id = routine["routineId"].as_str().unwrap().to_owned();
    progress::save_progress(store, &id, &json!({"attemptId": "attempt_1", "step": 1, "of": 1, "status": "running",
        "missionId": "mission_1", "runId": "run_1", "workspaceId": "ws_test", "startedAt": iso(started), "updatedAt": iso(started),
        "steps": routine["steps"], "route": routine["route"]}), None)
    .unwrap();
    id
}

/// Ten minutes ago, to the minute: a daily time that has just come.
fn just_came() -> DateTime<Local> {
    (Local::now() - Duration::minutes(10)).with_second(0).and_then(|t| t.with_nanosecond(0)).unwrap()
}

// case: says Missed for the time that passed, counts the run, and keeps it in the history
#[test]
fn says_missed_for_the_time_that_passed_counts_the_run_and_keeps_it_in_the_history() {
    let (root, store) = fixture();
    let slot = just_came();
    let id = running_since(&store, slot, slot - Duration::minutes(20));
    let before = Utc::now() - Duration::milliseconds(1);
    progress::record_run(&store, &id, "attempt_1", None, None).unwrap();
    let after = Utc::now() + Duration::milliseconds(1);
    let ended = list(&store).unwrap()["routines"][0].clone();
    assert_eq!(ended["runs"], 1);
    let last = instant(&ended["lastRunAt"]);
    assert!(last >= before && last <= after, "lastRunAt is the end: {last}");
    assert_eq!(ended["missedAt"], json!(iso(slot)));
    let history = ended["history"].as_array().cloned().unwrap_or_default();
    assert_eq!(history.len(), 1, "{history:?}");
    assert_eq!(history[0]["kind"], "missed");
    assert_eq!(history[0]["dueAt"], json!(iso(slot)));
    let recorded = instant(&history[0]["recordedAt"]);
    assert!(recorded >= before && recorded <= after, "recorded at the end: {recorded}");
    assert_eq!(history[0].as_object().unwrap().len(), 3, "kind, dueAt and recordedAt only: {:?}", history[0]);
    std::fs::remove_dir_all(root).unwrap();
}

// case: records nothing missed for a run that ended before its next time
#[test]
fn records_nothing_missed_for_a_run_that_ended_before_its_next_time() {
    let (root, store) = fixture();
    let slot = just_came();
    // The run that time itself started, ending now: its next time is tomorrow.
    let id = running_since(&store, slot, slot);
    progress::record_run(&store, &id, "attempt_1", None, None).unwrap();
    let ended = list(&store).unwrap()["routines"][0].clone();
    assert_eq!(ended["runs"], 1);
    assert!(ended.get("missedAt").is_none(), "{ended}");
    assert_eq!(ended["history"].as_array().cloned().unwrap_or_default(), Vec::<Value>::new());
    std::fs::remove_dir_all(root).unwrap();
}

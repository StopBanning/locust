//! Parity tests for `apps/desktop/src/shared/routine-schedule.test.ts`.
use super::schedule::{is_due, label, next, valid};
use chrono::{DateTime, Datelike, Local, TimeZone, Timelike};
use serde_json::{Value, json};

fn at(y: i32, m: u32, d: u32, h: u32, min: u32, s: u32) -> DateTime<Local> {
    Local
        .with_ymd_and_hms(y, m, d, h, min, s)
        .earliest()
        .unwrap()
}

fn utc(iso: &str) -> DateTime<Local> {
    DateTime::parse_from_rfc3339(iso)
        .unwrap()
        .with_timezone(&Local)
}

#[test]
// case: is every N whole hours within a week, or daily at a wall-clock time
fn is_every_n_whole_hours_within_a_week_or_daily_at_a_wall_clock_time() {
    assert!(valid(&json!({"kind": "every", "hours": 4})));
    assert!(valid(&json!({"kind": "every", "hours": 168})));
    assert!(!valid(&json!({"kind": "every", "hours": 0})));
    assert!(!valid(&json!({"kind": "every", "hours": 1.5})));
    assert!(!valid(&json!({"kind": "every", "hours": 169})));
    assert!(valid(&json!({"kind": "daily", "at": "09:30"})));
    assert!(!valid(&json!({"kind": "daily", "at": "24:00"})));
    assert!(!valid(&json!({"kind": "daily", "at": "9:30"})));
    assert!(!valid(&json!({"kind": "weekly"})));
    assert!(valid(&json!({"kind": "weekly", "days": [1, 3, 5], "at": "09:00"})));
    assert!(!valid(&json!({"kind": "weekly", "days": [], "at": "09:00"})));
    assert!(!valid(&json!({"kind": "weekly", "days": [3, 1], "at": "09:00"})));
    assert!(!valid(&json!({"kind": "weekly", "days": [1, 1], "at": "09:00"})));
    assert!(!valid(&json!({"kind": "weekly", "days": [7], "at": "09:00"})));
    assert!(valid(&json!({"kind": "once", "on": "2026-10-07T15:00"})));
    assert!(!valid(&json!({"kind": "once", "on": "2026-02-30T15:00"})));
    assert!(!valid(&json!({"kind": "once", "on": "2026-10-07 15:00"})));
    assert!(!valid(&Value::Null));
}

#[test]
// case: every N hours counts from the last run
fn every_n_hours_counts_from_the_last_run() {
    let last = utc("2026-09-05T10:00:00.000Z");
    let now = utc("2026-09-05T12:00:00.000Z");
    assert_eq!(
        next(&json!({"kind": "every", "hours": 4}), last, now),
        Some(utc("2026-09-05T14:00:00.000Z"))
    );
    assert!(!is_due(&json!({"kind": "every", "hours": 4}), last, now));
    assert!(is_due(&json!({"kind": "every", "hours": 2}), last, now));
}

#[test]
// case: a routine that missed several intervals while the app was closed is due once, now -- not once per miss
fn a_routine_that_missed_several_intervals_while_the_app_was_closed_is_due_once_now_not_once_per_miss() {
    let last = utc("2026-09-01T10:00:00.000Z");
    let now = utc("2026-09-05T12:00:00.000Z");
    let sched = json!({"kind": "every", "hours": 4});
    assert_eq!(next(&sched, last, now), Some(now));
    assert!(is_due(&sched, last, now));
}

#[test]
// case: daily at a time is today if still ahead, tomorrow once it has run today
fn daily_at_a_time_is_today_if_still_ahead_tomorrow_once_it_has_run_today() {
    let sched = json!({"kind": "daily", "at": "09:00"});
    let now = at(2026, 9, 5, 8, 0, 0);
    let last = at(2026, 9, 4, 9, 0, 0);
    let nxt = next(&sched, last, now).unwrap();
    assert_eq!((nxt.year(), nxt.month(), nxt.day(), nxt.hour(), nxt.minute()), (2026, 9, 5, 9, 0));
    assert!(!is_due(&sched, last, now));

    let later = at(2026, 9, 5, 9, 30, 0);
    assert!(is_due(&sched, last, later));

    let ran_today = at(2026, 9, 5, 9, 1, 0);
    let after_run = next(&sched, ran_today, later).unwrap();
    assert_eq!((after_run.day(), after_run.hour()), (6, 9));
    assert!(!is_due(&sched, ran_today, later));
}

#[test]
// case: on set days: the next chosen day at the time, never a day that was not chosen (0.517)
fn on_set_days_the_next_chosen_day_at_the_time_never_a_day_that_was_not_chosen() {
    let weekdays = json!({"kind": "weekly", "days": [1, 2, 3, 4, 5], "at": "09:00"});
    // Friday 2026-10-02 10:00, last ran Friday 09:00:30 -> next is Monday Oct 5, not Saturday.
    let now = at(2026, 10, 2, 10, 0, 0);
    let ran_friday = at(2026, 10, 2, 9, 0, 30);
    let nxt = next(&weekdays, ran_friday, now).unwrap();
    assert_eq!((nxt.weekday().num_days_from_sunday(), nxt.day(), nxt.hour()), (1, 5, 9));
    assert!(!is_due(&weekdays, ran_friday, now));
    // Saturday and Sunday pass without a run.
    assert!(!is_due(&weekdays, ran_friday, at(2026, 10, 3, 12, 0, 0)));
    assert!(!is_due(&weekdays, ran_friday, at(2026, 10, 4, 12, 0, 0)));
    assert!(is_due(&weekdays, ran_friday, at(2026, 10, 5, 9, 0, 0)));
}

#[test]
// case: on set days: a day missed while Locust was closed runs once, the next time it is open
fn on_set_days_a_day_missed_while_locust_was_closed_runs_once_the_next_time_it_is_open() {
    let mondays = json!({"kind": "weekly", "days": [1], "at": "09:00"});
    let ran_last_monday = at(2026, 9, 28, 9, 0, 30);
    // Opened Wednesday 2026-10-07: Monday the 5th was missed.
    let wednesday = at(2026, 10, 7, 14, 0, 0);
    assert!(is_due(&mondays, ran_last_monday, wednesday));
    // Once it has run, the next is the Monday after, not again now.
    assert!(!is_due(&mondays, wednesday, at(2026, 10, 7, 14, 5, 0)));
    assert_eq!(next(&mondays, wednesday, wednesday).unwrap().day(), 12);
}

#[test]
// case: on set days: made after this week's slot, it waits for the next one
fn on_set_days_made_after_this_weeks_slot_it_waits_for_the_next_one() {
    let mondays = json!({"kind": "weekly", "days": [1], "at": "09:00"});
    let made_tuesday = at(2026, 10, 6, 10, 0, 0);
    let check_at = at(2026, 10, 6, 10, 1, 0);
    assert!(!is_due(&mondays, made_tuesday, check_at));
    assert_eq!(next(&mondays, made_tuesday, check_at).unwrap().day(), 12);
}

#[test]
// case: once: due at its moment, then never again
fn once_due_at_its_moment_then_never_again() {
    let once = json!({"kind": "once", "on": "2026-10-07T15:00"});
    let made = at(2026, 10, 1, 12, 0, 0);
    assert!(!is_due(&once, made, at(2026, 10, 7, 14, 59, 0)));
    assert!(is_due(&once, made, at(2026, 10, 7, 15, 0, 0)));
    // Missed while closed: runs the next time Locust is open.
    assert!(is_due(&once, made, at(2026, 10, 9, 8, 0, 0)));
    let ran = at(2026, 10, 7, 15, 0, 10);
    assert!(next(&once, ran, at(2026, 10, 7, 15, 1, 0)).is_none());
    assert!(!is_due(&once, ran, at(2026, 10, 20, 15, 0, 0)));
}

#[test]
// case: reads on a card
fn reads_on_a_card() {
    // 0.882: Locust formats card times in 12-hour clock (no 24-hour times on screen).
    assert_eq!(label(&json!({"kind": "weekly", "days": [1, 2, 3, 4, 5], "at": "09:00"})), "weekdays at 9:00 AM");
    assert_eq!(label(&json!({"kind": "weekly", "days": [0, 6], "at": "10:30"})), "weekends at 10:30 AM");
    assert_eq!(label(&json!({"kind": "weekly", "days": [0, 1, 3], "at": "08:00"})), "Mon, Wed, Sun at 8:00 AM");
    assert_eq!(label(&json!({"kind": "weekly", "days": [0, 1, 2, 3, 4, 5, 6], "at": "08:00"})), "every day at 8:00 AM");
    assert_eq!(label(&json!({"kind": "once", "on": "2026-10-07T15:00"})), "once, Wed Oct 7 at 3:00 PM");
    assert_eq!(label(&Value::Null), "when you press Run");
    assert_eq!(label(&json!({"kind": "every", "hours": 1})), "every hour");
    assert_eq!(label(&json!({"kind": "every", "hours": 8})), "every 8 hours");
    assert_eq!(label(&json!({"kind": "daily", "at": "07:45"})), "daily at 7:45 AM");
}

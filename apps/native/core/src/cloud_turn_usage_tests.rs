use super::*;
use chrono::{DateTime, TimeZone, Utc};
use std::cell::Cell;

fn at(minute: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 13, 15, 0, 0).unwrap() + chrono::Duration::minutes(minute)
}

fn iso(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn spans(times: &[(i64, i64)]) -> Vec<(String, String)> {
    times.iter().map(|&(from, to)| (iso(at(from)), iso(at(to)))).collect()
}

#[test]
// case: counts each stretch with the idle minutes after it, and an overlap once
fn counts_each_cloud_stretch_with_idle_minutes_and_overlap_once() {
    let overlapping = spans(&[(0, 20), (25, 30)]);
    assert!((super::super::hours(&overlapping, at(240)) - 40.0 / 60.0).abs() < 0.00001);
    let separate = spans(&[(0, 20), (60, 65)]);
    assert!((super::super::hours(&separate, at(240)) - 45.0 / 60.0).abs() < 0.00001);
}

#[test]
// case: counts a computer still on only up to now, and takes the stretches in any order
fn counts_a_cloud_computer_up_to_now_and_accepts_unsorted_stretches() {
    assert!((super::super::hours(&spans(&[(0, 20)]), at(24)) - 24.0 / 60.0).abs() < 0.00001);
    assert!((super::super::hours(&spans(&[(60, 65), (0, 20)]), at(240)) - 45.0 / 60.0).abs() < 0.00001);
    assert_eq!(super::super::hours(&[], at(240)), 0.0);
    assert_eq!(super::super::hours(&[("not a time".into(), iso(at(5)))], at(240)), 0.0);
    assert_eq!(super::super::CLOUD_IDLE_MS, 600_000);
}

#[test]
// case: reads as a person says it, and keeps a month as GitHub does
fn cloud_hours_read_as_words_and_keep_a_utc_month() {
    assert_eq!(super::super::hours_words(0.05), "a few minutes");
    assert_eq!(super::super::hours_words(0.4), "24 minutes");
    assert_eq!(super::super::hours_words(2.46), "2.5 hours");
    assert_eq!(super::super::hours_words(31.6), "32 hours");
    assert_eq!(super::super::cloud_month(at(0)), "2026-10");
}

#[test]
// case: counts a turn from its readying to its end, and every step on the computer, for Settings' total
fn counts_cloud_turn_readying_through_final_change_and_retains_first_count() {
    let folder = TempDir::new("locust-cloud-meter");
    let file = folder.path.join("cloud-turns.json");
    let fake = FakeComputers::new(&FakeOptions::with_existing("Available"));
    let clock = Cell::new(0);
    let logged_ready = Cell::new(false);
    let logged_counts = Cell::new(false);
    let git = |args: &[&str], _: &Path| {
        Ok(if args[0] == "remote" {
            Ran::new(0, "https://github.com/acme/hearth.git\n", "")
        } else {
            Ran::new(1, "", "")
        })
    };
    let turns = CloudTurns::new(&file, &fake, &git, &git).with_now(|| {
        let calls = fake.calls.borrow();
        if calls.iter().any(|c| c == "ready") && !logged_ready.replace(true) {
            clock.set(clock.get() + 1);
        }
        if calls.iter().any(|c| c == "change:a") && !logged_counts.replace(true) {
            clock.set(clock.get() + 1);
        }
        iso(at(clock.get()))
    });

    assert!(super::super::recorded_usage(&file, at(0)).is_none());
    assert!(matches!(turns.prepare(PrepareInput {
        mission_id: "mission_a",
        folder: &folder.path,
        follows: None,
        open_code_version: Some("1.18.27"),
        agent: None,
        set_up: false,
    }, &mut |_| {}), CloudPrepared::Ok { .. }));
    let during_start = super::super::recorded_usage(&file, at(5)).unwrap();
    assert!((during_start.hours - 5.0 / 60.0).abs() < 0.00001, "{during_start:?}");
    clock.set(clock.get() + 20);
    turns.settled("mission_a");
    clock.set(clock.get() + 60);

    // Read the disk file again, not a counter on the controller we just used.
    let report = super::super::recorded_usage(&file, at(clock.get())).unwrap();
    assert!((report.hours - 32.0 / 60.0).abs() < 0.00001, "{report:?}");
    assert!((report.by_computer["locust-hearth-x7q9"] - 32.0 / 60.0).abs() < 0.00001);
    assert_eq!(report.since.as_deref(), Some("2026-10-13T15:00:00.000Z"));

    // A later read must not extend a finished turn again; a documented clock only grows the idle tail.
    turns.settled("mission_a");
    let later = super::super::recorded_usage(&file, at(200)).unwrap();
    assert_eq!(later.hours, report.hours);
}

#[test]
fn separates_cloud_computers_and_keeps_electron_usage_while_writing_a_turn() {
    let folder = TempDir::new("locust-cloud-imported-meter");
    let file = folder.path.join("cloud-turns.json");
    fs::write(&file, serde_json::json!({
        "tasks": {},
        "turns": {},
        "useSince": "2026-10-13T15:00:00.000Z",
        "use": { "2026-10": {
            "first": [["2026-10-13T15:00:00.000Z", "2026-10-13T15:20:00.000Z"]],
            "second": [["2026-10-13T15:00:00.000Z", "2026-10-13T15:20:00.000Z"]]
        }}
    }).to_string()).unwrap();
    let fake = FakeComputers::new(&FakeOptions::with_existing("Available"));
    let git = |args: &[&str], _: &Path| Ok(if args[0] == "remote" {
        Ran::new(0, "https://github.com/acme/hearth.git\n", "")
    } else {
        Ran::new(1, "", "")
    });
    let turns = CloudTurns::new(&file, &fake, &git, &git).with_now(|| iso(at(240)));
    assert!(matches!(turns.prepare(PrepareInput {
        mission_id: "mission_a",
        folder: &folder.path,
        follows: None,
        open_code_version: Some("1.18.27"),
        agent: None,
        set_up: false,
    }, &mut |_| {}), CloudPrepared::Ok { .. }));
    let measured = super::super::recorded_usage(&file, at(240)).unwrap();
    assert_eq!(measured.hours, 1.0);
    assert_eq!(measured.by_computer["first"], 0.5);
    assert_eq!(measured.by_computer["second"], 0.5);
    assert_eq!(measured.since.as_deref(), Some("2026-10-13T15:00:00.000Z"));
}

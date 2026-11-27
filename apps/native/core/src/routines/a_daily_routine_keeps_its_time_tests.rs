//! a-daily-routine-keeps-its-time.test.ts: a flat 24 hours drifted an hour across a clock change (QA-2026-09-29
//! round 2, R7). `nextRunAfter` is `schedule::next` here.
//!
//! Like Electron's, these cases mean something only where this process follows America/New_York: the schedule reads the
//! local clock, and a test cannot move the zone of a test binary whose other tests read it at the same time. Electron
//! returns early elsewhere; so do these. Run them with `TZ=America/New_York` for the check to bite.
use super::*;
use chrono::{DateTime, Local, TimeZone, Utc};

fn in_new_york() -> bool {
    // Only meaningful where this process follows the zone (`getTimezoneOffset() !== 240` returns early).
    let noon = Utc.with_ymd_and_hms(2026, 3, 8, 12, 0, 0).unwrap();
    Local.from_utc_datetime(&noon.naive_utc()).offset().local_minus_utc() == -4 * 3600
}

fn moment(s: &str) -> DateTime<Local> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Local)
}

fn still_runs_at_nine(last_run_at: &str, now: &str, next: &str) {
    if !in_new_york() {
        return;
    }
    let daily = json!({"kind": "daily", "at": "09:00"});
    let got = schedule::next(&daily, moment(last_run_at), moment(now)).map(|d| d.with_timezone(&Utc));
    assert_eq!(got, Some(moment(next).with_timezone(&Utc)));
}

// case: still runs at 09:00 the next day (spring forward, New York)
#[test]
fn still_runs_at_0900_the_next_day_spring_forward_new_york() {
    still_runs_at_nine("2026-03-07T09:00:00-05:00", "2026-03-07T12:00:00-05:00", "2026-03-08T09:00:00-04:00");
}

// case: still runs at 09:00 the next day (fall back, New York)
#[test]
fn still_runs_at_0900_the_next_day_fall_back_new_york() {
    still_runs_at_nine("2026-10-31T09:00:00-04:00", "2026-10-31T12:00:00-04:00", "2026-11-01T09:00:00-05:00");
}

//! Electron's `shared/a-time-of-day-is-twelve-hour.test.ts` (0.882).

use crate::time_marker::clock;
use chrono::NaiveDate;

fn at(h: u32, m: u32, s: u32) -> chrono::NaiveDateTime {
    NaiveDate::from_ymd_opt(2026, 11, 22).unwrap().and_hms_opt(h, m, s).unwrap()
}

#[test]
fn is_twelve_hour_with_no_leading_zero_and_midnight_and_noon_are_twelve() {
    // case: Colin, 22 Nov, on a usage card reading "resets 17:34": "I think we should just stop using military time,
    // honestly."
    assert_eq!(clock(&at(17, 34, 0), false), "5:34 PM");
    assert_eq!(clock(&at(9, 5, 0), false), "9:05 AM");
    assert_eq!(clock(&at(0, 0, 0), false), "12:00 AM");
    assert_eq!(clock(&at(12, 0, 0), false), "12:00 PM");
    assert_eq!(clock(&at(23, 59, 7), true), "11:59:07 PM");
}

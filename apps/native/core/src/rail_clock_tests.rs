//! Electron's `the-rail-clock-is-made-once.test.ts` (0.882): the rail clock is one kept formatter, and says the time
//! 12-hour with its seconds, whatever the computer's own clock setting.

use crate::time_marker::{clock, clock_of};
use chrono::{DateTime, Local};

#[test]
fn says_the_time_twelve_hour_with_its_seconds_whatever_the_computers_own_clock_setting() {
    // case: Colin, 22 Nov: "I think we should just stop using military time, honestly."
    for iso in ["2026-09-22T00:00:00.000Z", "2026-09-22T12:34:56.000Z", "2026-12-31T23:59:59.999Z"] {
        let at = DateTime::parse_from_rfc3339(iso).unwrap().with_timezone(&Local);
        assert_eq!(clock_of(iso), clock(&at, true));
        let said = clock_of(iso);
        let (time, half) = said.split_once(' ').unwrap();
        assert!(half == "AM" || half == "PM", "{said}");
        let parts: Vec<&str> = time.split(':').collect();
        assert_eq!(parts.len(), 3, "{said}");
        assert!((1..=2).contains(&parts[0].len()) && parts[1].len() == 2 && parts[2].len() == 2, "{said}");
    }
}

#[test]
fn says_nothing_for_a_time_that_is_not_one() {
    // case: says nothing for a time that is not one
    assert_eq!(clock_of("not a time"), "");
}

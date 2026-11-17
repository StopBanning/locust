//! Electron's `the-rail-clock-is-made-once.test.ts`: the rail clock says what `toLocaleTimeString`
//! with two-digit hours, minutes and seconds says, which on a Mac follows its 12/24-hour setting.

use crate::time_marker::{HourCycle, clock_of, clock_of_in, hour_cycle_of_pattern, system_hour_cycle};
use chrono::{DateTime, Local, Timelike};

const TIMES: [&str; 3] = ["2026-09-22T00:00:00.000Z", "2026-09-22T12:34:56.000Z", "2026-12-31T23:59:59.999Z"];

fn local(iso: &str) -> DateTime<Local> {
    DateTime::parse_from_rfc3339(iso).unwrap().with_timezone(&Local)
}

#[test]
fn says_a_twelve_hour_time_the_way_electron_wrote_it_on_a_twelve_hour_mac() {
    for iso in TIMES {
        let at = local(iso);
        let (pm, hour) = at.hour12();
        let expected = format!("{hour:02}:{:02}:{:02} {}", at.minute(), at.second(), if pm { "PM" } else { "AM" });
        assert_eq!(clock_of_in(iso, HourCycle::H12), expected);
    }
}

#[test]
fn says_midnight_and_noon_as_twelve_never_zero_on_a_twelve_hour_clock() {
    let midnight = Local::now().with_hour(0).unwrap().with_minute(5).unwrap().with_second(9).unwrap();
    let noon = midnight.with_hour(12).unwrap();
    assert_eq!(clock_of_in(&midnight.to_rfc3339(), HourCycle::H12), "12:05:09 AM");
    assert_eq!(clock_of_in(&noon.to_rfc3339(), HourCycle::H12), "12:05:09 PM");
    assert_eq!(clock_of_in(&midnight.to_rfc3339(), HourCycle::H24), "00:05:09");
}

#[test]
fn says_a_twenty_four_hour_time_with_two_digits_each() {
    let late = Local::now().with_hour(21).unwrap().with_minute(4).unwrap().with_second(3).unwrap();
    assert_eq!(clock_of_in(&late.to_rfc3339(), HourCycle::H24), "21:04:03");
}

#[test]
fn reads_the_macs_hour_pattern_for_which_clock_a_person_keeps() {
    assert_eq!(hour_cycle_of_pattern("h a"), HourCycle::H12);
    assert_eq!(hour_cycle_of_pattern("K a"), HourCycle::H12);
    assert_eq!(hour_cycle_of_pattern("HH"), HourCycle::H24);
    assert_eq!(hour_cycle_of_pattern("H"), HourCycle::H24);
    assert_eq!(hour_cycle_of_pattern("'h'HH"), HourCycle::H24, "a quoted letter is text, not an hour");
}

#[test]
fn the_rail_uses_the_systems_own_clock_and_asks_once() {
    let cycle = system_hour_cycle();
    assert_eq!(system_hour_cycle(), cycle);
    for iso in TIMES {
        assert_eq!(clock_of(iso), clock_of_in(iso, cycle));
    }
}

#[test]
fn says_nothing_for_a_time_that_is_not_one() {
    assert_eq!(clock_of("not a time"), "");
    assert_eq!(clock_of_in("not a time", HourCycle::H12), "");
}

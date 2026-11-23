//! AwayList's `when` (0.882, 24c94c0e): this computer's day and clock, 12-hour; an older day carries its time too.
use crate::away::when_label;
use chrono::{Local, TimeZone};

fn ms(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> i64 {
    Local.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap().timestamp_millis()
}
fn iso(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> String {
    Local.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap().to_rfc3339()
}

#[test]
fn today_is_the_bare_twelve_hour_time() {
    assert_eq!(when_label(&iso(2026, 10, 7, 17, 34), ms(2026, 10, 7, 20, 0)), "5:34 PM");
}

#[test]
fn yesterday_says_so_with_its_time() {
    assert_eq!(when_label(&iso(2026, 10, 6, 9, 5), ms(2026, 10, 7, 20, 0)), "yesterday 9:05 AM");
}

#[test]
fn an_older_day_names_the_month_and_day_and_keeps_its_time() {
    assert_eq!(when_label(&iso(2026, 10, 2, 0, 15), ms(2026, 10, 7, 20, 0)), "Oct 2 12:15 AM");
}

#[test]
fn a_time_that_does_not_parse_says_nothing() {
    assert_eq!(when_label("not a time", ms(2026, 10, 7, 20, 0)), "");
}

//! Time markers between turns in a long conversation, and the rail clock format.
//! Port of `clockTime`, `clockOf`, `elapsedInLabel`, `minutesBetween`, and `threadMarkers`
//! from `apps/desktop/src/renderer/src/missionView.ts`.

use chrono::{DateTime, Datelike, Local, Timelike};

/// A quiet gap worth marking. Below this a marker is noise; above it, the
/// reader deserves to know the mission sat still.
pub const QUIET_GAP_MINUTES: i64 = 2;

/// One turn's boundaries in a `ledger::Snapshot`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TurnTiming {
    /// Index in `Snapshot.items` where this turn's items start.
    pub before_item: usize,
    /// RFC3339 timestamp of the turn's first event, if any.
    pub first_at: Option<String>,
    /// RFC3339 timestamp of the turn's last event, if any.
    pub last_at: Option<String>,
}

/// A marker sitting above a turn after a quiet gap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreadMarker {
    /// Index of the item in `Snapshot.items` this marker sits above.
    pub before_item: usize,
    pub at: String,
    pub minutes_in: i64,
    /// Said only when the gap is the point, e.g. `waited 6 min`.
    pub note: Option<String>,
    /// `45 min in`, `3h 20m in`, `2d 4h in` -- never `959 min in`.
    pub elapsed: String,
    /// The date, when this turn is on a different day from the conversation's first.
    pub day: Option<String>,
}

/// Local `HH:MM:SS` clock for the signal rail, or empty string for an invalid timestamp.
pub fn clock_of(iso: &str) -> String {
    let Ok(at) = DateTime::parse_from_rfc3339(iso) else {
        return String::new();
    };
    let local = at.with_timezone(&Local);
    format!("{:02}:{:02}:{:02}", local.hour(), local.minute(), local.second())
}

/// Local `HH:MM` clock for a time marker, or `--:--` for an invalid timestamp.
pub fn clock_time(iso: &str) -> String {
    let Ok(at) = DateTime::parse_from_rfc3339(iso) else {
        return "--:--".to_owned();
    };
    let local = at.with_timezone(&Local);
    format!("{:02}:{:02}", local.hour(), local.minute())
}

/// How far into a conversation a turn is, in units a person thinks in.
pub fn elapsed_in_label(minutes: i64) -> String {
    let minutes = minutes.max(0);
    if minutes < 90 {
        return format!("{minutes} min in");
    }
    let hours = minutes / 60;
    if hours < 24 {
        let rest = minutes % 60;
        return if rest == 0 {
            format!("{hours}h in")
        } else {
            format!("{hours}h {rest}m in")
        };
    }
    let days = hours / 24;
    let rest = hours % 24;
    if rest == 0 {
        format!("{days}d in")
    } else {
        format!("{days}d {rest}h in")
    }
}

/// Whole minutes between two RFC3339 instants, floored, never negative.
pub fn minutes_between(from: &str, to: &str) -> i64 {
    let (Ok(start), Ok(end)) = (DateTime::parse_from_rfc3339(from), DateTime::parse_from_rfc3339(to)) else {
        return 0;
    };
    let ms = end.timestamp_millis() - start.timestamp_millis();
    if ms <= 0 {
        0
    } else {
        ms / 60_000
    }
}

fn same_calendar_day(first: &str, second: &str) -> bool {
    let (Ok(left), Ok(right)) = (DateTime::parse_from_rfc3339(first), DateTime::parse_from_rfc3339(second)) else {
        return true;
    };
    let left = left.with_timezone(&Local);
    let right = right.with_timezone(&Local);
    left.year() == right.year() && left.month() == right.month() && left.day() == right.day()
}

fn day_label(iso: &str) -> Option<String> {
    let at = DateTime::parse_from_rfc3339(iso).ok()?;
    Some(at.with_timezone(&Local).format("%b %-d").to_string())
}

/// Where a long conversation earns a time marker.
pub fn thread_markers(turns: &[TurnTiming]) -> Vec<ThreadMarker> {
    let Some(origin) = turns.iter().find_map(|t| t.first_at.as_deref()) else {
        return Vec::new();
    };
    let mut markers = Vec::new();
    for index in 1..turns.len() {
        let Some(previous) = turns[index - 1].last_at.as_deref() else {
            continue;
        };
        let Some(next) = turns[index].first_at.as_deref() else {
            continue;
        };
        let gap = minutes_between(previous, next);
        if gap < QUIET_GAP_MINUTES {
            continue;
        }
        let minutes_in = minutes_between(origin, next);
        let day = if same_calendar_day(origin, next) {
            None
        } else {
            day_label(next)
        };
        markers.push(ThreadMarker {
            before_item: turns[index].before_item,
            at: next.to_owned(),
            minutes_in,
            note: Some(format!("waited {gap} min")),
            elapsed: elapsed_in_label(minutes_in),
            day,
        });
    }
    markers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(before_item: usize, at: &str) -> TurnTiming {
        TurnTiming {
            before_item,
            first_at: Some(at.to_owned()),
            last_at: Some(at.to_owned()),
        }
    }

    #[test]
    fn elapsed_in_reads_minutes_hours_and_days() {
        assert_eq!(elapsed_in_label(0), "0 min in");
        assert_eq!(elapsed_in_label(45), "45 min in");
        assert_eq!(elapsed_in_label(89), "89 min in");

        assert_eq!(elapsed_in_label(90), "1h 30m in");
        assert_eq!(elapsed_in_label(120), "2h in");
        assert_eq!(elapsed_in_label(305), "5h 5m in");

        // The exact figure from Colin's screenshot (959-minutes-is-not-a-duration.test.ts).
        assert_eq!(elapsed_in_label(959), "15h 59m in");
        assert!(!elapsed_in_label(959).contains("959"));

        assert_eq!(elapsed_in_label(1_440), "1d in");
        assert_eq!(elapsed_in_label(3_000), "2d 2h in");
    }

    #[test]
    fn names_the_day_when_a_turn_has_crossed_one_and_not_before() {
        let overnight = thread_markers(&[
            turn(0, "2026-09-21T08:00:00.000Z"),
            turn(2, "2026-09-23T08:00:00.000Z"),
        ]);
        assert_eq!(overnight.len(), 1);
        assert!(overnight[0].day.is_some(), "a turn on another day must say which");
        assert_eq!(overnight[0].elapsed, "2d in");

        let same_day = thread_markers(&[
            turn(0, "2026-09-22T08:00:00.000Z"),
            turn(2, "2026-09-22T09:30:00.000Z"),
        ]);
        assert_eq!(same_day.len(), 1);
        assert_eq!(same_day[0].day, None);
        assert_eq!(same_day[0].elapsed, "1h 30m in");
    }

    #[test]
    fn says_how_long_the_silence_was_and_skips_short_gaps() {
        let markers = thread_markers(&[
            turn(0, "2026-09-22T08:00:00.000Z"),
            turn(2, "2026-09-22T08:05:00.000Z"),
        ]);
        assert_eq!(markers[0].note.as_deref(), Some("waited 5 min"));

        let short = thread_markers(&[
            turn(0, "2026-09-22T08:00:00.000Z"),
            turn(2, "2026-09-22T08:01:30.000Z"),
        ]);
        assert!(short.is_empty());
    }

    #[test]
    fn rail_clock_formats_valid_iso_and_rejects_invalid() {
        for iso in [
            "2026-09-22T00:00:00.000Z",
            "2026-09-22T12:34:56.000Z",
            "2026-12-31T23:59:59.999Z",
        ] {
            let parsed = DateTime::parse_from_rfc3339(iso).unwrap().with_timezone(&Local);
            let expected = format!("{:02}:{:02}:{:02}", parsed.hour(), parsed.minute(), parsed.second());
            assert_eq!(clock_of(iso), expected);
        }
        assert_eq!(clock_of("not a time"), "");
        assert_eq!(clock_time("not a time"), "--:--");
    }
}

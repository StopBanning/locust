//! a-cloud-watch-stops-when-it-should.test.ts. Electron's fake timers become a clock this test moves; the read is
//! a closure that answers at once, never, or when the test says.

use super::{CLOUD_WATCH_INTERVAL_MS, CLOUD_WATCH_LIMIT_MS, CloudWatch, WatchStep, WatchStop, cloud_updated_label};
use crate::claude_cloud::{ClaudeCloudReading, CloudExchange};

fn reading(answer: &str, at: &str) -> ClaudeCloudReading {
    ClaudeCloudReading::Ok {
        exchanges: vec![CloudExchange {
            prompt: "Test".into(),
            answer: Some(answer.into()),
            at: "2026-10-03T00:00:00Z".into(),
            model: None,
        }],
        checked_at: at.into(),
        diff: None,
        change_too_big: false,
        note: None,
    }
}

fn first() -> ClaudeCloudReading {
    reading("First", "2026-10-03T00:00:00Z")
}

/// What a read does when it starts: answers now (None: it threw), or is left under way.
enum Check {
    Now(Option<ClaudeCloudReading>),
    Pending,
}

/// The row: its watch, the clock, and what it saw (Electron's `check`, `onReading`, `onStopped` spies).
struct Row {
    watch: CloudWatch,
    now: u64,
    checks: usize,
    readings: Vec<ClaudeCloudReading>,
    stopped: Vec<WatchStop>,
}

impl Row {
    fn new(initial: Option<ClaudeCloudReading>) -> Self {
        Self { watch: CloudWatch::start(0, initial.as_ref()), now: 0, checks: 0, readings: Vec::new(), stopped: Vec::new() }
    }

    fn take(&mut self, read: Option<ClaudeCloudReading>) {
        if let Some((reading, stop)) = self.watch.read(self.now, read) {
            self.readings.push(reading);
            self.stopped.extend(stop);
        }
    }

    /// `vi.advanceTimersByTimeAsync(by)`: everything due up to and including the end runs.
    fn advance(&mut self, by: u64, check: &mut dyn FnMut(u64) -> Check) {
        let end = self.now + by;
        loop {
            match self.watch.poll(self.now) {
                WatchStep::Read => {
                    self.checks += 1;
                    if let Check::Now(read) = check(self.now) {
                        self.take(read);
                    }
                }
                WatchStep::Stopped(why) => self.stopped.push(why),
                WatchStep::Wait => match self.watch.next_wake() {
                    // A wake at or before now with nothing to do would spin forever: Electron's timer has fired.
                    Some(at) if at <= self.now => panic!("the watch waits at {at} for a moment already reached"),
                    Some(at) if at <= end => self.now = at,
                    _ => {
                        self.now = end;
                        return;
                    }
                },
            }
        }
    }
}

#[test]
// case: Watch reads every ninety seconds and stops after two unchanged reads despite new timestamps.
fn watch_reads_every_ninety_seconds_and_stops_after_two_unchanged_reads_despite_new_timestamps() {
    let mut row = Row::new(Some(first()));
    let mut check = |now: u64| Check::Now(Some(reading("First", &format!("2026-10-03T00:{:02}:00Z", now / 60_000))));
    row.advance(90_000 - 1, &mut check);
    assert_eq!(row.checks, 0);
    row.advance(1, &mut check);
    assert_eq!(row.checks, 1);
    row.advance(90_000 * 3, &mut check);
    assert_eq!(row.checks, 2);
    assert_eq!(row.stopped, vec![WatchStop::Unchanged]);
}

#[test]
// case: Watch resets its unchanged count when a reply or change arrives.
fn watch_resets_its_unchanged_count_when_a_reply_or_change_arrives() {
    let mut row = Row::new(Some(first()));
    let mut n = 0;
    let mut check = |_: u64| {
        n += 1;
        Check::Now(Some(reading(if n < 2 { "First" } else { "Second" }, "2026-10-03T00:00:00Z")))
    };
    row.advance(90_000 * 3, &mut check);
    assert_eq!(row.checks, 3);
    assert!(row.stopped.is_empty());
    row.advance(90_000, &mut check);
    assert_eq!(row.stopped, vec![WatchStop::Unchanged]);
}

#[test]
// case: Watch stops after thirty minutes even when every read brings something new.
fn watch_stops_after_thirty_minutes_even_when_every_read_brings_something_new() {
    let mut row = Row::new(Some(first()));
    let mut n = 0;
    let mut check = |_: u64| {
        n += 1;
        Check::Now(Some(reading(&n.to_string(), "2026-10-03T00:00:00Z")))
    };
    row.advance(30 * 60_000 + 90_000, &mut check);
    assert_eq!(row.checks, 19);
    assert_eq!(row.stopped, vec![WatchStop::Time]);
}

#[test]
// case: Closing a row cancels Watch and ignores a read that finishes afterward.
fn closing_a_row_cancels_watch_and_ignores_a_read_that_finishes_afterward() {
    let mut row = Row::new(Some(first()));
    row.advance(90_000, &mut |_| Check::Pending);
    row.watch.stop();
    row.take(Some(first()));
    row.advance(30 * 60_000, &mut |_| Check::Pending);
    assert_eq!(row.checks, 1);
    assert!(row.readings.is_empty());
    assert!(row.stopped.is_empty());
}

#[test]
// case: Watch never overlaps slow reads and stops at its deadline while a read is pending.
fn watch_never_overlaps_slow_reads_and_stops_at_its_deadline_while_a_read_is_pending() {
    let mut row = Row::new(Some(first()));
    row.advance(30 * 60_000 + 90_000, &mut |_| Check::Pending);
    assert_eq!(row.checks, 1);
    assert_eq!(row.stopped, vec![WatchStop::Time]);
}

#[test]
// case: Watch reports a failed read once and schedules no further reads.
fn watch_reports_a_failed_read_once_and_schedules_no_further_reads() {
    let mut row = Row::new(Some(first()));
    row.advance(30 * 60_000, &mut |_| Check::Now(None));
    assert_eq!(row.checks, 1);
    assert_eq!(row.readings.len(), 1);
    assert!(matches!(row.readings[0], ClaudeCloudReading::Err { .. }));
    assert_eq!(row.stopped, vec![WatchStop::Failed]);
}

#[test]
// case: The row names the number of minutes since its last successful read.
fn the_row_names_the_number_of_minutes_since_its_last_successful_read() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-10-03T00:02:59Z").unwrap().timestamp_millis();
    assert_eq!(cloud_updated_label("2026-10-03T00:00:00Z", now), "Updated 2 min ago");
}

#[test]
fn the_interval_and_the_limit_are_electrons() {
    assert_eq!(CLOUD_WATCH_INTERVAL_MS, 90_000);
    assert_eq!(CLOUD_WATCH_LIMIT_MS, 1_800_000);
}

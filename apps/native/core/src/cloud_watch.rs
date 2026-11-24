//! Watching a Claude cloud session while its row is open (`apps/desktop/src/renderer/src/cloudWatch.ts`, W4: "while
//! the session's row is open, re-read it ... every 90 s"; "stop when two reads in a row bring nothing new, after 30
//! minutes, or when the row closes"). No sends, only reads.
//!
//! Electron's timers become a clock the caller passes in: `poll` says when a read is due or the watch has stopped,
//! `read` takes what the read brought, and `next_wake` says when to poll again. The row owns its watch and stops it
//! on close.

use chrono::DateTime;

use crate::claude_cloud::ClaudeCloudReading;

pub const CLOUD_WATCH_INTERVAL_MS: u64 = 90_000;
pub const CLOUD_WATCH_LIMIT_MS: u64 = 30 * 60_000;

/// Why a watch stopped by itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchStop {
    /// Two reads in a row brought nothing new.
    Unchanged,
    /// Thirty minutes passed.
    Time,
    /// A read failed; it was shown once.
    Failed,
}

/// What the watch wants now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchStep {
    Wait,
    /// Start one read now, and hand what it brings to `read`.
    Read,
    Stopped(WatchStop),
}

pub const WATCH_READ_FAILED: &str = "Claude Code could not be reached to read it. Check again.";

#[derive(Debug, Clone)]
pub struct CloudWatch {
    deadline: u64,
    next: Option<u64>,
    busy: bool,
    active: bool,
    unchanged: u32,
    previous: Option<ClaudeCloudReading>,
}

/// A reading without the time it was taken: a newer `checkedAt` is not news.
fn content(reading: &ClaudeCloudReading) -> ClaudeCloudReading {
    match reading {
        ClaudeCloudReading::Ok { exchanges, diff, change_too_big, note, .. } => ClaudeCloudReading::Ok {
            exchanges: exchanges.clone(),
            checked_at: String::new(),
            diff: diff.clone(),
            change_too_big: *change_too_big,
            note: note.clone(),
        },
        failed => failed.clone(),
    }
}

impl CloudWatch {
    /// A watch from `now` (milliseconds on the caller's clock). With a reading already on the row, the first read
    /// is one interval away; without one, it is due at once.
    pub fn start(now: u64, initial: Option<&ClaudeCloudReading>) -> Self {
        Self {
            deadline: now + CLOUD_WATCH_LIMIT_MS,
            next: Some(if initial.is_some() { now + CLOUD_WATCH_INTERVAL_MS } else { now }),
            busy: false,
            active: true,
            unchanged: 0,
            previous: initial.map(content),
        }
    }

    pub fn poll(&mut self, now: u64) -> WatchStep {
        if !self.active {
            return WatchStep::Wait;
        }
        // The deadline comes before a read due at the same moment, as Electron's earlier timer does.
        if now >= self.deadline {
            self.active = false;
            return WatchStep::Stopped(WatchStop::Time);
        }
        if !self.busy && self.next.is_some_and(|at| now >= at) {
            self.busy = true;
            self.next = None;
            return WatchStep::Read;
        }
        WatchStep::Wait
    }

    /// When to poll again; None once stopped.
    pub fn next_wake(&self) -> Option<u64> {
        if !self.active {
            return None;
        }
        Some(self.next.map_or(self.deadline, |at| at.min(self.deadline)))
    }

    /// What a read brought (None: it failed outright). Returns the reading to show and, when the watch stops
    /// because of it, why; None when the row closed or the time ran out while the read was under way.
    pub fn read(
        &mut self,
        now: u64,
        reading: Option<ClaudeCloudReading>,
    ) -> Option<(ClaudeCloudReading, Option<WatchStop>)> {
        self.busy = false;
        if !self.active || now >= self.deadline {
            return None;
        }
        let reading = reading.unwrap_or_else(|| ClaudeCloudReading::Err { message: WATCH_READ_FAILED.into() });
        if matches!(reading, ClaudeCloudReading::Err { .. }) {
            self.active = false;
            return Some((reading, Some(WatchStop::Failed)));
        }
        let current = content(&reading);
        self.unchanged = if self.previous.as_ref() == Some(&current) { self.unchanged + 1 } else { 0 };
        self.previous = Some(current);
        if self.unchanged >= 2 {
            self.active = false;
            return Some((reading, Some(WatchStop::Unchanged)));
        }
        self.next = Some(now + CLOUD_WATCH_INTERVAL_MS);
        Some((reading, None))
    }

    /// The row closed: nothing more is read, and a read still under way is ignored.
    pub fn stop(&mut self) {
        self.active = false;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

/// "Updated N min ago", from the last successful read's time.
pub fn cloud_updated_label(checked_at: &str, now_ms: i64) -> String {
    let at = DateTime::parse_from_rfc3339(checked_at).map(|t| t.timestamp_millis()).unwrap_or(now_ms);
    let minutes = ((now_ms - at) / 60_000).max(0);
    format!("Updated {minutes} min ago")
}

#[cfg(test)]
#[path = "cloud_watch_tests.rs"]
mod tests;

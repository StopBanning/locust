//! WHERE THE SECONDS BEFORE "STARTED" WENT (Electron `start-timing.ts`, 0.602). The ledger recorded when a mission was
//! created and when the runtime said it had started, and nothing in between: a median 4.5 to 10.5 s per runtime that
//! nobody could split between Locust's own start path and the CLI's boot. The start path marks when each of its phases
//! began, the writer marks when the runtime first said it had begun, and the run's end writes one host note with the
//! durations -- shown under Details, kept in the saved record, and the first thing to read when a start feels slow.
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// The start path's phases, in the order they run (Electron's `StartPhase`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Looking,
    Briefing,
    ReadingFolder,
    StartingRuntime,
}

const ORDER: [Phase; 4] = [Phase::Looking, Phase::Briefing, Phase::ReadingFolder, Phase::StartingRuntime];

#[derive(Clone, Debug, Default)]
pub struct StartTiming {
    /// When the person's send reached the host (milliseconds).
    pub sent_at: i64,
    /// When each phase began; a phase that did not run is absent (a read-only run reads no folder).
    pub phase_at: Vec<(Phase, i64)>,
    /// When the runtime first said it had begun; set once.
    pub first_event_at: Option<i64>,
}

impl StartTiming {
    fn at(&self, phase: Phase) -> Option<i64> {
        self.phase_at.iter().find(|(p, _)| *p == phase).map(|(_, at)| *at)
    }
}

/// `toFixed(1)` of seconds, never below zero.
fn seconds(ms: i64) -> String {
    format!("{} s", crate::benchmark::to_fixed(ms.max(0) as f64 / 1000.0, 1))
}

/// The sentence, or None before the runtime has said anything (then there is nothing to date).
pub fn sentence(timing: &StartTiming, runtime_name: &str) -> Option<String> {
    let first = timing.first_event_at?;
    let marks: Vec<(Phase, i64)> = ORDER.iter().filter_map(|p| timing.at(*p).map(|at| (*p, at))).collect();
    let mut parts = Vec::new();
    for (index, (phase, from)) in marks.iter().enumerate() {
        let to = marks.get(index + 1).map_or(first, |(_, at)| *at);
        let took = seconds(to - from);
        match phase {
            Phase::Looking => parts.push(format!("looked for {runtime_name} {took}")),
            Phase::Briefing => parts.push(format!("briefed {took}")),
            Phase::ReadingFolder => parts.push(format!("read the folder {took}")),
            Phase::StartingRuntime => {}
        }
    }
    let boot = timing.at(Phase::StartingRuntime).map(|spawn| format!("{runtime_name} took {} to say it had started", seconds(first - spawn)));
    let total = format!("Started in {}", seconds(first - timing.sent_at));
    Some(match (parts.is_empty(), boot) {
        (true, None) => format!("{total}."),
        (true, Some(boot)) => format!("{total}: {boot}."),
        (false, None) => format!("{total}: {}.", parts.join(", ")),
        (false, Some(boot)) => format!("{total}: {}; {boot}.", parts.join(", ")),
    })
}

/// The host note for the record: an info-level diagnostic after the run's own events.
pub fn note(run: &str, mission: &str, adapter: &str, next_sequence: u64, at: &str, runtime_name: &str, timing: &StartTiming) -> Option<Value> {
    let message = sentence(timing, runtime_name)?;
    Some(json!({
        "runId": run, "missionId": mission, "occurredAt": at, "sourceAdapter": adapter,
        "id": format!("{run}:host:start-timing:{next_sequence}"), "sequence": next_sequence, "type": "adapter.diagnostic",
        "payload": {"level": "info", "code": "host.start-timing", "message": message, "terminal": false, "evidence": {"redacted": true}}
    }))
}

/// One send's timing, marked from the start path's thread and the writer's.
#[derive(Clone, Default)]
pub struct Clock(Arc<Mutex<StartTiming>>);

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

impl Clock {
    /// A send that reached the host now.
    pub fn sent() -> Self {
        Clock(Arc::new(Mutex::new(StartTiming { sent_at: now_ms(), ..Default::default() })))
    }
    /// A phase begins now.
    pub fn mark(&self, phase: Phase) {
        if let Ok(mut t) = self.0.lock()
            && t.at(phase).is_none()
        {
            t.phase_at.push((phase, now_ms()));
        }
    }
    /// The runtime said it had begun, now; only the first time counts.
    pub fn began(&self) {
        if let Ok(mut t) = self.0.lock()
            && t.first_event_at.is_none()
        {
            t.first_event_at = Some(now_ms());
        }
    }
    pub fn timing(&self) -> StartTiming {
        self.0.lock().map(|t| t.clone()).unwrap_or_default()
    }
}

#[cfg(test)]
#[path = "start_timing_tests.rs"]
mod tests;

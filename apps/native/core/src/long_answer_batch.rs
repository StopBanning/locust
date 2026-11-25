//! A streamed batch is written before it is shown (`main/streamed-event-batches.ts`, `main/durable-event-updates.ts`,
//! `shared/streamed-updates.ts`).
//!
//! A long answer arrives as many small `message.delta` events. The run loop (`native::drive_runtime`) holds text for
//! up to [`STREAM_BATCH_MS`] and writes what waits in ONE write and one sync ([`LedgerWriter::append_all`]); any event
//! that is not text (a step, a tool, an ending) makes everything waiting due at once, so order is kept across both. A
//! batch is on disk whole before anything reads it: the window shows what the ledger holds, and
//! [`persist_event_updates`] hands a batch on only after its append returned. A write that fails stops the batcher:
//! what waited is dropped with it, and the run ends, as a failed single write always ended it.
//!
//! Electron's batcher writes on its own promise, so a second batch can wait behind one in flight. Here the run loop
//! writes, synchronously, so there is never a write in flight while it pushes or flushes.

use std::io;
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::ledger_write::LedgerWriter;
use crate::runtime::Event;

/// How long text waits for company before it is written (`STREAM_BATCH_MS`).
pub const STREAM_BATCH_MS: u64 = 50;

/// Text held for at most one window; anything else written at once, behind it.
pub struct StreamBatcher {
    waiting: Vec<Event>,
    deadline: Option<Instant>,
    window: Duration,
    stopped: bool,
}

impl Default for StreamBatcher {
    fn default() -> Self {
        Self::with_window(Duration::from_millis(STREAM_BATCH_MS))
    }
}

impl StreamBatcher {
    pub fn with_window(window: Duration) -> Self {
        StreamBatcher { waiting: Vec::new(), deadline: None, window, stopped: false }
    }

    /// Events that wait to be written.
    pub fn pending_count(&self) -> usize {
        self.waiting.len()
    }

    /// When the text that waits falls due, if any waits.
    pub fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    /// Take events in. Text starts a window if none runs; anything else writes everything waiting now.
    pub fn push(
        &mut self,
        events: Vec<Event>,
        now: Instant,
        write: impl FnOnce(Vec<Event>) -> io::Result<()>,
    ) -> io::Result<()> {
        if self.stopped || events.is_empty() {
            return Ok(());
        }
        let activity = events.iter().any(|event| event.kind != "message.delta");
        self.waiting.extend(events);
        if activity {
            return self.write_now(write);
        }
        if self.deadline.is_none() {
            self.deadline = Some(now + self.window);
        }
        Ok(())
    }

    /// Write the text whose window has passed, without waiting for another record or a run ending.
    pub fn tick(&mut self, now: Instant, write: impl FnOnce(Vec<Event>) -> io::Result<()>) -> io::Result<()> {
        match self.deadline {
            Some(due) if now >= due => self.write_now(write),
            _ => Ok(()),
        }
    }

    /// Write everything waiting now (an approval's record, a run that ended, the process going away).
    pub fn flush(&mut self, write: impl FnOnce(Vec<Event>) -> io::Result<()>) -> io::Result<()> {
        self.write_now(write)
    }

    fn write_now(&mut self, write: impl FnOnce(Vec<Event>) -> io::Result<()>) -> io::Result<()> {
        self.deadline = None;
        if self.stopped || self.waiting.is_empty() {
            return Ok(());
        }
        let batch = std::mem::take(&mut self.waiting);
        let written = write(batch);
        if written.is_err() {
            // A ledger that refused a batch takes no more: nothing after it would read back as part of the run.
            self.stopped = true;
            self.waiting.clear();
        }
        written
    }
}

/// What a written batch shows (`streamedUpdates`): consecutive text as one update, every other event as its own.
#[derive(Clone, Debug, PartialEq)]
pub enum StreamedUpdate {
    MessageDeltas(Vec<Value>),
    Event(Value),
}

impl StreamedUpdate {
    pub fn kind(&self) -> &'static str {
        match self {
            StreamedUpdate::MessageDeltas(_) => "message-deltas",
            StreamedUpdate::Event(_) => "event",
        }
    }

    pub fn sequences(&self) -> Vec<u64> {
        match self {
            StreamedUpdate::MessageDeltas(events) => events.iter().filter_map(|e| e["sequence"].as_u64()).collect(),
            StreamedUpdate::Event(event) => event["sequence"].as_u64().into_iter().collect(),
        }
    }
}

pub fn streamed_updates(events: &[Value]) -> Vec<StreamedUpdate> {
    let mut updates = Vec::new();
    let mut deltas = Vec::new();
    for event in events {
        if event["type"] == "message.delta" {
            deltas.push(event.clone());
        } else {
            if !deltas.is_empty() {
                updates.push(StreamedUpdate::MessageDeltas(std::mem::take(&mut deltas)));
            }
            updates.push(StreamedUpdate::Event(event.clone()));
        }
    }
    if !deltas.is_empty() {
        updates.push(StreamedUpdate::MessageDeltas(deltas));
    }
    updates
}

/// Write a batch, then let it be known, then shown (`persistEventUpdates`): an append that fails, or a `track` that
/// fails after it, shows nothing.
pub fn persist_event_updates(
    writer: &mut LedgerWriter,
    events: &[Event],
    track: impl FnOnce(&[Value]) -> io::Result<()>,
    mut emit: impl FnMut(StreamedUpdate),
) -> io::Result<()> {
    let kept = writer.append_all(events)?;
    track(&kept)?;
    for update in streamed_updates(&kept) {
        emit(update);
    }
    Ok(())
}

#[cfg(test)]
#[path = "long_answer_batch_tests.rs"]
mod tests;

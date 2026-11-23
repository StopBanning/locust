//! LIVE2 #12 (23 Nov): a Scout lane run through the Rust app ended with "OpenCode sent output faster than Locust could
//! record it": the server's events waited in a queue of 256 while the engine was busy. Electron's opencode-serve-run.ts
//! holds 20,000 (`NotificationQueue(20_000)`) before it ends a run.
use super::opencode_serve::{MAX_QUEUED_EVENTS, event_channel, read_events};
use std::io::Cursor;
use std::sync::{Mutex, atomic::{AtomicBool, Ordering}};

fn stream(events: usize) -> Cursor<Vec<u8>> {
    let mut s = String::new();
    for i in 0..events {
        s.push_str(&format!("data: {{\"type\":\"message.part.updated\",\"properties\":{{\"n\":{i}}}}}\n\n"));
    }
    Cursor::new(s.into_bytes())
}

#[test]
fn a_burst_of_a_thousand_events_waits_for_a_busy_engine_instead_of_ending_the_run() {
    let (tx, rx) = event_channel();
    let (end, sid, overflow) = (AtomicBool::new(false), Mutex::new(None), AtomicBool::new(false));
    read_events(stream(1_000), &tx, &end, &sid, &overflow);
    assert!(!overflow.load(Ordering::Relaxed), "1,000 waiting events ended the run");
    assert_eq!(rx.try_iter().count(), 1_000);
}

#[test]
fn holds_as_many_events_as_electron_and_ends_the_run_only_past_that() {
    assert_eq!(MAX_QUEUED_EVENTS, 20_000);
    let (tx, rx) = event_channel();
    let (end, sid, overflow) = (AtomicBool::new(false), Mutex::new(None), AtomicBool::new(false));
    read_events(stream(MAX_QUEUED_EVENTS + 1), &tx, &end, &sid, &overflow);
    assert!(overflow.load(Ordering::Relaxed), "past the bound the run still ends rather than grow without limit");
    assert_eq!(rx.try_iter().count(), MAX_QUEUED_EVENTS);
}

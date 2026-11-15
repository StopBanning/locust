//! How tests that wait on another process wait (findings 8 of the 15 Nov review; notes/LOAD-NOTES.md).
//!
//! A test that starts a process (a shell, a fake agent, a Python server) waits for the thing it is checking to
//! happen, polling, and gives up only after `UNDER_LOAD`. When the event comes, the test goes on at once, so a long
//! limit costs nothing on a quiet machine. On a machine running four agents' builds, a process can take tens of
//! seconds to be scheduled, and a short limit failed tests on unchanged source.

use std::time::{Duration, Instant};

/// The outer limit for a wait on an event from a process the test started.
pub const UNDER_LOAD: Duration = Duration::from_secs(60);

/// Polls `done` every 10 ms until it is true, or panics with `what` after `UNDER_LOAD`. Returns how long it took.
pub fn wait_for(what: &str, mut done: impl FnMut() -> bool) -> Duration {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < UNDER_LOAD, "timed out after {UNDER_LOAD:?} waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
    start.elapsed()
}

/// With `LOCUST_LOAD_TIMES` set to a file, adds `name<TAB>milliseconds` to it: how long a load-sensitive test took,
/// for the worst-time table in notes/LOAD-NOTES.md. Without it, nothing.
pub fn note_time(name: &str, took: Duration) {
    let Ok(path) = std::env::var("LOCUST_LOAD_TIMES") else {
        return;
    };
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{name}\t{}", took.as_millis());
    }
}

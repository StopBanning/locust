//! Assertions for program data, never for filesystem, network, or runtime output.
pub(crate) fn regex(pattern: &'static str) -> regex::Regex {
    // Callers supply only source literals (or entries in literal arrays), exercised by tests.
    // A bad pattern is a programming error; no external input can make compilation fail.
    regex::Regex::new(pattern).expect("valid source regex")
}
pub(crate) fn lock<T>(mutex: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    // These locks protect internal state. Poison requires an earlier panic during mutation;
    // continuing with that partially updated state would violate the owner's invariants.
    mutex
        .lock()
        .expect("internal state mutation must not panic")
}

/// Nanoseconds since 1970 that never repeat within this process: the clock, or one past the last value handed out,
/// whichever is later. Names and ids built from the clock alone collided when two were made in the same tick: two
/// approval cards raised together got the same id, the second overwrote the first, and both turns failed with
/// "Approval channel closed" (found 11/11 by a Board test). Every clock-based id or temp name uses this.
pub(crate) fn unique_nanos() -> u128 {
    static LAST: std::sync::Mutex<u128> = std::sync::Mutex::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut last = lock(&LAST);
    *last = (*last + 1).max(now);
    *last
}

#[cfg(test)]
mod unique_tests {
    #[test]
    fn unique_nanos_never_repeat_even_across_threads() {
        let threads: Vec<_> = (0..8)
            .map(|_| std::thread::spawn(|| (0..2000).map(|_| super::unique_nanos()).collect::<Vec<_>>()))
            .collect();
        let mut all: Vec<u128> = threads.into_iter().flat_map(|t| t.join().expect("thread")).collect();
        let n = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), n, "a value repeated");
    }
}

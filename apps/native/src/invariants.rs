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

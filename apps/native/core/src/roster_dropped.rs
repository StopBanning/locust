//! A teammate record that does not parse is dropped, never silently (teammate-store.ts parsedFile, LIVE2 #16).
//!
//! One bad record must not cost someone their roster, so it is dropped and the rest load; but a teammate whose hue is
//! not one of the eleven vanished from the window and the door without a word. Electron says it once per read, in the
//! log; so does this.

/// Electron's words for `dropped` records, or nothing when every record read.
pub fn words(dropped: usize) -> Option<String> {
    (dropped > 0).then(|| format!("Roster file: dropped {dropped} teammate record(s) that did not parse."))
}

/// Said in the log when the roster read `read` records and kept `kept` of them.
pub fn say(read: usize, kept: usize) {
    let Some(words) = words(read.saturating_sub(kept)) else { return };
    #[cfg(test)]
    SAID.with(|said| said.borrow_mut().push(words.clone()));
    eprintln!("{words}");
}

#[cfg(test)]
thread_local! {
    /// What this thread's roster reads said, for the tests.
    pub(crate) static SAID: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
#[path = "roster_dropped_tests.rs"]
mod tests;

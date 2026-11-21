//! Electron's main/npm-precondition.test.ts: whether npm can be run reaches the window. A machine without npm is told
//! so, one with it is told so too (the control: a field that is always false would hide every Install on every
//! machine), and a check that asks nobody assumes npm is there.
use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// A check whose probe answers `found`, and how many times it was asked.
fn asking(found: bool) -> (NpmCheck, Arc<AtomicU32>) {
    let asked = Arc::new(AtomicU32::new(0));
    let counted = asked.clone();
    let check = NpmCheck::new(Some(Box::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
        found
    })));
    (check, asked)
}

// case: says so when npm is not there
#[test]
fn says_so_when_npm_is_not_there() {
    let (check, asked) = asking(false);
    assert_eq!(check.seen(), None, "nothing is known before npm is asked");
    assert!(!check.present());
    assert_eq!(check.seen(), Some(false));
    // Asked once and remembered, until an install finishes.
    assert!(!check.present());
    assert_eq!(asked.load(Ordering::SeqCst), 1);
    check.forget();
    assert_eq!(check.seen(), None);
    assert!(!check.present());
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    // npm is asked as a program that must start, finish cleanly, and do so in time.
    assert!(!answers("locust-no-such-program-anywhere", &["--version"], NPM_PROBE_TIMEOUT));
    assert!(!answers("python3", &["-c", "import sys; sys.exit(1)"], NPM_PROBE_TIMEOUT));
    let started = Instant::now();
    assert!(!answers("python3", &["-c", "import time; time.sleep(30)"], Duration::from_millis(300)));
    assert!(started.elapsed() < Duration::from_secs(10), "one that does not answer is ended at the limit");
    assert_eq!(NPM_PROBE_TIMEOUT, Duration::from_secs(5));
}

// case: says so when it is
#[test]
fn says_so_when_it_is() {
    let (check, asked) = asking(true);
    assert!(check.present());
    assert_eq!(check.seen(), Some(true));
    assert!(check.present());
    assert_eq!(asked.load(Ordering::SeqCst), 1);
    assert!(answers("python3", &["--version"], NPM_PROBE_TIMEOUT));
}

// case: assumes it is there when nobody asked
#[test]
fn assumes_it_is_there_when_nobody_asked() {
    assert!(NpmCheck::new(None).present());
    // This crate's tests never ask the machine's npm.
    assert!(this_machine().present());
}

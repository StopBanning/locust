//! WHETHER npm CAN BE RUN (Electron `main/index.ts` `npmPresent`, runtime-discovery.ts `npmPresent`).
//!
//! npm missing is a precondition, not a failure. The agents Settings installs run through npm, so a machine without
//! Node cannot use Install at all, and a button that cannot work is worse than the truth: pressed, it ended in "npm
//! stopped with an error after 1s", from the command the app had just offered to run. So npm is asked once whether it
//! answers, the answer is remembered, and a NOT INSTALLED row offers no working Install while npm is not there.
//!
//! - **Asked the way an install runs it**: `npm --version`, the cheapest thing npm answers, spawned with no shell from the
//!   PATH Locust adopted at launch, as `agent_install::Npm` spawns the install itself, so the two cannot disagree.
//! - **Bounded**: an npm that does not answer in five seconds (a shim with no version picked, a wrapper waiting on a
//!   proxy) is treated as not there.
//! - **Remembered until an install finishes**: it is an answer that changes when someone installs Node, not between
//!   sweeps, and an install finishing is the one moment it can have changed.
//! - **Assumed there when nobody asked**: with no probe (a test, or a check not yet answered) npm counts as present,
//!   so nothing starts hiding Install on a machine that was never asked.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long npm has to answer (Electron `NPM_PROBE_TIMEOUT_MS`).
pub const NPM_PROBE_TIMEOUT: Duration = Duration::from_secs(5);

/// The window's words for an Install that cannot run (Screens.tsx, the Install button's title).
pub const NPM_MISSING: &str = "Node.js is not on this machine, and this installs through npm.";

/// npm's answer, asked once and remembered.
pub struct NpmCheck {
    probe: Option<Box<dyn Fn() -> bool + Send + Sync>>,
    seen: Mutex<Option<bool>>,
}

impl NpmCheck {
    /// A check that asks `probe`; None asks nobody, and npm is assumed there.
    pub fn new(probe: Option<Box<dyn Fn() -> bool + Send + Sync>>) -> Self {
        Self { probe, seen: Mutex::new(None) }
    }

    /// Whether npm can be run: the remembered answer, or the probe's, asked now and remembered. Never call it on the
    /// window's thread: the first ask can take five seconds.
    pub fn present(&self) -> bool {
        if let Some(seen) = self.seen() {
            return seen;
        }
        let Some(probe) = &self.probe else { return true };
        // The lock is not held while npm is asked, so the window reading `seen` never waits on it.
        let found = probe();
        *self.seen.lock().unwrap_or_else(|e| e.into_inner()) = Some(found);
        found
    }

    /// The remembered answer, without asking: None before npm has been asked.
    pub fn seen(&self) -> Option<bool> {
        *self.seen.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Drop the remembered answer: an install finished, so the next sweep asks again.
    pub fn forget(&self) {
        *self.seen.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

/// Whether `program args` starts and exits cleanly within `limit`. One that cannot start, fails, or keeps running past
/// the limit (it is ended) does not answer.
pub fn answers(program: &str, args: &[&str], limit: Duration) -> bool {
    let child = std::process::Command::new(program).args(args).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
    let Ok(mut child) = child else { return false };
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

/// This machine's one check: `npm --version`, bounded. In this crate's own tests nobody is asked.
pub fn this_machine() -> &'static NpmCheck {
    static ONE: std::sync::OnceLock<NpmCheck> = std::sync::OnceLock::new();
    ONE.get_or_init(|| {
        if cfg!(test) {
            NpmCheck::new(None)
        } else {
            NpmCheck::new(Some(Box::new(|| answers("npm", &["--version"], NPM_PROBE_TIMEOUT))))
        }
    })
}

#[cfg(test)]
#[path = "npm_check_tests.rs"]
mod npm_check_tests;

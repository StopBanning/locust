//! When a surface's motion rests: Electron's `useCoverActivity.ts` (`watchCoverActivity`, 0.623), the rule Home's cover
//! and Settings' face legend share. Motion is held while the window is not focused, while it is hidden, or while the
//! person asks for reduced motion; otherwise it runs until 45 s have passed with no input and no work arriving.
//!
//! Electron keeps one deadline rather than a polling clock. This is the same rule as a value: the window tells it what
//! happened and when (`touch`, `set_focused`, `set_hidden`, `set_reduced`), asks whether it is `away`, and asks
//! `rests_in` for the one wake it needs. Once rested, or while held, it needs none.

/// `COVER_REST_AFTER_MS`: how long without input or work before motion rests.
pub const COVER_REST_AFTER_MS: u64 = 45_000;
const REST_AFTER_S: f64 = COVER_REST_AFTER_MS as f64 / 1000.0;

#[derive(Debug, Clone, PartialEq)]
pub struct CoverActivity {
    /// When input or work last arrived (`last`), in the caller's seconds.
    last_s: f64,
    focused: bool,
    hidden: bool,
    reduced: bool,
}

impl CoverActivity {
    /// Starts watching at `now_s`, as the window is: its first deadline is 45 s away (`last = performance.now()`).
    pub fn new(now_s: f64, focused: bool, hidden: bool, reduced: bool) -> Self {
        Self { last_s: now_s, focused, hidden, reduced }
    }

    /// `held()`: not focused, hidden, or reduced motion. Nothing wakes it while held.
    pub fn held(&self) -> bool {
        !self.focused || self.hidden || self.reduced
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    pub fn reduced(&self) -> bool {
        self.reduced
    }

    /// `presence.away()`: held, or 45 s without input or work.
    pub fn away(&self, now_s: f64) -> bool {
        self.held() || now_s - self.last_s >= REST_AFTER_S
    }

    /// The one deadline still set, in seconds from `now_s`: none while held or once rested (no timers left).
    pub fn rests_in(&self, now_s: f64) -> Option<f64> {
        let left = REST_AFTER_S - (now_s - self.last_s);
        (!self.held() && left > 0.0).then_some(left)
    }

    /// `touch`: input (pointer move or down, a key, the wheel) or work arriving. It resumes at once and the 45 s start
    /// again; a held window only has its time noted.
    pub fn touch(&mut self, now_s: f64) {
        self.last_s = now_s;
    }

    /// `focus` touches; `blur` holds.
    pub fn set_focused(&mut self, focused: bool, now_s: f64) {
        self.focused = focused;
        if focused {
            self.touch(now_s);
        }
    }

    /// `visibilitychange`: shown again and not otherwise held, it touches.
    pub fn set_hidden(&mut self, hidden: bool, now_s: f64) {
        self.hidden = hidden;
        if !self.held() {
            self.touch(now_s);
        }
    }

    /// The reduced-motion preference changing: let go of, it touches.
    pub fn set_reduced(&mut self, reduced: bool, now_s: f64) {
        self.reduced = reduced;
        if !self.held() {
            self.touch(now_s);
        }
    }
}

#[cfg(test)]
#[path = "cover_activity_tests.rs"]
mod tests;

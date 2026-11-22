//! WHAT HAPPENS WHEN A RUN ENDS, IN ORDER, EACH ON ITS OWN (Electron `main/run-end.ts`, A2.14).
//!
//! The run's memory goes first, once per run -- before a share starts the recipient, so that teammate is briefed with
//! what this run remembered -- and every step runs whatever the one before it did: a failure is noted, never carried
//! forward into skipping the rest.
//!
//! `native::send_from` keeps one of these per run. It remembers where the reply is read (before the edit check and
//! before the turn's slot is released), then shares and ends through the same value, whose memory step is then
//! already done.

/// One step of a run's end. `Err` is the step failing; its words go into the note.
pub type Step<'a> = Box<dyn FnOnce() -> Result<(), String> + 'a>;

pub struct RunEnd<'a> {
    mission_id: String,
    remembered: bool,
    note: Box<dyn FnMut(String) + 'a>,
}

impl<'a> RunEnd<'a> {
    pub fn new(mission_id: impl Into<String>, note: impl FnMut(String) + 'a) -> Self {
        Self { mission_id: mission_id.into(), remembered: false, note: Box::new(note) }
    }

    fn safely(&mut self, label: &str, step: impl FnOnce() -> Result<(), String>) {
        if let Err(error) = step() {
            (self.note)(format!("{label} failed for {}: {error}", self.mission_id));
        }
    }

    /// The run's memory, once: a second call, from the share or the end, does nothing.
    pub fn remember(&mut self, memory: impl FnOnce() -> Result<(), String>) {
        if self.remembered {
            return;
        }
        self.remembered = true;
        self.safely("memory", memory);
    }

    /// A reply shared with a teammate: memory first, then the relay, which may start the recipient.
    pub fn shared(&mut self, memory: impl FnOnce() -> Result<(), String>, share: impl FnOnce() -> Result<(), String>) {
        self.remember(memory);
        self.safely("relay share", share);
    }

    /// The run's end: memory first, then the relay's end, then each step after it in order.
    pub fn ended(
        &mut self,
        memory: impl FnOnce() -> Result<(), String>,
        relay: impl FnOnce() -> Result<(), String>,
        after: Vec<(&str, Step<'_>)>,
    ) {
        self.remember(memory);
        self.safely("relay", relay);
        for (label, step) in after {
            self.safely(label, step);
        }
    }
}

#[cfg(test)]
#[path = "run_end_tests.rs"]
mod run_end_tests;

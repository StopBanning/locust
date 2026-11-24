//! What pressing Stop means, given the run on screen (Electron's `stopPress.ts`, 0.806).
//!
//! A run is `starting` from the moment the person presses send until the engine answers with the run's name, and the
//! composer draws Stop for the whole of that, because the runtime is already going. There is nothing to cancel BY yet,
//! so a press against a starting run is REMEMBERED against the key the run is filed under, and the start honours it as
//! soon as the run can be named. Dropping it instead let a second turn run to completion (measured 2026-09-10, on
//! Claude Code and Codex alike).

/// The three answers; the middle one is the one that was missing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StopPress {
    /// Nothing is running, or the press has already been answered.
    Nothing,
    /// The ordinary case: the run has a name, so it can be cancelled now.
    Cancel { run_id: String },
    /// Still starting: remember it against the key it has, and cancel on arrival.
    CancelWhenNamed { key: String },
}

/// The run on screen, as far as Stop needs it: its phase in Electron's words and its name once it has one.
#[derive(Clone, Copy, Debug, Default)]
pub struct StoppableRun<'a> {
    pub phase: &'a str,
    pub run_id: Option<&'a str>,
}

/// The phases in which a mission is the person's to stop. The composer draws Stop for exactly these.
pub fn is_stoppable(phase: Option<&str>) -> bool {
    matches!(phase, Some("starting" | "running" | "cancelling"))
}

pub fn stop_press(run: Option<StoppableRun<'_>>, shown_key: Option<&str>) -> StopPress {
    let Some(run) = run.filter(|r| is_stoppable(Some(r.phase))) else {
        return StopPress::Nothing;
    };
    if let Some(run_id) = run.run_id {
        return StopPress::Cancel { run_id: run_id.to_owned() };
    }
    // No name yet. The key the run is filed under is the only handle there is, and without one there is nothing to
    // remember the press against.
    match shown_key {
        Some(key) => StopPress::CancelWhenNamed { key: key.to_owned() },
        None => StopPress::Nothing,
    }
}

#[cfg(test)]
#[path = "stop_press_tests.rs"]
mod tests;

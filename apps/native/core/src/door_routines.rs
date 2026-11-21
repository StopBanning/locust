//! What the door needs to run routines through the routine card's own path (index.ts's `routines`, `runRoutine`,
//! `routineSteps`, `routineLastStep`): the window's runner, offered once it is made, and the newest step each routine
//! started, kept as each one starts so a step that ended between two reads is not missed.
//!
//! No runner offered (the door with no window, `--serve-door`) is a Locust that does not offer routines to other apps.

use crate::routine_runner::RoutineRunner;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

static RUNNER: Mutex<Option<Arc<RoutineRunner>>> = Mutex::new(None);
static LAST_STEP: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

/// The window made its routine runner: the door may run routines through it.
pub fn offer(runner: Arc<RoutineRunner>) {
    *crate::invariants::lock(&RUNNER) = Some(runner);
}

/// The runner the window offered, if any.
pub fn runner() -> Option<Arc<RoutineRunner>> {
    crate::invariants::lock(&RUNNER).clone()
}

/// A routine's step started as `mission` (index.ts, on `mission-started` from a routine).
pub fn step_started(routine_id: &str, mission: &str) {
    let mut held = crate::invariants::lock(&LAST_STEP);
    let map = held.get_or_insert_with(HashMap::new);
    // A few hundred routines at most; never unbounded.
    if map.len() >= 500 && !map.contains_key(routine_id) {
        map.clear();
    }
    map.insert(routine_id.to_owned(), mission.to_owned());
}

/// The newest step `routine_id` started since Locust opened.
pub fn last_step(routine_id: &str) -> Option<String> {
    crate::invariants::lock(&LAST_STEP).as_ref()?.get(routine_id).cloned()
}

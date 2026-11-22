//! THE AGENTS' ANSWER AS THE WINDOW ASKS FOR IT (Electron `main/runtime-discovery.ts` createRuntimeDiscoveryService, and
//! the `probe.finished` notices of main's discovery log).
//!
//! A sweep starts a CLI per agent, so two asks that arrive together share one sweep, and an answer is reused for a
//! short while (ten seconds) before another is started. Whatever changed the machine -- Check again, a sign-in, an
//! install, an update, an agent's late answer -- invalidates it; a sweep that was already running when that happened
//! is not cached, and is asked once more before it answers. A sweep that could not run answers with Electron's generic
//! words, never with what went wrong inside it (a path, a CLI's own output).
//!
//! An agent whose check the sweep went on without is noted here when it answers, so the window can read the held
//! answer again -- which has that answer in it -- without starting a new check of the agent that just answered.

use std::collections::HashSet;
use std::sync::{Arc, Condvar, Mutex, OnceLock};

use crate::agents::Found;

/// How long an answer is reused before another sweep is started (`cacheTtlMs`).
pub const CACHE_TTL_MS: u64 = 10_000;

/// discoveryFailed(): what a sweep that could not run says, whatever the reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryFailed {
    pub code: &'static str,
    pub message: &'static str,
}

pub const DISCOVERY_FAILED: DiscoveryFailed =
    DiscoveryFailed { code: "DISCOVERY_FAILED", message: "Local runtime discovery could not complete." };

pub type Answered = Result<Vec<Found>, DiscoveryFailed>;

/// One sweep being built, shared by everyone who asks while it runs.
struct Building {
    done: Mutex<Option<Answered>>,
    cv: Condvar,
}

impl Building {
    fn wait(&self) -> Answered {
        let mut done = crate::invariants::lock(&self.done);
        while done.is_none() {
            done = self.cv.wait(done).unwrap_or_else(|e| e.into_inner());
        }
        done.clone().unwrap_or(Err(DISCOVERY_FAILED))
    }
}

#[derive(Default)]
struct Held {
    cached: Option<(u64, Answered)>,
    building: Option<Arc<Building>>,
    /// Bumped by `invalidate`, so a build that was already running knows it was.
    generation: u64,
}

/// What the service is built on: the sweep, the clock, and the held answer a late check is patched into.
pub struct Seams {
    pub probe: Box<dyn Fn() -> Vec<Found> + Send + Sync>,
    pub now: Box<dyn Fn() -> u64 + Send + Sync>,
    pub ttl_ms: u64,
    /// The held answer as it stands, late answers in it: a plain read, which starts no check.
    pub held: Box<dyn Fn() -> Option<Vec<Found>> + Send + Sync>,
}

pub struct DiscoveryService {
    seams: Seams,
    state: Mutex<Held>,
    /// Agents whose late check has finished, not yet read by the window.
    finished: Mutex<HashSet<String>>,
}

impl DiscoveryService {
    pub fn new(seams: Seams) -> Self {
        DiscoveryService { seams, state: Mutex::new(Held::default()), finished: Mutex::new(HashSet::new()) }
    }

    fn probe(&self) -> Answered {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (self.seams.probe)())).map_err(|_| DISCOVERY_FAILED)
    }

    /// The answer: the short-lived cached one, the sweep already running, or a new sweep.
    pub fn get(&self) -> Answered {
        let (building, generation_at_start) = {
            let mut state = crate::invariants::lock(&self.state);
            if let Some((expires, answer)) = &state.cached
                && (self.seams.now)() < *expires
            {
                return answer.clone();
            }
            if let Some(building) = &state.building {
                let building = building.clone();
                drop(state);
                return building.wait();
            }
            let building = Arc::new(Building { done: Mutex::new(None), cv: Condvar::new() });
            state.building = Some(building.clone());
            (building, state.generation)
        };
        let mut answer = self.probe();
        // Invalidated while it ran: asked once more, which is cheap -- the held answer, with the late one in it.
        if crate::invariants::lock(&self.state).generation != generation_at_start {
            answer = self.probe();
        }
        {
            let mut state = crate::invariants::lock(&self.state);
            if state.generation == generation_at_start {
                state.cached = Some(((self.seams.now)().saturating_add(self.seams.ttl_ms), answer.clone()));
            }
            if state.building.as_ref().is_some_and(|b| Arc::ptr_eq(b, &building)) {
                state.building = None;
            }
        }
        *crate::invariants::lock(&building.done) = Some(answer.clone());
        building.cv.notify_all();
        answer
    }

    /// Something changed: the next ask starts its own sweep, and one already running is not kept.
    pub fn invalidate(&self) {
        let mut state = crate::invariants::lock(&self.state);
        state.cached = None;
        state.building = None;
        state.generation += 1;
    }

    /// An agent the sweep went on without has answered: what is held is stale, and the window is told.
    pub fn answered_late(&self, id: &str) {
        self.invalidate();
        crate::invariants::lock(&self.finished).insert(id.to_owned());
    }

    /// The agents whose late checks finished since the window last looked, each told once.
    pub fn take_finished(&self) -> HashSet<String> {
        std::mem::take(&mut *crate::invariants::lock(&self.finished))
    }

    /// The held answer, read again: no agent is asked anything.
    pub fn held(&self) -> Option<Vec<Found>> {
        (self.seams.held)()
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// This process's service, on the real sweep.
pub fn shared() -> &'static DiscoveryService {
    static SHARED: OnceLock<DiscoveryService> = OnceLock::new();
    SHARED.get_or_init(|| {
        DiscoveryService::new(Seams {
            probe: Box::new(crate::agents::discover),
            now: Box::new(now_ms),
            ttl_ms: CACHE_TTL_MS,
            held: Box::new(crate::agents::held_answer),
        })
    })
}

#[cfg(test)]
#[path = "discovery_service_tests.rs"]
mod tests;

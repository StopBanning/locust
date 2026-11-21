//! A start asks only its own runtime (Electron `main/a-start-asks-only-its-runtime.test.ts`).
//!
//! Past five minutes, starting a run swept EVERY runtime and waited for the slowest -- 3-6 s before the first message
//! after a break. The start needs one answer.

use crate::agents::{Auth, Found, HeldAnswer, Probe, READY_TO_START_TTL_MS, StartReadiness, StartReadinessOptions};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

const TTL: u64 = 5 * 60_000;

fn runtime(id: &str, status: Probe) -> Found {
    Found {
        id: id.into(),
        display_name: id.into(),
        installed: true,
        version: None,
        auth: if status == Probe::Ready { Auth::Authenticated } else { Auth::Unauthenticated },
        ready: status == Probe::Ready,
        status,
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }
}

/// What a runtime asked alone answers: ready, and marked so a test can tell it from the held one.
fn fresh(id: &str) -> Found {
    Found { version: Some("asked alone".into()), ..runtime(id, Probe::Ready) }
}

struct Harness {
    readiness: StartReadiness,
    held: Arc<Mutex<Option<HeldAnswer>>>,
    sweeps: Arc<AtomicUsize>,
    asked: Arc<Mutex<Vec<String>>>,
    fail_next: Arc<AtomicUsize>,
}

fn harness(clock: u64) -> Harness {
    let held = Arc::new(Mutex::new(Some(HeldAnswer {
        at: 0,
        value: vec![runtime("claude", Probe::Ready), runtime("codex", Probe::Ready), runtime("opencode", Probe::Ready)],
    })));
    let sweeps = Arc::new(AtomicUsize::new(0));
    let asked = Arc::new(Mutex::new(Vec::new()));
    let fail_next = Arc::new(AtomicUsize::new(0));
    let now = Arc::new(AtomicU64::new(clock));
    let readiness = StartReadiness::new(StartReadinessOptions {
        ttl_ms: TTL,
        now: Box::new(move || now.load(Ordering::SeqCst)),
        held: Box::new({
            let held = held.clone();
            move || held.lock().unwrap().clone()
        }),
        store: Box::new({
            let held = held.clone();
            move |value| {
                let mut h = held.lock().unwrap();
                let at = h.as_ref().map_or(0, |h| h.at);
                *h = Some(HeldAnswer { at, value });
            }
        }),
        sweep: Box::new({
            let (held, sweeps) = (held.clone(), sweeps.clone());
            move || {
                sweeps.fetch_add(1, Ordering::SeqCst);
                held.lock().unwrap().as_ref().map(|h| h.value.clone()).unwrap_or_default()
            }
        }),
        ask_one: Box::new({
            let (asked, fail_next) = (asked.clone(), fail_next.clone());
            move |id| {
                asked.lock().unwrap().push(id.to_owned());
                if fail_next.swap(0, Ordering::SeqCst) == 1 {
                    return Err("probe blew up".into());
                }
                Ok(Some(fresh(id)))
            }
        }),
        sweep_only: ["antigravity".to_owned()].into_iter().collect(),
    });
    let first = held.lock().unwrap().clone().unwrap();
    readiness.swept(&first.value, 0);
    Harness { readiness, held, sweeps, asked, fail_next }
}

impl Harness {
    fn set_held(&self, value: Vec<Found>) {
        *self.held.lock().unwrap() = Some(HeldAnswer { at: 0, value });
    }
    fn asked(&self) -> Vec<String> {
        self.asked.lock().unwrap().clone()
    }
    fn sweeps(&self) -> usize {
        self.sweeps.load(Ordering::SeqCst)
    }
}

#[test]
fn the_start_trusts_a_ready_answer_for_five_minutes() {
    assert_eq!(READY_TO_START_TTL_MS, TTL);
}

// case: asks nothing while the last answer is fresh
#[test]
fn asks_nothing_while_the_last_answer_is_fresh() {
    let h = harness(60_000);
    let answer = h.readiness.for_start(Some("claude"));
    assert_eq!(h.sweeps(), 0);
    assert!(h.asked().is_empty());
    assert_eq!(answer.len(), 3);
    // One millisecond short of the limit is still fresh.
    let h = harness(TTL - 1);
    h.readiness.for_start(Some("claude"));
    assert_eq!((h.sweeps(), h.asked().len()), (0, 0));
}

// case: asks the starting runtime ALONE once the answer is stale
#[test]
fn asks_the_starting_runtime_alone_once_the_answer_is_stale() {
    let h = harness(TTL + 1);
    let answer = h.readiness.for_start(Some("claude"));
    assert_eq!(h.asked(), vec!["claude".to_owned()]);
    assert_eq!(h.sweeps(), 0);
    // The fresh answer replaces that runtime's alone, and is what the start gets and what is kept.
    assert_eq!(answer.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["claude", "codex", "opencode"]);
    assert_eq!(answer[0], fresh("claude"));
    assert_eq!(answer[1], runtime("codex", Probe::Ready));
    assert_eq!(answer[2], runtime("opencode", Probe::Ready));
    let kept = h.held.lock().unwrap().clone().unwrap();
    assert_eq!(kept.value, answer);
    // The sweep's own clock is left alone: the other runtimes were not asked.
    assert_eq!(kept.at, 0);
    // And that answer is fresh now: the next start on it asks nothing.
    h.readiness.for_start(Some("claude"));
    assert_eq!(h.asked().len(), 1);
    // A DIFFERENT runtime is still stale -- asking one did not vouch for all.
    h.readiness.for_start(Some("codex"));
    assert_eq!(h.asked().last().map(String::as_str), Some("codex"));
    assert_eq!(h.asked().len(), 2);
    assert_eq!(h.sweeps(), 0);
}

// case: still sweeps for a runtime that was not ready, and for Antigravity
#[test]
fn still_sweeps_for_a_runtime_that_was_not_ready_and_for_antigravity() {
    // The controls. A runtime that was signed out needs the full path (the screens learn from it too), and
    // Antigravity is not a CLI to ask alone.
    let h = harness(TTL + 1);
    h.set_held(vec![runtime("claude", Probe::AuthRequired), runtime("antigravity", Probe::Ready)]);
    h.readiness.for_start(Some("claude"));
    h.readiness.for_start(Some("antigravity"));
    assert_eq!(h.sweeps(), 2);
    assert!(h.asked().is_empty());
}

// case: falls back to the full sweep when asking alone fails
#[test]
fn falls_back_to_the_full_sweep_when_asking_alone_fails() {
    let h = harness(TTL + 1);
    h.fail_next.store(1, Ordering::SeqCst);
    h.readiness.for_start(Some("opencode"));
    assert_eq!(h.asked(), vec!["opencode".to_owned()]);
    assert_eq!(h.sweeps(), 1);
}

#[test]
fn a_start_with_no_runtime_or_no_answer_yet_sweeps() {
    let h = harness(60_000);
    h.readiness.for_start(None);
    assert_eq!(h.sweeps(), 1);
    *h.held.lock().unwrap() = None;
    h.readiness.for_start(Some("claude"));
    assert_eq!(h.sweeps(), 2);
    assert!(h.asked().is_empty());
}

#[test]
fn a_runtime_a_sweep_answered_later_is_fresh_from_that_answer_not_from_the_held_clock() {
    // index.ts stamps one runtime's own late answer (`startReadiness.swept([record], Date.now())`) after the sweep's
    // clock: that runtime is fresh from then, though the held answer is older than the limit.
    let h = harness(TTL + 1);
    let value = h.held.lock().unwrap().clone().unwrap().value;
    h.readiness.swept(&value[..1], TTL);
    h.readiness.for_start(Some("claude"));
    assert_eq!((h.sweeps(), h.asked().len()), (0, 0));
    // Another runtime, not stamped, is still stale.
    h.readiness.for_start(Some("codex"));
    assert_eq!(h.asked(), vec!["codex".to_owned()]);
}

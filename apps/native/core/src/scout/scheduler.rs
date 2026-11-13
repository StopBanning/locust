//! Timer scheduler abstraction for Scout: real background timer thread for production
//! and deterministic virtual clock + scheduler for tests.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub trait ScoutScheduler: Send + Sync {
    fn schedule(&self, interval_ms: u64, task: Box<dyn FnMut() + Send + 'static>) -> u64;
    fn clear(&self, handle: u64);
    fn later(&self, delay_ms: u64, task: Box<dyn FnOnce() + Send + 'static>) -> u64;
    fn cancel(&self, handle: u64);
    fn retry_sleep(&self, duration: Duration);
}

enum TaskKind {
    Interval {
        every: Duration,
        cb: Box<dyn FnMut() + Send + 'static>,
    },
    Once {
        cb: Option<Box<dyn FnOnce() + Send + 'static>>,
    },
}

struct RealEntry {
    due: Instant,
    kind: TaskKind,
}

struct RealState {
    entries: BTreeMap<u64, RealEntry>,
    stopped: bool,
}

pub struct RealScheduler {
    next_id: AtomicU64,
    shared: Arc<(Mutex<RealState>, Condvar)>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl RealScheduler {
    pub fn new() -> Arc<Self> {
        let shared = Arc::new((
            Mutex::new(RealState {
                entries: BTreeMap::new(),
                stopped: false,
            }),
            Condvar::new(),
        ));
        let bg = shared.clone();
        let handle = std::thread::spawn(move || loop {
            let mut due_ids = Vec::new();
            {
                let (lock, cv) = &*bg;
                let mut st = lock.lock().unwrap_or_else(|e| e.into_inner());
                loop {
                    if st.stopped {
                        return;
                    }
                    let check_now = Instant::now();
                    due_ids.clear();
                    let mut earliest: Option<Instant> = None;
                    for (&id, entry) in &st.entries {
                        if entry.due <= check_now {
                            due_ids.push(id);
                        } else {
                            earliest = Some(earliest.map_or(entry.due, |e| e.min(entry.due)));
                        }
                    }
                    if !due_ids.is_empty() {
                        break;
                    }
                    match earliest {
                        Some(t) => {
                            let wait = t.saturating_duration_since(check_now);
                            let (guard, _) = cv
                                .wait_timeout(st, wait)
                                .unwrap_or_else(|e| e.into_inner());
                            st = guard;
                        }
                        None => {
                            st = cv.wait(st).unwrap_or_else(|e| e.into_inner());
                        }
                    }
                }
            }
            // Pop callbacks outside the lock so callbacks can freely schedule/cancel.
            let mut to_run: Vec<Box<dyn FnOnce() + Send>> = Vec::new();
            {
                let (lock, _) = &*bg;
                let mut st = lock.lock().unwrap_or_else(|e| e.into_inner());
                if st.stopped {
                    return;
                }
                for id in due_ids {
                    if let Some(mut entry) = st.entries.remove(&id) {
                        match &mut entry.kind {
                            TaskKind::Once { cb } => {
                                if let Some(f) = cb.take() {
                                    to_run.push(f);
                                }
                            }
                            TaskKind::Interval { every, cb } => {
                                cb();
                                let step = (*every).max(Duration::from_millis(1));
                                entry.due = Instant::now() + step;
                                st.entries.insert(id, entry);
                            }
                        }
                    }
                }
            }
            for f in to_run {
                f();
            }
        });
        Arc::new(Self {
            next_id: AtomicU64::new(1),
            shared,
            worker: Mutex::new(Some(handle)),
        })
    }

    pub fn shutdown(&self) {
        let (lock, cv) = &*self.shared;
        {
            let mut st = lock.lock().unwrap_or_else(|e| e.into_inner());
            st.stopped = true;
            st.entries.clear();
            cv.notify_all();
        }
        if let Some(h) = self
            .worker
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            let _ = h.join();
        }
    }
}

impl Drop for RealScheduler {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl ScoutScheduler for RealScheduler {
    fn schedule(&self, interval_ms: u64, task: Box<dyn FnMut() + Send + 'static>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let step = Duration::from_millis(interval_ms.max(1));
        let (lock, cv) = &*self.shared;
        let mut st = lock.lock().unwrap_or_else(|e| e.into_inner());
        if !st.stopped {
            st.entries.insert(
                id,
                RealEntry {
                    due: Instant::now() + step,
                    kind: TaskKind::Interval {
                        every: step,
                        cb: task,
                    },
                },
            );
            cv.notify_all();
        }
        id
    }

    fn clear(&self, handle: u64) {
        let (lock, cv) = &*self.shared;
        let mut st = lock.lock().unwrap_or_else(|e| e.into_inner());
        st.entries.remove(&handle);
        cv.notify_all();
    }

    fn later(&self, delay_ms: u64, task: Box<dyn FnOnce() + Send + 'static>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let delay = Duration::from_millis(delay_ms);
        let (lock, cv) = &*self.shared;
        let mut st = lock.lock().unwrap_or_else(|e| e.into_inner());
        if !st.stopped {
            st.entries.insert(
                id,
                RealEntry {
                    due: Instant::now() + delay,
                    kind: TaskKind::Once { cb: Some(task) },
                },
            );
            cv.notify_all();
        }
        id
    }

    fn cancel(&self, handle: u64) {
        self.clear(handle);
    }

    fn retry_sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

struct FakeEntry {
    id: u64,
    due_ms: i64,
    kind: FakeKind,
}

enum FakeKind {
    Interval {
        every_ms: i64,
        cb: Box<dyn FnMut() + Send + 'static>,
    },
    Once {
        cb: Option<Box<dyn FnOnce() + Send + 'static>>,
    },
}

/// Deterministic virtual clock + timer queue for tests (analogous to Vitest fake timers).
pub struct FakeScheduler {
    now_ms: AtomicI64,
    next_id: AtomicU64,
    stopped: AtomicBool,
    entries: Mutex<BTreeMap<u64, FakeEntry>>,
}

impl FakeScheduler {
    pub fn new(initial_ms: i64) -> Arc<Self> {
        Arc::new(Self {
            now_ms: AtomicI64::new(initial_ms),
            next_id: AtomicU64::new(1),
            stopped: AtomicBool::new(false),
            entries: Mutex::new(BTreeMap::new()),
        })
    }

    pub fn now_ms(&self) -> i64 {
        self.now_ms.load(Ordering::SeqCst)
    }

    pub fn set_now_ms(&self, ms: i64) {
        self.now_ms.store(ms, Ordering::SeqCst);
    }

    /// Advances the virtual clock by `ms` milliseconds, firing all due timers in chronological order.
    pub fn advance_ms(&self, ms: u64) {
        let target = self.now_ms.load(Ordering::SeqCst) + ms as i64;
        loop {
            if self.stopped.load(Ordering::SeqCst) {
                self.now_ms.store(target, Ordering::SeqCst);
                return;
            }
            let next = {
                let mut map = self.entries.lock().unwrap_or_else(|e| e.into_inner());
                let earliest_key = map
                    .iter()
                    .filter(|(_, e)| e.due_ms <= target)
                    .min_by_key(|(id, e)| (e.due_ms, **id))
                    .map(|(id, _)| *id);
                earliest_key.and_then(|id| map.remove(&id))
            };
            let Some(mut entry) = next else {
                self.now_ms.store(target, Ordering::SeqCst);
                return;
            };
            self.now_ms.store(entry.due_ms.max(self.now_ms.load(Ordering::SeqCst)), Ordering::SeqCst);
            match &mut entry.kind {
                FakeKind::Once { cb } => {
                    if let Some(f) = cb.take() {
                        f();
                    }
                }
                FakeKind::Interval { every_ms, cb } => {
                    cb();
                    entry.due_ms += *every_ms;
                    if !self.stopped.load(Ordering::SeqCst) {
                        self.entries
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .insert(entry.id, entry);
                    }
                }
            }
        }
    }

    pub fn pending_count(&self) -> usize {
        self.entries.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}

impl ScoutScheduler for FakeScheduler {
    fn schedule(&self, interval_ms: u64, task: Box<dyn FnMut() + Send + 'static>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let every = (interval_ms as i64).max(1);
        let due = self.now_ms.load(Ordering::SeqCst) + every;
        if !self.stopped.load(Ordering::SeqCst) {
            self.entries
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(
                    id,
                    FakeEntry {
                        id,
                        due_ms: due,
                        kind: FakeKind::Interval {
                            every_ms: every,
                            cb: task,
                        },
                    },
                );
        }
        id
    }

    fn clear(&self, handle: u64) {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&handle);
    }

    fn later(&self, delay_ms: u64, task: Box<dyn FnOnce() + Send + 'static>) -> u64 {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let due = self.now_ms.load(Ordering::SeqCst) + delay_ms as i64;
        if !self.stopped.load(Ordering::SeqCst) {
            self.entries
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(
                    id,
                    FakeEntry {
                        id,
                        due_ms: due,
                        kind: FakeKind::Once { cb: Some(task) },
                    },
                );
        }
        id
    }

    fn cancel(&self, handle: u64) {
        self.clear(handle);
    }

    fn retry_sleep(&self, _duration: Duration) {}
}

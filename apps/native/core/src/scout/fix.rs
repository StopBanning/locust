//! Scout prepares a failing check's fix in an isolated copy of the folder (`main/scout-fix.ts`).

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::compare_copies::count_copy_line_changes_with;
use crate::routine_copy::{BringIn, RoutineCopies};

use super::scheduler::{RealScheduler, ScoutScheduler};
use super::types::clip;

pub const SCOUT_FIX_ROOT_ENV: &str = "LOCUST_SCOUT_FIX_ROOT";
pub const SCOUT_FIX_TURNS: usize = 2;
pub const SCOUT_FIX_MS: u64 = 30 * 60_000;
/// A fix's copy is named so, and the command guard knows a fix's folder by it.
pub const SCOUT_FIX_PREFIX: &str = "rt_scoutfix";

pub fn scout_fix_root_from(home: Option<&Path>, env_val: Option<&str>) -> PathBuf {
    if let Some(set) = env_val.map(str::trim).filter(|s| !s.is_empty()) {
        return PathBuf::from(set);
    }
    home.unwrap_or_else(|| Path::new("."))
        .join(".locust")
        .join("scout-fixes")
}

/// Where fixes are prepared: `~/.locust/scout-fixes`, or `LOCUST_SCOUT_FIX_ROOT` for a drive's own profile.
pub fn scout_fix_root() -> PathBuf {
    let env_val = std::env::var(SCOUT_FIX_ROOT_ENV).ok();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    scout_fix_root_from(home.as_deref(), env_val.as_deref())
}

pub fn new_fix_id() -> String {
    let raw = crate::invariants::unique_nanos() & 0xffff_ffff_ffff;
    format!("{SCOUT_FIX_PREFIX}{raw:012x}")
}

pub fn is_fix_id(fix_id: &str) -> bool {
    let Some(rest) = fix_id.strip_prefix(SCOUT_FIX_PREFIX) else {
        return false;
    };
    rest.len() == 12
        && rest
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn indent(lines: &[String]) -> Vec<String> {
    if lines.is_empty() {
        vec!["    (it printed nothing)".to_owned()]
    } else {
        lines.iter().map(|line| format!("    {line}")).collect()
    }
}

/// The first turn: what Scout found, where the teammate is, and what it may not do.
pub fn scout_fix_prompt(command: &str, lines: &[String]) -> String {
    let mut parts = vec![
        "Scout found this folder’s check failing and asked you to prepare a fix before the person looks. You are working in a copy of their folder: nothing you change reaches it until they press Apply, and opening a pull request stays their own step.".to_owned(),
        String::new(),
        format!("The check `{command}` fails. What it printed:"),
        String::new(),
    ];
    parts.extend(indent(lines));
    parts.push(String::new());
    parts.push(
        "Fix the cause, then say in one or two sentences what you changed. Do not weaken, skip or delete tests, or change the check itself, to make it pass. Do not push, add a remote, or use gh: none of that is allowed here. If you cannot fix it from what is here, change nothing and say exactly what is missing. Locust runs the check again after your turn.".to_owned(),
    );
    parts.join("\n")
}

/// A later turn: the check still fails after the last one.
pub fn scout_fix_again_prompt(command: &str, lines: &[String], turn: usize, of: usize) -> String {
    let mut parts = vec![
        format!("The check `{command}` still fails after your change (try {turn} of {of}). What it printed:"),
        String::new(),
    ];
    parts.extend(indent(lines));
    parts.push(String::new());
    parts.push(
        "Fix what is left, the same way: no weakened tests, no change to the check, no push, no gh. If you cannot, change nothing more and say what is missing.".to_owned(),
    );
    parts.join("\n")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScoutFixOutcome {
    Ready {
        files: Vec<String>,
        added: Option<usize>,
        removed: Option<usize>,
        mission_id: String,
    },
    Unfixed {
        why: String,
        mission_id: Option<String>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FixTurnPhase {
    Completed,
    Failed,
    Cancelled,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixTurnEnd {
    pub phase: FixTurnPhase,
    pub said: Option<String>,
    pub failed: Option<String>,
    pub asked: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixCheckResult {
    pub passed: bool,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct FixStartRequest<'a> {
    pub fix_id: &'a str,
    pub teammate_id: &'a str,
    pub prompt: &'a str,
    pub cwd: &'a Path,
    pub follow_up_of: Option<&'a str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixStarted {
    pub mission_id: String,
    pub run_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixPrepareInput {
    pub fix_id: String,
    pub teammate_id: String,
    pub name: String,
    pub command: String,
    pub folder: PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FixApplyResult {
    Brought(Vec<String>),
    YourChanges(Vec<String>),
    Nothing,
    Gone,
}

pub fn default_fix_git(args: &[&str], cwd: &Path) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("git could not run: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        Err(format!("{stdout}{stderr}"))
    }
}

pub trait ScoutFixDeps: Send + Sync {
    fn root(&self) -> PathBuf {
        scout_fix_root()
    }
    fn copies(&self) -> RoutineCopies {
        RoutineCopies::new(self.root())
    }
    fn run_check(&self, command: &str, cwd: &Path) -> FixCheckResult;
    fn start(&self, input: FixStartRequest<'_>) -> Result<FixStarted, String>;
    fn stop(&self, run_id: &str);
    fn end_of(&self, mission_id: &str) -> FixTurnEnd;
    fn git(&self, args: &[&str], cwd: &Path) -> Result<String, String> {
        default_fix_git(args, cwd)
    }
    fn now_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    }
    fn fix_ms(&self) -> u64 {
        SCOUT_FIX_MS
    }
}

type WakeSignal = Arc<(Mutex<Option<bool>>, Condvar)>;

struct FixRun {
    fix_id: String,
    run_id: Option<String>,
    stopped: Option<String>,
    wakes: Vec<WakeSignal>,
}

struct FixEngineState {
    running: Option<FixRun>,
    waiting: HashMap<String, WakeSignal>,
    ended_early: HashSet<String>,
    ended_early_order: VecDeque<String>,
}

impl FixEngineState {
    fn new() -> Self {
        Self {
            running: None,
            waiting: HashMap::new(),
            ended_early: HashSet::new(),
            ended_early_order: VecDeque::new(),
        }
    }
}

fn wake_with(sig: &WakeSignal, ended: bool) {
    let (lock, cv) = &**sig;
    let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_none() {
        *guard = Some(ended);
        cv.notify_all();
    }
}

struct RunningGuard<'a> {
    fixes: &'a ScoutFixes,
    fix_id: String,
}

impl Drop for RunningGuard<'_> {
    fn drop(&mut self) {
        let mut st = self.fixes.state.lock().unwrap_or_else(|e| e.into_inner());
        if st
            .running
            .as_ref()
            .is_some_and(|run| run.fix_id == self.fix_id)
        {
            st.running = None;
        }
    }
}

/// Runs Scout check-fix attempts in isolated routine copies with strict turn and time limits.
pub struct ScoutFixes {
    deps: Arc<dyn ScoutFixDeps>,
    scheduler: Arc<dyn ScoutScheduler>,
    state: Mutex<FixEngineState>,
}

impl ScoutFixes {
    pub fn new(deps: Arc<dyn ScoutFixDeps>) -> Arc<Self> {
        Self::with_scheduler(deps, RealScheduler::new())
    }

    pub fn with_scheduler(
        deps: Arc<dyn ScoutFixDeps>,
        scheduler: Arc<dyn ScoutScheduler>,
    ) -> Arc<Self> {
        Arc::new(Self {
            deps,
            scheduler,
            state: Mutex::new(FixEngineState::new()),
        })
    }

    fn until_ended(&self, mission_id: &str, ms: i64) -> bool {
        let signal: WakeSignal = Arc::new((Mutex::new(None), Condvar::new()));
        {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if st.ended_early.remove(mission_id) {
                return true;
            }
            if st
                .running
                .as_ref()
                .and_then(|r| r.stopped.as_ref())
                .is_some()
            {
                return false;
            }
            st.waiting.insert(mission_id.to_owned(), signal.clone());
            if let Some(run) = st.running.as_mut() {
                run.wakes.push(signal.clone());
            }
        }
        let timer_sig = signal.clone();
        let delay = ms.max(0) as u64;
        let timer_handle = self.scheduler.later(
            delay,
            Box::new(move || {
                wake_with(&timer_sig, false);
            }),
        );
        let ended = {
            let (lock, cv) = &*signal;
            let mut guard = lock.lock().unwrap_or_else(|e| e.into_inner());
            while guard.is_none() {
                guard = cv.wait(guard).unwrap_or_else(|e| e.into_inner());
            }
            guard.unwrap_or(false)
        };
        self.scheduler.cancel(timer_handle);
        {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            st.waiting.remove(mission_id);
            if let Some(run) = st.running.as_mut() {
                run.wakes.retain(|w| !Arc::ptr_eq(w, &signal));
            }
        }
        ended
    }

    fn stopped_reason(&self, fix_id: &str) -> Option<String> {
        let st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        st.running
            .as_ref()
            .filter(|r| r.fix_id == fix_id)
            .and_then(|r| r.stopped.clone())
    }

    fn line_counts(&self, fix_id: &str, folder: &Path) -> (Option<usize>, Option<usize>) {
        let root = self.deps.root();
        let copies = self.deps.copies();
        let Ok(copy) = copies.path(fix_id) else {
            return (None, None);
        };
        let Ok(changes) = copies.changes(fix_id) else {
            return (None, None);
        };
        let Ok((_, added, removed)) =
            count_copy_line_changes_with(folder, &copy, &root, &changes, |args, cwd| {
                self.deps.git(args, cwd)
            })
        else {
            return (None, None);
        };
        match added {
            Some(a) => (Some(a), Some(removed.unwrap_or(0))),
            None => (None, None),
        }
    }

    pub fn prepare(&self, input: &FixPrepareInput) -> ScoutFixOutcome {
        if !is_fix_id(&input.fix_id) {
            return ScoutFixOutcome::Unfixed {
                why: "That fix cannot be named so.".into(),
                mission_id: None,
            };
        }
        {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if st.running.is_some() {
                return ScoutFixOutcome::Unfixed {
                    why: "Scout is preparing another fix.".into(),
                    mission_id: None,
                };
            }
            st.running = Some(FixRun {
                fix_id: input.fix_id.clone(),
                run_id: None,
                stopped: None,
                wakes: Vec::new(),
            });
        }
        let _guard = RunningGuard {
            fixes: self,
            fix_id: input.fix_id.clone(),
        };
        let fix_ms = self.deps.fix_ms();
        let deadline = self.deps.now_ms() + fix_ms as i64;
        let copies = self.deps.copies();

        let copy = match copies.make(&input.fix_id, &input.folder) {
            Ok(path) => path,
            Err(why) => {
                return ScoutFixOutcome::Unfixed {
                    why: if why.trim().is_empty() {
                        "The folder could not be copied.".into()
                    } else {
                        why
                    },
                    mission_id: None,
                };
            }
        };

        // Its own repository, with no remote: git never walks up out of the copy.
        let _ = self.deps.git(&["init", "-q"], &copy);

        // The check fails here too, or there is nothing to fix: a check that passes in a fresh
        // copy reads something outside the folder, or failed once by chance.
        let mut check = self.deps.run_check(&input.command, &copy);
        if check.passed {
            return ScoutFixOutcome::Unfixed {
                why: format!(
                    "The check passes in a fresh copy of the folder, so there was nothing for {} to fix. It may depend on something outside the folder, or have failed once by chance.",
                    input.name
                ),
                mission_id: None,
            };
        }

        let mut last: Option<String> = None;
        for turn in 1..=SCOUT_FIX_TURNS {
            if let Some(why) = self.stopped_reason(&input.fix_id) {
                return ScoutFixOutcome::Unfixed {
                    why,
                    mission_id: last,
                };
            }
            let prompt = if turn == 1 {
                scout_fix_prompt(&input.command, &check.lines)
            } else {
                scout_fix_again_prompt(&input.command, &check.lines, turn, SCOUT_FIX_TURNS)
            };
            let started = match self.deps.start(FixStartRequest {
                fix_id: &input.fix_id,
                teammate_id: &input.teammate_id,
                prompt: &prompt,
                cwd: &copy,
                follow_up_of: last.as_deref(),
            }) {
                Ok(started) => started,
                Err(refused) => {
                    return ScoutFixOutcome::Unfixed {
                        why: format!("{} could not start: {refused}", input.name),
                        mission_id: last,
                    };
                }
            };
            last = Some(started.mission_id.clone());
            {
                let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(run) = st
                    .running
                    .as_mut()
                    .filter(|r| r.fix_id == input.fix_id)
                {
                    run.run_id = Some(started.run_id.clone());
                }
            }
            let in_time = self.until_ended(&started.mission_id, deadline - self.deps.now_ms());
            if !in_time {
                self.deps.stop(&started.run_id);
                let mins = ((fix_ms as f64) / 60_000.0).round() as u64;
                let why = self.stopped_reason(&input.fix_id).unwrap_or_else(|| {
                    format!("It ran past {mins} minutes, so Locust stopped it.")
                });
                return ScoutFixOutcome::Unfixed {
                    why,
                    mission_id: last,
                };
            }
            let end = self.deps.end_of(&started.mission_id);
            let words = end
                .said
                .as_deref()
                .map(|s| clip(s, 240))
                .filter(|s| !s.is_empty());
            match end.phase {
                FixTurnPhase::Failed => {
                    let why = match end.failed.as_deref() {
                        Some(msg) => format!("{}’s turn failed: {}", input.name, clip(msg, 200)),
                        None => format!("{}’s turn failed.", input.name),
                    };
                    return ScoutFixOutcome::Unfixed {
                        why,
                        mission_id: last,
                    };
                }
                FixTurnPhase::Completed => {}
                FixTurnPhase::Cancelled | FixTurnPhase::Other => {
                    let why = self
                        .stopped_reason(&input.fix_id)
                        .unwrap_or_else(|| format!("{}’s turn was stopped.", input.name));
                    return ScoutFixOutcome::Unfixed {
                        why,
                        mission_id: last,
                    };
                }
            }
            if end.asked {
                let why = match words {
                    Some(w) => format!("{} asked something instead: “{w}”", input.name),
                    None => format!("{} asked something instead.", input.name),
                };
                return ScoutFixOutcome::Unfixed {
                    why,
                    mission_id: last,
                };
            }
            let changes = match copies.changes(&input.fix_id) {
                Ok(changes) => changes,
                Err(_) => {
                    return ScoutFixOutcome::Unfixed {
                        why: "The copy's record could not be read.".into(),
                        mission_id: last,
                    };
                }
            };
            if changes.changed.is_empty() && changes.deleted.is_empty() {
                let why = match words {
                    Some(w) => format!("{} changed nothing, and said: “{w}”", input.name),
                    None => format!("{} changed nothing.", input.name),
                };
                return ScoutFixOutcome::Unfixed {
                    why,
                    mission_id: last,
                };
            }
            check = self.deps.run_check(&input.command, &copy);
            if check.passed {
                let mut files: Vec<String> = changes
                    .changed
                    .into_iter()
                    .chain(changes.deleted)
                    .collect();
                files.sort();
                let (added, removed) = self.line_counts(&input.fix_id, &input.folder);
                return ScoutFixOutcome::Ready {
                    files,
                    added,
                    removed,
                    mission_id: last.unwrap_or(started.mission_id),
                };
            }
        }

        let first = check
            .lines
            .iter()
            .find(|line| !line.trim().is_empty())
            .map(|line| clip(line, 160));
        let why = match first {
            Some(line) => format!("The check still fails after {SCOUT_FIX_TURNS} tries: {line}"),
            None => format!("The check still fails after {SCOUT_FIX_TURNS} tries."),
        };
        ScoutFixOutcome::Unfixed {
            why,
            mission_id: last,
        }
    }

    /// Into the person's folder, beside their own edits; refused, file by file, where they changed
    /// the same files since.
    pub fn apply(&self, fix_id: &str) -> FixApplyResult {
        if !is_fix_id(fix_id) {
            return FixApplyResult::Gone;
        }
        let copies = self.deps.copies();
        if copies.source(fix_id).is_none() {
            return FixApplyResult::Gone;
        }
        let Ok(brought) = copies.keep(fix_id) else {
            return FixApplyResult::Gone;
        };
        match brought {
            BringIn::Brought(files) => {
                // Special case (`scout-fix.ts:205`): remove the fix copy only after its changes land cleanly.
                let _ = copies.discard(fix_id);
                FixApplyResult::Brought(files)
            }
            BringIn::YourChanges(files) => FixApplyResult::YourChanges(files),
            BringIn::Nothing => FixApplyResult::Nothing,
        }
    }

    pub fn discard(&self, fix_id: &str) {
        if is_fix_id(fix_id) {
            let _ = self.deps.copies().discard(fix_id);
        }
    }

    /// Scout turned off, or Locust closing: the fix under way stops, and says why.
    pub fn stop(&self, why: &str) {
        let (run_id, wakes) = {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let Some(run) = st.running.as_mut() else {
                return;
            };
            run.stopped = Some(why.to_owned());
            (run.run_id.clone(), run.wakes.clone())
        };
        if let Some(id) = run_id {
            self.deps.stop(&id);
        }
        for sig in wakes {
            wake_with(&sig, false);
        }
    }

    pub fn busy(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .running
            .is_some()
    }

    /// Every run's end: a fix's turn is waited on here.
    pub fn on_run_ended(&self, mission_id: &str) {
        let wake = {
            let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(sig) = st.waiting.get(mission_id).cloned() {
                Some(sig)
            } else {
                if st.running.is_some() && st.ended_early.insert(mission_id.to_owned()) {
                    st.ended_early_order.push_back(mission_id.to_owned());
                    if st.ended_early_order.len() > 20 {
                        if let Some(oldest) = st.ended_early_order.pop_front() {
                            st.ended_early.remove(&oldest);
                        }
                    }
                }
                None
            }
        };
        if let Some(sig) = wake {
            wake_with(&sig, true);
        }
    }
}

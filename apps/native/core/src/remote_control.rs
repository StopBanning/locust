//! Claude Code's Remote Control, owned by one Settings switch (Electron `main/claude-remote-control.ts` and
//! `shared/claude-remote-control.ts`). While the switch is on, one `claude remote-control --spawn worktree --name Locust`
//! runs in the folder, so the person can start sessions on this computer from claude.ai. It ends when the switch goes
//! off or Locust quits. What Claude Code prints is kept as it printed it, the first 64 KiB of each stream. There is no
//! saved opt-in and no retry.
//!
//! Never answer a trust or enable question: when Claude Code asks one, the server is ended, and once it has closed, a
//! terminal opens in the folder so the person can answer it themselves and turn the switch on again.
//!
//! The process is reached only through `Launcher`, which reports what the process does through `Events`. Tests drive
//! those events by hand; nothing here runs `claude` in a test.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, Weak};
use std::time::{Duration, Instant};

pub const REMOTE_CONTROL_ARGS: [&str; 5] = ["remote-control", "--spawn", "worktree", "--name", "Locust"];
pub const REMOTE_CONTROL_LABEL: &str = "Let me start sessions on this computer from claude.ai";
pub const NOT_INSTALLED: &str = "Claude Code is not installed here. Settings > AI agents shows how to add it.";
pub const DID_NOT_END: &str = "Claude Code did not report that its Remote Control process ended.";
/// What Claude Code prints that only the person may answer.
const NEEDS_PERSON: &str = r"(?i)Trust [^\n]*\?\s*\[y/N\]|Workspace not trusted|Quick safety check|trust this folder|Do you trust|Enable Remote Control\?\s*\(y/n\)";
const LIMIT: usize = 64 * 1024;
const WATCHED: usize = 8192;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Off,
    Starting,
    Running,
    Stopping,
    Ended,
    NeedsPerson,
    Error,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RemoteControlState {
    pub enabled: bool,
    pub phase: Phase,
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
    pub truncated: bool,
    /// Set once the process closed: its exit code, or None when a signal ended it.
    pub exit_code: Option<Option<i32>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

/// The process the switch owns.
pub trait Owned: Send {
    fn pid(&self) -> u32;
    /// End the owned process and everything it started.
    fn kill_tree(&self);
}

/// How the switch reaches Claude Code and the person's terminal.
pub trait Launcher: Send + Sync {
    /// Claude Code's program, or None when it is not installed. An error is shown as it was said.
    fn discover(&self) -> Result<Option<PathBuf>, String>;
    /// Start `program args` in `cwd`, its output piped and its input closed, in a process group of its own; report
    /// what it does through `events`, always from another thread: `spawn` is called while the switch's state is held,
    /// so a report made from inside it would wait forever.
    fn spawn(&self, program: &Path, args: &[&str], cwd: &Path, events: Events) -> Result<Box<dyn Owned>, String>;
    /// A terminal in the folder running Claude Code, for the person to sign in or trust the folder themselves.
    fn open_setup(&self, program: &Path, cwd: &Path) -> Result<(), String>;
}

#[derive(Default)]
struct Inner {
    state: RemoteControlState,
    disposed: bool,
    generation: u64,
    child: Option<Box<dyn Owned>>,
    /// The process's generation, so a late event from an ended one changes nothing.
    child_generation: u64,
    asks: bool,
    setup_started: bool,
    watched: String,
    pending: [Vec<u8>; 2],
    stopping: bool,
}

struct Shared {
    inner: Mutex<Inner>,
    closed: Condvar,
    launcher: Arc<dyn Launcher>,
    folder: Box<dyn Fn() -> PathBuf + Send + Sync>,
    stop_deadline: Duration,
}

/// What the owned process did, reported by the launcher (or a test).
#[derive(Clone)]
pub struct Events {
    shared: Weak<Shared>,
    generation: u64,
}

#[derive(Clone)]
pub struct RemoteControl {
    shared: Arc<Shared>,
}

fn plain_terminal_text(text: &str) -> String {
    crate::invariants::regex(r"\x1b\[[0-?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[@-Z\\-_]").replace_all(text, "").into_owned()
}

impl RemoteControl {
    pub fn new(launcher: Arc<dyn Launcher>, folder: Box<dyn Fn() -> PathBuf + Send + Sync>) -> Self {
        Self::with_deadline(launcher, folder, Duration::from_millis(4000))
    }

    pub fn with_deadline(launcher: Arc<dyn Launcher>, folder: Box<dyn Fn() -> PathBuf + Send + Sync>, stop_deadline: Duration) -> Self {
        Self { shared: Arc::new(Shared { inner: Mutex::new(Inner::default()), closed: Condvar::new(), launcher, folder, stop_deadline }) }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.shared.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn snapshot(&self) -> RemoteControlState {
        self.lock().state.clone()
    }

    /// End the owned process and wait until it reports that it closed.
    fn stop(shared: &Shared) -> Result<(), String> {
        let mut inner = shared.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.child.is_none() {
            return Ok(());
        }
        inner.state.enabled = false;
        inner.state.phase = Phase::Stopping;
        if !inner.stopping {
            inner.stopping = true;
            // Only this owned tree.
            if let Some(child) = &inner.child {
                child.kill_tree();
            }
        }
        let deadline = Instant::now() + shared.stop_deadline;
        while inner.child.is_some() {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                inner.stopping = false;
                return Err(DID_NOT_END.into());
            }
            inner = shared.closed.wait_timeout(inner, left).unwrap_or_else(|e| e.into_inner()).0;
        }
        Ok(())
    }

    pub fn set_enabled(&self, enabled: bool) -> RemoteControlState {
        let token;
        {
            let mut inner = self.lock();
            if enabled && inner.state.enabled && !inner.disposed {
                return inner.state.clone();
            }
            inner.generation += 1;
            token = inner.generation;
            if !enabled || inner.disposed {
                inner.state.enabled = false;
                drop(inner);
                let stopped = Self::stop(&self.shared);
                let mut inner = self.lock();
                if token == inner.generation {
                    match stopped {
                        Ok(()) => inner.state.phase = Phase::Off,
                        Err(why) => {
                            inner.state.error = Some(why);
                            inner.state.phase = Phase::Error;
                        }
                    }
                }
                return inner.state.clone();
            }
            if inner.child.is_some() {
                return inner.state.clone();
            }
            inner.state = RemoteControlState { enabled: true, phase: Phase::Starting, ..Default::default() };
        }
        let fail = |this: &Self, why: String| {
            let mut inner = this.lock();
            if token == inner.generation {
                inner.state.error = Some(why);
                inner.state.enabled = false;
                inner.state.phase = Phase::Error;
            }
            inner.state.clone()
        };
        let program = match self.shared.launcher.discover() {
            Ok(found) => found,
            Err(why) => return fail(self, why),
        };
        {
            let inner = self.lock();
            if token != inner.generation || inner.disposed {
                return inner.state.clone();
            }
        }
        let Some(program) = program else { return fail(self, NOT_INSTALLED.into()) };
        let cwd = (self.shared.folder)();
        let mut inner = self.lock();
        if token != inner.generation || inner.disposed {
            return inner.state.clone();
        }
        inner.asks = false;
        inner.setup_started = false;
        inner.watched.clear();
        inner.pending = [Vec::new(), Vec::new()];
        inner.stopping = false;
        inner.child_generation = token;
        let events = Events { shared: Arc::downgrade(&self.shared), generation: token };
        // Spawned under the lock, so the process's first events wait until it is recorded as the owned one.
        match self.shared.launcher.spawn(&program, &REMOTE_CONTROL_ARGS, &cwd, events) {
            Ok(child) => inner.child = Some(child),
            Err(why) => {
                inner.state.error = Some(why);
                inner.state.enabled = false;
                inner.state.phase = Phase::Error;
            }
        }
        inner.state.clone()
    }

    /// Locust is quitting: end the owned process, waiting for it, and refuse any later enable.
    pub fn dispose(&self) {
        {
            let mut inner = self.lock();
            inner.disposed = true;
            inner.generation += 1;
            inner.state.enabled = false;
        }
        let _ = Self::stop(&self.shared);
    }
}

impl Events {
    fn with(&self, f: impl FnOnce(&Arc<Shared>, &mut Inner)) {
        let Some(shared) = self.shared.upgrade() else { return };
        let mut inner = shared.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.child_generation != self.generation {
            return;
        }
        f(&shared, &mut *inner);
    }

    /// The process started.
    pub fn spawned(&self) {
        self.with(|_, inner| {
            if !inner.asks && inner.generation == self.generation && inner.state.phase == Phase::Starting {
                inner.state.phase = Phase::Running;
            }
        });
    }

    /// Bytes the process printed; a character split across reads is kept until it is whole.
    pub fn output(&self, stream: Stream, bytes: &[u8]) {
        let generation = self.generation;
        self.with(|shared, inner| {
            let at = stream as usize;
            inner.pending[at].extend_from_slice(bytes);
            let text = match std::str::from_utf8(&inner.pending[at]) {
                Ok(text) => text.to_owned(),
                Err(e) if e.error_len().is_none() => {
                    let whole = e.valid_up_to();
                    let text = String::from_utf8_lossy(&inner.pending[at][..whole]).into_owned();
                    inner.pending[at].drain(..whole);
                    append(shared, inner, stream, &text, generation);
                    return;
                }
                Err(_) => String::from_utf8_lossy(&inner.pending[at]).into_owned(),
            };
            inner.pending[at].clear();
            append(shared, inner, stream, &text, generation);
        });
    }

    /// The process could not start or failed: said as it was said.
    pub fn error(&self, message: &str) {
        self.with(|_, inner| {
            inner.state.error = Some(message.to_owned());
            inner.state.enabled = false;
            inner.state.phase = Phase::Error;
        });
    }

    /// The process closed, with its exit code (None when a signal ended it).
    pub fn closed(&self, code: Option<i32>) {
        self.with(|shared, inner| {
            for stream in [Stream::Stdout, Stream::Stderr] {
                let rest = std::mem::take(&mut inner.pending[stream as usize]);
                if !rest.is_empty() {
                    append(shared, inner, stream, &String::from_utf8_lossy(&rest), self.generation);
                }
            }
            inner.child = None;
            inner.stopping = false;
            let phase = inner.state.phase;
            inner.state.enabled = false;
            inner.state.exit_code = Some(code);
            inner.state.phase = if inner.asks {
                Phase::NeedsPerson
            } else if phase == Phase::Stopping {
                Phase::Off
            } else if phase == Phase::Error || code != Some(0) {
                Phase::Error
            } else {
                Phase::Ended
            };
            shared.closed.notify_all();
        });
    }
}

fn append(shared: &Arc<Shared>, inner: &mut Inner, stream: Stream, text: &str, generation: u64) {
    let kept = match stream {
        Stream::Stdout => &mut inner.state.stdout,
        Stream::Stderr => &mut inner.state.stderr,
    };
    let room = LIMIT.saturating_sub(kept.len());
    let mut cut = text.len().min(room);
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    kept.push_str(&text[..cut]);
    inner.state.truncated |= text.len() > room;
    // Watched across chunks, even after the kept first lines reach their cap.
    inner.watched.push_str(text);
    if inner.watched.len() > WATCHED {
        let mut from = inner.watched.len() - WATCHED;
        while !inner.watched.is_char_boundary(from) {
            from += 1;
        }
        inner.watched.drain(..from);
    }
    if !inner.asks && crate::invariants::regex(NEEDS_PERSON).is_match(&plain_terminal_text(&inner.watched)) {
        inner.asks = true;
        if !inner.setup_started {
            inner.setup_started = true;
            inner.state.enabled = false;
            let shared = shared.clone();
            // The setup terminal opens only after the server has closed, off this event's thread.
            std::thread::spawn(move || setup(&shared, generation));
        }
    }
}

fn setup(shared: &Arc<Shared>, token: u64) {
    let stopped = RemoteControl::stop(shared);
    let program = {
        let mut inner = shared.inner.lock().unwrap_or_else(|e| e.into_inner());
        if inner.disposed || token != inner.generation {
            return;
        }
        if let Err(why) = stopped {
            inner.state.error = Some(why);
            inner.state.phase = Phase::Error;
            return;
        }
        inner.state.phase = Phase::NeedsPerson;
        match shared.launcher.discover() {
            Ok(Some(program)) => program,
            _ => return,
        }
    };
    let cwd = (shared.folder)();
    if let Err(why) = shared.launcher.open_setup(&program, &cwd) {
        let mut inner = shared.inner.lock().unwrap_or_else(|e| e.into_inner());
        if token == inner.generation && !inner.disposed {
            inner.state.error = Some(why);
            inner.state.phase = Phase::Error;
        }
    }
}

/// The line under the switch, its state first (Electron's `remoteControlSays`).
pub fn says(state: &RemoteControlState) -> String {
    match state.phase {
        Phase::NeedsPerson => "Complete setup in the Claude Code terminal, then enable this switch again.".into(),
        Phase::Starting => "Starting Claude Code\u{2026}".into(),
        Phase::Running => "On. Start a session in this folder from claude.ai or the Claude app; what Claude Code prints is below. It stops when you switch it off or quit Locust.".into(),
        Phase::Stopping => "Waiting for Claude Code to end\u{2026}".into(),
        Phase::Ended => "Claude Code ended.".into(),
        Phase::Error => match state.exit_code {
            None => "Claude Code reported a problem.".into(),
            Some(None) => "Claude Code reported a problem (exit unknown).".into(),
            Some(Some(code)) => format!("Claude Code reported a problem (exit {code})."),
        },
        Phase::Off => "Off. When on, you can start Claude Code sessions in this folder from claude.ai while Locust is open, until you switch it off or quit Locust.".into(),
    }
}

pub const NEEDS_SIGN_IN: &str = "It needs your claude.ai sign-in; API keys do not work. Team and Enterprise owners must allow Remote Control.";
pub const ONLY_THE_START: &str = "Only the start of what Claude Code printed is shown; the rest was left out.";

/// This computer's Claude Code, run for real.
pub struct RealLauncher {
    pub store: Option<crate::store::Store>,
}

struct RealOwned(u32);
impl Owned for RealOwned {
    fn pid(&self) -> u32 {
        self.0
    }
    fn kill_tree(&self) {
        crate::runtime::process::kill_process(self.0);
    }
}

impl Launcher for RealLauncher {
    fn discover(&self) -> Result<Option<PathBuf>, String> {
        Ok(crate::runtime::discovery::find().ok())
    }

    fn spawn(&self, program: &Path, args: &[&str], cwd: &Path, events: Events) -> Result<Box<dyn Owned>, String> {
        use std::io::Read;
        use std::process::{Command, Stdio};
        let mut command = Command::new(program);
        command.args(args).current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        crate::runtime::process::own_group(&mut command);
        let mut child = command.spawn().map_err(|e| e.to_string())?;
        let pid = child.id();
        let readers: Vec<_> = [(Stream::Stdout, child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>)), (Stream::Stderr, child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>))]
            .into_iter()
            .filter_map(|(stream, pipe)| {
                let mut pipe = pipe?;
                let events = events.clone();
                Some(std::thread::spawn(move || {
                    let mut buf = [0u8; 4096];
                    while let Ok(n) = pipe.read(&mut buf) {
                        if n == 0 {
                            break;
                        }
                        events.output(stream, &buf[..n]);
                    }
                }))
            })
            .collect();
        let spawned = events.clone();
        std::thread::spawn(move || {
            spawned.spawned();
            let code = child.wait().ok().and_then(|s| s.code());
            for reader in readers {
                let _ = reader.join();
            }
            events.closed(code);
        });
        Ok(Box::new(RealOwned(pid)))
    }

    fn open_setup(&self, program: &Path, cwd: &Path) -> Result<(), String> {
        let store = self.store.as_ref().ok_or("Open Claude Code in a terminal in this folder to complete setup, then enable this switch again.")?;
        if !cfg!(target_os = "macos") {
            return Err("Open Claude Code in a terminal in this folder to complete setup, then enable this switch again.".into());
        }
        // Beside the other terminal scripts (terminal_panel's), which remove themselves as they run.
        let script = store.attachments_dir().map_err(|e| e.to_string())?.join(format!("remote-control-setup-{}.command", crate::invariants::unique_nanos()));
        crate::runtime_terminal::open(program, &[], cwd, &script, store)
    }
}

#[cfg(test)]
#[path = "remote_control_tests.rs"]
mod tests;

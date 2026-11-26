//! The record a tester can send back (`main/diagnostics.ts` and the `note` writer in `main/index.ts`).
//!
//! `locust-errors.log` in the data folder: one line per thing that HAPPENED, rolled once at
//! [`crate::report_problem::MAX_LOG_BYTES`] so a crash loop leaves two files, never more. Lines are written for a
//! panic (the window's own thread is the native app's renderer), a child process Locust started that died, the
//! window stopping and starting to answer again, the first line of every run, and missions deleted in this session
//! that a history read brings back.
//!
//! What never goes in, because this file is made to be handed to someone else:
//! - nothing a person said: no prompt, reply, title or file contents. Missions are named by id only; a panic's
//!   message is kept only when it is a literal written in Locust's source, and one formatted at run time (which
//!   could carry what a conversation held) is replaced by a sentence saying it was left out;
//! - no keys: every detail passes through [`crate::secrets::scrub_secrets`] before it is written;
//! - no person's folder: the home folder is written as `~`, and the first line is built from the version, the
//!   platform, the kernel release, the build kind and the commit (hex digits or `dev`, see [`crate::build_info`]),
//!   never from a path, a user name or a host name;
//! - nothing leaves the machine: this module only appends to a local file (mode 0600 on unix) and has no network.

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use std::time::{Duration, Instant};

use chrono::{DateTime, SecondsFormat, Utc};

use crate::report_problem::{diagnostic_line, should_roll, startup_detail};

/// The file's name in the data folder, as Electron's (`errorLog()` in `main/index.ts`).
pub const LOG_FILE: &str = "locust-errors.log";

/// How long the window may be busy in one go before it is written down as not answering.
pub const UNRESPONSIVE_AFTER: Duration = Duration::from_secs(5);

/// Where the log lives under a data folder.
pub fn log_path(root: &Path) -> PathBuf {
    root.join(LOG_FILE)
}

fn rolled_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".1");
    PathBuf::from(name)
}

/// Append one line now. See [`note_at`].
pub fn note(root: &Path, label: &str, detail: &str) {
    note_at(root, Utc::now(), label, detail);
}

/// Append one line, rolling the file first once it has reached its cap. Every failure is swallowed: this runs while
/// something is already going wrong, and a writer that failed on its way to recording a crash would replace a report
/// with a second crash. The append comes after the roll and outside it, so a roll that fails still writes the line.
pub fn note_at(root: &Path, at: DateTime<Utc>, label: &str, detail: &str) {
    let _ = fs::create_dir_all(root);
    let path = log_path(root);
    let bytes = fs::metadata(&path).ok().map(|meta| meta.len());
    if should_roll(bytes) {
        // One generation: a rename over an existing `.1` replaces it, so the oldest is dropped.
        let _ = fs::rename(&path, rolled_path(&path));
    }
    let line = diagnostic_line(&at.to_rfc3339_opts(SecondsFormat::Millis, true), label, &private(detail));
    let mut open = fs::OpenOptions::new();
    open.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        open.mode(0o600);
    }
    if let Ok(mut file) = open.open(&path) {
        let _ = file.write_all(line.as_bytes());
    }
}

/// What a detail becomes before it is written: keys scrubbed, the home folder as `~`, one line.
pub fn private(detail: &str) -> String {
    let (scrubbed, _) = crate::secrets::scrub_secrets(detail);
    let mut said = scrubbed.replace(['\r', '\n'], " ");
    if let Some(home) = std::env::var_os("HOME").map(|h| h.to_string_lossy().trim_end_matches('/').to_owned())
        && home.len() > 1
    {
        said = said.replace(&home, "~");
    }
    said
}

/// What a death was, in words and in the raw token (`describeGone`): the prose for whoever opens the file first, the
/// token for whoever wrote the code.
pub fn describe_gone(what: &str, reason: Option<&str>, exit_code: Option<i32>) -> String {
    let reason = reason.unwrap_or("unknown");
    let code = exit_code.map(|c| format!(" exitCode={c}")).unwrap_or_default();
    let words = match reason {
        "clean-exit" => "exited normally",
        "abnormal-exit" => "exited abnormally",
        "killed" => "was killed",
        "crashed" => "crashed",
        "oom" => "ran out of memory",
        "launch-failed" => "could not start",
        "integrity-failure" => "failed an integrity check",
        _ => "ended for an unrecognised reason",
    };
    format!("{what} {words} (reason={reason}{code})")
}

/// Electron's reason for a child's exit status: a signal that kills is `killed`, one a fault raises is `crashed`, a
/// zero exit is `clean-exit`. A signal's code is written as a shell writes it, 128 plus the signal.
pub fn gone_of(status: &std::process::ExitStatus) -> (&'static str, Option<i32>) {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            let reason = match signal {
                libc::SIGKILL | libc::SIGTERM | libc::SIGINT | libc::SIGHUP => "killed",
                libc::SIGSEGV | libc::SIGBUS | libc::SIGILL | libc::SIGFPE | libc::SIGABRT | libc::SIGTRAP => "crashed",
                _ => "abnormal-exit",
            };
            return (reason, Some(128 + signal));
        }
    }
    match status.code() {
        Some(0) => ("clean-exit", Some(0)),
        code => ("abnormal-exit", code),
    }
}

/// Write down a child that ended, unless it ended cleanly: a log full of clean exits is a log nobody reads.
/// Returns whether a line was written.
pub fn note_child_gone(root: &Path, what: &str, status: &std::process::ExitStatus) -> bool {
    let (reason, code) = gone_of(status);
    if reason == "clean-exit" {
        return false;
    }
    note(root, "child-process-gone", &describe_gone(what, Some(reason), code));
    true
}

/// Wait on a child Locust started, on a thread of its own, and write down how it ended if not cleanly.
pub fn watch_child(root: PathBuf, what: &'static str, mut child: std::process::Child) {
    let _ = std::thread::Builder::new().name("child-watch".into()).spawn(move || {
        if let Ok(status) = child.wait() {
            note_child_gone(&root, what, &status);
        }
    });
}

/// A panic's line. The window's own thread is `main`; a panic there is the native app's renderer dying.
fn panic_detail(info: &std::panic::PanicHookInfo<'_>) -> String {
    let thread = std::thread::current();
    let name = thread.name().unwrap_or("unnamed");
    let at = info
        .location()
        .map(|place| format!("{}:{}:{}", place.file(), place.line(), place.column()))
        .unwrap_or_else(|| "an unknown place".into());
    let said = match info.payload().downcast_ref::<&'static str>() {
        Some(words) => (*words).to_owned(),
        None => "(its message was made while running, so it is left out)".to_owned(),
    };
    let window = if name == "main" { "the window stopped: " } else { "" };
    format!("{window}thread '{name}' panicked at {at}: {said}")
}

/// Write every panic to the log, then let the hook that was there before run too.
pub fn install_panic_hook(root: PathBuf) {
    let before = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        note(&root, "panic", &panic_detail(info));
        before(info);
    }));
}

/// The platform as Electron names it (`process.platform`).
fn platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        other => other,
    }
}

/// The kernel release (`os.release()`): `uname`'s release field, never its node name (the computer's name).
fn os_release() -> String {
    #[cfg(unix)]
    {
        // SAFETY: `uname` fills a zeroed struct it is handed; `release` is a NUL-terminated C string inside it.
        let mut about: libc::utsname = unsafe { std::mem::zeroed() };
        if unsafe { libc::uname(&mut about) } == 0 {
            let release = unsafe { std::ffi::CStr::from_ptr(about.release.as_ptr()) };
            return release.to_string_lossy().into_owned();
        }
    }
    "unknown".into()
}

/// The first line of every run, built from no path: the version, the platform, the kernel release, the build kind and
/// the commit the build came from (only ever hex digits or `dev`, so it can name no one).
pub fn startup_line_detail(version: &str, packaged: bool) -> String {
    startup_detail(version, crate::build_info::commit(), platform(), &os_release(), packaged)
}

/// Everything the window starts before it opens (`crashReporter.start` and `note('start', ...)` before
/// `app.whenReady()`): the panic hook, the run's first line and the watch on a window that stops answering.
pub fn begin(root: &Path, version: &str, packaged: bool) {
    install_panic_hook(root.to_path_buf());
    note(root, "start", &startup_line_detail(version, packaged));
    static WATCHING: OnceLock<()> = OnceLock::new();
    WATCHING.get_or_init(|| {
        let _ = watch_answering(Arc::clone(&WATCH), root.to_path_buf(), UNRESPONSIVE_AFTER, Duration::from_millis(500));
    });
}

/// Whether the window's thread is busy, and since when (`unresponsive` / `responsive` on each web contents).
#[derive(Default)]
pub struct Watch {
    depth: AtomicU64,
    busy_since_ms: AtomicU64,
    entered: AtomicU64,
    stop: AtomicBool,
}

static CLOCK: OnceLock<Instant> = OnceLock::new();

fn clock_ms() -> u64 {
    CLOCK.get_or_init(Instant::now).elapsed().as_millis() as u64 + 1
}

/// Held while the window's thread works; dropping it says the work is done.
pub struct Answering<'a>(&'a Watch);

impl Watch {
    pub fn answering(&self) -> Answering<'_> {
        self.entered.fetch_add(1, Ordering::Relaxed);
        if self.depth.fetch_add(1, Ordering::AcqRel) == 0 {
            self.busy_since_ms.store(clock_ms(), Ordering::Release);
        }
        Answering(self)
    }

    /// How many times the window has started a piece of work.
    pub fn entered(&self) -> u64 {
        self.entered.load(Ordering::Relaxed)
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

impl Drop for Answering<'_> {
    fn drop(&mut self) {
        if self.0.depth.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.0.busy_since_ms.store(0, Ordering::Release);
        }
    }
}

/// The window's own watch: `poll` holds [`answering`] while it works.
pub static WATCH: LazyLock<Arc<Watch>> = LazyLock::new(|| Arc::new(Watch::default()));

/// Mark the window's thread busy until the returned guard drops.
pub fn answering() -> Answering<'static> {
    WATCH.answering()
}

/// Watch `watch` on a thread: one line when a piece of work has run `after` or longer, and one when it ends.
pub fn watch_answering(
    watch: Arc<Watch>,
    root: PathBuf,
    after: Duration,
    tick: Duration,
) -> std::io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new().name("answering-watch".into()).spawn(move || {
        let mut stuck_since: Option<u64> = None;
        while !watch.stop.load(Ordering::Relaxed) {
            let since = watch.busy_since_ms.load(Ordering::Acquire);
            match stuck_since {
                None if since != 0 && clock_ms().saturating_sub(since) >= after.as_millis() as u64 => {
                    note(&root, "unresponsive", "a window stopped answering");
                    stuck_since = Some(since);
                }
                Some(was) if since != was => {
                    note(&root, "responsive", "it started answering again");
                    stuck_since = None;
                }
                _ => {}
            }
            std::thread::sleep(tick);
        }
    })
}

/// Missions deleted in this run, by id, so one that a history read brings back is said rather than argued about.
/// In memory only: it answers a question about one run, and a list of deletions that outlived it would be a record of
/// deletions, the opposite of the point.
#[derive(Default)]
pub struct DeletedThisSession {
    ids: Mutex<HashSet<String>>,
}

/// An id, never content: what a ledger names its files by.
fn is_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

impl DeletedThisSession {
    pub fn deleted(&self, root: &Path, mission_id: &str) {
        if !is_id(mission_id) {
            return;
        }
        crate::invariants::lock(&self.ids).insert(mission_id.to_owned());
        note(root, "mission-deleted", mission_id);
    }

    pub fn restored(&self, root: &Path, mission_id: &str) {
        if crate::invariants::lock(&self.ids).remove(mission_id) {
            note(root, "mission-restored", mission_id);
        }
    }

    pub fn knows(&self, mission_id: &str) -> bool {
        crate::invariants::lock(&self.ids).contains(mission_id)
    }

    /// Check one history read: any deleted id it lists is written down, by id, once.
    pub fn check_read<'a>(&self, root: &Path, listed: impl IntoIterator<Item = &'a str>) -> Vec<String> {
        let mut ids = crate::invariants::lock(&self.ids);
        if ids.is_empty() {
            return Vec::new();
        }
        let back: Vec<String> = listed.into_iter().filter(|id| ids.contains(*id)).map(str::to_owned).collect();
        if back.is_empty() {
            return back;
        }
        for id in &back {
            ids.remove(id);
        }
        drop(ids);
        note(root, "deleted-mission-returned", &back.join(" "));
        back
    }
}

/// This run's list.
pub fn this_session() -> &'static DeletedThisSession {
    static SESSION: LazyLock<DeletedThisSession> = LazyLock::new(DeletedThisSession::default);
    &SESSION
}

#[cfg(test)]
#[path = "a_beta_can_report_itself_tests.rs"]
mod a_beta_can_report_itself_tests;

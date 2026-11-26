//! SOAK 1: a day of use in an hour, against the engine, with fake agents only (notes/SOAK-NOTES.md).
//!
//! Each row is a file here with a short form that runs in the normal suite and a long form marked `#[ignore]`, run by
//! hand (`cargo test -p locust-core soak_tests:: -- --ignored --nocapture`). No test here runs a real agent or reaches
//! the network. The engine's process-wide numbers (memory, descriptors, threads, children) are only honest in a process
//! of their own, and a killed engine has to be another process anyway, so the rows that need one run this same test
//! binary again as a child (`child`), on one named test that does nothing unless `SOAK_CHILD` is set.
use crate::{
    engine::Teammate,
    native::{self, Send},
    store::Store,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{Duration, Instant},
};

mod many_turns;

static TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A disposable data folder: Electron's (read-only to the engine), native's, and a work folder for the teammate.
pub(crate) struct Temp(pub(crate) PathBuf);
impl Temp {
    pub(crate) fn new(what: &str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "locust-soak-{what}-{}-{}-{}",
            std::process::id(),
            crate::invariants::unique_nanos(),
            TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        Self::at(p)
    }
    /// The same folders at a path a parent chose (a child engine works in its parent's folder).
    pub(crate) fn at(p: PathBuf) -> Self {
        fs::create_dir_all(p.join("electron/mission-ledger")).unwrap();
        fs::create_dir_all(p.join("work")).unwrap();
        Self(p)
    }
    pub(crate) fn electron(&self) -> PathBuf {
        self.0.join("electron")
    }
    pub(crate) fn native(&self) -> PathBuf {
        self.0.join("native")
    }
    pub(crate) fn store(&self) -> Store {
        Store::new(self.native(), self.electron()).unwrap()
    }
    pub(crate) fn ledger_dir(&self) -> PathBuf {
        self.store().ledger_dir().unwrap()
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(crate) const FAKE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/soak_tests/fixtures/soak-app-server.py");

/// The soak fake standing in for Codex's app-server, on one scenario.
pub(crate) fn fake(scenario: &str) -> Option<(PathBuf, Vec<String>)> {
    Some((PathBuf::from("python3"), vec![FAKE.into(), scenario.into()]))
}

pub(crate) fn codex(t: &Temp) -> Teammate {
    Teammate {
        id: "tm_soak".into(),
        name: "Soak".into(),
        role: "Code & Migrations".into(),
        runtime: Some("codex".into()),
        model: Some("synthetic-model".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(t.0.join("work").to_string_lossy().into()),
    }
}

pub(crate) fn job(t: &Temp, conversation: Option<&str>, prompt: &str) -> Send {
    Send {
        teammate: codex(t),
        conversation: conversation.map(str::to_owned),
        prompt: prompt.into(),
        model: Some("synthetic-model".into()),
        effort: None,
    }
}

pub(crate) fn wait(what: &str, done: impl FnMut() -> bool) -> Duration {
    crate::load_waits::wait_for(what, done)
}

/// The turn is over and its thread done with it: this process no longer runs it, and no turn thread is still ending.
pub(crate) fn finished(id: &str) {
    wait("the turn to release its slot", || !native::owns(id));
}

// ---- what the process holds -------------------------------------------------------------------------------------

/// Resident memory in KiB: /proc/self/statm on Linux, `ps -o rss=` elsewhere (macOS).
pub(crate) fn rss_kib() -> u64 {
    if let Ok(statm) = fs::read_to_string("/proc/self/statm") {
        let pages: u64 = statm.split_whitespace().nth(1).and_then(|v| v.parse().ok()).unwrap_or(0);
        // SAFETY: sysconf only reads a configuration value.
        let page = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
        return pages * page / 1024;
    }
    let out = Command::new("ps").args(["-o", "rss=", "-p", &std::process::id().to_string()]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0)
}

/// Open file descriptors of this process.
pub(crate) fn fds() -> usize {
    let dir = if Path::new("/proc/self/fd").is_dir() { "/proc/self/fd" } else { "/dev/fd" };
    fs::read_dir(dir).map(|d| d.count()).unwrap_or(0)
}

/// Threads of this process.
pub(crate) fn threads() -> usize {
    if let Ok(d) = fs::read_dir("/proc/self/task") {
        return d.count();
    }
    let out = Command::new("ps").args(["-M", "-p", &std::process::id().to_string()]).output().unwrap();
    String::from_utf8_lossy(&out.stdout).lines().count().saturating_sub(1)
}

/// This process's children: (still running, exited but never reaped).
pub(crate) fn children() -> (usize, usize) {
    let me = std::process::id().to_string();
    let mut alive = 0;
    let mut zombies = 0;
    if let Ok(dir) = fs::read_dir("/proc") {
        for e in dir.flatten() {
            let Ok(stat) = fs::read_to_string(e.path().join("stat")) else { continue };
            // pid (comm) state ppid ...: comm may hold spaces, so read after its closing parenthesis.
            let Some(rest) = stat.rfind(')').map(|i| &stat[i + 1..]) else { continue };
            let mut f = rest.split_whitespace();
            let (state, ppid) = (f.next().unwrap_or(""), f.next().unwrap_or(""));
            if ppid == me {
                if state == "Z" { zombies += 1 } else { alive += 1 }
            }
        }
        return (alive, zombies);
    }
    let out = Command::new("ps").args(["-o", "stat=", "-g", &me]).output().unwrap();
    for l in String::from_utf8_lossy(&out.stdout).lines() {
        if l.starts_with('Z') { zombies += 1 } else { alive += 1 }
    }
    (alive, zombies)
}

/// One reading of everything the soak watches.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Reading {
    pub rss_kib: u64,
    pub fds: usize,
    pub threads: usize,
    pub children: usize,
    pub zombies: usize,
}
pub(crate) fn reading() -> Reading {
    let (children, zombies) = children();
    Reading { rss_kib: rss_kib(), fds: fds(), threads: threads(), children, zombies }
}

// ---- this test binary, again, as a child ------------------------------------------------------------------------

/// Set in a child run of this binary; a child test does nothing without it.
pub(crate) const CHILD: &str = "SOAK_CHILD";

pub(crate) fn is_child() -> bool {
    std::env::var_os(CHILD).is_some()
}

/// This test binary, on exactly one (ignored) test, alone, with these variables: the child's command, not yet run.
pub(crate) fn child(test: &str, envs: &[(&str, String)]) -> Command {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.args(["--exact", test, "--ignored", "--nocapture", "--test-threads=1", "-q"]);
    c.env(CHILD, "1");
    for (k, v) in envs {
        c.env(k, v);
    }
    c
}

/// Runs a child to its end and returns what it printed; panics with its output when it failed.
pub(crate) fn run_child(test: &str, envs: &[(&str, String)]) -> Output {
    let out = child(test, envs).output().unwrap();
    assert!(
        out.status.success(),
        "child {test} failed: {}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    out
}

/// The `SOAK key=value ...` lines a child printed, as key/value pairs in order.
pub(crate) fn soak_lines(out: &Output) -> Vec<Vec<(String, String)>> {
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.strip_prefix("SOAK "))
        .map(|l| {
            l.split_whitespace()
                .filter_map(|kv| kv.split_once('=').map(|(k, v)| (k.to_owned(), v.to_owned())))
                .collect()
        })
        .collect()
}

pub(crate) fn field<T: std::str::FromStr>(line: &[(String, String)], key: &str) -> T {
    line.iter()
        .find(|(k, _)| k == key)
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or_else(|| panic!("no {key} in {line:?}"))
}

pub(crate) fn since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

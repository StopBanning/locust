//! process-runner.ts's transport rules for a runtime that writes one JSON record per line: how large a record may be,
//! what stands in for one that is not carried, how many may wait unread, what of the machine's environment a child
//! sees, and what of its stderr is kept. The drivers keep their own protocols; these are the limits they share.
use std::{
    ffi::OsString,
    io::Read,
    process::Command,
    sync::{Arc, Mutex},
    time::Duration,
};

/// process-runner.ts maxRecordBytes: an ordinary record past this is skipped, and the run carries on.
pub const MAX_RECORD_BYTES: usize = 256 * 1024;
/// maxResultRecordBytes: a turn's `result` repeats the whole answer, so it gets a larger cap of its own.
pub const MAX_RESULT_RECORD_BYTES: usize = 8 * 1024 * 1024;
/// maxQueuedRecords. It was 64 (and 256 here): a fast runtime pushes more than that while one batch is written, and a
/// run was stopped for being productive (measured 2026-09-08). Overflow stays fatal: it means an unknown hole.
pub const MAX_QUEUED_RECORDS: usize = 2_048;
/// maxStderrBytes: what is kept of a child's stderr; the rest is counted, never held.
pub const MAX_STDERR_BYTES: usize = 64 * 1024;
/// cancellationGraceMs, terminationConfirmationMs, lingerAfterResultMs.
pub const CANCELLATION_GRACE: Duration = Duration::from_millis(1_500);
pub const TERMINATION_CONFIRMATION: Duration = Duration::from_millis(2_000);
pub const LINGER_AFTER_RESULT: Duration = Duration::from_secs(60);
/// Said when the queue overflowed (claude-events.ts finish, outputLimitExceeded).
pub const OUTPUT_LIMIT_MESSAGE: &str = "Claude Code sent output faster than Locust could record it, so the run was stopped rather than leave a gap in its record. Sending it again usually works.";

/// RUNTIME_ENVIRONMENT_ALLOWLIST: executable resolution, account homes, temporary files, locale, proxies and TLS
/// roots. Provider keys, the app's own variables and anything else on the machine are not passed on.
pub const ENVIRONMENT_ALLOWLIST: &[&str] = &[
    "APPDATA",
    "CLAUDE_CONFIG_DIR",
    "COMSPEC",
    "CODEX_HOME",
    "HOME",
    "HOMEDRIVE",
    "HOMEPATH",
    "HTTP_PROXY",
    "HTTPS_PROXY",
    "LANG",
    "LC_ALL",
    "LOCALAPPDATA",
    "LOGNAME",
    "NODE_EXTRA_CA_CERTS",
    "NO_COLOR",
    "NODEFAULTCURRENTDIRECTORYINEXEPATH",
    "NO_PROXY",
    "PATH",
    "PATHEXT",
    "SHELL",
    "SSL_CERT_DIR",
    "SSL_CERT_FILE",
    "SYSTEMROOT",
    "TEMP",
    "TMP",
    "TMPDIR",
    "USER",
    "USERNAME",
    "USERPROFILE",
    "WINDIR",
    "XDG_CACHE_HOME",
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
];

/// How one run is bounded; `Limits::default()` is Electron's.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_record_bytes: usize,
    pub max_result_record_bytes: usize,
    pub max_queued_records: usize,
    pub max_stderr_bytes: usize,
    pub cancellation_grace: Duration,
    pub termination_confirmation: Duration,
    pub linger_after_result: Duration,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_record_bytes: MAX_RECORD_BYTES,
            max_result_record_bytes: MAX_RESULT_RECORD_BYTES,
            max_queued_records: MAX_QUEUED_RECORDS,
            max_stderr_bytes: MAX_STDERR_BYTES,
            cancellation_grace: CANCELLATION_GRACE,
            termination_confirmation: TERMINATION_CONFIRMATION,
            linger_after_result: LINGER_AFTER_RESULT,
        }
    }
}

/// minimalEnvironment then `{...environment, ...spec.env}`: of the machine's variables only the allowlisted ones (by
/// upper-case name), with the spec's own on top, where nothing on the machine can undo them.
pub fn child_environment(
    machine: impl IntoIterator<Item = (OsString, OsString)>,
    spec: &[(OsString, OsString)],
) -> Vec<(OsString, OsString)> {
    let mut out: Vec<(OsString, OsString)> = machine
        .into_iter()
        .filter(|(k, _)| {
            let upper = k.to_string_lossy().to_ascii_uppercase();
            ENVIRONMENT_ALLOWLIST.contains(&upper.as_str())
        })
        .collect();
    for (k, v) in spec {
        out.retain(|(have, _)| have != k);
        out.push((k.clone(), v.clone()));
    }
    out
}

/// Give `command` only the allowlisted environment: what it sets itself stays and wins; what it removes stays removed.
pub fn keep_allowlisted_environment(command: &mut Command) {
    let mut spec = Vec::new();
    let mut removed = Vec::new();
    for (k, v) in command.get_envs() {
        match v {
            Some(v) => spec.push((k.to_owned(), v.to_owned())),
            None => removed.push(k.to_owned()),
        }
    }
    let kept = child_environment(std::env::vars_os(), &spec);
    command.env_clear();
    for (k, v) in kept {
        if !removed.contains(&k) {
            command.env(k, v);
        }
    }
}

/// startsAsTurnResult: from its first 64 bytes alone, whether a line is a turn's `result` record. Read from the start
/// because the line may be too long to hold whole.
pub fn starts_as_turn_result(line: &[u8]) -> bool {
    let head = &line[..line.len().min(64)];
    let mut at = 0;
    let space = |at: &mut usize| {
        while head.get(*at).is_some_and(|b| b.is_ascii_whitespace()) {
            *at += 1;
        }
    };
    let take = |at: &mut usize, word: &[u8]| {
        if head[*at..].starts_with(word) {
            *at += word.len();
            true
        } else {
            false
        }
    };
    space(&mut at);
    if !take(&mut at, b"{") {
        return false;
    }
    space(&mut at);
    if !take(&mut at, b"\"type\"") {
        return false;
    }
    space(&mut at);
    if !take(&mut at, b":") {
        return false;
    }
    space(&mut at);
    if !take(&mut at, b"\"result\"") {
        return false;
    }
    space(&mut at);
    matches!(head.get(at), Some(b',' | b'}'))
}

/// isTurnResult: a whole record whose type is `result`, not text that mentions one.
pub fn is_turn_result(raw: &str) -> bool {
    raw.contains("\"result\"")
        && serde_json::from_str::<serde_json::Value>(raw).is_ok_and(|v| v["type"] == "result")
}

/// What one stretch of stdout came to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    /// A whole record within its cap, as the runtime wrote it (no line end).
    Record(String),
    /// A record too large to carry: its first bytes (for a stand-in), its size (at least what was held), and whether
    /// it began as the turn's result, which still ends the turn.
    Dropped { head: String, bytes: usize, ends_turn: bool },
}

/// acceptStdout: splits bytes into records at line ends. A character split across reads is whole by the time its line
/// is; `\r\n` and blank lines are read as Electron reads them. A line past its cap is dropped and counted once, and
/// the rest of it is skipped up to its line end, so a tail is never read as a record of its own.
#[derive(Debug)]
pub struct Lines {
    held: Vec<u8>,
    skipping: bool,
    max_record: usize,
    max_result: usize,
    pub dropped: usize,
}
impl Lines {
    pub fn new(limits: &Limits) -> Self {
        Self {
            held: Vec::new(),
            skipping: false,
            max_record: limits.max_record_bytes,
            max_result: limits.max_result_record_bytes.max(limits.max_record_bytes),
            dropped: 0,
        }
    }
    fn cap_for(&self, line: &[u8]) -> usize {
        if starts_as_turn_result(line) { self.max_result } else { self.max_record }
    }
    fn emit(&mut self, line: &[u8], out: &mut Vec<Piece>) {
        if line.is_empty() {
            return;
        }
        if line.len() > self.cap_for(line) {
            self.dropped += 1;
            out.push(dropped(line, line.len()));
            return;
        }
        out.push(Piece::Record(String::from_utf8_lossy(line).into_owned()));
    }
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Piece> {
        let mut out = Vec::new();
        self.held.extend_from_slice(bytes);
        while let Some(end) = self.held.iter().position(|b| *b == b'\n') {
            let mut line: Vec<u8> = self.held.drain(..=end).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if self.skipping {
                self.skipping = false;
            } else {
                self.emit(&line, &mut out);
            }
        }
        if self.held.len() > self.cap_for(&self.held) {
            let held = std::mem::take(&mut self.held);
            if !self.skipping {
                self.skipping = true;
                self.dropped += 1;
                out.push(dropped(&held, held.len()));
            }
        }
        out
    }
    /// The stream ended: a last line with no line end is a record, unless its head was already dropped.
    pub fn end(&mut self) -> Vec<Piece> {
        let mut out = Vec::new();
        let mut held = std::mem::take(&mut self.held);
        if held.last() == Some(&b'\r') {
            held.pop();
        }
        if !self.skipping {
            self.emit(&held, &mut out);
        }
        self.skipping = false;
        out
    }
}

fn dropped(line: &[u8], bytes: usize) -> Piece {
    let head = &line[..line.len().min(65_536)];
    Piece::Dropped {
        head: String::from_utf8_lossy(head).into_owned(),
        bytes,
        ends_turn: starts_as_turn_result(line),
    }
}

/// standInFor: what takes a dropped record's place for a runtime that asked: its size, its type and the calls its
/// first 4 KB name, and none of its content.
pub fn stand_in(head: &str, bytes: usize) -> serde_json::Value {
    super::cursor::oversized_stand_in(head, bytes).unwrap_or_else(|| serde_json::json!({"type": "locust.oversized", "bytes": bytes, "callIds": []}))
}

/// appendBounded: the first `limit` bytes of stderr, and how many arrived in all.
#[derive(Debug, Default)]
pub struct BoundedStderr {
    kept: Vec<u8>,
    total: usize,
    limit: usize,
}
impl BoundedStderr {
    pub fn new(limit: usize) -> Self {
        Self { kept: Vec::new(), total: 0, limit }
    }
    pub fn push(&mut self, bytes: &[u8]) {
        self.total += bytes.len();
        let room = self.limit.saturating_sub(self.kept.len());
        self.kept.extend_from_slice(&bytes[..bytes.len().min(room)]);
    }
    /// What was kept, with the prompt redacted, and whether anything was cut.
    pub fn text(&self, prompt: &str) -> (String, bool) {
        (redact_prompt(&String::from_utf8_lossy(&self.kept), prompt), self.total > self.kept.len())
    }
}

/// Drain a child's stderr into a bounded capture (a pipe nobody drains stalls a chatty CLI).
pub fn capture_stderr(mut pipe: impl Read + Send + 'static, limit: usize) -> Arc<Mutex<BoundedStderr>> {
    let held = Arc::new(Mutex::new(BoundedStderr::new(limit)));
    let output = held.clone();
    std::thread::spawn(move || {
        let mut bytes = [0u8; 8192];
        while let Ok(n) = pipe.read(&mut bytes) {
            if n == 0 {
                break;
            }
            crate::invariants::lock(&output).push(&bytes[..n]);
        }
    });
    held
}

/// redactPrompt: the person's own words never ride out in a runtime's stderr.
pub fn redact_prompt(value: &str, prompt: &str) -> String {
    if prompt.is_empty() || !value.contains(prompt) {
        return value.to_owned();
    }
    value.replace(prompt, "[prompt redacted]")
}

/// A start that failed says which failure it was by the system's own code, and nothing else of the error (which can
/// carry a path or an argument).
pub fn launch_error(error: &std::io::Error) -> std::io::Error {
    let code = error.raw_os_error().and_then(errno_name);
    let message = match code {
        Some(code) => format!("Runtime process failed to start ({code})"),
        None => "Runtime process failed to start".to_owned(),
    };
    std::io::Error::new(error.kind(), message)
}

fn errno_name(code: i32) -> Option<&'static str> {
    #[cfg(unix)]
    {
        Some(match code {
            libc::ENOENT => "ENOENT",
            libc::EACCES => "EACCES",
            libc::ENOTDIR => "ENOTDIR",
            libc::EPERM => "EPERM",
            libc::ENOEXEC => "ENOEXEC",
            libc::E2BIG => "E2BIG",
            libc::EMFILE => "EMFILE",
            libc::ENOMEM => "ENOMEM",
            libc::EAGAIN => "EAGAIN",
            libc::ETXTBSY => "ETXTBSY",
            libc::ELOOP => "ELOOP",
            libc::ENAMETOOLONG => "ENAMETOOLONG",
            _ => return None,
        })
    }
    #[cfg(not(unix))]
    {
        let _ = code;
        None
    }
}

/// requestTermination's clock: SIGINT first, SIGKILL after the grace period, and a run whose close never comes is
/// settled as unconfirmed once the confirmation window is over. A driver asks `due` on each poll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    /// Nothing to do yet.
    Wait,
    /// The grace period is over: end the tree.
    Force,
    /// The forced signal went unanswered for the whole window: settle without a close.
    Unconfirmed,
}
#[derive(Debug, Clone, Copy)]
pub struct Stopping {
    pub requested: std::time::Instant,
    pub forced: Option<std::time::Instant>,
}
impl Stopping {
    pub fn new(now: std::time::Instant) -> Self {
        Self { requested: now, forced: None }
    }
    pub fn due(&mut self, now: std::time::Instant, limits: &Limits) -> Termination {
        match self.forced {
            None if now.duration_since(self.requested) >= limits.cancellation_grace => {
                self.forced = Some(now);
                Termination::Force
            }
            Some(at) if now.duration_since(at) >= limits.termination_confirmation => Termination::Unconfirmed,
            _ => Termination::Wait,
        }
    }
}

#[cfg(test)]
#[path = "process_runner_tests.rs"]
mod process_runner_tests;

#[cfg(test)]
#[path = "a_result_too_large_tests.rs"]
mod a_result_too_large_tests;

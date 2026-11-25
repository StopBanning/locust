//! Cursor Agent print/stream-json driver. Ports createCursorPrintCommand + process-runner prompt-on-stdin path.
use super::process::{Control, interrupt_process, kill_process};
use super::{Event, Runtime, Turn, cursor_events::CursorNormalizer};
use crate::approval::Mode;
use serde_json::Value;
use std::{
    collections::VecDeque,
    io::{self, BufReader, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
    },
    time::{Duration, Instant},
};

const MAX_LINE: u64 = 8 * 1024 * 1024;
const RECORD_CAP: usize = 256 * 1024;

enum StdinMsg {
    Bytes(Vec<u8>),
    Close,
}

pub struct Cursor {
    child: Child,
    input: SyncSender<StdinMsg>,
    control: Arc<Control>,
    lines: Receiver<io::Result<Line>>,
    normal: CursorNormalizer,
    queued: VecDeque<Event>,
    stopping: Option<Instant>,
    cancel: Option<Arc<AtomicBool>>,
    stdin_open: bool,
    exited: bool,
    exit_code: Option<i32>,
}

enum Line {
    Json(Value),
    Raw(String),
}

fn err(message: impl ToString) -> io::Error {
    io::Error::other(message.to_string())
}

/// createCursorPrintCommand sandbox mapping from Locust mode (codex-mission.ts).
pub fn sandbox_for(mode: Mode) -> &'static str {
    match mode {
        Mode::Auto => "full-access",
        Mode::AcceptEdits | Mode::ApproveEach => "workspace-write",
        Mode::Ask | Mode::Plan => "read-only",
    }
}

pub fn cursor_sandbox_available() -> bool {
    cfg!(any(target_os = "macos", target_os = "linux"))
}

/// What a Cursor turn refuses before it starts: Approve each (Cursor cannot stop and ask), and an effort level, which
/// createCursorPrintCommand refuses in the same words.
pub fn refusal(mode: Mode, effort: Option<&str>) -> Option<&'static str> {
    if mode == Mode::ApproveEach {
        return Some("Cursor does not support Approve-each in the native driver.");
    }
    effort.is_some_and(|e| !e.trim().is_empty()).then_some("Cursor Agent takes no effort level")
}

/// Argv after the executable, matching createCursorPrintCommand.
pub fn print_args(
    workspace: &Path,
    mode: Mode,
    model: Option<&str>,
    resume: Option<&str>,
    skills: Option<&str>,
) -> io::Result<Vec<String>> {
    let sandbox = sandbox_for(mode);
    let read_only = sandbox == "read-only";
    let mut args = vec![
        "--print".into(),
        "--output-format".into(),
        "stream-json".into(),
        "--stream-partial-output".into(),
        "--trust".into(),
    ];
    if !read_only {
        args.push("--approve-mcps".into());
    }
    args.push("--workspace".into());
    args.push(workspace.to_string_lossy().into_owned());
    // The run's skills, as a Cursor plugin folder (run-skills.ts `.cursor-plugin`).
    if let Some(dir) = skills {
        args.extend(["--plugin-dir".into(), dir.into()]);
    }
    if read_only {
        args.push("--mode".into());
        args.push("ask".into());
        if cursor_sandbox_available() {
            args.push("--sandbox".into());
            args.push("enabled".into());
        }
    }
    if let Some(model) = model.map(str::trim).filter(|m| !m.is_empty()) {
        args.push("--model".into());
        args.push(model.into());
    }
    if sandbox == "full-access" {
        args.push("--force".into());
    }
    if let Some(resume) = resume.map(str::trim).filter(|s| !s.is_empty()) {
        args.push("--resume".into());
        args.push(resume.into());
    }
    super::safety::assert_safe_runtime_command("cursor", &args, Some(sandbox)).map_err(err)?;
    Ok(args)
}

pub fn command(
    folder: &Path,
    mode: Mode,
    model: Option<&str>,
    resume: Option<&str>,
    skills: Option<&str>,
) -> io::Result<Command> {
    if mode == Mode::ApproveEach {
        return Err(err(
            "Cursor does not support Approve-each in the native driver.",
        ));
    }
    let mut c = Command::new(super::discovery::find_named("cursor-agent")?);
    c.args(print_args(folder, mode, model, resume, skills)?);
    c.current_dir(folder);
    Ok(c)
}

/// Stand-in for an oversized tool_call line (process-runner.ts standInFor).
pub(super) fn oversized_stand_in(head: &str, bytes: usize) -> Option<Value> {
    let start = head_of(head, 4096);
    let record_re = crate::invariants::regex(r#"^\s*\{\s*"type"\s*:\s*"([a-z_]{1,40})""#);
    let record_type = record_re
        .captures(start)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_owned()));
    let call_ids = call_ids_in(start);
    let edit = edit_report_in(head_of(head, 65_536));
    let mut v = json_map();
    v.insert(
        "type".into(),
        serde_json::Value::String("locust.oversized".into()),
    );
    v.insert("bytes".into(), serde_json::json!(bytes));
    if let Some(t) = record_type {
        v.insert("recordType".into(), serde_json::Value::String(t));
    }
    v.insert(
        "callIds".into(),
        serde_json::Value::Array(
            call_ids
                .into_iter()
                .map(serde_json::Value::String)
                .collect(),
        ),
    );
    if let Some(edit) = edit {
        v.insert("edit".into(), edit);
    }
    Some(serde_json::Value::Object(v))
}

/// The first `bytes` of a line, cut back to a whole character: a cut inside one would panic the reader.
fn head_of(line: &str, bytes: usize) -> &str {
    let mut end = bytes.min(line.len());
    while !line.is_char_boundary(end) {
        end -= 1;
    }
    &line[..end]
}

fn json_map() -> serde_json::Map<String, serde_json::Value> {
    serde_json::Map::new()
}

pub(super) fn edit_report_in(head: &str) -> Option<serde_json::Value> {
    // process-runner.ts editReportIn: only an edit's record, and only with both counts.
    if !head.contains(r#""editToolCall""#) {
        return None;
    }
    let count = |pattern: &'static str| {
        crate::invariants::regex(pattern)
            .captures(head)
            .and_then(|c| c.get(1)?.as_str().parse::<i64>().ok())
    };
    let added = count(r#""linesAdded"\s*:\s*(\d{1,9})"#)?;
    let removed = count(r#""linesRemoved"\s*:\s*(\d{1,9})"#)?;
    // The diff read as the JSON string it is; one that does not read is left out, and the counts stand.
    // Electron's pattern bounds the quoted diff at 16,384 pieces (a character or an escape); past that it does not
    // match, and the diff is left out. Rust's regex cannot hold that bound, so it is counted here.
    let diff = crate::invariants::regex(r#""diffString"\s*:\s*"((?:[^"\\]|\\.)*)""#)
        .captures(head)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str())
        .filter(|quoted| quoted_pieces(quoted) <= 16_384)
        .and_then(|quoted| serde_json::from_str::<String>(&format!("\"{quoted}\"")).ok())
        .filter(|d| !d.is_empty());
    let mut edit = json_map();
    edit.insert("added".into(), serde_json::json!(added));
    edit.insert("removed".into(), serde_json::json!(removed));
    if let Some(d) = diff {
        edit.insert("diff".into(), serde_json::Value::String(d));
    }
    Some(serde_json::Value::Object(edit))
}

/// How many pieces `(?:[^"\\]|\\.)` takes to cover a quoted JSON string in JavaScript: one per UTF-16 unit, an
/// escape (a backslash and what follows it) as one.
fn quoted_pieces(quoted: &str) -> usize {
    let mut chars = quoted.chars();
    let mut pieces = 0;
    while let Some(c) = chars.next() {
        if c == '\\' {
            // `\\.` in JavaScript takes one UTF-16 unit after the backslash; an astral character leaves its second
            // unit as a piece of its own.
            pieces += chars.next().map_or(0, char::len_utf16);
        } else {
            pieces += c.len_utf16();
        }
    }
    pieces
}

/// process-runner.ts callIdsIn: the calls a record's first bytes name, each once, an escaped line break kept as one
/// and any run of white space as `|`, the way the reader writes an item id.
pub(super) fn call_ids_in(head: &str) -> Vec<String> {
    let re = crate::invariants::regex(
        r#""(?:tool_use_id|call_id|callId|toolCallId)"\s*:\s*"((?:\\[nrt]|[A-Za-z0-9_.:-]){1,240})""#,
    );
    let space = crate::invariants::regex(r"\s+");
    let mut ids: Vec<String> = Vec::new();
    for c in re.captures_iter(head) {
        let unescaped = c[1].replace("\\n", "\n").replace("\\r", "\r").replace("\\t", "\t");
        let id = space.replace_all(&unescaped, "|").into_owned();
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids
}

/// cursor-result-stream.ts cursorResultStream: Cursor repeats its streamed answer in its `result` record, so every
/// string in that record is cut to its first 64 Ki characters as it arrives, escapes kept whole across reads. An answer
/// of any length then cannot push the record past the line limit and take its terminal status with it. Every other
/// record passes through as it is. Works on bytes: JSON's quotes, backslashes and line ends are ASCII, and a character
/// is kept or dropped whole (its lead byte settles its continuation bytes).
#[derive(Default)]
pub(super) struct ResultProjector {
    head: Vec<u8>,
    result: Option<bool>,
    quoted: bool,
    escape: Vec<u8>,
    unicode_left: u8,
    kept: usize,
    keeping: bool,
}

const RESULT_STRING_CAP: usize = 64 * 1024;

impl ResultProjector {
    pub(super) fn project(&mut self, chunk: &[u8], out: &mut Vec<u8>) {
        for &byte in chunk {
            if self.result.is_none() {
                self.head.push(byte);
                if self.head.len() < 64 && byte != b'\n' {
                    continue;
                }
                self.keep_or_drop(out);
            } else {
                self.accept(byte, out);
            }
            if byte == b'\n' {
                self.result = None;
                self.quoted = false;
                self.escape.clear();
                self.unicode_left = 0;
            }
        }
    }

    /// What is held when the stream ends. Electron's projector keeps a last line shorter than 64 bytes with no line
    /// end to itself; Rust passes it on.
    pub(super) fn finish(&mut self, out: &mut Vec<u8>) {
        if self.result.is_none() && !self.head.is_empty() {
            self.keep_or_drop(out);
        }
    }

    fn keep_or_drop(&mut self, out: &mut Vec<u8>) {
        let head = std::mem::take(&mut self.head);
        self.result = Some(
            crate::invariants::regex(r#"^\s*\{\s*"type"\s*:\s*"result"\s*[,}]"#).is_match(&String::from_utf8_lossy(&head)),
        );
        for byte in head {
            self.accept(byte, out);
        }
    }

    fn accept(&mut self, byte: u8, out: &mut Vec<u8>) {
        if self.result != Some(true) {
            out.push(byte);
        } else if !self.quoted {
            out.push(byte);
            if byte == b'"' {
                self.quoted = true;
                self.kept = 0;
            }
        } else if !self.escape.is_empty() {
            self.escape.push(byte);
            if self.escape.len() == 2 && byte == b'u' {
                self.unicode_left = 4;
            } else if self.unicode_left > 0 {
                self.unicode_left -= 1;
            }
            if self.unicode_left == 0 {
                if self.kept + self.escape.len() <= RESULT_STRING_CAP {
                    out.extend_from_slice(&self.escape);
                }
                self.kept += self.escape.len();
                self.escape.clear();
            }
        } else if byte == b'"' {
            out.push(byte);
            self.quoted = false;
        } else if byte == b'\\' {
            self.escape.push(byte);
        } else if byte & 0xC0 == 0x80 {
            // A continuation byte goes with its character.
            if self.keeping {
                out.push(byte);
            }
        } else {
            self.keeping = self.kept < RESULT_STRING_CAP;
            if self.keeping {
                out.push(byte);
            }
            self.kept += 1;
        }
    }
}

/// Cursor's output, read through the result projector.
pub(super) struct ProjectedOutput<R> {
    inner: R,
    projector: ResultProjector,
    pending: Vec<u8>,
    at: usize,
    ended: bool,
}

impl<R: std::io::Read> ProjectedOutput<R> {
    pub(super) fn new(inner: R) -> Self {
        Self { inner, projector: ResultProjector::default(), pending: Vec::new(), at: 0, ended: false }
    }
}

impl<R: std::io::Read> std::io::Read for ProjectedOutput<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        while self.at == self.pending.len() {
            if self.ended {
                return Ok(0);
            }
            self.pending.clear();
            self.at = 0;
            let mut chunk = [0u8; 8192];
            let n = self.inner.read(&mut chunk)?;
            if n == 0 {
                self.ended = true;
                self.projector.finish(&mut self.pending);
            } else {
                self.projector.project(&chunk[..n], &mut self.pending);
            }
        }
        let n = buf.len().min(self.pending.len() - self.at);
        buf[..n].copy_from_slice(&self.pending[self.at..self.at + n]);
        self.at += n;
        Ok(n)
    }
}

impl Cursor {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        let mut child = super::process::spawn(command, Stdio::piped(), Stdio::null())?;
        // spawn configured a piped stdin; no prior take occurs.
        let mut stdin = child
            .stdin
            .take()
            // Invariant: spawn configured this pipe and it has not been taken.
            .expect("spawn configured this pipe and it has not been taken");
        let control = Arc::new(Control::default());
        let pid = child.id();
        control.watch(pid);
        let (input, writes) = mpsc::sync_channel::<StdinMsg>(256);
        let writer_control = control.clone();
        std::thread::spawn(move || {
            for msg in writes {
                match msg {
                    StdinMsg::Bytes(bytes) => {
                        if let Err(e) = stdin.write_all(&bytes).and_then(|_| stdin.flush()) {
                            writer_control.fail(format!("Cursor stdin write failed: {e}"));
                            break;
                        }
                    }
                    StdinMsg::Close => break,
                }
            }
        });
        // spawn configured a piped stdout; no prior take occurs.
        let output = child
            .stdout
            .take()
            // Invariant: spawn configured this pipe and it has not been taken.
            .expect("spawn configured this pipe and it has not been taken");
        let (tx, lines) = mpsc::sync_channel(super::records::MAX_QUEUED_RECORDS);
        let reader_control = control.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(ProjectedOutput::new(output));
            loop {
                let mut line = Vec::new();
                let result = super::process::read_line(&mut reader, &mut line);
                match result {
                    Ok(0) => break, // clean EOF — do not spam the channel
                    Ok(_) if line.len() as u64 > MAX_LINE => {
                        let _ = tx.try_send(Err(err("Cursor stream-json line exceeds 8 MiB")));
                        break;
                    }
                    Ok(_) => {
                        let trimmed = if line.last() == Some(&b'\n') {
                            &line[..line.len() - 1]
                        } else {
                            &line[..]
                        };
                        let trimmed = if trimmed.last() == Some(&b'\r') {
                            &trimmed[..trimmed.len() - 1]
                        } else {
                            trimmed
                        };
                        if trimmed.is_empty() {
                            continue;
                        }
                        let value = if trimmed.len() > RECORD_CAP {
                            Ok(Line::Raw(String::from_utf8_lossy(trimmed).into_owned()))
                        } else {
                            match serde_json::from_slice::<Value>(trimmed) {
                                Ok(v) => Ok(Line::Json(v)),
                                Err(_) => {
                                    Ok(Line::Raw(String::from_utf8_lossy(trimmed).into_owned()))
                                }
                            }
                        };
                        match tx.try_send(value) {
                            Ok(()) => {}
                            Err(mpsc::TrySendError::Full(_)) => {
                                reader_control.fail(
                                    "Cursor notification queue overflow (2048 records); terminating run",
                                );
                                break;
                            }
                            Err(mpsc::TrySendError::Disconnected(_)) => break,
                        }
                    }
                    Err(e) => {
                        let _ = tx.try_send(Err(e));
                        break;
                    }
                }
            }
        });
        Ok(Self {
            child,
            input,
            control,
            lines,
            normal: CursorNormalizer::default(),
            queued: VecDeque::new(),
            stopping: None,
            cancel: None,
            stdin_open: true,
            exited: false,
            exit_code: None,
        })
    }

    pub fn with_cancel(mut self, flag: Arc<AtomicBool>) -> Self {
        self.cancel = Some(flag);
        self
    }

    fn cancelled(&self) -> bool {
        crate::run_cli::interrupted()
            || self
                .cancel
                .as_ref()
                .is_some_and(|f| f.load(Ordering::Relaxed))
    }

    fn send(&mut self, bytes: Vec<u8>) -> io::Result<()> {
        if !self.stdin_open {
            return Err(err("Cursor stdin is closed"));
        }
        match self.input.try_send(StdinMsg::Bytes(bytes)) {
            Ok(()) => Ok(()),
            Err(e) => {
                let message = format!("Cursor stdin queue unavailable; terminating run: {e}");
                self.control.fail(&message);
                Err(err(message))
            }
        }
    }

    fn close_stdin(&mut self) {
        if !self.stdin_open {
            return;
        }
        self.stdin_open = false;
        let _ = self.input.try_send(StdinMsg::Close);
    }

    fn receive(&self) -> io::Result<Option<Line>> {
        if let Some(message) = self.control.failure() {
            return Err(err(message));
        }
        match self.lines.recv_timeout(Duration::from_millis(50)) {
            Ok(v) => v.map(Some),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(_) => Err(err("Cursor transport disconnected")),
        }
    }

    fn transport_failure(&mut self, message: &str) -> Event {
        self.queued.clear();
        let diagnostic = self.normal.diagnostic("error", "native.transport", message);
        let terminal = self
            .normal
            .finish(self.stopping.is_some() || self.cancelled(), self.exit_code);
        self.queued.push_back(terminal);
        diagnostic
    }

    fn handle_line(&mut self, line: Line) -> io::Result<()> {
        match line {
            Line::Json(v) => {
                let is_result = v["type"].as_str() == Some("result");
                self.queued.extend(self.normal.accept(&v));
                if is_result {
                    self.close_stdin();
                }
            }
            Line::Raw(raw) => {
                if raw.len() > RECORD_CAP {
                    self.normal.note_oversized_drop();
                    if let Some(stand_in) = oversized_stand_in(&raw, raw.len()) {
                        self.queued.extend(self.normal.accept(&stand_in));
                    }
                    if raw.contains(r#""type":"result""#) || raw.contains(r#""type": "result""#) {
                        self.close_stdin();
                    }
                } else {
                    self.queued.extend(self.normal.accept_line(&raw));
                }
            }
        }
        Ok(())
    }

    fn poll_exit(&mut self) {
        if self.exited {
            return;
        }
        if let Ok(Some(status)) = self.child.try_wait() {
            self.exited = true;
            self.exit_code = status.code();
            self.close_stdin();
        }
    }

    /// Stdout can close a beat before waitpid; give the child a moment to exit.
    fn await_exit(&mut self, budget: Duration) {
        let deadline = Instant::now() + budget;
        while !self.exited && Instant::now() < deadline {
            self.poll_exit();
            if self.exited {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        self.poll_exit();
    }
}

impl Runtime for Cursor {
    fn start(&mut self, turn: &Turn) -> io::Result<()> {
        refusal(turn.mode, turn.effort.as_deref()).map_or(Ok(()), |why| Err(err(why)))?;
        if turn.prompt.trim().is_empty() {
            return Err(err("Prompt must be nonempty"));
        }
        // Prompt on stdin, then close (process-runner default transport).
        self.send(turn.prompt.as_bytes().to_vec())?;
        self.close_stdin();
        Ok(())
    }

    fn next_event(&mut self) -> io::Result<Option<Event>> {
        if self.normal.ended {
            return Ok(self.queued.pop_front());
        }
        if let Some(message) = self.control.failure() {
            return Ok(Some(self.transport_failure(&message)));
        }
        if self
            .stopping
            .is_some_and(|t| t.elapsed() >= Duration::from_secs(5))
        {
            self.queued.clear();
            kill_process(self.child.id());
            return Ok(Some(self.normal.finish(true, self.exit_code)));
        }
        if let Some(e) = self.queued.pop_front() {
            return Ok(Some(e));
        }
        self.poll_exit();
        match self.receive() {
            Ok(Some(line)) => {
                if let Err(e) = self.handle_line(line) {
                    return Ok(Some(self.transport_failure(&e.to_string())));
                }
                Ok(self.queued.pop_front())
            }
            Ok(None) => Ok(None),
            Err(_) => {
                // Reader finished (EOF). Drain anything still queued on the channel, then finish.
                loop {
                    match self.lines.try_recv() {
                        Ok(Ok(line)) => {
                            let _ = self.handle_line(line);
                        }
                        Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => break,
                        Err(mpsc::TryRecvError::Empty) => break,
                    }
                }
                if let Some(e) = self.queued.pop_front() {
                    return Ok(Some(e));
                }
                self.await_exit(Duration::from_millis(500));
                let cancelled = self.stopping.is_some() || self.cancelled();
                Ok(Some(self.normal.finish(cancelled, self.exit_code)))
            }
        }
    }

    fn stop(&mut self) -> io::Result<()> {
        if self.stopping.is_some() {
            return Ok(());
        }
        let now = Instant::now();
        self.stopping = Some(now);
        self.control.cancel_at(now + Duration::from_secs(5));
        interrupt_process(self.child.id());
        self.close_stdin();
        Ok(())
    }
}

impl Drop for Cursor {
    fn drop(&mut self) {
        self.control.close_and_kill(self.child.id());
        super::process::kill_reap(&mut self.child);
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn ask_mode_is_read_only_without_force_or_approve_mcps() {
        let args = print_args(Path::new("/tmp/ws"), Mode::Ask, None, None, None).unwrap();
        assert!(args.windows(2).any(|w| w == ["--mode", "ask"]));
        assert!(!args.iter().any(|a| a == "--force"));
        assert!(!args.iter().any(|a| a == "--approve-mcps"));
        assert!(
            args.windows(2)
                .any(|w| w == ["--output-format", "stream-json"])
        );
        assert!(args.contains(&"--stream-partial-output".into()));
        assert!(args.contains(&"--trust".into()));
    }

    #[test]
    fn auto_mode_forces_and_approves_mcps() {
        let args = print_args(
            Path::new("/tmp/ws"),
            Mode::Auto,
            Some("composer-1"),
            Some("sess"),
            None,
        )
        .unwrap();
        assert!(args.contains(&"--force".into()));
        assert!(args.contains(&"--approve-mcps".into()));
        assert!(args.windows(2).any(|w| w == ["--model", "composer-1"]));
        assert!(args.windows(2).any(|w| w == ["--resume", "sess"]));
        assert!(!args.iter().any(|a| a == "--mode"));
    }

    #[test]
    fn accept_edits_is_workspace_write() {
        let args = print_args(Path::new("/tmp/ws"), Mode::AcceptEdits, None, None, None).unwrap();
        assert!(args.contains(&"--approve-mcps".into()));
        assert!(!args.contains(&"--force".into()));
        assert!(!args.windows(2).any(|w| w == ["--mode", "ask"]));
    }
}

#[cfg(all(test, unix))]
#[path = "cursor_events_case_tests.rs"]
mod cursor_events_case_tests;
#[cfg(all(test, unix))]
#[path = "cursor_secrets_case_tests.rs"]
mod cursor_secrets_case_tests;

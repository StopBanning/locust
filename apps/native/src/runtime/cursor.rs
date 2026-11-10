//! Cursor Agent print/stream-json driver. Ports createCursorPrintCommand + process-runner prompt-on-stdin path.
use super::{
    Event, Runtime, Turn,
    cursor_events::CursorNormalizer,
};
use crate::approval::Mode;
use serde_json::Value;
use std::{
    collections::VecDeque,
    io::{self, BufRead, BufReader, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Condvar, Mutex,
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

#[derive(Default)]
struct ControlState {
    failure: Option<String>,
    deadline: Option<Instant>,
    closed: bool,
}
#[derive(Default)]
struct Control {
    state: Mutex<ControlState>,
    wake: Condvar,
}
impl Control {
    fn fail(&self, message: impl ToString) {
        let mut state = self.state.lock().unwrap();
        if state.failure.is_none() {
            state.failure = Some(message.to_string());
        }
        self.wake.notify_one();
    }
    fn failure(&self) -> Option<String> {
        self.state.lock().unwrap().failure.clone()
    }
}

fn kill_process(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
        libc::kill(pid as i32, libc::SIGKILL);
    }
}
fn interrupt_process(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(pid as i32), libc::SIGINT);
        libc::kill(pid as i32, libc::SIGINT);
    }
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

/// Argv after the executable, matching createCursorPrintCommand.
pub fn print_args(
    workspace: &Path,
    mode: Mode,
    model: Option<&str>,
    resume: Option<&str>,
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
    Ok(args)
}

pub fn command(
    folder: &Path,
    mode: Mode,
    model: Option<&str>,
    resume: Option<&str>,
) -> io::Result<Command> {
    let mut c = Command::new(super::discovery::find_named("cursor-agent")?);
    c.args(print_args(folder, mode, model, resume)?);
    c.current_dir(folder);
    Ok(c)
}

/// Stand-in for an oversized tool_call line (process-runner.ts standInFor).
fn oversized_stand_in(head: &str, bytes: usize) -> Option<Value> {
    let start = &head[..head.len().min(4096)];
    let record_type = regex::Regex::new(r#"^\s*\{\s*"type"\s*:\s*"([a-z_]{1,40})""#)
        .ok()
        .and_then(|re| re.captures(start))
        .and_then(|c| c.get(1).map(|m| m.as_str().to_owned()));
    let call_ids: Vec<String> = {
        let re = regex::Regex::new(r#""(?:call_id|toolCallId)"\s*:\s*"([^"\\]{1,512})""#).unwrap();
        re.captures_iter(start)
            .filter_map(|c| c.get(1).map(|m| m.as_str().replace(['\n', '\r', '\t'], "|")))
            .collect()
    };
    let edit = edit_report_in(&head[..head.len().min(65_536)]);
    let mut v = json_map();
    v.insert("type".into(), serde_json::Value::String("locust.oversized".into()));
    v.insert("bytes".into(), serde_json::json!(bytes));
    if let Some(t) = record_type {
        v.insert("recordType".into(), serde_json::Value::String(t));
    }
    v.insert(
        "callIds".into(),
        serde_json::Value::Array(call_ids.into_iter().map(serde_json::Value::String).collect()),
    );
    if let Some(edit) = edit {
        v.insert("edit".into(), edit);
    }
    Some(serde_json::Value::Object(v))
}

fn json_map() -> serde_json::Map<String, serde_json::Value> {
    serde_json::Map::new()
}

fn edit_report_in(head: &str) -> Option<serde_json::Value> {
    // Prefer editToolCall success counts/diff when present in the head.
    let added = regex::Regex::new(r#""(?:linesAdded|added)"\s*:\s*(\d+)"#)
        .ok()
        .and_then(|re| re.captures(head))
        .and_then(|c| c.get(1)?.as_str().parse::<i64>().ok());
    let removed = regex::Regex::new(r#""(?:linesRemoved|removed)"\s*:\s*(\d+)"#)
        .ok()
        .and_then(|re| re.captures(head))
        .and_then(|c| c.get(1)?.as_str().parse::<i64>().ok());
    let diff = regex::Regex::new(r#""diffString"\s*:\s*"((?:\\.|[^"\\])*)""#)
        .ok()
        .and_then(|re| re.captures(head))
        .and_then(|c| c.get(1).map(|m| m.as_str().replace("\\n", "\n").replace("\\\"", "\"").replace("\\\\", "\\")));
    if added.is_none() && removed.is_none() && diff.is_none() {
        return None;
    }
    let mut edit = json_map();
    if let Some(a) = added {
        edit.insert("added".into(), serde_json::json!(a));
    }
    if let Some(r) = removed {
        edit.insert("removed".into(), serde_json::json!(r));
    }
    if let Some(d) = diff {
        edit.insert("diff".into(), serde_json::Value::String(d));
    }
    Some(serde_json::Value::Object(edit))
}

impl Cursor {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let mut stdin = child.stdin.take().unwrap();
        let control = Arc::new(Control::default());
        let pid = child.id();
        let watchdog = control.clone();
        std::thread::spawn(move || {
            let mut state = watchdog.state.lock().unwrap();
            loop {
                if state.closed {
                    break;
                }
                if state.failure.is_some() || state.deadline.is_some_and(|d| Instant::now() >= d) {
                    kill_process(pid);
                    break;
                }
                let wait = state
                    .deadline
                    .map(|d| d.saturating_duration_since(Instant::now()))
                    .unwrap_or(Duration::from_secs(60));
                state = watchdog.wake.wait_timeout(state, wait).unwrap().0;
            }
        });
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
        let output = child.stdout.take().unwrap();
        let (tx, lines) = mpsc::sync_channel(256);
        let reader_control = control.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut line = Vec::new();
                use std::io::Read;
                let result = (&mut reader).take(MAX_LINE + 1).read_until(b'\n', &mut line);
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
                                Err(_) => Ok(Line::Raw(
                                    String::from_utf8_lossy(trimmed).into_owned(),
                                )),
                            }
                        };
                        match tx.try_send(value) {
                            Ok(()) => {}
                            Err(mpsc::TrySendError::Full(_)) => {
                                reader_control.fail(
                                    "Cursor notification queue overflow (256 records); terminating run",
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
        let diagnostic = self
            .normal
            .diagnostic("error", "native.transport", message);
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
        if turn.prompt.trim().is_empty() {
            return Err(err("Prompt must be nonempty"));
        }
        if turn.effort.as_deref().is_some_and(|e| !e.trim().is_empty()) {
            return Err(err("Cursor Agent takes no effort level"));
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
        {
            let mut state = self.control.state.lock().unwrap();
            state.deadline = Some(now + Duration::from_secs(5));
            self.control.wake.notify_one();
        }
        interrupt_process(self.child.id());
        self.close_stdin();
        Ok(())
    }
}

impl Drop for Cursor {
    fn drop(&mut self) {
        {
            let mut state = self.control.state.lock().unwrap();
            state.closed = true;
            self.control.wake.notify_one();
            kill_process(self.child.id());
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn ask_mode_is_read_only_without_force_or_approve_mcps() {
        let args = print_args(Path::new("/tmp/ws"), Mode::Ask, None, None).unwrap();
        assert!(args.windows(2).any(|w| w == ["--mode", "ask"]));
        assert!(!args.iter().any(|a| a == "--force"));
        assert!(!args.iter().any(|a| a == "--approve-mcps"));
        assert!(args.windows(2).any(|w| w == ["--output-format", "stream-json"]));
        assert!(args.contains(&"--stream-partial-output".into()));
        assert!(args.contains(&"--trust".into()));
    }

    #[test]
    fn auto_mode_forces_and_approves_mcps() {
        let args = print_args(Path::new("/tmp/ws"), Mode::Auto, Some("composer-1"), Some("sess")).unwrap();
        assert!(args.contains(&"--force".into()));
        assert!(args.contains(&"--approve-mcps".into()));
        assert!(args.windows(2).any(|w| w == ["--model", "composer-1"]));
        assert!(args.windows(2).any(|w| w == ["--resume", "sess"]));
        assert!(!args.iter().any(|a| a == "--mode"));
    }

    #[test]
    fn accept_edits_is_workspace_write() {
        let args = print_args(Path::new("/tmp/ws"), Mode::AcceptEdits, None, None).unwrap();
        assert!(args.contains(&"--approve-mcps".into()));
        assert!(!args.contains(&"--force".into()));
        assert!(!args.windows(2).any(|w| w == ["--mode", "ask"]));
    }
}

//! GitHub Copilot CLI json driver. Ports createCopilotPromptCommand + process-runner stdin:none.
use super::{
    Event, Runtime, Turn,
    copilot_events::CopilotNormalizer,
};
use crate::approval::Mode;
use serde_json::Value;
use std::{
    collections::VecDeque,
    io::{self, BufRead, BufReader},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    time::{Duration, Instant},
};

const MAX_LINE: u64 = 8 * 1024 * 1024;
pub const EXCLUDED_TOOLS: &str = "--excluded-tools=session_store_sql";

pub struct Copilot {
    child: Child,
    control: Arc<Control>,
    lines: Receiver<io::Result<Value>>,
    stderr_buf: Arc<Mutex<String>>,
    normal: CopilotNormalizer,
    queued: VecDeque<Event>,
    stopping: Option<Instant>,
    cancel: Option<Arc<AtomicBool>>,
    exited: bool,
    exit_code: Option<i32>,
}

#[derive(Default)]
struct ControlState { failure: Option<String>, deadline: Option<Instant>, closed: bool }
#[derive(Default)]
struct Control { state: Mutex<ControlState>, wake: Condvar }
impl Control {
    fn fail(&self, message: impl ToString) {
        let mut state = self.state.lock().unwrap();
        if state.failure.is_none() { state.failure = Some(message.to_string()); }
        self.wake.notify_one();
    }
    fn failure(&self) -> Option<String> { self.state.lock().unwrap().failure.clone() }
}

fn kill_process(pid: u32) {
    #[cfg(unix)]
    unsafe { libc::kill(-(pid as i32), libc::SIGKILL); libc::kill(pid as i32, libc::SIGKILL); }
}
fn interrupt_process(pid: u32) {
    #[cfg(unix)]
    unsafe { libc::kill(-(pid as i32), libc::SIGINT); libc::kill(pid as i32, libc::SIGINT); }
}
fn err(message: impl ToString) -> io::Error { io::Error::other(message.to_string()) }

pub fn sandbox_for(mode: Mode) -> &'static str {
    match mode {
        Mode::Auto => "full-access",
        Mode::AcceptEdits | Mode::ApproveEach => "workspace-write",
        Mode::Ask | Mode::Plan => "read-only",
    }
}

/// createCopilotPromptCommand argv after the executable.
pub fn prompt_args(
    prompt: &str,
    mode: Mode,
    model: Option<&str>,
    effort: Option<&str>,
    resume: Option<&str>,
    session_id: Option<&str>,
) -> Vec<String> {
    let sandbox = sandbox_for(mode);
    let mut args = vec![
        "-p".into(), prompt.into(),
        "--output-format".into(), "json".into(),
        "--allow-all-tools".into(),
        "--no-color".into(),
        EXCLUDED_TOOLS.into(),
    ];
    if sandbox == "read-only" {
        args.push("--deny-tool=write,shell".into());
    }
    if sandbox == "full-access" {
        args.push("--allow-all-paths".into());
    }
    if let Some(model) = model.map(str::trim).filter(|m| !m.is_empty()) {
        args.push("--model".into());
        args.push(model.into());
    }
    if let Some(effort) = effort.map(str::trim).filter(|e| !e.is_empty()) {
        args.push("--effort".into());
        args.push(effort.into());
    }
    if let Some(resume) = resume.map(str::trim).filter(|s| !s.is_empty()) {
        args.push(format!("--resume={resume}"));
    } else if let Some(session) = session_id.map(str::trim).filter(|s| !s.is_empty()) {
        args.push("--session-id".into());
        args.push(session.into());
    }
    args
}

pub fn command(
    folder: &Path,
    prompt: &str,
    mode: Mode,
    model: Option<&str>,
    effort: Option<&str>,
    resume: Option<&str>,
    session_id: Option<&str>,
) -> io::Result<Command> {
    let mut c = Command::new(super::discovery::find_named("copilot")?);
    c.args(prompt_args(prompt, mode, model, effort, resume, session_id));
    c.current_dir(folder);
    Ok(c)
}

impl Copilot {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        #[cfg(unix)]
        { use std::os::unix::process::CommandExt; command.process_group(0); }
        let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
        let control = Arc::new(Control::default());
        let pid = child.id();
        let watchdog = control.clone();
        std::thread::spawn(move || {
            let mut state = watchdog.state.lock().unwrap();
            loop {
                if state.closed { break; }
                if state.failure.is_some() || state.deadline.is_some_and(|d| Instant::now() >= d) {
                    kill_process(pid); break;
                }
                let wait = state.deadline.map(|d| d.saturating_duration_since(Instant::now())).unwrap_or(Duration::from_secs(60));
                state = watchdog.wake.wait_timeout(state, wait).unwrap().0;
            }
        });
        // Capture stderr for policy/model refusals.
        let stderr = child.stderr.take();
        let stderr_buf = Arc::new(Mutex::new(String::new()));
        if let Some(stderr) = stderr {
            let buf = stderr_buf.clone();
            std::thread::spawn(move || {
                let mut r = BufReader::new(stderr);
                let mut line = String::new();
                while r.read_line(&mut line).unwrap_or(0) > 0 {
                    if let Ok(mut g) = buf.lock() {
                        if g.len() < 64 * 1024 { g.push_str(&line); }
                    }
                    line.clear();
                }
            });
        }
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
                    Ok(0) => break,
                    Ok(_) if line.len() as u64 > MAX_LINE => {
                        let _ = tx.try_send(Err(err("Copilot stream-json line exceeds 8 MiB")));
                        break;
                    }
                    Ok(_) => {
                        let trimmed = if line.last()==Some(&b'\n'){&line[..line.len()-1]}else{&line[..]};
                        let trimmed = if trimmed.last()==Some(&b'\r'){&trimmed[..trimmed.len()-1]}else{trimmed};
                        if trimmed.is_empty() { continue; }
                        let value = serde_json::from_slice::<Value>(trimmed)
                            .map_err(|_| err("Copilot sent malformed json"));
                        match tx.try_send(value) {
                            Ok(()) => {}
                            Err(mpsc::TrySendError::Full(_)) => {
                                reader_control.fail("Copilot notification queue overflow (256 records); terminating run");
                                break;
                            }
                            Err(mpsc::TrySendError::Disconnected(_)) => break,
                        }
                    }
                    Err(e) => { let _ = tx.try_send(Err(e)); break; }
                }
            }
        });
        Ok(Self {
            child,
            control,
            lines,
            stderr_buf,
            normal: CopilotNormalizer::default(),
            queued: VecDeque::new(),
            stopping: None,
            cancel: None,
            exited: false,
            exit_code: None,
        })
    }

    pub fn with_cancel(mut self, flag: Arc<AtomicBool>) -> Self { self.cancel = Some(flag); self }
    fn cancelled(&self) -> bool {
        crate::run_cli::interrupted() || self.cancel.as_ref().is_some_and(|f| f.load(Ordering::Relaxed))
    }
    fn receive(&self) -> io::Result<Option<Value>> {
        if let Some(message) = self.control.failure() { return Err(err(message)); }
        match self.lines.recv_timeout(Duration::from_millis(50)) {
            Ok(v) => v.map(Some),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(_) => Err(err("Copilot transport disconnected")),
        }
    }
    fn transport_failure(&mut self, message: &str) -> Event {
        self.queued.clear();
        let diagnostic = self.normal.diagnostic("error","native.transport", message);
        let terminal = self.normal.finish(self.stopping.is_some() || self.cancelled(), self.exit_code);
        self.queued.push_back(terminal);
        diagnostic
    }
    fn poll_exit(&mut self) {
        if self.exited { return; }
        if let Ok(Some(status)) = self.child.try_wait() {
            self.exited = true;
            self.exit_code = status.code();
        }
    }
    fn await_exit(&mut self, budget: Duration) {
        let deadline = Instant::now() + budget;
        while !self.exited && Instant::now() < deadline {
            self.poll_exit();
            if self.exited { break; }
            std::thread::sleep(Duration::from_millis(5));
        }
        self.poll_exit();
    }
}

impl Runtime for Copilot {
    fn start(&mut self, turn: &Turn) -> io::Result<()> {
        if turn.prompt.trim().is_empty() { return Err(err("Prompt must be nonempty")); }
        // Prompt already in argv; stdin is null. Seed host session so run.started
        // never carries an empty runtimeThreadId (ledger rejects nonempty-required).
        let host = turn
            .resume
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| Some(turn.mission_id.clone()).filter(|s| !s.trim().is_empty()));
        self.normal = CopilotNormalizer::default().with_session(host);
        let _ = turn;
        Ok(())
    }
    fn next_event(&mut self) -> io::Result<Option<Event>> {
        if self.normal.ended { return Ok(self.queued.pop_front()); }
        if let Some(message) = self.control.failure() { return Ok(Some(self.transport_failure(&message))); }
        if self.stopping.is_some_and(|t| t.elapsed() >= Duration::from_secs(5)) {
            self.queued.clear();
            kill_process(self.child.id());
            return Ok(Some(self.normal.finish(true, self.exit_code)));
        }
        if let Some(e) = self.queued.pop_front() { return Ok(Some(e)); }
        self.poll_exit();
        match self.receive() {
            Ok(Some(v)) => {
                self.queued.extend(self.normal.accept(&v));
                Ok(self.queued.pop_front())
            }
            Ok(None) => Ok(None),
            Err(_) => {
                loop {
                    match self.lines.try_recv() {
                        Ok(Ok(v)) => self.queued.extend(self.normal.accept(&v)),
                        Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) | Err(mpsc::TryRecvError::Empty) => break,
                    }
                }
                if let Some(e) = self.queued.pop_front() { return Ok(Some(e)); }
                self.await_exit(Duration::from_millis(500));
                let stderr = self.stderr_buf.lock().unwrap().clone();
                self.normal.set_stderr(&stderr);
                let cancelled = self.stopping.is_some() || self.cancelled();
                Ok(Some(self.normal.finish(cancelled, self.exit_code)))
            }
        }
    }
    fn stop(&mut self) -> io::Result<()> {
        if self.stopping.is_some() { return Ok(()); }
        let now = Instant::now();
        self.stopping = Some(now);
        {
            let mut state = self.control.state.lock().unwrap();
            state.deadline = Some(now + Duration::from_secs(5));
            self.control.wake.notify_one();
        }
        interrupt_process(self.child.id());
        Ok(())
    }
}

impl Drop for Copilot {
    fn drop(&mut self) {
        { let mut state = self.control.state.lock().unwrap(); state.closed = true; self.control.wake.notify_one(); kill_process(self.child.id()); }
        let _ = self.child.kill(); let _ = self.child.wait();
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    #[test]
    fn read_only_denies_write_and_shell() {
        let args = prompt_args("hi", Mode::Ask, None, None, None, Some("sess"));
        assert!(args.iter().any(|a| a == "--deny-tool=write,shell"));
        assert!(args.windows(2).any(|w| w == ["--session-id", "sess"]));
        assert!(!args.iter().any(|a| a == "--allow-all-paths"));
        assert!(args.contains(&EXCLUDED_TOOLS.into()));
    }
    #[test]
    fn auto_allows_all_paths_and_model_effort() {
        let args = prompt_args("hi", Mode::Auto, Some("gpt"), Some("high"), Some("old"), None);
        assert!(args.contains(&"--allow-all-paths".into()));
        assert!(args.windows(2).any(|w| w == ["--model", "gpt"]));
        assert!(args.windows(2).any(|w| w == ["--effort", "high"]));
        assert!(args.iter().any(|a| a == "--resume=old"));
    }
}

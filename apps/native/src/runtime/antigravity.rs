//! Antigravity CLI (agy) driver. Ports createAgyPrintCommand + agy-json stdin.
use super::{
    Event, Runtime, Turn,
    agy_events::AgyNormalizer,
};
use crate::approval::Mode;
use serde_json::{Value, json};
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

enum StdinMsg { Bytes(Vec<u8>), Close }

pub struct Antigravity {
    child: Child,
    input: SyncSender<StdinMsg>,
    control: Arc<Control>,
    lines: Receiver<io::Result<Value>>,
    normal: AgyNormalizer,
    queued: VecDeque<Event>,
    stopping: Option<Instant>,
    cancel: Option<Arc<AtomicBool>>,
    stdin_open: bool,
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
fn kill_process(pid: u32) { #[cfg(unix)] unsafe { libc::kill(-(pid as i32), libc::SIGKILL); libc::kill(pid as i32, libc::SIGKILL); } }
fn interrupt_process(pid: u32) { #[cfg(unix)] unsafe { libc::kill(-(pid as i32), libc::SIGINT); libc::kill(pid as i32, libc::SIGINT); } }
fn err(message: impl ToString) -> io::Error { io::Error::other(message.to_string()) }

pub fn sandbox_for(mode: Mode) -> &'static str {
    match mode {
        Mode::Auto => "full-access",
        Mode::AcceptEdits | Mode::ApproveEach => "workspace-write",
        Mode::Ask | Mode::Plan => "read-only",
    }
}

pub fn user_line(prompt: &str) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&json!({
        "event": "user",
        "message": {"role":"user","content":[{"type":"text","text": prompt}]}
    })).unwrap();
    bytes.push(b'\n');
    bytes
}

pub fn print_args(mode: Mode, model: Option<&str>, effort: Option<&str>, resume: Option<&str>) -> Vec<String> {
    let sandbox = sandbox_for(mode);
    let mut args = vec!["--output-format".into(), "stream-json".into(), "--input-format".into(), "stream-json".into()];
    if sandbox == "workspace-write" { args.push("--mode".into()); args.push("accept-edits".into()); }
    if sandbox == "full-access" { args.push("--dangerously-skip-permissions".into()); }
    if let Some(model) = model.map(str::trim).filter(|m| !m.is_empty()) {
        args.push("--model".into()); args.push(model.into());
    }
    if let Some(effort) = effort.map(str::trim).filter(|e| !e.is_empty()) {
        args.push("--effort".into()); args.push(effort.into());
    }
    if let Some(resume) = resume.map(str::trim).filter(|s| !s.is_empty()) {
        args.push("--conversation".into()); args.push(resume.into());
    }
    args.push("-p=".into());
    args
}

pub fn command(folder: &Path, mode: Mode, model: Option<&str>, effort: Option<&str>, resume: Option<&str>) -> io::Result<Command> {
    if mode == Mode::ApproveEach { return Err(err("Antigravity does not support Approve-each in the native driver.")); }
    let mut c = Command::new(super::discovery::find_named("agy")?);
    c.args(print_args(mode, model, effort, resume)).current_dir(folder);
    Ok(c)
}

impl Antigravity {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        #[cfg(unix)] { use std::os::unix::process::CommandExt; command.process_group(0); }
        let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()?;
        let mut stdin = child.stdin.take().unwrap();
        let control = Arc::new(Control::default());
        let pid = child.id();
        let watchdog = control.clone();
        std::thread::spawn(move || {
            let mut state = watchdog.state.lock().unwrap();
            loop {
                if state.closed { break; }
                if state.failure.is_some() || state.deadline.is_some_and(|d| Instant::now() >= d) { kill_process(pid); break; }
                let wait = state.deadline.map(|d| d.saturating_duration_since(Instant::now())).unwrap_or(Duration::from_secs(60));
                state = watchdog.wake.wait_timeout(state, wait).unwrap().0;
            }
        });
        let (input, writes) = mpsc::sync_channel::<StdinMsg>(256);
        let writer_control = control.clone();
        std::thread::spawn(move || {
            for msg in writes {
                match msg {
                    StdinMsg::Bytes(bytes) => { if let Err(e) = stdin.write_all(&bytes).and_then(|_| stdin.flush()) { writer_control.fail(format!("Antigravity stdin write failed: {e}")); break; } }
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
                match (&mut reader).take(MAX_LINE + 1).read_until(b'\n', &mut line) {
                    Ok(0) => break,
                    Ok(_) if line.len() as u64 > MAX_LINE => { let _ = tx.try_send(Err(err("Antigravity stream-json line exceeds 8 MiB"))); break; }
                    Ok(_) => {
                        let trimmed = if line.last()==Some(&b'\n'){&line[..line.len()-1]}else{&line[..]};
                        let trimmed = if trimmed.last()==Some(&b'\r'){&trimmed[..trimmed.len()-1]}else{trimmed};
                        if trimmed.is_empty() { continue; }
                        let value = serde_json::from_slice::<Value>(trimmed).map_err(|_| err("Antigravity sent malformed json"));
                        match tx.try_send(value) {
                            Ok(()) => {}
                            Err(mpsc::TrySendError::Full(_)) => { reader_control.fail("Antigravity notification queue overflow"); break; }
                            Err(mpsc::TrySendError::Disconnected(_)) => break,
                        }
                    }
                    Err(e) => { let _ = tx.try_send(Err(e)); break; }
                }
            }
        });
        Ok(Self {
            child, input, control, lines,
            normal: AgyNormalizer::default(),
            queued: VecDeque::new(), stopping: None, cancel: None,
            stdin_open: true, exited: false, exit_code: None,
        })
    }
    pub fn with_cancel(mut self, flag: Arc<AtomicBool>) -> Self { self.cancel = Some(flag); self }
    fn cancelled(&self) -> bool { crate::run_cli::interrupted() || self.cancel.as_ref().is_some_and(|f| f.load(Ordering::Relaxed)) }
    fn send(&mut self, bytes: Vec<u8>) -> io::Result<()> {
        if !self.stdin_open { return Err(err("Antigravity stdin is closed")); }
        self.input.try_send(StdinMsg::Bytes(bytes)).map_err(|e| { let m=format!("Antigravity stdin queue unavailable: {e}"); self.control.fail(&m); err(m) })
    }
    fn close_stdin(&mut self) { if self.stdin_open { self.stdin_open=false; let _=self.input.try_send(StdinMsg::Close); } }
    fn receive(&self) -> io::Result<Option<Value>> {
        if let Some(message) = self.control.failure() { return Err(err(message)); }
        match self.lines.recv_timeout(Duration::from_millis(50)) {
            Ok(v) => v.map(Some),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(_) => Err(err("Antigravity transport disconnected")),
        }
    }
    fn poll_exit(&mut self) {
        if self.exited { return; }
        if let Ok(Some(status)) = self.child.try_wait() { self.exited=true; self.exit_code=status.code(); self.close_stdin(); }
    }
    fn await_exit(&mut self, budget: Duration) {
        let deadline = Instant::now()+budget;
        while !self.exited && Instant::now()<deadline { self.poll_exit(); if self.exited { break; } std::thread::sleep(Duration::from_millis(5)); }
        self.poll_exit();
    }
}

impl Runtime for Antigravity {
    fn start(&mut self, turn: &Turn) -> io::Result<()> {
        if turn.mode == Mode::ApproveEach { return Err(err("Antigravity does not support Approve-each in the native driver.")); }
        if turn.prompt.trim().is_empty() { return Err(err("Prompt must be nonempty")); }
        self.normal = AgyNormalizer::default().with_sandbox(sandbox_for(turn.mode));
        self.send(user_line(&turn.prompt))?;
        self.close_stdin();
        Ok(())
    }
    fn next_event(&mut self) -> io::Result<Option<Event>> {
        if self.normal.ended { return Ok(self.queued.pop_front()); }
        if let Some(message) = self.control.failure() {
            self.queued.clear();
            self.queued.extend(self.normal.finish(self.stopping.is_some()||self.cancelled(), self.exit_code));
            return Ok(Some(self.normal.diagnostic("error","native.transport",&message)));
        }
        if self.stopping.is_some_and(|t| t.elapsed() >= Duration::from_secs(5)) {
            self.queued.clear(); kill_process(self.child.id());
            self.queued.extend(self.normal.finish(true, self.exit_code));
            return Ok(self.queued.pop_front());
        }
        if let Some(e) = self.queued.pop_front() { return Ok(Some(e)); }
        self.poll_exit();
        match self.receive() {
            Ok(Some(v)) => {
                if v["event"].as_str()==Some("result") { self.close_stdin(); }
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
                if let Some(e)=self.queued.pop_front() { return Ok(Some(e)); }
                self.await_exit(Duration::from_millis(500));
                self.queued.extend(self.normal.finish(self.stopping.is_some()||self.cancelled(), self.exit_code));
                Ok(self.queued.pop_front())
            }
        }
    }
    fn stop(&mut self) -> io::Result<()> {
        if self.stopping.is_some() { return Ok(()); }
        let now = Instant::now();
        self.stopping = Some(now);
        { let mut state = self.control.state.lock().unwrap(); state.deadline = Some(now + Duration::from_secs(5)); self.control.wake.notify_one(); }
        interrupt_process(self.child.id());
        self.close_stdin();
        Ok(())
    }
}
impl Drop for Antigravity {
    fn drop(&mut self) {
        { let mut state = self.control.state.lock().unwrap(); state.closed=true; self.control.wake.notify_one(); kill_process(self.child.id()); }
        let _=self.child.kill(); let _=self.child.wait();
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    #[test]
    fn ask_is_read_only_without_skip() {
        let args = print_args(Mode::Ask, Some("gemini"), None, None);
        assert!(!args.iter().any(|a| a == "--dangerously-skip-permissions"));
        assert!(args.windows(2).any(|w| w == ["--model", "gemini"]));
        assert!(args.contains(&"-p=".into()));
    }
    #[test]
    fn auto_skips_permissions() {
        let args = print_args(Mode::Auto, None, Some("low"), Some("c1"));
        assert!(args.contains(&"--dangerously-skip-permissions".into()));
        assert!(args.windows(2).any(|w| w == ["--effort", "low"]));
        assert!(args.windows(2).any(|w| w == ["--conversation", "c1"]));
    }
}

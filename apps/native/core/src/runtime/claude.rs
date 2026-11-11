type ApprovalLifecycle = Arc<Mutex<Option<(Arc<Channel>, String)>>>;
// Claude Code print/stream-json driver. Ports createClaudePrintCommand + process-runner stream-json path.
use super::process::{Control, interrupt_process, kill_process};
use super::{Event, Runtime, Turn, claude_events::ClaudeNormalizer};
use crate::approval::{Channel, Mode};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io::{self, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
    },
    time::{Duration, Instant},
};

const MAX_LINE: u64 = 8 * 1024 * 1024;
/// Ask-mode tools from createClaudePrintCommand when sandbox is read-only.
const ASK_TOOLS: &str = "Read,Glob,Grep,Task,Skill,WebSearch,WebFetch,mcp__*";

enum StdinMsg {
    Bytes(Vec<u8>),
    Close,
}

pub struct Claude {
    child: Child,
    input: SyncSender<StdinMsg>,
    control: Arc<Control>,
    lines: Receiver<io::Result<Value>>,
    normal: ClaudeNormalizer,
    queued: VecDeque<Event>,
    stopping: Option<Instant>,
    cancel: Option<Arc<AtomicBool>>,
    /// stdin stays open until the turn's `result` (process-runner.ts A2.10).
    stdin_open: bool,
    /// Process exited; finish() still owes a terminal event.
    exited: bool,
    exit_code: Option<i32>,
    approvals: Option<Arc<Channel>>,
    mission_id: String,
    run_id: String,
    folder: PathBuf,
    mode: Mode,
    approval_lifecycle: ApprovalLifecycle,
}

fn err(message: impl ToString) -> io::Error {
    io::Error::other(message.to_string())
}

/// User turn line for `--input-format stream-json` (process-runner.ts:streamJsonUserLine).
pub fn user_line(prompt: &str) -> Vec<u8> {
    let mut bytes = json!({
        "type": "user",
        "message": {"role": "user", "content": prompt}
    })
    .to_string()
    .into_bytes();
    bytes.push(b'\n');
    bytes
}

/// Ask-mode argv from createClaudePrintCommand (sandbox read-only, no bridge).
pub fn ask_args(model: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "--restricted".into(),
        "--print".into(),
        "--input-format".into(),
        "stream-json".into(),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
        "--include-partial-messages".into(),
        "--permission-mode".into(),
        "manual".into(),
        "--tools".into(),
        ASK_TOOLS.into(),
    ];
    if let Some(model) = model.map(str::trim).filter(|m| !m.is_empty()) {
        args.push("--model".into());
        args.push(model.into());
    }
    args
}

pub fn approve_each_command(folder: &Path, model: Option<&str>) -> io::Result<Command> {
    let mut c = command(folder, model)?;
    let mut args = ask_args(model);
    let tools = args
        .iter()
        .position(|a| a == "--tools")
        // Invariant: ask_args always includes the literal --tools option.
        .expect("ask_args always includes the literal --tools option")
        + 1;
    args[tools] =
        "Read,Glob,Grep,Edit,Write,NotebookEdit,Bash,Task,Skill,WebSearch,WebFetch,mcp__*".into();
    // Rebuild rather than append a second --tools (which can change CLI parsing).
    c = Command::new(c.get_program());
    c.args(args)
        .current_dir(folder)
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("CLAUDE_API_KEY");
    Ok(c)
}

pub fn command(folder: &Path, model: Option<&str>) -> io::Result<Command> {
    let mut c = Command::new(super::discovery::find_named("claude")?);
    c.args(ask_args(model));
    c.current_dir(folder);
    // Never pass Locust API keys; Claude Code reads its own login.
    c.env_remove("ANTHROPIC_API_KEY");
    c.env_remove("CLAUDE_API_KEY");
    Ok(c)
}

impl Claude {
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
                            writer_control.fail(format!("Claude stdin write failed: {e}"));
                            break;
                        }
                    }
                    StdinMsg::Close => break,
                }
            }
            // Dropping stdin closes the pipe (turn result / stop).
        });
        let approval_lifecycle: ApprovalLifecycle = Arc::new(Mutex::new(None));
        let lifecycle = approval_lifecycle.clone();
        // spawn configured a piped stdout; no prior take occurs.
        let output = child
            .stdout
            .take()
            // Invariant: spawn configured this pipe and it has not been taken.
            .expect("spawn configured this pipe and it has not been taken");
        let (tx, lines) = mpsc::sync_channel(256);
        let reader_control = control.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut line = Vec::new();
                let result = super::process::read_line(&mut reader, &mut line);
                let value = match result {
                    Ok(0) => Err(err("Claude stdout closed before the turn completed")),
                    Ok(_) if line.len() as u64 > MAX_LINE => {
                        Err(err("Claude stream-json line exceeds 8 MiB"))
                    }
                    Ok(_) => {
                        let trimmed = if line.last() == Some(&b'\n') {
                            &line[..line.len() - 1]
                        } else {
                            &line[..]
                        };
                        if trimmed.is_empty() {
                            continue;
                        }
                        serde_json::from_slice(trimmed)
                            .map_err(|_| err("Claude sent malformed stream-json"))
                    }
                    Err(e) => Err(e),
                };
                let terminal = value.is_err();
                if (terminal || value.as_ref().is_ok_and(|v: &Value| v["type"] == "result"))
                    && let Some((channel, mission)) = crate::invariants::lock(&lifecycle).as_ref()
                {
                    channel.release(mission);
                }
                match tx.try_send(value) {
                    Ok(()) => {}
                    Err(mpsc::TrySendError::Full(_)) => {
                        reader_control.fail(
                            "Claude notification queue overflow (256 records); terminating run",
                        );
                        break;
                    }
                    Err(mpsc::TrySendError::Disconnected(_)) => break,
                }
                if terminal {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            input,
            control,
            lines,
            normal: ClaudeNormalizer::default(),
            queued: VecDeque::new(),
            stopping: None,
            cancel: None,
            stdin_open: true,
            exited: false,
            exit_code: None,
            approvals: None,
            mission_id: String::new(),
            run_id: String::new(),
            folder: PathBuf::new(),
            mode: Mode::Ask,
            approval_lifecycle,
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
            return Err(err("Claude stdin is closed"));
        }
        match self.input.try_send(StdinMsg::Bytes(bytes)) {
            Ok(()) => Ok(()),
            Err(e) => {
                let message = format!("Claude stdin queue unavailable; terminating run: {e}");
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

    fn receive(&self) -> io::Result<Option<Value>> {
        if let Some(message) = self.control.failure() {
            return Err(err(message));
        }
        match self.lines.recv_timeout(Duration::from_millis(50)) {
            Ok(v) => v.map(Some),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(_) => Err(err("Claude transport disconnected")),
        }
    }

    fn transport_failure(&mut self, message: &str) -> Event {
        self.queued.clear();
        let diagnostic = self.normal.diagnostic("native.transport", message);
        let terminal = self
            .normal
            .finish(self.stopping.is_some() || self.cancelled(), message);
        self.queued.push_back(terminal);
        diagnostic
    }

    fn handle_record(&mut self, v: Value) -> io::Result<()> {
        if v["type"].as_str() == Some("control_request") {
            if let Some(request_id) = v["request_id"].as_str() {
                let behavior = self.answer_control(&v, request_id)?;
                let reply = json!({
                    "type": "control_response",
                    "response": {
                        "subtype": "success",
                        "request_id": request_id,
                        "response": behavior
                    }
                });
                let mut bytes = serde_json::to_vec(&reply)?;
                bytes.push(b'\n');
                let _ = self.send(bytes);
            }
            self.queued.extend(self.normal.accept(&v));
            return Ok(());
        }
        let is_result = v["type"].as_str() == Some("result");
        self.queued.extend(self.normal.accept(&v));
        if is_result {
            // process-runner.ts: close stdin on the turn result so Claude exits.
            self.close_stdin();
        }
        Ok(())
    }

    fn answer_control(&mut self, v: &Value, request_id: &str) -> io::Result<Value> {
        let request = &v["request"];
        let subtype = request["subtype"].as_str().unwrap_or("");
        if subtype != "can_use_tool" {
            return Ok(json!({
                "behavior": "deny",
                "message": "Native declined an unsupported Claude control request."
            }));
        }
        let tool_name = request["tool_name"].as_str().unwrap_or("tool");
        let input = if request["input"].is_null() {
            json!({})
        } else {
            request["input"].clone()
        };
        if self.mode != Mode::ApproveEach || self.approvals.is_none() {
            let message = "Native declines approvals without an interactive Approve each host.";
            self.queued
                .push_back(self.normal.diagnostic("claude.approval_declined", message));
            return Ok(json!({
                "behavior": "deny",
                "message": message
            }));
        }
        let channel = self
            .approvals
            .clone()
            .ok_or_else(|| err("Approval host is unavailable"))?;
        let mission = self.mission_id.clone();
        let run = self.run_id.clone();
        let cwd = self.folder.to_string_lossy().into_owned();
        let cancel = self.cancel.clone();
        let is_cancelled = move || {
            crate::run_cli::interrupted()
                || cancel.as_ref().is_some_and(|f| f.load(Ordering::Relaxed))
        };
        match channel.ask_claude(
            &run,
            &mission,
            request_id,
            tool_name,
            &input,
            &cwd,
            &is_cancelled,
        ) {
            Ok(behavior) => Ok(behavior),
            Err(message) => {
                self.queued
                    .push_back(self.normal.diagnostic("claude.approval", &message));
                Ok(json!({"behavior":"deny","message": message}))
            }
        }
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
}

impl Runtime for Claude {
    fn start(&mut self, turn: &Turn) -> io::Result<()> {
        if turn.prompt.trim().is_empty() {
            return Err(err("Prompt must be nonempty"));
        }
        *crate::invariants::lock(&self.approval_lifecycle) = turn
            .approvals
            .as_ref()
            .map(|ch| (ch.clone(), turn.mission_id.clone()));
        self.approvals = turn.approvals.clone();
        self.mission_id = turn.mission_id.clone();
        self.run_id = turn.run_id.clone();
        self.folder = turn.folder.clone();
        self.mode = turn.mode;
        self.send(user_line(&turn.prompt))
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
            return Ok(Some(
                self.normal
                    .finish(true, "Claude cancellation timed out; process killed"),
            ));
        }
        if let Some(e) = self.queued.pop_front() {
            return Ok(Some(e));
        }
        self.poll_exit();
        if self.exited && self.queued.is_empty() && !self.normal.ended {
            let cancelled = self.stopping.is_some() || self.cancelled();
            let message = match self.exit_code {
                Some(0) => String::new(),
                Some(code) => format!("Claude Code exited with code {code}."),
                None => "Claude Code ended without an exit code.".into(),
            };
            return Ok(Some(self.normal.finish(cancelled, &message)));
        }
        match self.receive() {
            Ok(Some(v)) => {
                if let Err(e) = self.handle_record(v) {
                    return Ok(Some(self.transport_failure(&e.to_string())));
                }
                Ok(self.queued.pop_front())
            }
            Ok(None) => Ok(None),
            Err(e) => {
                self.poll_exit();
                if self.exited {
                    let cancelled = self.stopping.is_some() || self.cancelled();
                    Ok(Some(self.normal.finish(cancelled, &e.to_string())))
                } else {
                    Ok(Some(self.transport_failure(&e.to_string())))
                }
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
        // process-runner.ts:killProcessTree SIGINT, then SIGKILL after grace.
        interrupt_process(self.child.id());
        self.close_stdin();
        Ok(())
    }
}

impl Drop for Claude {
    fn drop(&mut self) {
        self.control.close_and_kill(self.child.id());
        super::process::kill_reap(&mut self.child);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn ask_args_match_read_only_print_command() {
        let args = ask_args(Some("haiku"));
        assert!(args.contains(&"--restricted".into()));
        assert!(args.contains(&"--print".into()));
        assert!(
            args.windows(2)
                .any(|w| w == ["--input-format", "stream-json"])
        );
        assert!(
            args.windows(2)
                .any(|w| w == ["--output-format", "stream-json"])
        );
        assert!(
            args.windows(2)
                .any(|w| w == ["--permission-mode", "manual"])
        );
        assert!(args.windows(2).any(|w| w == ["--tools", ASK_TOOLS]));
        assert!(args.windows(2).any(|w| w == ["--model", "haiku"]));
        assert!(!args.iter().any(|a| a.contains("bypassPermissions")));
    }

    #[test]
    fn approve_each_exposes_writes_without_bypassing_permissions() {
        let c = approve_each_command(Path::new("/tmp"), None).unwrap();
        let args: Vec<_> = c.get_args().map(|s| s.to_str().unwrap()).collect();
        let tools = args[args.iter().position(|s| *s == "--tools").unwrap() + 1];
        assert!(tools.contains("Write") && tools.contains("Bash"));
        assert!(
            args.windows(2)
                .any(|w| w == ["--permission-mode", "manual"])
        );
        assert!(!args.iter().any(|s| s.contains("bypass")));
    }

    #[test]
    fn a_result_with_an_unanswered_request_withdraws_the_card() {
        let mut command = Command::new("python3");
        command.args(["-c", r#"import json,time
print(json.dumps({"type":"control_request","request_id":"dead","request":{"subtype":"can_use_tool","tool_name":"Write","input":{}}}),flush=True)
time.sleep(.1)
print(json.dumps({"type":"result","subtype":"success","result":"ended"}),flush=True)
time.sleep(.1)
"#]);
        let channel = Channel::new();
        let mut runtime = Claude::spawn(&mut command).unwrap();
        let turn = Turn {
            prompt: "hello".into(),
            folder: PathBuf::from("/tmp"),
            model: None,
            effort: None,
            resume: None,
            mode: Mode::ApproveEach,
            approvals: Some(channel.clone()),
            mission_id: "dead".into(),
            run_id: "r".into(),
        };
        runtime.start(&turn).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while !runtime.normal.ended {
            runtime.next_event().unwrap();
            assert!(Instant::now() < deadline);
        }
        assert!(channel.waiting().is_empty());
        assert!(channel.take_recorded().is_empty());
    }

    #[test]
    fn stop_kills_a_child_that_ignores_stdin() {
        let mut command = Command::new("python3");
        command.args(["-c", "import time; time.sleep(60)"]);
        let mut runtime = Claude::spawn(&mut command).unwrap();
        let start = Instant::now();
        runtime.stop().unwrap();
        assert!(start.elapsed() < Duration::from_millis(250));
        loop {
            if runtime.child.try_wait().unwrap().is_some() {
                break;
            }
            assert!(start.elapsed() < Duration::from_secs(6));
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

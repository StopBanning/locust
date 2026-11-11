type ApprovalLifecycle = Arc<Mutex<Option<(Arc<Channel>, String, Option<String>)>>>;
use super::process::Control;
use super::{Event, Runtime, Turn, events::Normalizer};
use crate::approval::{Channel, Mode};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io::{self, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
    },
    time::{Duration, Instant},
};
const MAX_LINE: u64 = 8 * 1024 * 1024;
pub struct Codex {
    child: Child,
    input: SyncSender<Vec<u8>>,
    control: Arc<Control>,
    lines: Receiver<io::Result<Value>>,
    normal: Normalizer,
    queued: VecDeque<Event>,
    request_id: u64,
    stopping: Option<Instant>,
    /// Set by a host (the window) that wants the handshake abandoned; the CLI uses `run_cli::interrupted`.
    cancel: Option<Arc<AtomicBool>>,
    /// A stop was asked before the turn id was known; the interrupt goes out as soon as it is.
    interrupt_owed: bool,
    approvals: Option<Arc<Channel>>,
    mission_id: String,
    run_id: String,
    mode: Mode,
    approval_lifecycle: ApprovalLifecycle,
}

fn err(message: impl ToString) -> io::Error {
    io::Error::other(message.to_string())
}
impl Codex {
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
        let (input, writes) = mpsc::sync_channel::<Vec<u8>>(256);
        let writer_control = control.clone();
        std::thread::spawn(move || {
            for bytes in writes {
                if let Err(e) = stdin.write_all(&bytes).and_then(|_| stdin.flush()) {
                    writer_control.fail(format!("Codex stdin write failed: {e}"));
                    break;
                }
            }
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
                    Ok(0) => Err(err("Codex stdout closed before the turn completed")),
                    Ok(_) if line.len() as u64 > MAX_LINE => {
                        Err(err("Codex JSON-RPC line exceeds 8 MiB"))
                    }
                    Ok(_) => serde_json::from_slice(&line)
                        .map_err(|_| err("Codex sent malformed JSON-RPC")),
                    Err(e) => Err(e),
                };
                let terminal = value.is_err();
                if let Some((channel, mission, thread)) =
                    crate::invariants::lock(&lifecycle).as_ref()
                {
                    let own_end = value.as_ref().is_ok_and(|v: &Value| {
                        v["method"] == "turn/completed"
                            && thread.as_ref().is_some_and(|id| {
                                super::events::thread_id(&v["params"]).as_ref() == Some(id)
                            })
                    });
                    if terminal || own_end {
                        channel.release(mission);
                    }
                }
                match tx.try_send(value) {
                    Ok(()) => {}
                    Err(mpsc::TrySendError::Full(_)) => {
                        reader_control.fail(
                            "Codex notification queue overflow (256 records); terminating run",
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
            normal: Normalizer::default(),
            queued: VecDeque::new(),
            request_id: 0,
            stopping: None,
            cancel: None,
            interrupt_owed: false,
            approvals: None,
            mission_id: String::new(),
            run_id: String::new(),
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
    fn send(&mut self, v: Value) -> io::Result<()> {
        let mut bytes = serde_json::to_vec(&v)?;
        bytes.push(b'\n');
        match self.input.try_send(bytes) {
            Ok(()) => Ok(()),
            Err(e) => {
                let message = format!("Codex stdin queue unavailable; terminating run: {e}");
                self.control.fail(&message);
                Err(err(message))
            }
        }
    }
    fn receive(&self) -> io::Result<Option<Value>> {
        if let Some(message) = self.control.failure() {
            return Err(err(message));
        }
        match self.lines.recv_timeout(Duration::from_millis(50)) {
            Ok(v) => v.map(Some),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(_) => Err(err("Codex transport disconnected")),
        }
    }
    fn transport_failure(&mut self, message: &str) -> Event {
        self.queued.clear();
        let diagnostic = self.normal.diagnostic("native.transport", message);
        let terminal = self.normal.finish(self.stopping.is_some(), message);
        self.queued.push_back(terminal);
        diagnostic
    }
    fn notification(&mut self, v: Value) -> io::Result<()> {
        let Some(method) = v["method"].as_str() else {
            return Ok(());
        };
        if !v["id"].is_null() {
            let method_owned = method.to_owned();
            let wire_id = v["id"].clone();
            let params = v["params"].clone();
            let result = match method_owned.as_str() {
                "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
                    self.answer_approval(&method_owned, &params, wire_id.clone())?
                }
                "execCommandApproval" | "applyPatchApproval" => json!({"decision":"denied"}),
                _ => {
                    self.send(json!({"jsonrpc":"2.0","id":wire_id,"error":{"code":-32601,"message":"Native does not support this interactive request"}}))?;
                    self.queued.push_back(self.normal.diagnostic(
                        &method_owned,
                        "Codex requested an unsupported interaction; native declined it.",
                    ));
                    return Ok(());
                }
            };
            self.send(json!({"jsonrpc":"2.0","id":wire_id,"result":result}))?;
        } else {
            self.queued.extend(self.normal.accept(method, &v["params"]));
        }
        Ok(())
    }
    fn answer_approval(
        &mut self,
        method: &str,
        params: &Value,
        wire_id: Value,
    ) -> io::Result<Value> {
        if super::events::thread_id(params)
            .is_some_and(|id| self.normal.thread.as_ref() != Some(&id))
        {
            return Ok(json!({"decision":"reject"}));
        }
        // Only Approve-each stops for a card (`codexAppServerPolicy` → untrusted).
        // Ask / other modes auto-decline the way M1 did, even when a channel exists.
        if self.mode != Mode::ApproveEach || self.approvals.is_none() {
            self.queued.push_back(self.normal.diagnostic(
                method,
                "Codex requested approval; native declines approvals outside Approve each.",
            ));
            return Ok(json!({"decision":"decline"}));
        }
        let channel = self
            .approvals
            .clone()
            .ok_or_else(|| err("Approval host is unavailable"))?;
        let mission = self.mission_id.clone();
        let run = self.run_id.clone();
        let cancel = self.cancel.clone();
        let is_cancelled = move || {
            crate::run_cli::interrupted()
                || cancel.as_ref().is_some_and(|f| f.load(Ordering::Relaxed))
        };
        match channel.ask_codex(&run, &mission, method, params, wire_id, &is_cancelled) {
            Ok(result) => Ok(result),
            Err(message) => {
                self.queued
                    .push_back(self.normal.diagnostic(method, &message));
                Ok(json!({"decision":"reject"}))
            }
        }
    }
    fn request(&mut self, method: &str, params: Value) -> io::Result<Value> {
        self.request_id += 1;
        let id = self.request_id;
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            if Instant::now() >= deadline {
                return Err(err(format!("Codex timed out answering {method}")));
            }
            if self.cancelled() {
                return Err(err("Interrupted before the turn started"));
            }
            let Some(v) = self.receive()? else { continue };
            if v["id"] == id && v["method"].is_null() {
                if !v["error"].is_null() {
                    return Err(err(v["error"]["message"]
                        .as_str()
                        .unwrap_or("Codex refused request")));
                }
                return Ok(v["result"].clone());
            }
            self.notification(v)?;
        }
    }
}
pub fn read_only_config(settings: &Value) -> io::Result<Value> {
    if !settings.is_object() {
        return Err(err(
            "Could not read Codex settings; refusing to start Ask with unknown MCP programs",
        ));
    }
    let mut config = json!({"features.plugins":false,"features.browser_use":false,"features.browser_use_external":false,"features.in_app_browser":false,"features.computer_use":false});
    if !settings["mcp_servers"].is_null() {
        let servers = settings["mcp_servers"]
            .as_object()
            .ok_or_else(|| err("Invalid Codex MCP settings"))?;
        for (name, fields) in servers {
            if fields["url"].is_string() && fields.get("command").is_none() {
                continue;
            }
            if name.is_empty()
                || name.len() > 128
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"@_-".contains(&b))
            {
                return Err(err(
                    "Cannot safely disable a Codex MCP program with an invalid name",
                ));
            }
            config[format!("mcp_servers.{name}.enabled")] = json!(false);
        }
    }
    Ok(config)
}
impl Runtime for Codex {
    fn start(&mut self, turn: &Turn) -> io::Result<()> {
        *crate::invariants::lock(&self.approval_lifecycle) = turn
            .approvals
            .as_ref()
            .map(|ch| (ch.clone(), turn.mission_id.clone(), None));
        self.approvals = turn.approvals.clone();
        self.mission_id = turn.mission_id.clone();
        self.run_id = turn.run_id.clone();
        self.mode = turn.mode;
        self.request(
            "initialize",
            json!({"clientInfo":{"name":"locust","version":"0.1.0"}}),
        )?;
        self.send(json!({"jsonrpc":"2.0","method":"initialized"}))?;
        let settings = self.request("config/read", json!({"cwd":turn.folder}))?;
        let mut config = read_only_config(&settings["config"])?;
        if let Some(browser) = crate::browser_tools::config(&turn.mission_id) {
            config["mcp_servers.locust_browser"] = browser;
            config["mcp_servers.locust_browser.enabled"] = json!(true);
        }
        let policy = crate::approval::codex_app_server_policy(turn.mode, None);
        // Ports codex-app-server-run.ts handshake: `thread/start`, or `thread/resume` with the prior thread id and
        // `excludeTurns` (without it the answer carries the whole history as one line, past the 8 MiB limit).
        let result = match &turn.resume {
            None => self.request("thread/start",json!({"cwd":turn.folder,"sandbox":policy.sandbox,"approvalPolicy":policy.approval_policy,"config":config}))?,
            Some(prior) => {
                let mut params = json!({"threadId":prior,"excludeTurns":true,"cwd":turn.folder,"sandbox":policy.sandbox,"approvalPolicy":policy.approval_policy,"config":config});
                // The model on the THREAD too (0.864): reopening on the turn's own model leaves nothing to warn about.
                if let Some(model) = turn.model.as_deref().filter(|m| !m.trim().is_empty()) {
                    params["model"] = json!(model);
                }
                self.request("thread/resume", params)?
            }
        };
        let thread = result["thread"]["id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| err("Codex did not return a thread ID"))?
            .to_owned();
        self.normal.thread = Some(thread.clone());
        if let Some((_, _, id)) = crate::invariants::lock(&self.approval_lifecycle).as_mut() {
            *id = Some(thread.clone());
        }
        let mut params = json!({"threadId":thread,"approvalPolicy":policy.approval_policy,"input":[{"type":"text","text":turn.prompt}],"summary":"detailed"});
        if let Some(model) = &turn.model {
            params["model"] = json!(model);
        }
        // Only a plain word is passed as effort: a malformed one is refused by the server and takes the turn with it.
        if let Some(effort) = turn
            .effort
            .as_deref()
            .filter(|e| (1..=16).contains(&e.len()) && e.bytes().all(|b| b.is_ascii_lowercase()))
        {
            params["effort"] = json!(effort);
        }
        let result = self.request("turn/start", params)?;
        self.normal.turn = result["turn"]["id"]
            .as_str()
            .map(str::to_owned)
            .or(self.normal.turn.clone());
        Ok(())
    }
    fn next_event(&mut self) -> io::Result<Option<Event>> {
        if self.normal.ended {
            return Ok(self.queued.pop_front());
        }
        // Failure and cancellation preempt queued deltas, even during a flood.
        if let Some(message) = self.control.failure() {
            return Ok(Some(self.transport_failure(&message)));
        }
        if self
            .stopping
            .is_some_and(|t| t.elapsed() >= Duration::from_secs(5))
        {
            self.queued.clear();
            return Ok(Some(
                self.normal
                    .finish(true, "Codex cancellation timed out; process killed"),
            ));
        }
        if self.stopping.is_some() {
            self.send_interrupt();
        }
        if let Some(e) = self.queued.pop_front() {
            return Ok(Some(e));
        }
        match self.receive() {
            Ok(Some(v)) => {
                if let Err(e) = self.notification(v) {
                    return Ok(Some(self.transport_failure(&e.to_string())));
                }
                Ok(self.queued.pop_front())
            }
            Ok(None) => Ok(None),
            Err(e) => Ok(Some(self.transport_failure(&e.to_string()))),
        }
    }
    fn stop(&mut self) -> io::Result<()> {
        if self.stopping.is_some() {
            return Ok(());
        }
        let now = Instant::now();
        self.stopping = Some(now);
        self.control.cancel_at(now + Duration::from_secs(5));
        self.interrupt_owed = true;
        self.send_interrupt();
        Ok(())
    }
}
impl Codex {
    fn send_interrupt(&mut self) {
        if !self.interrupt_owed {
            return;
        }
        if let (Some(thread), Some(turn)) = (self.normal.thread.clone(), self.normal.turn.clone()) {
            self.interrupt_owed = false;
            self.request_id += 1;
            // A queue failure is recorded by next_event; it must not bypass the terminal ledger event.
            let _ = self.send(json!({"jsonrpc":"2.0","id":self.request_id,"method":"turn/interrupt","params":{"threadId":thread,"turnId":turn}}));
        }
    }
}
impl Drop for Codex {
    fn drop(&mut self) {
        self.control.close_and_kill(self.child.id());
        super::process::kill_reap(&mut self.child);
    }
}
pub fn command() -> io::Result<Command> {
    let mut c = Command::new(super::discovery::find()?);
    c.arg("app-server");
    Ok(c)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn stop_and_watchdog_survive_blocked_stdin_without_event_polling() {
        let mut command = Command::new("python3");
        command.args(["-c", "import time; time.sleep(60)"]);
        let mut runtime = Codex::spawn(&mut command).unwrap();
        runtime.normal.thread = Some("thread".into());
        runtime.normal.turn = Some("turn".into());
        // Much larger than the pipe; writer is stuck because the server never reads.
        runtime
            .send(json!({"payload":"x".repeat(1024 * 1024)}))
            .unwrap();
        std::thread::sleep(Duration::from_millis(100));
        let start = Instant::now();
        runtime.stop().unwrap();
        assert!(start.elapsed() < Duration::from_millis(250));
        // No next_event calls: cancellation must kill independently of both queues.
        loop {
            if runtime.child.try_wait().unwrap().is_some() {
                break;
            }
            assert!(
                start.elapsed() < Duration::from_secs(6),
                "watchdog failed to kill child"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        let mut event = runtime.next_event().unwrap().unwrap();
        if event.kind == "adapter.diagnostic" {
            event = runtime.next_event().unwrap().unwrap();
        }
        assert_eq!(event.kind, "run.cancelled");
    }
    #[test]
    fn stdout_overflow_terminates_without_consumer_draining() {
        let mut command = Command::new("python3");
        command.args(["-c", "import json; exec(\"while True: print(json.dumps({'method':'delta','params':{}}), flush=True)\")"]);
        let mut runtime = Codex::spawn(&mut command).unwrap();
        let start = Instant::now();
        while runtime.child.try_wait().unwrap().is_none() {
            assert!(
                start.elapsed() < Duration::from_secs(3),
                "overflow did not kill producer"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
        let event = runtime.next_event().unwrap().unwrap();
        assert_eq!(event.kind, "adapter.diagnostic");
        assert!(
            event
                .payload
                .to_string()
                .contains("notification queue overflow")
        );
        assert_eq!(runtime.next_event().unwrap().unwrap().kind, "run.failed");
    }
}

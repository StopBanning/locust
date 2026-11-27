//! Agent Client Protocol over stdio: acp-run.ts, with no filesystem or terminal offer.
use super::process::kill;
use super::{Event, Runtime, Turn, acp_events::AcpNormalizer};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io::{self, BufReader, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::{Duration, Instant},
};
const DEFAULT_MAX_QUEUED_RECORDS: usize = 20_000;
/// Said when the agent's records outran the queue: acp-run.ts `lose`, in Electron's words, as Claude Code's and Codex's
/// readers say it.
pub const OVERFLOW_MESSAGE: &str = "The agent sent output faster than Locust could record it, so the run was stopped rather than leave a gap in its record. Sending it again usually works.";

fn err(s: impl ToString) -> io::Error {
    io::Error::other(s.to_string())
}

/// commandWords: double quotes group arguments; no shell is ever invoked.
pub fn command_words(line: &str) -> io::Result<Vec<String>> {
    let mut words = vec![];
    let mut word = String::new();
    let mut quoted = false;
    let mut started = false;
    if line.encode_utf16().count() > 1000 {
        return Err(err("ACP command is too long"));
    }
    for c in line.trim().chars() {
        if c == '"' {
            quoted = !quoted;
            started = true;
            continue;
        }
        if !quoted && "|&;<>`$".contains(c) {
            return Err(err("ACP commands do not run through a shell"));
        }
        if !quoted && c.is_whitespace() {
            if started {
                words.push(std::mem::take(&mut word));
            }
            started = false;
        } else {
            word.push(c);
            started = true;
        }
    }
    if quoted {
        return Err(err("ACP command has an unclosed quote"));
    }
    if started {
        words.push(word);
    }
    let first = words
        .first()
        .ok_or_else(|| err("ACP command is empty"))?
        .to_lowercase();
    if matches!(
        first.as_str(),
        "cmd"
            | "cmd.exe"
            | "powershell"
            | "powershell.exe"
            | "pwsh"
            | "pwsh.exe"
            | "bash"
            | "sh"
            | "zsh"
    ) {
        return Err(err("ACP commands must name a program, not a shell"));
    }
    Ok(words)
}
/// Read only the saved agent description, never an agent's login/configuration files.
pub fn command(profile: &Path, folder: &Path, route: Option<&str>) -> io::Result<Command> {
    let route = route.ok_or_else(|| err("Choose a configured ACP agent"))?;
    let id = route.split_once("::").map_or(route, |(id, _)| id);
    let file = std::fs::File::open(profile.join("acp-agents.json"))?;
    let mut bytes = vec![];
    file.take(1_000_001).read_to_end(&mut bytes)?;
    if bytes.len() > 1_000_000 {
        return Err(err("ACP agent list exceeds 1 MB"));
    }
    let config: Value = serde_json::from_slice(&bytes)?;
    let mut taken = std::collections::HashSet::new();
    let mut count = 0;
    for a in config["agents"].as_array().into_iter().flatten() {
        let (Some(aid), Some(name), Some(line)) = (
            a["id"].as_str(),
            a["name"].as_str(),
            a["commandLine"].as_str(),
        ) else {
            continue;
        };
        let valid_id = aid.strip_prefix("acp_").is_some_and(|s| {
            !s.is_empty()
                && s.len() <= 30
                && s.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        });
        let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
        let Ok(words) = command_words(line) else {
            continue;
        };
        if !valid_id || name.is_empty() || name.encode_utf16().count() > 40 || !taken.insert(aid) {
            continue;
        }
        count += 1;
        if count > 12 {
            break;
        }
        if aid != id {
            continue;
        }
        let path = if Path::new(&words[0]).is_absolute() {
            if cfg!(windows) && !words[0].to_lowercase().ends_with(".exe") {
                return Err(err(
                    "On Windows, an absolute ACP program path must end in .exe",
                ));
            }
            words[0].clone().into()
        } else {
            super::discovery::find_named(&words[0])?
        };
        let mut c = Command::new(path);
        c.args(&words[1..]).current_dir(folder);
        if let Some((_, model)) = route.split_once("::").filter(|(_, m)| !m.is_empty())
            && let Some(option) = a["answer"]["modelOption"]["id"]
                .as_str()
                .filter(|s| !s.trim().is_empty() && s.len() <= 2048)
        {
            // Host-only metadata removed before spawning; not a flag or an agent environment key.
            c.env("LOCUST_NATIVE_ACP_MODEL_OPTION", option)
                .env("LOCUST_NATIVE_ACP_MODEL_VALUE", model);
        }
        return Ok(c);
    }
    Err(err("The configured ACP agent was not found"))
}

/// The value to ask for, given what the agent lists for a setting (acp-run.ts `resolveConfigValue`).
pub type ResolveConfig = Arc<dyn Fn(&str, &str, &[String]) -> String + Send + Sync>;

#[derive(Clone)]
pub struct Profile {
    pub runtime: String,
    pub required: Vec<(String, String)>,
    pub strict: bool,
    pub deadline: Option<Instant>,
    pub named: bool,
    /// When false (Kilo), Locust never raises a card; only Auto allows.
    pub host_permissions: bool,
    /// Settings set only when the agent offers them and lists the value, after the required ones (acp-run.ts
    /// `preferredConfig`): agy 1.5.18 offers Flash no `xhigh` and an Orchestrator no effort at all.
    pub preferred: Vec<(String, String)>,
    pub resolve: Option<ResolveConfig>,
}
impl Default for Profile {
    fn default() -> Self {
        Self {
            runtime: String::new(),
            required: vec![],
            strict: false,
            deadline: None,
            named: false,
            host_permissions: true,
            preferred: vec![],
            resolve: None,
        }
    }
}

/// The values the agent lists for a setting; none when it does not offer it.
fn config_choices(session: &Value, id: &str) -> Vec<String> {
    session["configOptions"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|c| c["id"] == id)
        .and_then(|c| c["options"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|o| o["value"].as_str().map(str::to_owned))
        .collect()
}
pub const CANCEL_GRACE: Duration = Duration::from_secs(3);
const STREAM_CLOSED: &str = "The agent's output stream closed before its turn finished.";
const AGENT_EXITED: &str = "The agent exited before its turn finished.";
const STREAM_FAILED: &str = "The agent's output stream failed before its turn finished.";

/// One line of the agent's output, as the reader keeps it: a JSON object, the end of the stream (closed, or failed
/// without needing an exit, acp-run.ts `onStdoutClose`), or nothing to keep (a blank or non-object line).
fn record_from(read: io::Result<usize>, line: &[u8]) -> Option<io::Result<Value>> {
    match read {
        Ok(0) => Some(Err(err(STREAM_CLOSED))),
        Ok(_) if line.len() > 8 * 1024 * 1024 => Some(Err(err("ACP line exceeds 8 MiB"))),
        Ok(_) if line.iter().all(u8::is_ascii_whitespace) => None,
        Ok(_) => match serde_json::from_slice::<Value>(line) {
            Ok(v) if v.is_object() => Some(Ok(v)),
            _ => None,
        },
        Err(_) => Some(Err(err(STREAM_FAILED))),
    }
}

pub struct Acp {
    child: Child,
    profile: Profile,
    admitted: bool,
    reaped: bool,
    input: SyncSender<Vec<u8>>,
    lines: Receiver<io::Result<Value>>,
    /// Records read off `lines` with a handshake answer, still to be handled in order.
    held: VecDeque<io::Result<Value>>,
    failure: Arc<Mutex<Option<String>>>,
    gone: Arc<AtomicBool>,
    prompt_id: Arc<AtomicU64>,
    answered: Arc<AtomicBool>,
    closed: Arc<AtomicBool>,
    reap_lock: Arc<Mutex<()>>,
    cancel: Option<Arc<AtomicBool>>,
    normal: AcpNormalizer,
    queued: VecDeque<Event>,
    id: u64,
    stopping: Option<Instant>,
    cancel_grace: Duration,
    model_config: Option<(String, String)>,
    turn: Option<Turn>,
    declined: bool,
    steering: Vec<String>,
    usage: serde_json::Map<String, Value>,
    stderr: Arc<Mutex<String>>,
    prompt_started: Option<Instant>,
}
impl Acp {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        let option = command
            .get_envs()
            .find(|(k, _)| *k == "LOCUST_NATIVE_ACP_MODEL_OPTION")
            .and_then(|(_, v)| v)
            .map(|v| v.to_string_lossy().into_owned());
        let value = command
            .get_envs()
            .find(|(k, _)| *k == "LOCUST_NATIVE_ACP_MODEL_VALUE")
            .and_then(|(_, v)| v)
            .map(|v| v.to_string_lossy().into_owned());
        command
            .env_remove("LOCUST_NATIVE_ACP_MODEL_OPTION")
            .env_remove("LOCUST_NATIVE_ACP_MODEL_VALUE");
        let mut child = super::process::spawn(command, Stdio::piped(), Stdio::piped())?;
        let failure = Arc::new(Mutex::new(None));
        let gone = Arc::new(AtomicBool::new(false));
        let closed = Arc::new(AtomicBool::new(false));
        let stderr = super::process::stderr_tail(
            child
                .stderr
                .take()
                // Invariant: spawn configured this pipe and it has not been taken.
                .expect("spawn configured this pipe and it has not been taken"),
        );
        // spawn configured a piped stdin; no prior take occurs.
        let mut stdin = child
            .stdin
            .take()
            // Invariant: spawn configured this pipe and it has not been taken.
            .expect("spawn configured this pipe and it has not been taken");
        let (input, writes) = mpsc::sync_channel::<Vec<u8>>(256);
        let fail = failure.clone();
        let pid = child.id();
        std::thread::spawn(move || {
            for bytes in writes {
                if let Err(e) = stdin.write_all(&bytes).and_then(|_| stdin.flush()) {
                    *crate::invariants::lock(&fail) = Some(format!("ACP stdin failed: {e}"));
                    kill(pid);
                    break;
                }
            }
        });
        let prompt_id = Arc::new(AtomicU64::new(0));
        let answered = Arc::new(AtomicBool::new(false));
        let observed_prompt = prompt_id.clone();
        let observed_answer = answered.clone();
        let stdout = child
            .stdout
            .take()
            // Invariant: spawn configured this pipe and it has not been taken.
            .expect("spawn configured this pipe and it has not been taken");
        let (tx, lines) = mpsc::sync_channel(DEFAULT_MAX_QUEUED_RECORDS);
        let fail = failure.clone();
        let dead = gone.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = vec![];
                let read = super::process::read_line(&mut reader, &mut line);
                let Some(record) = record_from(read, &line) else { continue };
                if record.as_ref().is_ok_and(|v| {
                    v["method"].is_null()
                        && v["id"].as_u64().is_some_and(|id| {
                            id != 0 && id == observed_prompt.load(Ordering::Relaxed)
                        })
                }) {
                    observed_answer.store(true, Ordering::Relaxed);
                }
                let terminal = record.is_err();
                if terminal {
                    dead.store(true, Ordering::Relaxed);
                }
                match tx.try_send(record) {
                    Ok(()) => {}
                    Err(mpsc::TrySendError::Full(_)) => {
                        *crate::invariants::lock(&fail) = Some(OVERFLOW_MESSAGE.into());
                        dead.store(true, Ordering::Relaxed);
                        kill(pid);
                        break;
                    }
                    Err(_) => break,
                }
                if terminal {
                    break;
                }
            }
        });
        let mut normal = AcpNormalizer::default();
        normal.started_at = crate::ledger_write::now();
        Ok(Self {
            child,
            profile: Profile::default(),
            admitted: false,
            reaped: false,
            input,
            lines,
            held: VecDeque::new(),
            failure,
            gone,
            prompt_id,
            answered,
            closed,
            reap_lock: Arc::new(Mutex::new(())),
            cancel: None,
            normal,
            queued: VecDeque::new(),
            id: 0,
            stopping: None,
            cancel_grace: CANCEL_GRACE,
            model_config: option.zip(value),
            turn: None,
            declined: false,
            steering: vec![],
            usage: serde_json::Map::new(),
            stderr,
            prompt_started: None,
        })
    }
    pub fn set_profile(&mut self, profile: Profile) {
        self.profile = profile;
    }
    pub fn admitted(&self) -> bool {
        self.admitted
    }
    pub fn confirm_exit(&mut self, timeout: Duration) -> bool {
        let _guard = crate::invariants::lock(&self.reap_lock);
        self.closed.store(true, Ordering::Relaxed);
        if self.reaped {
            return true;
        }
        kill(self.child.id());
        let _ = self.child.kill();
        self.reaped = super::process::confirm_exit(&mut self.child, timeout);
        self.reaped
    }
    pub fn startup(&mut self, t: &Turn) -> io::Result<()> {
        self.begin(t)
    }

    pub fn with_cancel(mut self, flag: Arc<AtomicBool>) -> Self {
        self.cancel = Some(flag);
        self
    }
    pub fn with_cancel_grace(mut self, grace: Duration) -> Self {
        self.cancel_grace = grace;
        self
    }
    fn cancelled(&self) -> bool {
        crate::run_cli::interrupted()
            || self
                .cancel
                .as_ref()
                .is_some_and(|f| f.load(Ordering::Relaxed))
    }
    fn send(&self, v: Value) -> io::Result<()> {
        let mut bytes = serde_json::to_vec(&v)?;
        bytes.push(b'\n');
        self.input
            .try_send(bytes)
            .map_err(|_| err("ACP stdin queue unavailable"))
    }
    /// Whether the agent has exited, given `budget` to do it: its output closing is read as its exit when it did
    /// (acp-run.ts says "The agent exited before its turn finished." on exit, and the stream's own words when only
    /// the stream closed). Reaped here under the reap lock, so nothing signals its PID again.
    fn exited_within(&mut self, budget: Duration) -> bool {
        let deadline = Instant::now() + budget;
        loop {
            {
                let _guard = crate::invariants::lock(&self.reap_lock);
                if self.reaped {
                    return true;
                }
                if let Ok(Some(_)) = self.child.try_wait() {
                    self.reaped = true;
                    self.closed.store(true, Ordering::Relaxed);
                    return true;
                }
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    fn receive(&mut self) -> io::Result<Option<Value>> {
        if let Some(e) = crate::invariants::lock(&self.failure).clone() {
            return Err(err(e));
        }
        let next = match self.held.pop_front() {
            Some(record) => Ok(record),
            None => self.lines.recv_timeout(Duration::from_millis(50)),
        };
        match next {
            Ok(Err(e)) => Err(self.lost(e)),
            Ok(Ok(v)) => Ok(Some(v)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "ACP transport disconnected",
            )),
        }
    }
    /// A closed stream is read as the agent's exit when it did exit; either way with its stderr after the words.
    fn lost(&mut self, e: io::Error) -> io::Error {
        if e.to_string() == STREAM_CLOSED && self.exited_within(Duration::from_millis(500)) {
            self.with_stderr(err(AGENT_EXITED))
        } else {
            self.with_stderr(e)
        }
    }
    /// What the agent wrote with an answer, read before the handshake takes its next step: an answer followed by the
    /// end of its output begins nothing more (acp-run.ts reads the whole chunk, EOF included, before session/new).
    fn ended_with_answer(&mut self) -> Option<io::Error> {
        while let Ok(record) = self.lines.try_recv() {
            match record {
                Err(e) => return Some(self.lost(e)),
                Ok(v) => self.held.push_back(Ok(v)),
            }
        }
        None
    }
    /// A lost transport's words, with what the agent last wrote to stderr after them.
    fn with_stderr(&self, e: io::Error) -> io::Error {
        let tail = crate::invariants::lock(&self.stderr);
        if tail.trim().is_empty() {
            io::Error::new(io::ErrorKind::ConnectionAborted, e)
        } else {
            io::Error::new(io::ErrorKind::ConnectionAborted, format!("{e} {}", tail.trim()))
        }
    }
    fn request(&mut self, method: &str, params: Value, deadline: Instant) -> io::Result<Value> {
        // A run stopped, or an agent gone, before the next step of the handshake is sent nothing more.
        if self.cancelled() {
            return Err(err("ACP startup stopped"));
        }
        if self.gone.load(Ordering::Relaxed) {
            return Err(err(STREAM_CLOSED));
        }
        self.id += 1;
        let id = self.id;
        self.send(json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))?;
        loop {
            if self.cancelled() {
                return Err(err("ACP startup stopped"));
            }
            if Instant::now() >= deadline {
                return Err(err("The agent did not answer in time."));
            }
            let Some(v) = self.receive()? else { continue };
            if v["method"].is_string() {
                self.handle(v, false)?;
                continue;
            }
            if v["id"] != id {
                continue;
            }
            if !v["error"].is_null() {
                return Err(err(v["error"]["message"]
                    .as_str()
                    .unwrap_or("The agent refused the request")));
            }
            if let Some(e) = self.ended_with_answer() {
                return Err(e);
            }
            return Ok(v["result"].clone());
        }
    }
    fn handle(&mut self, v: Value, recording: bool) -> io::Result<()> {
        if v["method"] == "session/update" && v["id"].is_null() {
            if recording
                && v["params"]["sessionId"]
                    .as_str()
                    .is_none_or(|s| Some(s) == self.normal.thread.as_deref())
            {
                self.queued
                    .extend(self.normal.update(&v["params"]["update"]));
            }
            return Ok(());
        }
        if v["id"].is_null() || !v["method"].is_string() {
            return Ok(());
        }
        let id = v["id"].clone();
        if v["method"] != "session/request_permission" {
            return self.send(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Locust offers this agent no files and no terminal."}}));
        }
        let t = self
            .turn
            .as_ref()
            .ok_or_else(|| err("Turn has not started"))?;
        let params = &v["params"];
        let allow = if self.profile.host_permissions
            && (!self.profile.named || t.mode == crate::approval::Mode::ApproveEach)
        {
            if let Some(ch) = &t.approvals {
                let cancel = self.cancel.clone();
                let gone = self.gone.clone();
                let answered = self.answered.clone();
                ch.ask_acp_runtime(
                    if self.profile.runtime.is_empty() {
                        "acp"
                    } else {
                        &self.profile.runtime
                    },
                    &t.run_id,
                    &t.mission_id,
                    params,
                    &t.folder.to_string_lossy(),
                    &|| {
                        crate::run_cli::interrupted()
                            || gone.load(Ordering::Relaxed)
                            || answered.load(Ordering::Relaxed)
                            || cancel.as_ref().is_some_and(|f| f.load(Ordering::Relaxed))
                    },
                )
                .unwrap_or(false)
            } else {
                false
            }
        } else {
            t.mode == crate::approval::Mode::Auto
        };
        let chosen = params["options"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|o| o["kind"] == if allow { "allow_once" } else { "reject_once" })
            .and_then(|o| o["optionId"].as_str());
        if !allow || chosen.is_none() {
            self.declined = true;
            if let Some(tool) = params["toolCall"]["toolCallId"].as_str() {
                self.normal.decline(tool);
            }
        }
        let outcome = if self.cancelled()
            || self.gone.load(Ordering::Relaxed)
            || self.answered.load(Ordering::Relaxed)
        {
            json!({"outcome":"cancelled"})
        } else if let Some(option) = chosen {
            json!({"outcome":"selected","optionId":option})
        } else {
            json!({"outcome":"cancelled"})
        };
        // An agent whose output has ended is told nothing: no answer to a question it can no longer act on.
        if self.gone.load(Ordering::Relaxed) {
            return Ok(());
        }
        self.send(json!({"jsonrpc":"2.0","id":id,"result":{"outcome":outcome}}))
    }
    /// ACP cannot add to an active prompt; steering waits for the next prompt boundary.
    pub fn steer(&mut self, text: &str) -> bool {
        if self.normal.ended
            || self.stopping.is_some()
            || self.cancelled()
            || text.trim().is_empty()
        {
            return false;
        }
        self.steering.push(text.to_owned());
        true
    }
    fn send_prompt(&mut self, prompt: &str) -> io::Result<()> {
        self.id += 1;
        self.prompt_started = Some(Instant::now());
        self.prompt_id.store(self.id, Ordering::Relaxed);
        self.answered.store(false, Ordering::Relaxed);
        self.declined = false;
        self.admitted = true;
        self.send(json!({"jsonrpc":"2.0","id":self.id,"method":"session/prompt","params":{"sessionId":self.normal.thread,"prompt":[{"type":"text","text":prompt}]}}))
    }
    fn prompt_result(&mut self, result: &Value) {
        let stop = result["stopReason"].as_str().unwrap_or("");
        self.queued.extend(self.normal.prompt_boundary(stop));
        for (k, v) in result["usage"].as_object().into_iter().flatten() {
            if let Some(n) = v.as_f64() {
                if !self.usage.contains_key(k) && self.usage.len() >= 32 {
                    continue;
                }
                let total = self.usage.get(k).and_then(Value::as_f64).unwrap_or(0.) + n;
                if total.is_finite() {
                    self.usage.insert(k.clone(), json!(total));
                }
            }
        }
        let declined_end = stop == "cancelled" && self.declined;
        if !self.cancelled()
            && self.stopping.is_none()
            && (declined_end || matches!(stop, "end_turn" | "max_tokens" | "max_turn_requests"))
            && !self.steering.is_empty()
        {
            let prompt = std::mem::take(&mut self.steering).join("\n\n");
            if let Err(e) = self.send_prompt(&prompt) {
                self.finish("", Value::Null, Some(&e.to_string()));
            }
        } else {
            self.finish(stop, Value::Object(self.usage.clone()), None);
        }
    }
    fn finish(&mut self, stop: &str, usage: Value, failure: Option<&str>) {
        self.queued.extend(self.normal.finish(
            stop,
            usage,
            self.stopping.is_some() || self.cancelled(),
            failure,
            self.declined,
        ));
        self.closed.store(true, Ordering::Relaxed);
        if !self.reaped {
            kill(self.child.id());
            let _ = self.child.kill();
        }
        // Keep the child unreaped until Drop: its PID cannot be reused by the stop watchdog.
        if let Some(t) = &self.turn
            && let Some(ch) = &t.approvals
        {
            ch.release(&t.mission_id);
        }
    }
    fn begin(&mut self, t: &Turn) -> io::Result<()> {
        if self.turn.is_some() {
            return Err(err("ACP turn already started"));
        }
        if t.prompt.trim().is_empty() {
            return Err(err("Prompt must be nonempty"));
        }
        self.turn = Some(Turn {
            prompt: t.prompt.clone(),
            folder: t.folder.clone(),
            model: t.model.clone(),
            effort: t.effort.clone(),
            resume: t.resume.clone(),
            mode: t.mode,
            approvals: t.approvals.clone(),
            mission_id: t.mission_id.clone(),
            run_id: t.run_id.clone(),
        });
        let deadline = self
            .profile
            .deadline
            .unwrap_or_else(|| Instant::now() + Duration::from_secs(60));
        let init=self.request("initialize",json!({"protocolVersion":1,"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false}}),deadline)?;
        if init["protocolVersion"] != 1 {
            return Err(err(format!(
                "The agent speaks version {} of the Agent Client Protocol, and Locust speaks 1.",
                init["protocolVersion"]
            )));
        }
        // What it can do is shown in Settings > AI agents; keeping it never stops the turn (acp-run.ts).
        crate::acp_capabilities::record_said(&self.profile.runtime, &init["agentCapabilities"]);
        let mut params = json!({"cwd":t.folder,"mcpServers":[]});
        if let Some(browser) = crate::browser_tools::config(&t.mission_id) {
            params["mcpServers"] = json!([{"name":"locust_browser","command":browser["command"],"args":browser["args"],"env":[]}]);
        }
        let session = if let Some(id) = &t.resume {
            if init["agentCapabilities"]["loadSession"] != true {
                return Err(err("The agent cannot continue an earlier session."));
            }
            params["sessionId"] = json!(id);
            self.normal.thread = Some(id.clone());
            self.request("session/load", params, deadline)?
        } else {
            let s = self.request("session/new", params, deadline)?;
            self.normal.thread = s["sessionId"]
                .as_str()
                .filter(|s| !s.trim().is_empty() && s.len() <= 2048)
                .map(str::to_owned);
            s
        };
        if self.normal.thread.is_none() {
            return Err(err("The agent did not start a session."));
        }
        self.normal.records += 1;
        // Persist identity even if startup fails after this point, without replaying the session's text.
        self.queued.push_back(self.normal.event("adapter.diagnostic",json!({"level":"info","code":"acp.session","message":"ACP session opened.","terminal":false,"runtimeThreadId":self.normal.thread,"evidence":{"runtimeEventType":"locust/session","redacted":true}})));
        if self.profile.runtime == "copilot" {
            let agent = "https://agentclientprotocol.com/protocol/session-modes#agent";
            if !session["modes"]["availableModes"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|m| m["id"] == agent)
            {
                return Err(err("The agent did not offer the mode this run needs, so it was not started."));
            }
            if session["modes"]["currentModeId"] != agent {
                self.request(
                    "session/set_mode",
                    json!({"sessionId":self.normal.thread,"modeId":agent}),
                    deadline,
                )?;
            }
        }
        let mut required = self.profile.required.clone();
        if let Some(model) = self.model_config.clone() {
            required.push(model);
        }
        let mut session = session;
        let resolve = self.profile.resolve.clone();
        let resolved = |session: &Value, config: &str, wanted: String| match &resolve {
            Some(f) => f(config, &wanted, &config_choices(session, config)),
            None => wanted,
        };
        for (config, value) in required {
            let value = resolved(&session, &config, value);
            let current = session["configOptions"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|c| c["id"] == config)
                .and_then(|c| c["currentValue"].as_str());
            if current.is_none() && self.profile.strict {
                return Err(err(format!(
                    "The agent did not offer the required {config} setting, so it was not started."
                )));
            }
            if current.is_some_and(|c| c != value) {
                let set = self.request(
                    "session/set_config_option",
                    json!({"sessionId":self.normal.thread,"configId":config,"value":value}),
                    deadline,
                )?;
                if !set["configOptions"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|c| c["id"] == config && c["currentValue"] == value)
                {
                    return Err(err(format!(
                        "The agent would not set {config} to {value}, so it was not started."
                    )));
                }
                if let (Some(held), Some(update)) = (session.as_object_mut(), set.as_object()) {
                    held.extend(update.clone());
                }
            }
        }
        for (config, value) in self.profile.preferred.clone() {
            let value = resolved(&session, &config, value);
            if !config_choices(&session, &config).contains(&value) {
                continue;
            }
            let current = session["configOptions"].as_array().into_iter().flatten().find(|c| c["id"] == config.as_str()).and_then(|c| c["currentValue"].as_str());
            if current == Some(value.as_str()) {
                continue;
            }
            let set = self.request("session/set_config_option", json!({"sessionId":self.normal.thread,"configId":config,"value":value}), deadline)?;
            if let (Some(held), Some(update)) = (session.as_object_mut(), set.as_object()) {
                held.extend(update.clone());
            }
        }
        // A stop that came with the last answer of the handshake sends no prompt.
        if self.cancelled() {
            return Err(err("ACP startup stopped"));
        }
        self.send_prompt(&t.prompt)
    }
}
impl Runtime for Acp {
    fn can_steer(&self) -> bool {
        true
    }
    fn steer(&mut self, text: &str) -> bool {
        Acp::steer(self, text)
    }
    fn start(&mut self, t: &Turn) -> io::Result<()> {
        if self.turn.is_some() {
            return Err(err("ACP turn already started"));
        }
        if t.prompt.trim().is_empty() {
            return Err(err("Prompt must be nonempty"));
        }
        if let Err(e) = self.begin(t) {
            self.finish("", Value::Null, Some(&e.to_string()));
        }
        Ok(())
    }
    fn next_event(&mut self) -> io::Result<Option<Event>> {
        if let Some(e) = self.queued.pop_front() {
            return Ok(Some(e));
        }
        if self.normal.ended {
            return Ok(None);
        }
        if self.cancelled() {
            self.stop()?;
        }
        if self
            .prompt_started
            .is_some_and(|t| t.elapsed() >= Duration::from_secs(24 * 60 * 60))
        {
            self.finish(
                "",
                Value::Null,
                Some("The agent did not answer its prompt within 24 hours."),
            );
            return Ok(self.queued.pop_front());
        }
        if self
            .stopping
            .is_some_and(|t| t.elapsed() >= self.cancel_grace)
        {
            self.finish("cancelled", Value::Null, None);
            return Ok(self.queued.pop_front());
        }
        match self.receive() {
            Ok(Some(v)) => {
                if v["method"].is_string() {
                    if let Err(e) = self.handle(v, true) {
                        self.finish("", Value::Null, Some(&e.to_string()));
                    }
                } else if v["id"] == self.id {
                    if v["error"].is_null() {
                        self.prompt_result(&v["result"]);
                    } else {
                        self.finish(
                            "",
                            Value::Null,
                            Some(
                                v["error"]["message"]
                                    .as_str()
                                    .unwrap_or("The agent failed its turn."),
                            ),
                        );
                    }
                }
            }
            Ok(None) => {}
            Err(e) => self.finish("", Value::Null, Some(&e.to_string())),
        }
        Ok(self.queued.pop_front())
    }
    fn stop(&mut self) -> io::Result<()> {
        if self.stopping.is_some() || self.normal.ended {
            return Ok(());
        }
        self.stopping = Some(Instant::now());
        if self.normal.thread.is_some() {
            let _=self.send(json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":self.normal.thread}}));
        }
        // Bound termination independently of the stdout/stdin reader threads.
        super::process::watch_group(
            self.child.id(),
            self.closed.clone(),
            self.reap_lock.clone(),
            self.cancel_grace,
        );
        Ok(())
    }
}
impl Drop for Acp {
    fn drop(&mut self) {
        let _guard = crate::invariants::lock(&self.reap_lock);
        self.closed.store(true, Ordering::Relaxed);
        if !self.reaped {
            kill(self.child.id());
            super::process::kill_reap(&mut self.child);
        }
        if let Some(t) = &self.turn
            && let Some(ch) = &t.approvals
        {
            ch.release(&t.mission_id);
        }
    }
}

#[cfg(test)]
#[path = "acp_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "acp_queue_tests.rs"]
mod queue_tests;

#[cfg(test)]
#[path = "acp_agent_support.rs"]
mod agent_support;

#[cfg(test)]
#[path = "acp_events_case_tests.rs"]
mod events_case_tests;

#[cfg(test)]
#[path = "acp_run_case_tests.rs"]
mod run_case_tests;

#[cfg(test)]
#[path = "acp_stream_close_tests.rs"]
mod stream_close_tests;

#[cfg(test)]
#[path = "antigravity_acp_tests.rs"]
mod antigravity_acp_tests;

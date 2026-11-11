//! opencode-serve-run.ts: authenticated owned loopback server, SSE, permissions and EOF recovery.
use super::{
    Event, Runtime, Turn,
    opencode_events::Normalizer,
    process::{Process, copy_command},
};
use crate::approval::Mode;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::{self, BufRead, BufReader, Read},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};
fn err(s: impl ToString) -> io::Error {
    io::Error::other(s.to_string())
}
pub fn encoded(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}
fn base64(bytes: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for b in bytes.chunks(3) {
        let n = ((b[0] as u32) << 16)
            | ((b.get(1).copied().unwrap_or(0) as u32) << 8)
            | (b.get(2).copied().unwrap_or(0) as u32);
        out.push(A[(n >> 18) as usize] as char);
        out.push(A[((n >> 12) & 63) as usize] as char);
        out.push(if b.len() > 1 {
            A[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if b.len() > 2 {
            A[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
fn password() -> io::Result<String> {
    let mut bytes = [0u8; 24];
    #[cfg(unix)]
    {
        std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    }
    #[cfg(not(unix))]
    {
        return Err(err(
            "OpenCode's local server needs an operating-system random source on this platform",
        ));
    }
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
#[derive(Clone)]
pub struct Api {
    base: String,
    auth: String,
    agent: ureq::Agent,
}
impl Api {
    fn new(base: String, password: &str) -> Self {
        Self {
            base,
            auth: format!(
                "Basic {}",
                base64(format!("opencode:{password}").as_bytes())
            ),
            agent: ureq::AgentBuilder::new()
                .redirects(0)
                .timeout_connect(Duration::from_secs(3))
                .timeout_read(Duration::from_secs(10))
                .timeout_write(Duration::from_secs(10))
                .build(),
        }
    }
    fn request(&self, path: &str, body: Option<&Value>) -> io::Result<Value> {
        let request = self
            .agent
            .request(
                if body.is_some() { "POST" } else { "GET" },
                &format!("{}{path}", self.base),
            )
            .set("authorization", &self.auth);
        let response = if let Some(body) = body {
            request.send_json(body)
        } else {
            request.call()
        }
        .map_err(|e| err(format!("OpenCode's server request failed: {e}")))?;
        let mut bytes = vec![];
        response
            .into_reader()
            .take(8 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 8 * 1024 * 1024 {
            return Err(err("OpenCode server response exceeds 8 MiB"));
        }
        if bytes.is_empty() {
            Ok(Value::Null)
        } else {
            serde_json::from_slice(&bytes).map_err(err)
        }
    }
}
struct Part {
    kind: String,
    message: String,
    synthetic: bool,
    text: String,
    last: Option<Instant>,
    sent: String,
}
pub struct Serve {
    command: Command,
    process: Option<Process>,
    api: Option<Api>,
    rx: Option<Receiver<io::Result<Value>>>,
    cancel: Arc<AtomicBool>,
    stream_ended: Arc<AtomicBool>,
    own_end: Arc<AtomicBool>,
    session_shared: Arc<Mutex<Option<String>>>,
    normal: Normalizer,
    queued: VecDeque<Event>,
    turn: Option<Turn>,
    session: String,
    roles: HashMap<String, String>,
    children: HashSet<String>,
    parts: HashMap<String, Part>,
    emitted: HashSet<String>,
    prompt_id: Option<String>,
    resumed: bool,
    prompted: bool,
    begun: bool,
    errored: bool,
    ended: bool,
    started: Instant,
    recovery: Option<Instant>,
    last_check: Instant,
    stopping: Option<Instant>,
    overflow: Arc<AtomicBool>,
}
impl Serve {
    pub fn new(command: Command, cancel: Arc<AtomicBool>) -> Self {
        Self {
            command,
            process: None,
            api: None,
            rx: None,
            cancel,
            stream_ended: Arc::new(AtomicBool::new(false)),
            own_end: Arc::new(AtomicBool::new(false)),
            session_shared: Arc::new(Mutex::new(None)),
            normal: Normalizer::new(None),
            queued: VecDeque::new(),
            turn: None,
            session: String::new(),
            roles: HashMap::new(),
            children: HashSet::new(),
            parts: HashMap::new(),
            emitted: HashSet::new(),
            prompt_id: None,
            resumed: false,
            prompted: false,
            begun: false,
            errored: false,
            ended: false,
            started: Instant::now(),
            recovery: None,
            last_check: Instant::now(),
            stopping: None,
            overflow: Arc::new(AtomicBool::new(false)),
        }
    }
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed) || crate::run_cli::interrupted()
    }
    fn finish(&mut self, reason: Option<&str>) {
        if self.ended {
            return;
        }
        self.ended = true;
        self.own_end.store(true, Ordering::Relaxed);
        let cancelled = self.stopping.is_some() || self.cancelled();
        let mut why = reason.unwrap_or("").to_string();
        let mut confirmed = true;
        if let Some(p) = &mut self.process {
            confirmed = p.stop();
            let tail = crate::invariants::lock(&p.stderr);
            if !why.is_empty() && !tail.trim().is_empty() {
                why.push('\n');
                why.push_str(tail.trim());
            }
        }
        self.queued.extend(self.normal.finish(
            cancelled,
            if cancelled {
                None
            } else {
                Some(if self.errored || reason.is_some() {
                    1
                } else {
                    0
                })
            },
            &why,
            self.overflow.load(Ordering::Relaxed),
        ));
        if let Some(e) = self.queued.back_mut() {
            e.payload["process"]["terminationUnconfirmed"] = json!(!confirmed);
            if reason.is_some() && e.kind == "run.failed" && self.normal.base.records == 0 {
                e.payload["message"] = json!(super::events::text(&why, 16384));
            }
        }
        if let Some(t) = &self.turn
            && let Some(ch) = &t.approvals
        {
            ch.release(&t.mission_id);
        }
    }
    fn begin(&mut self, t: &Turn) -> io::Result<()> {
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
        self.normal = Normalizer::new(t.model.as_deref());
        self.normal.base.started_at = crate::ledger_write::now();
        self.resumed = t.resume.is_some();
        let password = password()?;
        self.command.env("OPENCODE_SERVER_PASSWORD", &password);
        let until = Instant::now() + Duration::from_secs(30);
        let listening = crate::invariants::regex(r"listening on (http://127\.0\.0\.1:[0-9]+)");
        let mut restarts = 0;
        loop {
            if self.cancelled() {
                return Err(err("OpenCode startup stopped"));
            }
            let mut p = Process::spawn(&mut copy_command(&self.command))?;
            let mut output = String::new();
            let mut found = None;
            while Instant::now() < until && !self.cancelled() {
                if p.overflow.load(Ordering::Relaxed) {
                    return Err(err("OpenCode startup output overflow"));
                }
                match p.lines.recv_timeout(Duration::from_millis(50)) {
                    Ok(s) => {
                        output.push_str(&s);
                        if output.len() > 4000 {
                            output = output
                                .chars()
                                .rev()
                                .take(4000)
                                .collect::<String>()
                                .chars()
                                .rev()
                                .collect();
                        }
                        if let Some(m) = listening.captures(&output) {
                            found = Some(m[1].to_owned());
                            break;
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(_) => {
                        if p.exited()?.is_some() {
                            break;
                        }
                    }
                }
                if p.exited()?.is_some() {
                    break;
                }
            }
            if let Some(base) = found {
                self.api = Some(Api::new(base, &password));
                self.process = Some(p);
                break;
            }
            let busy = {
                let s = crate::invariants::lock(&p.stderr);
                s.contains("database is locked") || s.contains("SQLITE_BUSY")
            };
            if !p.stop() {
                return Err(err(
                    "OpenCode's server did not confirm its exit; no replacement was started",
                ));
            }
            if busy && restarts < 3 && Instant::now() < until && !self.cancelled() {
                restarts += 1;
                let backoff = Instant::now()
                    + Duration::from_millis(
                        400 * 2u64.pow(restarts - 1)
                            + (std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map_err(io::Error::other)?
                                .subsec_nanos() as u64
                                % 400),
                    );
                while Instant::now() < backoff.min(until) && !self.cancelled() {
                    std::thread::sleep(Duration::from_millis(10));
                }
                continue;
            }
            return Err(err(if Instant::now() >= until {
                "OpenCode's server did not say where it was listening."
            } else {
                "OpenCode's server exited before the turn finished."
            }));
        }
        let api = self
            .api
            .as_ref()
            .ok_or_else(|| err("OpenCode API is unavailable"))?
            .clone();
        let event_agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout_connect(Duration::from_secs(3))
            .timeout_read(Duration::from_secs(24 * 60 * 60))
            .build();
        let response = event_agent
            .get(&format!("{}/event", api.base))
            .set("authorization", &api.auth)
            .call()
            .map_err(err)?;
        let (tx, rx) = mpsc::sync_channel(256);
        self.rx = Some(rx);
        let gone = self.stream_ended.clone();
        let end = self.own_end.clone();
        let sid = self.session_shared.clone();
        let overflow = self.overflow.clone();
        std::thread::spawn(move || {
            let mut r = BufReader::new(response.into_reader());
            let mut data = String::new();
            loop {
                let mut line = vec![];
                match (&mut r)
                    .take(8 * 1024 * 1024 + 1)
                    .read_until(b'\n', &mut line)
                {
                    Ok(0) => break,
                    Err(_) => break,
                    Ok(_) if line.len() > 8 * 1024 * 1024 => {
                        overflow.store(true, Ordering::Relaxed);
                        break;
                    }
                    _ => {}
                }
                let line = String::from_utf8_lossy(&line);
                if line.trim().is_empty() {
                    if data.is_empty() {
                        continue;
                    }
                    let parsed = serde_json::from_str::<Value>(&data);
                    data.clear();
                    if let Ok(v) = parsed {
                        if v["type"] == "session.idle"
                            && crate::invariants::lock(&sid).as_deref()
                                == v["properties"]["sessionID"].as_str()
                        {
                            end.store(true, Ordering::Relaxed);
                        }
                        match tx.try_send(Ok(v)) {
                            Ok(()) => {}
                            Err(mpsc::TrySendError::Full(_)) => {
                                overflow.store(true, Ordering::Relaxed);
                                break;
                            }
                            Err(_) => break,
                        }
                    }
                } else if let Some(s) = line.strip_prefix("data:") {
                    if data.len() + s.len() > 8 * 1024 * 1024 {
                        overflow.store(true, Ordering::Relaxed);
                        break;
                    }
                    data.push_str(s.trim());
                }
            }
            gone.store(true, Ordering::Relaxed);
        });
        self.session = if let Some(id) = &t.resume {
            id.clone()
        } else {
            api.request("/session", Some(&json!({"title":"Locust"})))?["id"]
                .as_str()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| err("OpenCode's server would not start a session."))?
                .into()
        };
        *crate::invariants::lock(&self.session_shared) = Some(self.session.clone());
        self.normal.base.thread = Some(self.session.clone());
        if self.cancelled() {
            return Err(err("OpenCode startup stopped"));
        }
        let mut body = json!({"parts":[{"type":"text","text":t.prompt}]});
        if let Some((provider, model)) = t.model.as_deref().and_then(|s| s.split_once('/'))
            && !provider.is_empty()
        {
            body["model"] = json!({"providerID":provider,"modelID":model});
        }
        if let Some(e) = &t.effort {
            body["variant"] = json!(e);
        }
        self.prompted = true;
        api.request(
            &format!("/session/{}/prompt_async", encoded(&self.session)),
            Some(&body),
        )?;
        self.started = Instant::now();
        self.last_check = Instant::now();
        Ok(())
    }
    fn push(&mut self, mut record: Value) {
        record["sessionID"] = json!(self.session);
        self.queued.extend(self.normal.accept(&record));
    }
    fn partial(&mut self, id: &str, force: bool) {
        let Some(p) = self.parts.get_mut(id) else {
            return;
        };
        // Never a partial for a part whose finished text already went out (Electron's sendPartial checks
        // emitted `text:<id>`): a paced partial flushed at the step's end after the final text opened a second
        // message with the same words, so every native OpenCode reply showed twice (found live on 11/11).
        if p.kind != "text"
            || p.synthetic
            || self.roles.get(&p.message).is_none_or(|r| r != "assistant")
            || p.text.is_empty()
            || p.sent == p.text
            || self.emitted.contains(&format!("text:{id}"))
        {
            return;
        }
        if !force
            && p.last
                .is_some_and(|t| t.elapsed() < Duration::from_millis(120))
        {
            return;
        }
        p.last = Some(Instant::now());
        p.sent = p.text.clone();
        let part = json!({"id":id,"type":"text","text":p.text,"messageID":p.message});
        self.push(json!({"type":"text_partial","part":part}));
    }
    fn flush(&mut self) {
        let ids: Vec<_> = self.parts.keys().cloned().collect();
        for id in ids {
            self.partial(&id, true);
        }
    }
    fn permission(&mut self, p: &Value) -> io::Result<()> {
        let Some(id) = p["id"].as_str() else {
            return Ok(());
        };
        let t = self
            .turn
            .as_ref()
            .ok_or_else(|| err("Turn has not started"))?;
        let permission = p["permission"].as_str().unwrap_or("unknown");
        let params = json!({"toolCall":{"toolCallId":id,"title":permission,"kind":if permission=="bash"{"execute"}else if permission=="edit"{"edit"}else{"other"},"rawInput":{"command":p["metadata"]["command"].as_str().map(str::to_owned).unwrap_or_else(||p["patterns"].as_array().into_iter().flatten().filter_map(Value::as_str).collect::<Vec<_>>().join("\n"))}}});
        let allow = if t.mode == Mode::Auto {
            true
        } else if t.mode == Mode::ApproveEach {
            if let Some(ch) = &t.approvals {
                let cancel = self.cancel.clone();
                let gone = self.stream_ended.clone();
                let end = self.own_end.clone();
                ch.ask_acp_runtime(
                    "opencode",
                    &t.run_id,
                    &t.mission_id,
                    &params,
                    &t.folder.to_string_lossy(),
                    &|| {
                        crate::run_cli::interrupted()
                            || cancel.load(Ordering::Relaxed)
                            || gone.load(Ordering::Relaxed)
                            || end.load(Ordering::Relaxed)
                    },
                )
                .unwrap_or(false)
            } else {
                false
            }
        } else {
            false
        };
        if self.cancelled() || self.own_end.load(Ordering::Relaxed) {
            return Ok(());
        }
        let body = if allow {
            json!({"reply":"once"})
        } else {
            json!({"reply":"reject","message":if t.mode==Mode::ApproveEach&&t.approvals.is_some(){"The person declined this in Locust."}else{"Locust refused this without asking anyone: this teammate's mode does not allow it, so nothing was run."}})
        };
        self.api
            .as_ref()
            .ok_or_else(|| err("OpenCode API is unavailable"))?
            .request(&format!("/permission/{}/reply", encoded(id)), Some(&body))?;
        Ok(())
    }
    fn accept(&mut self, v: &Value) -> io::Result<()> {
        let kind = v["type"].as_str().unwrap_or("");
        let p = &v["properties"];
        let about = p["sessionID"]
            .as_str()
            .or(p["info"]["sessionID"].as_str())
            .or(p["part"]["sessionID"].as_str());
        if matches!(kind, "session.created" | "session.updated")
            && let (Some(child), Some(parent)) =
                (p["info"]["id"].as_str(), p["info"]["parentID"].as_str())
            && (parent == self.session || self.children.contains(parent))
        {
            self.children.insert(child.into());
        }
        if about != Some(self.session.as_str()) {
            if about.is_some_and(|s| self.children.contains(s)) && kind == "permission.asked" {
                self.permission(p)?;
            }
            return Ok(());
        }
        match kind {
            "message.updated" => {
                if let (Some(id), Some(role)) =
                    (p["info"]["id"].as_str(), p["info"]["role"].as_str())
                {
                    if self.prompted
                        && self.prompt_id.is_none()
                        && role == "user"
                        && !self.roles.contains_key(id)
                    {
                        self.prompt_id = Some(id.into());
                    }
                    self.roles.insert(id.into(), role.into());
                    if role == "assistant" {
                        self.begun = true;
                    }
                }
            }
            "message.part.delta" => {
                if p["field"] == "text"
                    && let (Some(id), Some(delta)) = (p["partID"].as_str(), p["delta"].as_str())
                {
                    let held = self.parts.entry(id.into()).or_insert(Part {
                        kind: String::new(),
                        message: p["messageID"].as_str().unwrap_or("").into(),
                        synthetic: false,
                        text: String::new(),
                        last: None,
                        sent: String::new(),
                    });
                    held.text.push_str(delta);
                    held.text = super::events::text(&held.text, 262144);
                    self.partial(id, false);
                }
            }
            "message.part.updated" => {
                let part = &p["part"];
                let part_kind = part["type"].as_str().unwrap_or("");
                if let Some(id) = part["id"].as_str() {
                    let held = self.parts.entry(id.into()).or_insert(Part {
                        kind: String::new(),
                        message: String::new(),
                        synthetic: false,
                        text: String::new(),
                        last: None,
                        sent: String::new(),
                    });
                    held.kind = part_kind.into();
                    held.message = part["messageID"].as_str().unwrap_or(&held.message).into();
                    held.synthetic = part["synthetic"] == true;
                    if let Some(s) = part["text"].as_str()
                        && s.len() >= held.text.len()
                    {
                        held.text = super::events::text(s, 262144);
                    }
                }
                let assistant = part["messageID"]
                    .as_str()
                    .and_then(|id| self.roles.get(id))
                    .is_some_and(|r| r == "assistant");
                if assistant {
                    self.begun = true;
                }
                let record = match part_kind {
                    "step-start" if assistant => "step_start",
                    "step-finish" if assistant => "step_finish",
                    "tool"
                        if assistant
                            && matches!(
                                part["state"]["status"].as_str(),
                                Some("completed" | "error")
                            ) =>
                    {
                        "tool_use"
                    }
                    "text"
                        if part["time"]["end"].is_number()
                            && (assistant || part["synthetic"] == true) =>
                    {
                        "text"
                    }
                    "text" if assistant && part["synthetic"] != true => {
                        if let Some(id) = part["id"].as_str() {
                            self.partial(id, false);
                        }
                        return Ok(());
                    }
                    _ => return Ok(()),
                };
                let signature = part["id"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| canonical(part));
                let key = format!("{record}:{signature}");
                if !self.emitted.insert(key) {
                    return Ok(());
                }
                if self.emitted.len() > 20000 {
                    return Err(err("OpenCode record deduplication overflow"));
                }
                if matches!(record, "tool_use" | "step_finish") {
                    self.flush();
                }
                self.push(json!({"type":record,"part":part}));
            }
            "permission.asked" => self.permission(p)?,
            "session.status" => {
                self.begun = true;
                if p["status"]["type"] == "retry" {
                    self.push(json!({"type":"locust.retry","message":p["status"]["message"],"attempt":p["status"]["attempt"]}));
                }
            }
            "session.error" => {
                self.errored = true;
                self.push(json!({"type":"error","error":p["error"]}));
            }
            "session.idle" if self.prompted => self.finish(None),
            _ => {}
        }
        Ok(())
    }
    fn recover(&mut self) -> io::Result<()> {
        self.last_check = Instant::now();
        let api = self
            .api
            .as_ref()
            .ok_or_else(|| err("OpenCode API is unavailable"))?
            .clone();
        let statuses = api.request("/session/status", None)?;
        if !statuses.is_object() {
            return Err(err("OpenCode's session status was not readable."));
        }
        let status = &statuses[&self.session];
        let kind = status["type"].as_str();
        if matches!(kind, Some("busy" | "retry")) {
            return Ok(());
        }
        if !status.is_null() && kind != Some("idle") {
            return Err(err("OpenCode's session status could not be identified."));
        }
        let messages = api.request(
            &format!("/session/{}/message", encoded(&self.session)),
            None,
        )?;
        let messages = messages
            .as_array()
            .ok_or_else(|| err("OpenCode's session messages were not readable."))?;
        if self.recovery.is_none() {
            if messages
                .iter()
                .any(|m| m["info"]["role"] == "assistant" || m["role"] == "assistant")
                || self.begun
            {
                return Ok(());
            }
            return Err(err(
                "OpenCode took the message but never began on it: its own storage was likely busy with another OpenCode on this computer. Send it again.",
            ));
        }
        let mut owned: Vec<_> = messages
            .iter()
            .filter(|m| {
                let info = &m["info"];
                let id = info["id"].as_str();
                info["role"] == "assistant"
                    && info["sessionID"] == self.session
                    && id.is_some()
                    && if let Some(prompt) = &self.prompt_id {
                        info["parentID"] == prompt.as_str()
                    } else {
                        !self.resumed
                            && id
                                .and_then(|id| self.roles.get(id))
                                .is_some_and(|r| r == "assistant")
                    }
            })
            .collect();
        owned.sort_by(|a, b| {
            a["info"]["time"]["created"]
                .as_f64()
                .unwrap_or(0.)
                .total_cmp(&b["info"]["time"]["created"].as_f64().unwrap_or(0.))
                .then_with(|| a["info"]["id"].as_str().cmp(&b["info"]["id"].as_str()))
        });
        let Some(last) = owned.last() else {
            return Err(err(
                "OpenCode's event stream closed before the turn finished. This turn's completion could not be confirmed.",
            ));
        };
        let info = &last["info"];
        if info["finish"] != "stop"
            || !info["error"].is_null()
            || !info["time"]["completed"].is_number()
            || !last["parts"].as_array().into_iter().flatten().any(|p| {
                p["messageID"] == info["id"]
                    && p["sessionID"] == self.session
                    && p["type"] == "step-finish"
                    && p["reason"] == "stop"
            })
        {
            return Err(err(
                "OpenCode's event stream closed before the turn finished. The session is no longer busy, but this turn's completion could not be confirmed.",
            ));
        }
        for m in owned {
            self.accept(&json!({"type":"message.updated","properties":{"sessionID":self.session,"info":m["info"]}}))?;
            let mut parts: Vec<_> = m["parts"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|p| p["messageID"] == m["info"]["id"] && p["sessionID"] == self.session)
                .collect();
            if parts
                .iter()
                .all(|p| p["id"].as_str().is_some_and(|id| id.starts_with("prt_")))
            {
                parts.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
            }
            for p in parts {
                self.accept(&json!({"type":"message.part.updated","properties":{"sessionID":self.session,"part":p}}))?;
            }
        }
        self.finish(None);
        Ok(())
    }
}
fn canonical(v: &Value) -> String {
    fn sorted(v: &Value) -> Value {
        match v {
            Value::Object(o) => {
                let mut pairs: Vec<_> = o.iter().collect();
                pairs.sort_by_key(|(k, _)| *k);
                Value::Object(
                    pairs
                        .into_iter()
                        .map(|(k, v)| (k.clone(), sorted(v)))
                        .collect(),
                )
            }
            Value::Array(a) => Value::Array(a.iter().map(sorted).collect()),
            _ => v.clone(),
        }
    }
    serde_json::to_string(&sorted(v)).unwrap_or_default()
}
impl Runtime for Serve {
    fn start(&mut self, t: &Turn) -> io::Result<()> {
        if self.turn.is_some() {
            return Err(err("OpenCode turn already started"));
        }
        if let Err(e) = self.begin(t) {
            self.finish(Some(&e.to_string()));
        }
        Ok(())
    }
    fn next_event(&mut self) -> io::Result<Option<Event>> {
        if let Some(e) = self.queued.pop_front() {
            return Ok(Some(e));
        }
        if self.ended {
            return Ok(None);
        }
        if self.cancelled() {
            self.stop()?;
        }
        if self
            .stopping
            .is_some_and(|t| t.elapsed() >= Duration::from_millis(1500))
        {
            self.finish(None);
            return Ok(self.queued.pop_front());
        }
        if self.overflow.load(Ordering::Relaxed)
            || self
                .process
                .as_ref()
                .is_some_and(|p| p.overflow.load(Ordering::Relaxed))
        {
            self.overflow.store(true, Ordering::Relaxed);
            self.finish(Some("OpenCode output overflow"));
            return Ok(self.queued.pop_front());
        }
        let result = self
            .rx
            .as_ref()
            .ok_or_else(|| err("OpenCode event stream is unavailable"))?
            .recv_timeout(Duration::from_millis(50));
        match result {
            Ok(Ok(v)) => {
                if let Err(e) = self.accept(&v) {
                    self.finish(Some(&e.to_string()));
                }
            }
            Ok(Err(e)) => self.finish(Some(&e.to_string())),
            Err(_) => {}
        }
        if self.ended {
            return Ok(self.queued.pop_front());
        }
        if self.stream_ended.load(Ordering::Relaxed) && self.recovery.is_none() {
            self.flush();
            self.recovery = Some(Instant::now());
            self.last_check = Instant::now() - Duration::from_secs(1);
        }
        if self
            .recovery
            .is_some_and(|t| t.elapsed() >= Duration::from_secs(300))
        {
            self.finish(Some("OpenCode's event stream closed before the turn finished. Recovery deadline expired; its outcome could not be confirmed."));
        } else if (self.recovery.is_some()
            && self.last_check.elapsed() >= Duration::from_millis(250)
            || !self.begun && self.last_check.elapsed() >= Duration::from_secs(60))
            && let Err(e) = self.recover()
        {
            self.finish(Some(&e.to_string()));
        }
        Ok(self.queued.pop_front())
    }
    fn stop(&mut self) -> io::Result<()> {
        if self.ended || self.stopping.is_some() {
            return Ok(());
        }
        self.cancel.store(true, Ordering::Relaxed);
        self.stopping = Some(Instant::now());
        if let Some(api) = self.api.clone() {
            let id = encoded(&self.session);
            std::thread::spawn(move || {
                let _ = api.request(&format!("/session/{id}/abort"), Some(&json!({})));
            });
        }
        Ok(())
    }
}
impl Drop for Serve {
    fn drop(&mut self) {
        if let Some(p) = &mut self.process {
            p.stop();
        }
        if let Some(t) = &self.turn
            && let Some(ch) = &t.approvals
        {
            ch.release(&t.mission_id);
        }
    }
}

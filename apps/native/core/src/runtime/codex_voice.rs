//! Realtime voice runs on a dedicated Codex app-server connection. The window owns its WebRTC peer.

use super::codex::Codex;
use crate::voice_call_rules::{self as rules, AddOn, Teammate};
use crate::voice_transcript::{Handoff, Speaker, Transcript};
use serde_json::{Value, json};
use std::{
    io,
    path::PathBuf,
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const ENDED_EARLY: &str = "The call was ended before it began.";
const MISSING_CODEX: &str =
    "Voice calls use ChatGPT’s voice through Codex CLI, which is not ready on this computer.";

#[derive(Clone)]
pub struct CallOptions {
    pub folder: PathBuf,
    pub offer: String,
    pub teammate: Option<Teammate>,
    pub voice: Option<String>,
    pub preview: Option<String>,
    pub connectors: bool,
    pub anchor_turn: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CallEvent {
    Partial(&'static str, String),
    Said(&'static str, String),
    Working(bool),
    Handed {
        to: String,
        task: String,
    },
    Asks {
        id: String,
        what: String,
        fields: Vec<(String, String)>,
    },
    Asked {
        id: String,
        allowed: bool,
    },
    Error(String),
    Ended(Option<String>),
}

/// A private server has the lifetime of its lease, including when cancellation overtakes opening it.
pub trait CallAddOn: Send {
    fn configuration(&self) -> &AddOn;
    fn take_handoffs(&self) -> Vec<Handoff> {
        Vec::new()
    }
    fn close_scope(&self) {}
}

pub trait CallHost: Send + Sync + 'static {
    fn policy_refusal(&self, _options: &CallOptions) -> Option<String> {
        None
    }
    fn discover(&self) -> Option<Command>;
    fn transcript_store(&self) -> Option<&crate::store::Store> {
        None
    }
    fn open_add_on(&self, _options: &CallOptions) -> Option<Box<dyn CallAddOn>> {
        None
    }
}

pub struct NativeHost {
    store: crate::store::Store,
    executable: PathBuf,
}

impl NativeHost {
    pub fn new(store: crate::store::Store, executable: PathBuf) -> Self {
        Self { store, executable }
    }
}

impl CallHost for NativeHost {
    fn policy_refusal(&self, options: &CallOptions) -> Option<String> {
        let mode = if options.preview.is_some() {
            "ask"
        } else {
            "approve-each"
        };
        crate::org_policy::turn_refusal(
            Some(&crate::org_policy::current()),
            "codex",
            None,
            mode,
            false,
            false,
        )
    }

    fn discover(&self) -> Option<Command> {
        let codex = super::discovery::find().ok()?;
        let mut command = Command::new(codex);
        command.arg("app-server");
        Some(command)
    }

    fn transcript_store(&self) -> Option<&crate::store::Store> {
        Some(&self.store)
    }

    fn open_add_on(&self, options: &CallOptions) -> Option<Box<dyn CallAddOn>> {
        let here = options.teammate.as_ref().and_then(|teammate| {
            let id = teammate.target.conversation_id.as_deref()?;
            (crate::people::owner_of(&self.store, id).as_deref()
                == Some(&teammate.target.teammate_id))
            .then_some(id)
        });
        crate::door::open_voice_door(
            self.store.clone(),
            &self.executable,
            options.folder.clone(),
            here,
        )
        .map(|door| Box::new(door) as Box<dyn CallAddOn>)
    }
}

#[derive(Clone, Copy)]
struct Times {
    request: Duration,
    answer: Duration,
    stop: Duration,
    ask: Duration,
}

impl Default for Times {
    fn default() -> Self {
        Self {
            request: Duration::from_secs(30),
            answer: Duration::from_secs(20),
            stop: Duration::from_millis(1_500),
            ask: Duration::from_secs(120),
        }
    }
}

enum Input {
    Answer(String, bool),
}

struct Active {
    preview: bool,
    stopping: Arc<AtomicBool>,
    quitting: Arc<AtomicBool>,
    input: Sender<Input>,
    worker: JoinHandle<()>,
}

pub struct CallWindow {
    pub started: Receiver<Result<String, String>>,
    pub events: Receiver<CallEvent>,
}

pub struct VoiceCalls {
    host: Arc<dyn CallHost>,
    current: Mutex<Option<Active>>,
    times: Times,
}

impl VoiceCalls {
    pub fn new(host: Arc<dyn CallHost>) -> Self {
        Self {
            host,
            current: Mutex::new(None),
            times: Times::default(),
        }
    }

    pub fn start(&self, options: CallOptions) -> Result<CallWindow, String> {
        if !rules::is_offer(&options.offer) {
            return Err("Talk could not read this window’s audio offer.".into());
        }
        if options
            .preview
            .as_deref()
            .is_some_and(|voice| !rules::is_voice(voice))
        {
            return Err("Choose a voice to hear.".into());
        }
        let preview = options.preview.is_some();
        let mut current = crate::invariants::lock(&self.current);
        if current.as_ref().is_some_and(|old| old.worker.is_finished()) {
            current.take();
        }
        if preview
            && current
                .as_ref()
                .is_some_and(|old| !old.preview && !old.stopping.load(Ordering::Acquire))
        {
            return Err("A call is on. End it to hear a voice.".into());
        }
        let previous = current.take();
        if let Some(old) = &previous {
            old.stopping.store(true, Ordering::Release);
        }

        let (start_send, started) = mpsc::channel();
        let (event_send, events) = mpsc::sync_channel(256);
        let (input, from_window) = mpsc::channel();
        let stopping = Arc::new(AtomicBool::new(false));
        let quitting = Arc::new(AtomicBool::new(false));
        let signals = Signals {
            stopping: stopping.clone(),
            quitting: quitting.clone(),
            events: event_send,
            from_window,
        };
        let host = self.host.clone();
        let times = self.times;
        let worker = thread::spawn(move || {
            if let Some(old) = previous {
                // Its process and private server close before the replacement spawns.
                let _ = old.worker.join();
            }
            if let Err(message) = run_call(host.as_ref(), &options, &signals, times, &start_send) {
                // If start already succeeded, the receiver can ignore this second result.
                let _ = start_send.send(Err(message.clone()));
                signals.event(CallEvent::Ended(Some(message)));
            }
        });
        *current = Some(Active {
            preview,
            stopping,
            quitting,
            input,
            worker,
        });
        Ok(CallWindow { started, events })
    }

    pub fn stop(&self) {
        if let Some(active) = crate::invariants::lock(&self.current).as_ref() {
            active.stopping.store(true, Ordering::Release);
        }
    }

    pub fn answer(&self, id: String, allowed: bool) {
        if let Some(active) = crate::invariants::lock(&self.current).as_ref() {
            let _ = active.input.send(Input::Answer(id, allowed));
        }
    }

    pub fn dispose(&self) {
        if let Some(active) = crate::invariants::lock(&self.current).as_ref() {
            active.quitting.store(true, Ordering::Release);
            active.stopping.store(true, Ordering::Release);
        }
    }
}

impl Drop for VoiceCalls {
    fn drop(&mut self) {
        self.dispose();
    }
}

struct Signals {
    stopping: Arc<AtomicBool>,
    quitting: Arc<AtomicBool>,
    events: mpsc::SyncSender<CallEvent>,
    from_window: Receiver<Input>,
}

impl Signals {
    fn check(&self) -> Result<(), String> {
        if self.stopping.load(Ordering::Acquire) {
            Err(ENDED_EARLY.into())
        } else {
            Ok(())
        }
    }

    fn event(&self, event: CallEvent) {
        if self.events.try_send(event).is_err() {
            self.stopping.store(true, Ordering::Release);
        }
    }
}

fn run_call(
    host: &dyn CallHost,
    options: &CallOptions,
    signals: &Signals,
    times: Times,
    started: &Sender<Result<String, String>>,
) -> Result<(), String> {
    signals.check()?;
    if let Some(reason) = host.policy_refusal(options) {
        return Err(reason);
    }
    signals.check()?;
    if options.folder.as_os_str().is_empty() {
        return Err("Pick a folder first.".into());
    }
    let command = host.discover();
    signals.check()?;
    let mut command = command.ok_or(MISSING_CODEX)?;
    let client = Codex::spawn(&mut command).map_err(wire_failure)?;
    let transcript = options
        .teammate
        .as_ref()
        .filter(|_| options.preview.is_none())
        .and_then(|teammate| {
            host.transcript_store().map(|_| {
                Transcript::new(
                    teammate.target.teammate_id.clone(),
                    teammate.name.clone(),
                    options.anchor_turn.clone(),
                    now(),
                )
            })
        });
    let mut call = Session {
        client,
        signals,
        times,
        thread: None,
        preview: options.preview.is_some(),
        connectors: options.preview.is_none() && options.connectors,
        add_on: None,
        answer: None,
        pending: Vec::new(),
        teammate: options.teammate.as_ref(),
        live: false,
        transcript,
    };

    if let Err(message) = call.begin(host, options).and_then(|()| signals.check()) {
        call.stop();
        return Err(signals.check().err().unwrap_or(message));
    }
    let answer = call
        .answer
        .take()
        .ok_or("The voice service did not answer in time. Try again.")?;
    if started.send(Ok(answer)).is_err() {
        call.stop();
        return Ok(());
    }
    call.live = true;
    let why = call.listen().err();
    if let Some(add_on) = &call.add_on {
        add_on.close_scope();
    }
    call.stop();
    call.collect_handoffs();
    if let (Some(transcript), Some(store)) = (call.transcript.take(), host.transcript_store())
        && let Err(why) = transcript.save(store, now())
    {
        signals.event(CallEvent::Error(format!(
            "The call’s words could not be saved. {why}"
        )));
    }
    drop(call);
    signals.event(CallEvent::Ended(why));
    Ok(())
}

struct Pending {
    id: String,
    wire_id: Value,
    deadline: Instant,
}

struct Session<'a> {
    client: Codex,
    signals: &'a Signals,
    times: Times,
    thread: Option<String>,
    preview: bool,
    connectors: bool,
    add_on: Option<Box<dyn CallAddOn>>,
    answer: Option<String>,
    pending: Vec<Pending>,
    teammate: Option<&'a Teammate>,
    live: bool,
    transcript: Option<Transcript>,
}

fn wire_failure(err: io::Error) -> String {
    rules::failure_words(&err.to_string())
}

impl Session<'_> {
    fn begin(&mut self, host: &dyn CallHost, options: &CallOptions) -> Result<(), String> {
        self.request("initialize", json!({
            "clientInfo": {"name": "locust", "title": "Locust", "version": env!("CARGO_PKG_VERSION")},
            "capabilities": {"experimentalApi": true, "requestAttestation": false}
        }), self.times.request)?;
        self.client
            .send(json!({"jsonrpc":"2.0","method":"initialized"}))
            .map_err(wire_failure)?;
        let account = self.request("account/read", json!({}), self.times.request)?;
        match account["account"]["type"].as_str() {
            Some("chatgpt") => {},
            Some(_) => return Err("Codex is signed in with an API key. Voice calls use your ChatGPT sign-in: sign Codex in with ChatGPT and try again.".into()),
            None => return Err("Codex is not signed in. Voice calls use your ChatGPT sign-in: sign Codex in with ChatGPT and try again.".into()),
        }
        let settings = self
            .request(
                "config/read",
                json!({"cwd":options.folder}),
                self.times.request,
            )
            .map_err(|_| rules::SETTINGS_UNREAD.to_owned())?;
        // Refuse a foreign media endpoint before opening anything else.
        rules::thread_config(&settings["config"], None, self.connectors)?;
        if !self.preview {
            self.add_on = host.open_add_on(options);
        }
        self.signals.check()?;
        let config = rules::thread_config(
            &settings["config"],
            self.add_on.as_ref().map(|lease| lease.configuration()),
            self.connectors,
        )?;
        let thread = self.request(
            "thread/start",
            json!({
                "cwd": options.folder, "sandbox": "read-only", "approvalPolicy": "untrusted",
                "ephemeral": true, "config": config,
            }),
            self.times.request,
        )?;
        let id = thread["thread"]["id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or("Codex did not start the call’s conversation.")?;
        self.thread = Some(id.to_owned());

        let mut start = json!({
            "threadId": id, "outputModality": "audio", "version": "v3",
            "transport": {"type":"webrtc", "sdp": options.offer},
        });
        if let Some(voice) = &options.preview {
            start["voice"] = json!(voice);
            start["includeStartupContext"] = json!(false);
            start["initialItems"] =
                json!([{"role":"developer","text":rules::preview_rules(voice)}]);
        } else {
            let folder = options
                .folder
                .file_name()
                .unwrap_or(options.folder.as_os_str())
                .to_string_lossy();
            start["realtimeStartInstructions"] = json!(rules::instructions(
                &folder,
                self.add_on.is_some(),
                self.connectors,
                self.teammate
            ));
            start["initialItems"] =
                json!([{"role":"developer","text":rules::voice_rules(self.teammate)}]);
            let voice = self
                .teammate
                .and_then(|teammate| teammate.voice.as_deref())
                .or(options.voice.as_deref());
            if let Some(voice) = voice.filter(|voice| rules::is_voice(voice)) {
                start["voice"] = json!(voice);
            }
        }
        let deadline = Instant::now() + self.times.answer;
        self.request(
            "thread/realtime/start",
            start,
            self.times.request.min(self.times.answer),
        )?;
        while self.answer.is_none() {
            self.signals.check()?;
            if Instant::now() >= deadline {
                return Err("The voice service did not answer in time. Try again.".into());
            }
            if let Some(wire) = self.client.receive().map_err(wire_failure)? {
                self.handle(wire)?;
            }
        }
        // A terminal notification already on the wire belongs to this start, even if it followed an SDP.
        while let Some(wire) = self.client.try_receive().map_err(wire_failure)? {
            self.signals.check()?;
            self.handle(wire)?;
        }
        self.signals.check()
    }

    fn request(&mut self, method: &str, params: Value, timeout: Duration) -> Result<Value, String> {
        self.signals.check()?;
        let id = self
            .client
            .begin_request(method, params)
            .map_err(wire_failure)?;
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            self.signals.check()?;
            if let Some(wire) = self.client.receive().map_err(wire_failure)? {
                if wire["method"].is_null() && wire["id"] == id {
                    if let Some(message) = wire["error"]["message"].as_str() {
                        return Err(rules::failure_words(message));
                    }
                    return Ok(wire["result"].clone());
                }
                self.handle(wire)?;
            }
        }
        Err(rules::failure_words(&format!(
            "Codex timed out answering {method}"
        )))
    }

    fn listen(&mut self) -> Result<(), String> {
        while self.signals.check().is_ok() {
            self.collect_handoffs();
            while let Ok(Input::Answer(id, allowed)) = self.signals.from_window.try_recv() {
                if let Some(index) = self.pending.iter().position(|pending| pending.id == id) {
                    let allowed = allowed && Instant::now() < self.pending[index].deadline;
                    self.settle(index, allowed)?;
                }
            }
            while let Some(index) = self
                .pending
                .iter()
                .position(|pending| Instant::now() >= pending.deadline)
            {
                self.settle(index, false)?;
            }
            if let Some(wire) = self.client.receive().map_err(wire_failure)? {
                self.handle(wire)?;
            }
        }
        Ok(())
    }

    fn collect_handoffs(&mut self) {
        let Some(add_on) = &self.add_on else { return };
        for handoff in add_on.take_handoffs() {
            if let Some(transcript) = &mut self.transcript {
                transcript.record_handoff(
                    &handoff.mission_id,
                    &handoff.teammate_id,
                    &handoff.name,
                    &handoff.task,
                    handoff.at,
                );
            }
        }
    }

    fn stop(&mut self) {
        while !self.pending.is_empty() {
            let _ = self.settle(self.pending.len() - 1, false);
        }
        let Some(thread) = &self.thread else { return };
        if !self.signals.stopping.load(Ordering::Acquire)
            || self.signals.quitting.load(Ordering::Acquire)
        {
            return;
        }
        if let Ok(id) = self
            .client
            .begin_request("thread/realtime/stop", json!({"threadId":thread}))
        {
            let until = Instant::now() + self.times.stop;
            while Instant::now() < until && !self.signals.quitting.load(Ordering::Acquire) {
                match self.client.receive() {
                    Ok(Some(wire)) if wire["id"] == id && wire["method"].is_null() => break,
                    Err(_) => break,
                    _ => {}
                }
            }
        }
    }

    fn result(&mut self, id: Value, result: Value) -> Result<(), String> {
        self.client
            .send(json!({"jsonrpc":"2.0","id":id,"result":result}))
            .map_err(wire_failure)
    }

    fn settle(&mut self, index: usize, allowed: bool) -> Result<(), String> {
        let pending = self.pending.swap_remove(index);
        self.signals.event(CallEvent::Asked {
            id: pending.id,
            allowed,
        });
        self.result(
            pending.wire_id,
            if allowed {
                json!({"action":"accept","content":{}})
            } else {
                json!({"action":"decline"})
            },
        )
    }

    fn handle(&mut self, wire: Value) -> Result<(), String> {
        let Some(method) = wire["method"].as_str() else {
            return Ok(());
        };
        let params = &wire["params"];
        let own_thread = match (&self.thread, params["threadId"].as_str()) {
            (Some(thread), Some(asked)) => thread == asked,
            (Some(_), None) => true,
            (None, _) => false,
        };
        if !wire["id"].is_null() {
            if method != "mcpServer/elicitation/request" {
                return self.result(wire["id"].clone(), json!({"decision":"decline"}));
            }
            let is_tool = params["_meta"]["codex_approval_kind"] == "mcp_tool_call";
            if own_thread
                && is_tool
                && params["serverName"] == rules::ADD_ON_NAME
                && self.add_on.is_some()
            {
                return self.result(wire["id"].clone(), json!({"action":"accept","content":{}}));
            }
            if own_thread
                && is_tool
                && self.live
                && self.connectors
                && params["serverName"] != rules::ADD_ON_NAME
                && self.pending.len() < 32
                && let Some((what, fields)) = connector_question(params)
            {
                let id = format!("call_{}", crate::invariants::unique_nanos());
                self.pending.push(Pending {
                    id: id.clone(),
                    wire_id: wire["id"].clone(),
                    deadline: Instant::now() + self.times.ask,
                });
                self.signals.event(CallEvent::Asks { id, what, fields });
                return Ok(());
            }
            return self.result(wire["id"].clone(), json!({"action":"decline"}));
        }
        if !own_thread {
            return Ok(());
        }
        match method {
            "thread/realtime/sdp" if !self.live => {
                self.answer = params["sdp"]
                    .as_str()
                    .filter(|sdp| rules::is_offer(sdp))
                    .map(str::to_owned);
            }
            "thread/realtime/transcript/delta" => {
                if let (Some(role), Some(delta)) = (
                    rules::role(&params["role"]),
                    params["delta"].as_str().filter(|s| !s.is_empty()),
                ) {
                    if let (Some(transcript), Some(speaker)) =
                        (&mut self.transcript, Speaker::from_wire(role))
                    {
                        transcript.delta(speaker, delta, now());
                    }
                    self.signals
                        .event(CallEvent::Partial(role, delta.to_owned()));
                }
            }
            "thread/realtime/transcript/done" => {
                if let (Some(role), Some(text)) =
                    (rules::role(&params["role"]), params["text"].as_str())
                {
                    if let (Some(transcript), Some(speaker)) =
                        (&mut self.transcript, Speaker::from_wire(role))
                    {
                        transcript.said(speaker, text, now());
                    }
                    self.signals.event(CallEvent::Said(role, text.to_owned()));
                }
            }
            "thread/realtime/error" => {
                let message = rules::failure_words(
                    params["message"]
                        .as_str()
                        .unwrap_or("The voice service reported an error."),
                );
                if !self.live {
                    return Err(message);
                }
                self.signals.event(CallEvent::Error(message));
            }
            "thread/realtime/closed" => {
                let reason = params["reason"].as_str().unwrap_or("requested");
                if self.live && reason == "requested" {
                    self.signals.stopping.store(true, Ordering::Release);
                } else {
                    let reason = clean_words(reason, 80);
                    return Err(format!("The voice service closed the call ({reason})."));
                }
            }
            "turn/started" => self.signals.event(CallEvent::Working(true)),
            "turn/completed" => self.signals.event(CallEvent::Working(false)),
            "item/started" if self.add_on.is_some() => self.handed(&params["item"]),
            _ => {}
        }
        Ok(())
    }

    fn handed(&self, item: &Value) {
        if item["type"] != "mcpToolCall" || item["server"] != rules::ADD_ON_NAME {
            return;
        }
        let args = &item["arguments"];
        let to = match item["tool"].as_str() {
            Some("start_conversation") => args["teammate"].as_str(),
            Some("send_message") => self.teammate.map(|teammate| teammate.name.as_str()),
            _ => None,
        };
        if let Some(to) = to {
            let task = args["message"].as_str().unwrap_or_default();
            self.signals.event(CallEvent::Handed {
                to: clean_words(to, 80),
                task: clean_words(task, 400),
            });
        }
    }
}

fn now() -> u64 {
    chrono::Utc::now().timestamp_millis().max(1) as u64
}

fn clean_words(value: &str, limit: usize) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(limit)
        .collect()
}

fn connector_question(params: &Value) -> Option<(String, Vec<(String, String)>)> {
    if params["mode"] != "form" {
        return None;
    }
    let server = params["serverName"].as_str()?;
    let tool = crate::invariants::regex("run tool \"([^\"]{1,120})\"")
        .captures(params["message"].as_str().unwrap_or(""))
        .and_then(|matched| matched.get(1))
        .map(|name| name.as_str());
    let words = |text: &str| clean_words(&text.replace(['_', '-'], " "), 140);
    let app_tool = tool.and_then(|tool| tool.split_once('.'));
    let (server, tool) = match (server, app_tool) {
        ("codex_apps", Some((app, action))) => {
            let app = words(app);
            let mut chars = app.chars();
            let name = chars.next().map_or_else(
                || "ChatGPT".into(),
                |first| format!("{}{}", first.to_uppercase(), chars.as_str()),
            );
            (name, words(action))
        }
        ("codex_apps", None) => ("ChatGPT".into(), words(tool.unwrap_or("a tool"))),
        (server, _) => (words(server), words(tool.unwrap_or("a tool"))),
    };
    let fields = params["_meta"]["tool_params_display"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|field| {
            let name = clean_words(field["display_name"].as_str()?, 60);
            if name.is_empty() {
                return None;
            }
            let value = &field["value"];
            let value = value
                .as_str()
                .map_or_else(|| value.to_string(), str::to_owned);
            Some((name, clean_words(&value, 600)))
        })
        .take(8)
        .collect();
    Some((clean_words(&format!("{server}: {tool}"), 140), fields))
}

#[cfg(all(test, unix))]
#[path = "codex_voice_tests.rs"]
mod tests;

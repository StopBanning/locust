//! OTHER APPS, FROM ANYWHERE (Electron `main/remote-mcp.ts`, `shared/remote-mcp.ts`, 0.841).
//!
//! Claude on the web, Claude Duet and any app that takes a remote MCP server use the person's Locust team at an address
//! on Locust's relay. The relay passes each request here and this computer decides every one of them:
//!
//! - An app signs in the way MCP asks (OAuth 2.1 with PKCE). Its sign-in becomes a card in Locust's window naming it,
//!   where it sends the person back to, and six digits the sign-in page shows too. Only Allow there makes a code.
//! - The code, and the keys the app trades it for, are made here and kept here as SHA-256 hashes (never the key itself,
//!   so no secret box is needed: there is no secret to seal). The relay passes keys on and never holds one. A key lasts
//!   a day; the app renews it with its refresh key, which lasts ninety days of not being used, and is replaced each time.
//! - The tools are exactly what an app on this computer gets (`locust-mcp-bridge.mjs`), run through [`McpTools`].
//! - Remove takes one app's keys away at once. Turning this off takes every app's.
//!
//! Every network call goes behind [`McpRelay`]; the relay's own side (room, pages, held requests) is not here. The live
//! relay is the phone link's own line (`PhoneAnywhere` is an `McpRelay`), and the tools are the door's
//! (`door::tools_from_anywhere`).

use crate::door_tools::ToolResult;
use crate::routine_hooks::{iso_millis, random_base64url, sha256_base64url};
use serde_json::{Map, Value, json};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

pub const REMOTE_MCP_FILE: &str = "remote-mcp.json";
pub const REMOTE_MCP_GET: &str = "remote-mcp:get";
pub const REMOTE_MCP_SET: &str = "remote-mcp:set";
pub const REMOTE_MCP_REMOVE: &str = "remote-mcp:remove";
pub const REMOTE_MCP_DECIDE: &str = "remote-mcp:decide";
pub const REMOTE_MCP_CHANGED: &str = "remote-mcp:changed";

/// An access key's life: renewed by the app with its refresh key, while this computer is on.
pub const ACCESS_MS: i64 = 24 * 60 * 60_000;
pub const REFRESH_MS: i64 = 90 * 24 * 60 * 60_000;
/// How long the person has to answer the card, and a code to be traded once allowed.
pub const ASKING_MS: i64 = 5 * 60_000;
pub const CODE_MS: i64 = 5 * 60_000;
/// Apps connected at once: the one used longest ago goes, past this.
pub const MAX_APPS: usize = 10;
/// The longest answer sent back through the relay; a tool's answer past it is said to be too long.
pub const MAX_ANSWER_CHARS: usize = 240 * 1024;
/// An app's last use is kept at most once a minute: a call is not a disk write.
pub const USED_SAVE_MS: i64 = 60_000;
const PROTOCOLS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

pub const NEEDS_FROM_ANYWHERE: &str = "Turn on From anywhere under Your phone first: apps reach Locust through the same relay.";
pub const RELAY_UNREACHED: &str = "The relay could not be reached. Nothing was turned on; try again in a moment.";
pub const NOT_SAVED: &str = "The setting could not be saved. It may change back when Locust restarts.";
pub const TOO_LONG: &str = "That answer is too long to send through Locust\u{2019}s relay. Read it in Locust.";
pub const NOT_COMPLETED: &str = "Locust could not complete that request. Nothing was retried.";

/// The tools, as the bridge an app on this computer runs lists them: word for word, which a test checks against
/// `resources/locust-mcp-bridge.mjs`, so an app is never told something different from anywhere.
pub fn remote_mcp_tools() -> Value {
    let schema = |properties: Value, required: &[&str]| json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false});
    let name = json!({"type": "string", "minLength": 1, "maxLength": 200});
    let message = json!({"type": "string", "minLength": 1, "maxLength": 8000});
    let effort = json!({"type": "string", "minLength": 1, "maxLength": 40});
    let flag = json!({"type": "boolean"});
    let mode = json!({"type": "string", "enum": ["ask", "plan", "accept-edits", "approve-each", "auto"]});
    let folder = json!({"type": "string", "minLength": 1, "maxLength": 1000});
    let tools: [(&str, &str, Value, bool); 12] = [
        ("list_teammates", r#"List Locust teammates: id, name, role, the folder Locust has open (every turn works there; the person changes it in Locust), runtime, model, whether busy, and runs_in: the mode a turn started from here would use. Read only."#, schema(json!({}), &[]), true),
        ("start_conversation", r#"Ask a Locust teammate by name or id. Ask mode (read only), unless the person has turned on "Use each teammate's own mode" in Locust; then the mode the teammate is set to there, or the mode you name: ask, plan, accept-edits, approve-each or auto (Auto only if Locust's Settings allow it). With that switch on you may also name a folder, a full path: one Locust knows, a folder inside one, or a worktree of one of their repositories; the turn works there instead of the open folder, and the conversation stays there. Any approval is answered by the person in Locust's window, never from here. Optionally a model and effort for this turn alone, from list_models; left out, the teammate's own. read_only true runs it in Ask whatever else is named. The reply says the mode and folder used. No automatic teammate handoffs. Shares remain visible in Locust. API/account usage and Locust monthly limits apply. Returns a conversation id; use read_reply to poll."#,
            schema(json!({"teammate": name, "message": message, "model": name, "effort": effort, "read_only": flag, "mode": mode, "folder": folder}), &["teammate", "message"]), false),
        ("send_message", r#"Continue a conversation started by this server, with no automatic teammate handoffs: in the conversation's own mode and folder, unless you name others as start_conversation takes them. Optionally a model, effort and read_only for this turn, as start_conversation takes them. No message is queued while it is working; poll read_reply before sending."#,
            schema(json!({"conversation_id": name, "message": message, "model": name, "effort": effort, "read_only": flag, "mode": mode, "folder": folder}), &["conversation_id", "message"]), false),
        ("read_reply", r#"Read the newest finished answer, current activity while still working (including a wait for the person to approve an action in Locust), or Locust's own failure words. Optionally wait_seconds, up to 20, to wait for the turn to finish first. Read only."#,
            schema(json!({"conversation_id": name, "wait_seconds": {"type": "integer", "minimum": 0, "maximum": 20}}), &["conversation_id"]), true),
        ("stop_turn", r#"Stop a turn started from here that is still working, by its conversation id. What it already changed stays, and the person can undo it in Locust. Never stops a turn the person started, unless they let apps see their conversations."#,
            schema(json!({"conversation_id": name}), &["conversation_id"]), false),
        ("list_conversations", r#"List the recent conversations in the folder Locust has open, the person's own too, newest first: id, teammate, title, who started it, AI agent, model, and state, including a turn stopped because its plan ran out, which carry_on can move to another agent. Only while the person has turned on "Let apps see your conversations" in Locust. Read only."#, schema(json!({}), &[]), true),
        ("carry_on", r#"Carry a conversation whose turn stopped because its plan ran out, or its provider failed first, on to another AI agent and model from list_models, as that turn's card in Locust does: the new turn is told what was already done. It runs in the stopped turn's own mode, and only an Ask turn unless the person has turned on "Use each teammate's own mode" in Locust."#,
            schema(json!({"conversation_id": name, "runtime": name, "model": name}), &["conversation_id", "runtime"]), false),
        ("list_models", r#"List the models a teammate's AI agent offers in this Locust, with the efforts each takes, and the model and effort it runs on now. With no teammate, every AI agent this Locust can run and its models, for carry_on. Read only."#, schema(json!({"teammate": name}), &[]), true),
        ("list_background_runs", r#"List Claude turns running in the background. Read only; never starts or stops them."#, schema(json!({}), &[]), true),
        ("list_routines", r#"List Locust routines (saved multi-step workflows): each step's teammate and mode, the inputs a run asks for, whether it changes files, and whether run_routine can start it from here. Read only."#, schema(json!({}), &[]), true),
        ("run_routine", r#"Run a Locust routine by id or name, with values for its inputs. A routine that changes files runs only if the person has turned on "Use each teammate's own mode" in Locust; any approval is answered by the person in Locust's window, never from here. A routine that asks for a folder runs only from Locust. API/account usage and Locust monthly limits apply. Returns the routine id; poll routine_status."#,
            schema(json!({"routine": name, "values": {"type": "object", "additionalProperties": {"type": "string", "maxLength": 4000}, "maxProperties": 12}}), &["routine"]), false),
        ("routine_status", r#"The step a routine started by run_routine is on, a wait for the person in Locust, or, once it ends, its last answer. Read only."#, schema(json!({"routine": name}), &["routine"]), true),
    ];
    Value::Array(
        tools
            .into_iter()
            .map(|(name, description, input, reads)| {
                let mut tool = json!({"name": name, "description": description, "inputSchema": input});
                if reads {
                    tool["annotations"] = json!({"readOnlyHint": true});
                }
                tool
            })
            .collect(),
    )
}

/// What the relay passes on: an app's request, and what this computer sends back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskKind {
    Connect,
    Token,
    Call,
}
#[derive(Debug, Clone)]
pub struct RemoteMcpAsk {
    pub ask: String,
    pub kind: AskKind,
    pub body: String,
    pub auth: Option<String>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteMcpReply {
    pub status: u16,
    pub body: String,
}

/// An app with keys: what it called itself, where it sends the person back, and when.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteMcpApp {
    pub id: String,
    pub name: String,
    pub host: String,
    pub added: String,
    pub used: Option<String>,
}
/// A sign-in waiting on the person's card.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteMcpAsking {
    pub request: String,
    pub name: String,
    /// Where the app sends the person back, as a host: the app is who that is.
    pub host: String,
    /// Six digits the sign-in page shows too.
    pub match_digits: String,
    pub until: i64,
}
/// What Settings and the window's card are told.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteMcpState {
    pub enabled: bool,
    /// The address an app is given, while this and From anywhere are both on.
    pub address: Option<String>,
    pub apps: Vec<RemoteMcpApp>,
    pub asking: Vec<RemoteMcpAsking>,
    pub message: Option<String>,
}

impl RemoteMcpState {
    /// `RemoteMcpState` as the renderer reads it (shared/remote-mcp.ts).
    pub fn to_json(&self) -> Value {
        let mut out = Map::new();
        out.insert("enabled".into(), json!(self.enabled));
        if let Some(address) = &self.address {
            out.insert("address".into(), json!(address));
        }
        out.insert(
            "apps".into(),
            Value::Array(
                self.apps
                    .iter()
                    .map(|app| {
                        let mut one = json!({"id": app.id, "name": app.name, "host": app.host, "added": app.added});
                        if let Some(used) = &app.used {
                            one["used"] = json!(used);
                        }
                        one
                    })
                    .collect(),
            ),
        );
        out.insert(
            "asking".into(),
            Value::Array(self.asking.iter().map(|a| json!({"request": a.request, "name": a.name, "host": a.host, "match": a.match_digits, "until": a.until})).collect()),
        );
        if let Some(message) = &self.message {
            out.insert("message".into(), json!(message));
        }
        Value::Object(out)
    }
}

/// The relay, behind a trait: tests bring a fake, and the real transport comes later (see [`StubMcpRelay`]).
pub trait McpRelay: Send + Sync {
    /// Where apps connect, while From anywhere is on: `None` otherwise.
    fn address(&self) -> Option<String>;
    /// The relay is told it may take apps, or no longer. `false` when it could not be reached.
    fn open(&self, on: bool) -> bool;
}

/// The live relay transport is not here yet: Apple's NSURLSession transport (the chat-apps lane) is not in
/// `core/Cargo.toml`, and `ureq` has no TLS. With this, From anywhere is never on, so nothing can be turned on.
pub struct StubMcpRelay;
impl McpRelay for StubMcpRelay {
    fn address(&self) -> Option<String> {
        None
    }
    fn open(&self, _on: bool) -> bool {
        false
    }
}

/// Locust's tool calls, as an app on this computer makes them (the door's engine).
pub trait McpTools: Send + Sync {
    fn call(&self, name: &str, arguments: &Value) -> Result<ToolResult, String>;
}

/// The door's own tools (`door::tools_from_anywhere`), or any function that answers a call as they do.
impl<F> McpTools for F
where
    F: Fn(&str, &Value) -> ToolResult + Send + Sync,
{
    fn call(&self, name: &str, arguments: &Value) -> Result<ToolResult, String> {
        Ok(self(name, arguments))
    }
}

type Now = Arc<dyn Fn() -> i64 + Send + Sync>;

pub struct Options {
    pub file: PathBuf,
    pub relay: Arc<dyn McpRelay>,
    pub tools: Arc<dyn McpTools>,
    /// Why apps may not use this Locust from anywhere (a company's rule), when they may not.
    pub refusal: Arc<dyn Fn() -> Option<String> + Send + Sync>,
    /// Told whenever what Settings and the card show changed.
    pub changed: Arc<dyn Fn(&RemoteMcpState) + Send + Sync>,
    /// An app asks to connect: said where the person may be, besides the card.
    pub asked: Arc<dyn Fn(&str) + Send + Sync>,
    pub now: Now,
    pub version: String,
    /// How long a sign-in card waits for the person (ASKING_MS; shorter in tests).
    pub asking_wait: Duration,
}

impl Options {
    pub fn new(file: PathBuf, relay: Arc<dyn McpRelay>, tools: Arc<dyn McpTools>) -> Self {
        Self {
            file,
            relay,
            tools,
            refusal: Arc::new(|| None),
            changed: Arc::new(|_| {}),
            asked: Arc::new(|_| {}),
            now: Arc::new(|| std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)),
            version: "1".into(),
            asking_wait: Duration::from_millis(ASKING_MS as u64),
        }
    }
}

#[derive(Debug, Clone)]
struct Grant {
    id: String,
    name: String,
    client_id: String,
    host: String,
    added: i64,
    used: Option<i64>,
    access: String,
    access_until: i64,
    refresh: String,
    refresh_until: i64,
}
impl Grant {
    fn to_json(&self) -> Value {
        let mut one = json!({"id": self.id, "name": self.name, "clientId": self.client_id, "host": self.host, "added": self.added});
        if let Some(used) = self.used {
            one["used"] = json!(used);
        }
        one["access"] = json!(self.access);
        one["accessUntil"] = json!(self.access_until);
        one["refresh"] = json!(self.refresh);
        one["refreshUntil"] = json!(self.refresh_until);
        one
    }
    fn from_json(value: &Value) -> Option<Self> {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        let number = |key: &str| value.get(key).and_then(Value::as_f64).map(|n| n as i64);
        Some(Self {
            id: text("id")?,
            name: text("name").unwrap_or_default(),
            client_id: text("clientId").unwrap_or_default(),
            host: text("host").unwrap_or_default(),
            added: number("added").unwrap_or(0),
            used: number("used"),
            access: text("access")?,
            access_until: number("accessUntil").unwrap_or(0),
            refresh: text("refresh")?,
            refresh_until: number("refreshUntil").unwrap_or(0),
        })
    }
}

struct Code {
    hash: String,
    client_id: String,
    redirect: String,
    challenge: String,
    name: String,
    host: String,
    until: i64,
}

type Decision = Arc<(Mutex<Option<bool>>, Condvar)>;
struct Asking {
    request: String,
    name: String,
    host: String,
    match_digits: String,
    until: i64,
    decision: Decision,
}

struct Inner {
    enabled: bool,
    grants: Vec<Grant>,
    message: Option<String>,
    codes: Vec<Code>,
    asking: Vec<Asking>,
    /// When an app's last use is next written down.
    used_due: Option<i64>,
}

/// Where an app sends the person back, as the card names it: the host, never a whole address.
pub fn host_of(redirect: &str) -> String {
    match url::Url::parse(redirect) {
        Ok(url) if url.scheme() == "http" => "an app on the computer it was started on".into(),
        Ok(url) => {
            let host = url.host_str().unwrap_or("");
            match url.port() {
                Some(port) => format!("{host}:{port}"),
                None => host.to_owned(),
            }
        }
        Err(_) => "an address it did not make clear".into(),
    }
}

fn same_hash(one: &str, other: &str) -> bool {
    one.len() == other.len() && one.bytes().zip(other.bytes()).fold(0u8, |diff, (a, b)| diff | (a ^ b)) == 0
}
fn reply(status: u16, body: Value) -> RemoteMcpReply {
    RemoteMcpReply { status, body: body.to_string() }
}
fn oauth_error(error: &str, words: &str) -> RemoteMcpReply {
    reply(400, json!({"error": error, "error_description": words}))
}
fn rpc_error(id: Option<&Value>, code: i64, words: &str) -> RemoteMcpReply {
    reply(200, json!({"jsonrpc": "2.0", "id": id.cloned().unwrap_or(Value::Null), "error": {"code": code, "message": words}}))
}
fn mcp_text(text: &str, is_error: bool) -> Value {
    ToolResult { text: text.into(), is_error, structured: None }.to_json()
}
fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}
fn is_key(text: &str) -> bool {
    text.len() == 43 && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
fn is_verifier(text: &str) -> bool {
    (43..=128).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'~' | b'-'))
}

pub struct RemoteMcp {
    options: Options,
    inner: Mutex<Inner>,
    // Do not send an old enabled snapshot after a newer settings change.
    relay_transition: Mutex<()>,
    /// Writes to the file one at a time, in order.
    writing: Mutex<()>,
}

impl RemoteMcp {
    pub fn new(options: Options) -> Self {
        Self {
            options,
            inner: Mutex::new(Inner { enabled: false, grants: Vec::new(), message: None, codes: Vec::new(), asking: Vec::new(), used_due: None }),
            relay_transition: Mutex::new(()),
            writing: Mutex::new(()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    fn now(&self) -> i64 {
        (self.options.now)()
    }

    fn state_of(&self, inner: &Inner) -> RemoteMcpState {
        let address = self.options.relay.address();
        let now = self.now();
        RemoteMcpState {
            enabled: inner.enabled,
            address: if inner.enabled { address } else { None },
            apps: inner
                .grants
                .iter()
                .map(|g| RemoteMcpApp { id: g.id.clone(), name: g.name.clone(), host: g.host.clone(), added: iso_millis(g.added), used: g.used.map(iso_millis) })
                .collect(),
            asking: inner
                .asking
                .iter()
                .filter(|a| a.until > now)
                .map(|a| RemoteMcpAsking { request: a.request.clone(), name: a.name.clone(), host: a.host.clone(), match_digits: a.match_digits.clone(), until: a.until })
                .collect(),
            message: inner.message.clone(),
        }
    }
    /// What the settings are now, without the due write: the window reads them through `look`.
    pub(crate) fn settings(&self) -> RemoteMcpState {
        let inner = self.lock();
        self.state_of(&inner)
    }
    /// Never called with the lock held: a listener may ask for the state.
    fn tell(&self) {
        let state = self.settings();
        (self.options.changed)(&state);
    }

    fn write(&self, enabled: bool, grants: &[Grant]) -> Result<(), String> {
        let _one_at_a_time = self.writing.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(parent) = self.options.file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let text = json!({"v": 1, "enabled": enabled, "grants": grants.iter().map(Grant::to_json).collect::<Vec<_>>()}).to_string();
        let temp = PathBuf::from(format!("{}.tmp", self.options.file.display()));
        {
            use std::io::Write;
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
            let mut file = options.open(&temp).map_err(|e| e.to_string())?;
            file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&temp, &self.options.file).map_err(|e| e.to_string())
    }
    fn persist(&self) -> Result<(), String> {
        let (enabled, grants) = {
            let mut inner = self.lock();
            inner.used_due = None;
            (inner.enabled, inner.grants.clone())
        };
        self.write(enabled, &grants)
    }

    /// A card the person answers: Allow makes a code the app trades for keys; Deny, or no answer, makes none.
    fn connect(&self, body: &str) -> RemoteMcpReply {
        let Ok(Value::Object(asked)) = serde_json::from_str::<Value>(body) else { return reply(400, json!({"error": "bad-ask"})) };
        let text = |key: &str| asked.get(key).and_then(Value::as_str).unwrap_or("").to_owned();
        let client_id = text("client_id");
        let redirect = text("redirect");
        let challenge = text("challenge");
        let digits = text("match");
        let digits = if digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_digit()) { digits } else { String::new() };
        let shown_name = match asked.get("client").and_then(Value::as_str) {
            Some(name) if !name.trim().is_empty() => name.chars().take(80).collect::<String>(),
            _ => "An app".to_owned(),
        };
        let enabled = self.lock().enabled;
        if !enabled || (self.options.refusal)().is_some() || client_id.is_empty() || redirect.is_empty() || !is_key(&challenge) || digits.is_empty() {
            return reply(403, json!({"error": "denied"}));
        }
        let Ok(request) = random_base64url(12) else { return reply(500, json!({"error": "server_error"})) };
        let decision: Decision = Arc::new((Mutex::new(None), Condvar::new()));
        {
            let mut inner = self.lock();
            let until = self.now() + ASKING_MS;
            inner.asking.push(Asking { request: request.clone(), name: shown_name.clone(), host: host_of(&redirect), match_digits: digits, until, decision: decision.clone() });
        }
        self.tell();
        (self.options.asked)(&shown_name);
        let allowed = {
            let (answer, bell) = &*decision;
            let guard = answer.lock().unwrap_or_else(|p| p.into_inner());
            let (guard, _timeout) = bell.wait_timeout_while(guard, self.options.asking_wait, |answer| answer.is_none()).unwrap_or_else(|p| p.into_inner());
            *guard
        };
        let Some(allowed) = allowed else {
            // No answer: the card goes, and no code is made.
            let was_there = {
                let mut inner = self.lock();
                let before = inner.asking.len();
                inner.asking.retain(|a| a.request != request);
                before != inner.asking.len()
            };
            if was_there {
                self.tell();
            }
            return reply(403, json!({"error": "denied"}));
        };
        if !allowed || !self.lock().enabled {
            return reply(403, json!({"error": "denied"}));
        }
        let Ok(code) = random_base64url(32) else { return reply(500, json!({"error": "server_error"})) };
        let mut inner = self.lock();
        let until = self.now() + CODE_MS;
        inner.codes.push(Code { hash: sha256_base64url(&code), client_id, redirect: redirect.clone(), challenge, name: shown_name, host: host_of(&redirect), until });
        drop(inner);
        reply(200, json!({"code": code}))
    }

    /// Keys for an app: its access and refresh keys made, only their hashes kept, the oldest-used app gone past MAX_APPS.
    fn keys(&self, mut grant: Grant) -> RemoteMcpReply {
        let (Ok(access), Ok(refresh)) = (random_base64url(32), random_base64url(32)) else { return reply(500, json!({"error": "server_error"})) };
        let now = self.now();
        grant.access = sha256_base64url(&access);
        grant.access_until = now + ACCESS_MS;
        grant.refresh = sha256_base64url(&refresh);
        grant.refresh_until = now + REFRESH_MS;
        let (enabled, next) = {
            let inner = self.lock();
            let mut others: Vec<&Grant> = inner.grants.iter().filter(|one| one.id != grant.id).collect();
            // Past MAX_APPS, the app used longest ago goes.
            others.sort_by_key(|one| std::cmp::Reverse(one.used.unwrap_or(one.added)));
            let kept: Vec<&str> = others.iter().take(MAX_APPS - 1).map(|one| one.id.as_str()).collect();
            let mut next: Vec<Grant> = inner.grants.iter().filter(|one| kept.contains(&one.id.as_str())).cloned().collect();
            next.push(grant);
            (inner.enabled, next)
        };
        if self.write(enabled, &next).is_err() {
            // Keys the computer could not keep are keys it must not give.
            return reply(500, json!({"error": "server_error"}));
        }
        {
            let mut inner = self.lock();
            inner.grants = next;
            inner.used_due = None;
        }
        self.tell();
        reply(200, json!({"access_token": access, "token_type": "Bearer", "expires_in": ACCESS_MS / 1000, "refresh_token": refresh, "scope": "locust"}))
    }

    /// RFC 6749 with PKCE: a code for keys, once; or a refresh key for new keys, the old ones gone.
    fn token(&self, body: &str) -> RemoteMcpReply {
        if !self.lock().enabled || (self.options.refusal)().is_some() {
            return oauth_error("invalid_grant", "This Locust no longer takes apps from anywhere.");
        }
        let fields: Vec<(String, String)> = if body.trim_start().starts_with('{') {
            match serde_json::from_str::<Value>(body) {
                Ok(Value::Object(map)) => map.into_iter().map(|(k, v)| (k, v.as_str().map_or_else(|| v.to_string(), str::to_owned))).collect(),
                _ => return oauth_error("invalid_request", "The request could not be read."),
            }
        } else {
            url::form_urlencoded::parse(body.as_bytes()).into_owned().collect()
        };
        let field = |key: &str| fields.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
        let now = self.now();
        match field("grant_type") {
            Some("authorization_code") => {
                let presented = sha256_base64url(field("code").unwrap_or(""));
                let found = {
                    let mut inner = self.lock();
                    let at = inner.codes.iter().position(|one| same_hash(&one.hash, &presented));
                    // Good once, right or wrong: a code tried is gone.
                    at.map(|at| inner.codes.remove(at))
                };
                let Some(found) = found else { return oauth_error("invalid_grant", "That code is not one this Locust made, or it was used already.") };
                let verifier = field("code_verifier").unwrap_or("");
                if found.until <= now {
                    return oauth_error("invalid_grant", "That code ran out. Connect again from the app.");
                }
                if field("client_id") != Some(found.client_id.as_str()) || field("redirect_uri").unwrap_or(&found.redirect) != found.redirect {
                    return oauth_error("invalid_grant", "That code was made for another app.");
                }
                if !is_verifier(verifier) || !same_hash(&sha256_base64url(verifier), &found.challenge) {
                    return oauth_error("invalid_grant", "The app could not prove it started this sign-in.");
                }
                let Ok(id) = random_base64url(9) else { return reply(500, json!({"error": "server_error"})) };
                self.keys(Grant { id, name: found.name, client_id: found.client_id, host: found.host, added: now, used: None, access: String::new(), access_until: 0, refresh: String::new(), refresh_until: 0 })
            }
            Some("refresh_token") => {
                let presented = sha256_base64url(field("refresh_token").unwrap_or(""));
                let grant = self.lock().grants.iter().find(|one| same_hash(&one.refresh, &presented)).cloned();
                let Some(grant) = grant.filter(|one| one.refresh_until > now) else {
                    return oauth_error("invalid_grant", "That refresh key is not one this Locust holds. Connect again from the app.");
                };
                if field("client_id").is_some_and(|given| given != grant.client_id) {
                    return oauth_error("invalid_grant", "That refresh key was made for another app.");
                }
                self.keys(Grant { used: Some(now), ..grant })
            }
            _ => reply(400, json!({"error": "unsupported_grant_type"})),
        }
    }

    /// The app's last use, kept at most once a minute: a call is not a disk write ([`RemoteMcp::tick`] writes it).
    fn note_use(&self, id: &str) {
        let at = self.now();
        let mut inner = self.lock();
        for one in inner.grants.iter_mut().filter(|one| one.id == id) {
            one.used = Some(at);
        }
        if inner.used_due.is_none() {
            inner.used_due = Some(at + USED_SAVE_MS);
        }
    }
    /// The window's look, every poll: writes an app's last use once it is a minute old (`tick`), then says the settings.
    /// One call, so a window cannot read the settings and forget the write.
    pub fn look(&self) -> RemoteMcpState {
        self.tick();
        self.settings()
    }

    /// Called now and then by whoever owns this: writes an app's last use once it is a minute old, and tells the window.
    pub fn tick(&self) {
        let due = { self.lock().used_due.is_some_and(|due| due <= self.now()) };
        if due {
            let _ = self.persist();
            self.tell();
        }
    }

    fn rpc(&self, id: Option<&Value>, result: Value) -> RemoteMcpReply {
        let id = id.cloned().unwrap_or(Value::Null);
        let text = json!({"jsonrpc": "2.0", "id": id, "result": result}).to_string();
        if utf16_len(&text) <= MAX_ANSWER_CHARS {
            return RemoteMcpReply { status: 200, body: text };
        }
        reply(200, json!({"jsonrpc": "2.0", "id": id, "result": mcp_text(TOO_LONG, true)}))
    }

    /// One MCP request, with the key it came with. JSON-RPC, one at a time, answered as JSON.
    fn call(&self, auth: Option<&str>, body: &str) -> RemoteMcpReply {
        let presented = auth.filter(|key| is_key(key)).map(sha256_base64url);
        let now = self.now();
        let grant = presented.and_then(|presented| self.lock().grants.iter().find(|one| same_hash(&one.access, &presented)).cloned());
        let enabled = self.lock().enabled;
        let Some(grant) = grant.filter(|one| enabled && (self.options.refusal)().is_none() && one.access_until > now) else {
            return reply(401, json!({"error": "invalid_token"}));
        };
        let request = match serde_json::from_str::<Value>(body) {
            Ok(Value::Object(map)) => map,
            Ok(_) => return rpc_error(None, -32600, "Send one JSON-RPC request at a time."),
            Err(_) => return rpc_error(None, -32700, "The request is not JSON."),
        };
        self.note_use(&grant.id);
        let id = request.get("id");
        let method = request.get("method").and_then(Value::as_str);
        let Some(method) = method.filter(|_| request.get("jsonrpc") == Some(&json!("2.0"))) else { return rpc_error(id, -32600, "Invalid request.") };
        // A notification is answered with nothing.
        if id.is_none() {
            return RemoteMcpReply { status: 202, body: String::new() };
        }
        let empty = Map::new();
        let params = request.get("params").and_then(Value::as_object).unwrap_or(&empty);
        match method {
            "initialize" => {
                let asked = PROTOCOLS.iter().find(|one| params.get("protocolVersion").and_then(Value::as_str) == Some(**one)).unwrap_or(&PROTOCOLS[0]);
                self.rpc(id, json!({"protocolVersion": asked, "capabilities": {"tools": {}}, "serverInfo": {"name": "locust", "version": self.options.version}}))
            }
            "ping" => self.rpc(id, json!({})),
            "tools/list" => self.rpc(id, json!({"tools": remote_mcp_tools()})),
            "tools/call" => {
                let tools = remote_mcp_tools();
                let wanted = params.get("name").and_then(Value::as_str);
                let Some(tool) = tools.as_array().and_then(|all| all.iter().find(|one| one["name"].as_str() == wanted && wanted.is_some())) else {
                    return rpc_error(id, -32602, "Unknown Locust tool.");
                };
                let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));
                let result = match self.options.tools.call(tool["name"].as_str().unwrap_or(""), &arguments) {
                    Ok(result) => result.to_json(),
                    Err(_) => mcp_text(NOT_COMPLETED, true),
                };
                self.rpc(id, result)
            }
            _ => rpc_error(id, -32601, "Method not found."),
        }
    }

    /// The saved state, read when Locust starts; an expired app is not kept.
    pub fn load(&self) -> RemoteMcpState {
        if let Ok(text) = std::fs::read_to_string(&self.options.file)
            && let Ok(read) = serde_json::from_str::<Value>(&text)
        {
            let now = self.now();
            let grants = read["grants"].as_array().map(|all| all.iter().filter_map(Grant::from_json).filter(|g| g.refresh_until > now).collect()).unwrap_or_default();
            let mut inner = self.lock();
            inner.enabled = read["enabled"] == json!(true) && (self.options.refusal)().is_none();
            inner.grants = grants;
        }
        self.settings()
    }

    /// The relay is told again, as the PC connects: it answers apps only while this says so.
    pub fn sync(&self) {
        let _transition = self.relay_transition.lock().unwrap_or_else(|p| p.into_inner());
        let enabled = self.lock().enabled;
        self.options.relay.open(enabled);
    }

    pub fn set_enabled(&self, next: &Value) -> RemoteMcpState {
        let transition = self.relay_transition.lock().unwrap_or_else(|p| p.into_inner());
        self.lock().message = None;
        let Some(next) = next.as_bool() else { return self.settings() };
        if next {
            if let Some(refused) = (self.options.refusal)() {
                self.lock().message = Some(refused);
                return self.settings();
            }
            if self.options.relay.address().is_none() {
                self.lock().message = Some(NEEDS_FROM_ANYWHERE.into());
                return self.settings();
            }
            if !self.options.relay.open(true) {
                self.lock().message = Some(RELAY_UNREACHED.into());
                return self.settings();
            }
            self.lock().enabled = true;
        } else {
            // Off takes every app's keys away, here and at the relay.
            let waiting: Vec<Decision> = {
                let mut inner = self.lock();
                let waiting = inner.asking.drain(..).map(|a| a.decision).collect();
                inner.codes.clear();
                inner.enabled = false;
                inner.grants.clear();
                waiting
            };
            for decision in waiting {
                settle(&decision, false);
            }
            self.options.relay.open(false);
        }
        if self.persist().is_err() {
            self.lock().message = Some(NOT_SAVED.into());
        }
        drop(transition);
        self.tell();
        self.settings()
    }

    pub fn remove(&self, id: &str) -> RemoteMcpState {
        self.lock().grants.retain(|grant| grant.id != id);
        let _ = self.persist();
        self.tell();
        self.settings()
    }

    /// The person's answer to a card.
    pub fn answer_sign_in(&self, request: &str, allow: bool) -> RemoteMcpState {
        let one = {
            let mut inner = self.lock();
            let enabled = inner.enabled;
            inner.asking.iter().position(|a| a.request == request).map(|at| (inner.asking.remove(at).decision, enabled))
        };
        if let Some((decision, enabled)) = one {
            settle(&decision, allow && enabled);
            self.tell();
        }
        self.settings()
    }

    /// An app's request the relay passed on. A connect waits here for the person's answer on the card.
    pub fn answer(&self, asked: &RemoteMcpAsk) -> RemoteMcpReply {
        match asked.kind {
            AskKind::Connect => self.connect(&asked.body),
            AskKind::Token => self.token(&asked.body),
            AskKind::Call => self.call(asked.auth.as_deref(), &asked.body),
        }
    }

    pub fn dispose(&self) {
        let waiting: Vec<Decision> = self.lock().asking.drain(..).map(|a| a.decision).collect();
        for decision in waiting {
            settle(&decision, false);
        }
    }
}

fn settle(decision: &Decision, allowed: bool) {
    let (answer, bell) = &**decision;
    let mut guard = answer.lock().unwrap_or_else(|p| p.into_inner());
    if guard.is_none() {
        *guard = Some(allowed);
    }
    bell.notify_all();
}

#[cfg(test)]
#[path = "remote_mcp_tests.rs"]
mod tests;

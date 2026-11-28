//! THE `locust` COMMAND (Electron `packages/cli/src/client.ts` and `index.ts`): a terminal's own client for Locust's
//! door. It reads the connection file Locust writes in its profile folder (`locust-mcp-connection.json`), checks that
//! it names this computer's loopback `/tools/call` and a 64-hex token, and posts `{name, arguments}` with the token as
//! a bearer. The door answers the private `cli/*` names in front of its tools (`cli_sessions.rs`), so a turn sent from
//! here is a turn the window shows, under the window's rules.
//!
//! No redirects, proxies, shell, automatic retry, or reads of another app's config: a refusal is passed through in
//! Locust's own words, and nothing is sent twice. Every call has an absolute deadline. A prompt that is interrupted, or
//! whose reply cannot be read, is cancelled by its own request id, so only this prompt stops.

use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

const SWITCH: &str = "Open Locust and turn on Settings > General > Let your other AI apps use Locust.";
const MAX_REPLY: u64 = 5 * 1024 * 1024;
/// The file Locust writes beside its settings (`MCP_CONNECTION_FILE`).
pub const CONNECTION_FILE: &str = "locust-mcp-connection.json";

pub const HELP: &str = "Usage: locust [--profile <Locust profile folder>] teammates | status | send <teammate> \"<message>\" | acp [--teammate <name or id>]
Locust must already be open, with Settings > General > Let your other AI apps use Locust on.
Turns use Ask mode unless Use each teammate's own mode is on in Locust. Cards are answered in Locust; an editor
answers its own turn's cards only when Let editors answer cards for their own turns is on. This command never signs
in or changes settings.";

/// Words for the person, said as they are (Electron's `LocalError`).
#[derive(Debug, Clone, PartialEq)]
pub struct LocalError(pub String);

impl std::fmt::Display for LocalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn local(text: impl Into<String>) -> LocalError {
    LocalError(text.into())
}

/// Electron's appData locations. `--profile` explicitly selects a development or `--user-data-dir` profile.
/// `platform` is Node's name for it: `win32`, `darwin`, `linux`.
pub fn profile_directory(platform: &str, env: &dyn Fn(&str) -> Option<String>, home: &str) -> Result<String, LocalError> {
    let set = |name: &str| env(name).filter(|v| !v.is_empty());
    if platform == "win32" {
        let Some(app_data) = set("APPDATA") else {
            return Err(local("The Windows profile folder could not be found. Pass --profile with Locust’s profile folder."));
        };
        return Ok(format!("{}\\@teammate\\desktop", app_data.trim_end_matches(['\\', '/'])));
    }
    let posix = |base: &str, rest: &str| format!("{}/{rest}", base.trim_end_matches('/'));
    if platform == "darwin" {
        return Ok(posix(home, "Library/Application Support/@teammate/desktop"));
    }
    let config = set("XDG_CONFIG_HOME").unwrap_or_else(|| posix(home, ".config"));
    Ok(posix(&config, "@teammate/desktop"))
}

/// Where the command looks when no `--profile` is given: Electron's profile, or the native app's own data folder
/// beside it (`<profile>-native`, or `LOCUST_NATIVE_DATA`) when only the native window's door is open.
fn default_profile() -> Result<PathBuf, LocalError> {
    let platform = match std::env::consts::OS {
        "windows" => "win32",
        "macos" => "darwin",
        _ => "linux",
    };
    let env = |name: &str| std::env::var(name).ok();
    let home = env("HOME").or_else(|| env("USERPROFILE")).unwrap_or_default();
    let electron = PathBuf::from(profile_directory(platform, &env, &home)?);
    if electron.join(CONNECTION_FILE).exists() {
        return Ok(electron);
    }
    let native = env("LOCUST_NATIVE_DATA").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| {
        let mut name = electron.as_os_str().to_owned();
        name.push("-native");
        PathBuf::from(name)
    });
    Ok(if native.join(CONNECTION_FILE).exists() { native } else { electron })
}

/// The connection file's url and token, or nothing when it names anything but this computer's `/tools/call`.
fn connection_of(raw: &str) -> Option<(String, String)> {
    let parsed: Value = serde_json::from_str(raw).ok()?;
    let (url, token) = (parsed.get("url")?.as_str()?, parsed.get("token")?.as_str()?);
    if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return None;
    }
    let address = url::Url::parse(url).ok()?;
    let plain = address.scheme() == "http"
        && address.host_str() == Some("127.0.0.1")
        && address.path() == "/tools/call"
        && address.port().is_some()
        && address.query().is_none()
        && address.fragment().is_none()
        && address.username().is_empty()
        && address.password().is_none();
    plain.then(|| (url.to_owned(), token.to_owned()))
}

/// Stops what it was given to (Electron's `AbortController`): every listener runs once, when it is aborted.
#[derive(Clone, Default)]
pub struct Abort(Arc<Mutex<AbortState>>);

#[derive(Default)]
struct AbortState {
    aborted: bool,
    next: u64,
    listeners: Vec<(u64, Box<dyn FnOnce() + Send>)>,
}

impl Abort {
    pub fn new() -> Self {
        Self::default()
    }

    fn state(&self) -> std::sync::MutexGuard<'_, AbortState> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn abort(&self) {
        let listeners = {
            let mut state = self.state();
            if state.aborted {
                return;
            }
            state.aborted = true;
            std::mem::take(&mut state.listeners)
        };
        for (_, listener) in listeners {
            listener();
        }
    }

    pub fn aborted(&self) -> bool {
        self.state().aborted
    }

    /// Runs `listener` once if this is aborted later; answers the id that takes it off again.
    fn on_abort(&self, listener: Box<dyn FnOnce() + Send>) -> Option<u64> {
        let mut state = self.state();
        if state.aborted {
            return None;
        }
        state.next += 1;
        let id = state.next;
        state.listeners.push((id, listener));
        Some(id)
    }

    fn off(&self, id: u64) {
        self.state().listeners.retain(|(i, _)| *i != id);
    }

    /// Sleep that ends early when this is aborted.
    pub(crate) fn sleep(&self, how_long: Duration) {
        let until = Instant::now() + how_long;
        while !self.aborted() {
            let now = Instant::now();
            if now >= until {
                return;
            }
            std::thread::sleep((until - now).min(Duration::from_millis(5)));
        }
    }
}

/// A teammate as the command sees it (`CliTeammate`).
#[derive(Debug, Clone, PartialEq)]
pub struct CliTeammate {
    pub id: String,
    pub name: String,
    pub runtime: Option<String>,
    pub model: Option<String>,
    pub runs_in: String,
}

impl CliTeammate {
    fn of(value: &Value) -> Option<Self> {
        let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        Some(Self { id: text("id")?, name: text("name")?, runtime: text("runtime"), model: text("model"), runs_in: text("runs_in")? })
    }
}

/// One editor session: its id, and its capability key, which is never printed, logged or kept.
#[derive(Debug, Clone, PartialEq)]
pub struct CliSession {
    pub session_id: String,
    pub key: String,
    pub teammate: CliTeammate,
    pub workspace: String,
}

/// One piece of a reply's text (`CliText`).
#[derive(Debug, Clone, PartialEq)]
pub struct CliText {
    pub sequence: u64,
    pub item_id: String,
    /// `append` or `replace`.
    pub operation: String,
    pub text: String,
}

/// A card waiting for the person (`CliApproval`).
#[derive(Debug, Clone, PartialEq)]
pub struct CliApproval {
    pub approval_id: String,
    pub kind: String,
    pub summary: String,
    pub detail: String,
    pub cwd: Option<String>,
    pub answer_in_locust: bool,
}

/// One running or waiting turn in Locust (`CliStatus.turns`).
#[derive(Debug, Clone, PartialEq)]
pub struct CliStatusTurn {
    pub mission_id: String,
    pub teammate: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CliStatus {
    pub workspace: String,
    pub turns: Vec<CliStatusTurn>,
}

/// How a prompt ended when it did not fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    EndTurn,
    Cancelled,
}

const UNREADABLE: &str = "Locust returned an unreadable answer. Update Locust; nothing was retried.";

fn unreadable() -> LocalError {
    local(UNREADABLE)
}

/// A cancel sent in the background: still running, or how it ended (a promise awaited more than once).
enum Sent {
    None,
    Running(std::thread::JoinHandle<Result<Value, LocalError>>),
    Done(Result<(), LocalError>),
}

fn settle(sent: &Mutex<Sent>) -> Result<(), LocalError> {
    let mut held = sent.lock().unwrap_or_else(|e| e.into_inner());
    match std::mem::replace(&mut *held, Sent::None) {
        Sent::None => Ok(()),
        Sent::Running(handle) => {
            let done = handle.join().unwrap_or_else(|_| Err(local("Locust could not finish. Check the conversation in Locust."))).map(|_| ());
            *held = Sent::Done(done.clone());
            done
        }
        Sent::Done(done) => {
            *held = Sent::Done(done.clone());
            done
        }
    }
}

/// The command's connection to Locust.
#[derive(Clone)]
pub struct LocustClient {
    profile: PathBuf,
    timeout: Duration,
    #[cfg(test)]
    test_call: Option<TestCall>,
}

#[cfg(test)]
type TestCall = Arc<dyn Fn(&str, Value) -> Result<Value, LocalError> + Send + Sync>;

impl LocustClient {
    pub fn new(profile: impl Into<PathBuf>) -> Self {
        Self::with_timeout(profile, Duration::from_millis(15_000))
    }

    pub fn with_timeout(profile: impl Into<PathBuf>, timeout: Duration) -> Self {
        Self { profile: profile.into(), timeout, #[cfg(test)] test_call: None }
    }

    #[cfg(test)]
    pub(crate) fn for_test(call: TestCall) -> Self {
        Self { test_call: Some(call), ..Self::new(PathBuf::new()) }
    }

    /// One call: the connection file read fresh, one request with the token, the answer's text read as JSON.
    pub fn call(&self, name: &str, args: Value) -> Result<Value, LocalError> {
        #[cfg(test)]
        if let Some(call) = &self.test_call {
            return call(name, args);
        }
        let not_open = || local(format!("Locust is not open, or its local server is off. {SWITCH}"));
        let raw = std::fs::read_to_string(self.profile.join(CONNECTION_FILE)).map_err(|_| not_open())?;
        let (url, token) = connection_of(&raw).ok_or_else(not_open)?;
        let late = || local("Locust did not answer in time. Nothing was retried; check the conversation in Locust before sending again.");
        // A one-use agent: no pooled connection, no proxy from the environment, and a redirect is an answer, not a hop.
        let agent = ureq::AgentBuilder::new().redirects(0).timeout(self.timeout).max_idle_connections(0).build();
        let payload = json!({"name": name, "arguments": args}).to_string();
        let response = agent
            .post(&url)
            .set("authorization", &format!("Bearer {token}"))
            .set("content-type", "application/json")
            .send_string(&payload);
        let response = match response {
            Ok(response) | Err(ureq::Error::Status(_, response)) => response,
            Err(ureq::Error::Transport(transport)) => {
                let timed_out = transport.to_string().contains("timed out")
                    || std::error::Error::source(&transport)
                        .and_then(|e| e.downcast_ref::<std::io::Error>())
                        .is_some_and(|e| matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock));
                return Err(if timed_out {
                    late()
                } else {
                    local(format!("The connection to Locust ended without an answer. Nothing was retried. {SWITCH} Check the conversation before sending again."))
                });
            }
        };
        if response.status() != 200 {
            return Err(if response.status() == 401 {
                local(format!("Locust refused this connection. {SWITCH}"))
            } else {
                local("This Locust could not complete the request. Update Locust and check the conversation before sending again.")
            });
        }
        let mut bytes = Vec::new();
        if let Err(error) = response.into_reader().take(MAX_REPLY + 1).read_to_end(&mut bytes) {
            return Err(if matches!(error.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) {
                late()
            } else {
                local("Locust’s answer was interrupted. Nothing was retried; read the conversation in Locust.")
            });
        }
        if bytes.len() as u64 > MAX_REPLY {
            return Err(local("Locust’s answer was too large. Read the conversation in Locust."));
        }
        let body: Value = serde_json::from_slice(&bytes).map_err(|_| unreadable())?;
        let content = body.get("content").and_then(Value::as_array).ok_or_else(unreadable)?;
        let text = content
            .iter()
            .filter(|c| c["type"] == "text")
            .map(|c| c["text"].as_str().map(str::to_owned).ok_or_else(unreadable))
            .collect::<Result<Vec<_>, _>>()?
            .join("\n");
        if body["isError"] == true {
            return Err(LocalError(text));
        }
        serde_json::from_str(&text).map_err(|_| unreadable())
    }

    pub fn teammates(&self) -> Result<Vec<CliTeammate>, LocalError> {
        let list = self.call("list_teammates", json!({}))?;
        list.as_array().ok_or_else(unreadable)?.iter().map(|t| CliTeammate::of(t).ok_or_else(unreadable)).collect()
    }

    pub fn status(&self) -> Result<CliStatus, LocalError> {
        let state = self.call("cli/status", json!({}))?;
        let turns = state["turns"].as_array().ok_or_else(unreadable)?;
        let turns = turns
            .iter()
            .map(|t| {
                let text = |key: &str| t[key].as_str().map(str::to_owned).ok_or_else(unreadable);
                Ok(CliStatusTurn { mission_id: text("missionId")?, teammate: text("teammate")?, state: text("state")? })
            })
            .collect::<Result<Vec<_>, LocalError>>()?;
        Ok(CliStatus { workspace: state["workspace"].as_str().unwrap_or_default().to_owned(), turns })
    }

    pub fn open(&self, teammate: Option<&str>, cwd: Option<&str>) -> Result<CliSession, LocalError> {
        let mut args = json!({});
        if let Some(teammate) = teammate {
            args["teammate"] = json!(teammate);
        }
        if let Some(cwd) = cwd {
            args["cwd"] = json!(cwd);
        }
        let opened = self.call("cli/open", args)?;
        (|| {
            Some(CliSession {
                session_id: opened["sessionId"].as_str()?.to_owned(),
                key: opened["key"].as_str()?.to_owned(),
                teammate: CliTeammate::of(&opened["teammate"])?,
                workspace: opened["workspace"].as_str()?.to_owned(),
            })
        })()
        .ok_or_else(unreadable)
    }

    fn auth(session: &CliSession) -> Value {
        json!({"sessionId": session.session_id, "key": session.key})
    }

    pub(crate) fn keepalive(&self, session: &CliSession) -> Result<Value, LocalError> {
        self.call("cli/keepalive", Self::auth(session))
    }

    pub fn close(&self, session: &CliSession) -> Result<Value, LocalError> {
        self.call("cli/close", Self::auth(session))
    }

    pub fn cancel(&self, session: &CliSession, request_id: Option<&str>) -> Result<Value, LocalError> {
        let mut args = Self::auth(session);
        if let Some(request_id) = request_id {
            args["requestId"] = json!(request_id);
        }
        self.call("cli/cancel", args)
    }

    /// Forward the editor's answer to its authenticated door session. Only the door may decide:
    /// it rechecks the saved opt-in and that this card is still pending on this session's current turn.
    pub(crate) fn answer_editor_card(&self, session: &CliSession, approval_id: &str, approve: bool) -> Result<Value, LocalError> {
        let mut args = Self::auth(session);
        args["approvalId"] = json!(approval_id);
        args["approve"] = json!(approve);
        self.call("cli/decide", args)
    }

    /// Send one message and follow its reply until the turn ends. Interrupted, or unable to read the reply, it cancels
    /// this prompt's own request and nothing else.
    pub fn prompt(
        &self,
        session: &CliSession,
        message: &str,
        signal: Option<&Abort>,
        on_text: &mut dyn FnMut(&CliText) -> Result<(), LocalError>,
        on_approvals: Option<&mut dyn FnMut(&[CliApproval], &Abort)>,
        poll: Duration,
    ) -> Result<Ended, LocalError> {
        self.follow(session, message, signal, on_text, on_approvals, poll, false)
    }

    pub(crate) fn prompt_owned(
        &self,
        session: &CliSession,
        message: &str,
        signal: Option<&Abort>,
        on_text: &mut dyn FnMut(&CliText) -> Result<(), LocalError>,
        on_approvals: Option<&mut dyn FnMut(&[CliApproval], &Abort)>,
        poll: Duration,
    ) -> Result<Ended, LocalError> {
        self.follow(session, message, signal, on_text, on_approvals, poll, true)
    }

    #[allow(clippy::too_many_arguments)]
    fn follow(
        &self,
        session: &CliSession,
        message: &str,
        signal: Option<&Abort>,
        on_text: &mut dyn FnMut(&CliText) -> Result<(), LocalError>,
        mut on_approvals: Option<&mut dyn FnMut(&[CliApproval], &Abort)>,
        poll: Duration,
        strict_owner: bool,
    ) -> Result<Ended, LocalError> {
        let aborted = || signal.is_some_and(Abort::aborted);
        if aborted() {
            return Ok(Ended::Cancelled);
        }
        let permissions = Abort::new();
        let request_id = request_id()?;
        let sent = Arc::new(Mutex::new(Sent::None));
        let stop: Arc<dyn Fn() + Send + Sync> = {
            let (client, session, request_id, sent, permissions) = (self.clone(), session.clone(), request_id.clone(), sent.clone(), permissions.clone());
            Arc::new(move || {
                permissions.abort();
                let mut held = sent.lock().unwrap_or_else(|e| e.into_inner());
                if matches!(*held, Sent::None) {
                    let (client, session, request_id) = (client.clone(), session.clone(), request_id.clone());
                    *held = Sent::Running(std::thread::spawn(move || client.cancel(&session, Some(&request_id))));
                }
            })
        };
        let listener = signal.and_then(|s| {
            let stop = stop.clone();
            s.on_abort(Box::new(move || stop()))
        });
        let followed = (|| -> Result<Ended, LocalError> {
            if aborted() { return Ok(Ended::Cancelled); }
            let mut start = Self::auth(session);
            start["message"] = json!(message);
            start["requestId"] = json!(request_id);
            self.call("cli/prompt", start)?;
            let mut cursor = 0u64;
            loop {
                if aborted() {
                    settle(&sent)?;
                    return Ok(Ended::Cancelled);
                }
                let mut args = Self::auth(session);
                args["after"] = json!(cursor);
                let answer = self.call("cli/poll", args)?;
                if aborted() {
                    settle(&sent)?;
                    return Ok(Ended::Cancelled);
                }
                let (turn, events, next, approvals) = poll_of(&answer).ok_or_else(unreadable)?;
                if strict_owner && !turn.as_ref()
                    .is_some_and(|t| t["requestId"].as_str() == Some(request_id.as_str()))
                {
                    return Err(unreadable());
                }
                for event in &events {
                    on_text(event)?;
                }
                cursor = next;
                if let Some(on_approvals) = on_approvals.as_mut() {
                    on_approvals(&approvals, &permissions);
                }
                match turn.as_ref().and_then(|t| t["phase"].as_str()) {
                    Some("completed") => return Ok(Ended::EndTurn),
                    Some("cancelled") => return Ok(Ended::Cancelled),
                    Some("failed") => {
                        let said = turn.as_ref().and_then(|t| t["message"].as_str());
                        return Err(local(said.unwrap_or("The teammate could not finish. Read the conversation in Locust.")));
                    }
                    _ => {}
                }
                match signal {
                    Some(signal) => signal.sleep(poll),
                    None => std::thread::sleep(poll),
                }
            }
        })();
        let ended = match followed {
            Ok(ended) => Ok(ended),
            Err(error) => {
                stop();
                let _ = settle(&sent);
                if aborted() { Ok(Ended::Cancelled) } else { Err(error) }
            }
        };
        permissions.abort();
        if let (Some(signal), Some(id)) = (signal, listener) {
            signal.off(id);
        }
        // A cancel that failed is said, never dropped in the background.
        settle(&sent)?;
        ended
    }
}

/// A random UUID's shape (`randomUUID`), version 4.
fn request_id() -> Result<String, LocalError> {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).map_err(|_| local("Locust could not finish. Check the conversation in Locust."))?;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    Ok(format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32]))
}

type Poll = (Option<Value>, Vec<CliText>, u64, Vec<CliApproval>);

fn poll_of(answer: &Value) -> Option<Poll> {
    let turn = answer.get("turn").filter(|t| t.is_object()).cloned();
    let events = answer["events"]
        .as_array()?
        .iter()
        .map(|e| {
            Some(CliText {
                sequence: e["sequence"].as_u64()?,
                item_id: e["itemId"].as_str()?.to_owned(),
                operation: e["operation"].as_str()?.to_owned(),
                text: e["text"].as_str()?.to_owned(),
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let approvals = answer["approvals"]
        .as_array()?
        .iter()
        .map(|a| {
            Some(CliApproval {
                approval_id: a["approvalId"].as_str()?.to_owned(),
                kind: a["kind"].as_str()?.to_owned(),
                summary: a["summary"].as_str()?.to_owned(),
                detail: a["detail"].as_str().unwrap_or_default().to_owned(),
                cwd: a["cwd"].as_str().map(str::to_owned),
                answer_in_locust: a["answerInLocust"] != false,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some((turn, events, answer["cursor"].as_u64()?, approvals))
}

/// ACP has append-only chunks. Do not print the final replacement a second time.
pub struct TextStream<'a> {
    items: HashMap<String, String>,
    write: Box<dyn FnMut(&str) -> Result<(), LocalError> + 'a>,
}

impl<'a> TextStream<'a> {
    pub fn new(write: impl FnMut(&str) -> Result<(), LocalError> + 'a) -> Self {
        Self { items: HashMap::new(), write: Box::new(write) }
    }

    pub fn accept(&mut self, event: &CliText) -> Result<(), LocalError> {
        let before = self.items.get(&event.item_id).cloned().unwrap_or_default();
        let after = if event.operation == "append" { format!("{before}{}", event.text) } else { event.text.clone() };
        if !after.starts_with(&before) {
            return Err(local("The agent revised text already streamed. Read the corrected answer in Locust."));
        }
        let new = after[before.len()..].to_owned();
        self.items.insert(event.item_id.clone(), after);
        if !new.is_empty() {
            (self.write)(&new)?;
        }
        Ok(())
    }
}

/// The command: its arguments in, its exit code out. What it prints goes to `out`; waits and refusals to `err`.
/// `signal` is the terminal's interrupt: a reply being followed is cancelled, and the command answers 130.
pub fn run(argv: &[String], out: &mut dyn Write, err: &mut dyn Write, signal: &Abort) -> i32 {
    run_command(argv, out, err, signal, None)
}

type AcpEntry = fn(&Path, Option<String>);

fn run_command(argv: &[String], out: &mut dyn Write, err: &mut dyn Write, signal: &Abort, acp: Option<AcpEntry>) -> i32 {
    match main(argv, out, err, signal, acp) {
        Ok(code) => code,
        Err(error) => {
            let _ = writeln!(err, "{error}");
            1
        }
    }
}

fn main(argv: &[String], out: &mut dyn Write, err: &mut dyn Write, signal: &Abort, acp: Option<AcpEntry>) -> Result<i32, LocalError> {
    let wrote = |_| local("Locust could not finish. Check the conversation in Locust.");
    let mut args: Vec<String> = argv.to_vec();
    let mut profile: Option<PathBuf> = None;
    if let Some(at) = args.iter().position(|a| a == "--profile") {
        let Some(value) = args.get(at + 1).filter(|v| !v.is_empty() && !v.starts_with("--")) else {
            writeln!(err, "{HELP}").map_err(wrote)?;
            return Ok(2);
        };
        profile = Some(std::path::absolute(Path::new(value)).unwrap_or_else(|_| PathBuf::from(value)));
        args.drain(at..at + 2);
    }
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        writeln!(out, "{HELP}").map_err(wrote)?;
        return Ok(0);
    }
    if args[0] == "acp" && args.len() != 1 && !(args.len() == 3 && args[1] == "--teammate") {
        writeln!(err, "{HELP}").map_err(wrote)?;
        return Ok(2);
    }
    let profile = match profile {
        Some(profile) => profile,
        None => default_profile()?,
    };
    if args[0] == "acp" && let Some(acp) = acp {
        acp(&profile.join(CONNECTION_FILE), args.get(2).cloned());
        return Ok(0);
    }
    let client = LocustClient::new(profile);
    if args[0] == "teammates" && args.len() == 1 {
        let team = client.teammates()?;
        if team.is_empty() {
            writeln!(out, "No teammates yet. Add a teammate in Locust.").map_err(wrote)?;
        }
        for t in team {
            let runtime = t.runtime.as_deref().unwrap_or("agent not set");
            let model = t.model.as_deref().unwrap_or("model not set");
            writeln!(out, "{}\t{}\t{runtime}\t{model}\t{}", t.name, t.id, t.runs_in).map_err(wrote)?;
        }
        return Ok(0);
    }
    if args[0] == "status" && args.len() == 1 {
        let state = client.status()?;
        if state.turns.is_empty() {
            writeln!(out, "Nothing is running or waiting in Locust.").map_err(wrote)?;
        }
        for t in state.turns {
            writeln!(out, "{}\t{}\t{}", t.state, t.teammate, t.mission_id).map_err(wrote)?;
        }
        return Ok(0);
    }
    if args[0] == "send" && args.len() == 3 {
        let session = client.open(Some(&args[1]), None)?;
        let mut seen = HashSet::new();
        let ended = {
            let mut stream = TextStream::new(|text: &str| {
                out.write_all(text.as_bytes()).and_then(|_| out.flush()).map_err(|_| local("The terminal did not keep up with the reply. Read it in Locust."))
            });
            let mut on_text = |event: &CliText| stream.accept(event);
            let mut on_approvals = |cards: &[CliApproval], _: &Abort| {
                for card in cards {
                    if seen.insert(card.approval_id.clone()) {
                        let _ = writeln!(err, "Waiting for you in Locust: {}", card.summary);
                    }
                }
            };
            client.prompt(&session, &args[2], Some(signal), &mut on_text, Some(&mut on_approvals), Duration::from_millis(250))
        };
        let ended = ended.and_then(|ended| writeln!(out).map(|_| ended).map_err(wrote));
        // The session is closed whatever happened; a close that fails is said over anything else.
        client.close(&session)?;
        return Ok(if ended? == Ended::Cancelled { 130 } else { 0 });
    }
    writeln!(err, "{HELP}").map_err(wrote)?;
    Ok(2)
}

const TERMINAL_PENDING_LIMIT: usize = 2 * 1024 * 1024;
const TERMINAL_SLOW: &str = "The terminal did not keep up with the reply. Read it in Locust.";

#[derive(Default)]
struct TerminalState {
    queued: Vec<u8>,
    in_flight: usize,
    failed: bool,
    closed: bool,
}

type TerminalQueue = Arc<(Mutex<TerminalState>, Condvar)>;

/// Do not block door polling or cancellation on a terminal that no longer reads its pipe.
struct TerminalOutput {
    queue: TerminalQueue,
    writer: Option<std::thread::JoinHandle<()>>,
}

impl TerminalOutput {
    fn new(mut out: impl Write + Send + 'static) -> std::io::Result<Self> {
        let queue: TerminalQueue = Arc::default();
        let copy = queue.clone();
        let writer = std::thread::Builder::new().name("locust-stdout".into()).spawn(move || {
            let (state, wake) = &*copy;
            loop {
                let mut held = state.lock().unwrap_or_else(|e| e.into_inner());
                held = wake.wait_while(held, |s| s.queued.is_empty() && !s.closed && !s.failed)
                    .unwrap_or_else(|e| e.into_inner());
                if held.failed || held.queued.is_empty() {
                    return;
                }
                let bytes = std::mem::take(&mut held.queued);
                held.in_flight = bytes.len();
                drop(held);
                let ok = out.write_all(&bytes).and_then(|_| out.flush()).is_ok();
                let mut held = state.lock().unwrap_or_else(|e| e.into_inner());
                held.in_flight = 0;
                if !ok {
                    held.failed = true;
                }
                wake.notify_all();
            }
        })?;
        Ok(Self { queue, writer: Some(writer) })
    }

    /// Tests and the worker can pause a physical write; queued and in-flight bytes still share one limit.
    fn finish(mut self, signal: &Abort) -> Result<(), LocalError> {
        let (state, wake) = &*self.queue;
        let queue = self.queue.clone();
        let listener = signal.on_abort(Box::new(move || {
            let _held = queue.0.lock().unwrap_or_else(|e| e.into_inner());
            queue.1.notify_all();
        }));
        let mut held = state.lock().unwrap_or_else(|e| e.into_inner());
        held.closed = true;
        wake.notify_all();
        let held = wake.wait_while(held, |s| !s.failed && !signal.aborted()
            && (!s.queued.is_empty() || s.in_flight > 0))
            .unwrap_or_else(|e| e.into_inner());
        let failed = held.failed;
        let interrupted = signal.aborted();
        drop(held);
        if let Some(id) = listener { signal.off(id); }
        if interrupted {
            return Ok(());
        }
        if failed {
            return Err(local(TERMINAL_SLOW));
        }
        if let Some(writer) = self.writer.take() {
            writer.join().map_err(|_| local(TERMINAL_SLOW))?;
        }
        Ok(())
    }

    fn slow(&self) -> std::io::Error {
        std::io::Error::other(TERMINAL_SLOW)
    }
}

impl Write for TerminalOutput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let (state, wake) = &*self.queue;
        let mut held = state.lock().unwrap_or_else(|e| e.into_inner());
        if held.failed || held.closed
            || held.queued.len() + held.in_flight + bytes.len() > TERMINAL_PENDING_LIMIT
        {
            held.failed = true;
            held.queued.clear();
            wake.notify_all();
            return Err(self.slow());
        }
        held.queued.extend_from_slice(bytes);
        wake.notify_all();
        Ok(bytes.len())
    }

    // Flushing happens on the writer thread. The final successful drain is in finish.
    fn flush(&mut self) -> std::io::Result<()> {
        if self.queue.0.lock().unwrap_or_else(|e| e.into_inner()).failed { Err(self.slow()) } else { Ok(()) }
    }
}

impl Drop for TerminalOutput {
    fn drop(&mut self) {
        let (state, wake) = &*self.queue;
        let mut held = state.lock().unwrap_or_else(|e| e.into_inner());
        held.closed = true;
        if self.writer.is_some() {
            // An overflowing, interrupted or broken pipe can be blocked in write(2). Do not join it.
            held.failed = true;
            held.queued.clear();
        }
        wake.notify_all();
    }
}

/// Standalone executable entry. No GUI or engine starts here; all terminal calls go through the existing door.
pub fn run_process(argv: &[String]) -> i32 {
    let signal = Abort::new();
    let mut err = std::io::stderr();
    // The existing ACP adapter owns its own stdio. Don't disguise it as the bounded terminal text path.
    let is_acp = argv.iter().enumerate().find(|(i, arg)| {
        arg.as_str() != "--profile" && (*i == 0 || argv[*i - 1] != "--profile")
    }).is_some_and(|(_, arg)| arg == "acp");
    if is_acp {
        return run_command(argv, &mut std::io::stdout(), &mut err, &signal, Some(crate::acp_agent::acp_main));
    }
    let stop = signal.clone();
    if ctrlc::set_handler(move || stop.abort()).is_err() {
        let _ = writeln!(err, "Locust could not watch for an interrupt. Nothing was sent.");
        return 1;
    }
    let Ok(mut out) = TerminalOutput::new(std::io::stdout()) else {
        let _ = writeln!(err, "{TERMINAL_SLOW}");
        return 1;
    };
    let code = run_command(argv, &mut out, &mut err, &signal, None);
    if code != 0 {
        return code;
    }
    if let Err(error) = out.finish(&signal) {
        let _ = writeln!(err, "{error}");
        return 1;
    }
    if signal.aborted() { 130 } else { 0 }
}

#[cfg(test)]
#[path = "cli_client_tests.rs"]
mod cli_client_tests;

#[cfg(test)]
#[path = "cli_terminal_tests.rs"]
mod cli_terminal_tests;

#[cfg(test)]
#[path = "cli_approval_default_tests.rs"]
mod cli_approval_default_tests;

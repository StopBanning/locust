//! LOCUST'S DOOR: the local server other apps on this computer use to hand work to the team (locust-mcp-host.ts,
//! app-folder.ts and resources/locust-mcp-bridge.mjs).
//!
//! Off until the person turns it on. On, it listens on 127.0.0.1 at a port the system picks and writes a connection
//! file (the port and a new 64-hex bearer token, owner-only) into the native data folder. Another app runs
//! `locust-native --door-bridge <connection file>`, a stdio MCP server that reads the file on every call and posts
//! `{name, arguments}` to `/tools/call`. The headers are checked before any body is read; a request from a browser
//! (any `Origin`), another method or another path is refused. A crashed process's token is never reused: the file is
//! removed at load and rewritten when the switch comes on.
//!
//! The native app keeps its own connection file in its own data folder, so it and Electron can both run.

use crate::door_tools::{Begun, Locust, Route, StartRequest, Tools, Turn};
use crate::engine::Teammate;
use crate::store::Store;
use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The connection file and the settings file, beside each other in the native data folder (MCP_CONNECTION_FILE and
/// MCP_SETTINGS_FILE).
pub const CONNECTION_FILE: &str = "locust-mcp-connection.json";
pub const SETTINGS_FILE: &str = "locust-mcp.json";
/// The name this door registers under in another app. Electron's is `locust`; both can be added while both run.
pub const SERVER_NAME: &str = "locust-native";

const BODY_LIMIT: usize = 256 * 1024;

/// What runs one tool call: its name and arguments in, the MCP result out.
pub type Caller = Arc<dyn Fn(&str, &Value) -> Value + Send + Sync>;
const HEADER_LIMIT: usize = 16 * 1024;

/// The person's switches, saved as Electron saves them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Settings {
    pub enabled: bool,
    /// "Use each teammate's own mode": a turn runs in the teammate's mode, or the mode and folder the app names.
    pub own_mode: bool,
    /// "Let apps see your conversations".
    pub conversations: bool,
    /// "Let editors answer cards for their own turns" (0.790): an editor's session may answer its own turn's cards.
    pub editor_answers: bool,
    /// An editor has opened a session here: Settings shows that switch from now on.
    pub editor_seen: bool,
}

impl Settings {
    /// Only a literal true widens anything; a missing or odd value is off.
    fn parse(value: &Value) -> Self {
        Self {
            enabled: value["enabled"] == true,
            own_mode: value["ownMode"] == true,
            conversations: value["conversations"] == true,
            editor_answers: value["editorAnswers"] == true,
            editor_seen: value["editorSeen"] == true,
        }
    }
    fn to_json(&self) -> Value {
        let mut out = json!({"enabled": self.enabled, "ownMode": self.own_mode});
        if self.editor_answers {
            out["editorAnswers"] = json!(true);
        }
        if self.editor_seen {
            out["editorSeen"] = json!(true);
        }
        if self.conversations {
            out["conversations"] = json!(true);
        }
        out
    }
}

/// What the window shows about the door: the switches, the lines that add it to another app while it is on, and a
/// message when a change did not take.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    pub settings: Settings,
    pub claude_command: Option<String>,
    pub codex_config: Option<String>,
    /// `openclaw mcp add`: OpenClaw's chat apps (WhatsApp, Telegram, iMessage, Slack) can then ask the team here.
    pub openclaw_command: Option<String>,
    pub message: Option<String>,
}

/// mcpSetup: the lines that add this door to Claude Code, Codex and OpenClaw. They carry the connection file's path,
/// never the token or a port, so they stay right across restarts. Single quotes, where a POSIX shell expands nothing.
/// OpenClaw's `mcp add` takes --command and a repeatable --arg, and connects before it saves.
pub fn setup(exe: &str, connection: &str) -> (String, String, String) {
    let quote = |v: &str| format!("'{}'", v.replace('\'', r"'\''"));
    (
        format!("claude mcp add --transport stdio --scope user {SERVER_NAME} -- {} --door-bridge {}", quote(exe), quote(connection)),
        format!(
            "[mcp_servers.{}]\ncommand = {}\nargs = [\"--door-bridge\", {}]",
            SERVER_NAME.replace('-', "_"),
            json!(exe),
            json!(connection)
        ),
        format!("openclaw mcp add {SERVER_NAME} --command {} --arg '--door-bridge' --arg {}", quote(exe), quote(connection)),
    )
}

/// Bearer tokens compared in constant time.
pub(crate) fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub(crate) fn random_hex(bytes: usize) -> std::io::Result<String> {
    let mut b = vec![0u8; bytes];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut b)?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// Writes `text` to `path` through an owner-only temporary file made fresh, then moved into place: no bearer bytes
/// exist anywhere a second account could read them.
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    let temp = path.with_extension("json.tmp");
    let _ = std::fs::remove_file(&temp);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    std::fs::rename(&temp, path)
}

/// One HTTP request, as far as the door needs it.
struct Request {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    /// Bytes of the body already read with the headers.
    rest: Vec<u8>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }
}

fn read_head(stream: &mut TcpStream) -> Option<Request> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 4096];
    let end = loop {
        if let Some(at) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
            break at;
        }
        if buffer.len() > HEADER_LIMIT {
            return None;
        }
        let n = stream.read(&mut chunk).ok()?;
        if n == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..n]);
    };
    let head = std::str::from_utf8(&buffer[..end]).ok()?;
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split(' ');
    let method = first.next()?.to_owned();
    let path = first.next()?.to_owned();
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_owned()))
        .collect();
    Some(Request { method, path, headers, rest: buffer[end + 4..].to_vec() })
}

fn respond(stream: &mut TcpStream, status: &str, body: Option<&Value>) {
    let text = body.map(|b| b.to_string()).unwrap_or_default();
    let kind = if body.is_some() { "content-type: application/json\r\n" } else { "" };
    let _ = write!(stream, "HTTP/1.1 {status}\r\n{kind}content-length: {}\r\nconnection: close\r\n\r\n{text}", text.len());
    let _ = stream.flush();
}

/// mcpRequestHandler: authenticate the headers before reading any body, refuse a browser or another path, then run the
/// tool. Revoking the token also refuses a request whose body was still arriving.
fn handle(mut stream: TcpStream, token: &Mutex<Option<String>>, call: &(dyn Fn(&str, &Value) -> Value + Send + Sync)) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let started = Instant::now();
    let Some(request) = read_head(&mut stream) else {
        respond(&mut stream, "400 Bad Request", None);
        return;
    };
    let expected = token.lock().unwrap_or_else(|e| e.into_inner()).clone();
    let supplied = request
        .header("authorization")
        .and_then(|a| a.strip_prefix("Bearer "))
        .filter(|t| t.len() == 64 && t.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
        .unwrap_or("");
    let Some(expected) = expected.filter(|e| same(supplied, e)) else {
        respond(&mut stream, "401 Unauthorized", None);
        return;
    };
    if request.header("origin").is_some() || request.method != "POST" || request.path != "/tools/call" {
        respond(&mut stream, "403 Forbidden", None);
        return;
    }
    let length = request.header("content-length").and_then(|l| l.parse::<usize>().ok());
    let Some(length) = length.filter(|l| *l <= BODY_LIMIT && request.header("transfer-encoding").is_none()) else {
        respond(&mut stream, "400 Bad Request", None);
        return;
    };
    let mut body = request.rest;
    let mut chunk = [0u8; 8192];
    while body.len() < length {
        if started.elapsed() > Duration::from_secs(30) {
            respond(&mut stream, "400 Bad Request", None);
            return;
        }
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => {
                respond(&mut stream, "400 Bad Request", None);
                return;
            }
            Ok(n) => body.extend_from_slice(&chunk[..n]),
        }
    }
    body.truncate(length);
    if token.lock().unwrap_or_else(|e| e.into_inner()).as_deref() != Some(expected.as_str()) {
        respond(&mut stream, "401 Unauthorized", None);
        return;
    }
    let input: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => {
            respond(&mut stream, "400 Bad Request", None);
            return;
        }
    };
    let Some(name) = input["name"].as_str() else {
        respond(&mut stream, "400 Bad Request", None);
        return;
    };
    let arguments = input.get("arguments").cloned().unwrap_or_else(|| json!({}));
    let answer = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| call(name, &arguments)));
    match answer {
        Ok(result) => respond(&mut stream, "200 OK", Some(&result)),
        Err(_) => respond(
            &mut stream,
            "500 Internal Server Error",
            Some(&crate::door_tools::ToolResult::refused("Locust could not complete that request. Nothing was retried.").to_json()),
        ),
    }
}

struct Listening {
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// The door: its switches, its listener and its tools.
pub struct Door {
    dir: PathBuf,
    exe: String,
    settings: Arc<Mutex<Settings>>,
    token: Arc<Mutex<Option<String>>>,
    listening: Mutex<Option<Listening>>,
    call: Caller,
    /// The organization's word on access from other apps (`policyFeatureRefusal(organizationPolicy, 'otherApps')`):
    /// while it refuses, the door stays shut and says why.
    refusal: Arc<dyn Fn() -> Option<String> + Send + Sync>,
    /// The editors' sessions on this door, closed with it.
    editors: Option<Arc<crate::cli_sessions::CliSessions>>,
}

pub(crate) struct VoiceDoor {
    door: Door,
    scope: Arc<crate::door_tools::VoiceScope>,
    config: crate::voice_call_rules::AddOn,
}

impl crate::runtime::codex_voice::CallAddOn for VoiceDoor {
    fn configuration(&self) -> &crate::voice_call_rules::AddOn { &self.config }

    fn take_handoffs(&self) -> Vec<crate::voice_transcript::Handoff> {
        self.scope.take_handoffs()
    }

    fn close_scope(&self) {
        self.scope.close();
    }
}

impl Drop for VoiceDoor {
    fn drop(&mut self) {
        self.scope.close();
        self.door.stop();
        let _ = std::fs::remove_dir(&self.door.dir);
    }
}

pub(crate) fn open_voice_door(store: Store, exe: &Path, folder: PathBuf, here: Option<&str>) -> Option<VoiceDoor> {
    let scope = Arc::new(crate::door_tools::VoiceScope::new(folder, here));
    let dir = store.check(&store.root().join("voice-call-mcp").join(format!("call-{}", crate::invariants::unique_nanos()))).ok()?;
    let settings = Arc::new(Mutex::new(Settings { own_mode: true, ..Settings::default() }));
    let tools = Tools::new(Engine { store, settings: settings.clone(), voice: true }).with_voice(scope.clone());
    let exe = exe.to_str()?;
    let door = Door::with(dir, exe, settings, Arc::new(move |name, args| tools.call(name, args).to_json()))
        .with_refusal(Arc::new(|| None));
    door.start().ok()?;
    let config = crate::voice_call_rules::AddOn {
        command: exe.into(),
        args: vec!["--voice-call-bridge".into(), door.connection_path().to_str()?.into()],
        env: Default::default(),
    };
    Some(VoiceDoor { door, scope, config })
}

/// The switches saved beside the connection file, through a temporary file moved into place.
fn save_settings(dir: &Path, settings: &Settings) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let path = dir.join(SETTINGS_FILE);
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, settings.to_json().to_string()).map_err(|e| e.to_string())?;
    std::fs::rename(&temp, &path).map_err(|e| e.to_string())
}

/// noteEditor: an editor opened a session, kept so Settings shows its switch from now on. Not saved means not kept.
fn note_editor(dir: &Path, settings: &Mutex<Settings>) {
    let mut held = settings.lock().unwrap_or_else(|e| e.into_inner());
    if held.editor_seen {
        return;
    }
    held.editor_seen = true;
    if save_settings(dir, &held).is_err() {
        held.editor_seen = false;
    }
}

impl Door {
    /// The live door over this machine's native engine.
    pub fn local(exe: &str) -> Result<Self, String> {
        Ok(Self::for_store(Store::local().map_err(|e| e.to_string())?, exe))
    }

    /// The live door over the native engine whose data is in `store` (the engine process's own, E4).
    pub fn for_store(store: Store, exe: &str) -> Self {
        let settings = Arc::new(Mutex::new(Settings::default()));
        let tools = Arc::new(Tools::new(Engine { store: store.clone(), settings: settings.clone(), voice: false }));
        let dir = store.root().to_path_buf();
        let host = Editors { engine: Engine { store, settings: settings.clone(), voice: false }, dir: dir.clone() };
        let editors = Arc::new(crate::cli_sessions::CliSessions::live(Arc::new(host)));
        let sessions = editors.clone();
        // The editors' `cli/*` names are answered in front of the tools (index.ts), never listed in the bridge.
        let call: Caller = Arc::new(move |name: &str, args: &Value| {
            if name.starts_with("cli/") { sessions.call(name, args).to_json() } else { tools.call(name, args).to_json() }
        });
        let mut door = Self::with(dir, exe, settings, call);
        door.editors = Some(editors);
        door
    }

    /// The native engine's tools, answered in this process, under the person's saved door switches (their own mode,
    /// their conversations): for Quick Ask and `--ask` when no door is open to ask.
    /// `ask_only` holds every turn to Ask whatever the saved switch says: for a caller that cannot show a card.
    pub fn native_tools(ask_only: bool) -> Result<Caller, String> {
        let store = Store::local().map_err(|e| e.to_string())?;
        let mut saved = std::fs::read_to_string(store.root().join(SETTINGS_FILE))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .map(|v| Settings::parse(&v))
            .unwrap_or_default();
        if ask_only {
            saved.own_mode = false;
        }
        let settings = Arc::new(Mutex::new(saved));
        let tools = Arc::new(Tools::new(Engine { store, settings, voice: false }));
        Ok(Arc::new(move |name: &str, args: &Value| tools.call(name, args).to_json()))
    }

    /// Where the native window's door writes its connection while it is open.
    pub fn native_connection() -> Option<PathBuf> {
        Store::local().ok().map(|store| store.root().join(CONNECTION_FILE))
    }

    /// A door over any tool caller (the tests use their own).
    pub fn with(
        dir: PathBuf,
        exe: &str,
        settings: Arc<Mutex<Settings>>,
        call: Caller,
    ) -> Self {
        Self {
            dir,
            exe: exe.to_owned(),
            settings,
            token: Arc::new(Mutex::new(None)),
            listening: Mutex::new(None),
            call,
            refusal: Arc::new(|| crate::org_policy::feature_refusal(Some(&crate::org_policy::current()), "otherApps")),
            editors: None,
        }
    }

    /// The same door, with the organization's refusal given (the tests use their own).
    pub fn with_refusal(mut self, refusal: Arc<dyn Fn() -> Option<String> + Send + Sync>) -> Self {
        self.refusal = refusal;
        self
    }

    pub fn connection_path(&self) -> PathBuf {
        self.dir.join(CONNECTION_FILE)
    }

    fn settings(&self) -> Settings {
        self.settings.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn state(&self, message: Option<String>) -> State {
        let settings = self.settings();
        let on = self.listening.lock().unwrap_or_else(|e| e.into_inner()).is_some();
        let (claude, codex, openclaw) = setup(&self.exe, &self.connection_path().to_string_lossy());
        State {
            settings: Settings { enabled: on, ..settings },
            claude_command: on.then_some(claude),
            codex_config: on.then_some(codex),
            openclaw_command: on.then_some(openclaw),
            // No message of its own: the organization's refusal, while there is one (locust-mcp-host.ts `state`).
            message: message.or_else(|| (self.refusal)()),
        }
    }

    fn save(&self) -> Result<(), String> {
        save_settings(&self.dir, &self.settings())
    }

    fn stop(&self) {
        *self.token.lock().unwrap_or_else(|e| e.into_inner()) = None;
        if let Some(mut listening) = self.listening.lock().unwrap_or_else(|e| e.into_inner()).take() {
            listening.stop.store(true, Ordering::Relaxed);
            if let Some(thread) = listening.thread.take() {
                let _ = thread.join();
            }
        }
        let _ = std::fs::remove_file(self.connection_path());
    }

    fn start(&self) -> Result<(), String> {
        if let Some(refused) = (self.refusal)() {
            return Err(refused);
        }
        if self.listening.lock().unwrap_or_else(|e| e.into_inner()).is_some() {
            return Ok(());
        }
        let failed = || "The local server could not be enabled. It is still off; no conversation was started.".to_string();
        std::fs::create_dir_all(&self.dir).map_err(|_| failed())?;
        let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| failed())?;
        let port = listener.local_addr().map_err(|_| failed())?.port();
        listener.set_nonblocking(true).map_err(|_| failed())?;
        let next = random_hex(32).map_err(|_| failed())?;
        let connection = json!({"url": format!("http://127.0.0.1:{port}/tools/call"), "token": next});
        if write_private(&self.connection_path(), &connection.to_string()).is_err() {
            let _ = std::fs::remove_file(self.connection_path());
            return Err(failed());
        }
        *self.token.lock().unwrap_or_else(|e| e.into_inner()) = Some(next);
        let stop = Arc::new(AtomicBool::new(false));
        let (stopping, token, call) = (stop.clone(), self.token.clone(), self.call.clone());
        let thread = std::thread::Builder::new()
            .name("locust-door".into())
            .spawn(move || {
                while !stopping.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let _ = stream.set_nonblocking(false);
                            let (token, call) = (token.clone(), call.clone());
                            let _ = std::thread::Builder::new()
                                .name("locust-door-call".into())
                                .spawn(move || handle(stream, &token, call.as_ref()));
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(25)),
                        Err(_) => std::thread::sleep(Duration::from_millis(100)),
                    }
                }
            })
            .map_err(|_| failed())?;
        *self.listening.lock().unwrap_or_else(|e| e.into_inner()) = Some(Listening { stop, thread: Some(thread) });
        Ok(())
    }

    /// At start: the old connection file goes (a crashed process's token is never reused), the saved switches are read,
    /// and the door opens if it was on.
    pub fn load(&self) -> State {
        let _ = std::fs::remove_file(self.connection_path());
        // Turned off by the organization: nothing is read or opened; the screen says why.
        if (self.refusal)().is_some() {
            return self.state(None);
        }
        let saved = std::fs::read_to_string(self.dir.join(SETTINGS_FILE))
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .map(|v| Settings::parse(&v))
            .unwrap_or_default();
        let enabled = saved.enabled;
        *self.settings.lock().unwrap_or_else(|e| e.into_inner()) = saved;
        if enabled && let Err(why) = self.start() {
            return self.state(Some(why));
        }
        self.state(None)
    }

    pub fn set_enabled(&self, enabled: bool) -> State {
        // Refused by the organization: nothing is opened or saved, and the switch the person saved stays as it was.
        if enabled && (self.refusal)().is_some() {
            return self.state(None);
        }
        let result = if enabled {
            self.start()
        } else {
            self.stop();
            Ok(())
        };
        self.settings.lock().unwrap_or_else(|e| e.into_inner()).enabled = enabled && result.is_ok();
        match result.and_then(|_| self.save()) {
            Ok(()) => self.state(None),
            Err(_) => {
                self.stop();
                self.state(Some("The local server setting could not be saved. The server is off; no conversation was started.".into()))
            }
        }
    }

    /// A switch that is not saved is not changed: a restart must not bring back what the screen no longer shows.
    fn set_flag(&self, change: impl Fn(&mut Settings, bool), next: bool, failed: &str) -> State {
        let before = self.settings();
        change(&mut self.settings.lock().unwrap_or_else(|e| e.into_inner()), next);
        match self.save() {
            Ok(()) => self.state(None),
            Err(_) => {
                *self.settings.lock().unwrap_or_else(|e| e.into_inner()) = before;
                self.state(Some(failed.into()))
            }
        }
    }

    pub fn set_own_mode(&self, next: bool) -> State {
        self.set_flag(|s, v| s.own_mode = v, next, "The mode setting could not be saved. It is unchanged.")
    }

    pub fn set_conversations(&self, next: bool) -> State {
        self.set_flag(|s, v| s.conversations = v, next, "The setting could not be saved. It is unchanged.")
    }

    /// "Let editors answer cards for their own turns".
    pub fn set_editor_answers(&self, next: bool) -> State {
        self.set_flag(|s, v| s.editor_answers = v, next, "The setting could not be saved. It is unchanged.")
    }

    /// An editor opened a session here (`noteEditor`).
    pub fn note_editor(&self) -> State {
        note_editor(&self.dir, &self.settings);
        self.state(None)
    }

    /// Locust is closing: the editors' sessions close, stopping only their own work, then the listener.
    pub fn dispose(&self) {
        if let Some(editors) = &self.editors {
            editors.dispose();
        }
        self.stop();
    }
}

impl Drop for Door {
    fn drop(&mut self) {
        if let Some(editors) = &self.editors {
            editors.dispose();
        }
        self.stop();
    }
}

// ---- the folder an app names ---------------------------------------------------------------------------------------

/// git's common directory for a folder, absolute: the repository a worktree belongs to.
fn common_dir(folder: &Path) -> Option<PathBuf> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .current_dir(folder)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    let dir = text.trim();
    (out.status.success() && !dir.is_empty()).then(|| PathBuf::from(dir).canonicalize().unwrap_or_else(|_| PathBuf::from(dir)))
}

/// app-folder.ts checkAppFolder: a folder Locust knows, one inside it, or a worktree of the same repository as one of
/// them. Nothing else: a folder the person never pointed Locust at is not the app's to open.
pub fn check_app_folder(requested: &str, known: &[PathBuf], common: &dyn Fn(&Path) -> Option<PathBuf>) -> Result<PathBuf, String> {
    let asked = Path::new(requested);
    if !asked.is_absolute() {
        return Err("folder is a full path, from the top of the disk.".into());
    }
    let path = asked.canonicalize().map_err(|_| format!("There is no folder at {requested}."))?;
    if !path.is_dir() {
        return Err(format!("{requested} is a file, not a folder."));
    }
    let known: Vec<PathBuf> = known.iter().filter_map(|k| k.canonicalize().ok()).collect();
    if known.iter().any(|base| path.starts_with(base)) {
        return Ok(path);
    }
    if let Some(own) = common(&path)
        && known.iter().any(|base| common(base).is_some_and(|theirs| theirs == own))
    {
        return Ok(path);
    }
    Err(format!(
        "{} is not a folder Locust knows: only the folder it has open, one it worked in, a teammate's own, a folder inside one, or a worktree of one of their repositories.",
        path.display()
    ))
}

/// The folders Locust knows (the ones it worked in, the teammates' own, the open one), and the rule over them.
pub fn check_folder(store: &Store, path: &str) -> Result<PathBuf, String> {
    let mut known: Vec<PathBuf> = crate::folders::list(store).unwrap_or_default().into_iter().map(|f| PathBuf::from(f.path)).collect();
    known.extend(crate::native::roster_in(store).unwrap_or_default().into_iter().filter_map(|t| t.folder.map(PathBuf::from)));
    known.extend(crate::native::open_folder());
    check_app_folder(path, &known, &common_dir)
}

/// The door's `list_models` answer for this computer, for the window's own model picker on the native engine.
pub fn list_models_here(teammate: Option<&str>) -> Result<String, String> {
    let store = Store::local().map_err(|e| e.to_string())?;
    let tools = Tools::new(Engine { store, settings: Arc::new(Mutex::new(Settings::default())), voice: false });
    let said = tools.list_models(teammate);
    if said.is_error { Err(said.text) } else { Ok(said.text) }
}

/// The saved routines, only when the window offered its runner: the door with no window offers none.
fn routines_if_offered(store: &Store, offered: bool) -> Option<Vec<Value>> {
    if !offered {
        return None;
    }
    crate::routines::progress::all(store).ok()
}

/// The live engine over `store`, for a test that drives the door's tools against the real routine runner.
#[cfg(test)]
pub(crate) fn routines_if_offered_for_test(store: &Store, offered: bool) -> Option<Vec<Value>> {
    routines_if_offered(store, offered)
}

#[cfg(test)]
pub(crate) fn tools_over(store: Store, own_mode: bool) -> Tools<Engine> {
    Tools::new(Engine { store, settings: Arc::new(Mutex::new(Settings { own_mode, ..Settings::default() })), voice: false })
}

// ---- the live engine ------------------------------------------------------------------------------------------------

/// The door's view of this machine's native engine.
pub struct Engine {
    store: Store,
    settings: Arc<Mutex<Settings>>,
    voice: bool,
}

impl Engine {
    fn efforts(&self) -> std::collections::HashMap<String, String> {
        let file = crate::people::team(&self.store).unwrap_or_default();
        file["teammates"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| Some((r["teammateId"].as_str()?.to_owned(), r["route"]["effort"].as_str()?.to_owned())))
            .collect()
    }
}

/// The routes of turns the door started, newest last (index.ts `routeOfTurn`): a turn's effort is held nowhere else.
static ROUTE_OF_TURN: Mutex<VecDeque<(String, Route)>> = Mutex::new(VecDeque::new());

/// index.ts `rememberRouteOfTurn`: at most 500, the oldest going first.
fn remember_route(mission: &str, route: Route) {
    let mut held = ROUTE_OF_TURN.lock().unwrap_or_else(|e| e.into_inner());
    held.retain(|(id, _)| id != mission);
    held.push_back((mission.to_owned(), route));
    while held.len() > 500 {
        held.pop_front();
    }
}

/// A turn the door started is running: its route is held for the follow-up that continues it.
fn begun(mission_id: String, mode: String, route: Route) -> Begun {
    let (runtime, model) = (Some(route.runtime.clone()).filter(|r| !r.is_empty()), route.model.clone());
    remember_route(&mission_id, route);
    Begun { mission_id, mode, runtime, model }
}

/// index.ts `routeOfTurnOrRecord`, for a turn not held: its record's agent, model and account; a record keeps no effort.
fn recorded_route(turn: Turn) -> Option<Route> {
    Some(Route { runtime: turn.runtime?, model: turn.model, effort: None, account: turn.account })
}

/// `routeOfTurnOrRecord`: the route held for the turn first (only it knows the effort), else what its record says.
fn route_for(mission: &str, read: impl FnOnce() -> Option<Turn>) -> Option<Route> {
    held_route(mission).filter(|r| !r.runtime.is_empty()).or_else(|| read().and_then(recorded_route))
}

fn held_route(mission: &str) -> Option<Route> {
    ROUTE_OF_TURN.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(id, _)| id == mission).map(|(_, r)| r.clone())
}

/// What the engine is asked to send for a door's turn, and the route the turn runs on. A follow-up's account goes as
/// an explicit pick (`routeOverrideFor`): the conversation's own, never the teammate's saved one.
/// The route held is what the turn runs at: with no effort named, on the teammate's own agent, `native::send_from` runs
/// it at the teammate's saved effort (`saved_effort`), as index.ts holds the effort `routeAtStart` resolved.
fn job_of(request: StartRequest, saved_effort: Option<String>) -> (crate::native::Send, crate::native::Origin, Route) {
    let mut started_by = json!({"kind": "mcp"});
    if let Some(f) = &request.folder {
        started_by["folder"] = json!(f.to_string_lossy());
    }
    let route = Route {
        runtime: request.runtime.clone().or_else(|| request.teammate.runtime.clone()).unwrap_or_default(),
        model: request.model.clone().or_else(|| request.teammate.model.clone()),
        effort: request.effort.clone().or_else(|| {
            let own_agent = request.runtime.is_none() || request.runtime == request.teammate.runtime;
            saved_effort.filter(|_| own_agent)
        }),
        account: request.account.clone().unwrap_or_else(|| request.teammate.account.clone()),
    };
    let job = crate::native::Send {
        teammate: request.teammate,
        conversation: request.follow_up_of,
        prompt: request.prompt,
        model: request.model,
        effort: request.effort,
    };
    let origin = crate::native::Origin {
        mode: Some(request.mode),
        folder: request.folder,
        started_by: Some(started_by),
        runtime: request.runtime,
        account: request.account,
        ..Default::default()
    };
    (job, origin, route)
}

impl Locust for Engine {
    fn teammates(&self) -> Result<Vec<(Teammate, Option<String>)>, String> {
        let efforts = self.efforts();
        Ok(crate::native::roster_in(&self.store)?
            .into_iter()
            .map(|t| {
                let effort = efforts.get(&t.id).cloned();
                (t, effort)
            })
            .collect())
    }

    fn busy(&self, teammate: &str) -> bool {
        crate::engine_api::api().running().iter().any(|id| crate::people::owner_of(&self.store, id).as_deref() == Some(teammate))
    }

    fn start(&self, request: StartRequest) -> Result<Begun, String> {
        let follow_up = request.follow_up_of.clone();
        let saved = self.efforts().get(&request.teammate.id).cloned();
        let (job, mut origin, route) = job_of(request, saved);
        if self.voice && let Some(started_by) = &mut origin.started_by {
            started_by["via"] = json!("voice");
        }
        let started = crate::engine_api::api().send(job, origin)?;
        // The new turn's own id: for a follow-up, the conversation's newest turn now.
        let mission_id = match &follow_up {
            Some(_) => crate::native::real_tip(&started.conversation_id)?,
            None => started.conversation_id,
        };
        Ok(begun(mission_id, started.mode, route))
    }

    fn follow_up_route(&self, mission: &str) -> Option<Route> {
        route_for(mission, || self.read(mission))
    }

    fn read(&self, mission: &str) -> Option<Turn> {
        let path = crate::native::find_ledger(mission)?;
        let records = crate::ledger::Tail::new(&path, 0).poll();
        Turn::from_records(&records)
    }

    fn newest(&self, mission: &str) -> Option<String> {
        crate::native::real_tip(mission).ok()
    }

    fn owner(&self, mission: &str) -> Option<String> {
        crate::people::owner_of(&self.store, mission)
    }

    fn live(&self, mission: &str) -> bool {
        crate::engine_api::api().owns(mission)
    }

    fn waiting(&self, mission: &str) -> bool {
        !crate::engine_api::api().waiting_for(mission).is_empty()
    }

    fn cancel(&self, mission: &str) -> bool {
        crate::engine_api::api().stop(mission).is_ok()
    }

    fn conversations(&self) -> Vec<Turn> {
        let Some(listed) = crate::native::list_recent(60, true) else {
            return Vec::new();
        };
        listed
            .summaries
            .iter()
            .filter(|s| self.newest(&s.mission_id).as_deref() == Some(s.mission_id.as_str()))
            .filter_map(|s| self.read(&s.mission_id))
            .take(20)
            .collect()
    }

    fn check_folder(&self, path: &str) -> Result<PathBuf, String> {
        check_folder(&self.store, path)
    }

    fn models(&self) -> Vec<Value> {
        crate::models::local()
    }

    fn carry_on(
        &self,
        mission: &str,
        runtime: &str,
        model: Option<&str>,
        also: &crate::carry_on::CarryOnAlso,
    ) -> Result<(String, Option<String>), String> {
        crate::engine_api::api().carry_with(mission, runtime, model, also).map(|(id, folder, _)| (id, folder))
    }

    fn folder(&self) -> Option<String> {
        crate::native::open_folder().map(|f| f.to_string_lossy().into_owned())
    }

    fn own_mode(&self) -> bool {
        self.settings.lock().unwrap_or_else(|e| e.into_inner()).own_mode
    }

    fn see_all(&self) -> bool {
        self.settings.lock().unwrap_or_else(|e| e.into_inner()).conversations
    }

    /// The routines, only where the window offered its runner (door_routines): the door with no window has none.
    fn routines(&self) -> Option<Vec<Value>> {
        routines_if_offered(&self.store, crate::door_routines::runner().is_some())
    }

    /// The routine card's own Run (index.ts `runRoutine`): the runner checks the values against the routine's inputs,
    /// and its refusal is passed on word for word.
    fn run_routine(&self, routine_id: &str, values: Option<&Value>) -> Result<String, String> {
        let runner = crate::door_routines::runner().ok_or("This Locust does not offer routines to other apps. Nothing was started.")?;
        runner.run_with_inputs(routine_id, None, values, None)?.map(|begun| begun.mission_id)
    }

    fn routine_steps(&self) -> Vec<(String, u64, u64, String)> {
        crate::door_routines::runner()
            .map(|runner| runner.running().into_iter().map(|p| (p.routine_id, p.step, p.of, p.mission_id)).collect())
            .unwrap_or_default()
    }

    fn routine_last_step(&self, routine_id: &str) -> Option<String> {
        crate::door_routines::last_step(routine_id)
    }

    fn usage(&self) -> Result<(std::collections::HashMap<String, String>, std::collections::HashMap<String, String>), String> {
        Ok((
            crate::usage_ledger::usage_windows(&self.store),
            crate::limits::limited_runtimes(&self.store),
        ))
    }

    fn save_model(&self, teammate_id: &str, route: &Value) -> Result<(), String> {
        let text = |key: &str| route.get(key).and_then(Value::as_str);
        let policy = crate::org_policy::current();
        if let Some(why) = crate::org_policy::turn_refusal(
            Some(&policy),
            text("runtime").unwrap_or(""),
            text("model"),
            text("mode").unwrap_or(""),
            false,
            text("account").is_some(),
        ) {
            return Err(why);
        }
        crate::people::remember_route(&self.store, teammate_id, route)
            .map_err(|_| "Locust could not save that model for the teammate.".to_owned())
    }
}

// ---- the editors' sessions over the live engine ---------------------------------------------------------------------

/// What an editor's session needs of this machine's native engine (index.ts's `createLocustCliSessions` options). A
/// turn's run id is the engine's own (`run_<turn>`), and its cards are those of that turn.
struct Editors {
    engine: Engine,
    dir: PathBuf,
}

/// The turn a run id names.
fn turn_of_run(run_id: &str) -> &str {
    run_id.strip_prefix("run_").unwrap_or(run_id)
}

/// Follows a started turn's ledger, feeding its events in once each, until it ends or nobody listens.
fn follow(mission: String, update: crate::cli_sessions::CliSink) {
    let mut seen = std::collections::HashSet::new();
    let mut tail: Option<crate::ledger::Tail> = None;
    loop {
        if tail.is_none() {
            tail = crate::native::find_ledger(&mission).map(|path| crate::ledger::Tail::new(path, 0));
        }
        for record in tail.as_mut().map(|t| t.poll()).unwrap_or_default() {
            let Some(event) = record.get("event") else { continue };
            if !seen.insert(event["sequence"].as_u64().unwrap_or(u64::MAX - seen.len() as u64)) {
                continue;
            }
            if !update(crate::cli_sessions::CliUpdate::Event(event.clone())) {
                return;
            }
        }
        if !update(crate::cli_sessions::CliUpdate::Ping) {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

impl crate::cli_sessions::CliHost for Editors {
    fn teammates(&self) -> Result<Vec<crate::cli_sessions::CliTeammate>, String> {
        let own = self.engine.own_mode();
        Ok(Locust::teammates(&self.engine)?
            .into_iter()
            .map(|(t, _)| crate::cli_sessions::CliTeammate {
                runs_in: if own && !t.runs_in.is_empty() { t.runs_in.clone() } else { "ask".into() },
                id: t.id,
                name: t.name,
                runtime: t.runtime,
                model: t.model,
            })
            .collect())
    }

    fn workspace(&self) -> Option<String> {
        self.engine.folder()
    }

    /// The door's own start path (`Engine::start`), on the teammate's saved route, in the mode the session was given.
    fn start(&self, request: crate::cli_sessions::CliStart, update: crate::cli_sessions::CliSink) -> Result<crate::cli_sessions::CliBegun, String> {
        let teammate = Locust::teammates(&self.engine)?
            .into_iter()
            .map(|(t, _)| t)
            .find(|t| t.id == request.teammate_id)
            .ok_or("This teammate is no longer in Locust. Run locust teammates and choose another.")?;
        let begun = Locust::start(
            &self.engine,
            StartRequest {
                teammate,
                prompt: request.prompt,
                follow_up_of: request.follow_up_of,
                mode: crate::approval::Mode::parse(&request.mode).unwrap_or(crate::approval::Mode::Ask),
                folder: None,
                runtime: None,
                model: None,
                effort: None,
                account: None,
            },
        )?;
        let run_id = format!("run_{}", begun.mission_id);
        update(crate::cli_sessions::CliUpdate::Run { mission_id: begun.mission_id.clone(), run_id: run_id.clone() });
        let mission = begun.mission_id.clone();
        let _ = std::thread::Builder::new().name("locust-editor-turn".into()).spawn(move || follow(mission, update));
        Ok(crate::cli_sessions::CliBegun { mission_id: begun.mission_id, run_id })
    }

    /// The window's own Stop.
    fn cancel(&self, run_id: &str) {
        let _ = crate::engine_api::api().stop(turn_of_run(run_id));
    }

    fn approvals(&self) -> Vec<crate::cli_sessions::CliCard> {
        crate::engine_api::api()
            .waiting()
            .into_iter()
            .map(|r| crate::cli_sessions::CliCard {
                run_id: format!("run_{}", r.mission_id),
                approval_id: r.approval_id,
                mission_id: r.mission_id,
                kind: r.kind.as_str().to_owned(),
                summary: r.summary,
                detail: r.detail,
                cwd: r.cwd,
                answer_in: r.answer_in,
            })
            .collect()
    }

    /// The window's own decision: approve once, or deny.
    fn answer(&self, approval_id: &str, approve: bool) -> bool {
        let decision = if approve { crate::approval::Decision::ApproveOnce } else { crate::approval::Decision::Deny };
        crate::engine_api::api().decide(approval_id, decision, None).is_ok()
    }

    fn editor_answers(&self) -> bool {
        self.engine.settings.lock().unwrap_or_else(|e| e.into_inner()).editor_answers
    }

    fn opened(&self) {
        note_editor(&self.dir, &self.engine.settings);
    }

    /// `locust status`: the newest turns in the open folder's records, each with its teammate and whether it runs or
    /// waits on the person.
    fn status(&self) -> Result<Value, String> {
        let team = Locust::teammates(&self.engine)?;
        let listed = crate::native::list_recent(50, true).map(|l| l.summaries).unwrap_or_default();
        let turns: Vec<Value> = listed
            .iter()
            .take(50)
            .map(|s| {
                let id = &s.mission_id;
                let teammate = Locust::owner(&self.engine, id)
                    .and_then(|owner| team.iter().find(|(t, _)| t.id == owner).map(|(t, _)| t.name.clone()))
                    .unwrap_or_else(|| "A teammate no longer here".into());
                let state = if Locust::waiting(&self.engine, id) {
                    "waiting"
                } else if Locust::live(&self.engine, id) {
                    "running"
                } else {
                    Locust::read(&self.engine, id).map(|t| t.phase()).unwrap_or("interrupted")
                };
                json!({"missionId": id, "teammate": teammate, "state": state})
            })
            .collect();
        Ok(json!({"workspace": self.engine.folder().unwrap_or_default(), "turns": turns}))
    }
}

// ---- the stdio bridge another app runs ------------------------------------------------------------------------------

const NOT_RUNNING: &str = "Locust is not running. Open it and try again.";

fn bridge_result(text: &str, is_error: bool) -> Value {
    crate::door_tools::ToolResult { text: text.into(), is_error, structured: None }.to_json()
}

/// The tools another app is shown (locust-mcp-bridge.mjs TOOLS), each that only reads marked so.
pub fn tool_list() -> Value {
    let schema = |properties: Value, required: &[&str]| json!({"type": "object", "properties": properties, "required": required, "additionalProperties": false});
    let string = json!({"type": "string", "minLength": 1, "maxLength": 200});
    let message = json!({"type": "string", "minLength": 1, "maxLength": 8000});
    let effort = json!({"type": "string", "minLength": 1, "maxLength": 40});
    let flag = json!({"type": "boolean"});
    let mode = json!({"type": "string", "enum": ["ask", "plan", "accept-edits", "approve-each", "auto"]});
    let folder = json!({"type": "string", "minLength": 1, "maxLength": 1000});
    let turn = json!({"conversation_id": string, "message": message, "model": string, "effort": effort, "read_only": flag, "mode": mode, "folder": folder});
    let tools = [
        ("list_teammates", "List Locust teammates: id, name, role, the folder Locust has open (every turn works there; the person changes it in Locust), runtime, model, whether busy, and runs_in: the mode a turn started from here would use. Read only.", schema(json!({}), &[]), true),
        ("start_conversation", "Ask a Locust teammate by name or id, or start a conversation with no teammate on an AI agent you name as runtime, from list_models with no teammate, as the person can in Locust's window. Ask mode (read only), unless the person has turned on \"Use each teammate's own mode\" in Locust; then the mode the teammate is set to there, or the mode you name: ask, plan, accept-edits, approve-each or auto (Auto only if Locust's Settings allow it). With that switch on you may also name a folder, a full path: one Locust knows, a folder inside one, or a worktree of one of their repositories; the turn works there instead of the open folder, and the conversation stays there. Any approval is answered by the person in Locust's window, never from here. read_only true runs it in Ask whatever else is named. The reply says the mode and folder used. No automatic teammate handoffs. Shares remain visible in Locust. API/account usage and Locust monthly limits apply. Returns a conversation id; use read_reply to poll.", schema(json!({"teammate": string, "runtime": effort, "message": message, "model": string, "effort": effort, "read_only": flag, "mode": mode, "folder": folder}), &["message"]), false),
        ("send_message", "Continue a conversation started by this server: in the conversation's own mode and folder, unless you name others as start_conversation takes them. No automatic teammate handoffs. No message is queued while it is working; poll read_reply before sending.", schema(turn, &["conversation_id", "message"]), false),
        ("read_reply", "Read the newest finished answer, current activity while still working (including a wait for the person to approve an action in Locust), or Locust's own failure words. An answer that asks the person to choose also gives the question and its numbered options, for answer_question. Optionally wait_seconds, up to 20, to wait for the turn to finish first. Read only.", schema(json!({"conversation_id": string, "wait_seconds": {"type": "integer", "minimum": 0, "maximum": 20}}), &["conversation_id"]), true),
        ("answer_question", "Answer the question a finished turn asked with choices (its card in Locust), by the option's number or label from read_reply: the next turn is sent exactly what pressing that option in Locust sends, in the conversation's own mode and folder. Approvals of an action are never answered from here.", schema(json!({"conversation_id": string, "choice": {"oneOf": [{"type": "integer", "minimum": 1, "maximum": 4}, {"type": "string", "minLength": 1, "maxLength": 90}]}}), &["conversation_id", "choice"]), false),
        ("stop_turn", "Stop a turn started from here that is still working, by its conversation id. What it already changed stays, and the person can undo it in Locust. Never stops a turn the person started, unless they let apps see their conversations.", schema(json!({"conversation_id": string}), &["conversation_id"]), false),
        ("list_conversations", "List the recent conversations in the folder Locust has open, the person's own too, newest first: id, teammate, title, who started it, AI agent, model, and state, including a turn stopped because its plan ran out or because Locust closed while it worked, which carry_on can pick up. Only while the person has turned on \"Let apps see your conversations\" in Locust. Read only.", schema(json!({}), &[]), true),
        ("carry_on", "Carry a conversation whose turn stopped because its plan ran out, or its provider failed first, on to another AI agent and model from list_models, as that turn's card in Locust does: the new turn is told what was already done. A turn Locust closed under while it worked is carried on too, on its own agent or another. A finished turn moves to another agent with a message for its next turn; a message also follows the carry-on words of a stopped turn. It runs in the stopped turn's own mode, and only an Ask turn unless the person has turned on \"Use each teammate's own mode\" in Locust.", schema(json!({"conversation_id": string, "runtime": string, "model": string, "message": message}), &["conversation_id", "runtime"]), false),
        ("read_usage", "What Locust last read of each AI agent's usage: how much of each window is used and when it resets, from that agent's newest run here and dated by it, and which agents are at their limit now. Read only.", schema(json!({}), &[]), true),
        ("set_teammate_model", "Set the model, and optionally the effort, a teammate runs on from now on, from list_models: its agent, account and mode stay. Only while the person has turned on both \"Let apps see your conversations\" and \"Use each teammate's own mode\" in Locust.", schema(json!({"teammate": string, "model": string, "effort": effort}), &["teammate", "model"]), false),
        ("list_models", "List the models a teammate's AI agent offers in this Locust, with the efforts each takes, and the model and effort it runs on now. With no teammate, every AI agent this Locust can run and its models, for carry_on. Read only.", schema(json!({"teammate": string}), &[]), true),
        ("list_background_runs", "List Claude turns running in the background. Read only; never starts or stops them.", schema(json!({}), &[]), true),
        ("list_routines", "List Locust routines (saved multi-step workflows). Read only.", schema(json!({}), &[]), true),
        ("run_routine", "Run a Locust routine by id or name, with values for its inputs. Returns the routine id; poll routine_status.", schema(json!({"routine": string, "values": {"type": "object", "additionalProperties": {"type": "string", "maxLength": 4000}, "maxProperties": 12}}), &["routine"]), false),
        ("routine_status", "The step a routine started by run_routine is on, a wait for the person in Locust, or, once it ends, its last answer. Read only.", schema(json!({"routine": string}), &["routine"]), true),
    ];
    Value::Array(
        tools
            .into_iter()
            .map(|(name, description, input, reads)| {
                let mut t = json!({"name": name, "description": description, "inputSchema": input});
                if reads {
                    t["annotations"] = json!({"readOnlyHint": true});
                }
                t
            })
            .collect(),
    )
}

pub(crate) fn voice_tool_list() -> Value {
    let mut tools: Vec<_> = tool_list().as_array().into_iter().flatten()
        .filter(|tool| tool["name"].as_str().is_some_and(|name| crate::voice_call_rules::TOOLS.contains(&name)))
        .cloned().collect();
    for tool in &mut tools {
        let args = match tool["name"].as_str() {
            Some("start_conversation") => Some(("teammate", "Ask a named teammate on its own agent, model, mode and folder. Wait for the person's yes before handing over.")),
            Some("send_message") => Some(("conversation_id", "Send the person's words into a conversation this call can continue.")),
            _ => None,
        };
        if let Some((who, description)) = args {
            let previous = &tool["inputSchema"]["properties"];
            let schema = json!({"type":"object","properties":{(who):previous[who],"message":previous["message"]},
                "required":[who,"message"],"additionalProperties":false});
            tool["description"] = json!(description);
            tool["inputSchema"] = schema;
        }
    }
    for (name, field, description) in [
        ("find_files","query","Find files by relative path inside the call's folder."),
        ("read_file","path","Read part of a text file by its relative path in the call's folder."),
        ("search_files","text","Search text files inside the call's folder for words."),
    ] {
        let mut input = json!({"type":"object","properties":{(field):{"type":"string","maxLength":1000}},
            "required":if name == "find_files" { vec![] } else { vec![field] },"additionalProperties":false});
        if name == "read_file" {
            input["properties"]["from_line"] = json!({"type":"integer","minimum":1});
        }
        tools.push(json!({"name":name,"description":description,"inputSchema":input,"annotations":{"readOnlyHint":true}}));
    }
    json!(tools)
}

/// A connection file that could only point at this machine's door: a profile file can never redirect the bearer token.
fn read_connection(path: &Path) -> Option<(String, String)> {
    let value: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let address = value["url"].as_str()?;
    let token = value["token"].as_str()?;
    let url = url::Url::parse(address).ok()?;
    let fine = url.scheme() == "http"
        && url.host_str() == Some("127.0.0.1")
        && url.path() == "/tools/call"
        && url.query().is_none()
        && url.fragment().is_none()
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_some()
        && token.len() == 64
        && token.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    fine.then(|| (address.to_owned(), token.to_owned()))
}

/// One tool call through the door, in the bridge's words when it does not come back.
pub fn bridge_call(connection: &Path, name: &str, arguments: &Value) -> Value {
    let Some((url, token)) = read_connection(connection) else {
        return bridge_result(NOT_RUNNING, true);
    };
    let agent = ureq::AgentBuilder::new().redirects(0).timeout(Duration::from_secs(60)).build();
    let answer = agent
        .post(&url)
        .set("authorization", &format!("Bearer {token}"))
        .set("content-type", "application/json")
        .send_string(&json!({"name": name, "arguments": arguments}).to_string());
    match answer {
        Ok(response) => {
            let mut body = Vec::new();
            if response.into_reader().take(5 * 1024 * 1024 + 1).read_to_end(&mut body).is_err() || body.len() > 5 * 1024 * 1024 {
                return bridge_result("Locust's answer could not be read. Nothing was retried.", true);
            }
            match serde_json::from_slice::<Value>(&body) {
                Ok(v) if v["content"].is_array() => v,
                _ => bridge_result("Locust's answer could not be read. Nothing was retried.", true),
            }
        }
        Err(ureq::Error::Status(401, _)) => {
            bridge_result("Locust refused this connection. Check the switch in Settings > General and try again.", true)
        }
        Err(ureq::Error::Status(..)) => bridge_result("Locust could not complete that request. Nothing was retried.", true),
        Err(ureq::Error::Transport(t)) => {
            let refused = t.kind() == ureq::ErrorKind::ConnectionFailed;
            let timed_out = t.to_string().to_lowercase().contains("timed out");
            bridge_result(
                if refused {
                    NOT_RUNNING
                } else if timed_out {
                    "Locust did not answer in time. Nothing was retried; check its history before starting again."
                } else {
                    "The connection to Locust ended without an answer. Nothing was retried; check its history before starting again."
                },
                true,
            )
        }
    }
}

/// The stdio MCP server another app spawns: JSON-RPC lines in, lines out. It never launches Locust, writes another
/// app's config or follows a redirect.
pub fn bridge_main(connection: &Path) {
    bridge_io(connection, std::io::stdin().lock(), std::io::stdout());
}

pub fn voice_bridge_main(connection: &Path) {
    bridge_with_tools(connection, std::io::stdin().lock(), std::io::stdout(), voice_tool_list());
}

fn bridge_io<W: Write + Send + 'static>(connection: &Path, input: impl BufRead, output: W) {
    bridge_with_tools(connection, input, output, tool_list());
}

fn bridge_with_tools<W: Write + Send + 'static>(connection: &Path, input: impl BufRead, output: W, tools: Value) {
    let out = Arc::new(Mutex::new(output));
    let send = |out: &Arc<Mutex<W>>, line: Value| {
        let mut o = out.lock().unwrap_or_else(|e| e.into_inner());
        let _ = writeln!(o, "{line}");
        let _ = o.flush();
    };
    let error = |id: Value, code: i64, message: &str| json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}});
    let names: Vec<String> = tools.as_array().into_iter().flatten().filter_map(|t| t["name"].as_str().map(str::to_owned)).collect();
    let mut calls = Vec::new();
    for line in input.lines() {
        let Ok(line) = line else { break };
        if line.len() > BODY_LIMIT {
            send(&out, error(Value::Null, -32600, "MCP request is too large. Nothing was started."));
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(rpc) = serde_json::from_str::<Value>(line) else {
            send(&out, error(Value::Null, -32700, "Invalid JSON"));
            continue;
        };
        if rpc["jsonrpc"] != "2.0" {
            send(&out, error(Value::Null, -32600, "Invalid request"));
            continue;
        }
        let Some(id) = rpc.get("id").cloned() else { continue };
        let method = rpc["method"].as_str().unwrap_or("");
        match method {
            "initialize" => {
                let asked = rpc["params"]["protocolVersion"].as_str().unwrap_or("");
                let version = if ["2024-11-05", "2025-03-26", "2025-06-18"].contains(&asked) { asked } else { "2025-06-18" };
                send(&out, json!({"jsonrpc": "2.0", "id": id, "result": {"protocolVersion": version, "capabilities": {"tools": {}}, "serverInfo": {"name": SERVER_NAME, "version": "1"}}}));
            }
            "ping" => send(&out, json!({"jsonrpc": "2.0", "id": id, "result": {}})),
            "tools/list" => send(&out, json!({"jsonrpc": "2.0", "id": id, "result": {"tools": tools}})),
            "tools/call" => {
                let name = rpc["params"]["name"].as_str().unwrap_or("").to_owned();
                if !names.contains(&name) {
                    send(&out, error(id, -32602, "Unknown Locust tool"));
                    continue;
                }
                let arguments = rpc["params"].get("arguments").cloned().unwrap_or_else(|| json!({}));
                let (out, connection) = (out.clone(), connection.to_path_buf());
                calls.push(std::thread::spawn(move || {
                    let answer = bridge_call(&connection, &name, &arguments);
                    let mut o = out.lock().unwrap_or_else(|e| e.into_inner());
                    let _ = writeln!(o, "{}", json!({"jsonrpc": "2.0", "id": id, "result": answer}));
                    let _ = o.flush();
                }));
            }
            _ => send(&out, error(id, -32601, "Method not found")),
        }
    }
    for call in calls {
        let _ = call.join();
    }
}

/// `locust-native --serve-door`: the door on, with no window, until stdin closes or the process is stopped. For
/// scripted checks and the daily driver's first runs.
pub fn serve_main(exe: &str) -> Result<(), String> {
    let door = Door::local(exe)?;
    door.load();
    // Open for this run only: the person's saved switch is left as it was.
    door.start()?;
    eprintln!("Locust's door is open: {}", door.connection_path().display());
    let mut sink = String::new();
    while std::io::stdin().read_line(&mut sink).map(|n| n > 0).unwrap_or(false) {
        sink.clear();
    }
    door.dispose();
    Ok(())
}

#[cfg(test)]
#[path = "door_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "an_app_names_a_folder_tests.rs"]
mod an_app_names_a_folder_tests;

#[cfg(test)]
#[path = "a_carried_conversation_tests.rs"]
mod a_carried_conversation_tests;

#[cfg(test)]
#[path = "door_stdio_tests.rs"]
mod door_stdio_tests;

#[cfg(test)]
#[path = "an_app_turn_after_carry_on_tests.rs"]
mod an_app_turn_after_carry_on_tests;

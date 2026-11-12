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

use crate::door_tools::{Begun, Locust, StartRequest, Tools, Turn};
use crate::engine::Teammate;
use crate::store::Store;
use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
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
}

impl Settings {
    /// Only a literal true widens anything; a missing or odd value is off.
    fn parse(value: &Value) -> Self {
        Self {
            enabled: value["enabled"] == true,
            own_mode: value["ownMode"] == true,
            conversations: value["conversations"] == true,
        }
    }
    fn to_json(&self) -> Value {
        let mut out = json!({"enabled": self.enabled, "ownMode": self.own_mode});
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
    pub message: Option<String>,
}

/// mcpSetup: the lines that add this door to Claude Code and Codex. They carry the connection file's path, never the
/// token or a port, so they stay right across restarts. Single quotes, where a POSIX shell expands nothing.
pub fn setup(exe: &str, connection: &str) -> (String, String) {
    let quote = |v: &str| format!("'{}'", v.replace('\'', r"'\''"));
    (
        format!("claude mcp add --transport stdio --scope user {SERVER_NAME} -- {} --door-bridge {}", quote(exe), quote(connection)),
        format!(
            "[mcp_servers.{}]\ncommand = {}\nargs = [\"--door-bridge\", {}]",
            SERVER_NAME.replace('-', "_"),
            json!(exe),
            json!(connection)
        ),
    )
}

/// Bearer tokens compared in constant time.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn random_hex(bytes: usize) -> std::io::Result<String> {
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
}

impl Door {
    /// The live door over this machine's native engine.
    pub fn local(exe: &str) -> Result<Self, String> {
        let store = Store::local().map_err(|e| e.to_string())?;
        let settings = Arc::new(Mutex::new(Settings::default()));
        let tools = Arc::new(Tools::new(Engine { store: store.clone(), settings: settings.clone() }));
        Ok(Self::with(store.root().to_path_buf(), exe, settings, Arc::new(move |name: &str, args: &Value| tools.call(name, args).to_json())))
    }

    /// A door over any tool caller (the tests use their own).
    pub fn with(
        dir: PathBuf,
        exe: &str,
        settings: Arc<Mutex<Settings>>,
        call: Caller,
    ) -> Self {
        Self { dir, exe: exe.to_owned(), settings, token: Arc::new(Mutex::new(None)), listening: Mutex::new(None), call }
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
        let (claude, codex) = setup(&self.exe, &self.connection_path().to_string_lossy());
        State {
            settings: Settings { enabled: on, ..settings },
            claude_command: on.then_some(claude),
            codex_config: on.then_some(codex),
            message,
        }
    }

    fn save(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let path = self.dir.join(SETTINGS_FILE);
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, self.settings().to_json().to_string()).map_err(|e| e.to_string())?;
        std::fs::rename(&temp, &path).map_err(|e| e.to_string())
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

    pub fn dispose(&self) {
        self.stop();
    }
}

impl Drop for Door {
    fn drop(&mut self) {
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

// ---- the live engine ------------------------------------------------------------------------------------------------

/// The door's view of this machine's native engine.
pub struct Engine {
    store: Store,
    settings: Arc<Mutex<Settings>>,
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
        crate::native::running_ids().iter().any(|id| crate::people::owner_of(&self.store, id).as_deref() == Some(teammate))
    }

    fn start(&self, request: StartRequest) -> Result<Begun, String> {
        let mut started_by = json!({"kind": "mcp"});
        if let Some(f) = &request.folder {
            started_by["folder"] = json!(f.to_string_lossy());
        }
        let runtime = request.runtime.clone().or_else(|| request.teammate.runtime.clone());
        let model = request.model.clone().or_else(|| request.teammate.model.clone());
        let job = crate::native::Send {
            teammate: request.teammate,
            conversation: request.follow_up_of.clone(),
            prompt: request.prompt,
            model: request.model,
            effort: None,
        };
        let origin = crate::native::Origin {
            mode: Some(request.mode),
            folder: request.folder,
            started_by: Some(started_by),
            runtime: request.runtime,
        };
        let started = crate::native::send_from(job, origin)?;
        // The new turn's own id: for a follow-up, the conversation's newest turn now.
        let mission_id = match &request.follow_up_of {
            Some(_) => crate::native::real_tip(&started.conversation_id)?,
            None => started.conversation_id,
        };
        Ok(Begun { mission_id, mode: started.mode, runtime, model })
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
        crate::native::owns(mission)
    }

    fn waiting(&self, mission: &str) -> bool {
        !crate::native::waiting_approvals_for(mission).is_empty()
    }

    fn cancel(&self, mission: &str) -> bool {
        crate::native::stop(mission).is_ok()
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

    fn runtimes(&self) -> Vec<String> {
        crate::carry_on::installed_runtimes()
    }

    fn carry_on(&self, mission: &str, runtime: &str, model: Option<&str>) -> Result<(String, Option<String>), String> {
        crate::native::carry(mission, runtime, model)
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
        ("start_conversation", "Ask a Locust teammate by name or id. Ask mode (read only), unless the person has turned on \"Use each teammate's own mode\" in Locust; then the mode the teammate is set to there, or the mode you name: ask, plan, accept-edits, approve-each or auto (Auto only if Locust's Settings allow it). With that switch on you may also name a folder, a full path: one Locust knows, a folder inside one, or a worktree of one of their repositories; the turn works there instead of the open folder, and the conversation stays there. Any approval is answered by the person in Locust's window, never from here. read_only true runs it in Ask whatever else is named. The reply says the mode and folder used. Returns a conversation id; use read_reply to poll.", schema(json!({"teammate": string, "message": message, "model": string, "effort": effort, "read_only": flag, "mode": mode, "folder": folder}), &["teammate", "message"]), false),
        ("send_message", "Continue a conversation started by this server: in the conversation's own mode and folder, unless you name others as start_conversation takes them. No message is queued while it is working; poll read_reply before sending.", schema(turn, &["conversation_id", "message"]), false),
        ("read_reply", "Read the newest finished answer, current activity while still working (including a wait for the person to approve an action in Locust), or Locust's own failure words. Optionally wait_seconds, up to 20, to wait for the turn to finish first. Read only.", schema(json!({"conversation_id": string, "wait_seconds": {"type": "integer", "minimum": 0, "maximum": 20}}), &["conversation_id"]), true),
        ("stop_turn", "Stop a turn started from here that is still working, by its conversation id. What it already changed stays, and the person can undo it in Locust. Never stops a turn the person started, unless they let apps see their conversations.", schema(json!({"conversation_id": string}), &["conversation_id"]), false),
        ("list_conversations", "List the recent conversations, the person's own too, newest first: id, teammate, title, who started it, AI agent, model, and state. Only while the person has turned on \"Let apps see your conversations\" in Locust. Read only.", schema(json!({}), &[]), true),
        ("list_models", "List the models a teammate's AI agent offers in this Locust, with the efforts each takes. Read only.", schema(json!({"teammate": string}), &[]), true),
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
    let out = Arc::new(Mutex::new(std::io::stdout()));
    let send = |out: &Arc<Mutex<std::io::Stdout>>, line: Value| {
        let mut o = out.lock().unwrap_or_else(|e| e.into_inner());
        let _ = writeln!(o, "{line}");
        let _ = o.flush();
    };
    let error = |id: Value, code: i64, message: &str| json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}});
    let tools = tool_list();
    let names: Vec<String> = tools.as_array().into_iter().flatten().filter_map(|t| t["name"].as_str().map(str::to_owned)).collect();
    let stdin = std::io::stdin();
    let mut calls = Vec::new();
    for line in stdin.lock().lines() {
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

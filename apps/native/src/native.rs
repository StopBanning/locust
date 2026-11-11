//! M2: the native engine behind the window. No Electron, no MCP door, no Node.
//!
//! * `roster` reads Electron's `teammates.json` READ-ONLY (shape from `apps/desktop/src/main/teammate-store.ts`).
//! * `send` starts or continues a Codex/Claude turn: a new native ledger (`ledger_write`), the runtime,
//!   all on a thread of its own. Approvals raise a card the window answers. The window tails the ledger file.
//! * `stop` asks the turn this process started to interrupt; the runtime records it as cancelled.
//!
//! Every write goes through `store::Store`, which refuses Electron's data folder. Nothing here opens a credential.
use crate::{
    approval::{Answer, Channel, Decision, Mode, Request as ApprovalRequest},
    engine::{self, Started, Teammate},
    ledger::{self, Tail},
    ledger_write::{Continuation, LedgerWriter},
    runtime::{
        Runtime, Turn, antigravity::Antigravity, claude::Claude, codex::Codex, copilot::Copilot,
        cursor::Cursor, events::Normalizer, kilo::Kilo, muse::Muse, opencode::OpenCode,
    },
    store::Store,
};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    io::{self, Read},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

/// Which engine the window is using. Shown in the status line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Door,
    Native,
}
impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Door => "Engine: Locust door",
            Kind::Native => "Engine: native",
        }
    }
}

/// `runs_in` of a teammate native cannot run yet; the window words it "runs in Locust for now".
pub const IN_LOCUST: &str = "in-locust";

/// `LOCUST_NATIVE_ENGINE=1` picks native whether or not Electron is up.
pub fn forced() -> bool {
    std::env::var_os("LOCUST_NATIVE_ENGINE").is_some_and(|v| v == "1")
}

/// Item 1, as a pure decision: forced native; else the door while it answers; native only when the door is unreachable
/// (a door that answered and refused is not down). When native has nothing either, the door's own words are shown.
pub fn choose(
    forced: bool,
    door: impl FnOnce() -> Result<Vec<Teammate>, String>,
    native: impl FnOnce() -> Result<Vec<Teammate>, String>,
) -> Result<(Kind, Vec<Teammate>), String> {
    if forced {
        return native().map(|team| (Kind::Native, team));
    }
    match door() {
        Ok(team) => Ok((Kind::Door, team)),
        Err(words) if door_unreachable(&words) => {
            native().map(|team| (Kind::Native, team)).map_err(|_| words)
        }
        Err(words) => Err(words),
    }
}

/// The door could not be reached at all (not: it answered and refused).
pub fn door_unreachable(words: &str) -> bool {
    words == engine::NOT_ON
        || words == engine::DOWN
        || words.starts_with("Locust's data folder could not be found")
}

/// Can native run this teammate this milestone? Codex or Claude, and never an account of its own (that needs its own
/// login home, which native does not touch).
pub fn supported(t: &Teammate) -> bool {
    matches!(
        t.runtime.as_deref(),
        Some("codex")
            | Some("claude")
            | Some("acp")
            | Some("opencode")
            | Some("kilo")
            | Some("cursor")
            | Some("copilot")
            | Some("antigravity")
            | Some("muse")
    ) && t.runs_in != IN_LOCUST
}

// ---- test hooks: one place, compiled only into tests -------------------------------------------------------------
#[cfg(test)]
pub(crate) mod hooks {
    use super::*;
    pub static LOCK: Mutex<()> = Mutex::new(());
    pub static STATE: Mutex<(Option<Store>, Option<(PathBuf, Vec<String>)>)> =
        Mutex::new((None, None));
    /// Serialises tests that point the engine at a temporary store and a fake `codex`.
    pub struct Guard {
        _lock: std::sync::MutexGuard<'static, ()>,
    }
    pub fn install(store: Store, command: Option<(PathBuf, Vec<String>)>) -> Guard {
        let guard = Guard {
            _lock: LOCK.lock().unwrap_or_else(|e| e.into_inner()),
        };
        *crate::invariants::lock(&STATE) = (Some(store), command);
        guard
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            *crate::invariants::lock(&STATE) = (None, None);
        }
    }
}

/// Native's own data folder, or None when it cannot be determined.
pub fn store() -> Option<Store> {
    #[cfg(test)]
    if let Some(store) = crate::invariants::lock(&hooks::STATE).0.clone() {
        return Some(store);
    }
    Store::local().ok()
}

fn electron_dir() -> Option<PathBuf> {
    store()
        .map(|s| s.electron().to_path_buf())
        .or_else(engine::default_data_folder)
}

/// Electron's ledger folder: only ever read.
pub fn electron_ledger_dir() -> Option<PathBuf> {
    electron_dir().map(|d| d.join("mission-ledger"))
}

fn command_for_turn(
    runtime: &str,
    folder: &Path,
    model: Option<&str>,
    mode: Mode,
    resume: Option<&str>,
    prompt: &str,
    effort: Option<&str>,
    session_id: Option<&str>,
) -> io::Result<Command> {
    #[cfg(test)]
    if let Some((program, args)) = crate::invariants::lock(&hooks::STATE).1.clone() {
        let mut c = Command::new(program);
        c.args(args);
        return Ok(c);
    }
    match runtime {
        "claude" => crate::runtime::claude::command(folder, model),
        "cursor" => crate::runtime::cursor::command(folder, mode, model, resume),
        "copilot" => crate::runtime::copilot::command(
            folder, prompt, mode, model, effort, resume, session_id,
        ),
        "antigravity" => crate::runtime::antigravity::command(folder, mode, model, effort, resume),
        "muse" => crate::runtime::muse::command(folder, mode, model, effort, resume),
        "acp" => crate::runtime::acp::command(
            &electron_dir().ok_or_else(|| io::Error::other("Locust profile not found"))?,
            folder,
            model,
        ),
        "opencode" => {
            let store = store()
                .ok_or_else(|| io::Error::other("Native data folder could not be determined"))?;
            crate::runtime::opencode::command(folder, mode, &store)
        }
        "kilo" => {
            let store = store()
                .ok_or_else(|| io::Error::other("Native data folder could not be determined"))?;
            crate::runtime::kilo::command(folder, &store)
        }
        _ => crate::runtime::codex::command(),
    }
}

// ---- the roster: teammates.json, read-only -----------------------------------------------------------------------
#[cfg(test)]
const MAX_FILE_BYTES: u64 = 1_000_000;
const MAX_TEAMMATES: usize = 64;
const HUES: [&str; 11] = [
    "lime", "blue", "violet", "clay", "teal", "butter", "rose", "slate", "pearl", "indigo", "coral",
];
const ROLES: [&str; 7] = [
    "Code & Migrations",
    "Research & Briefs",
    "Ops & Scheduling",
    "Docs & QA",
    "Data & Reporting",
    "Chief of Staff",
    "Custom",
];
// shared/runtimes.ts DISPLAY_NAMES (isMissionRuntime)
const RUNTIMES: [&str; 10] = [
    "codex",
    "claude",
    "cursor",
    "gemini",
    "opencode",
    "copilot",
    "antigravity",
    "muse",
    "kilo",
    "acp",
];
const MODES: [&str; 5] = ["ask", "accept-edits", "approve-each", "plan", "auto"];

fn control(c: char) -> bool {
    matches!(c, '\u{0}'..='\u{1f}' | '\u{7f}')
}
/// teammate-store.ts: safeId
fn safe_id(v: &Value) -> Option<&str> {
    let s = v.as_str()?;
    (!s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-'))
    .then_some(s)
}
/// teammate-store.ts: validName (length counted in UTF-16 units, as JavaScript counts it)
fn valid_name(v: &Value) -> Option<String> {
    let t = v.as_str()?.trim();
    let n = t.encode_utf16().count();
    (n > 0 && n <= 40 && !t.chars().any(control)).then(|| t.to_owned())
}
/// teammate-store.ts: validRoleTitle
fn valid_role_title(v: &Value) -> Option<String> {
    let t = v.as_str()?.trim();
    let n = t.encode_utf16().count();
    (n > 0 && n <= 60 && !t.chars().any(control)).then(|| t.to_owned())
}
/// teammate-store.ts: isTeammateFolder (absolute on either platform, bounded)
fn teammate_folder(v: &Value) -> Option<String> {
    let s = v.as_str()?;
    let windows = {
        let b = s.as_bytes();
        (b.len() >= 3
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && (b[2] == b'\\' || b[2] == b'/'))
            || s.starts_with("\\\\")
    };
    (!s.trim().is_empty() && s.len() <= 4096 && (s.starts_with('/') || windows))
        .then(|| s.to_owned())
}

struct Route {
    runtime: String,
    model: String,
    mode: String,
    account: bool,
}
/// teammate-store.ts: isTeammateRoute. A malformed route reads as no route.
fn route_of(v: &Value) -> Option<Route> {
    let runtime = v["runtime"].as_str().filter(|r| RUNTIMES.contains(r))?;
    let model = v["model"]
        .as_str()
        .filter(|m| !m.is_empty() && m.encode_utf16().count() <= 200)?;
    let mode = v["mode"].as_str().filter(|m| MODES.contains(m))?;
    let account = match &v["account"] {
        Value::Null => false,
        Value::String(a) => {
            let ok = a.strip_prefix("account_").is_some_and(|h| {
                h.len() == 32 && h.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
            });
            if !ok {
                return None;
            }
            true
        }
        _ => return None,
    };
    if !v["effort"].is_null()
        && !v["effort"]
            .as_str()
            .is_some_and(|e| !e.is_empty() && e.len() <= 40)
    {
        return None;
    }
    Some(Route {
        runtime: runtime.into(),
        model: model.into(),
        mode: mode.into(),
        account,
    })
}

/// teammate-store.ts: parsedFile / parsedTeammate, for the fields the window shows. The file is untrusted: a bad record
/// is dropped (the rest of the roster stays usable); a file that does not parse, or has another schema, is unreadable.
pub fn parse_roster(text: &str) -> Result<Vec<Teammate>, String> {
    const UNREADABLE: &str = "Locust's teammates.json could not be read.";
    let file: Value = serde_json::from_str(text).map_err(|_| UNREADABLE.to_string())?;
    if !file.is_object() || file["schemaVersion"] != 1 {
        return Err(UNREADABLE.into());
    }
    let mut out: Vec<Teammate> = Vec::new();
    for entry in file["teammates"]
        .as_array()
        .into_iter()
        .flatten()
        .take(MAX_TEAMMATES)
    {
        let (Some(id), Some(name)) = (safe_id(&entry["teammateId"]), valid_name(&entry["name"]))
        else {
            continue;
        };
        let role = entry["role"].as_str().filter(|r| ROLES.contains(r));
        let hue = entry["hue"].as_str().filter(|h| HUES.contains(h));
        let created = entry["createdAt"]
            .as_str()
            .is_some_and(|c| chrono::DateTime::parse_from_rfc3339(c).is_ok());
        let (Some(role), Some(_), true) = (role, hue, created) else {
            continue;
        };
        if out.iter().any(|t| t.id == id) {
            continue;
        }
        let title = if role == "Custom" {
            valid_role_title(&entry["roleTitle"])
        } else {
            None
        };
        let route = route_of(&entry["route"]);
        let (runtime, model, runs_in) = match &route {
            Some(r)
                if matches!(
                    r.runtime.as_str(),
                    "codex"
                        | "claude"
                        | "acp"
                        | "opencode"
                        | "kilo"
                        | "cursor"
                        | "copilot"
                        | "antigravity"
                        | "muse"
                ) && !r.account =>
            {
                (
                    Some(r.runtime.clone()),
                    Some(r.model.clone()),
                    r.mode.clone(),
                )
            }
            Some(r) => (
                Some(r.runtime.clone()),
                Some(r.model.clone()),
                IN_LOCUST.to_string(),
            ),
            None => (None, None, IN_LOCUST.to_string()),
        };
        out.push(Teammate {
            id: id.into(),
            name,
            role: title.unwrap_or_else(|| role.to_string()),
            runtime,
            model: model.filter(|m| m != "account-default"),
            busy: false,
            runs_in,
            folder: teammate_folder(&entry["folder"]),
        });
    }
    Ok(out)
}

/// Electron's roster. The file is opened for reading only; a missing file is an empty roster, as in Electron.
#[cfg(test)]
pub fn roster(electron: &Path) -> Result<Vec<Teammate>, String> {
    let path = electron.join("teammates.json");
    let file = match std::fs::File::open(&path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err("Locust's teammates.json could not be read.".into()),
    };
    let mut text = String::new();
    file.take(MAX_FILE_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|_| "Locust's teammates.json could not be read.".to_string())?;
    if text.len() as u64 > MAX_FILE_BYTES {
        return Err("Locust's teammates.json could not be read.".into());
    }
    parse_roster(&text)
}

pub fn load_roster() -> Result<Vec<Teammate>, String> {
    let store = Store::local().map_err(|e| e.to_string())?;
    let file = crate::people::team(&store)?;
    parse_roster(&file.to_string())
}

/// The folder Locust last had open (`workspace.json`, read-only), else Locust's default `~/Documents/Locust` if it exists.
fn workspace_folder(electron: &Path) -> Option<PathBuf> {
    let file = std::fs::File::open(electron.join("workspace.json")).ok()?;
    let mut text = String::new();
    file.take(64 * 1024).read_to_string(&mut text).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let path = PathBuf::from(value["path"].as_str()?);
    path.is_absolute().then_some(path).filter(|p| p.is_dir())
}

/// Conversation cwd: teammate folder, else Electron's last workspace, else ~/Documents/Locust.
/// Worktree paths (Electron `terminalFolder` when `worktree: true`) are not resolved here yet.
pub fn folder_for(teammate: &Teammate, electron: &Path) -> Result<PathBuf, String> {
    let candidates = [
        teammate.folder.as_ref().map(PathBuf::from),
        workspace_folder(electron),
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Documents").join("Locust")),
    ];
    candidates
        .into_iter()
        .flatten()
        .find(|p| p.is_absolute() && p.is_dir())
        .and_then(|p| p.canonicalize().ok())
        .ok_or_else(|| {
            "No folder to work in. Open Locust once and choose a project folder.".to_string()
        })
}

// ---- recents: both folders, native ones marked -------------------------------------------------------------------
pub struct Listed {
    pub summaries: Vec<ledger::Summary>,
    pub native: HashSet<String>,
}
/// Electron's read-only list merged with the native one, newest first. None when neither folder could be listed.
pub fn list_recent(limit: usize, native_engine: bool) -> Option<Listed> {
    let native_dir = store().and_then(|s| s.ledger_dir().ok());
    let theirs = electron_ledger_dir().and_then(|d| ledger::try_list_recent(&d, limit).ok());
    let mut ours = native_dir
        .as_ref()
        .and_then(|d| ledger::try_list_recent(d, limit).ok());
    // A native folder not made yet is simply empty; only the native engine reads that as "nothing to show".
    if ours.is_none() && native_engine && native_dir.is_some_and(|d| !d.exists()) {
        ours = Some(Vec::new());
    }
    if theirs.is_none() && ours.is_none() {
        return None;
    }
    let native: HashSet<String> = ours
        .iter()
        .flatten()
        .map(|m| m.mission_id.clone())
        .collect();
    let mut all: Vec<ledger::Summary> = theirs
        .into_iter()
        .flatten()
        .chain(ours.into_iter().flatten())
        .collect();
    all.sort_by(|a, b| {
        b.updated
            .cmp(&a.updated)
            .then_with(|| a.mission_id.cmp(&b.mission_id))
    });
    all.dedup_by(|a, b| a.mission_id == b.mission_id);
    all.truncate(limit);
    Some(Listed {
        summaries: all,
        native,
    })
}

/// A Session over both folders (Electron's read-only, then native's), for any conversation.
pub fn session(dir: PathBuf, id: String) -> ledger::Session {
    let session = ledger::Session::new(dir, id);
    match store().and_then(|s| s.ledger_dir().ok()) {
        Some(native) => session.also_read(native),
        None => session,
    }
}

// ---- resume point: which Codex thread a follow-up reopens ---------------------------------------------------------
pub(crate) fn find_ledger(id: &str) -> Option<PathBuf> {
    if !ledger::safe_id(id) {
        return None;
    }
    let native = store().and_then(|s| s.ledger_dir().ok());
    [native, electron_ledger_dir()]
        .into_iter()
        .flatten()
        .map(|d| d.join(format!("{id}.jsonl")))
        .find(|p| p.is_file())
}

/// The conversation's newest turn right now: the chain walked from `conversation` over Electron's folder (read-only)
/// and native's. A conversation that cannot be found is an error, never a guess.
pub fn real_tip(conversation: &str) -> Result<String, String> {
    let dir = electron_ledger_dir()
        .or_else(|| store().and_then(|s| s.ledger_dir().ok()))
        .ok_or("The conversation's ledger could not be found.")?;
    session(dir, conversation.to_owned())
        .chain_tip()
        .map_err(|_| "The conversation's ledger could not be found.".to_string())
}

pub struct Prior {
    pub runtime: String,
    /// Its newest turn is still running (written to within the last 15 minutes, no terminal record).
    pub running: bool,
    /// The stored `runtimeThreadId`, when the session is still resumable.
    pub thread: Option<String>,
}
/// Ports codex-mission.ts `runtimeThreadIdOf` and `resumableThreadOf`: the last runtimeThreadId in the mission's events;
/// nothing after a failure that ended the session or a moved folder; else the one its own `continuesFrom` carried.
pub fn prior_of(tip: &str) -> Result<Prior, String> {
    let path = find_ledger(tip).ok_or("The conversation's ledger could not be found.")?;
    let records = Tail::new(&path, 0).poll();
    let meta = records.clone();
    let meta = meta
        .iter()
        .find(|r| r["recordType"] == "mission.created")
        .map(|r| &r["metadata"])
        .ok_or("The conversation's ledger could not be read.")?;
    let runtime = meta["runtime"].as_str().unwrap_or_default().to_owned();
    let mut held: Option<String> = None;
    let mut ended = false;
    let mut moved = false;
    for event in records.iter().filter_map(|r| r.get("event")) {
        let payload = &event["payload"];
        if event["type"] == "run.failed" && payload["sessionEnded"] == true {
            held = None;
            ended = true;
            continue;
        }
        if event["type"] == "adapter.diagnostic" && payload["code"] == "host.folder_moved" {
            moved = true;
        }
        if let Some(t) = payload["runtimeThreadId"]
            .as_str()
            .filter(|t| !t.is_empty())
        {
            held = Some(t.to_owned());
        }
    }
    let thread = if moved || ended {
        None
    } else {
        held.or_else(|| {
            meta["continuesFrom"]["runtimeThreadId"]
                .as_str()
                .filter(|t| !t.is_empty() && t.len() <= 2048)
                .map(str::to_owned)
        })
    };
    let mut transcript = ledger::Transcript::default();
    transcript.apply(records);
    let fresh = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|m| std::time::SystemTime::now().duration_since(m).ok())
        .is_none_or(|age| age < ledger::ACTIVE_WINDOW);
    Ok(Prior {
        runtime,
        running: transcript.running && fresh,
        thread,
    })
}

// ---- running turns, stop ------------------------------------------------------------------------------------------
static RUNNING: Mutex<Option<HashMap<String, Arc<AtomicBool>>>> = Mutex::new(None);

fn with_running<T>(f: impl FnOnce(&mut HashMap<String, Arc<AtomicBool>>) -> T) -> T {
    let mut guard = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(HashMap::new))
}

struct Registered(Vec<String>);
impl Drop for Registered {
    fn drop(&mut self) {
        with_running(|m| self.0.iter().for_each(|k| drop(m.remove(k))));
    }
}

/// Did this process start a turn in this conversation that is still running? (Stop shows only for those.)
pub fn owns(conversation: &str) -> bool {
    with_running(|m| m.contains_key(conversation))
}

pub fn stop(conversation: &str) -> Result<String, String> {
    with_running(|m| m.get(conversation).cloned())
        .map(|flag| {
            flag.store(true, Ordering::Relaxed);
            "Stop requested".to_string()
        })
        .ok_or_else(|| "This window did not start that turn, so it will not stop it.".to_string())
}

static SHUTDOWN: Mutex<Option<std::thread::JoinHandle<()>>> = Mutex::new(None);

/// On window close: ask every turn to stop, at once and without waiting. A background thread watches for each turn to
/// write its terminal record (the runtime's own watchdog kills an unresponsive server after five seconds). The UI
/// thread never waits here; `wait_for_shutdown` runs after the window is gone.
pub fn shutdown_in_background() {
    shutdown_in_background_for(Duration::from_secs(8));
}

pub fn shutdown_in_background_for(limit: Duration) {
    with_running(|m| m.values().for_each(|f| f.store(true, Ordering::Relaxed)));
    let handle = std::thread::Builder::new()
        .name("native-shutdown".into())
        .spawn(move || {
            let deadline = Instant::now() + limit;
            while with_running(|m| !m.is_empty()) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(25));
            }
        });
    if let Ok(handle) = handle {
        *SHUTDOWN.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle);
    }
}

/// After the window has closed (nothing is drawing any more): let the cancelled records land, bounded by the same
/// limit the background thread uses. Returns at once when no turn was running.
pub fn wait_for_shutdown() {
    let handle = SHUTDOWN.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(handle) = handle {
        let _ = handle.join();
    }
}

/// The registry key of the turn the window may stop right now: the newest turn of what it shows, or the
/// conversation itself, but only when THIS process started it. Ownership is the registry, not a flag the window keeps
/// (which is cleared whenever the view changes).
pub fn owned_key(tip: Option<&str>, conversation: Option<&str>) -> Option<String> {
    [tip, conversation]
        .into_iter()
        .flatten()
        .find(|k| !k.is_empty() && owns(k))
        .map(str::to_owned)
}

// ---- approvals: one channel for the process; the window decides, the turn loop appends --------
static APPROVALS: Mutex<Option<std::sync::Arc<Channel>>> = Mutex::new(None);

pub(crate) fn approvals_channel() -> std::sync::Arc<Channel> {
    let mut guard = crate::invariants::lock(&APPROVALS);
    guard.get_or_insert_with(Channel::new).clone()
}

/// Cards waiting on the person right now.
pub fn waiting_approvals() -> Vec<ApprovalRequest> {
    approvals_channel().waiting()
}

/// Cards waiting for one mission (or conversation tip).
#[cfg(test)]
#[cfg(test)]
pub fn waiting_approvals_for(mission_id: &str) -> Vec<ApprovalRequest> {
    approvals_channel().waiting_for_mission(mission_id)
}

/// Answer a card from the window. Refused after the turn ended.
pub fn decide_approval(
    approval_id: &str,
    decision: Decision,
    reason: Option<String>,
) -> Result<(), String> {
    approvals_channel()
        .decide(Answer::Decision {
            approval_id: approval_id.into(),
            decision,
            reason,
        })
        .map(|_| ())
}

// ---- send --------------------------------------------------------------------------------------------------------
pub struct Send {
    pub teammate: Teammate,
    /// The conversation being continued (its anchor id). The newest turn is never taken from the window: it is read
    /// from disk, in both folders, when sending.
    pub conversation: Option<String>,
    pub prompt: String,
    pub model: Option<String>,
    pub effort: Option<String>,
}

/// A random version-4 UUID from the OS's random source: what Copilot accepts for `--session-id`.
pub(crate) fn uuid_v4() -> io::Result<String> {
    use std::io::Read;
    let mut b = [0u8; 16];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut b)?;
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    ))
}

fn new_id() -> String {
    format!(
        "mission_native_{}_{}",
        std::process::id(),
        crate::invariants::unique_nanos()
    )
}

/// Start (or continue) a Codex/Claude turn natively. Returns once the ledger exists and the turn is running on its own thread;
/// the window follows the ledger file. Approvals raise cards answered through `decide_approval`.
/// Never retried: a failure after the ledger exists is recorded in it, not tried again.
pub fn send(mut job: Send) -> Result<Started, String> {
    let store = store().ok_or("Native data folder could not be determined.")?;
    let saved = crate::people::team(&store)?;
    if let Some(entry) = saved["teammates"]
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["teammateId"] == job.teammate.id))
        && job.effort.is_none()
    {
        job.effort = entry["route"]["effort"].as_str().map(str::to_owned);
    }

    if !supported(&job.teammate) {
        return Err(format!("{} runs in Locust for now.", job.teammate.name));
    }
    // One turn at a time per conversation: refused before anything is read or created.
    if job.conversation.as_deref().is_some_and(owns) {
        return Err("A turn is already running in this conversation.".into());
    }
    let runtime_name = job.teammate.runtime.as_deref().unwrap_or("codex");
    let mode = Mode::parse(&job.teammate.runs_in).unwrap_or(Mode::Ask);
    let electron = store.electron().to_path_buf();
    let (continues, resume) = match job.conversation.as_deref() {
        None => (None, None),
        Some(conversation) => {
            let tip = real_tip(conversation)?;
            let tip = tip.as_str();
            if owns(tip) {
                return Err(
                    "A turn is still running in that conversation. Wait for it or stop it first."
                        .into(),
                );
            }
            let prior = prior_of(tip)?;
            if prior.running {
                return Err(
                    "A turn is still running in that conversation. Wait for it or stop it first."
                        .into(),
                );
            }
            if prior.runtime != runtime_name {
                return Err(format!(
                    "This conversation ran on another agent. Native continues {runtime_name} conversations only for now."
                ));
            }
            if matches!(
                runtime_name,
                "codex"
                    | "acp"
                    | "opencode"
                    | "kilo"
                    | "cursor"
                    | "copilot"
                    | "antigravity"
                    | "muse"
            ) {
                let thread = prior.thread.ok_or(match runtime_name {
                    "codex" => "This conversation has no Codex session to continue.",
                    "opencode" => "This conversation has no OpenCode session to continue.",
                    "kilo" => "This conversation has no Kilo session to continue.",
                    "cursor" => "This conversation has no Cursor session to continue.",
                    "copilot" => "This conversation has no Copilot session to continue.",
                    "antigravity" => "This conversation has no Antigravity session to continue.",
                    "muse" => "This conversation has no Muse session to continue.",
                    _ => "This conversation has no ACP session to continue.",
                })?;
                (
                    Some(Continuation {
                        mission_id: tip.to_owned(),
                        thread: Some(thread.clone()),
                    }),
                    Some(thread),
                )
            } else {
                // Claude print path has no thread resume in M3b.
                (
                    Some(Continuation {
                        mission_id: tip.to_owned(),
                        thread: None,
                    }),
                    None,
                )
            }
        }
    };
    let folder = folder_for(&job.teammate, &electron)?;
    let model = job
        .model
        .clone()
        .or_else(|| job.teammate.model.clone())
        .filter(|m| !m.is_empty() && m != "account-default");
    let model = if runtime_name == "kilo" {
        Some(crate::runtime::kilo::model_for_run(model.as_deref()))
    } else {
        model
    };
    let runtime_prompt = crate::people::brief(&store, &job.teammate.id, &folder, &job.prompt)?;
    let id = new_id();
    // Copilot needs a host session id on first turn so a failure before `result` can still resume.
    let session_id = if runtime_name == "copilot" && resume.is_none() {
        // Copilot takes only a UUID for a new session ("The value is not a valid UUID"), as the host mints one.
        Some(uuid_v4().map_err(|e| format!("Could not make a Copilot session id: {e}"))?)
    } else {
        None
    };
    let mut command = command_for_turn(
        runtime_name,
        &folder,
        model.as_deref(),
        mode,
        resume.as_deref(),
        &runtime_prompt,
        job.effort.as_deref(),
        session_id.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    // Test hooks supply their own wire fixtures; real Approve-each exposes the editing tools.
    if runtime_name == "claude"
        && mode == Mode::ApproveEach
        && command.get_program().to_string_lossy().ends_with("claude")
    {
        command = crate::runtime::claude::approve_each_command(&folder, model.as_deref())
            .map_err(|e| e.to_string())?;
    }
    command.current_dir(&folder);
    let anchor = job.conversation.clone().unwrap_or_else(|| id.clone());
    let flag = Arc::new(AtomicBool::new(false));
    let registered = with_running(|m| {
        if m.contains_key(&anchor) {
            return None;
        }
        m.insert(anchor.clone(), flag.clone());
        m.insert(id.clone(), flag.clone());
        Some(Registered(vec![anchor.clone(), id.clone()]))
    })
    .ok_or("A turn is already running in this conversation.")?;
    let channel = approvals_channel();
    let bridge = if runtime_name == "claude" && mode == Mode::ApproveEach {
        Some(
            crate::runtime::permission_bridge::Bridge::prepare(
                &mut command,
                channel.clone(),
                &id,
                &format!("run_{id}"),
                &folder.to_string_lossy(),
                flag.clone(),
            )
            .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let args: Vec<String> = command
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    let writer = LedgerWriter::create_runtime(
        &store,
        &id,
        &job.prompt,
        model.as_deref().unwrap_or("account-default"),
        &command.get_program().to_string_lossy(),
        runtime_name,
        &args,
        continues.as_ref(),
    )
    .map_err(|e| e.to_string())?;
    channel.register_teammate(&id, &job.teammate.id);
    let turn = Turn {
        prompt: runtime_prompt,
        folder,
        model,
        effort: job.effort,
        resume,
        mode,
        approvals: Some(channel.clone()),
        mission_id: id.clone(),
        run_id: format!("run_{id}"),
    };
    let runtime_owned = runtime_name.to_owned();
    let memory_store = store.clone();
    let memory_who = job.teammate.id.clone();
    let memory_folder = turn.folder.clone();
    let memory_id = id.clone();
    std::thread::Builder::new()
        .name("native-turn".into())
        .spawn(move || {
            // Keep ownership through the final memory write so shutdown/continuation cannot race it.
            let _memory_completion = registered;
            let checkpoint = crate::checkpoints::Turn::begin(&memory_store, &memory_folder);
            run_turn(
                writer,
                command,
                turn,
                flag,
                Registered(Vec::new()),
                runtime_owned,
                channel,
                bridge,
                store,
            );
            if let Err(error) = checkpoint.finish(&memory_store, &memory_id) {
                eprintln!("Undo checkpoint was not kept: {error}");
            }
            if let Ok(path) = memory_store.write_path(&memory_id) {
                let transcript = ledger::read_transcript(path);
                if !transcript.failed
                    && !transcript.running
                    && !transcript.items.iter().any(
                        |item| matches!(item, ledger::Item::Note { text, .. } if text == "Stopped"),
                    )
                {
                    let reply = transcript
                        .items
                        .iter()
                        .filter_map(|item| match item {
                            ledger::Item::Assistant { text, .. } => Some(text.as_str()),
                            _ => None,
                        })
                        .next_back()
                        .unwrap_or("")
                        .to_owned();
                    if let Err(error) = crate::people::record_reply(
                        &memory_store,
                        &memory_who,
                        &memory_folder,
                        &reply,
                    ) {
                        eprintln!("Memory was not recorded: {error}");
                    }
                }
            }
        })
        .map_err(|e| format!("Could not start the turn: {e}"))?;
    Ok(Started {
        conversation_id: anchor,
        mode: mode.as_str().into(),
    })
}

fn terminal(kind: &str) -> bool {
    matches!(kind, "run.completed" | "run.failed" | "run.cancelled")
}

fn record_failure(writer: &mut LedgerWriter, cancelled: bool, error: &io::Error) {
    let event = Normalizer::default().finish(cancelled, &error.to_string());
    let _ = writer.append(&event.kind, json!(event.payload), event.thread.as_deref());
}

fn flush_approvals(writer: &mut LedgerWriter, channel: &Channel, mission_id: &str) {
    for recorded in channel.take_recorded_for(mission_id) {
        let _ = writer.append_approval(&recorded.to_json());
    }
}

fn run_turn(
    mut writer: LedgerWriter,
    mut command: Command,
    turn: Turn,
    flag: Arc<AtomicBool>,
    registered: Registered,
    runtime_name: String,
    channel: std::sync::Arc<Channel>,
    _bridge: Option<crate::runtime::permission_bridge::Bridge>,
    store: Store,
) {
    let mut registered = Some(registered);
    let mission_id = turn.mission_id.clone();
    let finish = |writer: &mut LedgerWriter,
                  registered: &mut Option<Registered>,
                  channel: &Channel,
                  mission_id: &str| {
        flush_approvals(writer, channel, mission_id);
        channel.release(mission_id);
        drop(registered.take());
    };
    if runtime_name == "acp" {
        let mut runtime = match crate::runtime::acp::Acp::spawn(&mut command) {
            Ok(r) => r.with_cancel(flag.clone()),
            Err(e) => {
                record_failure(&mut writer, false, &e);
                return finish(&mut writer, &mut registered, &channel, &mission_id);
            }
        };
        if let Err(e) = runtime.start(&turn) {
            record_failure(&mut writer, flag.load(Ordering::Relaxed), &e);
            return finish(&mut writer, &mut registered, &channel, &mission_id);
        }
        drive_runtime(
            &mut runtime,
            &mut writer,
            &flag,
            &mut registered,
            &channel,
            &mission_id,
        );
        return;
    }
    if runtime_name == "opencode" {
        let mut runtime = match OpenCode::spawn(&mut command) {
            Ok(r) => r.with_cancel(flag.clone()),
            Err(e) => {
                record_failure(&mut writer, false, &e);
                return finish(&mut writer, &mut registered, &channel, &mission_id);
            }
        };
        if let Err(e) = runtime.start(&turn) {
            record_failure(&mut writer, flag.load(Ordering::Relaxed), &e);
            return finish(&mut writer, &mut registered, &channel, &mission_id);
        }
        drive_runtime(
            &mut runtime,
            &mut writer,
            &flag,
            &mut registered,
            &channel,
            &mission_id,
        );
        return;
    }
    if runtime_name == "kilo" {
        let mut runtime = match Kilo::spawn(&mut command) {
            Ok(r) => r.with_cancel(flag.clone()),
            Err(e) => {
                record_failure(&mut writer, false, &e);
                return finish(&mut writer, &mut registered, &channel, &mission_id);
            }
        };
        if let Err(e) = runtime.start(&turn) {
            record_failure(&mut writer, flag.load(Ordering::Relaxed), &e);
            return finish(&mut writer, &mut registered, &channel, &mission_id);
        }
        drive_runtime(
            &mut runtime,
            &mut writer,
            &flag,
            &mut registered,
            &channel,
            &mission_id,
        );
        return;
    }
    if runtime_name == "claude" {
        let mut runtime = match Claude::spawn(&mut command) {
            Ok(r) => r.with_cancel(flag.clone()),
            Err(e) => {
                record_failure(&mut writer, false, &e);
                return finish(&mut writer, &mut registered, &channel, &mission_id);
            }
        };
        if let Err(e) = runtime.start(&turn) {
            let cancelled = flag.load(Ordering::Relaxed);
            if cancelled {
                let _ = runtime.stop();
            }
            record_failure(&mut writer, cancelled, &e);
            return finish(&mut writer, &mut registered, &channel, &mission_id);
        }
        drive_runtime(
            &mut runtime,
            &mut writer,
            &flag,
            &mut registered,
            &channel,
            &mission_id,
        );
        return;
    }
    if runtime_name == "cursor" {
        let mut runtime = match Cursor::spawn(&mut command) {
            Ok(r) => r.with_cancel(flag.clone()),
            Err(e) => {
                record_failure(&mut writer, false, &e);
                return finish(&mut writer, &mut registered, &channel, &mission_id);
            }
        };
        if let Err(e) = runtime.start(&turn) {
            let cancelled = flag.load(Ordering::Relaxed);
            if cancelled {
                let _ = runtime.stop();
            }
            record_failure(&mut writer, cancelled, &e);
            return finish(&mut writer, &mut registered, &channel, &mission_id);
        }
        drive_runtime(
            &mut runtime,
            &mut writer,
            &flag,
            &mut registered,
            &channel,
            &mission_id,
        );
        return;
    }
    if runtime_name == "copilot" {
        let mut runtime = match Copilot::spawn(&mut command) {
            Ok(r) => r.with_cancel(flag.clone()),
            Err(e) => {
                record_failure(&mut writer, false, &e);
                return finish(&mut writer, &mut registered, &channel, &mission_id);
            }
        };
        if let Err(e) = runtime.start(&turn) {
            let cancelled = flag.load(Ordering::Relaxed);
            if cancelled {
                let _ = runtime.stop();
            }
            record_failure(&mut writer, cancelled, &e);
            return finish(&mut writer, &mut registered, &channel, &mission_id);
        }
        drive_runtime(
            &mut runtime,
            &mut writer,
            &flag,
            &mut registered,
            &channel,
            &mission_id,
        );
        return;
    }
    if runtime_name == "antigravity" {
        let mut runtime = match Antigravity::spawn(&mut command) {
            Ok(r) => r.with_cancel(flag.clone()),
            Err(e) => {
                record_failure(&mut writer, false, &e);
                return finish(&mut writer, &mut registered, &channel, &mission_id);
            }
        };
        if let Err(e) = runtime.start(&turn) {
            let cancelled = flag.load(Ordering::Relaxed);
            if cancelled {
                let _ = runtime.stop();
            }
            record_failure(&mut writer, cancelled, &e);
            return finish(&mut writer, &mut registered, &channel, &mission_id);
        }
        drive_runtime(
            &mut runtime,
            &mut writer,
            &flag,
            &mut registered,
            &channel,
            &mission_id,
        );
        return;
    }
    if runtime_name == "muse" {
        #[cfg(test)]
        let using_hook = crate::invariants::lock(&hooks::STATE).1.is_some();
        #[cfg(not(test))]
        let using_hook = false;
        let prompt_file = if using_hook {
            None
        } else {
            match crate::runtime::muse::prepare_prompt_file(&mut command, &turn.prompt, &store) {
                Ok(p) => Some(p),
                Err(e) => {
                    record_failure(&mut writer, false, &e);
                    return finish(&mut writer, &mut registered, &channel, &mission_id);
                }
            }
        };
        let mut runtime = match Muse::spawn_with_prompt_file(&mut command, prompt_file) {
            Ok(r) => r.with_cancel(flag.clone()),
            Err(e) => {
                record_failure(&mut writer, false, &e);
                return finish(&mut writer, &mut registered, &channel, &mission_id);
            }
        };
        if let Err(e) = runtime.start(&turn) {
            let cancelled = flag.load(Ordering::Relaxed);
            if cancelled {
                let _ = runtime.stop();
            }
            record_failure(&mut writer, cancelled, &e);
            return finish(&mut writer, &mut registered, &channel, &mission_id);
        }
        drive_runtime(
            &mut runtime,
            &mut writer,
            &flag,
            &mut registered,
            &channel,
            &mission_id,
        );
        return;
    }
    let mut runtime = match Codex::spawn(&mut command) {
        Ok(r) => r.with_cancel(flag.clone()),
        Err(e) => {
            record_failure(&mut writer, false, &e);
            return finish(&mut writer, &mut registered, &channel, &mission_id);
        }
    };
    if let Err(e) = runtime.start(&turn) {
        let cancelled = flag.load(Ordering::Relaxed);
        if cancelled {
            let _ = runtime.stop();
        }
        record_failure(&mut writer, cancelled, &e);
        return finish(&mut writer, &mut registered, &channel, &mission_id);
    }
    drive_runtime(
        &mut runtime,
        &mut writer,
        &flag,
        &mut registered,
        &channel,
        &mission_id,
    );
}

fn drive_runtime(
    runtime: &mut dyn Runtime,
    writer: &mut LedgerWriter,
    flag: &Arc<AtomicBool>,
    registered: &mut Option<Registered>,
    channel: &Channel,
    mission_id: &str,
) {
    let mut stopped = false;
    loop {
        flush_approvals(writer, channel, mission_id);
        if !stopped && flag.load(Ordering::Relaxed) {
            stopped = true;
            let _ = runtime.stop();
        }
        match runtime.next_event() {
            Ok(Some(event)) => {
                let done = terminal(&event.kind);
                if writer
                    .append(&event.kind, event.payload, event.thread.as_deref())
                    .is_err()
                    || done
                {
                    flush_approvals(writer, channel, mission_id);
                    channel.release(mission_id);
                    drop(registered.take());
                    return;
                }
            }
            Ok(None) => {}
            Err(e) => {
                record_failure(writer, stopped, &e);
                flush_approvals(writer, channel, mission_id);
                channel.release(mission_id);
                drop(registered.take());
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests;

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

/// Can native run this teammate? Named Codex and Claude Code accounts run natively with their own login home;
/// the person's own OpenCode API routes still say they run in Locust for now (own-models.ts).
pub fn supported(t: &Teammate) -> bool {
    if t.runtime.as_deref() == Some("opencode") && t.model.as_deref().is_some_and(|m| crate::spend::is_own_model("opencode", m)) {
        return false;
    }
    if t.account.is_some() && !matches!(t.runtime.as_deref(), Some("codex" | "claude")) {
        return false;
    }
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

// ---- test hooks: one place, compiled only into tests (and the window's tests, through the test-hooks feature) -------------------------------------------------------------
#[cfg(any(test, feature = "test-hooks"))]
pub mod hooks {
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
    #[cfg(any(test, feature = "test-hooks"))]
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
    references: &[&str],
    skills: Option<&str>,
) -> io::Result<Command> {
    #[cfg(any(test, feature = "test-hooks"))]
    if let Some((program, args)) = crate::invariants::lock(&hooks::STATE).1.clone() {
        let mut c = Command::new(program);
        c.args(args);
        return Ok(c);
    }
    match runtime {
        "claude" => crate::runtime::claude::bare_command(folder),
        "cursor" => crate::runtime::cursor::command(folder, mode, model, resume, skills),
        "copilot" => crate::runtime::copilot::command(
            folder, prompt, mode, model, effort, resume, session_id, references, skills,
        ),
        "antigravity" => crate::runtime::antigravity::command(folder, mode, model, effort, resume, references, skills),
        "muse" => crate::runtime::muse::command(folder, mode, model, effort, resume),
        "acp" => crate::runtime::acp::command(
            &electron_dir().ok_or_else(|| io::Error::other("Locust profile not found"))?,
            folder,
            model,
        ),
        "opencode" => {
            let store = store()
                .ok_or_else(|| io::Error::other("Native data folder could not be determined"))?;
            let mut c = crate::runtime::opencode::command(folder, mode, &store)?;
            // OpenCode 1's config gets the reference folders a read-only run may read (openCodeModeEnv).
            let config = c
                .get_envs()
                .find(|(k, _)| *k == "OPENCODE_CONFIG_CONTENT")
                .and_then(|(_, v)| v.and_then(|v| v.to_str()).and_then(|v| serde_json::from_str::<Value>(v).ok()));
            if let Some(config) = config.filter(|c| c["permission"].is_object() && (!references.is_empty() || skills.is_some())) {
                let config = crate::runtime::opencode_config::with_references(config, mode, references);
                c.env("OPENCODE_CONFIG_CONTENT", crate::runtime::opencode_config::with_skills(config, skills).to_string());
            }
            Ok(c)
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
#[cfg(any(test, feature = "test-hooks"))]
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
    account: Option<String>,
}
/// teammate-store.ts: isTeammateRoute. A malformed route reads as no route.
fn route_of(v: &Value) -> Option<Route> {
    let runtime = v["runtime"].as_str().filter(|r| RUNTIMES.contains(r))?;
    let model = v["model"]
        .as_str()
        .filter(|m| !m.is_empty() && m.encode_utf16().count() <= 200)?;
    let mode = v["mode"].as_str().filter(|m| MODES.contains(m))?;
    let account = match &v["account"] {
        Value::Null => None,
        Value::String(a) => {
            if !matches!(runtime, "codex" | "claude") || !crate::agent_accounts::is_account_id(a) {
                return None;
            }
            Some(a.clone())
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
        let (runtime, model, runs_in, account) = match &route {
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
                ) =>
            {
                (
                    Some(r.runtime.clone()),
                    Some(r.model.clone()),
                    r.mode.clone(),
                    r.account.clone(),
                )
            }
            Some(r) => (
                Some(r.runtime.clone()),
                Some(r.model.clone()),
                IN_LOCUST.to_string(),
                None,
            ),
            None => (None, None, IN_LOCUST.to_string(), None),
        };
        out.push(Teammate {
            id: id.into(),
            name,
            role: title.unwrap_or_else(|| role.to_string()),
            runtime,
            model: model.filter(|m| m != "account-default"),
            account,
            busy: false,
            runs_in,
            folder: teammate_folder(&entry["folder"]),
        });
    }
    Ok(out)
}

/// Electron's roster. The file is opened for reading only; a missing file is an empty roster, as in Electron.
#[cfg(any(test, feature = "test-hooks"))]
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

/// What a teammate's face is drawn from (teammate-store.ts: `hue` and `avatar`): the hue the person chose, the three
/// seeded parts when the record keeps them, and the bot they picked, if any. Pets are not drawn here yet; a teammate
/// wearing one shows its bot.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FaceLook {
    pub hue: String,
    /// headwear, accessory, mouth (avatar.ts AvatarSpec), when the record has a valid avatar.
    pub parts: Option<[u8; 3]>,
    /// The picked bot's shape and face (`eyes` or `mouth`).
    pub bot: Option<(String, String)>,
}

/// Each valid teammate's look, by id, from the same file `parse_roster` reads. Records it drops are left out.
pub fn parse_looks(text: &str) -> std::collections::HashMap<String, FaceLook> {
    let mut out = std::collections::HashMap::new();
    let Ok(file) = serde_json::from_str::<Value>(text) else { return out };
    let kept: Vec<String> = parse_roster(text).map(|r| r.into_iter().map(|t| t.id).collect()).unwrap_or_default();
    for entry in file["teammates"].as_array().into_iter().flatten().take(MAX_TEAMMATES) {
        let (Some(id), Some(hue)) = (safe_id(&entry["teammateId"]), entry["hue"].as_str().filter(|h| HUES.contains(h)))
        else {
            continue;
        };
        if !kept.iter().any(|k| k == id) || out.contains_key(id) {
            continue;
        }
        let avatar = &entry["avatar"];
        let part = |name: &str, count: u64| avatar[name].as_u64().filter(|v| *v < count).map(|v| v as u8);
        let parts = match (part("headwear", 6), part("accessory", 3), part("mouth", 4)) {
            (Some(h), Some(a), Some(m)) => Some([h, a, m]),
            _ => None,
        };
        let bot = (|| {
            let shape = avatar["bot"]["shape"].as_str()?;
            let face = avatar["bot"]["face"].as_str().filter(|f| matches!(*f, "eyes" | "mouth"))?;
            let ok = !shape.is_empty() && shape.len() <= 24 && shape.bytes().all(|b| b.is_ascii_lowercase());
            ok.then(|| (shape.to_owned(), face.to_owned()))
        })();
        out.insert(id.to_owned(), FaceLook { hue: hue.to_owned(), parts, bot });
    }
    out
}

/// The looks of the teammates in this computer's roster (see `load_roster`).
pub fn load_looks() -> std::collections::HashMap<String, FaceLook> {
    Store::local()
        .ok()
        .and_then(|store| crate::people::team(&store).ok())
        .map(|file| parse_looks(&file.to_string()))
        .unwrap_or_default()
}

pub fn load_roster() -> Result<Vec<Teammate>, String> {
    let store = Store::local().map_err(|e| e.to_string())?;
    roster_in(&store)
}

/// The roster in one data folder.
pub fn roster_in(store: &Store) -> Result<Vec<Teammate>, String> {
    parse_roster(&crate::people::team(store)?.to_string())
}

/// The folder a `workspace.json` names, when it is a folder that exists.
fn workspace_in(file: &Path) -> Option<PathBuf> {
    let file = std::fs::File::open(file).ok()?;
    let mut text = String::new();
    file.take(64 * 1024).read_to_string(&mut text).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let path = PathBuf::from(value["path"].as_str()?);
    path.is_absolute().then_some(path).filter(|p| p.is_dir())
}

/// The folder Locust has open: the native app's own choice (its `workspace.json`), else the one Electron last had open
/// (read only).
fn workspace_folder(electron: &Path) -> Option<PathBuf> {
    store().and_then(|s| workspace_in(&s.root().join("workspace.json"))).or_else(|| workspace_in(&electron.join("workspace.json")))
}

/// Opens a folder in the native app (Home's Choose folder, a project named in a message): its own `workspace.json`,
/// in Electron's shape. Electron's is never written.
pub fn set_open_folder(store: &Store, path: &Path) -> Result<(), String> {
    store.save_json("workspace.json", &json!({"schemaVersion": 1, "path": path.to_string_lossy()}))
}

/// Where a turn of this teammate runs, and the project it belongs to when that is the teammate's own copy: a teammate
/// set to `worktree` (Antigravity aside, as in Electron) works in `.locust/worktrees/<id>` on its own branch. When the
/// copy cannot be made, the turn is refused, saying why.
fn own_branch_folder(store: &Store, teammate: &Teammate, runtime: &str, folder: PathBuf) -> Result<(PathBuf, Option<PathBuf>), String> {
    let wants = crate::people::team(store)
        .ok()
        .and_then(|team| team["teammates"].as_array().and_then(|all| all.iter().find(|t| t["teammateId"] == teammate.id.as_str()).map(|t| t["worktree"] == true)))
        .unwrap_or(false);
    if !wants || runtime == "antigravity" {
        return Ok((folder, None));
    }
    match crate::worktrees::Manager::new(&folder).ensure(&teammate.id, &teammate.name, None) {
        Ok(tree) => Ok((tree, Some(folder))),
        Err(why) => {
            let mut chars = why.chars();
            let lowered = chars.next().map(|c| c.to_lowercase().collect::<String>() + chars.as_str()).unwrap_or_default();
            Err(format!("{} is set to work on its own branch, but {lowered}", teammate.name))
        }
    }
}

/// Commit a turn in a teammate's own copy on its branch, and record the receipt in the turn's stream: what was saved,
/// what was left out for being too big, or why nothing could be saved (the files then stay in the copy).
fn checkpoint_turn(store: &Store, mission: &str, root: &Path, teammate: &Teammate, prompt: &str, runtime: &str, model: Option<&str>) {
    use crate::worktrees::{CHECKPOINT_CODE, Manager, TurnOutcome, checkpoint_message, checkpoint_sentence};
    let Ok(path) = store.write_path(mission) else { return };
    let transcript = ledger::read_transcript(path.clone());
    let stopped = transcript.items.iter().any(|i| matches!(i, ledger::Item::Note { text, .. } if text == "Stopped"));
    let outcome = if stopped {
        TurnOutcome::Stopped
    } else if transcript.failed || transcript.running {
        TurnOutcome::Failed
    } else {
        TurnOutcome::Completed
    };
    let answer = transcript.items.iter().filter_map(|i| match i {
        ledger::Item::Assistant { text, .. } => Some(text.as_str()),
        _ => None,
    }).next_back();
    let message = checkpoint_message(prompt, answer, &teammate.id, &teammate.name, mission, runtime, model, outcome);
    let result = Manager::new(root).checkpoint(&teammate.id, &message);
    if let Some(sentence) = checkpoint_sentence(&result) {
        let level = if result.is_err() { "warning" } else { "info" };
        if let Err(error) = crate::ledger_write::append_note_with_level(&path, level, CHECKPOINT_CODE, &sentence) {
            eprintln!("The turn's receipt was not recorded: {error}");
        }
    }
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
    // A native turn whose process is gone (Locust quit or crashed before its last record) is not running, whatever the
    // file's age says.
    for summary in all.iter_mut().filter(|s| s.running) {
        if orphaned(&summary.mission_id) {
            summary.running = false;
        }
    }
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
pub fn find_ledger(id: &str) -> Option<PathBuf> {
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
    pub account: Option<String>,
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
    let account = meta["resolvedRouteId"]
        .as_str()
        .and_then(crate::agent_accounts::account_from_route)
        .map(str::to_owned);
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
        account,
        running: transcript.running && fresh,
        thread,
    })
}

// ---- running turns, stop ------------------------------------------------------------------------------------------
static RUNNING: Mutex<Option<HashMap<String, Arc<AtomicBool>>>> = Mutex::new(None);
static RUNNING_ACCOUNTS: Mutex<Option<HashMap<String, (crate::agent_accounts::Agent, String)>>> =
    Mutex::new(None);

fn with_running<T>(f: impl FnOnce(&mut HashMap<String, Arc<AtomicBool>>) -> T) -> T {
    let mut guard = RUNNING.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(HashMap::new))
}

fn with_running_accounts<T>(
    f: impl FnOnce(&mut HashMap<String, (crate::agent_accounts::Agent, String)>) -> T,
) -> T {
    let mut guard = RUNNING_ACCOUNTS.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(HashMap::new))
}

pub fn account_in_use(agent: crate::agent_accounts::Agent, account_id: &str) -> bool {
    with_running_accounts(|m| m.values().any(|(a, id)| *a == agent && id == account_id))
}

pub fn accounts_for(store: &Store) -> Arc<crate::agent_accounts::Accounts> {
    let login = Arc::new(
        |agent: crate::agent_accounts::Agent,
         home: &Path,
         env: &[(&'static str, String)],
         done: Box<dyn FnOnce(bool) + std::marker::Send>| {
            crate::agent_accounts::start_login(Path::new(agent.as_str()), agent, home, env, done)
        },
    );
    crate::agent_accounts::shared(
        store.root(),
        login,
        Box::new(|| crate::org_policy::feature_refusal(Some(&crate::org_policy::current()), "accounts")),
        Box::new(account_in_use),
    )
}

pub fn account_launch_env(
    store: &Store,
    runtime: &str,
    account: Option<&str>,
) -> Result<Vec<(&'static str, String)>, String> {
    let Some(account) = account else {
        return Ok(Vec::new());
    };
    if crate::agent_accounts::Agent::parse(runtime).is_none() {
        return Err(crate::agent_accounts::NOT_SUPPORTED.into());
    }
    accounts_for(store).launch_env(runtime, Some(account))
}

struct Registered(Vec<String>, Option<String>);
impl Drop for Registered {
    fn drop(&mut self) {
        with_running(|m| self.0.iter().for_each(|k| drop(m.remove(k))));
        if let Some(id) = &self.1 {
            with_running_accounts(|m| drop(m.remove(id)));
        }
    }
}

/// Every turn (and conversation anchor) this process is running now.
/// shared/live-missions.ts MAX_LIVE_MISSIONS: past this many turns at once the screen stalls, so the ninth is refused.
pub const MAX_LIVE_TURNS: usize = 8;

/// codex-mission.ts, the pool's refusal, in Electron's words.
pub fn too_many_live() -> String {
    format!("Up to {MAX_LIVE_TURNS} missions can run at once. Wait for one to finish or stop it first.")
}

/// Turns running in this process: each registers under its conversation and its own id, sharing one stop flag.
fn live_turns(m: &HashMap<String, Arc<AtomicBool>>) -> usize {
    m.values().map(Arc::as_ptr).collect::<std::collections::HashSet<_>>().len()
}

/// A native turn id names the process that ran it (`mission_native_<pid>_<nanos>`). It is orphaned when that process is
/// this one and no longer runs it, or another process that is gone. Ids of any other shape are never orphaned here.
pub fn orphaned(mission_id: &str) -> bool {
    let Some(pid) = mission_id
        .strip_prefix("mission_native_")
        .and_then(|rest| rest.split('_').next())
        .and_then(|pid| pid.parse::<u32>().ok())
    else {
        return false;
    };
    if pid == std::process::id() {
        return !owns(mission_id);
    }
    !process_alive(pid)
}

fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        // Signal 0 checks the process exists without touching it; EPERM means it exists under another user.
        let result = unsafe { libc::kill(pid as i32, 0) };
        result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

pub fn running_ids() -> Vec<String> {
    with_running(|m| m.keys().cloned().collect())
}

/// The folder Locust has open: Electron's last workspace, read only.
pub fn open_folder() -> Option<PathBuf> {
    workspace_folder(store()?.electron())
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

pub fn approvals_channel() -> std::sync::Arc<Channel> {
    let mut guard = crate::invariants::lock(&APPROVALS);
    guard.get_or_insert_with(Channel::new).clone()
}

/// Cards waiting on the person right now.
pub fn waiting_approvals() -> Vec<ApprovalRequest> {
    approvals_channel().waiting()
}

/// Cards waiting for one mission (or conversation tip): the door says a turn waits on the person.
pub fn waiting_approvals_for(mission_id: &str) -> Vec<ApprovalRequest> {
    approvals_channel().waiting_for_mission(mission_id)
}

/// Answer a card from the window. Refused after the turn ended.
/// A question card's answers, to the channel that holds it.
pub fn answer_approval(approval_id: &str, answers: Vec<(String, Vec<String>)>) -> Result<(), String> {
    approvals_channel().decide(Answer::Answers { approval_id: approval_id.into(), answers }).map(|_| ())
}

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
pub fn uuid_v4() -> io::Result<String> {
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

/// What a Claude Code turn's command line carries in this engine: the bridge outside Auto, the browser when the run
/// has it, the command guard in every mode, and no account connectors (none can be ticked here yet, and leaving them
/// out keeps about 170,000 tokens of their tool lists out of each turn). Skills, reference folders and forks are not
/// wired yet. `command_grid_tests` compares the same composition with Electron's.
pub fn claude_args(
    mode: Mode,
    bridge: Option<&str>,
    browser: Option<&str>,
    guard_settings: &str,
    model: Option<&str>,
    effort: Option<&str>,
    resume: Option<&str>,
    references: &[&str],
    skills: &[&str],
) -> Result<Vec<String>, String> {
    claude_args_with_fork(
        mode,
        bridge,
        browser,
        guard_settings,
        model,
        effort,
        resume,
        false,
        references,
        skills,
    )
}

#[allow(clippy::too_many_arguments)]
fn claude_args_with_fork(
    mode: Mode,
    bridge: Option<&str>,
    browser: Option<&str>,
    guard_settings: &str,
    model: Option<&str>,
    effort: Option<&str>,
    resume: Option<&str>,
    fork: bool,
    references: &[&str],
    skills: &[&str],
) -> Result<Vec<String>, String> {
    crate::runtime::claude::print_args(&crate::runtime::claude::PrintOptions {
        mode,
        bridge: if mode == Mode::Auto { None } else { bridge },
        browser,
        guard_settings: Some(guard_settings),
        model,
        effort,
        resume,
        fork,
        references,
        skills,
        ..Default::default()
    })
}

/// A reply on another agent: the brief that starts it, the link it records, and the stopped turn's checkpoint.
struct RouteSwitch {
    prompt: String,
    link: Value,
    checkpoint: Value,
    /// The stopped turn's ledger, when it is the native engine's own (the checkpoint is appended there).
    native_prior: Option<PathBuf>,
}

/// One turn's records, read whole: its metadata, its events in order, its checkpoints, and whether every line read.
pub struct TurnRecords {
    pub path: PathBuf,
    pub metadata: Value,
    pub events: Vec<Value>,
    pub checkpoints: usize,
    pub incomplete: bool,
}

pub fn turn_records(mission: &str) -> Option<TurnRecords> {
    let path = find_ledger(mission)?;
    let text = std::fs::read_to_string(&path).ok()?;
    let lines = text.lines().filter(|l| !l.trim().is_empty()).count();
    let records = Tail::new(&path, 0).poll();
    let metadata = records.iter().find(|r| r["recordType"] == "mission.created")?["metadata"].clone();
    let mut events: Vec<Value> = records.iter().filter_map(|r| r.get("event").cloned()).collect();
    events.sort_by_key(|e| e["sequence"].as_u64().unwrap_or(0));
    events.dedup_by_key(|e| e["sequence"].as_u64());
    let checkpoints = records.iter().filter(|r| r["recordType"] == "mission.checkpoint").count();
    Some(TurnRecords { incomplete: records.len() < lines, path, metadata, events, checkpoints })
}

/// earlierTurnsOf: the turns before this one, oldest first, up to four.
/// A conversation up to and including this turn, oldest first, as a cold brief quotes it (Electron
/// `[...earlierTurnsOf(prior), { asked, answered: latestFinal }]`).
pub fn turns_through(mission: &str) -> Vec<crate::carry_on::EarlierTurn> {
    let Some(turn) = turn_records(mission) else { return Vec::new() };
    let mut turns = earlier_turns(&turn.metadata);
    turns.push(crate::carry_on::EarlierTurn {
        asked: turn.metadata["prompt"].as_str().unwrap_or_default().to_owned(),
        answered: crate::carry_on::latest_final(&turn.events),
    });
    turns
}

fn earlier_turns(metadata: &Value) -> Vec<crate::carry_on::EarlierTurn> {
    let mut turns = Vec::new();
    let mut from = metadata["continuesFrom"]["missionId"].as_str().map(str::to_owned);
    let mut seen = HashSet::new();
    while let Some(id) = from.take() {
        if turns.len() >= 4 || !seen.insert(id.clone()) {
            break;
        }
        let Some(earlier) = turn_records(&id) else { break };
        turns.insert(
            0,
            crate::carry_on::EarlierTurn {
                asked: earlier.metadata["prompt"].as_str().unwrap_or_default().to_owned(),
                answered: crate::carry_on::latest_final(&earlier.events),
            },
        );
        from = earlier.metadata["continuesFrom"]["missionId"].as_str().map(str::to_owned);
    }
    turns
}

fn route_switch(store: &Store, tip: &str, reply: &str) -> Result<RouteSwitch, String> {
    let prior = turn_records(tip).ok_or("That conversation cannot be continued: its earlier mission is not in the ledger.")?;
    let epoch = prior.checkpoints as u64 + 1;
    let checkpoint = crate::carry_on::reconcile(&prior.metadata, &prior.events, prior.incomplete, "route-switch", epoch, &crate::ledger_write::now());
    if checkpoint["resumeSafety"] == "unsafe" {
        return Err(format!(
            "That conversation cannot be continued safely on another runtime: {}",
            checkpoint["safetyReason"].as_str().unwrap_or_default()
        ));
    }
    let from = prior.metadata["runtime"].as_str().unwrap_or_default();
    let briefing = crate::carry_on::compose_handoff_prompt(
        prior.metadata["prompt"].as_str().unwrap_or_default(),
        &checkpoint,
        crate::carry_on::display_name(from),
        Some(reply),
        &earlier_turns(&prior.metadata),
        crate::carry_on::how_turn_ended(&prior.events),
    )
    .ok_or("That conversation is too long to carry to another runtime with this reply.")?;
    let mut link = json!({"missionId": tip, "checkpointEpoch": epoch, "reason": "route-switch"});
    if !briefing.omitted.is_empty() {
        link["leftOut"] = json!(briefing.omitted);
    }
    let native_prior = store.ledger_dir().ok().filter(|dir| prior.path.starts_with(dir)).map(|_| prior.path.clone());
    Ok(RouteSwitch { prompt: briefing.prompt, link, checkpoint, native_prior })
}

/// Turns already carried on, so a second press while the first is starting is refused rather than run twice.
static CARRIED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// main/carry-on.ts `carry`: carry a turn that stopped on its plan, its provider or its process on to another agent.
/// The new turn continues the conversation in the stopped turn's mode (and an app's folder), told what was done.
/// Returns the new turn's id, the folder it works in and the mode it runs in.
pub fn carry(mission: &str, runtime: &str, model: Option<&str>) -> Result<(String, Option<String>, String), String> {
    if crate::invariants::lock(&CARRIED).iter().any(|m| m == mission) {
        return Err("This turn was already carried on.".into());
    }
    let store = store().ok_or("Native data folder could not be determined.")?;
    let prior = turn_records(mission).ok_or("This conversation is not in the record any more, so it cannot be carried on.")?;
    let phase_done = prior.events.iter().rev().find_map(|e| e["type"].as_str().filter(|t| matches!(*t, "run.completed" | "run.cancelled" | "run.failed")));
    let reason = match phase_done {
        Some("run.completed" | "run.cancelled") => None,
        _ => crate::carry_on::carry_on_reason(&prior.events),
    }
    .ok_or("This turn did not stop on its plan running out or its provider or process failing, so there is nothing to carry on.")?;
    let from = prior.metadata["runtime"].as_str().unwrap_or_default().to_owned();
    if runtime == from {
        let name = crate::carry_on::display_name(&from);
        return Err(if reason == crate::carry_on::Reason::Plan {
            format!("{name} is the agent whose plan ran out.")
        } else {
            format!("{name} is the agent that just failed.")
        });
    }
    let mode = prior.metadata["mode"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| if prior.metadata["sandbox"] == "read-only" { "ask".into() } else { "accept-edits".into() });
    if let Some(refused) = crate::carry_on::carry_on_refusal(&mode, runtime) {
        return Err(refused);
    }
    let owner = crate::people::owner_of(&store, mission).ok_or("This conversation no longer has a teammate, so it cannot be carried on.")?;
    let mut teammate = roster_in(&store)?
        .into_iter()
        .find(|t| t.id == owner)
        .ok_or("This conversation no longer has a teammate, so it cannot be carried on.")?;
    // A conversation an app sent to a folder is carried on there, checked again: it may be gone.
    let mut folder = None;
    if let Some(was) = crate::door_tools::conversation_folder(&prior.metadata["startedBy"]) {
        folder = Some(crate::door::check_folder(&store, &was).map_err(|why| format!("This conversation worked in {was}: {why}"))?);
    }
    crate::invariants::lock(&CARRIED).push(mission.to_owned());
    teammate.model = model.map(str::to_owned);
    teammate.account = None;
    let mut started_by = json!({"kind": "carry-on", "from": from});
    if let Some(f) = &folder {
        started_by["folder"] = json!(f.to_string_lossy());
    }
    let job = Send {
        teammate,
        conversation: Some(mission.to_owned()),
        prompt: crate::carry_on::carry_on_prompt(&from, reason),
        model: model.map(str::to_owned),
        effort: None,
    };
    let origin = Origin {
        mode: Mode::parse(&mode),
        folder: folder.clone(),
        started_by: Some(started_by),
        runtime: Some(runtime.to_owned()),
        account: Some(None),
        ..Default::default()
    };
    let started = send_from(job, origin).and_then(|s| real_tip(&s.conversation_id));
    match started {
        Ok(id) => Ok((id, folder.map(|f| f.to_string_lossy().into_owned()), mode)),
        Err(why) => {
            crate::invariants::lock(&CARRIED).retain(|m| m != mission);
            Err(why)
        }
    }
}

/// main/carry-on.ts onRunEnded: when the person picked an agent in Settings and turned on Without asking, a turn
/// that stopped on its plan, provider or process is carried on to it at once; when that cannot be, the stopped turn
/// says why in its own thread.
pub fn carry_on_without_asking(store: &Store, mission: &str) {
    let Some(chosen) = crate::people::team(store)
        .ok()
        .and_then(|team| crate::carry_on::parse_plan_runs_out(&team["settings"]["planRunsOut"]))
        .filter(|c| c.automatic)
    else {
        return;
    };
    let Some(prior) = turn_records(mission) else { return };
    let ended = prior.events.iter().rev().find_map(|e| e["type"].as_str().filter(|t| matches!(*t, "run.completed" | "run.cancelled" | "run.failed")));
    let reason = match ended {
        Some("run.completed" | "run.cancelled") => None,
        _ => crate::carry_on::carry_on_reason(&prior.events),
    };
    let Some(reason) = reason else { return };
    let name = crate::carry_on::display_name(&chosen.runtime).to_owned();
    let native_ledger = store.ledger_dir().ok().is_some_and(|dir| prior.path.starts_with(dir));
    let note = |message: String| {
        if native_ledger && let Err(error) = crate::ledger_write::append_note(&prior.path, "host.carry_on", &message) {
            eprintln!("The carry-on note was not kept: {error}");
        }
    };
    if prior.metadata["runtime"].as_str() == Some(chosen.runtime.as_str()) {
        note(match reason {
            crate::carry_on::Reason::Plan => format!("{name} is the agent picked to carry on when a plan runs out, and its own plan ran out, so nothing was carried on."),
            crate::carry_on::Reason::Process => format!("{name} is the agent picked to carry on, and its own process failed, so nothing was carried on."),
            crate::carry_on::Reason::Provider => format!("{name} is the agent picked to carry on, and it is the one whose provider failed, so nothing was carried on."),
        });
        return;
    }
    if let Err(why) = carry(mission, &chosen.runtime, chosen.model.as_deref()) {
        note(format!("Locust could not carry this on with {name}: {why}"));
    }
}

/// createSpendRefusal for a teammate's own limit (the port has no organization policy yet). Reads both ledgers, so it
/// runs only for a teammate that has a limit.
fn spend_gate(store: &Store, owners: &Value, teammate: &Teammate, limit: f64) -> Result<(), String> {
    let now = chrono::Local::now();
    let month = crate::spend::month_of(&now.to_rfc3339()).ok_or("The current month could not be read.")?;
    let turns = crate::usage_ledger::usage_turns(store).map_err(|_| "The spending record could not be read.".to_owned())?;
    let spent = crate::spend::spent_by(&turns, &month, |conversation| owners[conversation] == teammate.id.as_str());
    if crate::spend::limit_reached(Some(&spent), limit) {
        return Err(crate::spend::limit_refusal(&teammate.name, Some(&spent), limit, now));
    }
    Ok(())
}

/// createSpendRefusal under an organization's cap: the effective limit is the cap or a lower personal amount; an
/// owned turn counts its teammate's receipts, an unowned one the whole month's; and a record that cannot be read
/// closes the gate.
fn org_spend_gate(store: &Store, policy: &crate::org_policy::PolicyState, owners: &Value, teammate: &Teammate, owned: bool, personal: Option<f64>) -> Result<(), String> {
    let Some(limit) = crate::org_policy::monthly_limit(Some(policy), personal) else { return Ok(()) };
    let unreadable = || crate::org_policy::policy_refusal(policy, "monthlyLimitUsd", "The monthly spending limit could not be checked. Ask your administrator to restore access to the spending record.");
    let now = chrono::Local::now();
    let month = crate::spend::month_of(&now.to_rfc3339()).ok_or_else(unreadable)?;
    let turns = crate::usage_ledger::usage_turns(store).map_err(|_| unreadable())?;
    let spent = if owned {
        crate::spend::spent_by(&turns, &month, |conversation| owners[conversation] == teammate.id.as_str())
    } else {
        // Unowned work uses the whole month's receipts: dropping an owner cannot escape the cap.
        crate::spend::spent_by(&turns, &month, |_| true)
    };
    if crate::spend::limit_reached(Some(&spent), limit) {
        let who = if owned { teammate.name.as_str() } else { "Locust" };
        return Err(crate::org_policy::policy_refusal(policy, "monthlyLimitUsd", &format!("{who} has reached this month's effective limit of {limit:.2} USD. It starts again next month.")));
    }
    Ok(())
}

/// codex-mission.ts AUTO_OFF_REFUSAL.
pub const AUTO_OFF_REFUSAL: &str = "Auto mode is switched off for this workspace. Turn it on in Settings to let a run work outside the workspace folder. Nothing was recorded.";

/// A comparison column or judge slot (`codex-mission.ts`: `slot`), holding its own run slot, folder, and sealing.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlotOrigin {
    pub key: String,
    pub cwd: Option<PathBuf>,
    pub instructions: Option<String>,
    /// Candidate skill for a tune column, handed only to that column's run.
    pub skill: Option<crate::tune::TuneSkill>,
    /// Electron seals only Scout's runs (index.ts 8681, 10103): no brief, no skills, no browser here. Its full seal also
    /// refuses every OpenCode tool and starts Codex with none (codexSealedThreadConfig); that part is not ported yet,
    /// and nothing sets this until Scout is.
    pub sealed: bool,
}

/// What started a turn, when it was not the person in the window: another app through the door. A mode and folder it
/// named (already checked), and the `startedBy` the ledger records.
#[derive(Default, Clone)]
pub struct Origin {
    pub mode: Option<Mode>,
    pub folder: Option<PathBuf>,
    pub started_by: Option<Value>,
    /// A follow-up's runtime and model, kept from the conversation rather than the teammate's saved route.
    pub runtime: Option<String>,
    /// An explicit account pick (`Some(None)` = machine sign-in, `Some(Some(id))` = named account, `None` = follow teammate/default).
    pub account: Option<Option<String>>,
    /// The references the composer names for this turn (ids in `references.rs`); None keeps the conversation's.
    pub reference_ids: Option<Vec<String>>,
    /// A comparison column or judge slot.
    pub slot: Option<SlotOrigin>,
}

static EDIT_CHECK: std::sync::OnceLock<crate::edit_check::EditCheck> = std::sync::OnceLock::new();
static CHECK_FIX: std::sync::OnceLock<crate::edit_check::CheckFix> = std::sync::OnceLock::new();

fn edit_check() -> &'static crate::edit_check::EditCheck {
    EDIT_CHECK.get_or_init(crate::edit_check::EditCheck::new)
}

fn check_fix() -> &'static crate::edit_check::CheckFix {
    CHECK_FIX.get_or_init(crate::edit_check::CheckFix::new)
}

pub fn running_checks() -> Vec<(String, String)> {
    edit_check().running_checks()
}

/// Start (or continue) a turn the person asked for in the window.
pub fn send(job: Send) -> Result<Started, String> {
    send_from(job, Origin::default())
}

/// Start (or continue) a Codex/Claude turn natively. Returns once the ledger exists and the turn is running on its own thread;
/// the window follows the ledger file. Approvals raise cards answered through `decide_approval`.
/// Never retried: a failure after the ledger exists is recorded in it, not tried again.
pub fn send_from(mut job: Send, origin: Origin) -> Result<Started, String> {
    let own_agent = origin.runtime.is_none() || origin.runtime == job.teammate.runtime;
    if let Some(runtime) = &origin.runtime {
        job.teammate.runtime = Some(runtime.clone());
    }
    let store = store().ok_or("Native data folder could not be determined.")?;
    let saved = crate::people::team(&store)?;
    // The saved effort belongs to the teammate's own agent; another agent's turn (a carry-on) takes its own default.
    if let Some(entry) = saved["teammates"]
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["teammateId"] == job.teammate.id))
        && job.effort.is_none()
        && own_agent
    {
        job.effort = entry["route"]["effort"].as_str().map(str::to_owned);
    }

    if !supported(&job.teammate) {
        return Err(format!("{} runs in Locust for now.", job.teammate.name));
    }
    let effective_account = match &origin.account {
        Some(explicit) => explicit.clone(),
        None if own_agent => job.teammate.account.clone(),
        None => None,
    };
    // organization-policy.ts: every turn crosses the organization's policy before anything is discovered, started or
    // recorded. A broken policy file refuses every turn.
    let policy = crate::org_policy::current();
    {
        let runtime = job.teammate.runtime.as_deref().unwrap_or("codex");
        let mode = origin.mode.unwrap_or_else(|| Mode::parse(&job.teammate.runs_in).unwrap_or(Mode::Ask));
        let model = job.model.as_deref().or(job.teammate.model.as_deref());
        if let Some(why) = crate::org_policy::turn_refusal(
            Some(&policy),
            runtime,
            model,
            mode.as_str(),
            false,
            effective_account.is_some(),
        ) {
            return Err(why);
        }
    }
    // organization-spend.ts: a teammate with a monthly limit that this month's spending has reached does not start, and
    // the organization's cap holds over any personal amount, for a turn with no teammate in the roster too.
    let in_roster = saved["teammates"].as_array().and_then(|rows| rows.iter().find(|r| r["teammateId"] == job.teammate.id));
    let personal = in_roster.and_then(|entry| entry["monthlyLimitUsd"].as_f64()).filter(|l| crate::spend::is_monthly_limit(*l));
    if policy.policy.contains_key("monthlyLimitUsd") {
        org_spend_gate(&store, &policy, &saved["missionOwners"], &job.teammate, in_roster.is_some(), personal)?;
    } else if let Some(limit) = personal {
        spend_gate(&store, &saved["missionOwners"], &job.teammate, limit)?;
    }
    let side_req = crate::side_question::parse_started_by(origin.started_by.as_ref());
    let is_side = side_req.is_some();
    let is_slot = origin.slot.is_some();
    let sealed = origin.slot.as_ref().is_some_and(|s| s.sealed);
    // One turn at a time per conversation (or slot): refused before anything is read or created.
    if let Some(slot) = &origin.slot {
        if owns(&slot.key) {
            return Err("A turn is already running in this conversation.".into());
        }
    } else if !is_side && job.conversation.as_deref().is_some_and(owns) {
        return Err("A turn is already running in this conversation.".into());
    }
    // And no more than eight at once (checked again where the turn registers, so two sends cannot both slip in).
    if with_running(|m| live_turns(m)) >= MAX_LIVE_TURNS {
        return Err(too_many_live());
    }
    let runtime_name = job.teammate.runtime.as_deref().unwrap_or("codex");
    let mode = origin.mode.unwrap_or_else(|| Mode::parse(&job.teammate.runs_in).unwrap_or(Mode::Ask));
    // codex-mission.ts AUTO_OFF_REFUSAL: Auto runs only where the person turned it on in Settings.
    if mode == Mode::Auto && !crate::workspace_settings::load(&store).map(|s| s.auto_mode).unwrap_or(false) {
        return Err(AUTO_OFF_REFUSAL.into());
    }
    let launch_env = account_launch_env(&store, runtime_name, effective_account.as_deref())?;
    let electron = store.electron().to_path_buf();
    // A reply on ANOTHER agent or account (codex-mission.ts, the route switch): the conversation is reconciled, and a
    // brief of what was done starts the new agent/account cold, with the reply as the latest word.
    let mut switched: Option<RouteSwitch> = None;
    let mut side_prompt: Option<String> = None;
    let (continues, resume) = if let Some(side) = &side_req {
        let prepared = crate::side_question::prepare(&side.of, runtime_name, mode, &job.prompt)?;
        side_prompt = Some(prepared.runtime_prompt);
        (None, Some(prepared.resume_thread))
    } else {
        match job.conversation.as_deref() {
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
            if prior.runtime != runtime_name || prior.account != effective_account {
                switched = Some(route_switch(&store, tip, &job.prompt)?);
                (None, None)
            } else if runtime_name == "claude" {
                // A Claude conversation resumes its session when one was recorded; one that never reached Claude
                // Code's init (or ended it) starts a new one, as it always has here.
                let thread = prior.thread;
                (
                    Some(Continuation {
                        mission_id: tip.to_owned(),
                        thread: thread.clone(),
                    }),
                    thread,
                )
            } else if matches!(
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
    }
    };
    // A teammate set to work on its own branch runs in its own copy of the project, and each of its turns is
    // committed there with a receipt (worktrees.rs; Electron's peer.repositoryRoot).
    // A comparison slot with its own `cwd` already has its own copy or worktree.
    let (folder, repository_root) = if let Some(slot_cwd) = origin.slot.as_ref().and_then(|s| s.cwd.clone()) {
        (slot_cwd, None)
    } else {
        let folder = match &origin.folder {
            Some(named) => named.clone(),
            None => folder_for(&job.teammate, &electron)?,
        };
        own_branch_folder(&store, &job.teammate, runtime_name, folder)?
    };
    let checkpoint_prompt = job.prompt.clone();
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
    // Cursor takes its effort inside the model id (model-catalog.ts cursorChoiceFor): a level the family lists becomes
    // that variant's id; one it does not list stays, and the driver refuses it in words.
    let model = if runtime_name == "cursor" && job.effort.is_some() {
        let (chosen, effort) = crate::models::cursor_choice_for(&crate::models::local(), model.as_deref(), job.effort.as_deref());
        job.effort = effort;
        chosen
    } else {
        model
    };
    // A switch starts the new agent with the brief, not the bare reply, and records the brief as the turn's words.
    if let Some(switch) = &switched {
        job.prompt = switch.prompt.clone();
    }
    // The references this turn may read without its mode changing (referenceAccess), named in its prompt and handed
    // to the agents that take a folder.
    let folder_text = folder.to_string_lossy().into_owned();
    // The conversation it continues: the prior turn, or (for a switch) the turn it was briefed from.
    let previous: Option<String> = continues.as_ref().map(|c| c.mission_id.clone()).or_else(|| switched.as_ref().and_then(|s| s.link["missionId"].as_str().map(str::to_owned)));
    let attached = crate::references::select(&store, previous.as_deref(), origin.reference_ids.as_deref())?;
    let shared: Vec<String> = attached
        .iter()
        .filter(|item| crate::references::access(runtime_name, mode.as_str(), &item.path, Some(&folder_text), false).0)
        .map(|item| item.path.clone())
        .collect();
    let references: Vec<&str> = shared.iter().map(String::as_str).collect();
    let mut delivered: Vec<crate::workroom::Message> = Vec::new();
    let runtime_prompt = if let Some(side_text) = side_prompt {
        let refs = crate::references::section(&references);
        if refs.is_empty() {
            side_text
        } else {
            format!("{side_text}\n\n{refs}")
        }
    } else if sealed {
        let refs = crate::references::section(&references);
        if refs.is_empty() {
            job.prompt.clone()
        } else {
            format!("{}\n\n{refs}", job.prompt)
        }
    } else {
        let slot_inst = origin.slot.as_ref().and_then(|s| s.instructions.as_deref());
        // What teammates left for this one, quoted into its brief (peer-exchange.ts prepare). A workroom that cannot be
        // read is left out: the run goes on without it, and the messages wait.
        let (waiting, remaining) = crate::workroom::Workroom::for_store(&store)
            .unread(&job.teammate.id, crate::briefing::MAX_INBOUND_MESSAGES, &[])
            .map(|u| (u.messages, u.remaining))
            .unwrap_or_default();
        let (brief, quoted) = crate::people::turn_brief(&store, &job.teammate.id, &folder, &job.prompt, slot_inst, Some(runtime_name), &waiting, remaining)?;
        delivered = quoted;
        format!("{brief}\n\n{}", crate::references::section(&references))
    };
    let id = new_id();
    // Copilot needs a host session id on first turn so a failure before `result` can still resume.
    let session_id = if runtime_name == "copilot" && resume.is_none() {
        // Copilot takes only a UUID for a new session ("The value is not a valid UUID"), as the host mints one.
        Some(uuid_v4().map_err(|e| format!("Could not make a Copilot session id: {e}"))?)
    } else {
        None
    };
    // The run's skills (claude-skills.ts, run-skills.ts): Claude Code's plugins, or one folder for the agents that
    // read one. Copied for this run alone, and removed when it ends. Sealed runs get no teammate skills.
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let (claude_plugins, run_skills): (Vec<crate::skills::Plugin>, _) = if sealed {
        (Vec::new(), None)
    } else {
        let mut skill_sources = crate::skills::sources_for_store(&store, &job.teammate.id);
        skill_sources.candidate = origin.slot.as_ref().and_then(|s| s.skill.clone());
        let own_skills = saved["settings"]["claudeOwnSkills"] == true;
        let cp = if runtime_name == "claude" {
            crate::skills::prepare_claude(&store.root().join("claude-skills"), &folder, own_skills, home.as_deref(), mode == Mode::Auto, &skill_sources)
        } else {
            Vec::new()
        };
        let rs = if matches!(runtime_name, "codex" | "opencode" | "copilot" | "antigravity" | "cursor") {
            crate::skills::prepare_run(&store.root().join("run-skills"), &skill_sources)
        } else {
            None
        };
        (cp, rs)
    };
    let claude_skill_dirs: Vec<String> = claude_plugins.iter().map(|p| p.dir.to_string_lossy().into_owned()).collect();
    let claude_skills: Vec<&str> = claude_skill_dirs.iter().map(String::as_str).collect();
    let run_dir = run_skills.as_ref().map(|r| r.dir.to_string_lossy().into_owned());
    let run_skills_dir = run_skills.as_ref().map(|r| r.skills_dir.to_string_lossy().into_owned());
    if runtime_name == "codex"
        && let Some(dir) = &run_skills_dir
    {
        crate::skills::set_roots(&id, vec![dir.clone()]);
    }
    // OpenCode reads the skills folder itself; Copilot, Cursor and Antigravity read the folder around it.
    let agent_skills = match runtime_name {
        "opencode" => run_skills_dir.clone(),
        "copilot" | "cursor" | "antigravity" => run_dir.clone(),
        _ => None,
    };
    let skill_copies: Vec<PathBuf> =
        claude_plugins.first().and_then(|p| p.dir.parent().map(Path::to_path_buf)).into_iter().chain(run_skills.as_ref().map(|r| r.dir.clone())).collect();
    let mut command = command_for_turn(
        runtime_name,
        &folder,
        model.as_deref(),
        mode,
        resume.as_deref(),
        &runtime_prompt,
        job.effort.as_deref(),
        session_id.as_deref(),
        &references,
        agent_skills.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    command.current_dir(&folder);
    for (key, value) in &launch_env {
        command.env(key, value);
    }
    let codex_home = launch_env
        .iter()
        .find(|(k, _)| *k == "CODEX_HOME")
        .map(|(_, v)| PathBuf::from(v));
    // A side question on OpenCode 2 (ACP, no fork) gets a fresh session told the conversation so far: resuming would
    // add to the conversation's own session.
    let (resume, runtime_prompt) = match &side_req {
        Some(side) if runtime_name == "opencode" && crate::runtime::opencode::uses_acp(&command) => {
            (None, crate::side_question::fresh_session_prompt(&side.of, &runtime_prompt))
        }
        _ => (resume, runtime_prompt),
    };
    // No teammate ends the person's browser, Copilot too (0.721): the guard as a plugin, before the browser's config.
    if runtime_name == "copilot" {
        let plugin = crate::runtime::permission_bridge::bridge_executable()
            .and_then(|exe| crate::command_guard::write_copilot_plugin(&store.root().join("guard"), &exe))
            .map_err(|e| format!("Locust's command guard could not be set up: {e}"))?;
        command.args(["--plugin-dir", &plugin]);
    }
    let anchor = if is_side || is_slot {
        id.clone()
    } else {
        job.conversation.clone().unwrap_or_else(|| id.clone())
    };
    let side_slot = side_req.as_ref().map(|s| format!("side:{}", s.of));
    let compare_slot = origin.slot.as_ref().map(|s| s.key.clone());
    let flag = Arc::new(AtomicBool::new(false));
    let registered = with_running(|m| {
        if let Some(slot) = &side_slot {
            if m.contains_key(slot) {
                return Err("A side question is already running in this conversation.".to_owned());
            }
        } else if let Some(slot) = &compare_slot {
            if m.contains_key(slot) {
                return Err("A turn is already running in this conversation.".to_owned());
            }
        } else if m.contains_key(&anchor) {
            return Err("A turn is already running in this conversation.".to_owned());
        }
        if live_turns(m) >= MAX_LIVE_TURNS {
            return Err(too_many_live());
        }
        let mut keys = Vec::new();
        if let Some(slot) = side_slot {
            m.insert(slot.clone(), flag.clone());
            keys.push(slot);
        } else if let Some(slot) = compare_slot {
            m.insert(slot.clone(), flag.clone());
            keys.push(slot);
        } else {
            m.insert(anchor.clone(), flag.clone());
            keys.push(anchor.clone());
        }
        m.insert(id.clone(), flag.clone());
        keys.push(id.clone());
        let account_key = if let (Some(agent), Some(acc)) = (
            crate::agent_accounts::Agent::parse(runtime_name),
            effective_account.as_deref(),
        ) {
            with_running_accounts(|am| am.insert(id.clone(), (agent, acc.to_owned())));
            Some(id.clone())
        } else {
            None
        };
        Ok(Registered(keys, account_key))
    })?;
    let channel = approvals_channel();
    // Claude Code asks through the bridge in every mode but Auto, as Electron's host has it: a connector, a web fetch
    // or a command outside Accept edits' reach becomes a card instead of a refusal nobody sees.
    let bridge = if runtime_name == "claude" && mode != Mode::Auto {
        Some(
            crate::runtime::permission_bridge::Bridge::prepare(
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
    let browser_registration = if sealed {
        None
    } else {
        crate::browser_tools::Registration::prepare(&mut command, runtime_name, &id, &job.teammate.id, &folder, channel.clone(), flag.clone())?
    };
    if runtime_name == "claude" {
        let guard = crate::runtime::permission_bridge::bridge_executable()
            .and_then(|exe| crate::command_guard::write_claude_settings(&store.root().join("guard"), &exe))
            .map_err(|e| format!("Locust's command guard could not be set up: {e}"))?;
        command.args(claude_args_with_fork(
            mode,
            bridge.as_ref().map(|b| b.config()),
            browser_registration.as_ref().and_then(|r| r.claude_config()),
            &guard,
            model.as_deref(),
            job.effort.as_deref(),
            resume.as_deref(),
            is_side,
            &references,
            &claude_skills,
        )?);
    }
    let args: Vec<String> = command
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    // The mode and sandbox the turn really runs in, where it stands, and who started it, as Electron records them.
    let turn_workspace_id = crate::workspace_settings::workspace_id_for(&folder);
    let resolved_route_id = format!(
        "{runtime_name}:{}-account:{}",
        model.as_deref().unwrap_or("account-default"),
        effective_account.as_deref().unwrap_or("default")
    );
    let mut extra = json!({
        "mode": mode.as_str(),
        "sandbox": mode.sandbox(),
        "cwd": folder.to_string_lossy(),
        "workspaceId": turn_workspace_id,
        "resolvedRouteId": resolved_route_id,
    });
    if let Some(acc) = &effective_account {
        extra["account"] = json!(acc);
    }
    if let Some(by) = &origin.started_by {
        extra["startedBy"] = by.clone();
    }
    if let Some(switch) = &switched {
        extra["continuesFrom"] = switch.link.clone();
        // The stopped turn's checkpoint, written into its own ledger when that ledger is the native engine's;
        // Electron's ledgers are only read.
        if let Some(path) = &switch.native_prior {
            crate::ledger_write::append_checkpoint(path, &switch.checkpoint)
                .map_err(|_| "That conversation could not be reconciled, so it cannot be continued on another runtime.".to_string())?;
        }
    }
    let writer = LedgerWriter::create_runtime_with(
        &store,
        &id,
        &job.prompt,
        model.as_deref().unwrap_or("account-default"),
        &command.get_program().to_string_lossy(),
        runtime_name,
        &args,
        continues.as_ref(),
        &extra,
    )
    .map_err(|e| e.to_string())?;
    channel.register_teammate(&id, &job.teammate.id);
    if let Err(error) = crate::people::record_owner(&store, &id, &job.teammate.id) {
        eprintln!("The conversation's teammate was not recorded: {error}");
    }
    // The conversation keeps these references for its next turns (bind), with the folder this turn stood in.
    let attached_ids: Vec<String> = attached.iter().map(|i| i.id.clone()).collect();
    if let Err(error) = crate::references::bind(&store, &id, previous.as_deref(), &attached_ids, Some(&folder_text)) {
        eprintln!("The conversation's references were not kept: {error}");
    }
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
    // Only a turn the person sent from the chat box is carried on without a press: never one an app started, nor a
    // turn Locust carried on itself, so one empty plan cannot start a chain of them.
    let person_sent = origin.started_by.is_none();
    let memory_store = store.clone();
    let memory_delivery_store = store.clone();
    let delivery_mission = id.clone();
    let memory_who = job.teammate.id.clone();
    let memory_folder = turn.folder.clone();
    let memory_id = id.clone();
    let check_teammate = job.teammate.clone();
    let check_mode = mode;
    let check_runtime = runtime_owned.clone();
    let check_model = turn.model.clone();
    let check_effort = turn.effort.clone();
    let checkpoint_root = repository_root;
    let check_started_by_kind = origin
        .started_by
        .as_ref()
        .and_then(|v| v.get("kind"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    let check_started_by_fix = origin
        .started_by
        .as_ref()
        .and_then(|v| v.get("fix"))
        .and_then(Value::as_u64)
        .map(|n| n as u32);
    let check_apart = matches!(
        check_started_by_kind.as_deref(),
        Some("relay" | "side" | "compare" | "judge")
    );
    std::thread::Builder::new()
        .name("native-turn".into())
        .spawn(move || {
            // Keep ownership through the final memory write so shutdown/continuation cannot race it.
            let memory_completion = registered;
            let _browser_registration = browser_registration;
            let checkpoint = crate::checkpoints::Turn::begin(&memory_store, &memory_folder);
            run_turn(
                writer,
                command,
                turn,
                flag,
                Registered(Vec::new(), None),
                runtime_owned,
                channel,
                bridge,
                store,
                is_side,
                codex_home,
            );
            // The run's skill copies go with it.
            for copy in &skill_copies {
                let _ = std::fs::remove_dir_all(copy);
            }
            crate::skills::forget_roots(&memory_id);
            let (edited, seen_changes) = match checkpoint.finish_reporting_changes(&memory_store, &memory_id) {
                Ok((edited, seen)) => (Some(edited), seen),
                Err(error) => {
                    eprintln!("Undo checkpoint was not kept: {error}");
                    (None, None)
                }
            };
            // What a share carries as seen by the host: nothing for a read-only run, which cannot have changed files.
            let observed = if check_mode.sandbox() == "read-only" { None } else { seen_changes };
            let mut completed = false;
            if let Ok(path) = memory_store.write_path(&memory_id) {
                let transcript = ledger::read_transcript(path);
                completed = !transcript.failed
                    && !transcript.running
                    && !transcript.items.iter().any(
                        |item| matches!(item, ledger::Item::Note { text, .. } if text == "Stopped"),
                    );
                if completed && !is_side
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
                    // What the reply shares with a teammate goes to the workroom, to wait for that teammate's next run.
                    for said in crate::people::share_reply(&memory_store, &memory_who, &memory_id, &reply, observed.as_deref()) {
                        eprintln!("{said}");
                    }
                }
            }
            // Run the post-edit check and record its result before releasing the turn slot, so a reader
            // waiting on the slot sees the check, then release the slot so a fix turn can start.
            let checked_turn = crate::edit_check::CheckedTurn {
                mission_id: memory_id.clone(),
                workspace_id: Some(turn_workspace_id.clone()),
                teammate_id: Some(check_teammate.id.clone()),
                completed,
                apart: check_apart,
                started_by_kind: check_started_by_kind.clone(),
                started_by_fix: check_started_by_fix,
                sent_by_person: person_sent,
            };
            let check_result = if edited == Some(true) && check_mode.sandbox() != "read-only" {
                edit_check()
                    .after(
                        &memory_folder,
                        Some(&memory_id),
                        Some(&turn_workspace_id),
                        |ws| crate::edit_check::check_command_for(&memory_store, ws),
                        |_| {},
                        |cmd, cwd| {
                            crate::edit_check::run_check_command(
                                cmd,
                                cwd,
                                crate::edit_check::CHECK_TIMEOUT_MS,
                            )
                        },
                    )
                    .map(|result| {
                        if let Ok(path) = memory_store.write_path(&memory_id) {
                            let rec = crate::edit_check::edit_check_record_json(
                                &result,
                                &crate::ledger_write::now(),
                            );
                            if let Err(error) = crate::ledger_write::append_edit_check(&path, &rec)
                            {
                                eprintln!("Edit check was not recorded: {error}");
                            }
                        }
                        result
                    })
            } else {
                None
            };
            // A turn in the teammate's own copy is committed to its branch, however it ended, with a receipt.
            // A side question never commits to a teammate's branch.
            if !is_side && let Some(root) = &checkpoint_root {
                checkpoint_turn(&memory_store, &memory_id, root, &check_teammate, &checkpoint_prompt, &check_runtime, check_model.as_deref());
            }
            // The turn is no longer this window's: a carry-on or check-fix may now continue its conversation.
            drop(memory_completion);
            if person_sent {
                carry_on_without_asking(&memory_store, &memory_id);
            }
            if let Some(result) = check_result {
                run_check_fix(
                    &memory_store,
                    &checked_turn,
                    crate::edit_check::CheckFixResultOrNoEdits::Result(result),
                    &check_teammate,
                    &memory_folder,
                    check_mode,
                    &check_runtime,
                    check_model.as_deref(),
                    check_effort.as_deref(),
                );
            } else if check_started_by_kind.as_deref() == Some("check-fix")
                && edited == Some(false)
            {
                run_check_fix(
                    &memory_store,
                    &checked_turn,
                    crate::edit_check::CheckFixResultOrNoEdits::NoEdits,
                    &check_teammate,
                    &memory_folder,
                    check_mode,
                    &check_runtime,
                    check_model.as_deref(),
                    check_effort.as_deref(),
                );
            }
        })
        .map_err(|e| format!("Could not start the turn: {e}"))?;
    // The run is live: what its brief quoted is delivered (never before, so a start that fails consumes nothing).
    if !delivered.is_empty() {
        let ids: Vec<String> = delivered.iter().map(|m| m.message_id.clone()).collect();
        if let Err(error) = crate::workroom::Workroom::for_store(&memory_delivery_store).mark_delivered(&ids, &delivery_mission) {
            eprintln!("The teammate messages were not marked delivered, so they will be shown again: {error}");
        }
    }
    Ok(Started {
        conversation_id: anchor,
        mode: mode.as_str().into(),
    })
}

#[allow(clippy::too_many_arguments)]
fn run_check_fix(
    store: &Store,
    turn: &crate::edit_check::CheckedTurn,
    result: crate::edit_check::CheckFixResultOrNoEdits,
    teammate: &Teammate,
    folder: &Path,
    mode: Mode,
    runtime: &str,
    model: Option<&str>,
    effort: Option<&str>,
) {
    check_fix().on_checked(
        turn,
        result,
        |ws| crate::edit_check::most_fixes_for(store, ws),
        |t| {
            owns(&t.mission_id)
                || real_tip(&t.mission_id).ok().as_deref() != Some(t.mission_id.as_str())
        },
        |_| {
            if teammate.name.trim().is_empty() {
                "the teammate".into()
            } else {
                teammate.name.clone()
            }
        },
        |req| {
            let mut tm = teammate.clone();
            tm.runtime = Some(runtime.to_owned());
            tm.model = model.map(str::to_owned);
            let job = Send {
                teammate: tm,
                prompt: req.prompt,
                conversation: Some(req.follow_up_of),
                model: model.map(str::to_owned),
                effort: effort.map(str::to_owned),
            };
            let origin = Origin {
                mode: Some(mode),
                folder: Some(folder.to_path_buf()),
                started_by: Some(req.started_by.to_json()),
                runtime: Some(runtime.to_owned()),
                account: None,
                reference_ids: None,
                slot: None,
            };
            match crate::engine_api::api().send(job, origin) {
                Ok(started) => {
                    let tip = real_tip(&started.conversation_id)
                        .unwrap_or(started.conversation_id);
                    crate::edit_check::FixStartOutcome::Ok { mission_id: tip }
                }
                Err(message) => {
                    if message.contains("already running") {
                        crate::edit_check::FixStartOutcome::Busy
                    } else {
                        crate::edit_check::FixStartOutcome::Error { message }
                    }
                }
            }
        },
        |mission_id, message, level| {
            if let Ok(path) = store.write_path(mission_id) {
                let _ = crate::ledger_write::append_note_with_level(
                    &path,
                    level,
                    "host.check_fix",
                    message,
                );
            }
        },
    );
}

/// One short read-only run for a panel ("Write it from the changes"), as Electron's panel runs are: the teammate's own
/// agent and model, always Ask, kept in `panel-runs` beside the native ledgers so it is never listed as a
/// conversation, stopped after three minutes. Answers with the run's final reply. Blocks: call it off the UI thread.
pub fn run_once(teammate: &Teammate, folder: &Path, prompt: &str) -> Result<String, String> {
    if !supported(teammate) {
        return Err(format!("{} runs in Locust for now.", teammate.name));
    }
    let store = store().ok_or("Native data folder could not be determined.")?;
    let panel = Store::new(store.root().join("panel-runs"), store.electron().to_path_buf()).map_err(|e| e.to_string())?;
    let runtime_name = teammate.runtime.as_deref().unwrap_or("codex");
    let launch_env = account_launch_env(&store, runtime_name, teammate.account.as_deref())?;
    let model = teammate.model.clone().filter(|m| !m.is_empty() && m != "account-default");
    let session = if runtime_name == "copilot" { Some(uuid_v4().map_err(|e| e.to_string())?) } else { None };
    let mut command = command_for_turn(runtime_name, folder, model.as_deref(), Mode::Ask, None, prompt, None, session.as_deref(), &[], None)
        .map_err(|e| e.to_string())?;
    command.current_dir(folder);
    for (key, value) in &launch_env {
        command.env(key, value);
    }
    let codex_home = launch_env
        .iter()
        .find(|(k, _)| *k == "CODEX_HOME")
        .map(|(_, v)| PathBuf::from(v));
    let id = new_id();
    let args: Vec<String> = command.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    let writer = LedgerWriter::create_runtime(
        &panel,
        &id,
        prompt,
        model.as_deref().unwrap_or("account-default"),
        &command.get_program().to_string_lossy(),
        runtime_name,
        &args,
        None,
    )
    .map_err(|e| e.to_string())?;
    let turn = Turn {
        prompt: prompt.to_owned(),
        folder: folder.to_owned(),
        model,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: id.clone(),
        run_id: format!("run_{id}"),
    };
    let flag = Arc::new(AtomicBool::new(false));
    let done = Arc::new(AtomicBool::new(false));
    {
        // Three minutes, as Electron waits, then the run is stopped (never retried).
        let (flag, done) = (flag.clone(), done.clone());
        std::thread::spawn(move || {
            let start = std::time::Instant::now();
            while !done.load(Ordering::Relaxed) && start.elapsed() < Duration::from_secs(180) {
                std::thread::sleep(Duration::from_millis(200));
            }
            if !done.load(Ordering::Relaxed) {
                flag.store(true, Ordering::Relaxed);
            }
        });
    }
    run_turn(
        writer,
        command,
        turn,
        flag.clone(),
        Registered(Vec::new(), None),
        runtime_name.to_owned(),
        approvals_channel(),
        None,
        panel.clone(),
        false,
        codex_home,
    );
    done.store(true, Ordering::Relaxed);
    if flag.load(Ordering::Relaxed) {
        return Err("It took too long to write.".into());
    }
    let transcript = ledger::read_transcript(panel.write_path(&id).map_err(|e| e.to_string())?);
    transcript
        .items
        .iter()
        .filter_map(|item| match item {
            ledger::Item::Assistant { text, .. } => Some(text.clone()),
            _ => None,
        })
        .next_back()
        .filter(|t| !t.trim().is_empty())
        .ok_or_else(|| "It did not answer.".to_owned())
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

#[allow(clippy::too_many_arguments)]
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
    fork: bool,
    codex_home: Option<PathBuf>,
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
            Ok(r) => r.with_cancel(flag.clone()).with_fork(fork),
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
        #[cfg(any(test, feature = "test-hooks"))]
        let using_hook = crate::invariants::lock(&hooks::STATE).1.is_some();
        #[cfg(not(any(test, feature = "test-hooks")))]
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
    // No teammate ends the person's browser, Codex too (0.720): the guard as a hook in this thread's config.
    let guard = crate::command_guard::codex_config_for_home(&turn.folder, codex_home.as_deref());
    let mut runtime = match Codex::spawn(&mut command) {
        Ok(r) => r.with_cancel(flag.clone()).with_thread_config(guard).with_fork(fork),
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

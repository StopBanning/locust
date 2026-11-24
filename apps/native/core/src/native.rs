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
            | Some("apple")
    ) && t.runs_in != IN_LOCUST
}

// ---- test hooks: one place, compiled only into tests (and the window's tests, through the test-hooks feature) -------------------------------------------------------------
#[cfg(any(test, feature = "test-hooks"))]
pub mod hooks {
    use super::*;
    pub static LOCK: Mutex<()> = Mutex::new(());
    pub static STATE: Mutex<(Option<Store>, Option<(PathBuf, Vec<String>)>)> =
        Mutex::new((None, None));
    /// While set, a turn whose end is recorded is held before the engine lets go of it, as its disk observation and
    /// share can hold it (the-next-turn-is-not-refused-while-the-last-winds-down).
    pub static HOLD_WIND_DOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    /// Fired once when `record_disk_observation` is about to take the after-snapshot (`qa-0365-concurrency.test.ts`).
    pub static ON_AFTER_DISK: Mutex<Option<Box<dyn FnOnce() + std::marker::Send>>> = Mutex::new(None);
    /// What a start's readiness is asked through while a test holds the engine. None: the fake stands in for the
    /// runtime, and nothing is asked.
    pub static READINESS: Mutex<Option<Arc<crate::agents::StartReadiness>>> = Mutex::new(None);
    /// What a `/context` run answers while set, in place of Claude Code (Electron's `LOCUST_CONTEXT_FIXTURE_PATH`).
    pub static CLAUDE_CONTEXT_RUN: Mutex<Option<Arc<crate::claude_context::Run>>> = Mutex::new(None);
    /// Serialises tests that point the engine at a temporary store and a fake `codex`.
    pub struct Guard {
        _lock: std::sync::MutexGuard<'static, ()>,
    }
    pub fn install(store: Store, command: Option<(PathBuf, Vec<String>)>) -> Guard {
        let guard = Guard {
            _lock: LOCK.lock().unwrap_or_else(|e| e.into_inner()),
        };
        // Whatever the last holder left still ending is done before this test's state is set.
        settle_turn_threads();
        clear_poison();
        super::reset_relay_state();
        crate::runtime::opencode_config::FAIL_V2_GUARD.with(|f| f.set(false));
        *crate::invariants::lock(&STATE) = (Some(store), command);
        *crate::invariants::lock(&READINESS) = None;
        *crate::invariants::lock(&ON_AFTER_DISK) = None;
        *crate::invariants::lock(&CLAUDE_CONTEXT_RUN) = None;
        guard
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            // Tests that use the engine hold LOCK one at a time, so every turn still running is this test's: stop it
            // and wait for its ending, so no turn outlives its test into the next one.
            super::with_running(|m| m.values().for_each(|f| f.store(true, std::sync::atomic::Ordering::Relaxed)));
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
            while super::with_running(|m| !m.is_empty()) && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            // A turn leaves the running list before its thread has done its ending: the relay's hand-off and its
            // cap, the rooms, Scout, carry-on. Wait for that too, stopping any turn the ending starts, or it acts on
            // the next test's store and makes a relay in the next test's state.
            settle_turn_threads();
            clear_poison();
            super::reset_relay_state();
            crate::runtime::opencode_config::FAIL_V2_GUARD.with(|f| f.set(false));
            *crate::invariants::lock(&STATE) = (None, None);
            *crate::invariants::lock(&READINESS) = None;
            *crate::invariants::lock(&ON_AFTER_DISK) = None;
        }
    }
    /// An empty saved-rules store for a test that asks for approval and expects a card. It holds LOCK as `install` does,
    /// so a rules test's saved allow can never answer this test's request: on 24 Nov a rules test's "git status" allow,
    /// read through the shared hook, left approval_declined_tests waiting forever for its card in a whole-package run.
    pub struct NoSavedRules {
        _guard: Guard,
        root: PathBuf,
    }
    impl Drop for NoSavedRules {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    pub fn no_saved_rules() -> NoSavedRules {
        let root = std::env::temp_dir().join(format!("locust-no-rules-{}", crate::invariants::unique_nanos()));
        std::fs::create_dir_all(root.join("electron")).unwrap();
        let store = Store::new(root.join("native"), root.join("electron")).unwrap();
        NoSavedRules { _guard: install(store, None), root }
    }
    /// Every turn thread, its ending included, is gone: turns still running are told to stop, again each time, since an
    /// ending can start another (a relay's next hop). Bounded, as the load limit bounds a test's waits; past it the
    /// count is said rather than hidden, and the guard goes on.
    fn settle_turn_threads() {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while super::TURN_THREADS.load(std::sync::atomic::Ordering::SeqCst) > 0 {
            super::with_running(|m| m.values().for_each(|f| f.store(true, std::sync::atomic::Ordering::Relaxed)));
            if std::time::Instant::now() >= deadline {
                eprintln!(
                    "{} turn thread(s) still ending after 60 s; the next test may see them",
                    super::TURN_THREADS.load(std::sync::atomic::Ordering::SeqCst)
                );
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// A test that failed while holding one of the engine's locks leaves it poisoned. Outside tests that is a bug and
    /// stops the program (`invariants::lock`); between tests it must not turn one failure into a second panic during
    /// cleanup, which aborts the whole test binary and hides every other result (found 14 Nov: one stuck relay test
    /// took the core suite down with it). The state is reset right after, so nothing half-written survives.
    fn clear_poison() {
        super::RELAY.clear_poison();
        super::ACTIVE_TEAMMATES.clear_poison();
        super::STEER_CAPABLE.clear_poison();
        super::STEER_QUEUES.clear_poison();
        super::RELAY_NOTICES.clear_poison();
        super::RELAY_STARTING.clear_poison();
        super::ROOM_NOTICES.clear_poison();
        STATE.clear_poison();
        READINESS.clear_poison();
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
    sealed: bool,
    kilo_home: Option<&Path>,
) -> io::Result<Command> {
    #[cfg(any(test, feature = "test-hooks"))]
    {
        let hooked = crate::invariants::lock(&hooks::STATE).1.clone();
        if let Some((program, args)) = hooked {
            if runtime == "opencode"
                && args.iter().any(|a| a == "acp")
                && crate::runtime::opencode_config::FAIL_V2_GUARD.with(std::cell::Cell::get)
            {
                let store = store()
                    .ok_or_else(|| io::Error::other("Native data folder could not be determined"))?;
                let mut probe_cmd = Command::new(&program);
                crate::runtime::opencode_config::configure_with_seal(&mut probe_cmd, mode, &store, true, sealed)?;
            }
            let mut c = Command::new(program);
            c.args(args);
            return Ok(c);
        }
    }
    match runtime {
        "claude" => crate::runtime::claude::bare_command(folder),
        "cursor" => crate::runtime::cursor::command(folder, mode, model, resume, skills),
        "copilot" => crate::runtime::copilot::command(
            folder, prompt, mode, model, effort, resume, session_id, references, skills,
        ),
        "antigravity" => crate::runtime::antigravity::command(folder, mode, model, effort, resume, references, skills),
        "muse" => crate::runtime::muse::command(folder, mode, model, effort, resume),
        "apple" => crate::runtime::apple::command(mode),
        "acp" => {
            let profile = store()
                .filter(|s| s.root().join(crate::acp_agents::ACP_AGENTS_FILE).is_file())
                .map(|s| s.root().to_path_buf())
                .or_else(electron_dir)
                .ok_or_else(|| io::Error::other("Locust profile not found"))?;
            crate::runtime::acp::command(&profile, folder, model)
        }
        "opencode" => {
            let store = store()
                .ok_or_else(|| io::Error::other("Native data folder could not be determined"))?;
            let mut c = crate::runtime::opencode::command_with_seal(folder, mode, &store, sealed)?;
            // OpenCode 1's config gets the reference folders a read-only run may read (openCodeModeEnv).
            let config = c
                .get_envs()
                .find(|(k, _)| *k == "OPENCODE_CONFIG_CONTENT")
                .and_then(|(_, v)| v.and_then(|v| v.to_str()).and_then(|v| serde_json::from_str::<Value>(v).ok()));
            if let Some(config) = config.filter(|c| !sealed && c["permission"].is_object() && (!references.is_empty() || skills.is_some())) {
                // The person's own skill paths go beside the run's folder (opencode-skill-paths.ts, 0.816); settings
                // OpenCode could not read refuse the run before it starts, naming the file.
                let person = match skills {
                    Some(_) => crate::opencode_skill_paths::for_this_process(folder).map_err(io::Error::other)?,
                    None => Vec::new(),
                };
                let config = crate::runtime::opencode_config::with_references(config, mode, references);
                c.env("OPENCODE_CONFIG_CONTENT", crate::runtime::opencode_config::with_skills(config, skills, &person).to_string());
            }
            Ok(c)
        }
        "kilo" => {
            let store = store()
                .ok_or_else(|| io::Error::other("Native data folder could not be determined"))?;
            // A run that is no conversation's turn (a panel's) has a Kilo folder of its own, as Electron gives it one in
            // the temp folder.
            let own;
            let home = match kilo_home {
                Some(home) => home,
                None => {
                    own = std::env::temp_dir().join(format!("locust-kilo-{}", new_id()));
                    std::fs::create_dir_all(&own)?;
                    &own
                }
            };
            crate::runtime::kilo::command(folder, &store, home)
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
const RUNTIMES: [&str; 11] = [
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
    "apple",
];
const MODES: [&str; 5] = ["ask", "accept-edits", "approve-each", "plan", "auto"];

fn control(c: char) -> bool {
    matches!(c, '\u{0}'..='\u{1f}' | '\u{7f}')
}
/// teammate-store.ts: safeId
pub(crate) fn safe_id(v: &Value) -> Option<&str> {
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
                        | "apple"
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
    // Dropped, but never silently (LIVE2 #16).
    crate::roster_dropped::say(file["teammates"].as_array().map_or(0, |a| a.len().min(MAX_TEAMMATES)), out.len());
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
/// seeded parts when the record keeps them, the bot they picked (including optional screen flag), and any pet rig
/// (`codex-buddy`) they picked.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FaceLook {
    pub hue: String,
    /// headwear, accessory, mouth (avatar.ts AvatarSpec), when the record has a valid avatar.
    pub parts: Option<[u8; 3]>,
    /// The picked bot's shape and face (`eyes` or `mouth`).
    pub bot: Option<(String, String)>,
    /// Explicit screen preference (`true` / `false`) stored on `avatar.bot.screen`.
    pub bot_screen: Option<bool>,
    /// Picked pet rig (`avatar.pet`), e.g. Codex Buddy (`codex-buddy`).
    pub pet: Option<crate::bot_spec::PetRef>,
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
            if avatar["bot"].get("screen").is_some_and(|v| !v.is_boolean()) {
                return None;
            }
            let shape = avatar["bot"]["shape"].as_str()?;
            let face = avatar["bot"]["face"].as_str().filter(|f| matches!(*f, "eyes" | "mouth"))?;
            let ok = !shape.is_empty() && shape.len() <= 24 && shape.bytes().all(|b| b.is_ascii_lowercase());
            ok.then(|| (shape.to_owned(), face.to_owned()))
        })();
        let bot_screen = bot.as_ref().and_then(|_| avatar["bot"]["screen"].as_bool());
        let pet = (|| {
            let source = match avatar["pet"]["source"].as_str()? {
                "gallery" => crate::bot_spec::PetSource::Gallery,
                "codex" => crate::bot_spec::PetSource::Codex,
                "bundled" => crate::bot_spec::PetSource::Bundled,
                _ => return None,
            };
            let pet_id = avatar["pet"]["id"].as_str()?;
            if !crate::bot_spec::is_pet_id(pet_id) {
                return None;
            }
            Some(crate::bot_spec::PetRef {
                source,
                id: pet_id.to_owned(),
                screen: avatar["pet"]["screen"].as_bool(),
            })
        })();
        out.insert(
            id.to_owned(),
            FaceLook {
                hue: hue.to_owned(),
                parts,
                bot,
                bot_screen,
                pet,
            },
        );
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

/// Snapshots the folder after a write-capable turn and records `observed_edit` events (or a shared-folder notice).
/// Answers the paths the turn changed, nothing when it changed none or the folder could not be read.
fn record_disk_observation(
    store: &Store,
    mission_id: &str,
    folder: &Path,
    disk_before: Option<crate::disk_observation::WorkspaceSnapshot>,
    checkpoint: &crate::checkpoints::Turn,
) -> Vec<String> {
    let Some(before) = disk_before else { return Vec::new() };
    #[cfg(any(test, feature = "test-hooks"))]
    {
        let hook = crate::invariants::lock(&hooks::ON_AFTER_DISK).take();
        if let Some(hook) = hook {
            hook();
        }
    }
    let before_lookup = |rel: &str| checkpoint.before_text(store, rel);
    let host = crate::disk_observation::DefaultDiskHost::with_before_text(&before_lookup);
    let Some(after) = crate::disk_observation::snapshot_workspace(folder, &host) else {
        return Vec::new();
    };
    let changed = crate::disk_observation::changed_paths(&before, &after);
    if changed.is_empty() {
        return Vec::new();
    }
    let Ok(path) = store.write_path(mission_id) else {
        return Vec::new();
    };
    let existing_events: Vec<Value> = turn_records(mission_id).map(|t| t.events).unwrap_or_default();
    let shared = checkpoint.is_shared();
    let _ = crate::ledger_write::append_events(&path, |run, mission, adapter, next_seq, at| {
        if shared {
            vec![crate::disk_observation::shared_tree_notice(
                run, mission, adapter, next_seq, at, &changed,
            )]
        } else {
            let unreported: std::collections::BTreeSet<String> =
                crate::disk_observation::unreported_paths(&changed, &existing_events)
                    .into_iter()
                    .collect();
            let reported: std::collections::BTreeSet<String> = changed
                .iter()
                .filter(|p| !unreported.contains(*p))
                .cloned()
                .collect();
            let patches = crate::disk_observation::observed_patches(
                folder,
                &after,
                &changed,
                &host,
                Some(&before),
            );
            crate::disk_observation::observed_edit_events(
                run, mission, adapter, next_seq, &changed, at, &patches, &reported,
            )
        }
    });
    changed
}

/// Commit a turn in a teammate's own copy on its branch, and record the receipt in the turn's stream: what was saved,
/// what was left out for being too big, or why nothing could be saved (the files then stay in the copy).
fn checkpoint_turn(store: &Store, mission: &str, root: &Path, teammate: &Teammate, prompt: &str, runtime: &str, model: Option<&str>) {
    use crate::worktrees::{CHECKPOINT_CODE, Manager, checkpoint_message, checkpoint_sentence, turn_outcome_of};
    let Ok(path) = store.write_path(mission) else { return };
    let transcript = ledger::read_transcript(path.clone());
    let event_types: Vec<String> = std::fs::read_to_string(&path)
        .ok()
        .map(|text| {
            text.lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .filter_map(|v| v.get("event").and_then(|e| e.get("type")).and_then(Value::as_str).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let kind_refs: Vec<&str> = event_types.iter().map(String::as_str).collect();
    let outcome = turn_outcome_of(&kind_refs);
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

/// Looks up a known folder path by workspace ID in native's `folders.json`, falling back read-only to Electron's.
fn known_folder_of(store: &Store, electron: &Path, id: &str) -> Result<Option<String>, String> {
    if let Some(path) = crate::folders::path_of(store, id)? {
        return Ok(Some(path));
    }
    let electron_file = electron.join(crate::folders::FOLDERS_FILE);
    if let Ok(text) = std::fs::read_to_string(&electron_file)
        && let Ok(val) = serde_json::from_str::<Value>(&text)
        && let Some(arr) = val.get("folders").and_then(|v| v.as_array())
    {
        for entry in arr {
            if entry.get("id").and_then(|v| v.as_str()) == Some(id)
                && let Some(path) = entry.get("path").and_then(|v| v.as_str())
                && Path::new(path).is_absolute()
            {
                return Ok(Some(path.to_owned()));
            }
        }
    }
    Ok(None)
}

/// Resolves the working folder for a turn: continuing turns run in the conversation's own folder rather than the
/// window's current folder, and refuse when that folder is unknown or missing.
fn resolve_run_folder(
    store: &Store,
    electron: &Path,
    teammate: &Teammate,
    runtime_name: &str,
    origin: &Origin,
    continues_mission_id: Option<&str>,
) -> Result<(PathBuf, Option<PathBuf>), String> {
    if let Some(slot_cwd) = origin.slot.as_ref().and_then(|s| s.cwd.clone()) {
        return Ok((slot_cwd, None));
    }
    let current = match &origin.folder {
        Some(named) => named.clone(),
        None => folder_for(teammate, electron)?,
    };
    let mut run_folder = current.clone();
    if let Some(earlier_id) = continues_mission_id
        && let Some(earlier) = turn_records(earlier_id)
        && let Some(earlier_ws) = earlier.metadata["workspaceId"].as_str()
        // "native" is the placeholder a record written without its folder carries (older Rust builds, the terminal's
        // records): it names no folder, so it is never a reason to refuse; the turn runs where it is asked to.
        && earlier_ws != "native"
        && earlier_ws != crate::workspace_settings::workspace_id_for(&current)
    {
        let Some(path) = known_folder_of(store, electron, earlier_ws)? else {
            return Err("Locust does not know which folder this conversation ran in, so it will not continue it somewhere else. Start a new conversation instead.".into());
        };
        if !Path::new(&path).is_dir() {
            return Err(format!("This conversation ran in {path}, and that folder is not there any more."));
        }
        run_folder = PathBuf::from(path);
    }
    if origin.slot.is_none() {
        let _ = crate::folders::use_folder(store, &run_folder, &chrono::Utc::now().to_rfc3339());
    }
    own_branch_folder(store, teammate, runtime_name, run_folder)
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
    /// How many conversations the records keep, said only when that is more than are listed (`totalMissions`).
    pub total: Option<usize>,
    /// What the records say about their own damage (mission-history's `unreadableCount`, the listed ones' issues).
    pub damage: ListDamage,
}

/// The records' damage under the list (Screens.tsx `ledgerDamageWords`): two facts, never merged into one number.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListDamage {
    /// Listed conversations kept up to a bad record.
    pub incomplete: usize,
    /// Files no conversation could be read from, counted over every file, not only the listed ones.
    pub unreadable: usize,
    /// A records folder that is there but could not be listed (`HISTORY_UNAVAILABLE`).
    pub folder_unreadable: bool,
}

impl ListDamage {
    /// Screens.tsx's words for it, or nothing when the records are whole.
    pub fn words(&self) -> Option<String> {
        if self.folder_unreadable {
            return Some("the records could not be read".into());
        }
        let mut parts = Vec::new();
        if self.incomplete > 0 {
            parts.push(format!("{} with an incomplete receipt", self.incomplete));
        }
        if self.unreadable > 0 {
            parts.push(format!("{} {} could not be read", self.unreadable, if self.unreadable == 1 { "file" } else { "files" }));
        }
        (!parts.is_empty()).then(|| parts.join(" · "))
    }
}

/// The most conversations the window lists, newest first (mission-history.ts `MAX_LISTED_MISSIONS`): far past the old
/// 300, and a history longer than this says how much of it is listed.
pub const MAX_LISTED_MISSIONS: usize = 2_000;

/// What a list capped at `listed` says of a history of `total` (Screens.tsx), or nothing when all of it is listed.
pub fn listed_words(listed: usize, total: Option<usize>) -> Option<String> {
    let total = total.filter(|t| *t > listed)?;
    let thousands = |n: usize| {
        let digits = n.to_string();
        let mut out = String::new();
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                out.push(',');
            }
            out.push(c);
        }
        out
    };
    Some(format!("Only the newest {} of {} turns are listed", thousands(listed), thousands(total)))
}
/// Newest first, each conversation once, the first `limit`; and the records' whole count (`whole`, asked only when
/// the list is full) when that is more than are listed.
pub(crate) fn capped(mut all: Vec<ledger::Summary>, limit: usize, whole: impl FnOnce() -> usize) -> (Vec<ledger::Summary>, Option<usize>) {
    all.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| a.mission_id.cmp(&b.mission_id)));
    all.dedup_by(|a, b| a.mission_id == b.mission_id);
    let total = (all.len() >= limit).then(whole);
    all.truncate(limit);
    let total = total.filter(|t| *t > all.len());
    (all, total)
}

/// Electron's read-only list merged with the native one, newest first. None when neither folder could be listed.
pub fn list_recent(limit: usize, native_engine: bool) -> Option<Listed> {
    let native_dir = store().and_then(|s| s.ledger_dir().ok());
    let mut damage = ListDamage::default();
    let mut incomplete = HashSet::new();
    let mut cut_tail_only = HashSet::new();
    // A folder not there is empty; one there that cannot be listed is said, not taken for empty.
    let mut read = |dir: &Path| match crate::history_cache::global().list_recent_with_damage(dir, limit) {
        Ok((summaries, found)) => {
            damage.unreadable += found.unreadable;
            incomplete.extend(found.incomplete);
            cut_tail_only.extend(found.cut_tail_only);
            Some(summaries)
        }
        Err(error) => {
            damage.folder_unreadable |= error.kind() != std::io::ErrorKind::NotFound;
            None
        }
    };
    let theirs = electron_ledger_dir().and_then(|d| read(&d));
    // A run whose answer belongs to an Electron panel (a commit message, a walkthrough) is never a conversation.
    let panel_runs = electron_dir().map(|d| crate::panel_runs::PanelRuns::of_electron(&d).ids()).unwrap_or_default();
    let theirs = theirs.map(|t| t.into_iter().filter(|s| !panel_runs.contains(&s.mission_id)).collect::<Vec<_>>());
    let mut ours = native_dir.as_ref().and_then(|d| read(d));
    // A native folder not made yet is simply empty; only the native engine reads that as "nothing to show".
    if ours.is_none() && native_engine && native_dir.as_ref().is_some_and(|d| !d.exists()) {
        ours = Some(Vec::new());
    }
    // A records folder there that could not be listed is said ("the records could not be read"), never None's fallback.
    if theirs.is_none() && ours.is_none() && !damage.folder_unreadable {
        return None;
    }
    let native: HashSet<String> = ours
        .iter()
        .flatten()
        .map(|m| m.mission_id.clone())
        .collect();
    let all: Vec<ledger::Summary> = theirs
        .into_iter()
        .flatten()
        .chain(ours.into_iter().flatten())
        .collect();
    let (mut all, total) = capped(all, limit, || {
        let mut ids: HashSet<String> = HashSet::new();
        for dir in [electron_ledger_dir(), native_dir.clone()].into_iter().flatten() {
            ids.extend(crate::history_cache::global().mission_ids(&dir).unwrap_or_default());
        }
        ids.retain(|id| !panel_runs.contains(id));
        ids.len()
    });
    // A native turn whose process is gone (Locust quit or crashed before its last record) is not running, whatever the
    // file's age says.
    for summary in all.iter_mut().filter(|s| s.running) {
        if orphaned(&summary.mission_id) {
            summary.running = false;
        }
    }
    // A turn this window is writing has a last line with no end only for the moment of a write: the record arriving.
    damage.incomplete = all
        .iter()
        .filter(|s| incomplete.contains(&s.mission_id) && !(cut_tail_only.contains(&s.mission_id) && owns(&s.mission_id)))
        .count();
    Some(Listed {
        summaries: all,
        native,
        total,
        damage,
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

/// Like `session`, also attaching the conversation's folder and checkpoint store so finished turns carry per-turn diffs.
pub fn session_for(dir: PathBuf, id: String, folder: Option<&str>) -> ledger::Session {
    let mut s = session(dir, id).with_folder(folder);
    if let Some(ws) = folder.filter(|f| !f.is_empty()).map(PathBuf::from)
        && let Some(st) = store()
    {
        s = s.with_checkpoints(st, ws);
    }
    s
}

// ---- resume point: which Codex thread a follow-up reopens ---------------------------------------------------------
/// App.tsx coldStart (0.496): whether a turn started cold is the host's to say. It did when it is the person's reply on
/// the same agent and its follow-up names no session to resume (`followsUp.runtimeThreadId` absent): the turn before
/// failed before its runtime started, the mode changed since, or its session was not there. An edit starts cold too
/// but says its own note; a switch to another agent or account draws its divider; a turn Locust or a teammate started
/// (carry-on, a fix, a relay's hop, a side question, a comparison's column) is not the person's reply.
pub fn started_cold(metadata: &Value) -> bool {
    let link = &metadata["continuesFrom"];
    link["missionId"].as_str().is_some_and(|m| !m.is_empty())
        && link["reason"] == "follow-up"
        && link["runtimeThreadId"].as_str().is_none_or(str::is_empty)
        && link["edited"] != true
        && metadata["startedBy"].is_null()
        && metadata["relay"].is_null()
}

/// [`started_cold`] of a turn, from its first record alone (`mission.created`), so the window can ask it once per tip.
pub fn turn_started_cold(mission: &str) -> bool {
    use std::io::{BufRead, Read};
    let Some(file) = find_ledger(mission).and_then(|path| std::fs::File::open(path).ok()) else { return false };
    let mut first = String::new();
    if std::io::BufReader::new(file.take(1 << 20)).read_line(&mut first).is_err() {
        return false;
    }
    serde_json::from_str::<Value>(&first)
        .ok()
        .filter(|record| record["recordType"] == "mission.created")
        .is_some_and(|record| started_cold(&record["metadata"]))
}

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
    /// The mode its header recorded; None for a header that names none (never read as a change of mode).
    pub mode: Option<String>,
    /// The CLI version its header recorded (`cliVersion`).
    pub cli_version: Option<String>,
    /// The command arguments its header recorded (`command.args`).
    pub command_args: Option<Vec<String>>,
    /// Its session compacted during the turn (brief_sessions::compacted_during): it holds a summary, not the brief.
    pub compacted: bool,
}

/// codex-mission.ts: OpenCode follow-up guard against loading a foreign permission history across a V1/V2
/// protocol switch or from an imported/non-ACP session.
pub fn opencode_follow_up_needs_fresh_session(
    prior_mode: Option<&str>,
    prior_cli_version: Option<&str>,
    prior_command_args: Option<&[String]>,
    current_is_acp: bool,
) -> bool {
    let prior_is_acp = prior_cli_version.is_some_and(crate::runtime::opencode::version_uses_acp);
    let protocol_changed = prior_is_acp != current_is_acp;
    let unverified_acp_session = current_is_acp
        && (prior_mode.is_none()
            || prior_command_args
                .is_none_or(|args| args.last().map(String::as_str) != Some("acp")));
    protocol_changed || unverified_acp_session
}
/// One `/context` reader per store (index.ts's `claudeContextReader`): each conversation's last Claude launch is kept in
/// the store, and a read is one lazy, silent `/context` run after the turn has ended.
static CLAUDE_CONTEXT: Mutex<Option<(PathBuf, Arc<crate::claude_context::ClaudeContextReader>)>> = Mutex::new(None);

/// Turns starting that continue a conversation, before they are running (codex-mission.ts `contextStarting`): each by
/// the conversation it was sent on, the key `RUNNING` takes once it registers.
static CONTEXT_STARTING: Mutex<Vec<(u64, String)>> = Mutex::new(Vec::new());

/// One turn starting on a conversation, held until the start returns, by which time it is running or has failed.
struct ContextStarting(u64);
impl ContextStarting {
    fn on(conversation: &str) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let key = NEXT.fetch_add(1, Ordering::SeqCst);
        crate::invariants::lock(&CONTEXT_STARTING).push((key, conversation.to_owned()));
        Self(key)
    }
}
impl Drop for ContextStarting {
    fn drop(&mut self) {
        crate::invariants::lock(&CONTEXT_STARTING).retain(|(key, _)| *key != self.0);
    }
}

/// A turn's `mission.created` metadata, read from the top of its ledger only.
fn turn_header(id: &str) -> Option<Value> {
    use std::io::BufRead;
    let file = std::fs::File::open(find_ledger(id)?).ok()?;
    std::io::BufReader::new(file)
        .lines()
        .take(64)
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
        .find(|record| record["recordType"] == "mission.created")
        .map(|record| record["metadata"].clone())
}

/// index.ts's ancestors: the newest turn and every turn it continues, newest first, at most 512.
fn context_ancestors(tip: &str, header: &Value) -> Vec<String> {
    let mut ancestors = vec![tip.to_owned()];
    let mut prior = header["continuesFrom"]["missionId"].as_str().map(str::to_owned);
    while let Some(id) = prior.take() {
        if ancestors.contains(&id) || ancestors.len() >= 512 {
            break;
        }
        prior = turn_header(&id).and_then(|h| h["continuesFrom"]["missionId"].as_str().map(str::to_owned));
        ancestors.push(id);
    }
    ancestors
}

/// codex-mission.ts `contextBusy(ancestors)`: a turn running in this process, or starting before it has a ledger row,
/// on the newest turn or any turn it continues. The ancestors are walked only while something runs or starts.
fn context_busy(tip: &str, header: &Value) -> bool {
    let mut live: Vec<String> = crate::invariants::lock(&CONTEXT_STARTING).iter().map(|(_, id)| id.clone()).collect();
    live.extend(with_running(|m| m.keys().cloned().collect::<Vec<_>>()));
    !live.is_empty() && context_ancestors(tip, header).iter().any(|id| live.contains(id))
}

/// A turn sent to the cloud computer (cloud-turns.ts `of`, 0.761): its session is on the codespace, not here.
fn cloud_turn(mission: &str) -> bool {
    let Some(store) = store() else { return false };
    [store.root(), store.electron()]
        .iter()
        .any(|dir| crate::cloud_turns::recorded_of(&dir.join("cloud-turns.json"), mission).is_some())
}

/// index.ts's `resolve` for the `/context` reader: the conversation's newest turn and its Claude session; busy while it
/// or any turn it continues is running or starting; and, for a turn whose launch was never kept, the launch its
/// recorded command gives (`recordedClaudeContextLaunch`). Nothing for another agent or a cloud turn.
fn claude_context_turn(conversation: &str) -> Option<crate::claude_context::ClaudeContextTurn> {
    let tip = real_tip(conversation).ok()?;
    let prior = prior_of(&tip).ok()?;
    if prior.runtime != "claude" || cloud_turn(&tip) {
        return None;
    }
    let session_id = prior.thread.clone()?;
    let header = turn_header(&tip)?;
    let busy = prior.running || context_busy(&tip, &header);
    let command = &header["command"];
    let args: Option<Vec<String>> = command["args"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect());
    let recorded = command["executablePath"].as_str().zip(args.as_deref());
    let cwd = header["cwd"].as_str().or_else(|| command["cwd"].as_str()).filter(|c| !c.is_empty());
    let sandbox = header["sandbox"].as_str().filter(|s| matches!(*s, "read-only" | "workspace-write" | "full-access"));
    let fallback = sandbox.and_then(|sandbox| crate::claude_context::recorded_claude_context_launch(recorded, cwd, sandbox));
    Some(crate::claude_context::ClaudeContextTurn { mission_id: tip, session_id, busy, account: prior.account, fallback })
}

fn claude_context_reader() -> Option<Arc<crate::claude_context::ClaudeContextReader>> {
    use crate::claude_context::{ClaudeContextReader, ClaudeContextTurn, ContextResources, ReaderOptions, prepare_claude_context_launch};
    let store = store()?;
    let mut held = crate::invariants::lock(&CLAUDE_CONTEXT);
    if let Some((root, reader)) = held.as_ref()
        && root == store.root()
    {
        return Some(reader.clone());
    }
    let resolve = claude_context_turn;
    let prepare_store = store.clone();
    let prepare = move |launch: &crate::claude_context::ClaudeContextLaunch, turn: &ClaudeContextTurn| {
        let store = &prepare_store;
        let env = account_launch_env(store, "claude", turn.account.as_deref())?.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
        let permission = |run: &str, mission: &str, cwd: &str| -> Result<(String, crate::claude_context::Held), String> {
            let bridge = crate::runtime::permission_bridge::Bridge::prepare(approvals_channel(), mission, run, cwd, Arc::new(AtomicBool::new(false)))
                .map_err(|e| e.to_string())?;
            Ok((bridge.config().to_owned(), Box::new(bridge)))
        };
        let browser = |_run: &str, mission: &str, cwd: &str| -> Result<(String, crate::claude_context::Held), String> {
            let mut unused = std::process::Command::new("claude");
            let registration = crate::browser_tools::Registration::prepare(&mut unused, "claude", mission, "context-inspection", Path::new(cwd), approvals_channel(), Arc::new(AtomicBool::new(false)))?
                .ok_or("The browser is unavailable.")?;
            let config = registration.claude_config().ok_or("The browser is unavailable.")?.to_owned();
            Ok((config, Box::new(registration)))
        };
        let skills = |own: bool, auto: bool| -> Result<(Vec<(String, String)>, crate::claude_context::Held), String> {
            struct Copies(Vec<PathBuf>);
            impl Drop for Copies {
                fn drop(&mut self) {
                    crate::skills::remove_copies(&self.0);
                }
            }
            let home = std::env::var_os("HOME").map(PathBuf::from);
            let plugins = crate::skills::prepare_claude(&store.root().join("claude-skills"), Path::new(&launch.cwd), own, home.as_deref(), auto, &crate::skills::sources_for_store(store, ""));
            let copies = Copies(crate::skills::copies_of(&plugins, None));
            Ok((plugins.iter().map(|p| (p.name.to_owned(), p.dir.to_string_lossy().into_owned())).collect(), Box::new(copies)))
        };
        prepare_claude_context_launch(launch, turn, &ContextResources { env, permission: &permission, browser: &browser, skills: &skills })
    };
    #[cfg(any(test, feature = "test-hooks"))]
    let run: Option<Arc<crate::claude_context::Run>> = Some(Arc::new(|command: &crate::claude_context::ContextCommand, stop: &crate::claude_context::Stop| {
        let stand_in = crate::invariants::lock(&hooks::CLAUDE_CONTEXT_RUN).clone();
        match stand_in {
            Some(run) => run(command, stop),
            None => crate::claude_context::run_claude_context(command, stop),
        }
    }));
    #[cfg(not(any(test, feature = "test-hooks")))]
    let run = None;
    let reader = Arc::new(ClaudeContextReader::new(ReaderOptions {
        file: store.root().join("claude-context-launches.json"),
        resolve: Arc::new(resolve),
        prepare: Arc::new(prepare),
        run,
        timeout: std::time::Duration::from_secs(10),
    }));
    *held = Some((store.root().to_path_buf(), reader.clone()));
    Some(reader)
}

/// The context ring's breakdown (`readContextBreakdown`): Claude Code's own `/context` estimate for the conversation's
/// last finished turn, or nothing while a turn runs, for another agent, or when it could not be read. Blocks until
/// known, so the window asks off its thread.
pub fn read_claude_context(conversation: &str) -> Option<crate::claude_context::ClaudeContextBreakdown> {
    if !ledger::safe_id(conversation) {
        return None;
    }
    claude_context_reader()?.read(conversation)
}

/// Ports codex-mission.ts `runtimeThreadIdOf` and `resumableThreadOf`: the last runtimeThreadId in the mission's events;
/// nothing after a failure that ended the session or a moved folder; else the one its own `continuesFrom` carried.
pub fn prior_of(tip: &str) -> Result<Prior, String> {
    let path = find_ledger(tip).ok_or("The conversation's ledger could not be found.")?;
    let records = Tail::new(&path, 0).poll();
    let fresh = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|m| std::time::SystemTime::now().duration_since(m).ok())
        .is_none_or(|age| age < ledger::ACTIVE_WINDOW);
    prior_from_records(&records, fresh)
}

pub fn prior_from_records(records: &[Value], fresh: bool) -> Result<Prior, String> {
    let meta = records
        .iter()
        .find(|r| r["recordType"] == "mission.created")
        .map(|r| &r["metadata"])
        .ok_or("The conversation's ledger could not be read.")?;
    let runtime = meta["runtime"].as_str().unwrap_or_default().to_owned();
    let account = meta["resolvedRouteId"]
        .as_str()
        .and_then(crate::agent_accounts::account_from_route)
        .map(str::to_owned);
    let hub_antigravity =
        runtime == "antigravity" && meta["resolvedRouteId"].as_str() == Some("antigravity:hub");
    let mut held: Option<String> = None;
    let mut ended = false;
    let mut moved = false;
    for event in records.iter().filter_map(|r| r.get("event")) {
        let payload = &event["payload"];
        if event["type"] == "run.failed"
            && (payload["sessionEnded"] == true
                || (runtime == "antigravity"
                    && payload["message"]
                        .as_str()
                        .is_some_and(|m| m.to_ascii_lowercase().contains("trajectory not found"))))
        {
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
    let thread = if moved || ended || hub_antigravity {
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
    transcript.apply(records.to_vec());
    let cli_version = meta["cliVersion"].as_str().map(str::to_owned);
    let command_args = meta["command"]["args"].as_array().map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect::<Vec<_>>()
    });
    Ok(Prior {
        runtime,
        account,
        running: transcript.running && fresh,
        thread,
        mode: meta["mode"].as_str().map(str::to_owned),
        cli_version,
        command_args,
        compacted: crate::brief_sessions::compacted_during(&records.iter().filter_map(|r| r.get("event").cloned()).collect::<Vec<_>>()),
    })
}

// ---- running turns, stop ------------------------------------------------------------------------------------------
static RUNNING: Mutex<Option<HashMap<String, Arc<AtomicBool>>>> = Mutex::new(None);

/// Turn threads alive, their ending included (test builds): the hook guard waits for none (`hooks::settle_turn_threads`).
/// `RUNNING` cannot say this: a turn leaves it before its ending runs.
#[cfg(any(test, feature = "test-hooks"))]
static TURN_THREADS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// One turn thread counted in `TURN_THREADS`, from before it is spawned until the thread is done (or never started).
#[cfg(any(test, feature = "test-hooks"))]
struct TurnThread;
#[cfg(any(test, feature = "test-hooks"))]
impl TurnThread {
    fn begin() -> Self {
        TURN_THREADS.fetch_add(1, Ordering::SeqCst);
        TurnThread
    }
}
#[cfg(any(test, feature = "test-hooks"))]
impl Drop for TurnThread {
    fn drop(&mut self) {
        TURN_THREADS.fetch_sub(1, Ordering::SeqCst);
    }
}
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

struct Registered(Vec<String>, Option<String>, Option<(String, String)>);
impl Drop for Registered {
    fn drop(&mut self) {
        with_running(|m| self.0.iter().for_each(|k| drop(m.remove(k))));
        if let Some(id) = &self.1 {
            with_running_accounts(|m| drop(m.remove(id)));
        }
        if let Some((teammate_id, mission_id)) = &self.2 {
            let mut active = crate::invariants::lock(&ACTIVE_TEAMMATES);
            active.retain(|(t, m, _)| !(t == teammate_id && m == mission_id));
            if let Some(capable) = crate::invariants::lock(&STEER_CAPABLE).as_mut() {
                capable.remove(mission_id);
            }
            if let Some(queues) = crate::invariants::lock(&STEER_QUEUES).as_mut() {
                queues.remove(mission_id);
            }
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

/// What a press of Stop means for a run that may not have a name yet (Electron's `stopPress`).
pub use crate::stop_press::{StopPress, StoppableRun, is_stoppable, stop_press};

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
    /// Who the turn is for; `nobody` for a conversation that has no teammate.
    pub teammate: Teammate,
    /// The conversation being continued (its anchor id). The newest turn is never taken from the window: it is read
    /// from disk, in both folders, when sending.
    pub conversation: Option<String>,
    pub prompt: String,
    pub model: Option<String>,
    pub effort: Option<String>,
}

/// A CONVERSATION WITH NO TEAMMATE (Electron 0.878; Colin, 17 Nov: "Can we make it so you can interact with
/// conversations?"). One the person started without a teammate, or whose teammate was removed, is continued on its own
/// agent and model, belonging to nobody: no owner is recorded, no teammate's instructions, skills, keys or memory go
/// into its brief, and no allow rule can be saved for one. The organization's policy and spending cap still hold.
pub fn nobody(runtime: Option<String>, model: Option<String>) -> Teammate {
    Teammate {
        id: String::new(),
        name: "This conversation".into(),
        role: String::new(),
        runtime,
        model,
        account: None,
        busy: false,
        runs_in: String::new(),
        folder: None,
    }
}

/// A turn for `nobody`: the empty id no roster entry can have.
pub fn is_nobody(teammate: &Teammate) -> bool {
    teammate.id.is_empty()
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

/// A Claude Code command with the command guard and without any ticked connectors or session fork.
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
    claude_args_for_turn(crate::runtime::claude::PrintOptions {
        mode,
        bridge,
        browser,
        guard_settings: Some(guard_settings),
        model,
        effort,
        resume,
        references,
        skills,
        ..Default::default()
    }, false)
}

/// Load account connectors when the teammate has a tick, then apply the turn's approval policy to their allow rules.
pub(crate) fn claude_args_for_turn(
    mut options: crate::runtime::claude::PrintOptions<'_>,
    ask_every_connector: bool,
) -> Result<Vec<String>, String> {
    options.account_connectors = !options.connectors.is_empty();
    options.connectors =
        crate::runtime::claude::claude_connector_rules(options.mode, ask_every_connector, options.connectors);
    crate::runtime::claude::print_args(&options)
}

/// A reply on another agent: the brief that starts it, the link it records, and the stopped turn's checkpoint.
struct RouteSwitch {
    prompt: String,
    link: Value,
    checkpoint: Value,
    /// What the composed handoff carries, for the preview under the box (`handoff_preview`).
    kept: Vec<String>,
    omitted: Vec<String>,
    task_clipped: bool,
    /// A long task handed over by file (`.locust/attachments/conversation-<id>.md`).
    task_by_file: bool,
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

/// What a reply that cannot resume is told (codex-mission.ts `coldEarlier`): the conversation through its newest turn,
/// except that newest turn when it went unanswered and this reply is the same words again (Send again, 0.496).
/// The first turn of the conversation `mission` is in, walked back through each turn's `continuesFrom` (codex-mission.ts
/// `firstTurnOf`): a link to a turn not in the ledger ends the walk there.
fn first_turn_of(mission: &str) -> String {
    let mut first = mission.to_owned();
    let mut seen = HashSet::new();
    while seen.insert(first.clone()) {
        let Some(earlier) = turn_records(&first).and_then(|t| t.metadata["continuesFrom"]["missionId"].as_str().map(str::to_owned)) else { break };
        first = earlier;
    }
    first
}

fn cold_earlier(tip: &str, prompt: &str) -> Vec<crate::carry_on::EarlierTurn> {
    let Some(turn) = turn_records(tip) else { return Vec::new() };
    let mut turns = earlier_turns(&turn.metadata);
    let asked = turn.metadata["prompt"].as_str().unwrap_or_default();
    let answered = crate::carry_on::latest_final(&turn.events);
    let again = answered.as_deref().unwrap_or_default().trim().is_empty() && asked.trim() == prompt.trim();
    if !again {
        turns.push(crate::carry_on::EarlierTurn { asked: asked.to_owned(), answered });
    }
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

/// `write`: a send writes a long task's file; a preview only names where it would go (`longTaskFilePath`).
fn route_switch(store: &Store, tip: &str, reply: &str, left_out_by_you: &[String], write: bool) -> Result<RouteSwitch, String> {
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
    let original = prior.metadata["prompt"].as_str().unwrap_or_default();
    // A long task goes by file (long-task-file.ts, 0.513), into the project the conversation worked in.
    let task_file = if write {
        crate::door_tools::conversation_folder(&prior.metadata["startedBy"])
            .map(PathBuf::from)
            .or_else(|| workspace_folder(store.electron()))
            .and_then(|project| crate::carry_on::long_task_file(original, &project, tip))
    } else {
        crate::carry_on::long_task_file_path(original, tip)
    };
    let briefing = crate::carry_on::compose_handoff_brief(
        original,
        &checkpoint,
        crate::carry_on::display_name(from),
        Some(reply),
        &earlier_turns(&prior.metadata),
        crate::carry_on::how_turn_ended(&prior.events),
        left_out_by_you,
        task_file.as_deref(),
    )
    .ok_or("That conversation is too long to carry to another runtime with this reply.")?;
    // The file rides as an attachment, in front of the brief, like any other file the run is handed.
    let prompt = match &task_file {
        Some(file) => crate::attachments::with_attachments(&briefing.prompt, std::slice::from_ref(file)),
        None => briefing.prompt,
    };
    let mut link = json!({"missionId": tip, "checkpointEpoch": epoch, "reason": "route-switch"});
    if !briefing.omitted.is_empty() {
        link["leftOut"] = json!(briefing.omitted);
    }
    if !briefing.left_out_by_you.is_empty() {
        link["leftOutByYou"] = json!(briefing.left_out_by_you);
    }
    let native_prior = store.ledger_dir().ok().filter(|dir| prior.path.starts_with(dir)).map(|_| prior.path.clone());
    Ok(RouteSwitch { prompt, link, checkpoint, native_prior, kept: briefing.kept, omitted: briefing.omitted, task_clipped: briefing.task_clipped, task_by_file: task_file.is_some() })
}

/// What a reply on `runtime` in this conversation would carry, composed as the send composes it (Electron's
/// HANDOFF_PREVIEW_CHANNEL). Reads only: nothing is written or started.
pub fn handoff_preview(conversation: &str, runtime: &str, reply: &str, left_out_by_you: &[String]) -> crate::carry_on::HandoffPreview {
    use crate::carry_on::HandoffPreview;
    let Some(store) = store() else { return HandoffPreview::Same };
    let Ok(tip) = real_tip(conversation) else { return HandoffPreview::Same };
    let Some(prior) = turn_records(&tip) else { return HandoffPreview::Same };
    if prior.metadata["runtime"].as_str().unwrap_or_default() == runtime {
        return HandoffPreview::Same;
    }
    match route_switch(&store, &tip, reply, left_out_by_you, false) {
        Ok(sw) => HandoffPreview::Switch {
            unsettled_count: sw.checkpoint["unsettledActions"].as_array().map_or(0, Vec::len),
            kept: sw.kept,
            omitted: sw.omitted,
            task_by_file: sw.task_by_file,
            task_clipped: sw.task_clipped,
        },
        Err(message) => HandoffPreview::Refused(message),
    }
}

/// Turns already carried on, so a second press while the first is starting is refused rather than run twice.
static CARRIED: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// main/carry-on.ts `carry`: carry a turn that stopped on its plan, its provider or its process on to another agent.
/// The new turn continues the conversation in the stopped turn's mode (and an app's folder), told what was done.
/// Returns the new turn's id, the folder it works in and the mode it runs in.
pub fn carry(mission: &str, runtime: &str, model: Option<&str>) -> Result<(String, Option<String>, String), String> {
    carry_with(mission, runtime, model, &crate::carry_on::CarryOnAlso::default())
}

/// main/carry-on.ts `carry` with an app's extras (0.882): picking up a turn Locust closed under (on its own agent too),
/// or moving a finished conversation to another agent with a message (or appending a message to a stopped carry-on).
pub fn carry_with(
    mission: &str,
    runtime: &str,
    model: Option<&str>,
    also: &crate::carry_on::CarryOnAlso,
) -> Result<(String, Option<String>, String), String> {
    if crate::invariants::lock(&CARRIED).iter().any(|m| m == mission) {
        return Err("This turn was already carried on.".into());
    }
    let store = store().ok_or("Native data folder could not be determined.")?;
    let prior = turn_records(mission).ok_or("This conversation is not in the record any more, so it cannot be carried on.")?;
    let from = prior.metadata["runtime"].as_str().unwrap_or_default().to_owned();
    let mode = prior.metadata["mode"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| if prior.metadata["sandbox"] == "read-only" { "ask".into() } else { "accept-edits".into() });
    let decision = crate::carry_on::carry_decision(
        crate::carry_on::phase_of(&prior.events),
        &prior.events,
        &from,
        runtime,
        &mode,
        also,
    )?;
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
        prompt: decision.prompt,
        model: model.map(str::to_owned),
        effort: None,
    };
    let origin = Origin {
        mode: Mode::parse(&mode),
        folder: folder.clone(),
        started_by: Some(started_by),
        runtime: Some(runtime.to_owned()),
        account: Some(None),
        allow_cut_off: decision.cut_off,
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
    let reason = crate::carry_on::stopped_for(crate::carry_on::phase_of(&prior.events), &prior.events);
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

/// createSpendRefusal for a teammate's own limit, with no organization cap. Reads both ledgers, so it runs only for a
/// teammate that has a limit. A limit that cannot be checked lets the turn start and says so in the log, as Electron's
/// `spendRefusalFor` does: only an organization's cap closes the gate on an unreadable record (`org_spend_gate`).
fn spend_gate(store: &Store, owners: &Value, teammate: &Teammate, limit: f64) -> Result<(), String> {
    let now = chrono::Local::now();
    let unchecked = |why: &str| eprintln!("spend-limit: could not check {}'s monthly limit: {why}", teammate.id);
    let Some(month) = crate::spend::month_of(&now.to_rfc3339()) else {
        unchecked("The current month could not be read.");
        return Ok(());
    };
    let turns = match crate::usage_ledger::usage_turns(store) {
        Ok(turns) => turns,
        Err(_) => {
            unchecked("The spending record could not be read.");
            return Ok(());
        }
    };
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
    /// Electron seals only Scout's runs (index.ts 8681, 10103): no brief, no skills, no browser, OpenCode refuses every
    /// tool (`OPENCODE_SEALED_CONFIG`), and Codex starts with no shell/apps/web_search/MCP/connectors (`sealed_config`).
    pub sealed: bool,
}

/// Started again from an edited earlier message (0.498): the new turn continues the turn BEFORE the edited one, cold (its
/// session also holds every turn after it, which are being set aside), and says it is an edit in its `continuesFrom`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rewind {
    /// The conversation's newest turn, which the new branch replaces as the current one. It must not be running.
    pub tip: String,
    /// The turn before the edited one; None when the first message was edited, which starts a new conversation.
    pub before: Option<String>,
    /// The turns the edit sets aside (0.512): what they sent and nobody read is never delivered.
    pub set_aside: Vec<String>,
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
    /// Workroom message ids a relayed turn was started to answer, quoted even if marked delivered.
    pub include_messages: Vec<String>,
    /// Serialized `RelayOrigin` recorded in `mission.created["metadata"]["relay"]`.
    pub relay: Option<Value>,
    /// Handoff sections the person opted to omit when switching runtimes.
    pub left_out_by_you: Vec<String>,
    /// Earlier posts and finished answers in a room, for a turn posted to a room.
    pub room_history: Option<crate::room_history::RoomHistory>,
    /// Picking up a turn Locust closed under (0.882): not live in this process, so its fresh ledger mtime does not block.
    pub allow_cut_off: bool,
    /// The turn is an edited earlier message, started again from the turn before it.
    pub rewind: Option<Rewind>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayNotice {
    pub mission_id: String,
    pub message: String,
    pub level: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayStarting {
    pub teammate_id: String,
    pub name: String,
    pub answering: String,
    pub hop: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RelayLiveState {
    pub notices: Vec<RelayNotice>,
    pub starting: Option<RelayStarting>,
}

static ACTIVE_TEAMMATES: Mutex<Vec<(String, String, PathBuf)>> = Mutex::new(Vec::new());
static STEER_CAPABLE: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);
static STEER_QUEUES: Mutex<Option<HashMap<String, Vec<String>>>> = Mutex::new(None);
static RELAY_NOTICES: Mutex<Vec<RelayNotice>> = Mutex::new(Vec::new());
static RELAY_STARTING: Mutex<Vec<RelayStarting>> = Mutex::new(Vec::new());
static ROOM_NOTICES: Mutex<Vec<crate::room_tasks::RoomNotice>> = Mutex::new(Vec::new());
static RELAY: Mutex<Option<Arc<crate::relay::Relay<NativeRelayHost>>>> = Mutex::new(None);
static EDIT_CHECK: std::sync::OnceLock<crate::edit_check::EditCheck> = std::sync::OnceLock::new();
static CHECK_FIX: std::sync::OnceLock<crate::edit_check::CheckFix> = std::sync::OnceLock::new();

pub(crate) fn push_room_notice(notice: crate::room_tasks::RoomNotice) {
    let mut list = crate::invariants::lock(&ROOM_NOTICES);
    if list.len() >= 128 {
        list.remove(0);
    }
    list.push(notice);
}

/// What finished replies did to memory, for the Memory screen's notice (memory-reader.ts `memory-changed`), oldest
/// first, until the window takes them.
static MEMORY_CHANGES: Mutex<Vec<crate::people::MemoryChanged>> = Mutex::new(Vec::new());

pub fn push_memory_changed(update: crate::people::MemoryChanged) {
    if update.is_empty() {
        return;
    }
    let mut list = crate::invariants::lock(&MEMORY_CHANGES);
    if list.len() >= 32 {
        list.remove(0);
    }
    list.push(update);
}

pub fn take_memory_changes() -> Vec<crate::people::MemoryChanged> {
    std::mem::take(&mut *crate::invariants::lock(&MEMORY_CHANGES))
}

pub fn take_room_notices() -> Vec<crate::room_tasks::RoomNotice> {
    std::mem::take(&mut *crate::invariants::lock(&ROOM_NOTICES))
}

pub(crate) fn is_room_teammate_busy_error(msg: &str) -> bool {
    msg.ends_with(" is already active in another run.")
}

#[cfg(any(test, feature = "test-hooks"))]
pub(crate) fn reset_relay_state() {
    *crate::invariants::lock(&RELAY) = None;
    crate::invariants::lock(&ACTIVE_TEAMMATES).clear();
    *crate::invariants::lock(&STEER_CAPABLE) = None;
    *crate::invariants::lock(&STEER_QUEUES) = None;
    crate::invariants::lock(&RELAY_NOTICES).clear();
    crate::invariants::lock(&RELAY_STARTING).clear();
    crate::invariants::lock(&ROOM_NOTICES).clear();
}

fn push_relay_notice(mission_id: &str, message: &str, level: &str) {
    let mut notices = crate::invariants::lock(&RELAY_NOTICES);
    notices.push(RelayNotice {
        mission_id: mission_id.to_owned(),
        message: message.to_owned(),
        level: level.to_owned(),
    });
    if notices.len() > 200 {
        let extra = notices.len() - 200;
        notices.drain(..extra);
    }
}

/// What the engine holds for a relay, said in one line, for a test whose wait on a relay gave up: the turns running,
/// the teammates active in a relayed run, the relay's notices and the runs it is starting, and the hooked store's team.
#[cfg(any(test, feature = "test-hooks"))]
pub fn relay_debug() -> String {
    let running = crate::engine_api::api().running();
    let active = crate::invariants::lock(&ACTIVE_TEAMMATES).clone();
    let notices = crate::invariants::lock(&RELAY_NOTICES).clone();
    let starting = crate::invariants::lock(&RELAY_STARTING).clone();
    let relay_made = crate::invariants::lock(&RELAY).is_some();
    let team = store().and_then(|s| std::fs::read_to_string(s.root().join("teammates.json")).ok());
    format!(
        "running={running:?} active={active:?} notices={notices:?} starting={starting:?} relay_made={relay_made} store={:?} team={team:?}",
        store().map(|s| s.root().to_path_buf())
    )
}

pub fn relay_live_state(mission_ids: &[String]) -> RelayLiveState {
    let notices = crate::invariants::lock(&RELAY_NOTICES)
        .iter()
        .filter(|n| mission_ids.iter().any(|m| m == &n.mission_id))
        .cloned()
        .collect();
    let starting = crate::invariants::lock(&RELAY_STARTING)
        .iter()
        .rev()
        .find(|s| mission_ids.iter().any(|m| m == &s.answering))
        .cloned();
    RelayLiveState { notices, starting }
}

fn relay() -> Arc<crate::relay::Relay<NativeRelayHost>> {
    let mut slot = crate::invariants::lock(&RELAY);
    if let Some(existing) = slot.as_ref() {
        return existing.clone();
    }
    let created = Arc::new(crate::relay::Relay::new(NativeRelayHost));
    *slot = Some(created.clone());
    created
}

struct NativeRelayHost;

impl NativeRelayHost {
    fn folder_for_mission(&self, mission_id: &str) -> Option<PathBuf> {
        if let Some((_, _, folder)) = crate::invariants::lock(&ACTIVE_TEAMMATES)
            .iter()
            .find(|(_, m, _)| m == mission_id)
        {
            return Some(folder.clone());
        }
        turn_records(mission_id)
            .and_then(|r| {
                r.metadata["cwd"]
                    .as_str()
                    .or_else(|| r.metadata["command"]["cwd"].as_str())
                    .map(PathBuf::from)
            })
            .filter(|p| p.is_dir())
    }
}

impl crate::relay::RelayHost for NativeRelayHost {
    fn enabled(&self) -> bool {
        let Some(store) = store() else { return true };
        crate::workspace_settings::load(&store).map(|s| s.relay).unwrap_or(true)
    }

    fn exchange_of(&self, mission_id: &str) -> Option<String> {
        let store = store()?;
        let rooms = crate::room_store::list(&store).ok()?;
        crate::room_store::exchange_of_room_post(&rooms, mission_id)
    }

    fn hop_cap(&self) -> usize {
        let Some(store) = store() else {
            return crate::relay::MAX_RELAY_HOPS;
        };
        crate::workspace_settings::load(&store)
            .map(|s| s.relay_hop_cap as usize)
            .unwrap_or(crate::relay::MAX_RELAY_HOPS)
    }

    fn peer_context_for(&self, teammate_id: &str) -> Option<crate::relay::PeerRosterEntry> {
        let store = store()?;
        crate::people::peer_roster_entry(&store, teammate_id)
    }

    fn final_reply_of(&self, mission_id: &str) -> Option<String> {
        let rec = turn_records(mission_id)?;
        crate::carry_on::latest_final(&rec.events)
    }

    fn how_ended(&self, mission_id: &str) -> Option<String> {
        let rec = turn_records(mission_id)?;
        crate::carry_on::how_turn_ended(&rec.events).map(str::to_owned)
    }

    fn post_returned_answer(
        &self,
        from_teammate_id: &str,
        from_name: &str,
        from_mission_id: &str,
        to_teammate_id: &str,
        to_name: &str,
        text: &str,
    ) -> Option<String> {
        let store = store()?;
        let from = crate::workroom::Party {
            teammate_id: from_teammate_id.to_owned(),
            name: from_name.to_owned(),
        };
        let to = crate::workroom::Party {
            teammate_id: to_teammate_id.to_owned(),
            name: to_name.to_owned(),
        };
        let msg = crate::workroom::Workroom::for_store(&store)
            .post(&from, from_mission_id, &to, text, None)
            .ok()?;
        if let Ok(path) = store.write_path(from_mission_id)
            && path.is_file()
        {
            let _ = crate::ledger_write::append_peer_links(
                &path,
                &[("posted", &msg.message_id, &msg.to.teammate_id, &msg.posted_at)],
            );
        }
        Some(msg.message_id)
    }

    fn cursor_holds_read_only(&self) -> bool {
        #[cfg(target_os = "windows")]
        {
            false
        }
        #[cfg(not(target_os = "windows"))]
        {
            true
        }
    }

    fn route_of(&self, mission_id: &str) -> Option<crate::relay::TurnRoute> {
        let rec = turn_records(mission_id)?;
        let runtime = rec.metadata["runtime"].as_str()?.to_owned();
        let model = rec.metadata["model"]
            .as_str()
            .filter(|m| !m.is_empty() && *m != "account-default")
            .map(str::to_owned);
        let account = rec.metadata["account"]
            .as_str()
            .filter(|a| !a.is_empty())
            .map(str::to_owned);
        let mode = rec.metadata["mode"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| {
                if rec.metadata["sandbox"] == "workspace-write" {
                    "accept-edits".to_owned()
                } else {
                    "ask".to_owned()
                }
            });
        Some(crate::relay::TurnRoute {
            runtime,
            model,
            effort: None,
            account,
            mode,
        })
    }

    fn out_of_plan(&self, runtime: &str, account: Option<&str>) -> Option<String> {
        let store = store()?;
        let key = crate::agent_accounts::usage_key(runtime, account);
        crate::usage_ledger::limited_runtimes(&store).get(&key).cloned()
    }

    fn start(&self, req: crate::relay::StartRequest) -> Result<crate::relay::StartResult, String> {
        let store = store().ok_or_else(|| "Native data folder could not be determined.".to_owned())?;
        if with_running(|m| live_turns(m)) >= MAX_LIVE_TURNS {
            return Ok(crate::relay::StartResult::Busy { pool: true });
        }
        if crate::invariants::lock(&ACTIVE_TEAMMATES)
            .iter()
            .any(|(t, _, _)| t == &req.peer.teammate_id)
        {
            return Ok(crate::relay::StartResult::Busy { pool: false });
        }
        let roster = roster_in(&store)?;
        let mut tm = roster
            .into_iter()
            .find(|t| t.id == req.peer.teammate_id)
            .ok_or_else(|| format!("{} is no longer on the roster.", req.peer.name))?;
        tm.runtime = Some(req.runtime.clone());
        tm.model = req.model.clone();
        tm.account = req.account.clone();
        let folder = req
            .relay
            .last_mission_of
            .last()
            .and_then(|(_, m)| self.folder_for_mission(m))
            .or_else(|| tm.folder.as_deref().map(PathBuf::from).filter(|p| p.is_dir()))
            .or_else(open_folder);
        let job = Send {
            teammate: tm,
            conversation: req.follow_up_of,
            prompt: req.prompt,
            model: req.model,
            effort: req.effort,
        };
        let origin = Origin {
            mode: Mode::parse(&req.mode),
            folder,
            started_by: Some(json!({"kind": "relay", "hop": req.relay.hop})),
            runtime: Some(req.runtime),
            account: Some(req.account),
            include_messages: req.relay.answering.clone(),
            relay: Some(req.relay.to_json()),
            ..Default::default()
        };
        match crate::engine_api::api().send(job, origin) {
            Ok(started) => {
                let mission_id = real_tip(&started.conversation_id).unwrap_or(started.conversation_id);
                Ok(crate::relay::StartResult::Started {
                    run_id: format!("run_{mission_id}"),
                    mission_id,
                })
            }
            Err(msg) => {
                if msg == too_many_live() {
                    Ok(crate::relay::StartResult::Busy { pool: true })
                } else if msg.contains("already running") {
                    Ok(crate::relay::StartResult::Busy { pool: false })
                } else {
                    Ok(crate::relay::StartResult::Refused { message: msg })
                }
            }
        }
    }

    fn assign_owner(&self, teammate_id: &str, mission_id: &str) {
        if let Some(store) = store() {
            let _ = crate::people::record_owner(&store, mission_id, teammate_id);
        }
    }

    fn hub_of(&self, teammate_id: &str) -> Option<String> {
        let store = store()?;
        crate::people::hub_of(&store, teammate_id)
    }

    fn remember_hub(&self, teammate_id: &str, mission_id: &str, began: bool) {
        let Some(store) = store() else { return };
        let _ = crate::people::remember_hub(&store, teammate_id, mission_id);
        if began && let Some(entry) = self.peer_context_for(teammate_id) {
            let _ = crate::people::set_mission_title(&store, mission_id, &format!("{}'s replies", entry.name));
        }
    }

    fn still_waiting(&self, teammate_id: &str) -> Option<bool> {
        let store = store()?;
        crate::workroom::Workroom::for_store(&store)
            .unread(teammate_id, 1, &[])
            .ok()
            .map(|u| !u.messages.is_empty())
    }

    fn may_interrupt(&self) -> bool {
        let Some(store) = store() else { return false };
        crate::workspace_settings::load(&store).map(|s| s.interrupt).unwrap_or(false)
    }

    fn stop_work_of(&self, teammate_id: &str) -> bool {
        let missions: Vec<String> = crate::invariants::lock(&ACTIVE_TEAMMATES)
            .iter()
            .filter(|(t, _, _)| t == teammate_id)
            .map(|(_, m, _)| m.clone())
            .collect();
        let mut any = false;
        for m in missions {
            if stop(&m).is_ok() {
                any = true;
            }
        }
        any
    }

    fn steer_work_of(&self, teammate_id: &str, text: &str) -> bool {
        let missions: Vec<String> = crate::invariants::lock(&ACTIVE_TEAMMATES)
            .iter()
            .filter(|(t, _, _)| t == teammate_id)
            .map(|(_, m, _)| m.clone())
            .collect();
        let capable = crate::invariants::lock(&STEER_CAPABLE);
        let target = missions.into_iter().find(|m| capable.as_ref().is_some_and(|s| s.contains(m)));
        drop(capable);
        let Some(mission_id) = target else {
            return false;
        };
        let mut queues = crate::invariants::lock(&STEER_QUEUES);
        queues
            .get_or_insert_with(HashMap::new)
            .entry(mission_id)
            .or_default()
            .push(text.to_owned());
        true
    }

    fn notify(&self, update: crate::relay::RelayUpdate) {
        match update {
            crate::relay::RelayUpdate::Notice {
                mission_id,
                message,
                level,
                ..
            } => {
                let lvl = match level {
                    crate::relay::RelayNoticeLevel::Info => "info",
                    crate::relay::RelayNoticeLevel::Warning => "warning",
                };
                push_relay_notice(&mission_id, &message, lvl);
            }
            crate::relay::RelayUpdate::Starting {
                teammate_id,
                name,
                answering,
                hop,
            } => {
                let mut list = crate::invariants::lock(&RELAY_STARTING);
                list.retain(|s| s.teammate_id != teammate_id);
                list.push(RelayStarting {
                    teammate_id,
                    name,
                    answering,
                    hop,
                });
            }
            crate::relay::RelayUpdate::StartSettled { teammate_id } => {
                crate::invariants::lock(&RELAY_STARTING).retain(|s| s.teammate_id != teammate_id);
            }
            crate::relay::RelayUpdate::MissionStarted { .. } => {}
        }
    }

    fn note(&self, mission_id: &str, message: &str) {
        let Some(store) = store() else { return };
        if let Ok(path) = store.write_path(mission_id)
            && path.is_file()
        {
            let _ = crate::ledger_write::append_note_with_level(&path, "info", "host.relay_ended", message);
        }
    }
}

fn edit_check() -> &'static crate::edit_check::EditCheck {
    EDIT_CHECK.get_or_init(crate::edit_check::EditCheck::new)
}

fn check_fix() -> &'static crate::edit_check::CheckFix {
    CHECK_FIX.get_or_init(crate::edit_check::CheckFix::new)
}

pub fn running_checks() -> Vec<(String, String)> {
    edit_check().running_checks()
}

/// codex-mission.ts MAX_PROMPT_LENGTH, counted as Electron counts a string's length (UTF-16 units).
pub const MAX_PROMPT_LENGTH: usize = 8_000;

/// codex-mission.ts INVALID_PROMPT's words.
pub const INVALID_PROMPT: &str = "Enter a mission between 1 and 8,000 characters.";

/// codex-mission.ts validPrompt: some words, at most MAX_PROMPT_LENGTH, and no NUL.
fn valid_prompt(prompt: &str) -> bool {
    !prompt.trim().is_empty() && prompt.encode_utf16().count() <= MAX_PROMPT_LENGTH && !prompt.contains('\0')
}

/// codex-mission.ts notReadyRefusal: which kind of not ready, as discovery found it.
pub fn not_ready_refusal(runtime: &str, found: Option<&crate::agents::Found>) -> String {
    let name = crate::failure_words::runtime_display_name(runtime);
    let Some(found) = found.filter(|f| f.installed) else {
        return format!("{name} is not installed on this machine. Install it, then retry discovery.");
    };
    match found.status {
        crate::agents::Probe::AuthRequired => format!("{name} is signed out. Sign in to it, then retry discovery."),
        crate::agents::Probe::Offline => format!("{name} could not be reached just now: it did not answer when Locust checked on it, which usually means its own service is down. Try again in a minute, or pick another model."),
        // codex-mission.ts readiness 'unknown' (Antigravity's silent `agy models`): not "install or sign in" (LIVE2 #19).
        status @ (crate::agents::Probe::Unknown | crate::agents::Probe::ProbeFailed)
            if status == crate::agents::Probe::Unknown || runtime == "antigravity" =>
        {
            format!("{name} did not answer in time when Locust checked whether it was ready, so nothing was sent. Try again in a moment.")
        }
        _ => format!("{name} is not ready. Install or sign in to it, then retry discovery."),
    }
}

/// What discovery says of the runtime a turn is about to start on (index.ts discoverForStart): the held answer while
/// it is fresh, that runtime asked alone once it is stale. None when nothing is asked: a test's fake runtime.
fn found_for_start(runtime: &str) -> Option<Vec<crate::agents::Found>> {
    #[cfg(any(test, feature = "test-hooks"))]
    {
        if let Some(readiness) = crate::invariants::lock(&hooks::READINESS).clone() {
            return Some(crate::agents::waits_for_its_own_check(Some(runtime), readiness.for_start(Some(runtime)), |_| None));
        }
        if crate::invariants::lock(&hooks::STATE).0.is_some() {
            return None;
        }
    }
    Some(crate::agents::discover_for_start(Some(runtime)))
}

/// codex-mission.ts start: a runtime that is not available, ready and with a program to run is refused in words before
/// anything is recorded. Your own agent is found by its command line, never by discovery; an added account's sign-in is
/// its own folder's, which this does not ask.
pub(crate) fn start_readiness_refusal(runtime: &str, account: Option<&str>) -> Option<String> {
    if runtime == "acp" || account.is_some() {
        return None;
    }
    let found = found_for_start(runtime)?;
    let chosen = found.iter().find(|entry| entry.id == runtime);
    if chosen.is_some_and(|c| c.installed && c.ready) {
        return None;
    }
    Some(not_ready_refusal(runtime, chosen))
}

/// Start (or continue) a turn the person asked for in the window.
pub fn send(job: Send) -> Result<Started, String> {
    send_from(job, Origin::default())
}

/// Start (or continue) a Codex/Claude turn natively. Returns once the ledger exists and the turn is running on its own thread;
/// the window follows the ledger file. Approvals raise cards answered through `decide_approval`.
/// Never retried: a failure after the ledger exists is recorded in it, not tried again.
pub fn send_from(mut job: Send, mut origin: Origin) -> Result<Started, String> {
    // codex-mission.ts validPrompt: refused before anything is read, started or recorded.
    if !valid_prompt(&job.prompt) {
        return Err(INVALID_PROMPT.into());
    }
    // Where the seconds before "started" go (start_timing): the send reached the host now.
    let clock = crate::start_timing::Clock::sent();
    let own_agent = origin.runtime.is_none() || origin.runtime == job.teammate.runtime;
    if let Some(runtime) = &origin.runtime {
        job.teammate.runtime = Some(runtime.clone());
    }
    let store = store().ok_or("Native data folder could not be determined.")?;
    let saved = crate::people::team(&store)?;
    // ROUTE AUTHORITY AT THE MOMENT OF START (route-at-start.ts): the window's copy of a teammate is not their choice.
    // A turn that names its own route (a pick in the box for this conversation, or the conversation's own route on a
    // follow-up) is that route; any other starts on the route the host keeps for the teammate, its effort and account
    // included; a teammate with none keeps what was asked, or the defaults.
    {
        let explicit = origin.runtime.is_some() || job.model.is_some();
        let kept = crate::route_at_start::saved_route_in(&saved, &job.teammate.id);
        let resolved = crate::route_at_start::route_at_start(
            &crate::route_at_start::StartRouteInput {
                runtime: job.teammate.runtime.clone(),
                model: job.model.clone().or_else(|| job.teammate.model.clone()),
                effort: job.effort.clone(),
                // No picker override: the route is applied only when nothing was picked for this turn (below).
                ..Default::default()
            },
            Some((job.teammate.id.as_str(), kept.as_ref())),
        );
        if !explicit && kept.is_some() {
            job.teammate.runtime = Some(resolved.runtime);
            job.teammate.model = resolved.model;
            job.teammate.account = resolved.account;
            job.effort = resolved.effort;
        }
    }
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
    let mut effective_account = match &origin.account {
        Some(explicit) => explicit.clone(),
        None if own_agent => job.teammate.account.clone(),
        None => None,
    };
    // A NEW conversation that names no account runs on the agent's `forNew` account (Electron 0.876), before the
    // policy gate so it judges the real account. Never a reply, rewind or side question.
    let is_new_conversation = job.conversation.is_none()
        && origin.rewind.is_none()
        && crate::side_question::parse_started_by(origin.started_by.as_ref()).is_none();
    if is_new_conversation && origin.account.is_none() && effective_account.is_none() {
        let runtime = job.teammate.runtime.as_deref().unwrap_or("codex");
        if crate::agent_accounts::Agent::parse(runtime).is_some()
            && let Ok(Some(for_new)) = accounts_for(&store).for_new(runtime)
        {
            effective_account = Some(for_new);
        }
    }
    // Asking Scout always runs in Ask mode (read-only) and sealed against tools/connectors,
    // with its 14-day look log brief passed via slot.instructions, before policy evaluation. An app's own work sent
    // through the door is not asking Scout (asks_scout).
    if origin.slot.is_none()
        && let Some(slot) = crate::scout::host::asking_scout_slot(&store, &job.teammate.id, origin.started_by.as_ref())
    {
        origin.mode = Some(Mode::Ask);
        origin.slot = Some(slot);
    }
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
    // An edit replaces the conversation's newest turn: that one must not be running either.
    if let Some(rewind) = &origin.rewind
        && owns(&rewind.tip)
    {
        return Err("A turn is still running in that conversation. Wait for it or stop it first.".into());
    }
    // And no more than eight at once (checked again where the turn registers, so two sends cannot both slip in).
    if with_running(|m| live_turns(m)) >= MAX_LIVE_TURNS {
        return Err(too_many_live());
    }
    let is_room = origin
        .started_by
        .as_ref()
        .and_then(|v| v["kind"].as_str())
        == Some("room");
    if is_room
        && crate::invariants::lock(&ACTIVE_TEAMMATES)
            .iter()
            .any(|(t, _, _)| t == &job.teammate.id)
    {
        return Err(format!("{} is already active in another run.", job.teammate.name));
    }
    let runtime_name = job.teammate.runtime.as_deref().unwrap_or("codex");
    let mode = origin.mode.unwrap_or_else(|| Mode::parse(&job.teammate.runs_in).unwrap_or(Mode::Ask));
    // codex-mission.ts AUTO_OFF_REFUSAL: Auto runs only where the person turned it on in Settings.
    if mode == Mode::Auto && !crate::workspace_settings::load(&store).map(|s| s.auto_mode).unwrap_or(false) {
        return Err(AUTO_OFF_REFUSAL.into());
    }
    // codex-mission.ts: whether the runtime is ready is asked before anything is recorded or started, and a start on
    // one that is not says which kind of not ready.
    clock.mark(crate::start_timing::Phase::Looking);
    if let Some(why) = start_readiness_refusal(runtime_name, effective_account.as_deref()) {
        return Err(why);
    }
    // The route as the person asked for it (index.ts rememberRoute): the model and level before Cursor folds the level
    // into its model's id.
    let asked_route = {
        let mut route = json!({
            "runtime": runtime_name,
            "model": job.model.as_deref().or(job.teammate.model.as_deref()).filter(|m| !m.is_empty()).unwrap_or("account-default"),
            "mode": mode.as_str(),
        });
        if let Some(effort) = &job.effort {
            route["effort"] = json!(effort);
        }
        if let Some(account) = &effective_account {
            route["account"] = json!(account);
        }
        route
    };
    let launch_env = account_launch_env(&store, runtime_name, effective_account.as_deref())?;
    let electron = store.electron().to_path_buf();
    // A reply on ANOTHER agent or account (codex-mission.ts, the route switch): the conversation is reconciled, and a
    // brief of what was done starts the new agent/account cold, with the reply as the latest word.
    let mut switched: Option<RouteSwitch> = None;
    let mut side_prompt: Option<String> = None;
    // A cold start (an edit started again from the turn before it, or a reply that cannot resume): what was said up to
    // there, and whether it is an edit.
    let mut cold: Option<(Vec<crate::carry_on::EarlierTurn>, bool)> = None;
    let mut rewind_link: Option<Value> = None;
    // A2.5: whether the session a reply resumes compacted during its last turn.
    let mut resumed_compacted = false;
    let (mut continues, mut resume, continues_folder_mission) = if let Some(side) = &side_req {
        let prepared = crate::side_question::prepare(&side.of, runtime_name, mode, &job.prompt)?;
        side_prompt = Some(prepared.runtime_prompt);
        (None, Some(prepared.resume_thread), Some(side.of.clone()))
    } else if let Some(rewind) = &origin.rewind {
        // Cold, on whichever agent is chosen: resuming the session of the turn before would hand the model the very
        // turns being set aside. The first message edited has nothing before it: a new conversation.
        if let Some(before) = rewind.before.as_deref() {
            if owns(before) || turn_records(before).is_none() {
                return Err("That conversation cannot be continued: its earlier mission is not in the ledger.".into());
            }
            cold = Some((turns_through(before), true));
            rewind_link = Some(json!({"missionId": before, "checkpointEpoch": 1, "reason": "follow-up", "edited": true}));
        }
        let folder_mission = rewind.before.clone().unwrap_or_else(|| rewind.tip.clone());
        (None, None, Some(folder_mission))
    } else {
        match job.conversation.as_deref() {
        None => (None, None, None),
        Some(conversation) => {
            let tip = real_tip(conversation)?;
            let tip_owned = tip.clone();
            let tip = tip.as_str();
            if owns(tip) {
                return Err(
                    "A turn is still running in that conversation. Wait for it or stop it first."
                        .into(),
                );
            }
            let prior = prior_of(tip)?;
            if prior.running && !origin.allow_cut_off {
                return Err(
                    "A turn is still running in that conversation. Wait for it or stop it first."
                        .into(),
                );
            }
            if prior.runtime != runtime_name || prior.account != effective_account {
                switched = Some(route_switch(&store, tip, &job.prompt, &origin.left_out_by_you, true)?);
                (None, None, Some(tip_owned))
            } else {
                // codex-mission.ts: a reply on the same agent resumes its session, unless the mode changed since (a
                // resumed session keeps the tools it was built with, so the mode on screen would not be the one that
                // runs), OpenCode switched protocols or the prior ACP session was imported/unverified, or there is no
                // session to resume (a turn that failed before its runtime started). Then it starts cold, still in
                // this conversation, told what was said so far.
                let mode_changed = prior.mode.as_deref().is_some_and(|m| m != mode.as_str());
                let opencode_fresh = if runtime_name == "opencode" && !mode_changed && prior.thread.is_some() {
                    let current_is_acp = {
                        #[cfg(any(test, feature = "test-hooks"))]
                        {
                            if let Some((_, args)) = crate::invariants::lock(&hooks::STATE).1.as_ref() {
                                args.iter().any(|a| a == "acp")
                            } else {
                                crate::runtime::opencode::is_installed_v2()
                            }
                        }
                        #[cfg(not(any(test, feature = "test-hooks")))]
                        {
                            crate::runtime::opencode::is_installed_v2()
                        }
                    };
                    opencode_follow_up_needs_fresh_session(
                        prior.mode.as_deref(),
                        prior.cli_version.as_deref(),
                        prior.command_args.as_deref(),
                        current_is_acp,
                    )
                } else {
                    false
                };
                let thread = if mode_changed || opencode_fresh { None } else { prior.thread };
                resumed_compacted = prior.compacted;
                if thread.is_none() {
                    cold = Some((cold_earlier(tip, &job.prompt), false));
                }
                (
                    Some(Continuation {
                        mission_id: tip.to_owned(),
                        thread: thread.clone(),
                    }),
                    thread,
                    Some(tip_owned),
                )
            }
        }
    }
    };
    // A teammate set to work on its own branch runs in its own copy of the project, and each of its turns is
    // committed there with a receipt (worktrees.rs; Electron's peer.repositoryRoot).
    // A comparison slot with its own `cwd` already has its own copy or worktree.
    let (folder, repository_root) = resolve_run_folder(
        &store,
        &electron,
        &job.teammate,
        runtime_name,
        &origin,
        continues_folder_mission.as_deref(),
    )?;
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
    // that variant's id; one it does not list stays, and is refused in words below.
    // With no model there is no family to look in, so the catalog is not read.
    let model = if runtime_name == "cursor" && job.effort.is_some() && model.is_some() {
        let (chosen, effort) = crate::models::cursor_choice_for(&crate::models::local(), model.as_deref(), job.effort.as_deref());
        job.effort = effort;
        chosen
    } else {
        model
    };
    // codex-mission.ts: options that cannot be turned into a command are refused before the mission is written, so no
    // record is left of a run that never existed. Cursor takes no effort level beyond its model's variants.
    if runtime_name == "cursor"
        && let Some(why) = crate::runtime::cursor::refusal(mode, job.effort.as_deref())
    {
        return Err(why.into());
    }
    // A switch starts the new agent with the brief, not the bare reply, and records the brief as the turn's words.
    if let Some(switch) = &switched {
        job.prompt = switch.prompt.clone();
    }
    // M16 (attachments-for-run.ts): the files the message attached, placed where this run reads them when it runs
    // anywhere but the project folder. Only what is sent changes; the record keeps the project's paths.
    // The project is the folder the person has open (codex-mission.ts runFolder): a teammate's own copy reads from its
    // repository, a column's copy from the folder it copied, and a teammate with a folder of its own from the open one.
    let project_folder = repository_root
        .clone()
        .or_else(|| origin.slot.as_ref().and_then(|s| s.cwd.as_ref()).and_then(|_| origin.folder.clone()))
        .or_else(|| workspace_folder(&electron));
    let sent_words = crate::attachments_for_run::attachments_for_run(&job.prompt, project_folder.as_deref().unwrap_or(&folder), &folder);
    // The references this turn may read without its mode changing (referenceAccess), named in its prompt and handed
    // to the agents that take a folder.
    let folder_text = folder.to_string_lossy().into_owned();
    // The conversation it continues: the prior turn, or (for a switch) the turn it was briefed from.
    let previous: Option<String> = continues
        .as_ref()
        .map(|c| c.mission_id.clone())
        .or_else(|| switched.as_ref().and_then(|s| s.link["missionId"].as_str().map(str::to_owned)))
        .or_else(|| rewind_link.as_ref().and_then(|l| l["missionId"].as_str().map(str::to_owned)));
    // A CONVERSATION WITH A KILO TEAMMATE KEEPS ONE KILO FOLDER (codex-mission.ts, 0.845). Kilo keeps its sessions in its
    // data folder, and every turn had the one folder, so a follow-up's session/load could not tell one conversation's
    // sessions from another's -- and a folder that never held the session answered "Internal error: OpenCode service
    // failure". A conversation's turns now share a folder in the profile, named for its first turn. One whose folder was
    // not there (begun before this, or on another computer) starts afresh, told what was said before. A first turn's
    // folder is named for it once it has its id, below.
    let mut kilo_home: Option<PathBuf> = None;
    if runtime_name == "kilo"
        && let Some(earlier) = previous.as_deref()
    {
        let (home, made) = crate::runtime::kilo::conversation_home(&store, &first_turn_of(earlier)).map_err(|e| e.to_string())?;
        if made && resume.is_some() {
            if origin.rewind.is_none() {
                cold = Some((cold_earlier(earlier, &job.prompt), false));
            }
            resume = None;
            // The record says this followed that, without claiming a resume that did not happen.
            if let Some(continuation) = continues.as_mut() {
                continuation.thread = None;
            }
        }
        kilo_home = Some(home);
    }
    let attached = crate::references::select(&store, previous.as_deref(), origin.reference_ids.as_deref())?;
    let shared: Vec<String> = attached
        .iter()
        .filter(|item| crate::references::access(runtime_name, mode.as_str(), &item.path, Some(&folder_text), false).0)
        .map(|item| item.path.clone())
        .collect();
    let references: Vec<&str> = shared.iter().map(String::as_str).collect();
    let mut delivered: Vec<crate::workroom::Message> = Vec::new();
    // What this turn's session will hold once it is sent (A2.5); none for a bare command, a side question or a
    // sealed run, whose next turn is then briefed whole.
    let mut brief_record: Option<crate::brief_sessions::BriefRecord> = None;
    // A command the person typed goes to the runtime bare (0.426): a CLI reads a slash command only at the very start of
    // what it is given, and a brief before it made `/compact` just words. Only the person's own message counts: never
    // words an app, a routine, a relay, a room or a comparison started, and never a reply on another agent (a brief of
    // the earlier turns starts that one cold, and there is no session of its own to act on).
    let typed_command = !is_side
        && switched.is_none()
        && origin.started_by.is_none()
        && origin.relay.is_none()
        && origin.slot.is_none()
        && crate::runtime_commands::for_store(&store).is_command(Some(runtime_name), &job.prompt);
    let command_name = if typed_command { crate::runtime_commands::command_named(&job.prompt) } else { None };
    let runtime_prompt = if let Some(side_text) = side_prompt {
        let refs = crate::references::section(&references);
        if refs.is_empty() {
            side_text
        } else {
            format!("{side_text}\n\n{refs}")
        }
    } else if typed_command {
        // Claude Code is sent the command itself. OpenCode takes it by name and only what follows as its message; Codex's
        // `review` and `compact` go as requests of their own (the name and message are not a turn), and `init` is a turn
        // carrying Locust's words for it.
        let named = command_name.as_deref().filter(|_| matches!(runtime_name, "opencode" | "codex"));
        match named {
            None => job.prompt.trim().to_owned(),
            Some(name) => {
                let after = crate::runtime_commands::after_name(&job.prompt, name);
                if runtime_name == "codex" && name == "init" {
                    [crate::runtime_commands::CODEX_INIT_PROMPT, after.as_str()].iter().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join("\n\n")
                } else {
                    after
                }
            }
        }
    } else if runtime_name == crate::runtime::apple::APPLE_RUNTIME {
        // Apple's on-device model gets a brief of its own, short and without the tag rules (runtime::apple::brief); a
        // cold follow-up is still told what was said.
        let words = match &cold {
            Some((earlier, edited)) => crate::carry_on::compose_cold_follow_up(earlier, &sent_words, *edited),
            None => sent_words.clone(),
        };
        let own = origin.slot.as_ref().and_then(|s| s.instructions.clone()).or_else(|| {
            crate::people::team(&store).ok().and_then(|file| {
                file["teammates"].as_array()?.iter().find(|t| t["teammateId"] == job.teammate.id.as_str())?["instructions"]
                    .as_str()
                    .map(str::to_owned)
            })
        });
        crate::runtime::apple::brief(&job.teammate.name, &job.teammate.role, own.as_deref(), &words)
    } else if sealed {
        let refs = crate::references::section(&references);
        let mut out = match origin
            .slot
            .as_ref()
            .and_then(|s| s.instructions.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(inst) => format!("{inst}\n\n{sent_words}"),
            None => sent_words.clone(),
        };
        if !refs.is_empty() {
            out = format!("{out}\n\n{refs}");
        }
        out
    } else {
        let slot_inst = origin.slot.as_ref().and_then(|s| s.instructions.as_deref());
        // What teammates left for this one, quoted into its brief (peer-exchange.ts prepare). A workroom that cannot be
        // read is left out: the run goes on without it, and the messages wait.
        let (waiting, remaining) = crate::workroom::Workroom::for_store(&store)
            .unread(&job.teammate.id, crate::briefing::MAX_INBOUND_MESSAGES, &origin.include_messages)
            .map(|u| (u.messages, u.remaining))
            .unwrap_or_default();
        // A cold start is told what was said before (an edit: before the point it went back to).
        let words = match &cold {
            Some((earlier, edited)) => crate::carry_on::compose_cold_follow_up(earlier, &sent_words, *edited),
            None => sent_words.clone(),
        };
        // A conversation's group briefs every turn after the folder's own and before the teammate's.
        let group_brief = crate::groups::group_brief_for_tip(&store, previous.as_deref());
        // A run in its own copy (worktree or comparison slot copy) stands apart from the project folder and is never
        // told the project folder's name or path (a-run-in-its-own-copy-is-not-told-the-project-folder).
        let own_folder = if repository_root.is_some() || origin.slot.as_ref().and_then(|s| s.cwd.as_ref()).is_some() {
            Some(folder.as_path())
        } else {
            None
        };
        let brief_folder = repository_root.as_deref().unwrap_or(&folder);
        // The whole memory list where the run stands, so the brief can name the file for what it does not paste.
        crate::people::write_brief_memory_file(&store, brief_folder, own_folder);
        // A2.5: a turn that resumes a session which did not compact is sent only what that session was not given.
        let resumed = continues.as_ref().filter(|c| c.thread.is_some() && resume.is_some());
        let earlier_brief = resumed
            .filter(|_| !resumed_compacted)
            .and_then(|c| crate::brief_sessions::BriefSessions::for_store(&store).after(&c.mission_id));
        let plan = crate::brief_sessions::brief_plan(resumed.is_some(), resumed_compacted, earlier_brief.as_ref());
        clock.mark(crate::start_timing::Phase::Briefing);
        let (brief, quoted, given) = crate::people::turn_brief_held(
            &store,
            &job.teammate.id,
            brief_folder,
            &words,
            slot_inst,
            group_brief
                .as_ref()
                .map(|g| (g.group_name.as_str(), g.instructions.as_str())),
            Some(runtime_name),
            &waiting,
            remaining,
            origin.room_history.as_ref(),
            own_folder,
            plan.already_given.as_ref(),
        )?;
        brief_record = Some(crate::brief_sessions::BriefRecord {
            given: crate::brief_sessions::brief_held(plan.already_given.as_ref(), &given),
            turns: plan.turns,
        });
        delivered = quoted;
        // A1.4: which memories this teammate was given, so one nobody is given in a month can say so.
        if let Err(error) = crate::people::note_brief_given(&store, brief_folder, own_folder, &words) {
            eprintln!("Which memories were briefed was not noted: {error}");
        }
        format!("{brief}\n\n{}", crate::references::section(&references))
    };
    // A follow-up to a turn the person undid starts with what Undo put back (undone-note.ts, 0.784): the session it
    // resumes still remembers writing those files. Not a side question, and not a sealed run.
    let runtime_prompt = match continues.as_ref().filter(|_| !is_side && !sealed && !typed_command).and_then(|c| crate::checkpoints::undone(&store, &folder, &c.mission_id)).and_then(|r| crate::undone_note::undone_note(&r)) {
        Some(told) => format!("{told}\n\n{runtime_prompt}"),
        None => runtime_prompt,
    };
    // codex-mission.ts: a read-only OpenCode run is told it has no shell (A2.20), and a Plan run is told to plan, last,
    // so it is the instruction closest to the answer; the sandbox is checked as well as the mode. Never after a
    // command the person typed, which goes to the runtime alone.
    let runtime_prompt = if typed_command {
        runtime_prompt
    } else {
        let mut told = runtime_prompt;
        if runtime_name == "opencode" && mode.sandbox() == "read-only" {
            told = format!("{told}\n\n{}", crate::briefing::opencode_read_only_section());
        }
        if mode == Mode::Plan && mode.sandbox() == "read-only" {
            told = format!("{told}\n\n{}", crate::briefing::plan_section());
        }
        told
    };
    let id = new_id();
    // Best effort: a record that is not written costs the next turn a full brief, and nothing else.
    if let Some(record) = &brief_record
        && let Err(error) = crate::brief_sessions::BriefSessions::for_store(&store).record(&id, record)
    {
        eprintln!("What this session was told was not recorded: {error}");
    }
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
    let skill_copies: Vec<PathBuf> = crate::skills::copies_of(&claude_plugins, run_skills.as_ref());
    let kilo_home = match kilo_home {
        None if runtime_name == "kilo" => Some(crate::runtime::kilo::conversation_home(&store, &id).map_err(|e| e.to_string())?.0),
        held => held,
    };
    // The conversation counts as busy for `/context` from here until this start returns, running or refused
    // (codex-mission.ts `contextStarting`): before the turn registers and writes its ledger row, no read starts.
    let _context_starting = job.conversation.as_deref().map(ContextStarting::on);
    // A Claude turn starting stops any `/context` read first, and waits for it (codex-mission.ts `beforeStart`).
    if runtime_name == "claude"
        && let Some(reader) = claude_context_reader()
    {
        reader.before_start();
    }
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
        sealed,
        kilo_home.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    // Kilo's own config and data are the conversation's folder, never the person's, whatever built the command.
    if let Some(home) = &kilo_home {
        for (key, value) in crate::runtime::kilo::isolation_env(home) {
            command.env(key, value);
        }
    }
    // openCodeModeEnv: a worktree run reaches its parent repository's `.git`; a run with no shell gets Locust's git tool.
    if runtime_name == "opencode" {
        let git_reads = std::env::current_exe().ok().map(|exe| vec![exe.to_string_lossy().into_owned(), "--git-reads".to_owned()]);
        let root = repository_root.as_deref().map(|r| r.to_string_lossy().into_owned());
        let at = folder.to_string_lossy().into_owned();
        crate::runtime::opencode_config::rewrite_config(&mut command, |config| {
            crate::runtime::opencode_config::for_run(config, mode, sealed, root.as_deref(), git_reads.as_deref(), &at)
        });
    }
    command.current_dir(&folder);
    for (key, value) in &launch_env {
        command.env(key, value);
    }
    // A model of your own runs as an OpenCode provider declared in the run's environment (own-models.ts).
    if runtime_name == "opencode"
        && let Some(own) = model.as_deref().filter(|m| crate::own_models::is_own_route_model(m))
    {
        crate::own_models::add_provider_env(&mut command, &store, own)?;
    }
    // A teammate's own keys (teammate_keys, 0.833): environment variables of its run, on an agent measured to pass them
    // to its commands, never in a sealed run, whose prompt holds other people's words. None kept: no keychain is asked.
    let run_keys = if !sealed && crate::teammate_keys::reaches(runtime_name) { crate::teammate_keys::for_store(&store).run_keys(&job.teammate.id) } else { Vec::new() };
    for key in &run_keys {
        command.env(&key.name, &key.value);
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
    let anchor = if is_side || is_slot || origin.rewind.is_some() {
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
        let register_teammate = side_slot.is_none() && compare_slot.is_none();
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
        let teammate_key = if register_teammate {
            crate::invariants::lock(&ACTIVE_TEAMMATES).push((
                job.teammate.id.clone(),
                id.clone(),
                folder.clone(),
            ));
            Some((job.teammate.id.clone(), id.clone()))
        } else {
            None
        };
        Ok(Registered(keys, account_key, teammate_key))
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
        let connectors: Vec<&str> = in_roster
            .filter(|_| !sealed)
            .and_then(|entry| entry["connectors"].as_array())
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let ask_every_connector = std::env::var("LOCUST_ASK_CONNECTORS").as_deref() == Ok("1")
            || crate::workspace_settings::parse_settings(&saved["settings"]).ask_connectors;
        command.args(claude_args_for_turn(
            crate::runtime::claude::PrintOptions {
                mode,
                bridge: bridge.as_ref().map(|b| b.config()),
                browser: browser_registration.as_ref().and_then(|r| r.claude_config()),
                guard_settings: Some(&guard),
                model: model.as_deref(),
                effort: job.effort.as_deref(),
                resume: resume.as_deref(),
                fork: is_side,
                references: &references,
                skills: &claude_skills,
                connectors: &connectors,
                ..Default::default()
            },
            ask_every_connector,
        )?);
    }
    let args: Vec<String> = command
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    // The launch kept for `/context` after the turn (codex-mission.ts `claudeContext.remember`): paths and flags only.
    if runtime_name == "claude"
        && let Some(reader) = claude_context_reader()
    {
        let launch = crate::claude_context::ClaudeContextLaunch {
            cwd: folder.to_string_lossy().into_owned(),
            executable_path: command.get_program().to_string_lossy().into_owned(),
            args: args.clone(),
            sandbox: mode.sandbox().to_owned(),
            permission: bridge.as_ref().map(|b| b.config().to_owned()),
            browser: browser_registration.as_ref().and_then(|r| r.claude_config()).map(str::to_owned),
            plugins: claude_plugins.iter().map(|p| (p.name.to_owned(), p.dir.to_string_lossy().into_owned())).collect(),
        };
        if let Err(error) = reader.remember(&id, launch) {
            eprintln!("The Claude launch for the context menu was not kept: {error}");
        }
    }
    // The mode and sandbox the turn really runs in, where it stands, and who started it, as Electron records them.
    let turn_workspace_id = crate::workspace_settings::workspace_id_for(repository_root.as_deref().unwrap_or(&folder));
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
    if runtime_name == "opencode" {
        let cli_version = command
            .get_envs()
            .find(|(k, _)| *k == "LOCUST_NATIVE_OPENCODE_VERSION")
            .and_then(|(_, v)| v.map(|v| v.to_string_lossy().into_owned()))
            .or_else(|| crate::runtime::opencode::uses_acp(&command).then(|| "2.0.0".to_owned()));
        if let Some(version) = cli_version {
            extra["cliVersion"] = json!(version);
        }
    }
    if let Some(acc) = &effective_account {
        extra["account"] = json!(acc);
    }
    if let Some(by) = &origin.started_by {
        extra["startedBy"] = by.clone();
    }
    if let Some(relay_val) = &origin.relay {
        extra["relay"] = relay_val.clone();
    }
    if let Some(link) = &rewind_link {
        extra["continuesFrom"] = link.clone();
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
    let mut writer = LedgerWriter::create_runtime_with(
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
    // The record is made before the runtime starts, so nothing ran: said as ledgerFailure.ts says it, the host's own
    // reason quoted and Locust named as the one that would not start work it could not record.
    .map_err(|e| crate::ledger_failure::sentence(Some(&format!("{e}."))))?;
    // Whatever the run says or does that holds a key's value has it taken out before it is written.
    writer.take_out(run_keys);
    // An edit partway through sets turns aside: what they posted to a teammate and nobody read is never delivered
    // (0.512). Editing the first message keeps the old conversation as it was, so nothing of it is retired.
    if let Some(rewind) = origin.rewind.as_ref().filter(|r| r.before.is_some())
        && let Err(error) = crate::edit_earlier::retire_set_aside_messages(&crate::workroom::Workroom::for_store(&store), &rewind.set_aside)
    {
        // Left waiting on a failure: a stale message delivered is recoverable, a workroom broken is not.
        eprintln!("The messages the set-aside turns sent were not retired: {error}");
    }
    for msg in &delivered {
        let _ = writer.write_peer_link("received", &msg.message_id, &msg.from.teammate_id, &msg.posted_at);
    }
    // What the brief quoted is delivered once the runtime has begun (codex-mission.ts), never before: a run refused at
    // once, or one that only says why it could not start, leaves the messages waiting for the next real turn.
    if !delivered.is_empty() {
        let ids: Vec<String> = delivered.iter().map(|m| m.message_id.clone()).collect();
        let (delivery_store, delivery_mission) = (store.clone(), id.clone());
        writer.when_began(Box::new(move || {
            if let Err(error) = crate::workroom::Workroom::for_store(&delivery_store).mark_delivered(&ids, &delivery_mission) {
                eprintln!("The teammate messages were not marked delivered, so they will be shown again: {error}");
            }
        }));
    }
    // The first word from the runtime dates the end of the start.
    let began_clock = clock.clone();
    writer.when_began(Box::new(move || began_clock.began()));
    // A turn for nobody stays nobody's: no owner, and no teammate a card's allow rule could be saved for.
    if !is_nobody(&job.teammate) {
        channel.register_teammate(&id, &job.teammate.id);
        if let Err(error) = crate::people::record_owner(&store, &id, &job.teammate.id) {
            eprintln!("The conversation's teammate was not recorded: {error}");
        }
        // A start remembers the route it used, for a teammate that has none yet (index.ts startMission): a relay's
        // hop, a comparison's column and a side question are not starts of the teammate's own.
        let own_start = origin.relay.is_none() && origin.slot.is_none() && !is_side;
        if own_start
            && let Err(error) = crate::people::remember_route_of_start(&store, &job.teammate.id, &asked_route)
        {
            eprintln!("The route this turn started on was not remembered: {error}");
        }
    }
    // The conversation keeps these references for its next turns (bind), with the folder this turn stood in.
    let attached_ids: Vec<String> = attached.iter().map(|i| i.id.clone()).collect();
    if let Err(error) = crate::references::bind(&store, &id, previous.as_deref(), &attached_ids, Some(&folder_text)) {
        eprintln!("The conversation's references were not kept: {error}");
    }
    // Its GitHub copies are not refreshed or collected under it until the run ends (or its start fails).
    let reference_lease = crate::references::lease(&store, &id, &attached_ids);
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
    // Codex's `review` and `compact` and every OpenCode command go to the runtime as the command itself.
    let slash_command = command_name.clone().filter(|name| match runtime_name {
        "opencode" => true,
        "codex" => matches!(name.as_str(), "review" | "compact"),
        _ => false,
    });
    // Only a turn the person sent from the chat box is carried on without a press: never one an app started, nor a
    // turn Locust carried on itself, so one empty plan cannot start a chain of them.
    let person_sent = origin.started_by.is_none();
    let memory_store = store.clone();
    let memory_who = job.teammate.id.clone();
    let memory_name = job.teammate.name.clone();
    let memory_folder = turn.folder.clone();
    let memory_id = id.clone();
    let memory_prompt = job.prompt.clone();
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
        Some("relay" | "side" | "compare" | "judge" | "scout")
    );
    let relay_origin = origin.relay.as_ref().and_then(crate::relay::RelayOrigin::from_json);
    let relay_started_by = origin
        .started_by
        .as_ref()
        .and_then(crate::relay::MissionStarter::from_json);
    let relay_teammate = crate::people::peer_roster_entry(&store, &job.teammate.id);
    // Counted before the spawn, so a test's guard can never look between the spawn and the count.
    #[cfg(any(test, feature = "test-hooks"))]
    let counted = TurnThread::begin();
    std::thread::Builder::new()
        .name("native-turn".into())
        .spawn(move || {
            // Dropped last of all, when the closure's whole ending is done.
            #[cfg(any(test, feature = "test-hooks"))]
            let _counted = counted;
            // Keep ownership through the final memory write so shutdown/continuation cannot race it.
            let memory_completion = registered;
            let _reference_lease = reference_lease;
            let _browser_registration = browser_registration;
            let checkpoint = crate::checkpoints::Turn::begin(&memory_store, &memory_folder);
            let cursor_guard = crate::cursor_config::CursorRunGuard::begin(memory_store.root(), &runtime_owned, &memory_folder, check_mode, check_model.as_deref());
            let disk_before = if check_mode.sandbox() == "read-only" {
                None
            } else {
                clock.mark(crate::start_timing::Phase::ReadingFolder);
                crate::disk_observation::snapshot_workspace(
                    &memory_folder,
                    &crate::disk_observation::DefaultDiskHost::new(),
                )
            };
            clock.mark(crate::start_timing::Phase::StartingRuntime);
            let timing_runtime = crate::engine::runtime_name(&runtime_owned).to_owned();
            run_turn(
                writer,
                command,
                turn,
                flag,
                Registered(Vec::new(), None, None),
                runtime_owned,
                channel,
                bridge,
                store,
                is_side,
                sealed,
                codex_home,
                slash_command,
            );
            cursor_guard.end();
            // The run's process has exited: leave ACTIVE before taking the after-snapshot or firing the relay,
            // so a relayed reply or a run started while the disk is still being read is not marked as sharing the folder.
            checkpoint.leave_active();
            // The run's skill copies go with it.
            crate::skills::remove_copies(&skill_copies);
            crate::skills::forget_roots(&memory_id);
            let changed = record_disk_observation(&memory_store, &memory_id, &memory_folder, disk_before, &checkpoint);
            // Where the seconds before "started" went: one host note after the run's own events.
            if let Ok(path) = memory_store.write_path(&memory_id) {
                let timing = clock.timing();
                let _ = crate::ledger_write::append_events(&path, |run, mission, adapter, next_seq, at| {
                    crate::start_timing::note(run, mission, adapter, next_seq, at, &timing_runtime, &timing).into_iter().collect()
                });
            }
            // A2.9: kept for the overlap note in another teammate's next brief here (recent_edits).
            if !memory_who.is_empty() {
                crate::recent_edits::record(&memory_folder.to_string_lossy(), &memory_who, &memory_name, &changed);
            }
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
            let mut posted_relayed = Vec::new();
            // The run's end in order, each step on its own (run-end.ts): memory first and once, then the relay.
            let mut run_end = crate::run_end::RunEnd::new(memory_id.clone(), |line| eprintln!("{line}"));
            if let Ok(path) = memory_store.write_path(&memory_id) {
                let transcript = ledger::read_transcript(&path);
                completed = !transcript.failed
                    && !transcript.running
                    && !transcript.items.iter().any(
                        |item| matches!(item, ledger::Item::Note { text, .. } if text == "Stopped"),
                    );
                if completed && !is_side && !sealed {
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
                    run_end.remember(|| {
                        let (said, changed) = crate::people::record_reply_changes(&memory_store, &memory_who, &memory_folder, &reply, Some(&memory_id), Some(&memory_prompt))?;
                        for (line, level) in said {
                            push_relay_notice(&memory_id, &line, level);
                        }
                        push_memory_changed(changed);
                        Ok(())
                    });
                    // What the reply shares with a teammate goes to the workroom, to wait for that teammate's next run,
                    // and may automatically start or steer the recipient through the relay.
                    let (posted, relayed, notices) = crate::people::share_reply_for_relay(
                        &memory_store,
                        &memory_who,
                        &memory_id,
                        &reply,
                        observed.as_deref(),
                    );
                    if !posted.is_empty() {
                        let links: Vec<(&str, &str, &str, &str)> = posted
                            .iter()
                            .map(|msg| {
                                (
                                    "posted",
                                    msg.message_id.as_str(),
                                    msg.to.teammate_id.as_str(),
                                    msg.posted_at.as_str(),
                                )
                            })
                            .collect();
                        if let Err(error) = crate::ledger_write::append_peer_links(&path, &links) {
                            eprintln!("Teammate share links were not recorded: {error}");
                        }
                    }
                    posted_relayed = relayed;
                    for said in notices {
                        push_relay_notice(&memory_id, &said, "warning");
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
            // A side question, and any turn with a slot of its own (a comparison's column, a judge, a scout chat), never
            // commits to a teammate's branch, with a folder of its own or without (Electron codex-mission.ts
            // `mission.slot === undefined`).
            if !is_side && !is_slot && let Some(root) = &checkpoint_root {
                checkpoint_turn(&memory_store, &memory_id, root, &check_teammate, &checkpoint_prompt, &check_runtime, check_model.as_deref());
            }
            // The turn is no longer this window's: a carry-on or check-fix may now continue its conversation,
            // and the relay may hand off posted messages or return unanswered questions.
            #[cfg(any(test, feature = "test-hooks"))]
            while hooks::HOLD_WIND_DOWN.load(Ordering::Relaxed) {
                std::thread::sleep(Duration::from_millis(5));
            }
            drop(memory_completion);
            if !posted_relayed.is_empty()
                && let Some(peer) = relay_teammate.clone()
            {
                let sharing = crate::relay::SharingMission {
                    run_id: format!("run_{memory_id}"),
                    mission_id: memory_id.clone(),
                    runtime: check_runtime.clone(),
                    sandbox: check_mode.sandbox().to_owned(),
                    model: check_model.clone(),
                    peer,
                    relay: relay_origin.clone(),
                    started_by: relay_started_by.clone(),
                };
                // Memory was applied where the reply was read, before the turn's slot was released.
                run_end.shared(|| Ok(()), || {
                    relay().on_shared(&sharing, &posted_relayed);
                    Ok(())
                });
            }
            if !is_side {
                let ended = crate::relay::EndedMission { mission_id: memory_id.clone(), peer: relay_teammate, relay: relay_origin };
                run_end.ended(
                    || Ok(()),
                    || {
                        relay().on_run_ended(&ended);
                        Ok(())
                    },
                    vec![(
                        "room tasks",
                        Box::new(|| {
                            crate::rooms::on_run_ended(&memory_store, &memory_id);
                            Ok(())
                        }),
                    )],
                );
            }
            // After the relay's and rooms' own endings, before Scout's (main/index.ts run-end steps).
            crate::approval_record_note::on_run_ended(&memory_store, &memory_id);
            crate::scout::host::on_run_ended(&memory_store, &memory_id);
            crate::chat_host::on_run_ended(&memory_id);
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
                ..Default::default()
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
    let mut command = command_for_turn(runtime_name, folder, model.as_deref(), Mode::Ask, None, prompt, None, session.as_deref(), &[], None, false, None)
        .map_err(|e| e.to_string())?;
    command.current_dir(folder);
    for (key, value) in &launch_env {
        command.env(key, value);
    }
    if runtime_name == "opencode"
        && let Some(own) = model.as_deref().filter(|m| crate::own_models::is_own_route_model(m))
    {
        crate::own_models::add_provider_env(&mut command, &store, own)?;
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
        Registered(Vec::new(), None, None),
        runtime_name.to_owned(),
        approvals_channel(),
        None,
        panel.clone(),
        false,
        false,
        codex_home,
        None,
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

/// Every card answered is written to the turn's ledger; an answer the ledger refused is remembered, to be written (or
/// said) when the run ends (approval-record-note.ts, `unwrittenAnswers.remember` in main/index.ts recordAnswer).
fn flush_approvals(writer: &mut LedgerWriter, channel: &Channel, mission_id: &str) {
    for recorded in channel.take_recorded_for(mission_id) {
        let approval = recorded.to_json();
        let written = match crate::approval_record_note::refused_for_test() {
            Some(why) => Err(why),
            None => writer.append_approval(&approval).map_err(|e| e.to_string()),
        };
        if let Err(why) = written {
            crate::invariants::lock(&crate::approval_record_note::UNWRITTEN).remember(mission_id, approval, &why);
        }
    }
}

/// index.ts `onDeniedSaying`: a denial the person explained reaches the run as its next input, in their words; a run
/// that cannot take one just then is said so. Codex's app-server steers its own (runtime::codex).
fn pass_on_denials(runtime: &mut dyn Runtime, channel: &Channel, mission_id: &str) {
    for denied in channel.take_denied_saying_for_mission(mission_id) {
        if !runtime.steer(&format!("I declined that. {}", denied.reason)) {
            push_relay_notice(
                mission_id,
                "Your reason was not passed on: the run could not take a message just then. Say it in your next message.",
                "warning",
            );
        }
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
    sealed: bool,
    codex_home: Option<PathBuf>,
    slash: Option<String>,
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
            &store,
        );
        return;
    }
    if runtime_name == "opencode" {
        let mut runtime = match OpenCode::spawn(&mut command) {
            Ok(r) => r.with_cancel(flag.clone()).with_fork(fork).with_command(slash.clone()),
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
            &store,
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
            &store,
        );
        return;
    }
    if runtime_name == "claude" {
        let mut runtime = match Claude::launch(&mut command, &flag) {
            Ok(r) => r,
            Err(e) => {
                record_failure(&mut writer, flag.load(Ordering::Relaxed), &e);
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
            &store,
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
        // A read Cursor is refused is explained where it happens (`explainADeniedRead`): its tools say only
        // "permission denied", and the rule in `.cursorignore` that hides the folder is named beside the failure.
        let mut explained = crate::cursor_ignore::ExplainsDeniedReads::new(&mut runtime, &turn.folder, std::env::var_os("HOME").map(PathBuf::from));
        drive_runtime(
            &mut explained,
            &mut writer,
            &flag,
            &mut registered,
            &channel,
            &mission_id,
            &store,
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
            &store,
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
            &store,
        );
        return;
    }
    if runtime_name == "apple" {
        let mut runtime = match crate::runtime::apple::Apple::spawn(&mut command) {
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
        drive_runtime(&mut runtime, &mut writer, &flag, &mut registered, &channel, &mission_id, &store);
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
            &store,
        );
        return;
    }
    // No teammate ends the person's browser, Codex too (0.720): the guard as a hook in this thread's config.
    let guard = crate::command_guard::codex_config_for_home(&turn.folder, codex_home.as_deref());
    let mut runtime = match Codex::spawn(&mut command) {
        Ok(r) => r
            .with_cancel(flag.clone())
            .with_thread_config(guard)
            .with_fork(fork)
            .with_sealed(sealed)
            .with_command(slash.clone()),
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
        &store,
    );
}

fn drive_runtime(
    runtime: &mut dyn Runtime,
    writer: &mut LedgerWriter,
    flag: &Arc<AtomicBool>,
    registered: &mut Option<Registered>,
    channel: &Channel,
    mission_id: &str,
    store: &Store,
) {
    if runtime.can_steer() {
        crate::invariants::lock(&STEER_CAPABLE)
            .get_or_insert_with(std::collections::HashSet::new)
            .insert(mission_id.to_owned());
    }
    let finish_steer = || {
        if let Some(capable) = crate::invariants::lock(&STEER_CAPABLE).as_mut() {
            capable.remove(mission_id);
        }
        if let Some(queues) = crate::invariants::lock(&STEER_QUEUES).as_mut() {
            queues.remove(mission_id);
        }
    };
    let mut stopped = false;
    loop {
        flush_approvals(writer, channel, mission_id);
        pass_on_denials(runtime, channel, mission_id);
        if !stopped && flag.load(Ordering::Relaxed) {
            stopped = true;
            let _ = runtime.stop();
        }
        if !stopped {
            let queued = crate::invariants::lock(&STEER_QUEUES)
                .as_mut()
                .and_then(|q| q.remove(mission_id));
            if let Some(msgs) = queued {
                for msg in msgs {
                    let _ = runtime.steer(&msg);
                }
            }
        }
        let event = runtime.next_event();
        // Claude Code's own commands, announced at the start of the run, kept once outside the mission's record.
        if let Some(listed) = runtime.learned_commands() {
            crate::runtime_commands::keep_learned("claude", &listed);
        }
        match event {
            Ok(Some(event)) => {
                if event.kind == "adapter.diagnostic" {
                    crate::scout::host::on_diagnostic(store, mission_id, &event.payload);
                }
                let done = terminal(&event.kind);
                let written = writer.append(&event.kind, event.payload, event.thread.as_deref());
                // A record that cannot be written stops the run (returning drops the runtime, which ends its process),
                // and the thread says so in ledgerFailure.ts's words: the run was stopped, never held.
                if let Err(error) = &written {
                    for line in crate::ledger_failure::lines(Some(&format!("{error}.")), None, false) {
                        push_relay_notice(mission_id, &line, "warning");
                    }
                }
                if written.is_err() || done {
                    flush_approvals(writer, channel, mission_id);
                    pass_on_denials(runtime, channel, mission_id);
                    finish_steer();
                    channel.release(mission_id);
                    drop(registered.take());
                    return;
                }
            }
            Ok(None) => {}
            Err(e) => {
                record_failure(writer, stopped, &e);
                flush_approvals(writer, channel, mission_id);
                pass_on_denials(runtime, channel, mission_id);
                finish_steer();
                channel.release(mission_id);
                drop(registered.take());
                return;
            }
        }
    }
}

/// Path to the persisted Claude Code background runs file inside the native data folder.
pub fn background_runs_path(store: &Store) -> PathBuf {
    store.root().join("claude-background.json")
}

/// Host adapter connecting `claude_background::BackgroundRuns` to Locust's native store,
/// Claude CLI runner, and terminal catch-up.
pub struct NativeBackgroundHost {
    store: Store,
    program: Option<String>,
    prefix_args: Vec<String>,
    extra_env: Vec<(String, String)>,
}

impl crate::claude_background::BackgroundHost for NativeBackgroundHost {
    fn run_claude(
        &self,
        args: &[String],
        cwd: &Path,
        account: Option<&str>,
    ) -> crate::claude_background::ClaudeCliOutput {
        let mut cmd = if let Some(ref prog) = self.program {
            let mut c = Command::new(prog);
            c.args(&self.prefix_args);
            c
        } else {
            match crate::runtime::claude::bare_command(cwd) {
                Ok(c) => c,
                Err(e) => {
                    return crate::claude_background::ClaudeCliOutput {
                        code: 127,
                        stdout: String::new(),
                        stderr: format!(
                            "Claude Code is not installed or not signed in on this computer: {e}"
                        ),
                    };
                }
            }
        };
        if let Some(account_id) = account {
            if let Ok(env_pairs) = account_launch_env(&self.store, "claude", Some(account_id)) {
                for (k, v) in env_pairs {
                    cmd.env(k, v);
                }
            }
        }
        for (k, v) in &self.extra_env {
            cmd.env(k, v);
        }
        crate::runtime::claude::run_background_cli(cmd, cwd, args)
    }

    fn session_of(&self, conversation: &str, _account: Option<&str>) -> Option<String> {
        let newest = real_tip(conversation).unwrap_or_else(|_| conversation.to_string());
        let (meta, rows) = crate::runtime_terminal::mission(&newest).ok()?;
        if meta["runtime"].as_str() != Some("claude") {
            return None;
        }
        crate::runtime_terminal::session(&rows)
    }

    fn bring_in(
        &self,
        run: &crate::claude_background::BackgroundRun,
    ) -> crate::claude_background::BringInOutcome {
        if let Some(ref conversation) = run.conversation {
            match crate::runtime_terminal::catch_up(conversation) {
                Ok(imported) if imported > 0 => {
                    let tip = real_tip(conversation).unwrap_or_else(|_| conversation.clone());
                    return crate::claude_background::BringInOutcome::BroughtIn(tip);
                }
                Ok(_) => {
                    return crate::claude_background::BringInOutcome::Refused(
                        "Its turn was not found in Claude Code's record of the conversation."
                            .to_string(),
                    );
                }
                Err(err) => {
                    return crate::claude_background::BringInOutcome::Refused(err);
                }
            }
        }
        if run.session_id.is_none() {
            return crate::claude_background::BringInOutcome::Refused(
                "Claude Code never said which session it was.".to_string(),
            );
        }
        crate::claude_background::BringInOutcome::Refused(
            "Its turn was not found in Claude Code's record of the conversation.".to_string(),
        )
    }
}

/// Creates a `BackgroundRuns` manager backed by `store`'s `claude-background.json` and real Claude CLI.
pub fn background_runs_for(
    store: &Store,
) -> crate::claude_background::BackgroundRuns<NativeBackgroundHost> {
    crate::claude_background::BackgroundRuns::new(
        background_runs_path(store),
        NativeBackgroundHost {
            store: store.clone(),
            program: None,
            prefix_args: Vec::new(),
            extra_env: Vec::new(),
        },
    )
}

/// Same as `background_runs_for`, with an explicit executable and prefix args (used by tests with fake scripts).
pub fn background_runs_for_with_program(
    store: &Store,
    program: &str,
    prefix_args: Vec<String>,
    extra_env: Vec<(String, String)>,
) -> crate::claude_background::BackgroundRuns<NativeBackgroundHost> {
    crate::claude_background::BackgroundRuns::new(
        background_runs_path(store),
        NativeBackgroundHost {
            store: store.clone(),
            program: Some(program.to_string()),
            prefix_args,
            extra_env,
        },
    )
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "long_history_tests.rs"]
mod long_history_tests;

#[cfg(test)]
#[path = "torn_words_tests.rs"]
mod torn_words_tests;

#[cfg(test)]
#[path = "last_activity_tests.rs"]
mod last_activity_tests;

#[cfg(test)]
#[path = "cold_start_tests.rs"]
mod cold_start_tests;

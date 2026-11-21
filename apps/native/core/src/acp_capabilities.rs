//! WHAT EACH ACP AGENT LAST SAID IT CAN DO (Electron `main/acp-capabilities.ts`, W12, 0.566).
//!
//! An Agent Client Protocol agent answers `initialize` with its capabilities (`acp-run.ts` `acpCapabilitiesOf`); this
//! keeps the last answer per runtime, in memory and in `acp-capabilities.json` beside the other profile files, so
//! Settings > AI agents can say it after a relaunch. Shown only: nothing here changes what Locust offers an agent (no
//! files, no terminal). A lost write costs one sentence until the next run, so nothing here fails loudly, and the file
//! is untrusted on read like every other.
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub const FILE_NAME: &str = "acp-capabilities.json";
const SCHEMA_VERSION: u64 = 1;
/// The six things an agent says, in Electron's order (`KEYS`).
pub const KEYS: [&str; 6] = ["continuesSessions", "images", "audio", "embeddedContext", "mcpOverHttp", "mcpOverSse"];
/// The runtimes whose ACP runs say what they can do (codex-mission.ts passes `onCapabilities` for these three only).
pub const SAID_BY: [&str; 3] = ["kilo", "opencode", "copilot"];

/// What an agent's `initialize` answer says it can do (`acpCapabilitiesOf`): each one true only when said true.
pub fn capabilities_of(agent_capabilities: &Value) -> [bool; 6] {
    let given = &agent_capabilities;
    let prompt = &given["promptCapabilities"];
    let mcp = &given["mcpCapabilities"];
    [
        given["loadSession"] == true,
        prompt["image"] == true,
        prompt["audio"] == true,
        prompt["embeddedContext"] == true,
        mcp["http"] == true,
        mcp["sse"] == true,
    ]
}

/// `Number.isNaN(Date.parse(toldAt))` read the other way: an ISO time, or a bare date.
fn a_time(told_at: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(told_at).is_ok() || chrono::NaiveDate::parse_from_str(told_at, "%Y-%m-%d").is_ok()
}

/// The file's runtimes, entry by entry: an entry missing a boolean or a real `toldAt` is read as nothing, and so is a
/// whole file that is not JSON, not version 1, or has no `runtimes` object.
fn parsed(text: &str) -> Map<String, Value> {
    let mut out = Map::new();
    let Ok(value) = serde_json::from_str::<Value>(text) else { return out };
    if value["schemaVersion"].as_u64() != Some(SCHEMA_VERSION) || !value["schemaVersion"].is_number() {
        return out;
    }
    let Some(runtimes) = value["runtimes"].as_object() else { return out };
    for (runtime, entry) in runtimes {
        let Some(told_at) = entry["toldAt"].as_str().filter(|t| a_time(t)) else { continue };
        if !KEYS.iter().all(|key| entry[*key].is_boolean()) {
            continue;
        }
        let mut kept = Map::new();
        for key in KEYS {
            kept.insert(key.into(), entry[key].clone());
        }
        kept.insert("toldAt".into(), json!(told_at));
        out.insert(runtime.clone(), Value::Object(kept));
    }
    out
}

/// The store (`createAcpCapabilitiesStore`): `load` replaces what is known with the file's, `record` keeps an answer
/// and writes the file, `get` is what Settings is given.
pub struct AcpCapabilitiesStore {
    path: PathBuf,
    now: fn() -> String,
    known: Map<String, Value>,
}

/// Now, as JavaScript's `toISOString` writes it: milliseconds and `Z`.
pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

impl AcpCapabilitiesStore {
    pub fn new(root: &Path, now: fn() -> String) -> Self {
        Self { path: root.join(FILE_NAME), now, known: Map::new() }
    }
    pub fn load(&mut self) {
        self.known = parsed(&std::fs::read_to_string(&self.path).unwrap_or_default());
    }
    pub fn record(&mut self, runtime: &str, said: [bool; 6]) {
        let mut next = Map::new();
        for (key, value) in KEYS.iter().zip(said) {
            next.insert((*key).into(), json!(value));
        }
        next.insert("toldAt".into(), json!((self.now)()));
        self.known.insert(runtime.into(), Value::Object(next));
        self.persist();
    }
    pub fn get(&self, runtime: &str) -> Option<Value> {
        self.known.get(runtime).cloned()
    }
    /// Written whole beside the file and moved over it, so a reader never sees half of one; a failure is dropped.
    fn persist(&self) {
        let body = json!({"schemaVersion": SCHEMA_VERSION, "runtimes": Value::Object(self.known.clone())});
        let Ok(body) = serde_json::to_string_pretty(&body) else { return };
        let temp = PathBuf::from(format!("{}.{}.tmp", self.path.display(), std::process::id()));
        if std::fs::write(&temp, body).is_ok() && std::fs::rename(&temp, &self.path).is_err() {
            let _ = std::fs::remove_file(&temp);
        }
    }
}

/// This app's store, in its own data folder, loaded the first time it is wanted (index.ts loads it at launch).
pub fn shared() -> Option<&'static Mutex<AcpCapabilitiesStore>> {
    static SHARED: OnceLock<Option<Mutex<AcpCapabilitiesStore>>> = OnceLock::new();
    SHARED
        .get_or_init(|| {
            let store = crate::store::Store::local().ok()?;
            let root = store.check(store.root()).ok()?;
            let mut kept = AcpCapabilitiesStore::new(&root, now_iso);
            kept.load();
            Some(Mutex::new(kept))
        })
        .as_ref()
}

/// What `runtime`'s agent said at `initialize`, kept when it is one of the runtimes that say it; never fails a turn.
pub fn record_said(runtime: &str, agent_capabilities: &Value) {
    if !SAID_BY.contains(&runtime) {
        return;
    }
    if let Some(store) = shared()
        && let Ok(mut store) = store.lock()
    {
        store.record(runtime, capabilities_of(agent_capabilities));
    }
}

/// What `runtime`'s agent said at its last run, with when, for the agent's row in Settings.
pub fn said_by(runtime: &str) -> Option<Value> {
    shared()?.lock().ok()?.get(runtime)
}

#[cfg(test)]
#[path = "acp_capabilities_tests.rs"]
mod acp_capabilities_tests;

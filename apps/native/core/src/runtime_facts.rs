//! WHAT EACH AGENT'S CLI SAID ABOUT ITSELF, KEPT BETWEEN LAUNCHES (Electron `main/runtime-facts.ts`).
//!
//! `--version` and `--help` are functions of the file on disk: the same file prints the same thing every time, and each
//! costs a CLI start on a sweep that runs at every launch. Discovery decides what is still true -- it fingerprints the
//! files a launch touches (`agents::fingerprint_of`) and only asks this for a fingerprint it has -- so this is a plain
//! map, in memory while the app runs and in `runtime-facts.json` between runs.
//!
//! Everything here is an optimisation, so nothing here fails loudly: a file that cannot be read is an empty map, and
//! one that cannot be written costs one extra probe next launch, which is what the app did before this existed.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use serde_json::{Value, json};

use crate::agents::{BinaryFacts, FactsCache};

/// How many binaries are remembered: a fingerprint changes on every CLI update, so entries gather one per update per
/// agent. When it is full the oldest go, because the newest fingerprints are the files on disk now.
pub const MAX_REMEMBERED_BINARIES: usize = 200;
const SCHEMA_VERSION: u64 = 1;
pub const FILE_NAME: &str = "runtime-facts.json";

#[derive(Clone, Debug, PartialEq)]
struct Entry {
    version_text: String,
    capability_text: String,
    /// When this was learned, so the cap drops the oldest.
    at: u64,
}

/// The store (`createRuntimeFactsStore`): `load` once before the first sweep, then `get` and `set` from any thread.
pub struct RuntimeFactsStore {
    path: PathBuf,
    now: Box<dyn Fn() -> u64 + Send + Sync>,
    held: Mutex<HashMap<String, Entry>>,
}

/// Untrusted on read, like every file in the profile: an entry of the wrong shape is dropped, never parsed loosely.
fn parse_entries(text: &str) -> Vec<(String, Entry)> {
    let Ok(value) = serde_json::from_str::<Value>(text) else { return Vec::new() };
    if value["schemaVersion"].as_u64() != Some(SCHEMA_VERSION) {
        return Vec::new();
    }
    let Some(binaries) = value["binaries"].as_array() else { return Vec::new() };
    let mut entries = Vec::new();
    for raw in binaries {
        let (Some(fingerprint), Some(version_text), Some(capability_text)) =
            (raw["fingerprint"].as_str(), raw["versionText"].as_str(), raw["capabilityText"].as_str())
        else {
            continue;
        };
        if fingerprint.is_empty() {
            continue;
        }
        let at = raw["at"].as_u64().unwrap_or(0);
        entries.push((fingerprint.to_owned(), Entry { version_text: version_text.to_owned(), capability_text: capability_text.to_owned(), at }));
        if entries.len() >= MAX_REMEMBERED_BINARIES {
            break;
        }
    }
    entries
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

impl RuntimeFactsStore {
    pub fn new(root: &Path, now: Box<dyn Fn() -> u64 + Send + Sync>) -> Self {
        Self { path: root.join(FILE_NAME), now, held: Mutex::new(HashMap::new()) }
    }

    /// Read the file. No file, or a file that is not this schema's, is an empty map.
    pub fn load(&self) {
        let Ok(text) = std::fs::read_to_string(&self.path) else { return };
        let mut held = crate::invariants::lock(&self.held);
        for (fingerprint, entry) in parse_entries(&text) {
            held.insert(fingerprint, entry);
        }
    }

    /// How many binaries are remembered.
    pub fn size(&self) -> usize {
        crate::invariants::lock(&self.held).len()
    }

    /// Written whole beside the file and moved over it, newest last; a failure is dropped.
    fn persist(&self, held: &HashMap<String, Entry>) {
        let mut binaries: Vec<(&String, &Entry)> = held.iter().collect();
        binaries.sort_by_key(|(_, e)| e.at);
        let from = binaries.len().saturating_sub(MAX_REMEMBERED_BINARIES);
        let binaries: Vec<Value> = binaries[from..]
            .iter()
            .map(|(fingerprint, e)| json!({"fingerprint": fingerprint, "versionText": e.version_text, "capabilityText": e.capability_text, "at": e.at}))
            .collect();
        let Ok(body) = serde_json::to_string(&json!({"schemaVersion": SCHEMA_VERSION, "binaries": binaries})) else { return };
        let temp = PathBuf::from(format!("{}.{}.tmp", self.path.display(), std::process::id()));
        if std::fs::write(&temp, body).is_ok() && std::fs::rename(&temp, &self.path).is_err() {
            let _ = std::fs::remove_file(&temp);
        }
    }
}

impl FactsCache for RuntimeFactsStore {
    fn get(&self, fingerprint: &str) -> Option<BinaryFacts> {
        crate::invariants::lock(&self.held)
            .get(fingerprint)
            .map(|e| BinaryFacts { version_text: e.version_text.clone(), capability_text: e.capability_text.clone() })
    }

    fn set(&self, fingerprint: &str, facts: BinaryFacts) {
        let mut held = crate::invariants::lock(&self.held);
        if let Some(existing) = held.get(fingerprint)
            && existing.version_text == facts.version_text
            && existing.capability_text == facts.capability_text
        {
            return;
        }
        let at = (self.now)();
        held.insert(fingerprint.to_owned(), Entry { version_text: facts.version_text, capability_text: facts.capability_text, at });
        // Bounded in memory as well as on disk: this map lives for the whole session.
        if held.len() > MAX_REMEMBERED_BINARIES {
            let mut by_age: Vec<(String, u64)> = held.iter().map(|(k, e)| (k.clone(), e.at)).collect();
            by_age.sort_by_key(|(_, at)| *at);
            let over = held.len() - MAX_REMEMBERED_BINARIES;
            for (fingerprint, _) in by_age.into_iter().take(over) {
                held.remove(&fingerprint);
            }
        }
        self.persist(&held);
    }
}

/// This app's store, in its own data folder, read the first time it is wanted (index.ts reads it at launch).
pub fn shared() -> Option<&'static RuntimeFactsStore> {
    static SHARED: OnceLock<Option<RuntimeFactsStore>> = OnceLock::new();
    SHARED
        .get_or_init(|| {
            let store = crate::store::Store::local().ok()?;
            let root = store.check(store.root()).ok()?;
            let kept = RuntimeFactsStore::new(&root, Box::new(now_ms));
            kept.load();
            Some(kept)
        })
        .as_ref()
}

#[cfg(test)]
#[path = "runtime_facts_tests.rs"]
mod tests;

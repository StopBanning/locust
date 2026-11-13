//! Keys a teammate needs (Electron `main/teammate-keys.ts`, `shared/teammate-keys.ts`, 0.833). A teammate that is to
//! charge a test card or read a private API asks for a key by name in a `locust-key` block; the window draws a masked
//! box under that answer; and the key is kept as the operating system protects it, for that teammate alone. Its runs
//! get it as an environment variable of that name, on an agent measured to pass it to its commands, and nothing else
//! does. The brief names the variable, never its value, and anything a run says that holds the value has it taken out
//! before it is recorded or shown.
//!
//! Kept in `teammate-keys.json` in the native data folder, readable by this account alone, each value as the
//! [`SecretBox`] sealed it (this Mac's keychain). Names come back to the window; values only to [`TeammateKeys::run_keys`].

use crate::own_models::SecretBox;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const KEY_TAG: &str = "locust-key";
pub const FILE: &str = "teammate-keys.json";
pub const MAX_KEY_LENGTH: usize = 4_096;
pub const MAX_KEYS_PER_TEAMMATE: usize = 20;

/// The agents a key reaches, each measured with a real run in Electron (`KEYS_REACH`). Codex drops every variable
/// named like a key from its commands; Antigravity runs its commands in its own app.
pub const KEYS_REACH: [&str; 3] = ["claude", "cursor", "opencode"];
pub const KEYS_REACH_SAID: &str = "Claude Code, Cursor and OpenCode";

pub fn reaches(runtime: &str) -> bool {
    KEYS_REACH.contains(&runtime)
}

const RESERVED_NAMES: [&str; 39] = [
    "GITHUB_TOKEN", "APPDATA", "COMSPEC", "HOME", "HOMEDRIVE", "HOMEPATH", "HTTP_PROXY", "HTTPS_PROXY", "NO_PROXY", "LANG", "LC_ALL", "LOCALAPPDATA", "LOGNAME", "NO_COLOR",
    "NODEFAULTCURRENTDIRECTORYINEXEPATH", "PATH", "PATHEXT", "SHELL", "SSL_CERT_DIR", "SSL_CERT_FILE", "SYSTEMROOT", "TEMP", "TMP", "TMPDIR", "USER", "USERNAME", "USERPROFILE",
    "WINDIR", "XDG_CACHE_HOME", "XDG_CONFIG_HOME", "XDG_DATA_HOME", "PWD", "OLDPWD", "TERM", "PROMPT", "PS1", "DYLD_INSERT_LIBRARIES", "DYLD_LIBRARY_PATH", "LD_PRELOAD",
];

/// Why a name will not do, in plain words; None when it will.
pub fn key_name_refusal(name: &str) -> Option<String> {
    if !crate::invariants::regex(r"^[A-Z][A-Z0-9_]{1,63}$").is_match(name) {
        return Some("A key\u{2019}s name is in capitals, digits and underscores, starting with a letter, like STRIPE_KEY.".into());
    }
    let reserved = crate::invariants::regex(r"^(?:ANTHROPIC_|CLAUDE_|OPENAI_|CODEX_|OPENCODE_|GEMINI_|GOOGLE_|CURSOR_|COPILOT_|GH_|AZURE_OPENAI_|LOCUST_|ELECTRON_|NODE_|NPM_)");
    if reserved.is_match(name) || RESERVED_NAMES.contains(&name) {
        let other = crate::invariants::regex(r"^[A-Z]+_").replace(name, "MY_");
        return Some(format!("{name} is a name an agent or this computer reads for itself, so a key can\u{2019}t take it. Pick another, like {other}."));
    }
    None
}

/// Why a value will not do; None when it will.
pub fn key_value_refusal(value: &str) -> Option<String> {
    if value.is_empty() {
        return Some("Paste the key first.".into());
    }
    if value.chars().count() > MAX_KEY_LENGTH {
        return Some(format!("That is longer than any key Locust keeps ({MAX_KEY_LENGTH} characters)."));
    }
    if value.contains(['\r', '\n', '\0']) {
        return Some("A key is one line. Paste only the key itself.".into());
    }
    None
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyRequest {
    pub name: String,
    pub why: String,
}

/// The keys a message asks for, outside code, each once and at most three.
pub fn key_requests_in(text: &str) -> Vec<KeyRequest> {
    let block = crate::invariants::regex(r#"<locust-key\s+name\s*=\s*"([^"]{1,80})"\s*>([\s\S]*?)</locust-key>"#);
    let mut found: Vec<KeyRequest> = vec![];
    for (start, end, _) in crate::protocol_tags::blocks_outside_code(text, &block) {
        let Some(caps) = block.captures(&text[start..end]) else { continue };
        let name = caps[1].trim().to_owned();
        if found.iter().any(|f| f.name == name) {
            continue;
        }
        let why: String = caps[2].split_whitespace().collect::<Vec<_>>().join(" ").chars().take(300).collect();
        found.push(KeyRequest { name, why });
        if found.len() == 3 {
            break;
        }
    }
    found
}

/// What a teammate is told of its keys: their names, how to use them, and how to ask for another.
pub fn keys_section(names: &[String], runtime_takes_them: bool) -> String {
    if !runtime_takes_them {
        return if names.is_empty() {
            String::new()
        } else {
            format!("Keys: the person gave you {}, but Locust does not give keys to this agent yet, so you cannot use them here. Say so if a task needs one.", names.join(", "))
        };
    }
    let mut lines = vec!["Keys:".to_owned()];
    if let Some(first) = names.first() {
        lines.push(format!(
            "You have these keys as environment variables: {}. Use them from commands and code by name (for example ${first} or process.env.{first}). Never print, echo, log or write a key's value anywhere: Locust takes the value out of anything it records, and the key is the person's.",
            names.join(", ")
        ));
    }
    lines.push(format!("If a task needs a key you do not have, do not ask for it in the chat. Write <{KEY_TAG} name=\"NAME\">what it is for</{KEY_TAG}> on a line of its own, with the variable's name in capitals, and stop: the person is shown a box to put it in, and you have it from your next turn."));
    lines.join("\n")
}

/// A teammate's key as a run is given it.
#[derive(Clone, PartialEq, Eq)]
pub struct RunKey {
    pub name: String,
    pub value: String,
}

impl std::fmt::Debug for RunKey {
    // A key's value never reaches a log, a panic message or a test's failure output.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "RunKey({})", self.name)
    }
}

const SHORTEST_TAKEN: usize = 4;

/// Text with every key's exact value replaced by its name in brackets, longest first so no value is left in part.
pub fn without_key_values(text: &str, keys: &[RunKey]) -> String {
    let mut sorted: Vec<&RunKey> = keys.iter().collect();
    sorted.sort_by(|a, b| b.value.len().cmp(&a.value.len()));
    let mut clean = text.to_owned();
    for key in sorted {
        if key.value.chars().count() < SHORTEST_TAKEN {
            continue;
        }
        if clean.contains(&key.value) {
            clean = clean.replace(&key.value, &format!("[key {}]", key.name));
        }
    }
    clean
}

/// Any value (an event, its payload, its evidence) with each key's value taken out of every string in it.
pub fn scrub_key_values(value: &Value, keys: &[RunKey]) -> Value {
    if keys.is_empty() {
        return value.clone();
    }
    match value {
        Value::String(s) => Value::String(without_key_values(s, keys)),
        Value::Array(items) => Value::Array(items.iter().map(|i| scrub_key_values(i, keys)).collect()),
        Value::Object(map) => Value::Object(map.iter().map(|(k, v)| (k.clone(), scrub_key_values(v, keys))).collect()),
        other => other.clone(),
    }
}

/// A change's answer: the teammate's key names after it, or why it did not happen with the names as they are.
pub type Changed = Result<Vec<String>, (String, Vec<String>)>;

type Saved = BTreeMap<String, BTreeMap<String, String>>;

pub struct TeammateKeys {
    file: PathBuf,
    secrets: Arc<dyn SecretBox>,
    /// One change at a time, each reading what the last one wrote.
    writing: Mutex<()>,
}

impl TeammateKeys {
    pub fn new(directory: &Path, secrets: Arc<dyn SecretBox>) -> Self {
        TeammateKeys { file: directory.join(FILE), secrets, writing: Mutex::new(()) }
    }

    fn load(&self) -> Saved {
        let Ok(text) = std::fs::read_to_string(&self.file) else { return Saved::new() };
        let Ok(parsed) = serde_json::from_str::<Value>(&text) else { return Saved::new() };
        if parsed["schemaVersion"] != 1 {
            return Saved::new();
        }
        serde_json::from_value(parsed["teammates"].clone()).unwrap_or_default()
    }

    fn save(&self, saved: &Saved) -> Result<(), String> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let directory = self.file.parent().ok_or("no folder")?;
        std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        let temp = self.file.with_extension("json.tmp");
        let _ = std::fs::remove_file(&temp);
        // The sealed keys are written only into a file that is this account's alone from the start.
        let mut out = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&temp).map_err(|e| e.to_string())?;
        out.write_all(json!({"schemaVersion": 1, "teammates": saved}).to_string().as_bytes()).map_err(|e| e.to_string())?;
        out.sync_all().map_err(|e| e.to_string())?;
        std::fs::rename(&temp, &self.file).map_err(|e| e.to_string())
    }

    fn names_of(saved: &Saved, teammate: &str) -> Vec<String> {
        saved.get(teammate).map(|kept| kept.keys().cloned().collect()).unwrap_or_default()
    }

    /// The names of a teammate's keys, sorted. Their values never come back to the window.
    pub fn names(&self, teammate: &str) -> Vec<String> {
        Self::names_of(&self.load(), teammate)
    }

    pub fn set(&self, teammate: &str, name: &str, value: &str) -> Changed {
        let _one = self.writing.lock().unwrap_or_else(|e| e.into_inner());
        let mut saved = self.load();
        let (name, value) = (name.trim(), value.trim());
        let fail = |message: String, saved: &Saved| Err((message, Self::names_of(saved, teammate)));
        if teammate.is_empty() {
            return fail("Choose a teammate first.".into(), &saved);
        }
        if let Some(refused) = key_name_refusal(name).or_else(|| key_value_refusal(value)) {
            return fail(refused, &saved);
        }
        let kept = saved.get(teammate).cloned().unwrap_or_default();
        if !kept.contains_key(name) && kept.len() >= MAX_KEYS_PER_TEAMMATE {
            return fail(format!("A teammate keeps {MAX_KEYS_PER_TEAMMATE} keys at most. Remove one first."), &saved);
        }
        if !self.secrets.available() {
            return fail("This computer cannot encrypt the key, so it was not kept.".into(), &saved);
        }
        let Ok(sealed) = self.secrets.seal(value) else {
            return fail("The key could not be kept on this computer. Nothing changed.".into(), &saved);
        };
        let replaced = kept.get(name).cloned();
        saved.entry(teammate.to_owned()).or_default().insert(name.to_owned(), sealed.clone());
        if self.save(&saved).is_err() {
            self.secrets.forget(&sealed);
            return fail("The key could not be kept on this computer. Nothing changed.".into(), &self.load());
        }
        if let Some(old) = replaced {
            self.secrets.forget(&old);
        }
        Ok(Self::names_of(&saved, teammate))
    }

    pub fn remove(&self, teammate: &str, name: &str) -> Changed {
        let _one = self.writing.lock().unwrap_or_else(|e| e.into_inner());
        let mut saved = self.load();
        let Some(sealed) = saved.get_mut(teammate).and_then(|kept| kept.remove(name)) else {
            return Ok(Self::names_of(&saved, teammate));
        };
        if saved.get(teammate).is_some_and(BTreeMap::is_empty) {
            saved.remove(teammate);
        }
        if self.save(&saved).is_err() {
            return Err(("The key could not be removed. It is unchanged.".into(), self.names(teammate)));
        }
        self.secrets.forget(&sealed);
        Ok(Self::names_of(&saved, teammate))
    }

    /// A teammate's keys, opened, for one of its runs. None kept: the keychain is not asked (a password box on every
    /// run start, Electron 0.854). One that will not open here is left out.
    pub fn run_keys(&self, teammate: &str) -> Vec<RunKey> {
        let kept = self.load().remove(teammate).unwrap_or_default();
        if kept.is_empty() || !self.secrets.available() {
            return vec![];
        }
        kept.into_iter()
            .filter(|(name, _)| key_name_refusal(name).is_none())
            .filter_map(|(name, sealed)| self.secrets.open(&sealed).ok().map(|value| RunKey { name, value }))
            .collect()
    }

    /// A removed teammate's keys go with it.
    pub fn forget(&self, teammate: &str) {
        let _one = self.writing.lock().unwrap_or_else(|e| e.into_inner());
        let mut saved = self.load();
        let Some(kept) = saved.remove(teammate) else { return };
        if self.save(&saved).is_ok() {
            for sealed in kept.values() {
                self.secrets.forget(sealed);
            }
        }
    }

    /// Whether any teammate has a key kept, read without opening one.
    pub fn keeps_key(&self) -> bool {
        self.load().values().any(|kept| !kept.is_empty())
    }
}

/// The keys kept in a native data folder, in this process's box.
pub fn for_store(store: &crate::store::Store) -> TeammateKeys {
    TeammateKeys::new(store.root(), crate::own_models::secrets())
}

#[cfg(test)]
#[path = "teammate_keys_tests.rs"]
mod tests;

//! LOCUST IN THE CLAUDE APP, IN ONE PRESS (Electron `main/claude-app-connection.ts`, 0.740 and Sol's hardening, 0.741).
//!
//! Adding Locust to the Claude app's tools by hand is five steps: its settings file, a block with paths and commas,
//! a quit from the tray. So Locust does the edit, and only when the person presses Add: it keeps the file as it was
//! beside it (once, the original), adds one `locust` entry under `mcpServers`, changes nothing else, and Remove takes
//! that entry out again. A file it cannot read is left alone, never rewritten from a guess.
//!
//! - One change at a time, and a change starts again if the file moved under it (Claude saving its own settings).
//! - Only Locust's own entry (one that runs Locust's bridge: this app's `--door-bridge`, or Electron's
//!   `locust-mcp-bridge.mjs`) is changed or removed; a different server the person named `locust` is theirs.
//! - The file keeps its permissions, and a settings file that is a link is written at its target.
//! - An entry that points at another copy of Locust, or at files that are gone, says so and offers Update.
//! - Whether Claude still has to restart is worked out from the facts: a Claude started before Locust's last change
//!   has not read it.
//! - Settings holding a number JSON cannot keep exactly are left alone.

use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

/// The original, kept beside the settings file the first time Locust changes it.
pub const BACKUP_SUFFIX: &str = ".before-locust";
const ENTRY: &str = "locust";
const ELECTRON_BRIDGE: &str = "locust-mcp-bridge.mjs";
const OWN_BRIDGE: &str = "--door-bridge";

pub const UNREADABLE: &str = "The Claude app\u{2019}s settings file could not be read, so Locust left it as it is.";
pub const INEXACT: &str = "The Claude app\u{2019}s settings hold a number Locust cannot keep exactly, so Locust left them as they are.";
pub const TAKEN: &str = "The Claude app already has a different add-on named \u{201c}locust\u{201d}, so Locust left it alone.";
pub const FAILED: &str = "Locust could not save the Claude app\u{2019}s settings. Nothing was changed.";

/// What Locust's entry runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub command: String,
    pub args: Vec<String>,
}

/// Where Locust stands in the Claude app's settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Connection {
    /// The Claude app is not installed here.
    NoApp,
    Unreadable(String),
    NotAdded { restart: bool },
    Taken(String),
    Added { restart: bool, stale: bool },
}

/// Where the Claude app keeps its settings file, or none where it has none Locust knows.
pub fn config_file(platform: &str, app_data: Option<&str>, home: &Path) -> Option<PathBuf> {
    match platform {
        "win32" => app_data.filter(|a| !a.is_empty()).map(|a| Path::new(a).join("Claude").join("claude_desktop_config.json")),
        "darwin" => Some(home.join("Library").join("Application Support").join("Claude").join("claude_desktop_config.json")),
        _ => None,
    }
}

/// Locust's own entry: one that runs Locust's bridge. Anything else named `locust` is the person's.
fn ours(entry: &Value) -> bool {
    let (Some(_), Some(args)) = (entry["command"].as_str(), entry["args"].as_array()) else { return false };
    match args.first().and_then(Value::as_str) {
        Some(first) => first == OWN_BRIDGE || first.ends_with(&format!("/{ELECTRON_BRIDGE}")) || first.ends_with(&format!("\\{ELECTRON_BRIDGE}")),
        None => false,
    }
}

/// Every number kept exactly by a JSON round trip: an integer past 2^53 is not.
fn exact(value: &Value) -> bool {
    const SAFE: u64 = (1 << 53) - 1;
    match value {
        Value::Number(n) => n.as_u64().map(|u| u <= SAFE).or_else(|| n.as_i64().map(|i| i.unsigned_abs() <= SAFE)).unwrap_or_else(|| n.as_f64().is_some_and(|f| f.fract() != 0.0 || f.abs() <= SAFE as f64)),
        Value::Array(a) => a.iter().all(exact),
        Value::Object(o) => o.values().all(exact),
        _ => true,
    }
}

enum Read {
    None,
    Read { text: String, settings: Map<String, Value> },
    Unreadable,
    Inexact,
}

fn read(path: &Path) -> Read {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Read::None,
        Err(_) => return Read::Unreadable,
    };
    if text.trim().is_empty() {
        return Read::Read { text, settings: Map::new() };
    }
    match serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')) {
        Ok(Value::Object(settings)) if exact(&Value::Object(settings.clone())) => Read::Read { text, settings },
        Ok(Value::Object(_)) => Read::Inexact,
        _ => Read::Unreadable,
    }
}

/// The file itself: a settings file that is a link is read and written at its target.
fn target(file: &Path) -> PathBuf {
    match std::fs::symlink_metadata(file) {
        Ok(m) if m.file_type().is_symlink() => std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf()),
        _ => file.to_path_buf(),
    }
}

/// When Locust last changed Claude's settings and last restarted Claude, in its own folder, so a Locust restart does
/// not forget that Claude still has to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    pub changed_at: Option<i64>,
    pub restarted_at: Option<i64>,
}

/// When the Claude app started (ms): None when it is not running; Some(None) when the system cannot say.
pub type StartedAt = dyn Fn() -> Option<Option<i64>> + Send + Sync;

pub struct Connector {
    pub file: Option<PathBuf>,
    pub entry: Entry,
    pub marks_file: PathBuf,
    pub started_at: Box<StartedAt>,
    pub relaunch: Option<Box<dyn Fn() -> Result<(), String> + Send + Sync>>,
    pub now: Box<dyn Fn() -> i64 + Send + Sync>,
}

impl Connector {
    fn marks(&self) -> Marks {
        let v: Value = std::fs::read_to_string(&self.marks_file).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Value::Null);
        Marks { changed_at: v["changedAt"].as_i64(), restarted_at: v["restartedAt"].as_i64() }
    }

    fn write_marks(&self, marks: Marks) {
        let mut v = Map::new();
        if let Some(at) = marks.changed_at {
            v.insert("changedAt".into(), json!(at));
        }
        if let Some(at) = marks.restarted_at {
            v.insert("restartedAt".into(), json!(at));
        }
        if let Some(dir) = self.marks_file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(&self.marks_file, Value::Object(v).to_string());
    }

    /// Whether the Claude app has not yet read Locust's last change: it started before that change. Where the system
    /// cannot say when it started, the last restart Locust made stands in for it.
    fn unread(&self) -> bool {
        let marks = self.marks();
        let Some(changed) = marks.changed_at else { return false };
        match (self.started_at)() {
            None => false,
            Some(None) => marks.restarted_at.is_none_or(|r| r < changed),
            Some(Some(started)) => started < changed,
        }
    }

    pub fn status(&self) -> Connection {
        let Some(file) = &self.file else { return Connection::NoApp };
        if !file.parent().is_some_and(Path::exists) {
            return Connection::NoApp;
        }
        let settings = match read(&target(file)) {
            Read::Unreadable => return Connection::Unreadable(UNREADABLE.into()),
            Read::Inexact => return Connection::Unreadable(INEXACT.into()),
            Read::None => Map::new(),
            Read::Read { settings, .. } => settings,
        };
        let restart = self.unread();
        let Some(entry) = settings.get("mcpServers").and_then(Value::as_object).and_then(|s| s.get(ENTRY)) else {
            return Connection::NotAdded { restart };
        };
        if !ours(entry) {
            return Connection::Taken(TAKEN.into());
        }
        let args: Vec<&str> = entry["args"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
        let command = entry["command"].as_str().unwrap_or("");
        let here: Vec<&str> = self.entry.args.iter().map(String::as_str).collect();
        // Pointing at a Locust that is not this one, or at files that are gone. This app's own bridge is a flag on its
        // program; Electron's is a script beside it, which must still be there.
        let stale = command != self.entry.command
            || args.first() != here.first()
            || args.get(1) != here.get(1)
            || !Path::new(command).exists()
            || args.first().is_some_and(|first| *first != OWN_BRIDGE && !Path::new(first).exists());
        Connection::Added { restart, stale }
    }

    /// One change, start to end: read, change, write beside it, and swap it in only if the file is still what was
    /// read; otherwise start again from what is there now. A few tries, then say so.
    fn change(&self, edit: &dyn Fn(Map<String, Value>) -> Result<Option<Map<String, Value>>, String>) -> Result<(), String> {
        let file = self.file.as_ref().ok_or("The Claude app is not installed on this computer.")?;
        let path = target(file);
        for _ in 0..4 {
            let before = read(&path);
            let (settings, before_text) = match before {
                Read::Unreadable => return Err(UNREADABLE.into()),
                Read::Inexact => return Err(INEXACT.into()),
                Read::None => (Map::new(), None),
                Read::Read { text, settings } => (settings, Some(text)),
            };
            let Some(next) = edit(settings)? else { return Ok(()) };
            let temp = path.with_file_name(format!("{}.locust-{}", path.file_name().and_then(|n| n.to_str()).unwrap_or("settings"), crate::invariants::unique_nanos()));
            let attempt = (|| -> std::io::Result<bool> {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                // Exclusive: never into something already there.
                let mut out = std::fs::OpenOptions::new().write(true).create_new(true).open(&temp)?;
                std::io::Write::write_all(&mut out, format!("{}\n", serde_json::to_string_pretty(&Value::Object(next.clone())).unwrap_or_default()).as_bytes())?;
                drop(out);
                // The file keeps its permissions.
                if before_text.is_some()
                    && let Ok(meta) = std::fs::metadata(&path)
                {
                    std::fs::set_permissions(&temp, meta.permissions())?;
                }
                let now_text = std::fs::read_to_string(&path).ok();
                if now_text != before_text {
                    let _ = std::fs::remove_file(&temp);
                    return Ok(false);
                }
                // The original, kept once: never over a copy already there.
                if before_text.is_some() {
                    let backup = PathBuf::from(format!("{}{BACKUP_SUFFIX}", path.display()));
                    if !backup.exists() {
                        std::fs::copy(&path, &backup)?;
                    }
                }
                std::fs::rename(&temp, &path)?;
                Ok(true)
            })();
            match attempt {
                Ok(true) => return Ok(()),
                Ok(false) => continue,
                Err(_) => {
                    let _ = std::fs::remove_file(&temp);
                    return Err(FAILED.into());
                }
            }
        }
        Err("The Claude app kept changing its settings, so Locust stopped. Try again.".into())
    }

    fn changed(&self) {
        let marks = self.marks();
        self.write_marks(Marks { changed_at: Some((self.now)()), ..marks });
    }

    /// Add Locust's entry, or point an old one here (Update).
    pub fn add(&self) -> Result<(), String> {
        if !self.file.as_ref().and_then(|f| f.parent()).is_some_and(Path::exists) {
            return Err("The Claude app is not installed on this computer.".into());
        }
        let entry = self.entry.clone();
        self.change(&move |mut settings| {
            let servers = match settings.get("mcpServers") {
                None => Map::new(),
                Some(Value::Object(s)) => s.clone(),
                Some(_) => return Err(UNREADABLE.into()),
            };
            if servers.get(ENTRY).is_some_and(|e| !ours(e)) {
                return Err(TAKEN.into());
            }
            let mut servers = servers;
            servers.insert(ENTRY.into(), json!({"command": entry.command, "args": entry.args, "env": {}}));
            settings.insert("mcpServers".into(), Value::Object(servers));
            Ok(Some(settings))
        })?;
        self.changed();
        Ok(())
    }

    /// Take Locust's entry out, and only Locust's.
    pub fn remove(&self) -> Result<(), String> {
        if self.file.is_none() {
            return Ok(());
        }
        let removed_flag = std::cell::Cell::new(false);
        self.change(&|mut settings| {
            let Some(Value::Object(servers)) = settings.get("mcpServers").cloned() else { return Ok(None) };
            let Some(entry) = servers.get(ENTRY) else { return Ok(None) };
            if !ours(entry) {
                return Err(TAKEN.into());
            }
            let mut servers = servers;
            servers.remove(ENTRY);
            if servers.is_empty() {
                settings.remove("mcpServers");
            } else {
                settings.insert("mcpServers".into(), Value::Object(servers));
            }
            removed_flag.set(true);
            Ok(Some(settings))
        })?;
        if removed_flag.get() {
            self.changed();
        }
        Ok(())
    }

    /// Close the Claude app and open it again, only when the person presses Restart.
    pub fn restart(&self) -> Result<(), String> {
        let Some(relaunch) = &self.relaunch else { return Err("Quit the Claude app and open it again.".into()) };
        relaunch().map_err(|_| "Locust could not restart the Claude app. Quit it from its icon by the clock and open it again.".to_owned())?;
        let marks = self.marks();
        self.write_marks(Marks { restarted_at: Some((self.now)()), ..marks });
        Ok(())
    }
}

/// The Card's sentence for where Locust stands (Electron's LocustMcpSettings).
pub fn connection_words(connection: &Connection) -> String {
    match connection {
        Connection::NoApp => String::new(),
        Connection::Unreadable(m) | Connection::Taken(m) => m.clone(),
        Connection::NotAdded { restart: true } => "Removed. The Claude app keeps using Locust until it restarts; restarting closes everything running in it.".into(),
        Connection::NotAdded { restart: false } => "Lets a Claude chat ask your teammates and read their replies. Locust backs up Claude\u{2019}s settings first.".into(),
        Connection::Added { stale: true, .. } => "Added, but for a copy of Locust that is not this one, or that has moved. Update points it here.".into(),
        Connection::Added { restart: true, .. } => "Added. The Claude app reads it when it starts, and it is open now: restart it to finish. Restarting closes everything running in the Claude app.".into(),
        Connection::Added { .. } => "Added. Ask a Claude chat about your Locust teammates; it asks before it uses them.".into(),
    }
}

/// Whether the Claude app is running, on a Mac (Electron asks `application "Claude" is running`). The system cannot
/// say when it started, so a running app answers Some(None).
pub fn mac_started_at() -> Option<Option<i64>> {
    let out = std::process::Command::new("osascript").args(["-e", "application \"Claude\" is running"]).output().ok()?;
    (String::from_utf8_lossy(&out.stdout).trim() == "true").then_some(None)
}

/// Quit the Claude app and open it again, on a Mac.
pub fn mac_relaunch() -> Result<(), String> {
    std::process::Command::new("osascript").args(["-e", "tell application \"Claude\" to quit"]).status().map_err(|e| e.to_string())?;
    std::thread::sleep(std::time::Duration::from_millis(1500));
    std::process::Command::new("open").args(["-a", "Claude"]).status().map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
#[path = "claude_app_tests.rs"]
mod tests;

//! THE FILES A TEAMMATE'S OWN RECORD NAMES, WHEREVER THEY ARE (Electron `main/named-files.ts`, 0.787 and 0.868).
//!
//! Codex, asked to write its review to D:\work\scratch, wrote it there with its own file tool and handed it over by its
//! whole path. Locust drew no button: the host opens a file only inside the folders it knows (`file_menu::decide_reveal`),
//! so the thread said "Not shown as a file", and the turn's own "created a file" row would not open it either. Colin,
//! 2026-10-18: "claude code and the other apps still let you access the file".
//!
//! So a whole path outside those folders is honoured when a teammate's recorded turn names it: in a file block it wrote,
//! or as the whole target of a step of its own that finished (Codex's apply_patch, Claude Code's Write or Read). The host
//! reads these from the ledger, never from the request. A step that was declined or failed names nothing here. A network
//! path is never one.
//!
//! And a relative path in a file block (0.868: `../locust-wt/spotify-artist/REPORT.md`, "wont let me copy") is kept as it
//! was written and resolved only against the folder that turn ran in, never against every folder the host knows.

use crate::invariants::lock;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::UNIX_EPOCH;

/// A step that did not do what it set out to: its target is not the teammate's (`DID_NOT_HAPPEN`).
fn did_not_happen(status: &str) -> bool {
    let status = status.to_lowercase();
    ["declin", "refus", "denied", "reject", "fail", "error", "cancel"].iter().any(|w| status.contains(w))
}

fn bounded(path: &str) -> bool {
    !path.is_empty() && path.encode_utf16().count() <= 400 && !path.chars().any(|c| matches!(c, '\u{0}'..='\u{1f}' | '\u{7f}'))
}

/// Node's `resolve` for a whole path: `.` and `..` taken out, never above the root.
fn resolve(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                if out.parent().is_some() {
                    out.pop();
                }
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// One spelling per file: resolved, and on Windows without case (`namedFileKey`). None for anything but a whole path on
/// this computer -- on Windows one that starts with a drive, which no share or device path does.
pub fn named_file_key(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if !bounded(trimmed) {
        return None;
    }
    #[cfg(windows)]
    let whole = {
        let b = trimmed.as_bytes();
        b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
    };
    #[cfg(not(windows))]
    let whole = trimmed.starts_with('/');
    if !whole || !Path::new(trimmed).is_absolute() {
        return None;
    }
    let resolved = resolve(Path::new(trimmed)).to_string_lossy().into_owned();
    #[cfg(windows)]
    return Some(resolved.to_lowercase());
    #[cfg(not(windows))]
    Some(resolved)
}

/// A reply is one thing, not the pieces it arrived in (`joinMessageFragments`): each message's text, whole.
fn joined_messages(events: &[Value]) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    let mut text: HashMap<String, String> = HashMap::new();
    for event in events.iter().filter(|e| e["type"] == "message.delta") {
        let p = &event["payload"];
        let item = p["itemId"].as_str().unwrap_or_default().to_owned();
        let piece = p["text"].as_str().unwrap_or_default();
        match text.get_mut(&item) {
            None => {
                order.push(item.clone());
                text.insert(item, piece.to_owned());
            }
            Some(held) if p["operation"] == "replace" => *held = piece.to_owned(),
            Some(held) => held.push_str(piece),
        }
    }
    order.into_iter().filter_map(|item| text.remove(&item)).collect()
}

fn push_once(named: &mut Vec<String>, key: String) {
    if !named.contains(&key) {
        named.push(key);
    }
}

/// The whole paths one turn's record names as the teammate's (`filesNamedIn`): what it handed over, and what its finished
/// steps acted on.
pub fn files_named_in(events: &[Value]) -> Vec<String> {
    let mut named = Vec::new();
    for message in joined_messages(events) {
        if message.contains("<locust-file") {
            for file in crate::protocol_tags::parse_file_blocks(&message) {
                if let Some(key) = named_file_key(&file.path) {
                    push_once(&mut named, key);
                }
            }
        }
    }
    for event in events.iter().filter(|e| e["type"] == "tool.completed") {
        let p = &event["payload"];
        let Some(command) = p["command"].as_str() else { continue };
        if p["status"].as_str().is_some_and(did_not_happen) {
            continue;
        }
        // A line that is a path and nothing else. A shell command names paths inside a sentence of its own, and is not
        // read for them.
        for line in command.split('\n') {
            if let Some(key) = named_file_key(line) {
                push_once(&mut named, key);
            }
        }
    }
    named
}

/// The relative paths a turn's file blocks hand over, as written (`relativeFilesNamedIn`). Only file blocks: a step's
/// target was always a whole path, and a shell command is not read.
pub fn relative_files_named_in(events: &[Value]) -> Vec<String> {
    let mut named = Vec::new();
    for message in joined_messages(events) {
        if !message.contains("<locust-file") {
            continue;
        }
        for file in crate::protocol_tags::parse_file_blocks(&message) {
            let path = file.path.trim();
            if !bounded(path) {
                continue;
            }
            // A whole path is files_named_in's; a network or drive-relative spelling is never one.
            let b = path.as_bytes();
            if Path::new(path).is_absolute() || matches!(b[0], b'/' | b'\\') || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':') {
                continue;
            }
            push_once(&mut named, path.to_owned());
        }
    }
    named
}

/// The key a relative handed path names once it is resolved against the folder its turn ran in (`relativeNamedKey`).
pub fn relative_named_key(folder: &str, path: &str) -> Option<String> {
    named_file_key(&Path::new(folder).join(path).to_string_lossy())
}

#[derive(Clone)]
struct Entry {
    stamp: (u64, u128),
    named: Vec<String>,
    relative: Vec<String>,
    workspace_id: Option<String>,
}

fn cache() -> &'static Mutex<HashMap<PathBuf, Entry>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Entry>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

fn read_entry(path: &Path, stamp: (u64, u128)) -> Entry {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut workspace_id = None;
    let mut events: BTreeMap<u64, Value> = BTreeMap::new();
    for record in text.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()) {
        if record["recordType"] == "mission.created" && workspace_id.is_none() {
            workspace_id = record["metadata"]["workspaceId"].as_str().map(str::to_owned);
        }
        if let Some(event) = record.get("event")
            && let Some(seq) = event["sequence"].as_u64()
        {
            events.insert(seq, event.clone());
        }
    }
    let events: Vec<Value> = events.into_values().collect();
    Entry { stamp, named: files_named_in(&events), relative: relative_files_named_in(&events), workspace_id }
}

/// Every ledger file's entry in `dirs`, brought up to date: a file that changed since the last read is parsed again, one
/// that is gone is forgotten.
fn refreshed(dirs: &[PathBuf]) -> Vec<Entry> {
    let mut present: Vec<(PathBuf, (u64, u128))> = Vec::new();
    for dir in dirs {
        let Ok(listing) = std::fs::read_dir(dir) else { continue };
        for item in listing.flatten() {
            let path = item.path();
            if path.extension().is_none_or(|e| e != "jsonl") || !item.file_type().is_ok_and(|t| t.is_file()) {
                continue;
            }
            let Ok(meta) = item.metadata() else { continue };
            let modified = meta.modified().unwrap_or(UNIX_EPOCH).duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
            present.push((path, (meta.len(), modified)));
        }
    }
    let mut held = lock(cache());
    let here: HashSet<&PathBuf> = present.iter().map(|(p, _)| p).collect();
    held.retain(|p, _| !dirs.iter().any(|d| p.starts_with(d)) || here.contains(p));
    let mut out = Vec::with_capacity(present.len());
    for (path, stamp) in present {
        let entry = match held.get(&path).filter(|e| e.stamp == stamp) {
            Some(hit) => hit.clone(),
            None => {
                let read = read_entry(&path, stamp);
                held.insert(path, read.clone());
                read
            }
        };
        out.push(entry);
    }
    out
}

/// Whether a teammate's recorded turn names this whole path as its own (`namedByATeammate`), read from the ledger
/// folders `dirs` again whenever a file in them changes: a turn still running is read as it grows. A relative handover
/// counts against the one folder its turn ran in: a folder the host knows (`folders`), matched by the id the record keeps
/// for it, never tried against every folder until one fits.
pub fn named_by_a_teammate(dirs: &[PathBuf], path: &str, folders: &[String]) -> bool {
    let Some(key) = named_file_key(path) else { return false };
    let folder_of: HashMap<String, &str> =
        folders.iter().map(|f| (crate::workspace_settings::workspace_id_for(Path::new(f)), f.as_str())).collect();
    refreshed(dirs).iter().any(|entry| {
        if entry.named.contains(&key) {
            return true;
        }
        let Some(folder) = folder_of.get(entry.workspace_id.as_deref().unwrap_or_default()) else { return false };
        entry.relative.iter().any(|relative| relative_named_key(folder, relative).as_deref() == Some(key.as_str()))
    })
}

/// The ledger folders this app reads conversations from: native's own and Electron's (`native::find_ledger`).
pub fn ledger_dirs() -> Vec<PathBuf> {
    [crate::native::store().and_then(|s| s.ledger_dir().ok()), crate::native::electron_ledger_dir()].into_iter().flatten().collect()
}

#[cfg(test)]
#[path = "named_files_tests.rs"]
mod named_files_tests;

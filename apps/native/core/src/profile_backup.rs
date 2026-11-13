//! BACK UP AND RESTORE (Electron `main/profile-backup.ts`; PARITY row 115).
//!
//! A backup is a new folder, `Locust backup YYYY-MM-DD HHMM`, holding a copy of
//! the profile's own files under `profile/` and a manifest (`locust-backup.json`)
//! written LAST, so a folder without one is a backup that did not finish. Every
//! copied file is named in the manifest with its size and SHA-256, read back
//! from the copy.
//!
//! What it carries is named, one file and one folder at a time, never
//! "everything but": a store added later is left out until it is classified
//! here, so nothing new is copied without a decision. Own-model keys and
//! anything in the keychain are never carried.
//!
//! A restore is read by the same reader as the preview, and refused when the
//! manifest is missing, a file does not match its hash, the backup came from a
//! newer Locust, or a run is going. It is not applied while the app holds its
//! stores in memory: the request is written to `pending-restore.json`, the app
//! restarts, and the restore is applied at the next start before any store is
//! read. What it replaces is moved aside inside the profile (`before-restore-...`),
//! never deleted.

use chrono::{DateTime, Local, Utc};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const BACKUP_SCHEMA: u32 = 1;
pub const BACKUP_MANIFEST: &str = "locust-backup.json";
/// The restore asked for, written by the request, applied and removed at the next start.
pub const PENDING_RESTORE_FILE: &str = "pending-restore.json";
/// What the last restore did, kept until the window has said so once.
pub const LAST_RESTORE_FILE: &str = "last-restore.json";
/// Where each conversation with a Kilo teammate keeps Kilo's own data.
pub const KILO_CONVERSATIONS_FOLDER: &str = "kilo-conversations";

/// The files a backup carries: the person's setup.
pub const BACKED_UP_FILES: &[&str] = &[
    "teammates.json",
    "routines.json",
    "memories.json",
    "approval-rules.json",
    "groups.json",
    "rooms.json",
    "compares.json",
    "folders.json",
    "pets-removed.json",
    "calls.json",
    "scout.json",
    "scout-looks.json",
    "issue-origins.json",
    "panel-runs.json",
    "appearance.json",
    "folder-cautions.json",
    "tune-lessons.json",
];

/// The folders a backup carries: conversations, rooms, pets, and skills.
pub const BACKED_UP_FOLDERS: &[&str] = &[
    "mission-ledger",
    "workroom",
    "pets",
    "skill-library",
    "teammate-skills",
];

/// Never copied out of a folder a backup carries: the ledger's write probe and machine-local undo checkpoints.
const NEVER_COPIED: &[&str] = &[".writable", "checkpoints"];

/// What a backup never carries, and why — in the manifest, so a backup says what it is not.
pub const LEFT_OUT: &[(&str, &str)] = &[
    ("routine-hooks.json", "private link passwords and collected webhook data: tied to this computer and never backed up"),
    ("claude-context-launches.json", "machine-local flags for context inspection: recorded again by the next Claude turn"),
    ("conversation-references.json", "reference folders belong to this machine: choose them again after a restore"),
    ("reference-repositories", "shallow reference copies: downloaded again after choosing a repository"),
    ("reference-trash", "recoverable old reference copies on this machine, never permanently deleted by reference cleanup"),
    ("agent-accounts", "added agent homes and sign-ins: they stay on this machine and are never copied into a backup"),
    ("locust-mcp-connection.json", "the local MCP server token and address: they belong to this computer’s account and are minted again"),
    ("locust-mcp.json", "whether other AI apps can use Locust on this machine: enable again after a restore"),
    ("phone.json", "the key your phone on the home network was paired with: it belongs to this machine, so pair again after a restore"),
    ("phone-relay.json", "this computer's keys and the phones paired with it through Locust's relay: they belong to this machine, so pair again after a restore (0.826)"),
    ("voice-call-mcp", "a voice call's own token for handing work to teammates: minted again for every call (0.739)"),
    ("voice-call.json", "the voice Talk speaks in, and the keys that start it from any app: preferences, picked again in a moment after a restore (0.745, 0.755)"),
    ("voice-previews", "the greeting each voice gave the first time it was played: made again by playing it (0.751)"),
    ("reply-pages", "pages from replies, written from the reply again whenever it is opened"),
    ("claude-app.json", "when Locust last changed the Claude app's settings on this machine, to say whether Claude needs a restart (0.741)"),
    ("own-models.json", "your own model keys: this computer locks them to your account, so add them again after a restore"),
    ("voice-settings.json", "your voice API key and consent: this computer locks the key to your account, so choose voice typing again after a restore"),
    ("claude-cloud.json", "cloud sessions, which belong to this machine and its sign-in"),
    ("cloud-tasks.json", "cloud tasks, which belong to this machine and its sign-in"),
    ("cloud-turns.json", "turns sent to your cloud computer, which name this machine's folders: their changes stay on the codespace, on GitHub (0.761)"),
    ("brief-sessions.json", "each agent's own session names, which do not carry to another machine"),
    (KILO_CONVERSATIONS_FOLDER, "Kilo's own record of each conversation with a Kilo teammate: it stays on this machine, and after a restore a reply starts a new Kilo session, told what was said before (0.845)"),
    ("runtime-commands.json", "where each AI agent is installed on this machine"),
    ("runtime-facts.json", "what was found out about the AI agents installed here"),
    ("runtime-updates.json", "the agent updates this machine installed"),
    ("cursor-connector-holds.json", "this machine's Cursor settings, held while Locust runs it"),
    ("cursor-default-model.json", "this machine's Cursor default model, kept to put back"),
    ("terminal-imports.json", "conversations already brought in from this machine's terminal"),
    ("usage-readings.json", "usage read from this machine's accounts"),
    ("memory-recall-cache.json", "a cache that rebuilds itself"),
    ("voice", "voice typing's engine and model, downloaded again when it is next used"),
    ("claude-background.json", "Claude turns running in the background on this computer, which Claude Code keeps"),
    ("claude-skills", "skills copied for runs in progress, made again for each run"),
    ("run-skills", "skills copied for Codex and OpenCode runs in progress, made again for each run"),
    ("checkpoints", "copies of your folders from before each turn, for Undo: they belong to the folders on this machine, and can hold files like .env"),
    ("routine-copies", "the copies routines work in, and what their last runs changed there: they belong to the folders on this machine, and can hold files like .env (Electron keeps them outside its profile)"),
    ("queued-messages.json", "messages waiting to send from here: a restore never sends anything"),
    ("away.json", "what happened while this machine was away"),
    ("seen-version.json", "which version this machine last showed"),
    ("keychain-opened.json", "which version last opened this machine's keychain"),
    ("update-lane.json", "this machine's update choice"),
    ("window.json", "this machine's window size and place"),
    ("workspace.json", "the folder this machine last had open"),
    ("background.json", "whether Locust keeps running on this machine"),
    ("locust-errors.log", "this machine's error log"),
    ("pets-cache", "a cache that rebuilds itself"),
    ("mac-updates", "a Mac update being downloaded"),
    ("dictionaries", "the spelling dictionary this machine downloaded"),
    ("discord.json", "your Discord bot and the account paired with it: this computer locks the bot token to your account, so set Discord up again after a restore"),
    ("slack.json", "your Slack app and the person paired with it: this computer locks its tokens to your account, so set Slack up again after a restore"),
    ("telegram.json", "your Telegram bot and the chat paired with it: this computer locks the bot token to your account, so set Telegram up again after a restore"),
    ("remote-mcp.json", "the apps let in from anywhere, and their keys: they belong to this computer's room on the relay, so turn Use from anywhere on and allow each app again"),
    ("acp-agents.json", "your own agents, which name programs installed on this computer: add them again after a restore"),
    ("teammate-keys.json", "each teammate's keys: this computer locks them to your account, so add them again after a restore"),
    ("awake.json", "whether Locust keeps this computer awake"),
    ("scout-away.json", "Scout's looks while this computer is off: its key belongs to this computer's room on the relay, so turn it on again"),
    (PENDING_RESTORE_FILE, "a restore's own note of what to restore when Locust next starts"),
    (LAST_RESTORE_FILE, "a restore's own note of how it went, read once"),
    ("account-copies", "a copy brought from another computer, opened here before the restore applies it: brought again if needed"),
    ("account-sync.json", "whether this computer syncs, and the key its copy is sealed with, locked by this computer: sign in and turn sync on again after a restore"),
    ("profile-sync.json", "what this computer last sent to its sealed copy on Locust's relay: the next sync works it out again"),
    // Native-only files/folders in the native data directory:
    ("guard", "command guard hooks and settings generated for runs on this computer"),
    ("engine.lock", "the shared engine process lock on this computer"),
    ("engine.sock", "the shared engine process socket on this computer"),
    ("engine.log", "the shared engine process log on this computer"),
    (".locust", "pasted or attached files kept for conversations on this computer"),
    ("panel-runs", "internal store root for commit and walkthrough panel runs on this computer"),
    ("watched_pull_requests.json", "pull requests watched on this computer's GitHub CLI: watched again when opened"),
    ("skills.json", "native skill index rebuilt from the skill folders"),
];

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BackupCounts {
    pub teammates: usize,
    pub routines: usize,
    pub memories: usize,
    pub rules: usize,
    pub groups: usize,
    pub rooms: usize,
    pub compares: usize,
    pub folders: usize,
    pub conversations: usize,
}

impl BackupCounts {
    pub fn to_json(&self) -> Value {
        json!({
            "teammates": self.teammates,
            "routines": self.routines,
            "memories": self.memories,
            "rules": self.rules,
            "groups": self.groups,
            "rooms": self.rooms,
            "compares": self.compares,
            "folders": self.folders,
            "conversations": self.conversations,
        })
    }

    pub fn from_json(v: &Value) -> Self {
        let get = |k: &str| v.get(k).and_then(Value::as_u64).unwrap_or(0) as usize;
        Self {
            teammates: get("teammates"),
            routines: get("routines"),
            memories: get("memories"),
            rules: get("rules"),
            groups: get("groups"),
            rooms: get("rooms"),
            compares: get("compares"),
            folders: get("folders"),
            conversations: get("conversations"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupFileEntry {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupManifest {
    pub schema: u32,
    pub app: String,
    pub app_version: String,
    pub created_at: String,
    pub counts: BackupCounts,
    pub files: Vec<BackupFileEntry>,
    pub left_out: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupWritten {
    pub folder: PathBuf,
    pub counts: BackupCounts,
    pub bytes: u64,
    pub files: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupReading {
    pub folder: PathBuf,
    pub manifest: BackupManifest,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RestoreOutcome {
    pub ok: bool,
    pub folder: String,
    pub at: String,
    pub aside: Option<String>,
    pub counts: Option<BackupCounts>,
    pub reason: Option<String>,
}

impl RestoreOutcome {
    pub fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("ok".into(), Value::Bool(self.ok));
        obj.insert("folder".into(), Value::String(self.folder.clone()));
        obj.insert("at".into(), Value::String(self.at.clone()));
        if let Some(aside) = &self.aside {
            obj.insert("aside".into(), Value::String(aside.clone()));
        }
        if let Some(counts) = &self.counts {
            obj.insert("counts".into(), counts.to_json());
        }
        if let Some(reason) = &self.reason {
            obj.insert("reason".into(), Value::String(reason.clone()));
        }
        Value::Object(obj)
    }

    pub fn from_json(v: &Value) -> Option<Self> {
        let obj = v.as_object()?;
        Some(Self {
            ok: obj.get("ok")?.as_bool()?,
            folder: obj.get("folder")?.as_str()?.to_owned(),
            at: obj.get("at")?.as_str()?.to_owned(),
            aside: obj.get("aside").and_then(Value::as_str).map(str::to_owned),
            counts: obj.get("counts").map(BackupCounts::from_json),
            reason: obj.get("reason").and_then(Value::as_str).map(str::to_owned),
        })
    }
}

fn sha256_file(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    let digest = Sha256::digest(&bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for b in digest {
        hex.push_str(&format!("{b:02x}"));
    }
    Ok(hex)
}

fn list_length(path: &Path, key: &str) -> usize {
    let Ok(text) = fs::read_to_string(path) else {
        return 0;
    };
    let Ok(val) = serde_json::from_str::<Value>(&text) else {
        return 0;
    };
    val.get(key)
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

/// What a profile holds, by the person's own nouns.
pub fn count_profile(root: &Path) -> BackupCounts {
    let conversations = fs::read_dir(root.join("mission-ledger"))
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_ok_and(|t| t.is_file())
                && e.file_name().to_str().is_some_and(|n| n.ends_with(".jsonl"))
        })
        .count();
    BackupCounts {
        teammates: list_length(&root.join("teammates.json"), "teammates"),
        routines: list_length(&root.join("routines.json"), "routines"),
        memories: list_length(&root.join("memories.json"), "memories"),
        rules: list_length(&root.join("approval-rules.json"), "rules"),
        groups: list_length(&root.join("groups.json"), "groups"),
        rooms: list_length(&root.join("rooms.json"), "rooms"),
        compares: list_length(&root.join("compares.json"), "compares"),
        folders: list_length(&root.join("folders.json"), "folders"),
        conversations,
    }
}

/// Every file under `folder/base`, relative to `folder`, with forward slashes; the never-copied left out.
pub fn files_under(folder: &Path, base: &str) -> Vec<String> {
    let dir = if base.is_empty() {
        folder.to_path_buf()
    } else {
        folder.join(base)
    };
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut sorted: Vec<_> = entries.filter_map(Result::ok).collect();
    sorted.sort_by_key(|e| e.file_name());
    for entry in sorted {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if NEVER_COPIED.contains(&name.as_str()) {
            continue;
        }
        let rel = if base.is_empty() {
            name
        } else {
            format!("{base}/{name}")
        };
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_dir() {
            found.extend(files_under(folder, &rel));
        } else if ft.is_file() {
            found.push(rel);
        }
    }
    found
}

/// The profile files a backup carries, relative to the profile, that exist.
pub fn carried_files(profile: &Path) -> Vec<String> {
    let mut files = Vec::new();
    for name in BACKED_UP_FILES {
        if profile.join(name).is_file() {
            files.push((*name).to_owned());
        }
    }
    for name in BACKED_UP_FOLDERS {
        files.extend(files_under(profile, name));
    }
    files
}

/// `Locust backup 2026-10-04 2015`, the person's own clock: a folder name they will read.
pub fn backup_folder_name(at: DateTime<Local>) -> String {
    format!("Locust backup {}", at.format("%Y-%m-%d %H%M"))
}

/// A NEW backup folder in `parent`, the copy and its manifest. Never writes into
/// an existing folder: a name already taken is refused, not overwritten.
pub fn write_backup(
    profile: &Path,
    parent: &Path,
    app_version: &str,
    now: DateTime<Local>,
) -> Result<BackupWritten, String> {
    if !parent.exists() {
        return Err("That folder could not be found.".into());
    }
    let name = backup_folder_name(now);
    let folder = parent.join(&name);
    if folder.exists() {
        return Err(format!(
            "A backup named \"{name}\" is already there. Try again in a minute."
        ));
    }

    let result: io::Result<BackupWritten> = (|| {
        let mut entries = Vec::new();
        let mut bytes = 0u64;
        for rel in carried_files(profile) {
            let mut from = profile.to_path_buf();
            let mut to = folder.join("profile");
            for part in rel.split('/') {
                from.push(part);
                to.push(part);
            }
            if let Some(dir) = to.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::copy(&from, &to)?;
            // Read back from the COPY: the manifest vouches for what is in the backup, not what was meant to be.
            let size = fs::metadata(&to)?.len();
            let digest = sha256_file(&to)?;
            entries.push(BackupFileEntry {
                path: rel,
                bytes: size,
                sha256: digest,
            });
            bytes += size;
        }
        let counts = count_profile(&folder.join("profile"));
        let left_out: Vec<String> = LEFT_OUT.iter().map(|(k, _)| (*k).to_owned()).collect();
        let manifest = json!({
            "schema": BACKUP_SCHEMA,
            "app": "Locust",
            "appVersion": app_version,
            "createdAt": now.with_timezone(&Utc).to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            "counts": counts.to_json(),
            "files": entries.iter().map(|e| json!({
                "path": e.path,
                "bytes": e.bytes,
                "sha256": e.sha256,
            })).collect::<Vec<_>>(),
            "leftOut": left_out,
        });
        fs::create_dir_all(&folder)?;
        // Last: a folder with no manifest is a backup that did not finish, and the reader says so.
        let body = format!("{}\n", serde_json::to_string_pretty(&manifest).map_err(io::Error::other)?);
        fs::write(folder.join(BACKUP_MANIFEST), body)?;
        Ok(BackupWritten {
            folder: folder.clone(),
            counts,
            bytes,
            files: entries.len(),
        })
    })();

    result.map_err(|e| format!("The backup could not be written: {e}"))
}

/// Compare two `major.minor.patch` versions; a part that is not a number counts as 0.
pub fn compare_versions(left: &str, right: &str) -> Ordering {
    let parse = |s: &str| -> Vec<u64> {
        s.split('.')
            .map(|part| {
                let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
                digits.parse::<u64>().unwrap_or(0)
            })
            .collect()
    };
    let a = parse(left);
    let b = parse(right);
    let len = a.len().max(b.len());
    for i in 0..len {
        let av = a.get(i).copied().unwrap_or(0);
        let bv = b.get(i).copied().unwrap_or(0);
        match av.cmp(&bv) {
            Ordering::Equal => continue,
            ord => return ord,
        }
    }
    Ordering::Equal
}

/// Whether a path in a manifest is one a backup carries: a named file, or under a named folder.
pub fn carries(path: &str) -> bool {
    let first = path.split('/').next().unwrap_or("");
    BACKED_UP_FILES.contains(&path) || BACKED_UP_FOLDERS.contains(&first)
}

/// THE READER, for the preview and the restore alike: the manifest, then every
/// file it names, by size and hash. The first thing wrong is the reason.
pub fn read_backup(folder: &Path, app_version: &str) -> Result<BackupReading, String> {
    let raw = fs::read_to_string(folder.join(BACKUP_MANIFEST)).map_err(|_| {
        "This folder is not a Locust backup, or the backup did not finish: it has no locust-backup.json.".to_owned()
    })?;
    let val: Value = serde_json::from_str(&raw).map_err(|_| {
        "This folder is not a Locust backup, or the backup did not finish: it has no locust-backup.json.".to_owned()
    })?;
    let app = val.get("app").and_then(Value::as_str);
    let files_val = val.get("files").and_then(Value::as_array);
    let manifest_version = val.get("appVersion").and_then(Value::as_str);
    let (Some("Locust"), Some(files_arr), Some(manifest_version)) = (app, files_val, manifest_version) else {
        return Err("This folder's locust-backup.json is not one Locust wrote.".into());
    };
    let Some(schema) = val.get("schema").and_then(Value::as_u64) else {
        return Err(format!(
            "This backup was made by a newer Locust ({manifest_version}). Update Locust, then restore it."
        ));
    };
    if schema > u64::from(BACKUP_SCHEMA) {
        return Err(format!(
            "This backup was made by a newer Locust ({manifest_version}). Update Locust, then restore it."
        ));
    }
    if compare_versions(manifest_version, app_version) == Ordering::Greater {
        return Err(format!(
            "This backup was made by Locust {manifest_version}, newer than this one ({app_version}). Update Locust, then restore it."
        ));
    }

    let mut entries = Vec::with_capacity(files_arr.len());
    let mut total_bytes = 0u64;
    for item in files_arr {
        let named = item.get("path").and_then(Value::as_str);
        let Some(named) = named else {
            let shown = item.get("path").map(ToString::to_string).unwrap_or_else(|| "undefined".into());
            return Err(format!("The backup names a file it should not carry: {shown}."));
        };
        if named.split('/').any(|p| p == ".." || p.is_empty()) || !carries(named) {
            return Err(format!("The backup names a file it should not carry: {named}."));
        }
        let mut path = folder.join("profile");
        for part in named.split('/') {
            path.push(part);
        }
        let Ok(meta) = fs::metadata(&path) else {
            return Err(format!("The backup is missing {named}."));
        };
        let expected_bytes = item.get("bytes").and_then(Value::as_u64);
        let expected_hash = item.get("sha256").and_then(Value::as_str);
        let actual_hash = sha256_file(&path).ok();
        if expected_bytes != Some(meta.len()) || expected_hash != actual_hash.as_deref() {
            return Err(format!(
                "{named} has changed since the backup was made, so the backup cannot be trusted."
            ));
        }
        total_bytes += meta.len();
        entries.push(BackupFileEntry {
            path: named.to_owned(),
            bytes: meta.len(),
            sha256: actual_hash.unwrap_or_default(),
        });
    }

    let counts = val
        .get("counts")
        .map(BackupCounts::from_json)
        .unwrap_or_default();
    let created_at = val
        .get("createdAt")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let left_out = val
        .get("leftOut")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    Ok(BackupReading {
        folder: folder.to_path_buf(),
        manifest: BackupManifest {
            schema: schema as u32,
            app: "Locust".into(),
            app_version: manifest_version.to_owned(),
            created_at,
            counts,
            files: entries,
            left_out,
        },
        bytes: total_bytes,
    })
}

/// The restore asked for: read, refused for a live run, and written down for
/// the next start. The caller restarts the app.
pub fn request_restore(
    folder: &Path,
    profile: &Path,
    app_version: &str,
    live_runs: usize,
    now: DateTime<Utc>,
) -> Result<BackupCounts, String> {
    if live_runs > 0 {
        let prefix = if live_runs == 1 {
            "A run is".to_owned()
        } else {
            format!("{live_runs} runs are")
        };
        let pronoun = if live_runs == 1 { "it has" } else { "they have" };
        return Err(format!(
            "{prefix} going. Restore once {pronoun} finished or been stopped."
        ));
    }
    let reading = read_backup(folder, app_version)?;
    let pending = json!({
        "folder": folder.to_string_lossy(),
        "requestedAt": now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    });
    fs::write(
        profile.join(PENDING_RESTORE_FILE),
        format!("{pending}\n"),
    )
    .map_err(|e| format!("Could not write pending restore: {e}"))?;
    Ok(reading.manifest.counts)
}

fn stamp(at: DateTime<Local>) -> String {
    at.format("%Y%m%d-%H%M%S").to_string()
}

fn write_last_restore(profile: &Path, outcome: &RestoreOutcome) {
    let _ = fs::write(
        profile.join(LAST_RESTORE_FILE),
        format!("{}\n", outcome.to_json()),
    );
}

fn remove_any(path: &Path) {
    if path.is_dir() {
        let _ = fs::remove_dir_all(path);
    } else {
        let _ = fs::remove_file(path);
    }
}

/// AT THE NEXT START, before any store is read: the restore written down, read
/// again (the folder may have changed since), what it replaces moved aside, the
/// backup's files copied in. Anything that fails puts back what was moved.
/// A pending restore, applied only while no engine has the stores in memory. The engine holds `engine.lock` for as long
/// as it runs, and a store writing its old state back over the restored file is the failure that would look like
/// success (profile-backup.ts). So a window or Quick Ask applies it only if it can take the lock, waiting up to `wait`
/// for an engine that is still exiting; the engine itself applies it as it starts (`engine_process::Server`). Left
/// pending when the lock stays taken.
pub fn apply_pending_restore_alone(
    profile: &Path,
    app_version: &str,
    now: DateTime<Local>,
    wait: std::time::Duration,
) -> Option<RestoreOutcome> {
    if !profile.join(PENDING_RESTORE_FILE).is_file() {
        return None;
    }
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(profile.join(crate::engine_process::LOCK))
        .ok()?;
    let end = std::time::Instant::now() + wait;
    loop {
        // SAFETY: flock on a descriptor this function owns; it only reads the descriptor.
        let taken = unsafe { libc::flock(std::os::fd::AsRawFd::as_raw_fd(&lock), libc::LOCK_EX | libc::LOCK_NB) };
        if taken == 0 {
            break;
        }
        if std::time::Instant::now() >= end {
            return None;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    // The lock is let go when `lock` closes, after the restore.
    let outcome = apply_pending_restore(profile, app_version, now);
    drop(lock);
    outcome
}

pub fn apply_pending_restore(
    profile: &Path,
    app_version: &str,
    now: DateTime<Local>,
) -> Option<RestoreOutcome> {
    apply_pending_restore_with_copy(profile, app_version, now, |from, to| {
        fs::copy(from, to).map(|_| ())
    })
}

pub fn apply_pending_restore_with_copy<F>(
    profile: &Path,
    app_version: &str,
    now: DateTime<Local>,
    mut copy: F,
) -> Option<RestoreOutcome>
where
    F: FnMut(&Path, &Path) -> io::Result<()>,
{
    let pending_path = profile.join(PENDING_RESTORE_FILE);
    let raw = fs::read_to_string(&pending_path).ok()?;
    let pending: Value = serde_json::from_str(&raw).ok()?;
    // Taken off first: a restore that fails at every start must not be retried at every start.
    let _ = fs::remove_file(&pending_path);

    let folder_str = pending
        .get("folder")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let at_iso = now
        .with_timezone(&Utc)
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    let reading = match read_backup(Path::new(&folder_str), app_version) {
        Ok(r) => r,
        Err(reason) => {
            let outcome = RestoreOutcome {
                ok: false,
                folder: folder_str,
                at: at_iso,
                aside: None,
                counts: None,
                reason: Some(reason),
            };
            write_last_restore(profile, &outcome);
            return Some(outcome);
        }
    };

    let aside = profile.join(format!("before-restore-{}", stamp(now)));
    let mut moved: Vec<&str> = Vec::new();
    let mut copying = false;

    let work: io::Result<()> = (|| {
        fs::create_dir_all(&aside)?;
        for &name in BACKED_UP_FILES.iter().chain(BACKED_UP_FOLDERS.iter()) {
            let src = profile.join(name);
            if !src.exists() {
                continue;
            }
            fs::rename(&src, aside.join(name))?;
            moved.push(name);
        }
        copying = true;
        for entry in &reading.manifest.files {
            let mut from = PathBuf::from(&folder_str).join("profile");
            let mut to = profile.to_path_buf();
            for part in entry.path.split('/') {
                from.push(part);
                to.push(part);
            }
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent)?;
            }
            copy(&from, &to)?;
        }
        Ok(())
    })();

    match work {
        Ok(()) => {
            let outcome = RestoreOutcome {
                ok: true,
                folder: folder_str,
                at: at_iso,
                aside: Some(aside.to_string_lossy().into_owned()),
                counts: Some(reading.manifest.counts),
                reason: None,
            };
            write_last_restore(profile, &outcome);
            Some(outcome)
        }
        Err(err) => {
            if copying {
                for &name in BACKED_UP_FILES.iter().chain(BACKED_UP_FOLDERS.iter()) {
                    if !moved.contains(&name) {
                        remove_any(&profile.join(name));
                    }
                }
            }
            for &name in &moved {
                remove_any(&profile.join(name));
                let _ = fs::rename(aside.join(name), profile.join(name));
            }
            let _ = fs::remove_dir(&aside);
            let outcome = RestoreOutcome {
                ok: false,
                folder: folder_str,
                at: at_iso,
                aside: None,
                counts: None,
                reason: Some(format!(
                    "The restore could not be finished, and nothing was changed: {err}"
                )),
            };
            write_last_restore(profile, &outcome);
            Some(outcome)
        }
    }
}

/// The last restore's outcome, once: read and taken away.
pub fn take_last_restore(profile: &Path) -> Option<RestoreOutcome> {
    let path = profile.join(LAST_RESTORE_FILE);
    let raw = fs::read_to_string(&path).ok()?;
    let val: Value = serde_json::from_str(&raw).ok()?;
    let _ = fs::remove_file(&path);
    RestoreOutcome::from_json(&val)
}

/// WHAT A BACKUP HOLDS, IN WORDS (Electron `renderer/src/backupWords.ts`).
pub fn counts_line(counts: &BackupCounts) -> String {
    let parts: [(usize, &str, &str); 8] = [
        (counts.teammates, "teammate", "teammates"),
        (counts.routines, "routine", "routines"),
        (counts.memories, "memory", "memories"),
        (counts.rules, "saved approval", "saved approvals"),
        (counts.groups, "group", "groups"),
        (counts.rooms, "room", "rooms"),
        (counts.compares, "comparison", "comparisons"),
        (counts.conversations, "conversation", "conversations"),
    ];
    let said: Vec<String> = parts
        .iter()
        .filter(|(c, _, _)| *c > 0)
        .map(|(c, one, many)| format!("{c} {}", if *c == 1 { *one } else { *many }))
        .collect();
    match said.len() {
        0 => "nothing yet".into(),
        1 => said[0].clone(),
        n => format!("{}, and {}", said[..n - 1].join(", "), said[n - 1])
            .replace(", and ", " and "), // wait: Oxford comma? No! Electron does `${said.slice(0, -1).join(', ')} and ${said.at(-1)!}`
    }
}

/// The last part of a path, as a person reads a folder's name.
pub fn folder_name(path: &str) -> String {
    path.split(['/', '\\'])
        .filter(|p| !p.is_empty())
        .next_back()
        .unwrap_or(path)
        .to_owned()
}

/// What a backup did, in one sentence.
pub fn backed_up_line(folder: &str, counts: &BackupCounts, size: &str) -> String {
    format!(
        "Backed up {} ({size}) to \"{}\".",
        counts_line(counts),
        folder_name(folder)
    )
}

/// What the last restore did, said once after the restart it needed.
pub fn restore_notice_line(outcome: &RestoreOutcome) -> String {
    if !outcome.ok {
        return format!(
            "The restore was not applied, and nothing here changed. {}",
            outcome.reason.as_deref().unwrap_or("")
        )
        .trim()
        .to_owned();
    }
    let what = outcome
        .counts
        .as_ref()
        .map(counts_line)
        .unwrap_or_else(|| "the backup".into());
    let aside = outcome
        .aside
        .as_deref()
        .map(|a| {
            format!(
                " What was here before is kept in \"{}\", in Locust's profile folder.",
                folder_name(a)
            )
        })
        .unwrap_or_default();
    format!(
        "Restored {what} from \"{}\".{aside}",
        folder_name(&outcome.folder)
    )
}

/// Human-readable byte size (matching Electron's `formatBytes`).
pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let kb = bytes as f64 / 1024.0;
    if kb < 1024.0 {
        return format!("{kb:.1} KB");
    }
    let mb = kb / 1024.0;
    if mb < 1024.0 {
        return format!("{mb:.1} MB");
    }
    let gb = mb / 1024.0;
    format!("{gb:.1} GB")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 10, 4, 20, 15, 0).unwrap()
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            crate::native::uuid_v4().unwrap_or_else(|_| "0".into())
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn make_profile(teammates: &[&str]) -> PathBuf {
        let root = temp_dir("locust-backup-profile");
        let tms: Vec<Value> = teammates.iter().map(|n| json!({"name": n})).collect();
        fs::write(root.join("teammates.json"), format!("{}\n", json!({"schemaVersion": 15, "teammates": tms, "missionOwners": {}, "settings": {}}))).unwrap();
        fs::write(root.join("routines.json"), format!("{}\n", json!({"schemaVersion": 3, "routines": [{"name": "Morning brief"}]}))).unwrap();
        fs::write(root.join("memories.json"), format!("{}\n", json!({"schemaVersion": 2, "memories": [{"text": "a"}, {"text": "b"}, {"text": "c"}], "forgotten": []}))).unwrap();
        fs::write(root.join("approval-rules.json"), format!("{}\n", json!({"schemaVersion": 1, "rules": [{"id": "r1"}]}))).unwrap();
        fs::write(root.join("groups.json"), format!("{}\n", json!({"schemaVersion": 1, "groups": [], "members": {}}))).unwrap();
        fs::write(root.join("rooms.json"), format!("{}\n", json!({"schemaVersion": 1, "rooms": [{"id": "room"}]}))).unwrap();
        fs::write(root.join("compares.json"), format!("{}\n", json!({"schemaVersion": 1, "compares": []}))).unwrap();
        fs::write(root.join("folders.json"), format!("{}\n", json!({"schemaVersion": 1, "folders": [{"path": "C:/work"}]}))).unwrap();
        // The machine's and the account's: never carried.
        fs::write(root.join("own-models.json"), format!("{}\n", json!({"keys": {"openai": "sk-live-THIS-MUST-NEVER-LEAVE"}}))).unwrap(); // public-guard: allow
        fs::write(root.join("claude-cloud.json"), format!("{}\n", json!({"sessions": ["s1"]}))).unwrap();
        fs::write(root.join("window.json"), format!("{}\n", json!({"width": 1440}))).unwrap();
        fs::create_dir_all(root.join("mission-ledger/.trash")).unwrap();
        fs::create_dir_all(root.join("mission-ledger/checkpoints/ws_1")).unwrap();
        fs::write(root.join("mission-ledger/checkpoints/ws_1/secret.env"), "SECRET=1\n").unwrap();
        fs::write(root.join("mission-ledger/mission_a.jsonl"), "{\"recordType\":\"mission.created\"}\n").unwrap();
        fs::write(root.join("mission-ledger/mission_b.jsonl"), "{\"recordType\":\"mission.created\"}\n").unwrap();
        fs::write(root.join("mission-ledger/.trash/mission_c.jsonl"), "{\"recordType\":\"mission.created\"}\n").unwrap();
        fs::write(root.join("mission-ledger/.writable"), "").unwrap();
        fs::create_dir_all(root.join("workroom")).unwrap();
        fs::write(root.join("workroom/workroom.jsonl"), "{\"message\":\"hi\"}\n").unwrap();
        root
    }

    fn do_backup(root: &Path) -> PathBuf {
        let parent = temp_dir("locust-backup-to");
        write_backup(root, &parent, "0.614.0", at()).unwrap().folder
    }

    #[test]
    fn backup_creates_folder_with_manifest_and_hashes() {
        let root = make_profile(&["Wren", "Atlas"]);
        let parent = temp_dir("locust-backup-to");
        let written = write_backup(&root, &parent, "0.614.0", at()).unwrap();
        assert_eq!(written.folder, parent.join("Locust backup 2026-10-04 2015"));
        assert_eq!(
            written.counts,
            BackupCounts {
                teammates: 2,
                routines: 1,
                memories: 3,
                rules: 1,
                groups: 0,
                rooms: 1,
                compares: 0,
                folders: 1,
                conversations: 2,
            }
        );
        let manifest: Value =
            serde_json::from_str(&fs::read_to_string(written.folder.join(BACKUP_MANIFEST)).unwrap())
                .unwrap();
        let mut paths: Vec<String> = manifest["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["path"].as_str().unwrap().to_owned())
            .collect();
        paths.sort();
        assert_eq!(
            paths,
            vec![
                "approval-rules.json",
                "compares.json",
                "folders.json",
                "groups.json",
                "memories.json",
                "mission-ledger/.trash/mission_c.jsonl",
                "mission-ledger/mission_a.jsonl",
                "mission-ledger/mission_b.jsonl",
                "rooms.json",
                "routines.json",
                "teammates.json",
                "workroom/workroom.jsonl",
            ]
        );
        assert_eq!(manifest["appVersion"], "0.614.0");
        assert_eq!(manifest["schema"], 1);
        assert!(manifest["leftOut"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v == "own-models.json"));
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn backup_never_carries_keys_signin_or_machine_files() {
        let root = make_profile(&["Wren", "Atlas"]);
        let folder = do_backup(&root);
        let mut everything = String::new();
        fn walk(dir: &Path, out: &mut String) {
            for e in fs::read_dir(dir).unwrap().flatten() {
                if e.path().is_dir() {
                    walk(&e.path(), out);
                } else if let Ok(s) = fs::read_to_string(e.path()) {
                    out.push_str(&s);
                    out.push('\n');
                }
            }
        }
        walk(&folder, &mut everything);
        assert!(!everything.contains("sk-live-THIS-MUST-NEVER-LEAVE")); // public-guard: allow
        assert!(!everything.contains("SECRET=1"));
        for name in [
            "own-models.json",
            "claude-cloud.json",
            "window.json",
            "mission-ledger/.writable",
            "mission-ledger/checkpoints",
        ] {
            let mut p = folder.join("profile");
            for part in name.split('/') {
                p.push(part);
            }
            assert!(!p.exists(), "{name} should not exist in backup");
        }
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(folder.parent().unwrap());
    }

    #[test]
    fn backup_refuses_existing_folder_for_same_minute() {
        let root = make_profile(&["Wren", "Atlas"]);
        let parent = temp_dir("locust-backup-to");
        assert!(write_backup(&root, &parent, "0.614.0", at()).is_ok());
        assert!(write_backup(&root, &parent, "0.614.0", at()).is_err());
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&parent);
    }

    #[test]
    fn reader_validates_manifest_hashes_versions_and_paths() {
        let root = make_profile(&["Wren", "Atlas"]);
        let folder = do_backup(&root);

        let reading = read_backup(&folder, "0.614.0").unwrap();
        assert_eq!(reading.manifest.counts.teammates, 2);

        let empty = temp_dir("locust-not-a-backup");
        let err = read_backup(&empty, "0.614.0").unwrap_err();
        assert!(err.contains("no locust-backup.json"), "{err}");

        let err = read_backup(&folder, "0.613.0").unwrap_err();
        assert!(err.contains("newer than this one") && err.contains("Update Locust"), "{err}");

        // Tamper schema
        let manifest_path = folder.join(BACKUP_MANIFEST);
        let mut manifest: Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest["schema"] = json!(2);
        fs::write(&manifest_path, manifest.to_string()).unwrap();
        let err = read_backup(&folder, "0.614.0").unwrap_err();
        assert!(err.contains("newer Locust"), "{err}");
        manifest["schema"] = json!(1);

        // Tamper disallowed paths
        for bad in ["own-models.json", "../teammates.json", "mission-ledger/../../x"] {
            let mut m = manifest.clone();
            m["files"][0]["path"] = json!(bad);
            fs::write(&manifest_path, m.to_string()).unwrap();
            let err = read_backup(&folder, "0.614.0").unwrap_err();
            assert!(err.contains("should not carry"), "{bad}: {err}");
        }
        fs::write(&manifest_path, manifest.to_string()).unwrap();

        // Tamper file content
        fs::write(
            folder.join("profile/teammates.json"),
            format!("{}\n", json!({"schemaVersion": 15, "teammates": [{"name": "Mallory"}]})),
        )
        .unwrap();
        let err = read_backup(&folder, "0.614.0").unwrap_err();
        assert!(err.contains("teammates.json has changed"), "{err}");

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&empty);
        let _ = fs::remove_dir_all(folder.parent().unwrap());
    }

    #[test]
    fn compare_versions_and_folder_name() {
        assert_eq!(compare_versions("0.614.0", "0.99.0"), Ordering::Greater);
        assert_eq!(compare_versions("0.614.0", "0.614.0"), Ordering::Equal);
        assert_eq!(compare_versions("0.613.9", "0.614.0"), Ordering::Less);
        assert_eq!(backup_folder_name(at()), "Locust backup 2026-10-04 2015");
    }

    #[test]
    fn restore_refused_while_run_is_going() {
        let root = make_profile(&["Wren", "Atlas"]);
        let folder = do_backup(&root);
        let err = request_restore(&folder, &root, "0.614.0", 1, Utc::now()).unwrap_err();
        assert!(err.contains("A run is going"), "{err}");
        assert!(!root.join(PENDING_RESTORE_FILE).exists());
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(folder.parent().unwrap());
    }

    #[test]
    fn restore_applies_at_next_start_moves_aside_and_leaves_keys_untouched() {
        let source = make_profile(&["Wren", "Atlas"]);
        let folder = do_backup(&source);
        let here = make_profile(&["Juno"]);
        fs::write(
            here.join("mission-ledger/mission_z.jsonl"),
            "{\"recordType\":\"mission.created\"}\n",
        )
        .unwrap();

        assert!(request_restore(&folder, &here, "0.614.0", 0, Utc::now()).is_ok());
        let outcome = apply_pending_restore(&here, "0.614.0", at()).unwrap();
        assert!(outcome.ok);
        assert_eq!(outcome.counts.as_ref().unwrap().teammates, 2);
        assert_eq!(outcome.counts.as_ref().unwrap().conversations, 2);

        let tms: Value =
            serde_json::from_str(&fs::read_to_string(here.join("teammates.json")).unwrap()).unwrap();
        assert_eq!(tms["teammates"], json!([{"name": "Wren"}, {"name": "Atlas"}]));

        let mut ledger_names: Vec<String> = fs::read_dir(here.join("mission-ledger"))
            .unwrap()
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n != "checkpoints")
            .collect();
        ledger_names.sort();
        assert_eq!(ledger_names, vec![".trash", "mission_a.jsonl", "mission_b.jsonl"]);

        let aside = PathBuf::from(outcome.aside.as_deref().unwrap());
        let old_tms: Value =
            serde_json::from_str(&fs::read_to_string(aside.join("teammates.json")).unwrap())
                .unwrap();
        assert_eq!(old_tms["teammates"], json!([{"name": "Juno"}]));
        assert!(aside.join("mission-ledger/mission_z.jsonl").exists());

        assert!(fs::read_to_string(here.join("own-models.json"))
            .unwrap()
            .contains("sk-live-THIS-MUST-NEVER-LEAVE")); // public-guard: allow
        assert!(here.join("window.json").exists());

        assert!(!here.join(PENDING_RESTORE_FILE).exists());
        assert!(apply_pending_restore(&here, "0.614.0", at()).is_none());
        assert!(take_last_restore(&here).is_some_and(|o| o.ok));
        assert!(take_last_restore(&here).is_none());

        let _ = fs::remove_dir_all(&source);
        let _ = fs::remove_dir_all(&here);
        let _ = fs::remove_dir_all(folder.parent().unwrap());
    }

    #[test]
    fn a_restore_waits_while_an_engine_holds_the_stores_and_the_engine_applies_it_as_it_starts() {
        let source = make_profile(&["Wren", "Atlas"]);
        let this = env!("CARGO_PKG_VERSION");
        let backup = |root: &Path| write_backup(root, &temp_dir("locust-backup-to"), this, at()).unwrap().folder;
        let folder = backup(&source);
        // The engine's socket path must stay short (a Unix socket name has a small limit).
        let here = std::env::temp_dir().join(format!("lb-{}", std::process::id()));
        let _ = fs::remove_dir_all(&here);
        fs::rename(make_profile(&["Juno"]), &here).unwrap();
        assert!(request_restore(&folder, &here, this, 0, Utc::now()).is_ok());
        let teammates = || fs::read_to_string(here.join("teammates.json")).unwrap();

        // An engine runs here: a window's start leaves the restore pending rather than restoring under it.
        let engine = crate::engine_process::Server::start(&here).unwrap();
        assert!(teammates().contains("Wren"), "the engine applied the pending restore as it took the lock");
        drop(engine);

        // Asked again with the engine gone and nothing pending: nothing happens.
        assert!(apply_pending_restore_alone(&here, this, at(), std::time::Duration::ZERO).is_none());

        // A second restore, requested while an engine holds the lock: the window does not apply it.
        let other = make_profile(&["Mira"]);
        let second = backup(&other);
        let engine = crate::engine_process::Server::start(&here).unwrap();
        assert!(request_restore(&second, &here, this, 0, Utc::now()).is_ok());
        assert!(apply_pending_restore_alone(&here, this, at(), std::time::Duration::from_millis(200)).is_none());
        assert!(here.join(PENDING_RESTORE_FILE).is_file(), "left pending");
        assert!(!teammates().contains("Mira"), "nothing restored under a running engine");
        drop(engine);
        // With the lock free, the next start applies it.
        // As a window's start does: a moment for the lock to be let go (the full suite runs many tests at once).
        let outcome = apply_pending_restore_alone(&here, this, at(), std::time::Duration::from_secs(5))
            .unwrap_or_else(|| panic!("not applied; still pending: {}", here.join(PENDING_RESTORE_FILE).is_file()));
        assert!(outcome.ok);
        assert!(teammates().contains("Mira"));
        let _ = fs::remove_dir_all(&here);
    }

    #[test]
    fn restore_revalidates_at_start_and_rolls_back_on_partial_copy_failure() {
        let source = make_profile(&["Wren"]);
        let folder = do_backup(&source);
        let here = make_profile(&["Juno"]);

        request_restore(&folder, &here, "0.614.0", 0, Utc::now()).unwrap();
        fs::write(
            folder.join("profile/routines.json"),
            format!("{}\n", json!({"schemaVersion": 3, "routines": []})),
        )
        .unwrap();
        let outcome = apply_pending_restore(&here, "0.614.0", at()).unwrap();
        assert!(!outcome.ok);
        assert!(outcome
            .reason
            .as_deref()
            .unwrap()
            .contains("routines.json has changed"));
        let tms: Value =
            serde_json::from_str(&fs::read_to_string(here.join("teammates.json")).unwrap()).unwrap();
        assert_eq!(tms["teammates"], json!([{"name": "Juno"}]));
        assert!(here.join(LAST_RESTORE_FILE).exists());
        assert!(!fs::read_dir(&here)
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with("before-restore-")));

        // Partial copy failure rollback
        let folder2 = do_backup(&make_profile(&["Wren", "Atlas"]));
        let before = fs::read_to_string(here.join("teammates.json")).unwrap();
        request_restore(&folder2, &here, "0.614.0", 0, Utc::now()).unwrap();
        let mut copies = 0usize;
        let outcome = apply_pending_restore_with_copy(&here, "0.614.0", at(), |from, to| {
            copies += 1;
            if copies == 4 {
                return Err(io::Error::other("the disk is full"));
            }
            fs::copy(from, to).map(|_| ())
        })
        .unwrap();
        assert!(!outcome.ok);
        assert!(outcome
            .reason
            .as_deref()
            .unwrap()
            .contains("nothing was changed: the disk is full"));
        assert_eq!(fs::read_to_string(here.join("teammates.json")).unwrap(), before);
        assert!(!fs::read_dir(&here)
            .unwrap()
            .flatten()
            .any(|e| e.file_name().to_string_lossy().starts_with("before-restore-")));

        let _ = fs::remove_dir_all(&source);
        let _ = fs::remove_dir_all(&here);
        let _ = fs::remove_dir_all(folder.parent().unwrap());
        let _ = fs::remove_dir_all(folder2.parent().unwrap());
    }

    #[test]
    fn backup_words_match_electron() {
        let none = BackupCounts::default();
        assert_eq!(
            counts_line(&BackupCounts {
                teammates: 6,
                routines: 1,
                memories: 106,
                conversations: 186,
                ..none.clone()
            }),
            "6 teammates, 1 routine, 106 memories and 186 conversations"
        );
        assert_eq!(
            counts_line(&BackupCounts {
                rules: 1,
                conversations: 1,
                ..none.clone()
            }),
            "1 saved approval and 1 conversation"
        );
        assert_eq!(
            counts_line(&BackupCounts {
                teammates: 1,
                ..none.clone()
            }),
            "1 teammate"
        );
        assert_eq!(counts_line(&none), "nothing yet");
        assert_eq!(
            backed_up_line(
                "C:\\Users\\me\\Backups\\Locust backup 2026-10-04 2015",
                &BackupCounts {
                    teammates: 2,
                    conversations: 3,
                    ..none.clone()
                },
                "1.2 MB"
            ),
            "Backed up 2 teammates and 3 conversations (1.2 MB) to \"Locust backup 2026-10-04 2015\"."
        );
        assert_eq!(folder_name("D:/x/y/"), "y");
        assert_eq!(
            restore_notice_line(&RestoreOutcome {
                ok: true,
                folder: "C:\\b\\Locust backup 2026-10-04 2015".into(),
                at: String::new(),
                aside: Some("C:\\p\\before-restore-20261004-201500".into()),
                counts: Some(BackupCounts {
                    teammates: 2,
                    ..none
                }),
                reason: None,
            }),
            "Restored 2 teammates from \"Locust backup 2026-10-04 2015\". What was here before is kept in \"before-restore-20261004-201500\", in Locust's profile folder."
        );
        assert_eq!(
            restore_notice_line(&RestoreOutcome {
                ok: false,
                folder: "C:\\b".into(),
                at: String::new(),
                aside: None,
                counts: None,
                reason: Some("The backup is missing teammates.json.".into()),
            }),
            "The restore was not applied, and nothing here changed. The backup is missing teammates.json."
        );
    }

    #[test]
    fn every_file_the_native_app_keeps_in_its_profile_is_classified() {
        let core_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let ui_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../src");
        let re_join = regex::Regex::new(r#"(?:store|s)\.root\(\)\.join\("([^"]+)"\)"#).unwrap();
        let re_store = regex::Regex::new(r#"(?:seeded_json|save_json)\(\s*"([^"]+)""#).unwrap();
        let re_const = regex::Regex::new(r#"pub const [A-Z0-9_]+_FILE:\s*&str\s*=\s*"([^"]+)""#).unwrap();

        let mut named = std::collections::BTreeSet::new();
        let mut stack = vec![core_src, ui_src];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else { continue };
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    if p.file_name().is_some_and(|n| n != "tests") {
                        stack.push(p);
                    }
                } else if p.extension().is_some_and(|e| e == "rs") {
                    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    if name.ends_with("_tests.rs") || name == "tests.rs" {
                        continue;
                    }
                    let text = fs::read_to_string(&p).unwrap();
                    let prod = text.split("#[cfg(test)]").next().unwrap_or(&text);
                    for cap in re_join.captures_iter(prod) {
                        named.insert(cap[1].to_owned());
                    }
                    for cap in re_store.captures_iter(prod) {
                        named.insert(cap[1].to_owned());
                    }
                    for cap in re_const.captures_iter(prod) {
                        named.insert(cap[1].to_owned());
                    }
                }
            }
        }
        named.insert(crate::away::AWAY_FILE.to_owned());
        named.insert(crate::folders::FOLDERS_FILE.to_owned());
        named.insert(crate::references::FILE.to_owned());
        named.insert(crate::door::CONNECTION_FILE.to_owned());
        named.insert(crate::door::SETTINGS_FILE.to_owned());

        let mut classified = std::collections::BTreeSet::new();
        for &f in BACKED_UP_FILES {
            classified.insert(f);
        }
        for &f in BACKED_UP_FOLDERS {
            classified.insert(f);
        }
        for &(f, _) in LEFT_OUT {
            classified.insert(f);
        }

        let unclassified: Vec<String> = named
            .into_iter()
            .filter(|n| !classified.contains(n.as_str()))
            .collect();
        assert_eq!(unclassified, Vec::<String>::new(), "Unclassified files in native data folder");
    }
}

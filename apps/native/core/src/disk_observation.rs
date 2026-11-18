//! What a run changed on disk, observed rather than reported — port of `main/disk-observation.ts`.

use crate::approval_patch::{FileChangeKind, FileChangeRecord, unified_patch_text};
use crate::mission_export::{by_helper, reports_a_change};
use crate::runtime::cursor_events::tool_patch_from;
use crate::secrets::scrub_secrets;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

pub const MAX_COMPARED_FILE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_OBSERVED_FILE_BYTES: usize = 64 * 1024;
pub const MAX_UNTRACKED_TEXTS: usize = 400;
pub const MAX_UNTRACKED_TEXT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_UNTRACKED_LOOKED_AT: usize = 5_000;
pub const MAX_FOLDER_DEPTH: usize = 8;
pub const WRITTEN_SLACK_MS: u64 = 2_000;

const UNTRACKED: &str = "??";
const TEXT_MARK: char = '\x01';
const STAMP_MARK: char = '\x02';

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceSnapshot {
    pub entries: BTreeMap<String, String>,
    pub partial: bool,
    pub taken_at: Option<u64>,
    pub written_at: BTreeMap<String, u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStat {
    pub size: u64,
    pub mtime_ms: u64,
    pub ctime_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
    Link,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderEntry {
    pub name: String,
    pub kind: EntryKind,
}

#[derive(Clone, Debug)]
pub struct GitOutputError {
    pub stdout: String,
    pub message: String,
}

pub trait DiskObservationHost {
    fn run_git(&self, args: &[&str], cwd: &Path) -> Result<String, GitOutputError>;
    fn stat_of(&self, path: &Path) -> Option<FileStat>;
    fn read_text(&self, path: &Path) -> Option<String>;
    fn read_text_up_to(&self, path: &Path, max: usize) -> Option<String>;
    fn list_directory(&self, dir: &Path) -> io::Result<Vec<FolderEntry>>;
    fn now_ms(&self) -> u64;
    fn before_text(&self, _rel: &str) -> Option<String> {
        None
    }
}

pub struct DefaultDiskHost<'a> {
    before_lookup: Option<&'a dyn Fn(&str) -> Option<String>>,
}

impl<'a> DefaultDiskHost<'a> {
    pub fn new() -> Self {
        Self { before_lookup: None }
    }

    pub fn with_before_text(before_lookup: &'a dyn Fn(&str) -> Option<String>) -> Self {
        Self {
            before_lookup: Some(before_lookup),
        }
    }
}

impl Default for DefaultDiskHost<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl DiskObservationHost for DefaultDiskHost<'_> {
    fn run_git(&self, args: &[&str], cwd: &Path) -> Result<String, GitOutputError> {
        let mut cmd = Command::new("git");
        cmd.args([
            "-c",
            "core.quotePath=false",
            "-c",
            "core.fsmonitor=false",
            "-c",
            "core.untrackedCache=false",
        ]);
        cmd.args(args);
        cmd.current_dir(cwd);
        let out = cmd.output().map_err(|e| GitOutputError {
            stdout: String::new(),
            message: e.to_string(),
        })?;
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        if out.status.success() {
            Ok(stdout)
        } else {
            Err(GitOutputError {
                stdout,
                message: String::from_utf8_lossy(&out.stderr).into_owned(),
            })
        }
    }

    fn stat_of(&self, path: &Path) -> Option<FileStat> {
        let meta = fs::metadata(path).ok()?;
        if !meta.is_file() {
            return None;
        }
        let mtime_ms = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        #[cfg(unix)]
        let ctime_ms = {
            use std::os::unix::fs::MetadataExt;
            Some((meta.ctime() as u64).saturating_mul(1000).saturating_add((meta.ctime_nsec() as u64) / 1_000_000))
        };
        #[cfg(not(unix))]
        let ctime_ms = None;
        Some(FileStat {
            size: meta.len(),
            mtime_ms,
            ctime_ms,
        })
    }

    fn read_text(&self, path: &Path) -> Option<String> {
        self.read_text_up_to(path, MAX_OBSERVED_FILE_BYTES)
    }

    fn read_text_up_to(&self, path: &Path, max: usize) -> Option<String> {
        // Never through a link: one inside the project can point anywhere (a key file), and its text would land in the
        // conversation's record. Electron's readFile follows it; this port does not (handoff: never follow a link out).
        if fs::symlink_metadata(path).ok()?.file_type().is_symlink() {
            return None;
        }
        let meta = fs::metadata(path).ok()?;
        if !meta.is_file() || meta.len() > max as u64 {
            return None;
        }
        let file = fs::File::open(path).ok()?;
        let mut buf = Vec::new();
        file.take((max + 1) as u64).read_to_end(&mut buf).ok()?;
        if buf.len() > max || buf.contains(&0) {
            return None;
        }
        String::from_utf8(buf).ok()
    }

    fn list_directory(&self, dir: &Path) -> io::Result<Vec<FolderEntry>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let ft = entry.file_type()?;
            let kind = if ft.is_symlink() {
                EntryKind::Link
            } else if ft.is_dir() {
                EntryKind::Dir
            } else if ft.is_file() {
                EntryKind::File
            } else {
                EntryKind::Other
            };
            entries.push(FolderEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                kind,
            });
        }
        Ok(entries)
    }

    fn now_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn before_text(&self, rel: &str) -> Option<String> {
        self.before_lookup.and_then(|f| f(rel))
    }
}

fn last_touched(seen: FileStat) -> u64 {
    seen.mtime_ms.max(seen.ctime_ms.unwrap_or(0))
}

pub fn holds_secrets(path: &str) -> bool {
    static PART_RE: OnceLock<Regex> = OnceLock::new();
    static EXT_RE: OnceLock<Regex> = OnceLock::new();
    static KEY_RE: OnceLock<Regex> = OnceLock::new();
    let part_re = PART_RE.get_or_init(|| {
        Regex::new(r"(?i)credential|secret|token|auth|cookie|session|password|passwd|private[-_]?key|keychain|keystore|wallet").unwrap()
    });
    let ext_re = EXT_RE.get_or_init(|| {
        Regex::new(r"(?i)\.(key|pem|p12|pfx|jks|kdbx|gpg|asc|crt|cer|der|ovpn|ppk)$").unwrap()
    });
    let key_re = KEY_RE.get_or_init(|| Regex::new(r"(?i)^id_(rsa|dsa|ecdsa|ed25519)").unwrap());
    path.split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .any(|part| {
            part.starts_with('.')
                || part_re.is_match(part)
                || ext_re.is_match(part)
                || key_re.is_match(part)
        })
}

fn is_skipped_folder(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name,
            "node_modules" | "__pycache__" | "venv" | "site-packages"
        )
}

fn is_skipped_file(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.starts_with("~$")
        || matches!(lower.as_str(), "desktop.ini" | "thumbs.db" | ".ds_store")
}

pub fn status_of(entry: Option<&str>) -> Option<&str> {
    entry.and_then(|s| s.get(..2))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EntryDataKind {
    Text,
    Stamp,
    Status,
}

fn kind_of(entry: &str) -> EntryDataKind {
    if entry.contains(TEXT_MARK) {
        EntryDataKind::Text
    } else if entry.contains(STAMP_MARK) {
        EntryDataKind::Stamp
    } else {
        EntryDataKind::Status
    }
}

pub fn text_of(entry: Option<&str>) -> Option<&str> {
    entry.and_then(|s| s.find(TEXT_MARK).map(|at| &s[at + 1..]))
}

pub fn parse_porcelain(output: &str) -> WorkspaceSnapshot {
    parse_porcelain_ordered(output).0
}

fn parse_porcelain_ordered(output: &str) -> (WorkspaceSnapshot, Vec<String>) {
    let mut entries = BTreeMap::new();
    let mut order = Vec::new();
    let fields: Vec<&str> = output.split('\0').collect();
    let mut i = 0;
    while i < fields.len() {
        let entry = fields[i];
        i += 1;
        if entry.len() < 4 {
            continue;
        }
        let status = &entry[..2];
        let path = &entry[3..];
        if !entries.contains_key(path) {
            order.push(path.to_owned());
        }
        entries.insert(path.to_owned(), status.to_owned());
        if status.starts_with('R') || status.starts_with('C') {
            i += 1;
        }
    }
    (
        WorkspaceSnapshot {
            entries,
            ..Default::default()
        },
        order,
    )
}

fn carry_untracked(
    snapshot: &mut BTreeMap<String, String>,
    order: &[String],
    workspace_path: &Path,
    host: &dyn DiskObservationHost,
    mut written_at: Option<&mut BTreeMap<String, u64>>,
) {
    let paths: Vec<String> = order
        .iter()
        .filter(|p| snapshot.get(*p).is_some_and(|s| s.as_str() == UNTRACKED))
        .cloned()
        .collect();
    let mut looked_at = 0usize;
    let mut texts = 0usize;
    let mut text_bytes = 0usize;
    for path in paths {
        let absolute = workspace_path.join(&path);
        if holds_secrets(&path) {
            if let Some(seen) = host.stat_of(&absolute) {
                snapshot.insert(
                    path.clone(),
                    format!("{UNTRACKED}{STAMP_MARK}{}:{}", seen.size, seen.mtime_ms),
                );
                if let Some(w) = written_at.as_deref_mut() {
                    w.insert(path, last_touched(seen));
                }
            }
            continue;
        }
        if looked_at >= MAX_UNTRACKED_LOOKED_AT {
            break;
        }
        looked_at += 1;
        let seen = host.stat_of(&absolute);
        if let Some(seen) = seen
            && let Some(w) = written_at.as_deref_mut()
        {
            w.insert(path.clone(), last_touched(seen));
        }
        let size = seen.map(|s| s.size as usize).unwrap_or(0);
        if size <= MAX_OBSERVED_FILE_BYTES
            && texts < MAX_UNTRACKED_TEXTS
            && text_bytes + size <= MAX_UNTRACKED_TEXT_BYTES
            && let Some(text) = host.read_text(&absolute)
        {
            texts += 1;
            text_bytes += text.len();
            snapshot.insert(path, format!("{UNTRACKED}{TEXT_MARK}{text}"));
            continue;
        }
        if let Some(seen) = seen {
            snapshot.insert(
                path,
                format!("{UNTRACKED}{STAMP_MARK}{}:{}", seen.size, seen.mtime_ms),
            );
        }
    }
}

pub fn snapshot_workspace(
    workspace_path: &Path,
    host: &dyn DiskObservationHost,
) -> Option<WorkspaceSnapshot> {
    let output = match host.run_git(
        &["status", "--porcelain", "-z", "--untracked-files=all"],
        workspace_path,
    ) {
        Ok(out) => out,
        Err(_) => return snapshot_folder(workspace_path, host),
    };
    let (mut snapshot, order) = parse_porcelain_ordered(&output);
    carry_untracked(&mut snapshot.entries, &order, workspace_path, host, None);
    let keys: Vec<(String, String)> = snapshot
        .entries
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    let mut stamped = 0usize;
    for (path, status) in keys {
        if status == UNTRACKED || status.starts_with(UNTRACKED) || status.chars().take(2).any(|c| c == 'D') {
            continue;
        }
        if stamped >= MAX_UNTRACKED_LOOKED_AT {
            break;
        }
        stamped += 1;
        if let Some(seen) = host.stat_of(&workspace_path.join(&path)) {
            snapshot.entries.insert(
                path,
                format!("{status}{STAMP_MARK}{}:{}", seen.size, seen.mtime_ms),
            );
        }
    }
    Some(snapshot)
}

pub fn snapshot_folder(
    workspace_path: &Path,
    host: &dyn DiskObservationHost,
) -> Option<WorkspaceSnapshot> {
    let taken_at = host.now_ms();
    let mut found = Vec::new();
    let mut partial = false;
    let mut queue = VecDeque::new();
    queue.push_back((workspace_path.to_path_buf(), String::new(), 0usize));

    'walk: while let Some((dir, relative, depth)) = queue.pop_front() {
        let entries = match host.list_directory(&dir) {
            Ok(list) => list,
            Err(_) => {
                if depth == 0 {
                    return None;
                }
                partial = true;
                continue;
            }
        };
        for entry in entries {
            let rel = if relative.is_empty() {
                entry.name.clone()
            } else {
                format!("{relative}/{}", entry.name)
            };
            match entry.kind {
                EntryKind::Dir => {
                    if is_skipped_folder(&entry.name) {
                        continue;
                    }
                    if depth + 1 > MAX_FOLDER_DEPTH {
                        partial = true;
                        continue;
                    }
                    queue.push_back((dir.join(&entry.name), rel, depth + 1));
                }
                EntryKind::File if !is_skipped_file(&entry.name) => {
                    if found.len() >= MAX_UNTRACKED_LOOKED_AT {
                        partial = true;
                        break 'walk;
                    }
                    found.push(rel);
                }
                _ => {}
            }
        }
    }

    let mut entries: BTreeMap<String, String> =
        found.iter().map(|p| (p.clone(), UNTRACKED.to_owned())).collect();
    let mut written_at = BTreeMap::new();
    carry_untracked(&mut entries, &found, workspace_path, host, Some(&mut written_at));
    Some(WorkspaceSnapshot {
        entries,
        partial,
        taken_at: Some(taken_at),
        written_at,
    })
}

pub fn changed_paths(before: &WorkspaceSnapshot, after: &WorkspaceSnapshot) -> Vec<String> {
    let mut paths = BTreeSet::new();
    let shifted = |path: &str, was: Option<&str>, now: &str| -> bool {
        let is_shifted = match was {
            None => before.partial,
            Some(w) => (before.partial || after.partial) && kind_of(w) != kind_of(now),
        };
        if !is_shifted {
            return false;
        }
        let Some(written) = after.written_at.get(path).copied() else {
            return false;
        };
        let Some(taken_at) = before.taken_at else {
            return false;
        };
        written < taken_at.saturating_sub(WRITTEN_SLACK_MS)
    };

    for (path, status) in &after.entries {
        let was = before.entries.get(path).map(String::as_str);
        if was != Some(status.as_str()) && !shifted(path, was, status) {
            paths.insert(path.clone());
        }
    }
    let whole = !before.partial && !after.partial;
    if whole {
        for path in before.entries.keys() {
            if !after.entries.contains_key(path) {
                paths.insert(path.clone());
            }
        }
    }
    paths.into_iter().collect()
}

pub fn unreported_paths(changed: &[String], events: &[Value]) -> Vec<String> {
    let mut named = Vec::new();
    for event in events {
        let ty = event["type"].as_str().unwrap_or_default();
        if !matches!(ty, "tool.started" | "tool.completed" | "tool.failed") {
            continue;
        }
        let payload = &event["payload"];
        let name = payload["name"].as_str().unwrap_or_default();
        let tool_kind = payload["toolKind"].as_str();
        let command = payload["command"].as_str();
        if !reports_a_change(name, tool_kind, command) || by_helper(payload) {
            continue;
        }
        named.push(name.to_lowercase());
        if let Some(cmd) = command {
            named.push(cmd.to_lowercase());
        }
    }
    changed
        .iter()
        .filter(|path| {
            let tail = path
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(path.as_str())
                .to_lowercase();
            !named.iter().any(|text| text.contains(&tail))
        })
        .cloned()
        .collect()
}

fn diff_texts(
    host: &dyn DiskObservationHost,
    workspace_path: &Path,
    path: &str,
    earlier: &str,
    later: &str,
) -> String {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("locust-observed-{}-{stamp}", std::process::id()));
    if fs::create_dir_all(&dir).is_err() {
        return String::new();
    }
    let a = dir.join("a");
    let b = dir.join("b");
    let out = if fs::write(&a, earlier).is_ok() && fs::write(&b, later).is_ok() {
        let a_s = a.to_string_lossy().into_owned();
        let b_s = b.to_string_lossy().into_owned();
        match host.run_git(
            &[
                "diff",
                "--no-color",
                "--no-ext-diff",
                "--no-textconv",
                "--no-index",
                "--",
                &a_s,
                &b_s,
            ],
            workspace_path,
        ) {
            Ok(s) => s,
            Err(e) => e.stdout,
        }
    } else {
        String::new()
    };
    let _ = fs::remove_dir_all(&dir);
    let Some(hunk_at) = out.find("\n@@") else {
        return String::new();
    };
    format!("--- a/{path}\n+++ b/{path}{}", &out[hunk_at..])
}

pub fn observed_patches(
    workspace_path: &Path,
    after: &WorkspaceSnapshot,
    paths: &[String],
    host: &dyn DiskObservationHost,
    before: Option<&WorkspaceSnapshot>,
) -> BTreeMap<String, Value> {
    let mut patches = BTreeMap::new();
    for path in paths {
        if holds_secrets(path) {
            continue;
        }
        let entry = after.entries.get(path).map(String::as_str);
        if status_of(entry) == Some(UNTRACKED) {
            let mut read = text_of(entry)
                .map(str::to_owned)
                .or_else(|| host.read_text(&workspace_path.join(path)));
            let mut found = text_of(before.and_then(|b| b.entries.get(path).map(String::as_str)))
                .map(str::to_owned);
            let before_untracked =
                status_of(before.and_then(|b| b.entries.get(path).map(String::as_str)))
                    == Some(UNTRACKED);
            if found.is_none()
                && before_untracked
                && let Some(kept) = host.before_text(path)
            {
                found = Some(kept);
                if read.is_none() {
                    read = host.read_text_up_to(&workspace_path.join(path), MAX_COMPARED_FILE_BYTES);
                }
            }
            let Some(read) = read.filter(|r| !r.is_empty()) else {
                continue;
            };
            let text = scrub_secrets(&read).0;
            let earlier = found.map(|f| scrub_secrets(&f).0);
            if earlier.is_none() && before_untracked {
                continue;
            }
            let unified = match earlier {
                None => unified_patch_text(
                    &[FileChangeRecord {
                        path: path.clone(),
                        kind: FileChangeKind::Add,
                        move_path: None,
                        diff: text,
                    }],
                    None,
                ),
                Some(earlier) => diff_texts(host, workspace_path, path, &earlier, &text),
            };
            if let Some(patch) = tool_patch_from(&unified) {
                patches.insert(path.clone(), patch);
            }
            continue;
        }
        let Ok(mut unified) = host.run_git(
            &[
                "diff",
                "--no-color",
                "--no-ext-diff",
                "--no-textconv",
                "--",
                path,
            ],
            workspace_path,
        ) else {
            continue;
        };
        if unified.trim().is_empty() {
            let Ok(cached) = host.run_git(
                &[
                    "diff",
                    "--no-color",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--cached",
                    "--",
                    path,
                ],
                workspace_path,
            ) else {
                continue;
            };
            unified = cached;
        }
        let scrubbed = scrub_secrets(&unified).0;
        if let Some(patch) = tool_patch_from(&scrubbed) {
            patches.insert(path.clone(), patch);
        }
    }
    patches
}

pub fn shared_tree_notice(
    run_id: &str,
    mission_id: &str,
    source_adapter: &str,
    next_sequence: u64,
    at: &str,
    paths: &[String],
) -> Value {
    let count = paths.len();
    let msg = format!(
        "Another teammate was working in this folder at the same time, so what changed on disk cannot be told apart. {count} {} in the folder {} different from before this run; {}",
        if count == 1 { "file" } else { "files" },
        if count == 1 { "is" } else { "are" },
        if count == 1 {
            "it is not counted as this run's work."
        } else {
            "none of them are counted as this run's work."
        }
    );
    json!({
        "runId": run_id,
        "missionId": mission_id,
        "occurredAt": at,
        "sourceAdapter": source_adapter,
        "id": format!("{run_id}:disk:shared:{next_sequence}"),
        "sequence": next_sequence,
        "type": "adapter.diagnostic",
        "payload": {
            "level": "info",
            "code": "host.shared_workspace",
            "message": msg,
            "terminal": false,
            "evidence": { "redacted": true }
        }
    })
}

pub fn observed_edit_events(
    run_id: &str,
    mission_id: &str,
    source_adapter: &str,
    next_sequence: u64,
    paths: &[String],
    at: &str,
    patches: &BTreeMap<String, Value>,
    reported: &BTreeSet<String>,
) -> Vec<Value> {
    let mut events = Vec::with_capacity(paths.len() * 2);
    let mut sequence = next_sequence;
    for (index, path) in paths.iter().enumerate() {
        let item_id = format!("disk-observed-{}", index + 1);
        let status = if !reported.contains(path) {
            "observed on disk"
        } else if patches.contains_key(path) {
            "reported by the runtime, read from disk"
        } else {
            "reported by the runtime, changed on disk"
        };
        let started_payload = json!({
            "itemId": item_id,
            "toolKind": "observed_edit",
            "name": "edit",
            "command": path,
            "status": status,
            "phase": "started",
            "evidence": { "redacted": true }
        });
        events.push(json!({
            "runId": run_id,
            "missionId": mission_id,
            "occurredAt": at,
            "sourceAdapter": source_adapter,
            "id": format!("{run_id}:disk:{sequence}"),
            "sequence": sequence,
            "type": "tool.started",
            "payload": started_payload
        }));
        sequence += 1;
        let mut completed_payload = json!({
            "itemId": item_id,
            "toolKind": "observed_edit",
            "name": "edit",
            "command": path,
            "status": status,
            "phase": "completed",
            "evidence": { "redacted": true }
        });
        if let Some(patch) = patches.get(path) {
            completed_payload["patch"] = patch.clone();
        }
        events.push(json!({
            "runId": run_id,
            "missionId": mission_id,
            "occurredAt": at,
            "sourceAdapter": source_adapter,
            "id": format!("{run_id}:disk:{sequence}"),
            "sequence": sequence,
            "type": "tool.completed",
            "payload": completed_payload
        }));
        sequence += 1;
    }
    events
}

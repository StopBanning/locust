//! Import conversations from Claude Code, Codex, OpenCode, or Cursor Agent.
//!
//! Ports `apps/desktop/src/main/session-import.ts` and `apps/desktop/src/main/agent-session-import.ts`.
//! Nothing is read whole to list; only the chosen session is read to import, and Locust's record
//! goes into the native data folder only (never into an agent's own folder or Electron's folder).

use crate::folders;
use crate::people;
use crate::runtime_terminal::{self, Exchange};
use crate::store::Store;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

const HEAD_BYTES: usize = 64 * 1024;
const TAIL_BYTES: usize = 256 * 1024;
const OPEN_NOW_MS: i64 = 2 * 60_000;
const MAX_TITLE: usize = 90;
const MAX_PROMPT: usize = 8_000;
const MAX_ANSWER: usize = 16_000;
pub const MAX_IMPORTED_EXCHANGES: usize = 200;
pub const DEFAULT_DAYS: i64 = 30;
const MAX_CURSOR_BLOB: usize = 16 * 1024 * 1024;
const MAX_CURSOR_META_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImportRuntime {
    Claude,
    Codex,
    Cursor,
    Opencode,
}

impl ImportRuntime {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Cursor => "cursor",
            Self::Opencode => "opencode",
        }
    }

    pub fn display_name(self) -> &'static str {
        crate::engine::runtime_name(self.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportableSession {
    pub runtime: ImportRuntime,
    pub session_id: String,
    pub title: String,
    pub cwd: String,
    pub folder_name: String,
    pub updated_at: String,
    pub bytes: u64,
    pub open_now: bool,
    pub folder_missing: bool,
}

#[derive(Clone, Debug)]
pub struct SessionPlaces {
    pub claude_home: PathBuf,
    pub codex_home: PathBuf,
    pub cursor_home: Option<PathBuf>,
}

impl SessionPlaces {
    /// Real user home folders for the live window launch. Never called in tests.
    pub fn from_env() -> Option<Self> {
        let home = std::env::var_os("HOME")
            .filter(|h| !h.is_empty())
            .map(PathBuf::from)?;
        let claude_home = std::env::var_os("CLAUDE_CONFIG_DIR")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".claude"));
        let codex_home = std::env::var_os("CODEX_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".codex"));
        let cursor_home = Some(home.join(".cursor"));
        Some(Self {
            claude_home,
            codex_home,
            cursor_home,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct ListOptions {
    pub now_ms: Option<i64>,
    pub days: Option<i64>,
    pub skip: HashSet<String>,
    pub other_sessions: Vec<ImportableSession>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportSuccess {
    pub mission_id: String,
    pub first_mission_id: String,
    pub title: String,
    pub turns: usize,
    pub skipped: usize,
}

fn now_utc_ms() -> i64 {
    Utc::now().timestamp_millis()
}

fn iso_from_ms(ms: i64) -> Option<String> {
    if ms.abs() >= 8_640_000_000_000_000 {
        return None;
    }
    DateTime::<Utc>::from_timestamp_millis(ms)
        .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

fn parse_iso_ms(iso: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(iso)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

fn one_line(text: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let count = flat.chars().count();
    if count <= MAX_TITLE {
        flat
    } else {
        let truncated: String = flat.chars().take(MAX_TITLE - 1).collect();
        format!("{}…", truncated.trim_end())
    }
}

fn bounded(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_owned()
    } else {
        let prefix: String = text.chars().take(max.saturating_sub(1)).collect();
        format!("{prefix}…")
    }
}

fn read_slice(path: &Path, from: u64, length: usize) -> io::Result<String> {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(from))?;
    let mut buf = vec![0u8; length];
    let mut read_total = 0;
    while read_total < length {
        let n = file.read(&mut buf[read_total..])?;
        if n == 0 {
            break;
        }
        read_total += n;
    }
    Ok(String::from_utf8_lossy(&buf[..read_total]).into_owned())
}

fn lines_of(text: &str, drop_first: bool) -> Vec<Value> {
    let mut parts: Vec<&str> = text.split('\n').collect();
    if drop_first && !parts.is_empty() {
        parts.remove(0);
    }
    parts.pop();
    parts
        .into_iter()
        .filter_map(|line| {
            let v: Value = serde_json::from_str(line).ok()?;
            v.is_object().then_some(v)
        })
        .collect()
}

fn claude_typed(entry: &Value) -> Option<String> {
    if entry["type"] != "user"
        || entry["isSidechain"] == true
        || entry["isMeta"] == true
        || entry["isCompactSummary"] == true
    {
        return None;
    }
    let content = &entry["message"]["content"];
    let said = if let Some(s) = content.as_str() {
        Some(s.to_owned())
    } else if let Some(arr) = content.as_array() {
        if arr.iter().any(|b| b["type"] == "tool_result") {
            None
        } else {
            let joined = arr
                .iter()
                .filter(|b| b["type"] == "text")
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            Some(joined)
        }
    } else {
        None
    }?;
    let trimmed = said.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('<')
        || trimmed.starts_with("[Request interrupted")
    {
        return None;
    }
    Some(trimmed.to_owned())
}

fn locust_cloud_worktree_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)[\\/]\.claude[\\/]worktrees[\\/]locust-cloud-[a-f0-9]+[\\/]?$").unwrap()
    })
}

fn claude_session(path: &Path, bytes: u64, mtime_ms: i64, now_ms: i64) -> Option<ImportableSession> {
    let head_len = (bytes as usize).min(HEAD_BYTES);
    let head = lines_of(&read_slice(path, 0, head_len).ok()?, false);
    let tail = if bytes <= HEAD_BYTES as u64 {
        head.clone()
    } else {
        let tail_len = (bytes as usize).min(TAIL_BYTES);
        let from = bytes.saturating_sub(tail_len as u64);
        lines_of(&read_slice(path, from, tail_len).ok()?, true)
    };
    if head
        .iter()
        .any(|e| e["isSidechain"] == true && e["type"] == "user")
    {
        return None;
    }
    let seen: Vec<&Value> = head.iter().chain(tail.iter()).collect();
    let by_person = seen.iter().copied().find(|e| {
        e["entrypoint"]
            .as_str()
            .is_some_and(|ep| ep != "sdk-cli")
            && e["sessionId"].is_string()
            && e["cwd"].is_string()
    })?;
    let session_id = by_person["sessionId"].as_str()?.to_owned();
    let cwd = by_person["cwd"].as_str()?.to_owned();
    if locust_cloud_worktree_re().is_match(&cwd) {
        return None;
    }
    let mut title: Option<String> = None;
    for entry in &seen {
        if entry["type"] == "custom-title"
            && let Some(t) = entry["customTitle"].as_str()
        {
            title = Some(t.to_owned());
        } else if entry["type"] == "ai-title"
            && let Some(t) = entry["aiTitle"].as_str()
            && title.is_none()
        {
            title = Some(t.to_owned());
        }
    }
    let typed = head
        .iter()
        .find_map(claude_typed)
        .or_else(|| tail.iter().rev().find_map(claude_typed));
    if title.is_none() && typed.is_none() {
        return None;
    }
    let resolved_title = one_line(
        title
            .as_deref()
            .or(typed.as_deref())
            .unwrap_or("A Claude Code session"),
    );
    let folder_name = folders::folder_name(&cwd);
    Some(ImportableSession {
        runtime: ImportRuntime::Claude,
        session_id,
        title: resolved_title,
        cwd,
        folder_name,
        updated_at: iso_from_ms(mtime_ms)?,
        bytes,
        open_now: now_ms - mtime_ms < OPEN_NOW_MS,
        folder_missing: false,
    })
}

fn codex_names(codex_home: &Path) -> HashMap<String, String> {
    let mut names = HashMap::new();
    let Ok(text) = fs::read_to_string(codex_home.join("session_index.jsonl")) else {
        return names;
    };
    for line in text.lines() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if let (Some(id), Some(thread_name)) =
            (entry["id"].as_str(), entry["thread_name"].as_str())
            && !thread_name.trim().is_empty()
        {
            names.insert(id.to_owned(), thread_name.to_owned());
        }
    }
    names
}

fn codex_session(
    path: &Path,
    bytes: u64,
    mtime_ms: i64,
    now_ms: i64,
    names: &HashMap<String, String>,
) -> Option<ImportableSession> {
    let head_len = (bytes as usize).min(HEAD_BYTES);
    let head = lines_of(&read_slice(path, 0, head_len).ok()?, false);
    let meta = head.iter().find(|e| e["type"] == "session_meta")?;
    let payload = meta["payload"].as_object()?;
    let session_id = payload.get("id")?.as_str()?.to_owned();
    let cwd = payload.get("cwd")?.as_str()?.to_owned();
    let originator = payload
        .get("originator")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let source = match payload.get("source") {
        Some(v) if !v.is_null() => serde_json::to_string(v).unwrap_or_default(),
        _ => String::new(),
    };
    let orig_lower = originator.to_ascii_lowercase();
    let src_lower = source.to_ascii_lowercase();
    if orig_lower.starts_with("locust")
        || orig_lower.contains("exec")
        || src_lower.contains("exec")
        || src_lower.contains("subagent")
    {
        return None;
    }
    let first_prompt = head
        .iter()
        .filter(|e| {
            e["type"] == "response_item"
                && e["payload"]["type"] == "message"
                && e["payload"]["role"] == "user"
        })
        .filter_map(|e| {
            let arr = e["payload"]["content"].as_array()?;
            let joined = arr
                .iter()
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let trimmed = joined.trim().to_owned();
            if trimmed.is_empty()
                || trimmed.starts_with('<')
                || trimmed.starts_with("# AGENTS.md")
            {
                None
            } else {
                Some(trimmed)
            }
        })
        .next();
    if first_prompt.is_none() && !names.contains_key(&session_id) {
        return None;
    }
    let resolved_title = one_line(
        names
            .get(&session_id)
            .map(String::as_str)
            .or(first_prompt.as_deref())
            .unwrap_or("A Codex session"),
    );
    let folder_name = folders::folder_name(&cwd);
    Some(ImportableSession {
        runtime: ImportRuntime::Codex,
        session_id,
        title: resolved_title,
        cwd,
        folder_name,
        updated_at: iso_from_ms(mtime_ms)?,
        bytes,
        open_now: now_ms - mtime_ms < OPEN_NOW_MS,
        folder_missing: false,
    })
}

fn file_mtime_ms(meta: &fs::Metadata) -> Option<i64> {
    let modified = meta.modified().ok()?;
    let dur = modified.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(dur.as_millis()).ok()
}

pub fn list_importable_sessions(
    places: &SessionPlaces,
    options: ListOptions,
) -> Vec<ImportableSession> {
    let now_ms = options.now_ms.unwrap_or_else(now_utc_ms);
    let days = options.days.unwrap_or(DEFAULT_DAYS);
    let since_ms = now_ms - days * 86_400_000;
    let skip = &options.skip;

    let mut found: Vec<ImportableSession> = options
        .other_sessions
        .into_iter()
        .filter(|s| {
            !skip.contains(&s.session_id)
                && parse_iso_ms(&s.updated_at).is_some_and(|ms| ms >= since_ms)
        })
        .collect();

    // Claude Code: <claudeHome>/projects/<folder>/<session>.jsonl
    let projects = places.claude_home.join("projects");
    if let Ok(folders_iter) = fs::read_dir(&projects) {
        for folder_entry in folders_iter.flatten() {
            let Ok(files_iter) = fs::read_dir(folder_entry.path()) else {
                continue;
            };
            for file_entry in files_iter.flatten() {
                let path = file_entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let Ok(meta) = fs::metadata(&path) else {
                    continue;
                };
                if !meta.is_file() || meta.len() == 0 {
                    continue;
                }
                let Some(mtime_ms) = file_mtime_ms(&meta) else {
                    continue;
                };
                if mtime_ms < since_ms {
                    continue;
                }
                if let Some(session) = claude_session(&path, meta.len(), mtime_ms, now_ms)
                    && !skip.contains(&session.session_id)
                {
                    found.push(session);
                }
            }
        }
    }

    // Codex: <codexHome>/sessions/YYYY/MM/DD/rollout-*.jsonl
    let names = codex_names(&places.codex_home);
    let sessions_dir = places.codex_home.join("sessions");
    if let Ok(years) = fs::read_dir(&sessions_dir) {
        for year_entry in years.flatten() {
            let Ok(year) = year_entry.file_name().to_string_lossy().parse::<i32>() else {
                continue;
            };
            let Ok(months) = fs::read_dir(year_entry.path()) else {
                continue;
            };
            for month_entry in months.flatten() {
                let Ok(month) = month_entry.file_name().to_string_lossy().parse::<u32>() else {
                    continue;
                };
                let Ok(days_dir) = fs::read_dir(month_entry.path()) else {
                    continue;
                };
                for day_entry in days_dir.flatten() {
                    let Ok(day) = day_entry.file_name().to_string_lossy().parse::<u32>() else {
                        continue;
                    };
                    if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
                        let day_start = Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()).timestamp_millis();
                        if day_start + 86_400_000 * 2 < since_ms {
                            continue;
                        }
                    }
                    let Ok(files) = fs::read_dir(day_entry.path()) else {
                        continue;
                    };
                    for file_entry in files.flatten() {
                        let name = file_entry.file_name();
                        let name_str = name.to_string_lossy();
                        if !name_str.starts_with("rollout-") || !name_str.ends_with(".jsonl") {
                            continue;
                        }
                        let path = file_entry.path();
                        let Ok(meta) = fs::metadata(&path) else {
                            continue;
                        };
                        if !meta.is_file() || meta.len() == 0 {
                            continue;
                        }
                        let Some(mtime_ms) = file_mtime_ms(&meta) else {
                            continue;
                        };
                        if mtime_ms < since_ms {
                            continue;
                        }
                        if let Some(session) =
                            codex_session(&path, meta.len(), mtime_ms, now_ms, &names)
                            && !skip.contains(&session.session_id)
                        {
                            found.push(session);
                        }
                    }
                }
            }
        }
    }

    for session in &mut found {
        session.folder_missing = !Path::new(&session.cwd).is_dir();
    }
    found.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    found
}

fn text_blocks(content: &Value) -> Vec<Value> {
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|b| b["type"] == "text" && b["text"].is_string())
        .map(|b| json!({"type": "text", "text": bounded(b["text"].as_str().unwrap_or(""), MAX_ANSWER)}))
        .collect()
}

fn slim_claude(entry: &Value) -> Option<Value> {
    let typ = entry["type"].as_str()?;
    let message = &entry["message"];
    if typ == "user" {
        let content = &message["content"];
        let has_result = content
            .as_array()
            .is_some_and(|arr| arr.iter().any(|b| b["type"] == "tool_result"));
        let slim_content = if let Some(s) = content.as_str() {
            json!(bounded(s, MAX_PROMPT * 2))
        } else if has_result {
            json!([{"type": "tool_result"}])
        } else {
            Value::Array(text_blocks(content))
        };
        return Some(json!({
            "type": "user",
            "timestamp": entry["timestamp"],
            "isSidechain": entry["isSidechain"],
            "entrypoint": "cli",
            "isMeta": entry["isMeta"],
            "promptSource": entry["promptSource"],
            "isCompactSummary": entry["isCompactSummary"],
            "isVisibleInTranscriptOnly": entry["isVisibleInTranscriptOnly"],
            "message": { "content": slim_content }
        }));
    }
    if typ == "assistant" {
        return Some(json!({
            "type": "assistant",
            "timestamp": entry["timestamp"],
            "isSidechain": entry["isSidechain"],
            "message": {
                "model": message["model"],
                "stop_reason": message["stop_reason"],
                "content": text_blocks(&message["content"]),
            }
        }));
    }
    if typ == "system" && entry["subtype"] == "turn_duration" {
        return Some(json!({
            "type": "system",
            "timestamp": entry["timestamp"],
            "isSidechain": entry["isSidechain"],
            "subtype": "turn_duration"
        }));
    }
    None
}

fn slim_codex(entry: &Value) -> Option<Value> {
    let typ = entry["type"].as_str()?;
    let payload = &entry["payload"];
    if typ == "turn_context" {
        return Some(json!({
            "type": "turn_context",
            "timestamp": entry["timestamp"],
            "payload": { "model": payload["model"] }
        }));
    }
    if typ == "event_msg"
        && (payload["type"] == "task_complete" || payload["type"] == "turn_aborted")
    {
        return Some(json!({
            "type": "event_msg",
            "timestamp": entry["timestamp"],
            "payload": { "type": payload["type"] }
        }));
    }
    if typ == "response_item" && payload["type"] == "message" {
        let content: Vec<Value> = payload["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|b| {
                let t = b["text"].as_str()?;
                let btype = b["type"].as_str().unwrap_or("text");
                Some(json!({ "type": btype, "text": bounded(t, MAX_ANSWER) }))
            })
            .collect();
        return Some(json!({
            "type": "response_item",
            "timestamp": entry["timestamp"],
            "payload": {
                "type": "message",
                "role": payload["role"],
                "content": content
            }
        }));
    }
    None
}

pub fn session_exchanges(runtime: ImportRuntime, path: &Path) -> io::Result<Vec<Exchange>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut kept = String::new();
    for line in reader.lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let slim = match runtime {
            ImportRuntime::Claude => slim_claude(&entry),
            ImportRuntime::Codex => slim_codex(&entry),
            _ => None,
        };
        if let Some(s) = slim {
            kept.push_str(&s.to_string());
            kept.push('\n');
        }
    }
    Ok(runtime_terminal::exchanges(runtime.as_str(), &kept, ""))
}

pub fn transcript_path_for(
    runtime: ImportRuntime,
    session_id: &str,
    places: &SessionPlaces,
) -> Option<PathBuf> {
    if !(8..=80).contains(&session_id.len())
        || !session_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return None;
    }
    match runtime {
        ImportRuntime::Claude => {
            let projects = places.claude_home.join("projects");
            let folders = fs::read_dir(&projects).ok()?;
            for folder in folders.flatten() {
                let candidate = folder.path().join(format!("{session_id}.jsonl"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
            None
        }
        ImportRuntime::Codex => {
            let sessions = places.codex_home.join("sessions");
            let newest_first = |dir: &Path| -> Vec<PathBuf> {
                let mut entries: Vec<PathBuf> = fs::read_dir(dir)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path())
                    .collect();
                entries.sort();
                entries.reverse();
                entries
            };
            let suffix = format!("-{session_id}.jsonl");
            for year in newest_first(&sessions) {
                for month in newest_first(&year) {
                    for day in newest_first(&month) {
                        if let Ok(files) = fs::read_dir(&day) {
                            for f in files.flatten() {
                                if f.file_name().to_string_lossy().ends_with(&suffix) {
                                    return Some(f.path());
                                }
                            }
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Cursor Agent (meta.json + store.db)
// ---------------------------------------------------------------------------

fn is_uuid(s: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"^[a-fA-F0-9]{8}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{4}-[a-fA-F0-9]{12}$")
            .unwrap()
    });
    re.is_match(s)
}

fn is_hex32(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(hex.len() / 2);
    for pair in hex.as_bytes().chunks_exact(2) {
        let hi = (pair[0] as char).to_digit(16)? as u8;
        let lo = (pair[1] as char).to_digit(16)? as u8;
        out.push((hi << 4) | lo);
    }
    Some(out)
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0xf) as usize] as char);
    }
    out
}

fn cursor_meta(folder: &Path) -> Result<Value, String> {
    let mut file = File::open(folder.join("meta.json")).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; MAX_CURSOR_META_BYTES];
    let mut read_total = 0;
    while read_total < MAX_CURSOR_META_BYTES {
        let n = file.read(&mut buf[read_total..]).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        read_total += n;
    }
    if read_total == MAX_CURSOR_META_BYTES {
        return Err("Cursor chat metadata is too large.".into());
    }
    let text = String::from_utf8(buf[..read_total].to_vec()).map_err(|e| e.to_string())?;
    let val: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if !val.is_object() || val["schemaVersion"].as_i64() != Some(1) {
        return Err("Unknown Cursor chat format.".into());
    }
    Ok(val)
}

fn run_sqlite_readonly(db_path: &Path, sql: &str) -> Result<String, String> {
    let out = Command::new("sqlite3")
        .arg("-readonly")
        .arg("-noheader")
        .arg("-batch")
        .arg(db_path)
        .arg(sql)
        .output()
        .map_err(|e| format!("sqlite3 could not run: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_owned());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn cursor_store_meta(folder: &Path) -> Result<Value, String> {
    let db_path = folder.join("store.db");
    if !db_path.is_file() {
        return Err("Cursor store.db is missing.".into());
    }
    let out = run_sqlite_readonly(
        &db_path,
        "SELECT value FROM meta WHERE key = '0' AND length(value) < 131072 LIMIT 1;",
    )?;
    let hex_val = out.trim();
    if hex_val.is_empty() {
        return Err("Unknown Cursor session format.".into());
    }
    let raw = decode_hex(hex_val).ok_or_else(|| "Unknown Cursor session format.".to_owned())?;
    let text = String::from_utf8(raw).map_err(|_| "Unknown Cursor session format.".to_owned())?;
    let meta: Value =
        serde_json::from_str(&text).map_err(|_| "Unknown Cursor session format.".to_owned())?;
    if !meta.is_object() {
        return Err("Unknown Cursor session format.".into());
    }
    Ok(meta)
}

/// Field 1 in Cursor's root protobuf is the ordered message blob hashes.
pub fn cursor_message_ids(bytes: &[u8]) -> Result<Vec<String>, String> {
    let mut at = 0usize;
    let integer = |at: &mut usize| -> Result<u64, String> {
        let mut value: u64 = 0;
        let mut shift = 0u32;
        while shift < 49 && *at < bytes.len() {
            let byte = bytes[*at];
            *at += 1;
            value = value
                .checked_add(((byte & 127) as u64) << shift)
                .ok_or_else(|| "Invalid Cursor message index.".to_owned())?;
            if (byte & 128) == 0 {
                return Ok(value);
            }
            shift += 7;
        }
        Err("Invalid Cursor message index.".into())
    };
    let mut ids = Vec::new();
    while at < bytes.len() {
        let tag = integer(&mut at)?;
        let field = tag >> 3;
        if field == 0 {
            return Err("Invalid Cursor message index.".into());
        }
        let wire = tag & 7;
        match wire {
            0 => {
                let _ = integer(&mut at)?;
            }
            1 => {
                at += 8;
            }
            5 => {
                at += 4;
            }
            2 => {
                let length = usize::try_from(integer(&mut at)?)
                    .map_err(|_| "Truncated Cursor message index.".to_owned())?;
                let end = at
                    .checked_add(length)
                    .ok_or_else(|| "Truncated Cursor message index.".to_owned())?;
                if end > bytes.len() {
                    return Err("Truncated Cursor message index.".into());
                }
                if field == 1 {
                    if length != 32 {
                        return Err("Unknown Cursor message reference.".into());
                    }
                    ids.push(encode_hex(&bytes[at..end]));
                }
                at = end;
            }
            _ => return Err("Unknown Cursor message index.".into()),
        }
        if at > bytes.len() {
            return Err("Truncated Cursor message index.".into());
        }
    }
    Ok(ids)
}

fn json_number_ms(val: &Value) -> Option<i64> {
    val.as_i64()
        .or_else(|| val.as_f64().filter(|f| f.is_finite()).map(|f| f as i64))
}

pub fn list_cursor_sessions(home: &Path, now_ms: Option<i64>) -> Vec<ImportableSession> {
    let now_ms = now_ms.unwrap_or_else(now_utc_ms);
    let mut sessions = Vec::new();
    let chats = home.join("chats");
    let Ok(projects) = fs::read_dir(&chats) else {
        return sessions;
    };
    for project_entry in projects.flatten() {
        let project_name = project_entry.file_name();
        if !is_hex32(&project_name.to_string_lossy()) {
            continue;
        }
        let Ok(ids) = fs::read_dir(project_entry.path()) else {
            continue;
        };
        for id_entry in ids.flatten() {
            let id = id_entry.file_name().to_string_lossy().into_owned();
            if !is_uuid(&id) {
                continue;
            }
            let folder = id_entry.path();
            let Ok(info) = cursor_meta(&folder) else {
                continue;
            };
            if info["hasConversation"] != true {
                continue;
            }
            let Some(cwd) = info["cwd"].as_str().map(str::to_owned) else {
                continue;
            };
            let Some(updated_ms) = json_number_ms(&info["updatedAtMs"]) else {
                continue;
            };
            let Some(updated_at) = iso_from_ms(updated_ms) else {
                continue;
            };
            let Ok(meta) = cursor_store_meta(&folder) else {
                continue;
            };
            if meta["agentId"].as_str() != Some(id.as_str()) || !meta["latestRootBlobId"].is_string()
            {
                continue;
            }
            let bytes = fs::metadata(folder.join("store.db"))
                .map(|m| m.len())
                .unwrap_or(0);
            let title = one_line(
                meta["name"]
                    .as_str()
                    .unwrap_or("A Cursor Agent session"),
            );
            let folder_name = folders::folder_name(&cwd);
            sessions.push(ImportableSession {
                runtime: ImportRuntime::Cursor,
                session_id: id,
                title,
                cwd,
                folder_name,
                updated_at,
                bytes,
                open_now: now_ms - updated_ms < OPEN_NOW_MS,
                folder_missing: false,
            });
        }
    }
    sessions
}

fn fetch_cursor_blobs(db_path: &Path, ids: &[String]) -> Result<HashMap<String, Vec<u8>>, String> {
    let mut map = HashMap::new();
    for chunk in ids.chunks(100) {
        let mut unique: Vec<&str> = Vec::new();
        for id in chunk {
            if !is_hex64(id) {
                return Err("A Cursor message is unavailable or too large to import.".into());
            }
            if !unique.contains(&id.as_str()) {
                unique.push(id.as_str());
            }
        }
        let list = unique
            .iter()
            .map(|id| format!("'{id}'"))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id || '|' || hex(data) FROM blobs WHERE length(data) <= {MAX_CURSOR_BLOB} AND id IN ({list});"
        );
        let out = run_sqlite_readonly(db_path, &sql)?;
        for line in out.lines() {
            let Some((id, hex_data)) = line.split_once('|') else {
                continue;
            };
            let bytes = decode_hex(hex_data.trim())
                .ok_or_else(|| "A Cursor message is unavailable or too large to import.".to_owned())?;
            map.insert(id.to_owned(), bytes);
        }
    }
    Ok(map)
}

fn extract_words(val: &Value) -> String {
    if let Some(s) = val.as_str() {
        return s.to_owned();
    }
    val.as_array()
        .into_iter()
        .flatten()
        .filter(|p| p["type"] == "text" && p["text"].is_string())
        .filter_map(|p| p["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn user_query_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?s)<user_query>\s*(.*?)\s*</user_query>").unwrap())
}

pub fn cursor_exchanges(home: &Path, session_id: &str, cwd: &str) -> Result<Vec<Exchange>, String> {
    if !is_uuid(session_id) {
        return Err("Invalid Cursor chat ID.".into());
    }
    let chats = home.join("chats");
    let projects = fs::read_dir(&chats).map_err(|_| {
        "The Cursor chat could not be found. Nothing was imported.".to_owned()
    })?;
    for project_entry in projects.flatten() {
        if !is_hex32(&project_entry.file_name().to_string_lossy()) {
            continue;
        }
        let folder = project_entry.path().join(session_id);
        let Ok(info) = cursor_meta(&folder) else {
            continue;
        };
        if info["cwd"].as_str() != Some(cwd) {
            continue;
        }
        let meta = cursor_store_meta(&folder)?;
        let Some(root_blob_id) = meta["latestRootBlobId"].as_str() else {
            return Err("Cursor chat identity changed.".into());
        };
        if meta["agentId"].as_str() != Some(session_id) {
            return Err("Cursor chat identity changed.".into());
        }
        let db_path = folder.join("store.db");
        let root_map = fetch_cursor_blobs(&db_path, &[root_blob_id.to_owned()])?;
        let root_bytes = root_map
            .get(root_blob_id)
            .ok_or_else(|| "A Cursor message is unavailable or too large to import.".to_owned())?;
        let message_ids = cursor_message_ids(root_bytes)?;
        let blobs = fetch_cursor_blobs(&db_path, &message_ids)?;

        let started_at = json_number_ms(&info["createdAtMs"])
            .and_then(iso_from_ms)
            .or_else(|| json_number_ms(&meta["createdAt"]).and_then(iso_from_ms))
            .ok_or_else(|| "Cursor chat dates are unavailable.".to_owned())?;
        let finished_at = json_number_ms(&info["updatedAtMs"])
            .and_then(iso_from_ms)
            .ok_or_else(|| "Cursor chat dates are unavailable.".to_owned())?;

        let mut exchanges: Vec<Exchange> = Vec::new();
        let mut prompt: Option<String> = None;
        let mut answer = String::new();
        let mut finished = false;
        let close = |prompt: &mut Option<String>,
                     answer: &mut String,
                     finished: &mut bool,
                     exchanges: &mut Vec<Exchange>| {
            if let Some(p) = prompt.take()
                && *finished
                && !answer.trim().is_empty()
            {
                let mut ex = Exchange::default();
                ex.prompt = p;
                ex.answer = Some(answer.clone());
                ex.started = started_at.clone();
                ex.finished = finished_at.clone();
                exchanges.push(ex);
            }
            answer.clear();
            *finished = false;
        };

        for id in &message_ids {
            let bytes = blobs
                .get(id)
                .ok_or_else(|| "A Cursor message is unavailable or too large to import.".to_owned())?;
            let message: Value = serde_json::from_slice(bytes)
                .map_err(|_| "Unknown Cursor message format.".to_owned())?;
            if !message.is_object() {
                return Err("Unknown Cursor message format.".into());
            }
            let role = message["role"].as_str().unwrap_or("");
            if role == "user" {
                let content = extract_words(&message["content"]).trim().to_owned();
                let text = user_query_re()
                    .captures(&content)
                    .and_then(|c| c.get(1).map(|m| m.as_str().to_owned()))
                    .unwrap_or(content);
                if text.is_empty() || text.starts_with('<') {
                    continue;
                }
                close(&mut prompt, &mut answer, &mut finished, &mut exchanges);
                prompt = Some(text.chars().take(MAX_PROMPT).collect());
            } else if role == "assistant" && prompt.is_some() {
                let combined = format!("{answer}\n{}", extract_words(&message["content"]));
                answer = combined.trim().chars().take(MAX_ANSWER).collect();
                finished = !message["content"]
                    .as_array()
                    .is_some_and(|arr| arr.iter().any(|p| p["type"] == "tool-call"));
            }
        }
        close(&mut prompt, &mut answer, &mut finished, &mut exchanges);
        return Ok(exchanges);
    }
    Err("The Cursor chat could not be found. Nothing was imported.".into())
}

// ---------------------------------------------------------------------------
// OpenCode (CLI `session list --format json` & `export <id>`)
// ---------------------------------------------------------------------------

fn is_opencode_id(id: &str) -> bool {
    id.strip_prefix("ses_")
        .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_alphanumeric()))
}

pub fn open_code_sessions(text: &str, now_ms: Option<i64>) -> Result<Vec<ImportableSession>, String> {
    let now_ms = now_ms.unwrap_or_else(now_utc_ms);
    let rows: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let arr = rows
        .as_array()
        .ok_or_else(|| "OpenCode did not return a session list.".to_owned())?;
    let mut out = Vec::new();
    for row in arr {
        if !row.is_object() {
            continue;
        }
        let Some(id) = row["id"].as_str().filter(|id| is_opencode_id(id)) else {
            continue;
        };
        let Some(title) = row["title"].as_str() else {
            continue;
        };
        let Some(cwd) = row["directory"].as_str() else {
            continue;
        };
        let Some(updated_ms) = json_number_ms(&row["updated"]) else {
            continue;
        };
        let Some(updated_at) = iso_from_ms(updated_ms) else {
            continue;
        };
        if !row["parentID"].is_null() {
            continue;
        }
        out.push(ImportableSession {
            runtime: ImportRuntime::Opencode,
            session_id: id.to_owned(),
            title: one_line(title),
            cwd: cwd.to_owned(),
            folder_name: folders::folder_name(cwd),
            updated_at,
            bytes: 0,
            open_now: now_ms - updated_ms < OPEN_NOW_MS,
            folder_missing: false,
        });
    }
    Ok(out)
}

pub fn open_code_exchanges(text: &str, session_id: &str) -> Result<Vec<Exchange>, String> {
    let exported: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if !exported.is_object()
        || !exported["info"].is_object()
        || exported["info"]["id"].as_str() != Some(session_id)
        || !exported["messages"].is_array()
    {
        return Err("OpenCode returned a different or unsupported session.".into());
    }
    struct PromptAcc {
        id: String,
        text: String,
        started_at: String,
        answer: String,
        finished_at: Option<String>,
        model: Option<String>,
    }
    let mut order: Vec<PromptAcc> = Vec::new();
    let mut index_by_id: HashMap<String, usize> = HashMap::new();

    for message in exported["messages"].as_array().unwrap() {
        let Some(info) = message["info"].as_object() else {
            continue;
        };
        let Some(parts) = message["parts"].as_array() else {
            continue;
        };
        if info.get("sessionID").and_then(|v| v.as_str()) != Some(session_id) {
            continue;
        }
        let Some(time) = info.get("time").and_then(|v| v.as_object()) else {
            continue;
        };
        let filtered_parts: Vec<Value> = parts
            .iter()
            .filter(|p| p.is_object() && p["synthetic"] != true && p["ignored"] != true)
            .cloned()
            .collect();
        let words = extract_words(&Value::Array(filtered_parts)).trim().to_owned();
        let role = info.get("role").and_then(|v| v.as_str()).unwrap_or("");
        if role == "user" {
            let Some(msg_id) = info.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(started_at) = time
                .get("created")
                .and_then(json_number_ms)
                .and_then(iso_from_ms)
            else {
                continue;
            };
            if !words.is_empty() {
                let idx = order.len();
                order.push(PromptAcc {
                    id: msg_id.to_owned(),
                    text: words.chars().take(MAX_PROMPT).collect(),
                    started_at,
                    answer: String::new(),
                    finished_at: None,
                    model: None,
                });
                index_by_id.insert(msg_id.to_owned(), idx);
            }
        } else if role == "assistant" {
            let Some(parent_id) = info.get("parentID").and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(&idx) = index_by_id.get(parent_id) else {
                continue;
            };
            let acc = &mut order[idx];
            let combined = format!("{}\n{words}", acc.answer);
            acc.answer = combined.trim().chars().take(MAX_ANSWER).collect();
            let is_stop = info.get("finish").and_then(|v| v.as_str()) == Some("stop");
            let has_error = info.get("error").is_some_and(|v| !v.is_null());
            if is_stop && !has_error {
                acc.finished_at = time
                    .get("completed")
                    .and_then(json_number_ms)
                    .and_then(iso_from_ms);
            }
            if let (Some(model_id), Some(provider_id)) = (
                info.get("modelID").and_then(|v| v.as_str()),
                info.get("providerID").and_then(|v| v.as_str()),
            ) {
                acc.model = Some(format!("{provider_id}/{model_id}"));
            }
        }
    }

    Ok(order
        .into_iter()
        .filter_map(|p| {
            let finished = p.finished_at?;
            let _ = p.id;
            let mut ex = Exchange::default();
            ex.prompt = p.text;
            ex.answer = if p.answer.is_empty() {
                None
            } else {
                Some(p.answer)
            };
            ex.started = p.started_at;
            ex.finished = finished;
            ex.model = p.model;
            Some(ex)
        })
        .collect())
}

pub trait OpenCodeRunner {
    fn run_opencode(&self, args: &[&str]) -> Result<Option<String>, String>;
}

pub struct SystemOpenCodeRunner;

impl OpenCodeRunner for SystemOpenCodeRunner {
    fn run_opencode(&self, args: &[&str]) -> Result<Option<String>, String> {
        let Ok(exe) = crate::runtime::discovery::find_named("opencode") else {
            return Ok(None);
        };
        let out = Command::new(exe)
            .args(args)
            .output()
            .map_err(|_| "OpenCode could not read its sessions. Nothing was imported.".to_owned())?;
        if !out.status.success() {
            return Err("OpenCode could not read its sessions. Nothing was imported.".into());
        }
        Ok(Some(String::from_utf8_lossy(&out.stdout).into_owned()))
    }
}

pub struct OpenCodeSessionReader<R: OpenCodeRunner> {
    runner: R,
}

impl<R: OpenCodeRunner> OpenCodeSessionReader<R> {
    pub fn new(runner: R) -> Self {
        Self { runner }
    }

    pub fn list(&self, now_ms: Option<i64>) -> Result<Vec<ImportableSession>, String> {
        let Some(output) = self
            .runner
            .run_opencode(&["session", "list", "--format", "json"])?
        else {
            return Ok(Vec::new());
        };
        open_code_sessions(&output, now_ms)
    }

    pub fn exchanges(&self, session_id: &str) -> Result<Vec<Exchange>, String> {
        if !is_opencode_id(session_id) {
            return Err("Invalid OpenCode session ID.".into());
        }
        let Some(output) = self.runner.run_opencode(&["export", session_id])? else {
            return Err("OpenCode is no longer installed. Nothing was imported.".into());
        };
        open_code_exchanges(&output, session_id)
    }
}

// ---------------------------------------------------------------------------
// Held sessions & importing a session into native's ledger
// ---------------------------------------------------------------------------

pub fn held_session_ids(store: &Store) -> HashSet<String> {
    let mut held = HashSet::new();
    if let Ok(imports) = store.seeded_json("terminal-imports.json", json!({}))
        && let Some(obj) = imports.as_object()
    {
        for key in obj.keys() {
            held.insert(key.clone());
        }
    }
    if let Ok(dir) = store.ledger_dir() {
        collect_thread_ids(&dir, &mut held);
    }
    if let Some(electron_dir) = crate::native::electron_ledger_dir() {
        collect_thread_ids(&electron_dir, &mut held);
    }
    held
}

fn collect_thread_ids(dir: &Path, into: &mut HashSet<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(file) = File::open(&path) else {
            continue;
        };
        for line in BufReader::new(file).lines().map_while(Result::ok) {
            let Ok(rec) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if rec["recordType"] == "mission.event" {
                if let Some(t) = rec["event"]["runtimeThreadId"].as_str()
                    && !t.is_empty()
                {
                    into.insert(t.to_owned());
                }
                if let Some(t) = rec["event"]["payload"]["runtimeThreadId"].as_str()
                    && !t.is_empty()
                {
                    into.insert(t.to_owned());
                }
            }
        }
    }
}

pub fn import_session(
    store: &Store,
    places: &SessionPlaces,
    session: &ImportableSession,
    read_opencode: Option<&dyn Fn(&str) -> Result<Vec<Exchange>, String>>,
) -> Result<ImportSuccess, String> {
    import_session_with_id(store, places, session, read_opencode, &mut || {
        crate::native::uuid_v4().map_err(|e| e.to_string())
    })
}

pub fn import_session_with_id(
    store: &Store,
    places: &SessionPlaces,
    session: &ImportableSession,
    read_opencode: Option<&dyn Fn(&str) -> Result<Vec<Exchange>, String>>,
    create_id: &mut dyn FnMut() -> Result<String, String>,
) -> Result<ImportSuccess, String> {
    if session.folder_missing {
        return Err(
            "Its folder is no longer available. Nothing was imported; restore the folder and open the list again."
                .into(),
        );
    }
    let mut imports = store.seeded_json("terminal-imports.json", json!({}))?;
    if !imports.is_object() {
        return Err("Terminal import history could not be read. Nothing was imported.".into());
    }
    if imports.get(&session.session_id).is_some_and(|v| !v.is_null()) {
        return Err("That conversation is already in Locust.".into());
    }

    let all = match session.runtime {
        ImportRuntime::Cursor | ImportRuntime::Opencode => {
            if !Path::new(&session.cwd).is_dir() {
                return Err(
                    "Its folder is no longer available. Nothing was imported; restore the folder and open the list again."
                        .into(),
                );
            }
            if session.runtime == ImportRuntime::Cursor {
                let cursor_home = places
                    .cursor_home
                    .as_deref()
                    .ok_or_else(|| "Cursor folder is unavailable.".to_owned())?;
                cursor_exchanges(cursor_home, &session.session_id, &session.cwd)?
            } else if let Some(reader) = read_opencode {
                reader(&session.session_id)?
            } else {
                OpenCodeSessionReader::new(SystemOpenCodeRunner).exchanges(&session.session_id)?
            }
        }
        ImportRuntime::Claude | ImportRuntime::Codex => {
            let path = transcript_path_for(session.runtime, &session.session_id, places)
                .ok_or_else(|| {
                    "Its file could not be found. It may have been deleted or moved.".to_owned()
                })?;
            session_exchanges(session.runtime, &path)
                .map_err(|_| "Its file could not be read.".to_owned())?
        }
    };

    if all.is_empty() {
        return Err("It has no finished exchange to bring in yet.".into());
    }
    let total = all.len();
    let keep_from = total.saturating_sub(MAX_IMPORTED_EXCHANGES);
    let skipped = keep_from;
    let exchanges: Vec<Exchange> = all.into_iter().skip(keep_from).collect();

    let cwd = folders::same_as(store, Path::new(&session.cwd))
        .unwrap_or_else(|_| session.cwd.clone());
    let workspace_id = people::workspace_id(Path::new(&cwd));
    let _ = folders::learn(store, &[(workspace_id.clone(), cwd)]);

    let runtime = session.runtime.as_str();
    let resolved_route_id = format!("{runtime}-account:default");
    let mut previous: Option<String> = None;
    let mut first: Option<String> = None;
    let mut last_finished = String::new();
    let count = exchanges.len();

    for (idx, exchange) in exchanges.into_iter().enumerate() {
        let mission_id = format!("mission_{}", create_id()?);
        let run_id = format!("run_{}", create_id()?);
        let mut metadata = json!({
            "missionId": mission_id,
            "runId": run_id,
            "prompt": bounded(&exchange.prompt, MAX_PROMPT),
            "runtime": runtime,
            "model": "account-default",
            "requestedRouteId": runtime,
            "resolvedRouteId": resolved_route_id,
            "cliVersion": null,
            "workspaceId": workspace_id,
            "sandbox": "workspace-write",
            "executionPolicyVersion": 1,
            "createdAt": exchange.started,
            "startedBy": { "kind": "terminal", "exchange": idx + 1 }
        });
        if let Some(prev) = &previous {
            metadata["continuesFrom"] = json!({
                "missionId": prev,
                "checkpointEpoch": 1,
                "reason": "follow-up",
                "runtimeThreadId": session.session_id
            });
        }
        let mut records = vec![json!({
            "schemaVersion": 27,
            "recordType": "mission.created",
            "ledgerSequence": 1,
            "occurredAt": exchange.started,
            "metadata": metadata
        })];

        let evidence = json!({ "redacted": true, "runtimeEventType": "import.transcript" });
        let mut events = vec![(
            "run.started",
            exchange.started.clone(),
            json!({ "runtimeThreadId": session.session_id, "evidence": evidence }),
        )];
        if let Some(answer) = exchange.answer {
            events.push((
                "message.delta",
                exchange.finished.clone(),
                json!({
                    "itemId": "imported_answer",
                    "operation": "replace",
                    "text": bounded(&answer, MAX_ANSWER),
                    "final": true,
                    "evidence": evidence
                }),
            ));
        }
        let mut completed_payload = json!({
            "runtimeThreadId": session.session_id,
            "process": {
                "exitCode": null,
                "signal": null,
                "stderr": "",
                "stderrTruncated": false,
                "recordCount": exchange.lines,
                "inputDeliveryFailed": false,
                "outputLimitExceeded": false,
                "oversizedRecordsDropped": 0,
                "forcedTerminationAttempted": false,
                "terminationUnconfirmed": false,
                "startedAt": exchange.started,
                "finishedAt": exchange.finished
            }
        });
        if let Some(model) = exchange.model {
            completed_payload["resolvedModel"] = json!(model);
        }
        events.push(("run.completed", exchange.finished.clone(), completed_payload));

        for (i, (kind, at, payload)) in events.into_iter().enumerate() {
            records.push(json!({
                "schemaVersion": 27,
                "recordType": "mission.event",
                "ledgerSequence": i + 2,
                "occurredAt": at,
                "event": {
                    "id": format!("{run_id}:{}", i + 1),
                    "runId": run_id,
                    "missionId": mission_id,
                    "sequence": i + 1,
                    "occurredAt": at,
                    "sourceAdapter": runtime,
                    "runtimeThreadId": session.session_id,
                    "type": kind,
                    "payload": payload
                }
            }));
        }

        let dir = store.ledger_dir().map_err(|e| e.to_string())?;
        let pending = format!(".{mission_id}.tmp");
        let mut f = store
            .create_new_file_in(&dir, &pending)
            .map_err(|e| e.to_string())?;
        for record in records {
            writeln!(f, "{record}").map_err(|e| e.to_string())?;
        }
        f.sync_all().map_err(|e| e.to_string())?;
        fs::rename(
            dir.join(&pending),
            store.write_path(&mission_id).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;

        last_finished = exchange.finished;
        if first.is_none() {
            first = Some(mission_id.clone());
        }
        previous = Some(mission_id);
    }

    imports[&session.session_id] = json!({ "through": last_finished });
    store.save_json("terminal-imports.json", &imports)?;

    Ok(ImportSuccess {
        mission_id: previous.unwrap(),
        first_mission_id: first.unwrap(),
        title: session.title.clone(),
        turns: count,
        skipped,
    })
}

/// Combined listing across Claude Code, Codex, OpenCode, and Cursor Agent for the window.
pub fn list_all_for_window(
    store: &Store,
    places: &SessionPlaces,
    opencode_runner: &dyn OpenCodeRunner,
    now_ms: Option<i64>,
) -> (Vec<ImportableSession>, Vec<String>) {
    let mut warnings = Vec::new();
    let mut other_sessions = Vec::new();

    match OpenCodeSessionReader::new(RunnerRef(opencode_runner)).list(now_ms) {
        Ok(list) => other_sessions.extend(list),
        Err(_) => warnings.push(
            "OpenCode sessions could not be listed. Your other sessions are still shown.".into(),
        ),
    }

    if let Some(cursor_home) = &places.cursor_home {
        other_sessions.extend(list_cursor_sessions(cursor_home, now_ms));
    }

    let skip = held_session_ids(store);
    let sessions = list_importable_sessions(
        places,
        ListOptions {
            now_ms,
            days: None,
            skip,
            other_sessions,
        },
    );
    (sessions, warnings)
}

struct RunnerRef<'a>(&'a dyn OpenCodeRunner);

impl OpenCodeRunner for RunnerRef<'_> {
    fn run_opencode(&self, args: &[&str]) -> Result<Option<String>, String> {
        self.0.run_opencode(args)
    }
}

#[cfg(test)]
#[path = "session_import_tests.rs"]
mod tests;

//! The workroom: the channel teammates post to each other on (Electron `packages/mission-store/src/workroom.ts`).
//!
//! Its own append-only file, never interleaved into a mission ledger: a message between two teammates has two
//! missions with a stake in it. Every message is attributed and routed to ONE named teammate; there is no broadcast,
//! and a teammate's message can inform, never instruct. Same discipline as the ledger: a fixed schema, a contiguous
//! sequence, an fsync per append, a file changed between appends noticed, strict checking on read, and a torn tail
//! repaired rather than fatal.

use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const WORKROOM_SCHEMA_VERSION: u64 = 1;
/// The share cap (peer-share's MAX_SHARE_TEXT_LENGTH), in UTF-16 units as Electron counts.
pub const MAX_WORKROOM_MESSAGE_LENGTH: usize = 6_000;
pub const MAX_WORKROOM_NAME_LENGTH: usize = 40;
pub const MAX_OBSERVED_PATHS: usize = 40;
const MAX_WORKROOM_BYTES: u64 = 16 * 1024 * 1024;
const MAX_RECORD_BYTES: usize = 64 * 1024;
pub const FILE_NAME: &str = "workroom.jsonl";
pub const FOLDER: &str = "workroom";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Party {
    pub teammate_id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub message_id: String,
    pub sequence: u64,
    pub from: Party,
    /// The mission whose work produced the message.
    pub from_mission: String,
    pub to: Party,
    pub text: String,
    pub posted_at: String,
    /// What the host saw change during the run that sent it; None when it did not look.
    pub observed: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Delivery {
    pub message_id: String,
    pub mission_id: String,
    pub delivered_at: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueCode {
    ReadFailed,
    FileTooLarge,
    TruncatedTail,
    InvalidRecord,
    UnsupportedSchema,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub code: IssueCode,
    pub message: &'static str,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub messages: Vec<Message>,
    pub deliveries: Vec<Delivery>,
    pub issues: Vec<Issue>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unread {
    /// Oldest first, after any message asked for by id.
    pub messages: Vec<Message>,
    pub remaining: usize,
}

struct Parsed {
    snapshot: Snapshot,
    next_sequence: u64,
    byte_length: u64,
    valid_byte_length: u64,
}

fn is_safe_id(v: &str) -> bool {
    (1..=128).contains(&v.len()) && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn is_timestamp(v: &str) -> bool {
    v.len() <= 128 && !v.contains('\0') && chrono::DateTime::parse_from_rfc3339(v).is_ok()
}

fn units(v: &str) -> usize {
    v.encode_utf16().count()
}

pub fn is_workroom_name(v: &str) -> bool {
    !v.trim().is_empty() && units(v) <= MAX_WORKROOM_NAME_LENGTH && !v.chars().any(|c| (c as u32) < 0x20 || c as u32 == 0x7f)
}

pub fn is_workroom_text(v: &str) -> bool {
    !v.trim().is_empty()
        && units(v) <= MAX_WORKROOM_MESSAGE_LENGTH
        && !v.chars().any(|c| matches!(c as u32, 0x00..=0x08 | 0x0b | 0x0c | 0x0e..=0x1f | 0x7f))
}

/// A path as the host reports it: short, printable, one line.
pub fn is_observed_path(v: &str) -> bool {
    !v.is_empty() && units(v) <= 300 && !v.chars().any(|c| (c as u32) < 0x20 || c as u32 == 0x7f)
}

fn parsed_observed(v: &Value) -> Option<Vec<String>> {
    let list = v.as_array()?;
    if list.len() > MAX_OBSERVED_PATHS + 1 {
        return None;
    }
    let more = crate::invariants::regex(r"^\+\d+ more$");
    list.iter().map(|e| e.as_str().filter(|s| is_observed_path(s) || more.is_match(s)).map(str::to_owned)).collect()
}

fn parsed_party(v: &Value) -> Option<Party> {
    let id = v["teammateId"].as_str().filter(|s| is_safe_id(s))?;
    let name = v["name"].as_str().filter(|s| is_workroom_name(s))?;
    Some(Party { teammate_id: id.into(), name: name.into() })
}

fn parsed_message(v: &Value, sequence: u64) -> Option<Message> {
    if !v.is_object() {
        return None;
    }
    let from = parsed_party(&v["from"])?;
    let to = parsed_party(&v["to"])?;
    let from_mission = v["from"]["missionId"].as_str().filter(|s| is_safe_id(s))?;
    let message_id = v["messageId"].as_str().filter(|s| is_safe_id(s))?;
    let text = v["text"].as_str().filter(|s| is_workroom_text(s))?;
    let posted_at = v["postedAt"].as_str().filter(|s| is_timestamp(s))?;
    if from.teammate_id == to.teammate_id {
        return None;
    }
    Some(Message {
        message_id: message_id.into(),
        sequence,
        from,
        from_mission: from_mission.into(),
        to,
        text: text.into(),
        posted_at: posted_at.into(),
        observed: parsed_observed(&v["observed"]),
    })
}

fn message_json(m: &Message) -> Value {
    let mut v = json!({
        "messageId": m.message_id,
        "from": {"teammateId": m.from.teammate_id, "name": m.from.name, "missionId": m.from_mission},
        "to": {"teammateId": m.to.teammate_id, "name": m.to.name},
        "text": m.text,
        "postedAt": m.posted_at,
    });
    if let Some(observed) = &m.observed {
        v["observed"] = json!(observed);
    }
    v
}

fn issue(code: IssueCode, message: &'static str) -> Issue {
    Issue { code, message }
}

fn read_file(path: &Path) -> Parsed {
    let empty = |issues: Vec<Issue>| Parsed { snapshot: Snapshot { issues, ..Default::default() }, next_sequence: 1, byte_length: 0, valid_byte_length: 0 };
    let meta = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return empty(vec![]),
        Err(_) => return empty(vec![issue(IssueCode::ReadFailed, "The workroom could not be read.")]),
    };
    if !meta.is_file() {
        return empty(vec![issue(IssueCode::ReadFailed, "The workroom was not a regular file.")]);
    }
    if meta.len() > MAX_WORKROOM_BYTES {
        return empty(vec![issue(IssueCode::FileTooLarge, "The workroom exceeded the local safety limit.")]);
    }
    let Ok(text) = std::fs::read_to_string(path) else { return empty(vec![issue(IssueCode::ReadFailed, "The workroom could not be read.")]) };
    let mut issues = Vec::new();
    let mut lines: Vec<&str> = text.split('\n').collect();
    if lines.last() == Some(&"") {
        lines.pop();
    }
    if !text.ends_with('\n') && !lines.is_empty() {
        lines.pop();
        issues.push(issue(IssueCode::TruncatedTail, "An incomplete final workroom record was ignored."));
    }
    let (mut messages, mut deliveries) = (Vec::new(), Vec::new());
    let (mut seen, mut delivered) = (HashSet::new(), HashSet::new());
    let mut expected = 1u64;
    let mut valid = 0u64;
    for line in lines {
        let bad = |why: &'static str| issue(IssueCode::InvalidRecord, why);
        if line.len() > MAX_RECORD_BYTES {
            issues.push(bad("An oversized workroom record and its tail were ignored."));
            break;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            issues.push(bad("An invalid workroom record and its tail were ignored."));
            break;
        };
        if !value.is_object() {
            issues.push(bad("An invalid workroom record and its tail were ignored."));
            break;
        }
        if value["schemaVersion"] != json!(WORKROOM_SCHEMA_VERSION) {
            issues.push(issue(IssueCode::UnsupportedSchema, "The workroom uses an unsupported schema."));
            break;
        }
        let occurred = value["occurredAt"].as_str().filter(|s| is_timestamp(s));
        if value["sequence"] != json!(expected) || occurred.is_none() {
            issues.push(bad("A noncontiguous workroom record and its tail were ignored."));
            break;
        }
        let occurred = occurred.unwrap_or_default();
        match value["recordType"].as_str() {
            Some("workroom.message") => match parsed_message(&value["message"], expected) {
                Some(m) if m.posted_at == occurred && !seen.contains(&m.message_id) => {
                    seen.insert(m.message_id.clone());
                    messages.push(m);
                }
                _ => {
                    issues.push(bad("An invalid workroom message and its tail were ignored."));
                    break;
                }
            },
            Some("workroom.delivery") => {
                let d = &value["delivery"];
                let parsed = (|| {
                    let message_id = d["messageId"].as_str().filter(|s| is_safe_id(s))?;
                    let mission_id = d["missionId"].as_str().filter(|s| is_safe_id(s))?;
                    let at = d["deliveredAt"].as_str().filter(|s| is_timestamp(s))?;
                    Some(Delivery { message_id: message_id.into(), mission_id: mission_id.into(), delivered_at: at.into() })
                })();
                match parsed {
                    Some(d) if d.delivered_at == occurred && seen.contains(&d.message_id) && !delivered.contains(&d.message_id) => {
                        delivered.insert(d.message_id.clone());
                        deliveries.push(d);
                    }
                    _ => {
                        issues.push(bad("An invalid workroom delivery and its tail were ignored."));
                        break;
                    }
                }
            }
            _ => {
                issues.push(bad("An unknown workroom record and its tail were ignored."));
                break;
            }
        }
        expected += 1;
        valid += line.len() as u64 + 1;
    }
    Parsed { snapshot: Snapshot { messages, deliveries, issues }, next_sequence: expected, byte_length: text.len() as u64, valid_byte_length: valid }
}

pub struct Workroom {
    root: PathBuf,
    now: Box<dyn Fn() -> String + Send + Sync>,
    create_id: Box<dyn Fn() -> String + Send + Sync>,
    /// One write at a time (Electron's serialize).
    serial: Mutex<()>,
}

impl Workroom {
    pub fn new(root: &Path) -> Self {
        Self::with(
            root,
            Box::new(|| chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
            Box::new(|| crate::native::uuid_v4().map(|u| u.replace('-', "")).unwrap_or_else(|_| format!("{:x}", crate::invariants::unique_nanos()))),
        )
    }

    pub fn with(root: &Path, now: Box<dyn Fn() -> String + Send + Sync>, create_id: Box<dyn Fn() -> String + Send + Sync>) -> Self {
        Self { root: root.to_path_buf(), now, create_id, serial: Mutex::new(()) }
    }

    /// The workroom kept in a native data folder.
    pub fn for_store(store: &crate::store::Store) -> Self {
        Self::new(&store.root().join(FOLDER))
    }

    fn path(&self) -> PathBuf {
        self.root.join(FILE_NAME)
    }

    /// The file as it is, a torn tail cut back to the last complete record.
    fn read_for_writing(&self) -> Result<Parsed, String> {
        let parsed = read_file(&self.path());
        if parsed.snapshot.issues.is_empty() || !parsed.snapshot.issues.iter().all(|i| i.code == IssueCode::TruncatedTail) {
            return Ok(parsed);
        }
        let file = std::fs::OpenOptions::new().write(true).open(self.path()).map_err(|e| e.to_string())?;
        file.set_len(parsed.valid_byte_length).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        Ok(read_file(&self.path()))
    }

    fn append(&self, build: impl FnOnce(&Parsed) -> Result<Vec<Value>, String>) -> Result<Parsed, String> {
        std::fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.root, std::fs::Permissions::from_mode(0o700));
        }
        let parsed = self.read_for_writing()?;
        if !parsed.snapshot.issues.is_empty() {
            return Err("Workroom is unavailable".into());
        }
        let records = build(&parsed)?;
        if records.is_empty() {
            return Ok(parsed);
        }
        let mut lines = String::new();
        for record in &records {
            let line = format!("{record}\n");
            if line.len() > MAX_RECORD_BYTES {
                return Err("Workroom record exceeds its safety limit".into());
            }
            lines.push_str(&line);
        }
        if parsed.byte_length + lines.len() as u64 > MAX_WORKROOM_BYTES {
            return Err("Workroom exceeds its safety limit".into());
        }
        let mut options = std::fs::OpenOptions::new();
        options.append(true).create(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options.open(self.path()).map_err(|e| e.to_string())?;
        let meta = file.metadata().map_err(|e| e.to_string())?;
        if !meta.is_file() || meta.len() != parsed.byte_length {
            return Err("Workroom changed outside the active writer".into());
        }
        file.write_all(lines.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        Ok(parsed)
    }

    /// Post one message from a teammate's run to another teammate. The same words from the same run to the same
    /// teammate are posted once (A2.2): a retry or a block written twice must not deliver twice.
    pub fn post(&self, from: &Party, from_mission: &str, to: &Party, text: &str, observed: Option<&[String]>) -> Result<Message, String> {
        let _one = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        if !is_safe_id(&from.teammate_id) || !is_workroom_name(&from.name) || !is_safe_id(from_mission) {
            return Err("Workroom sender is invalid".into());
        }
        if !is_safe_id(&to.teammate_id) || !is_workroom_name(&to.name) {
            return Err("Workroom recipient is invalid".into());
        }
        if from.teammate_id == to.teammate_id {
            return Err("A teammate cannot message itself".into());
        }
        if !is_workroom_text(text) {
            return Err("Workroom message text is invalid".into());
        }
        let posted_at = (self.now)();
        let observed = observed.map(|paths| {
            let valid: Vec<String> = paths.iter().filter(|p| is_observed_path(p)).cloned().collect();
            let mut kept: Vec<String> = valid.iter().take(MAX_OBSERVED_PATHS).cloned().collect();
            if valid.len() > MAX_OBSERVED_PATHS {
                kept.push(format!("+{} more", valid.len() - MAX_OBSERVED_PATHS));
            }
            kept
        });
        let mut message = Message {
            message_id: format!("wm_{}", (self.create_id)()),
            sequence: 0,
            from: from.clone(),
            from_mission: from_mission.into(),
            to: to.clone(),
            text: text.into(),
            posted_at: posted_at.clone(),
            observed,
        };
        let mut already = None;
        self.append(|parsed| {
            already = parsed.snapshot.messages.iter().find(|m| m.from_mission == message.from_mission && m.to.teammate_id == message.to.teammate_id && m.text == message.text).cloned();
            if already.is_some() {
                return Ok(vec![]);
            }
            message.sequence = parsed.next_sequence;
            let body = message_json(&message);
            if parsed_message(&body, message.sequence).is_none() {
                return Err("Workroom message is not readable by the workroom reader".into());
            }
            Ok(vec![json!({"schemaVersion": WORKROOM_SCHEMA_VERSION, "recordType": "workroom.message", "sequence": message.sequence, "occurredAt": posted_at, "message": body})])
        })?;
        Ok(already.unwrap_or(message))
    }

    /// What waits for a teammate, oldest first, at most `limit` (after any asked for by id), and how many more wait.
    pub fn unread(&self, teammate_id: &str, limit: usize, include: &[String]) -> Result<Unread, String> {
        let _one = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        if !is_safe_id(teammate_id) {
            return Err("Teammate id is invalid".into());
        }
        let parsed = self.read_for_writing()?;
        if !parsed.snapshot.issues.is_empty() {
            return Err("Workroom is unavailable".into());
        }
        let delivered: HashSet<&str> = parsed.snapshot.deliveries.iter().map(|d| d.message_id.as_str()).collect();
        let waiting: Vec<&Message> = parsed.snapshot.messages.iter().filter(|m| m.to.teammate_id == teammate_id && !delivered.contains(m.message_id.as_str())).collect();
        let asked: Vec<&Message> = waiting.iter().copied().filter(|m| include.contains(&m.message_id)).collect();
        let mut chosen: Vec<Message> = asked.iter().map(|m| (*m).clone()).collect();
        chosen.extend(waiting.iter().filter(|m| !asked.iter().any(|a| a.message_id == m.message_id)).map(|m| (*m).clone()));
        chosen.truncate(limit.max(asked.len()));
        Ok(Unread { remaining: waiting.len().saturating_sub(chosen.len()), messages: chosen })
    }

    /// The messages went into this mission's prompt; recorded in the channel, so "unread" is derived from the file.
    pub fn mark_delivered(&self, message_ids: &[String], mission_id: &str) -> Result<(), String> {
        let _one = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        if !is_safe_id(mission_id) {
            return Err("Mission id is invalid".into());
        }
        let mut unique: Vec<&String> = Vec::new();
        for id in message_ids {
            if !unique.contains(&id) {
                unique.push(id);
            }
        }
        if unique.is_empty() {
            return Ok(());
        }
        if !unique.iter().all(|id| is_safe_id(id)) {
            return Err("Message id is invalid".into());
        }
        let at = (self.now)();
        self.append(|parsed| {
            let known: HashSet<&str> = parsed.snapshot.messages.iter().map(|m| m.message_id.as_str()).collect();
            let delivered: HashSet<&str> = parsed.snapshot.deliveries.iter().map(|d| d.message_id.as_str()).collect();
            let mut sequence = parsed.next_sequence;
            unique
                .iter()
                .map(|id| {
                    if !known.contains(id.as_str()) {
                        return Err("Unknown workroom message".to_owned());
                    }
                    if delivered.contains(id.as_str()) {
                        return Err("Workroom message was already delivered".to_owned());
                    }
                    let mut delivery = Map::new();
                    delivery.insert("messageId".into(), json!(id));
                    delivery.insert("missionId".into(), json!(mission_id));
                    delivery.insert("deliveredAt".into(), json!(at));
                    let record = json!({"schemaVersion": WORKROOM_SCHEMA_VERSION, "recordType": "workroom.delivery", "sequence": sequence, "occurredAt": at, "delivery": delivery});
                    sequence += 1;
                    Ok(record)
                })
                .collect()
        })?;
        Ok(())
    }

    pub fn read(&self) -> Snapshot {
        let _one = self.serial.lock().unwrap_or_else(|e| e.into_inner());
        read_file(&self.path()).snapshot
    }
}

#[cfg(test)]
#[path = "workroom_tests.rs"]
mod tests;

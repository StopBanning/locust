//! Read-only readers for Locust's append-only mission ledgers.
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

pub const MAX_LINE: usize = 1024 * 1024;
pub const MAX_ASSISTANT: usize = 131_072;
pub const MAX_EVENTS: usize = 2000;
const MAX_JSON_DEPTH: usize = 6;
const MAX_JSON_ARRAY: usize = 32;
const MAX_JSON_KEYS: usize = 48;
// Fix 4: keep running conversations active until silent for 15 minutes instead of 120 seconds.
pub const ACTIVE_WINDOW: Duration = Duration::from_secs(15 * 60);
/// How often a Session re-lists the folder (for follow-up files) unless told otherwise.
pub const RESCAN: Duration = Duration::from_secs(2);
const HEAD: u64 = MAX_LINE as u64 + 1;
const TAIL: u64 = 2 * MAX_LINE as u64;
pub type Record = Value;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Metadata {
    pub mission_id: String,
    pub prompt: String,
    pub runtime: String,
    pub model: String,
    pub created_at: String,
    pub parent: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    pub parent: Option<String>,
    pub words: String,
    pub mission_id: String,
    pub title: String,
    pub runtime: String,
    pub model: String,
    pub created_at: String,
    pub running: bool,
    pub failed: bool,
    pub updated: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Status {
    Running,
    Done,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    User(String),
    CarryOn {
        conversation_id: String,
    },
    Assistant {
        item_id: String,
        text: String,
        r#final: bool,
    },
    Tool {
        name: String,
        status: Status,
        summary: String,
        detail: Box<ToolDetail>,
    },
    Plan {
        id: String,
        steps: Vec<PlanStep>,
        finished: bool,
    },
    Thought {
        id: String,
        text: String,
        duration_ms: Option<u64>,
    },
    Note {
        level: String,
        text: String,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolDetail {
    pub id: String,
    pub parent: String,
    pub arguments: String,
    pub output: String,
    pub output_reported: bool,
    pub runtime_status: String,
    pub duration_ms: Option<u64>,
    pub exit_code: Option<i64>,
    pub helper: bool,
    pub helper_type: String,
    pub background: bool,
    pub finished: bool,
    pub progress: String,
    pub background_ended: String,
    pub children: Vec<Item>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct PlanStep {
    pub text: String,
    pub state: String,
}

fn sensitive_key(key: &str) -> bool {
    crate::secrets::sensitive_key(key)
}

fn scrub_secrets(text: &str) -> String {
    crate::secrets::redact_secrets(text)
}

fn scrub(text: &str) -> String {
    let safe = scrub_secrets(text);
    let (bounded, _) = capped(&safe, MAX_ASSISTANT);
    if bounded.len() < safe.len() {
        format!("{bounded}… [truncated]")
    } else {
        safe
    }
}

/// Cap and redact JSON before pretty-printing (codex-events.ts:sanitizeJson + SENSITIVE_KEY).
fn scrub_json(value: &Value, depth: usize) -> Value {
    if depth >= MAX_JSON_DEPTH {
        return Value::String("[depth limit]".into());
    }
    match value {
        Value::Null | Value::Bool(_) => value.clone(),
        Value::Number(n) => {
            if n.as_f64().is_some_and(|f| f.is_finite()) {
                value.clone()
            } else {
                Value::String(n.to_string())
            }
        }
        Value::String(s) => Value::String(scrub_secrets(s)),
        Value::Array(rows) => Value::Array(
            rows.iter()
                .take(MAX_JSON_ARRAY)
                .map(|row| scrub_json(row, depth + 1))
                .collect(),
        ),
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, child) in map.iter().take(MAX_JSON_KEYS) {
                let safe_key = if key.contains('\0') || key.len() > 512 {
                    format!(
                        "{}[truncated]",
                        key.replace('\0', "").chars().take(500).collect::<String>()
                    )
                } else {
                    key.clone()
                };
                if sensitive_key(key) {
                    out.insert(safe_key, Value::String("[redacted]".into()));
                } else {
                    out.insert(safe_key, scrub_json(child, depth + 1));
                }
            }
            Value::Object(out)
        }
    }
}

fn display_value(value: &Value) -> String {
    match value {
        Value::String(s) => scrub(s),
        other => scrub(&serde_json::to_string_pretty(&scrub_json(other, 0)).unwrap_or_default()),
    }
}

pub fn read_plan(value: &Value) -> Vec<PlanStep> {
    value
        .as_array()
        .or_else(|| value.get("plan").and_then(Value::as_array))
        .map(|rows| {
            rows.iter()
                .take(256)
                .filter_map(|row| {
                    let text = row.as_str().or_else(|| {
                        ["step", "text", "title", "name", "content"]
                            .iter()
                            .find_map(|k| row.get(k).and_then(Value::as_str))
                    })?;
                    if text.is_empty() {
                        return None;
                    }
                    let status = string(row, "status").to_lowercase();
                    let state = if status.contains("complete") || status == "done" {
                        "done"
                    } else if status.contains("progress")
                        || status == "running"
                        || status == "active"
                    {
                        "running"
                    } else {
                        "pending"
                    };
                    Some(PlanStep {
                        text: scrub(text),
                        state: state.into(),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn duration_text(ms: u64) -> String {
    let seconds = (ms + 500) / 1000;
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {:02}m", seconds / 3600, seconds / 60 % 60)
    }
}

pub fn bounded_output(output: &str, all: bool) -> String {
    let trimmed = output.trim_end();
    let lines: Vec<_> = trimmed.split('\n').collect();
    if all || lines.len() <= 16 {
        return trimmed.into();
    }
    format!(
        "{}\n… {} more lines …\n{}",
        lines[..8].join("\n"),
        lines.len() - 16,
        lines[lines.len() - 8..].join("\n")
    )
}

// Ledger timestamps are RFC3339. Parse offsets and fractions without a date dependency.
fn timestamp(value: &Value) -> Option<i64> {
    let t = value.get("occurredAt")?.as_str()?;
    let n = |a: usize, b: usize| t.get(a..b)?.parse::<i64>().ok();
    let (mut y, m, d) = (n(0, 4)?, n(5, 7)?, n(8, 10)?);
    y -= i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yy = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let days = era * 146097 + yy * 365 + yy / 4 - yy / 100 + doy - 719468;
    let rest = t.get(19..)?;
    let offset_at = rest.find(['Z', '+', '-'])?;
    let fraction = &rest[..offset_at];
    let millis = if fraction.is_empty() {
        0
    } else {
        format!("{:<03}", fraction.strip_prefix('.')?)
            .replace(' ', "0")
            .get(..3)?
            .parse::<i64>()
            .ok()?
    };
    let zone = &rest[offset_at..];
    let offset = if zone == "Z" {
        0
    } else {
        let h = zone.get(1..3)?.parse::<i64>().ok()?;
        let m = zone.get(4..6)?.parse::<i64>().ok()?;
        (h * 60 + m) * 60 * if zone.starts_with('-') { -1 } else { 1 }
    };
    Some(((days * 24 + n(11, 13)?) * 3600 + n(14, 16)? * 60 + n(17, 19)? - offset) * 1000 + millis)
}
fn took(event: &Value, p: &Value, start: Option<&i64>) -> Option<u64> {
    p.get("durationMs")
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && *v >= 0.0)
        .map(|v| v as u64)
        .or_else(|| Some((timestamp(event)? - *start?).max(0) as u64))
}

fn string(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

pub fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub fn metadata(record: &Record) -> Option<Metadata> {
    if record.get("recordType")?.as_str()? != "mission.created" {
        return None;
    }
    let m = record.get("metadata")?;
    let id = string(m, "missionId");
    if !safe_id(&id) {
        return None;
    }
    Some(Metadata {
        mission_id: id,
        prompt: string(m, "prompt"),
        runtime: string(m, "runtime"),
        model: string(m, "model"),
        created_at: string(m, "createdAt"),
        parent: m
            .get("continuesFrom")
            .map(|v| string(v, "missionId"))
            .filter(|id| safe_id(id)),
    })
}

// Fix 8: return both the UTF-16-bounded slice and its UTF-16 unit count for incremental tracking.
fn capped(text: &str, limit: usize) -> (&str, usize) {
    let mut used = 0;
    let mut end = 0;
    for c in text.chars() {
        let len = c.len_utf16();
        if used + len > limit {
            break;
        }
        used += len;
        end += c.len_utf8();
    }
    (&text[..end], used)
}

fn one_line(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(240)
        .collect()
}

fn decode(line: &[u8]) -> Option<Record> {
    let mut value: Value = serde_json::from_slice(line).ok()?;
    let version = value.get("schemaVersion")?.as_u64()?;
    if !(1..=27).contains(&version) {
        return None;
    }
    match value.get("recordType")?.as_str()? {
        "mission.created" => {
            metadata(&value)?;
        }
        "mission.event" => {
            let event = value.get_mut("event")?;
            event.get("sequence")?.as_u64()?;
            event.get("type")?.as_str()?;
            // Provenance is unnecessary for drawing; do not retain raw evidence.
            if let Some(payload) = event.get_mut("payload").and_then(Value::as_object_mut) {
                payload.remove("evidence");
                // Never interpret raw Codex reasoning or legacy exec reasoning items.
                payload.remove("content");
                payload.remove("raw");
                payload.remove("reasoning_content");
                payload.remove("item");
            }
        }
        _ => return None,
    }
    Some(value)
}

#[derive(Clone, Default)]
struct Lines {
    pending: Vec<u8>,
    skipping: bool,
}

impl Lines {
    fn feed(&mut self, bytes: &[u8], records: &mut Vec<Record>) {
        for &byte in bytes {
            if byte == b'\n' {
                if !self.skipping
                    && let Some(record) = decode(&self.pending)
                {
                    records.push(record);
                }
                self.pending.clear();
                self.skipping = false;
            } else if !self.skipping {
                if self.pending.len() == MAX_LINE {
                    // Skip oversized lines without allocating their full length.
                    self.pending.clear();
                    self.skipping = true;
                } else {
                    self.pending.push(byte);
                }
            }
        }
    }
}

/// Offset includes bytes in the retained incomplete last line.
pub struct Tail {
    path: PathBuf,
    pub offset: u64,
    pub reset: bool,
    pub error: Option<io::ErrorKind>,
    pub modified: Option<SystemTime>,
    lines: Lines,
    marker: Vec<u8>,
    first: bool,
}

impl Tail {
    pub fn new(path: impl Into<PathBuf>, offset: u64) -> Self {
        Self {
            path: path.into(),
            offset,
            reset: false,
            error: None,
            modified: None,
            lines: Lines::default(),
            marker: Vec::new(),
            first: true,
        }
    }

    pub fn poll(&mut self) -> Vec<Record> {
        self.error = None;
        match self.read_new() {
            Ok(records) => records,
            Err(error) => {
                self.reset = false;
                self.error = Some(error.kind());
                Vec::new()
            }
        }
    }

    fn read_new(&mut self) -> io::Result<Vec<Record>> {
        // Production code never requests write, append, truncate, or create access.
        let mut file = OpenOptions::new().read(true).open(&self.path)?;
        let meta = file.metadata()?;
        let modified = meta.modified().ok();

        // Fix 1: stage offset/lines/marker/reset locally and guard marker subtraction so polls commit atomically.
        let mut offset = self.offset;
        let mut lines = self.lines.clone();
        let mut marker = self.marker.clone();
        let mut reset = meta.len() < offset;
        if !reset && !marker.is_empty() {
            if offset < marker.len() as u64 {
                reset = true;
            } else {
                file.seek(SeekFrom::Start(offset - marker.len() as u64))?;
                let mut actual = vec![0; marker.len()];
                reset = file.read_exact(&mut actual).is_err() || actual != marker;
            }
        }
        if reset {
            // The marker helps detect truncation followed by regrowth between polls.
            offset = 0;
            lines = Lines::default();
            marker.clear();
        }

        if self.first && offset > 0 && !reset {
            // Fix 5 (Finding 7): recover an incomplete line preceding a non-zero initial offset instead of dropping its completion.
            let back = offset.min(MAX_LINE as u64) as usize;
            file.seek(SeekFrom::Start(offset - back as u64))?;
            let mut prefix = vec![0; back];
            file.read_exact(&mut prefix)?;
            if let Some(pos) = prefix.iter().rposition(|&b| b == b'\n') {
                lines.pending.extend_from_slice(&prefix[pos + 1..]);
            } else if (back as u64) == offset && back < MAX_LINE {
                lines.pending.extend_from_slice(&prefix);
            } else {
                lines.skipping = true;
            }
        }

        // Snapshot EOF so an active writer cannot keep this worker reading forever.
        let mut left = meta.len().saturating_sub(offset);
        let mut records = Vec::new();
        if left > 0 || reset || (self.first && offset > 0 && marker.is_empty()) {
            file.seek(SeekFrom::Start(offset))?;
            let mut buffer = [0; 8192];
            while left > 0 {
                let wanted = left.min(buffer.len() as u64) as usize;
                let n = file.read(&mut buffer[..wanted])?;
                if n == 0 {
                    break;
                }
                lines.feed(&buffer[..n], &mut records);
                offset += n as u64;
                left -= n as u64;
            }

            let count = offset.min(64) as usize;
            file.seek(SeekFrom::Start(offset - count as u64))?;
            marker = vec![0; count];
            file.read_exact(&mut marker)?;
        }

        self.modified = modified;
        self.offset = offset;
        self.lines = lines;
        self.marker = marker;
        self.reset = reset;
        self.first = false;
        Ok(records)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Transcript {
    pub metadata: Option<Metadata>,
    pub items: Vec<Item>,
    pub running: bool,
    pub failed: bool,
    pub error: Option<io::ErrorKind>,
    // Sequence keys make out-of-order batches and duplicate records replay correctly.
    events: BTreeMap<u64, Value>,
    checkpoints: Vec<Value>,
    pub recovery: Option<crate::recovery::Offer>,
    pub context: Option<crate::context_usage::Reading>,
}

impl Transcript {
    pub fn apply(&mut self, records: Vec<Record>) {
        if records.is_empty() {
            return;
        }
        for record in records {
            if record["recordType"] == "mission.checkpoint" {
                self.checkpoints.push(record["checkpoint"].clone());
                self.checkpoints
                    .sort_by_key(|p| p["epoch"].as_u64().unwrap_or(0));
                self.checkpoints.dedup_by_key(|p| p["epoch"].as_u64());
                if self.checkpoints.len() > MAX_EVENTS {
                    self.checkpoints.remove(0);
                }
            } else if let Some(m) = metadata(&record) {
                self.metadata = Some(m);
            } else if let Some(event) = record.get("event")
                && let Some(sequence) = event.get("sequence").and_then(Value::as_u64)
            {
                self.events.insert(sequence, event.clone());
                while self.events.len() > MAX_EVENTS {
                    self.events.pop_first();
                }
            }
        }

        let mut items = Vec::new();
        if let Some(m) = &self.metadata {
            items.push(Item::User(scrub(&m.prompt)));
        }
        // Fix 8: store (item_index, utf16_units) so streaming deltas count UTF-16 units incrementally.
        let mut messages = HashMap::<String, (usize, usize)>::new();
        let mut tools = HashMap::<String, usize>::new();
        let mut plans = HashMap::<String, usize>::new();
        let mut starts = HashMap::<String, i64>::new();
        self.running = self.metadata.is_some();
        self.failed = false;

        self.context = None;
        let mut carry = false;
        let mut finished = false;
        for (sequence, event) in &self.events {
            let kind = string(event, "type");
            let p = event.get("payload").unwrap_or(&Value::Null);
            let id = string(p, "itemId");
            match kind.as_str() {
                "message.delta" if !id.is_empty() => {
                    let (at, used) = messages.entry(id.clone()).or_insert_with(|| {
                        items.push(Item::Assistant {
                            item_id: id,
                            text: String::new(),
                            r#final: false,
                        });
                        (items.len() - 1, 0)
                    });
                    if let Some(Item::Assistant { text, r#final, .. }) = items.get_mut(*at) {
                        let piece = scrub_secrets(&string(p, "text"));
                        if string(p, "operation") == "replace" {
                            let (slice, count) = capped(&piece, MAX_ASSISTANT);
                            text.clear();
                            text.push_str(slice);
                            *used = count;
                        } else {
                            let left = MAX_ASSISTANT.saturating_sub(*used);
                            let (slice, count) = capped(&piece, left);
                            text.push_str(slice);
                            *used += count;
                        }
                        *r#final = p.get("final").and_then(Value::as_bool).unwrap_or(false);
                    }
                }
                "step.started" | "step.completed" | "step.failed" => {
                    let reasoning = string(p, "stepKind") == "reasoning"
                        || string(p, "itemType").to_lowercase().contains("reasoning");
                    let step_key = format!("step:{id}");
                    if string(p, "itemType") == "background" {
                        if let Some(at) = tools.get(&id)
                            && let Some(Item::Tool { detail, .. }) = items.get_mut(*at)
                        {
                            detail.background = true;
                            if kind == "step.started" {
                                detail.progress = scrub(&string(p, "message"));
                                continue;
                            }
                            let state = string(p, "status");
                            detail.background_ended = if kind == "step.failed" {
                                "failed".into()
                            } else if matches!(
                                state.as_str(),
                                "completed" | "failed" | "stopped" | "stopped-with-run" | "ended"
                            ) {
                                state
                            } else {
                                "ended".into()
                            };
                            if detail.helper
                                && let Some(message) = p.get("message").and_then(Value::as_str)
                            {
                                detail.output = scrub(message);
                                detail.output_reported = true;
                            }
                        }
                    } else if kind == "step.started" {
                        if let Some(t) = timestamp(event) {
                            starts.entry(step_key).or_insert(t);
                        }
                    } else if reasoning {
                        // missionView.ts consumes only normalized message, never evidence/content.
                        let text = scrub(string(p, "message").trim());
                        let duration_ms = took(event, p, starts.get(&step_key));
                        starts.remove(&step_key);
                        if !text.is_empty() || duration_ms.is_some_and(|ms| ms >= 1000) {
                            items.push(Item::Thought {
                                id: format!("{id}:{sequence}"),
                                text,
                                duration_ms,
                            });
                        }
                    } else {
                        starts.remove(&step_key);
                    }
                }
                "tool.started" | "tool.completed" | "tool.failed" => {
                    let key = if id.is_empty() {
                        sequence.to_string()
                    } else {
                        id
                    };
                    if kind == "tool.started"
                        && let Some(t) = timestamp(event)
                    {
                        starts.entry(key.clone()).or_insert(t);
                    }
                    let at = *tools.entry(key.clone()).or_insert_with(|| {
                        items.push(Item::Tool {
                            name: String::new(),
                            status: Status::Running,
                            summary: String::new(),
                            detail: Box::new(ToolDetail {
                                id: key.clone(),
                                ..ToolDetail::default()
                            }),
                        });
                        items.len() - 1
                    });
                    if let Some(Item::Tool {
                        name,
                        status,
                        summary,
                        detail,
                    }) = items.get_mut(at)
                    {
                        let candidate = string(p, "name");
                        if !candidate.is_empty() {
                            *name = scrub(&candidate);
                        }
                        detail.helper = matches!(
                            name.to_lowercase().as_str(),
                            "task" | "agent" | "subagent" | "spawn_agent"
                        ) || name.to_lowercase().starts_with("subagent:");
                        if let Some(parent) = p.get("parentItemId").and_then(Value::as_str) {
                            detail.parent = parent.into();
                        }
                        *status = if kind.ends_with("failed") {
                            Status::Failed
                        } else if kind.ends_with("completed") {
                            Status::Done
                        } else {
                            Status::Running
                        };
                        for field in ["arguments", "input", "command"] {
                            if let Some(value) = p.get(field) {
                                detail.arguments = display_value(value);
                            }
                        }
                        if let Some(output) = p.get("output").and_then(Value::as_str) {
                            detail.output = scrub(output);
                            detail.output_reported = true;
                        }
                        if let Some(state) = p.get("status").and_then(Value::as_str) {
                            detail.runtime_status = state.into();
                        }
                        if let Some(code) = p.get("exitCode").and_then(Value::as_i64) {
                            detail.exit_code = Some(code);
                        }
                        if let Some(v) = p.get("background").and_then(Value::as_bool) {
                            detail.background = v;
                        }
                        if let Some(v) = p.get("backgroundEnded").and_then(Value::as_str) {
                            detail.background_ended = v.into();
                        }
                        if detail.helper
                            && let Some(v) = p.get("status").and_then(Value::as_str)
                            && !matches!(
                                v,
                                "error"
                                    | "completed"
                                    | "failed"
                                    | "in_progress"
                                    | "started"
                                    | "cancelled"
                            )
                        {
                            detail.helper_type = scrub(v);
                        }
                        if kind != "tool.started" {
                            detail.duration_ms = took(event, p, starts.get(&key));
                            starts.remove(&key);
                        }
                        // Prefer title; never let a later command wipe an earlier title (missionView.ts:3557).
                        let title = string(p, "title");
                        if !title.is_empty() {
                            *summary = one_line(&scrub(&title));
                        } else if summary.is_empty() {
                            let command = string(p, "command");
                            if !command.is_empty() {
                                *summary = one_line(&scrub(&command));
                            }
                        }
                    }
                }
                "plan.updated" => {
                    let key = "plan".to_owned();
                    let at = *plans.entry(key.clone()).or_insert_with(|| {
                        items.push(Item::Plan {
                            id: key.clone(),
                            steps: Vec::new(),
                            finished: false,
                        });
                        items.len() - 1
                    });
                    items[at] = Item::Plan {
                        id: key,
                        steps: read_plan(&p["plan"]),
                        finished: false,
                    };
                }
                "adapter.diagnostic" => {
                    let level = string(p, "level");
                    if string(p, "code").ends_with(".context_compacted") {
                        self.context = None;
                        items.push(Item::Note {
                            level: String::new(),
                            text: scrub(&string(p, "message")),
                        });
                    } else if level == "warning" || level == "error" {
                        items.push(Item::Note {
                            level,
                            text: scrub(&string(p, "message")),
                        });
                    }
                }
                "route.limit_detected" => {
                    if !finished {
                        carry = true;
                        self.running = false;
                    }
                    let text = scrub(&string(p, "message"));
                    if !text.is_empty() {
                        items.push(Item::Note {
                            level: "warning".into(),
                            text,
                        });
                    }
                }
                "run.failed" => {
                    self.running = false;
                    self.failed = true;
                    finished = false;
                    carry |= carry_on_failure(p);
                    let text = scrub(&string(p, "message"));
                    if !text.is_empty() {
                        items.push(Item::Note {
                            level: "error".into(),
                            text,
                        });
                    }
                }
                // Fix 4: a later completion or restart clears an earlier completed/failed flag.
                "run.completed" | "run.cancelled" => {
                    carry = false;
                    finished = true;
                    if kind == "run.completed" {
                        self.context = crate::context_usage::Reading::from_usage(&p["usage"])
                            .or(self.context.take());
                    }
                    if kind == "run.cancelled" {
                        items.push(Item::Note {
                            level: String::new(),
                            text: "Stopped".into(),
                        });
                    }
                    self.running = false;
                    self.failed = false;
                }
                "run.started" => {
                    carry = false;
                    finished = false;
                    self.running = true;
                    self.failed = false;
                }
                _ => {}
            }
        }
        if carry
            && !self.running
            && !finished
            && let Some(m) = &self.metadata
        {
            items.push(Item::CarryOn {
                conversation_id: m.mission_id.clone(),
            });
        }
        // Attach children in reverse order so nested helpers retain their own calls.
        for at in (0..items.len()).rev() {
            let parent = match &items[at] {
                Item::Tool { detail, .. } => detail.parent.clone(),
                _ => continue,
            };
            if parent.is_empty() {
                continue;
            }
            if let Some(to) = items.iter().position(
                |i| matches!(i, Item::Tool { detail, .. } if detail.id == parent && detail.helper),
            ) && to < at
            {
                let child = items.remove(at);
                if let Item::Tool { detail, .. } = &mut items[to] {
                    detail.children.insert(0, child);
                }
            }
        }
        // Orphaned parentItemId calls stay nowhere (missionView.ts:3281-3282).
        items
            .retain(|item| !matches!(item, Item::Tool { detail, .. } if !detail.parent.is_empty()));
        fn qualify(item: &mut Item, mission: &str, finished: bool) {
            match item {
                Item::Tool { detail, .. } => {
                    detail.finished = finished;
                    if !mission.is_empty() {
                        detail.id = format!("{mission}:{}", detail.id);
                    }
                    for child in &mut detail.children {
                        qualify(child, mission, finished);
                    }
                }
                Item::Plan {
                    id,
                    finished: ended,
                    ..
                } => {
                    *ended = finished;
                    if !mission.is_empty() {
                        *id = format!("{mission}:{id}");
                    }
                }
                Item::Thought { id, .. } if !mission.is_empty() => {
                    *id = format!("{mission}:{id}");
                }
                _ => {}
            }
        }
        let mission = self
            .metadata
            .as_ref()
            .map(|m| m.mission_id.as_str())
            .unwrap_or("");
        for item in &mut items {
            qualify(item, mission, !self.running);
        }
        let phase = if self.running {
            "interrupted"
        } else if self.failed {
            "failed"
        } else if self
            .events
            .values()
            .rev()
            .find(|e| {
                matches!(
                    e["type"].as_str(),
                    Some("run.completed" | "run.failed" | "run.cancelled")
                )
            })
            .is_some_and(|e| e["type"] == "run.cancelled")
        {
            "cancelled"
        } else {
            "completed"
        };
        self.recovery =
            crate::recovery::offer(phase, &self.checkpoints, usize::from(self.error.is_some()));
        self.items = items;
    }
}

fn carry_on_failure(payload: &Value) -> bool {
    let detail = ["detail", "message", "reason", "error"]
        .iter()
        .filter_map(|key| payload.get(key))
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    detail.contains("limit") || detail.contains("provider")
}

// Standalone snapshot API; the window uses Session to keep its byte offsets.
pub fn read_transcript(path: impl AsRef<Path>) -> Transcript {
    let mut tail = Tail::new(path.as_ref(), 0);
    let records = tail.poll();
    let mut transcript = Transcript::default();
    transcript.apply(records);
    transcript.error = tail.error;
    transcript
}

fn head_with_offset(path: &Path) -> io::Result<(Vec<Record>, u64, Lines)> {
    let mut file = OpenOptions::new().read(true).open(path)?.take(HEAD);
    let mut lines = Lines::default();
    let mut records = Vec::new();
    let mut read_bytes = 0u64;
    let mut bytes = [0; 8192];
    loop {
        let n = file.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        read_bytes += n as u64;
        lines.feed(&bytes[..n], &mut records);
        if records.iter().any(|r| metadata(r).is_some()) {
            break;
        }
    }
    Ok((records, read_bytes, lines))
}

fn head(path: &Path) -> io::Result<Vec<Record>> {
    head_with_offset(path).map(|(records, _, _)| records)
}

// Fix 4: start reading tail bytes after `after` so a file already read in full by head() is never re-read.
fn end_after(path: &Path, after: u64, mut lines: Lines) -> io::Result<Vec<Record>> {
    let mut file = OpenOptions::new().read(true).open(path)?;
    let len = file.metadata()?.len();
    if len <= after {
        return Ok(Vec::new());
    }
    let start = len.saturating_sub(TAIL).max(after);
    if start > after {
        lines = Lines::default();
        file.seek(SeekFrom::Start(start - 1))?;
        let mut byte = [0];
        file.read_exact(&mut byte)?;
        lines.skipping = byte[0] != b'\n';
    }
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::new();
    file.take(TAIL).read_to_end(&mut bytes)?;
    let mut records = Vec::new();
    lines.feed(&bytes, &mut records);
    Ok(records)
}

fn fresh(modified: Option<SystemTime>, now: SystemTime) -> bool {
    modified.is_some_and(|m| now.duration_since(m).unwrap_or_default() < ACTIVE_WINDOW)
}

// Convenience API when an unreadable directory need not be distinguished from an empty one.
#[cfg(any(test, feature = "test-hooks"))]
pub fn list_recent(dir: impl AsRef<Path>, limit: usize) -> Vec<Summary> {
    try_list_recent(dir.as_ref(), limit).unwrap_or_default()
}

pub fn try_list_recent(dir: &Path, limit: usize) -> io::Result<Vec<Summary>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir)? {
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_ok_and(|t| t.is_file())
            || entry.path().extension().is_none_or(|e| e != "jsonl")
        {
            continue;
        }
        let path = entry.path();
        let Some(id) = path.file_stem().and_then(|v| v.to_str()) else {
            continue;
        };
        if !safe_id(id) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        paths.push((
            meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            path,
            meta.len(),
        ));
    }
    paths.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    let mut summaries = Vec::new();
    for (modified, path, len) in paths {
        if summaries.len() == limit {
            break;
        }
        let Ok((mut records, read_bytes, lines)) = head_with_offset(&path) else {
            continue;
        };
        let Some(m) = records.iter().find_map(metadata) else {
            continue;
        };
        // Fix 4: at most HEAD + TAIL bytes; skip end_after entirely when head_with_offset already read the whole file.
        if read_bytes < len
            && let Ok(tail) = end_after(&path, read_bytes, lines)
        {
            records.extend(tail);
        }
        // Fix 4: replay run lifecycle events in sequence order so a later run.started clears earlier completed/failed state.
        let mut events = BTreeMap::new();
        for record in &records {
            if let Some(event) = record.get("event")
                && let Some(seq) = event.get("sequence").and_then(Value::as_u64)
            {
                events.insert(seq, event);
            }
        }
        let mut running = true;
        let mut failed = false;
        for event in events.values() {
            match event.get("type").and_then(Value::as_str) {
                Some("run.started") => {
                    running = true;
                    failed = false;
                }
                Some("route.limit_detected") => {
                    running = false;
                }
                Some("run.failed") => {
                    running = false;
                    failed = true;
                }
                Some("run.completed" | "run.cancelled") => {
                    running = false;
                    failed = false;
                }
                _ => {}
            }
        }
        let updated = events
            .values()
            .rev()
            .find_map(|e| e.get("occurredAt").and_then(Value::as_str))
            .unwrap_or(&m.created_at)
            .to_owned();
        let title: String = m
            .prompt
            .lines()
            .next()
            .unwrap_or_default()
            .trim()
            .chars()
            .take(60)
            .collect();
        summaries.push(Summary {
            parent: m.parent,
            words: m.prompt,
            mission_id: m.mission_id,
            title: if title.is_empty() {
                "Untitled".into()
            } else {
                title
            },
            runtime: m.runtime,
            model: m.model,
            created_at: m.created_at,
            running: running && fresh(Some(modified), SystemTime::now()),
            failed,
            updated,
        });
    }
    Ok(summaries)
}

struct Reader {
    id: String,
    tail: Tail,
    transcript: Transcript,
    drained: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub recovery: Option<crate::recovery::Offer>,
    pub context: Option<crate::context_usage::Reading>,
    pub items: Vec<Item>,
    pub tip: String,
    pub running: bool,
    pub failed: bool,
}

/// A follow-up is a NEW mission file although MCP returns the original conversation ID.
pub struct Session {
    dir: PathBuf,
    /// Further read-only folders listed with `dir` (the native engine's own ledger folder): a continuation file may
    /// live in a different folder from the conversation it continues.
    extra: Vec<PathBuf>,
    anchor: String,
    readers: Vec<Reader>,
    scanned: Option<Instant>,
    headers: HashMap<PathBuf, (u64, Metadata)>,
    rescan: Duration,
    listed: bool,
}

impl Session {
    pub fn new(dir: PathBuf, mission_id: String) -> Self {
        Self {
            dir,
            extra: Vec::new(),
            anchor: mission_id,
            readers: Vec::new(),
            scanned: None,
            headers: HashMap::new(),
            rescan: RESCAN,
            listed: false,
        }
    }

    /// Also list `dir` when scanning. A folder that cannot be listed is skipped as long as another one can.
    pub fn also_read(mut self, dir: PathBuf) -> Self {
        if dir != self.dir && !self.extra.contains(&dir) {
            self.extra.push(dir);
        }
        self
    }

    /// How long a running tip may go between folder scans (default RESCAN). A caller that
    /// is not looking (a hidden window) can ask for less. Ended tips are always re-scanned.
    pub fn set_rescan(&mut self, every: Duration) {
        self.rescan = every;
    }

    /// Whether the last folder scan could list the folder at all. When poll() fails, this
    /// tells "folder unreadable" from "folder fine, this conversation's file not there".
    pub fn dir_listed(&self) -> bool {
        self.listed
    }

    /// True once the newest file records its run as completed, failed or cancelled. Unlike
    /// Snapshot::running this does not turn false when the file merely goes quiet.
    pub fn tip_ended(&self) -> bool {
        self.readers
            .last()
            .is_some_and(|r| r.transcript.metadata.is_some() && !r.transcript.running)
    }

    /// Terminal state of the newest turn only; ancestor stops must not hold a later follow-up.
    pub fn tip_phase(&self) -> &'static str {
        let Some(reader) = self.readers.last() else {
            return "running";
        };
        let t = &reader.transcript;
        if t.running {
            "running"
        } else if t.failed {
            "failed"
        } else if t
            .items
            .iter()
            .any(|i| matches!(i, Item::Note { text, .. } if text == "Stopped"))
        {
            "cancelled"
        } else {
            "completed"
        }
    }

    /// How long the newest file has gone without being written, as of `now`.
    pub fn tip_quiet(&self, now: SystemTime) -> Option<Duration> {
        let modified = self.readers.last()?.tail.modified?;
        Some(now.duration_since(modified).unwrap_or_default())
    }

    pub fn anchor(&self) -> &str {
        &self.anchor
    }

    #[cfg(any(test, feature = "test-hooks"))]
    pub fn cached_headers(&self) -> usize {
        self.headers.len()
    }

    #[cfg(any(test, feature = "test-hooks"))]
    pub fn rescan(&self) -> Duration {
        self.rescan
    }

    pub fn poll(&mut self) -> io::Result<Snapshot> {
        if !safe_id(&self.anchor) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let tip_finished = self
            .readers
            .last()
            .is_none_or(|r| !r.transcript.running && r.tail.offset > 0);
        if self.scanned.is_none_or(|at| at.elapsed() >= self.rescan) || tip_finished {
            let paths = self.chain()?;
            let mut old: HashMap<_, _> =
                self.readers.drain(..).map(|r| (r.id.clone(), r)).collect();
            for (id, path) in paths {
                self.readers.push(old.remove(&id).unwrap_or_else(|| Reader {
                    id,
                    tail: Tail::new(path, 0),
                    transcript: Transcript::default(),
                    drained: false,
                }));
            }
            self.scanned = Some(Instant::now());
        }

        let mut snapshot = Snapshot::default();
        let count = self.readers.len();
        for (index, reader) in self.readers.iter_mut().enumerate() {
            let is_tip = index + 1 == count;
            if is_tip {
                reader.drained = false;
            }
            // Fix 5: drain an ancestor once more after a child file appears so final parent lines are not missed.
            if is_tip
                || !reader.drained
                || reader.tail.offset == 0
                || !reader.tail.lines.pending.is_empty()
            {
                let records = reader.tail.poll();
                if reader.tail.reset {
                    reader.transcript = Transcript::default();
                }
                reader.transcript.error = reader.tail.error;
                if let Some(kind) = reader.transcript.error {
                    return Err(kind.into());
                }
                let empty_poll = records.is_empty();
                reader.transcript.apply(records);
                if !is_tip
                    && reader.tail.offset > 0
                    && reader.tail.lines.pending.is_empty()
                    && (!reader.transcript.running || empty_poll)
                {
                    reader.drained = true;
                }
            }
            snapshot.items.extend(
                reader
                    .transcript
                    .items
                    .iter()
                    .filter(|item| is_tip || !matches!(item, Item::CarryOn { .. }))
                    .cloned(),
            );
            if reader
                .transcript
                .events
                .values()
                .any(|e| string(&e["payload"], "code").ends_with(".context_compacted"))
            {
                snapshot.context = None;
            }
            snapshot.context = reader.transcript.context.clone().or(snapshot.context);
            snapshot.tip = reader.id.clone();
            snapshot.running =
                reader.transcript.running && fresh(reader.tail.modified, SystemTime::now());
            snapshot.failed = reader.transcript.failed;
            snapshot.recovery = if snapshot.running {
                None
            } else {
                reader.transcript.recovery.clone()
            };
        }
        Ok(snapshot)
    }

    /// The conversation's newest turn, read fresh from every folder this session lists (never a cached view).
    pub fn chain_tip(&mut self) -> io::Result<String> {
        if !safe_id(&self.anchor) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        self.headers.clear();
        self.chain()?
            .pop()
            .map(|(id, _)| id)
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }

    fn chain(&mut self) -> io::Result<Vec<(String, PathBuf)>> {
        let mut all = HashMap::new();
        self.listed = false;
        let mut folders = Vec::new();
        let mut first_error = None;
        for dir in std::iter::once(&self.dir).chain(self.extra.iter()) {
            match fs::read_dir(dir) {
                Ok(entries) => {
                    self.listed = true;
                    folders.push(entries);
                }
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }
        if !self.listed {
            return Err(first_error.unwrap_or_else(|| io::ErrorKind::NotFound.into()));
        }
        // Fix 3: skip unreadable directory entries instead of failing the whole session.
        for entry in folders.into_iter().flatten() {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_ok_and(|t| t.is_file())
                || entry.path().extension().is_none_or(|e| e != "jsonl")
            {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let path = entry.path();
            let len = meta.len();
            let refresh = self
                .headers
                .get(&path)
                .is_none_or(|(cached_len, _)| len < *cached_len);
            if refresh && let Ok(records) = head(&path) {
                if let Some(m) = records.iter().find_map(metadata) {
                    self.headers.insert(path.clone(), (len, m));
                } else {
                    self.headers.remove(&path);
                }
            }
            if let Some((_, m)) = self.headers.get(&path).cloned() {
                all.insert(
                    m.mission_id.clone(),
                    (m, path, meta.modified().unwrap_or(SystemTime::UNIX_EPOCH)),
                );
            }
        }
        if !all.contains_key(&self.anchor) {
            return Err(io::ErrorKind::NotFound.into());
        }

        // Fix 5: keep the exact ancestor path up to `self.anchor`, then walk only `self.anchor`'s own descendants.
        let mut result = Vec::new();
        let mut current = self.anchor.clone();
        let mut seen = HashSet::new();
        while seen.insert(current.clone()) {
            let Some((m, path, _)) = all.get(&current) else {
                break;
            };
            result.push((current.clone(), path.clone()));
            let Some(parent) = m.parent.as_ref() else {
                break;
            };
            if !all.contains_key(parent) || seen.contains(parent) {
                break;
            }
            current = parent.clone();
        }
        result.reverse();

        let mut tip = self.anchor.clone();
        while let Some(next) = all
            .values()
            .filter(|v| {
                v.0.parent.as_deref() == Some(tip.as_str()) && !seen.contains(&v.0.mission_id)
            })
            .max_by(|a, b| {
                a.2.cmp(&b.2)
                    .then_with(|| a.0.mission_id.cmp(&b.0.mission_id))
            })
        {
            tip = next.0.mission_id.clone();
            seen.insert(tip.clone());
            result.push((tip.clone(), next.1.clone()));
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);

    pub(super) struct Temp(pub(super) PathBuf);
    impl Temp {
        pub(super) fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "locust-ledger-test-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self, id: &str) -> PathBuf {
            self.0.join(format!("{id}.jsonl"))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn created(id: &str, prompt: &str) -> Value {
        json!({
            "schemaVersion":27,
            "recordType":"mission.created",
            "metadata":{
                "missionId":id,
                "prompt":prompt,
                "runtime":"codex",
                "model":"sample",
                "createdAt":"2026-01-01T00:00:00Z"
            }
        })
    }

    fn synthetic_transcript(rows: Vec<Value>) -> Transcript {
        // Exercise the JSONL boundary, including raw-evidence removal.
        let jsonl = rows.iter().map(|r| format!("{r}\n")).collect::<String>();
        let mut records = Vec::new();
        Lines::default().feed(jsonl.as_bytes(), &mut records);
        let mut transcript = Transcript::default();
        transcript.apply(records);
        transcript
    }

    #[test]
    fn rich_tool_preserves_arguments_output_and_clock() {
        let mut end = event(
            2,
            "tool.completed",
            json!({"itemId":"t","name":"Bash","output":"first\nlast","exitCode":0}),
        );
        end["event"]["occurredAt"] = json!("2026-01-02T00:00:03.250Z");
        let mut start = event(
            1,
            "tool.started",
            json!({"itemId":"t","name":"Bash","command":"synthetic-command","arguments":{"path":"fixture.txt"}}),
        );
        start["event"]["occurredAt"] = json!("2026-01-01T23:59:59.750Z");
        let t = synthetic_transcript(vec![start, end]);
        let Item::Tool { status, detail, .. } = &t.items[0] else {
            panic!("tool");
        };
        assert_eq!(*status, Status::Done);
        assert_eq!(detail.arguments, "synthetic-command");
        assert_eq!(detail.output, "first\nlast");
        assert_eq!(detail.duration_ms, Some(3500));
        assert_eq!(detail.exit_code, Some(0));
        let output = (1..=40)
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            bounded_output(&output, false),
            format!(
                "{}\n… 24 more lines …\n{}",
                (1..=8)
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join("\n"),
                (33..=40)
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        );
        assert_eq!(bounded_output(&output, true), output);
    }

    #[test]
    fn rich_plan_updates_states_and_spellings() {
        let t = synthetic_transcript(vec![
            event(1, "plan.updated", json!({"itemId":"p","plan":["Old"]})),
            event(
                2,
                "plan.updated",
                json!({"itemId":"p","plan":{"plan":[{"content":"Read","status":"completed"},{"step":"Check","status":"in_progress"},{"title":"Report","status":"pending"}]}}),
            ),
        ]);
        assert_eq!(t.items.len(), 1);
        let Item::Plan { steps, .. } = &t.items[0] else {
            panic!("plan");
        };
        assert_eq!(
            steps.iter().map(|s| s.state.as_str()).collect::<Vec<_>>(),
            ["done", "running", "pending"]
        );
        assert_eq!(steps[0].text, "Read");
    }

    #[test]
    fn rich_helper_nests_calls_by_parent_not_adjacency() {
        let t = synthetic_transcript(vec![
            event(
                1,
                "tool.started",
                json!({"itemId":"h","name":"Agent","status":"Explore","command":"Inspect fixture"}),
            ),
            event(
                2,
                "tool.started",
                json!({"itemId":"own","name":"Read","command":"own.txt"}),
            ),
            event(
                3,
                "tool.started",
                json!({"itemId":"h2","parentItemId":"h","name":"Agent"}),
            ),
            event(
                4,
                "tool.started",
                json!({"itemId":"c","parentItemId":"h2","name":"Read","command":"fixture.txt"}),
            ),
            event(
                5,
                "tool.completed",
                json!({"itemId":"c","parentItemId":"h2","name":"Read","output":"Synthetic contents"}),
            ),
            event(
                6,
                "tool.completed",
                json!({"itemId":"h","name":"Agent","output":"Synthetic report"}),
            ),
        ]);
        assert_eq!(t.items.len(), 2);
        let Item::Tool { detail: h, .. } = &t.items[0] else {
            panic!("helper");
        };
        assert!(h.helper);
        assert_eq!(h.helper_type, "Explore");
        assert_eq!(h.output, "Synthetic report");
        assert_eq!(h.children.len(), 1);
        let Item::Tool { detail: h2, .. } = &h.children[0] else {
            panic!("nested helper");
        };
        let Item::Tool {
            detail: c, status, ..
        } = &h2.children[0]
        else {
            panic!("child");
        };
        assert_eq!(c.output, "Synthetic contents");
        assert_eq!(*status, Status::Done);
    }

    #[test]
    fn rich_reasoning_uses_only_normalized_public_message_and_duration() {
        let t = synthetic_transcript(vec![
            event(
                1,
                "step.started",
                json!({"itemId":"r","stepKind":"item","itemType":"reasoning"}),
            ),
            event(
                2,
                "step.completed",
                json!({"itemId":"r","stepKind":"item","itemType":"reasoning","message":"Synthetic public summary. password=synthetic-value", "durationMs":12500}),
            ),
        ]);
        let Item::Thought {
            text, duration_ms, ..
        } = &t.items[0]
        else {
            panic!("thought");
        };
        assert_eq!(text, "Synthetic public summary. password=[redacted]");
        assert_eq!(*duration_ms, Some(12500));
        assert_eq!(duration_text(12500), "13s");
        assert_eq!(duration_text(61500), "1m 02s");
        assert!(scrub(&"😀".repeat(MAX_ASSISTANT)).ends_with("[truncated]"));
    }

    #[test]
    fn helper_background_end_updates_original_row() {
        let t = synthetic_transcript(vec![
            event(
                1,
                "tool.started",
                json!({"itemId":"h","name":"Agent","background":true}),
            ),
            event(2, "tool.completed", json!({"itemId":"h","name":"Agent"})),
            event(
                3,
                "step.completed",
                json!({"itemId":"h","stepKind":"item","itemType":"background","status":"completed","message":"Synthetic final report"}),
            ),
        ]);
        assert_eq!(t.items.len(), 1);
        let Item::Tool { detail, .. } = &t.items[0] else {
            panic!("helper");
        };
        assert_eq!(detail.background_ended, "completed");
        assert_eq!(detail.output, "Synthetic final report");
    }

    #[test]
    fn hidden_reasoning_never_becomes_display_text() {
        let rows = vec![event(
            1,
            "step.completed",
            json!({"itemId":"hidden","stepKind":"reasoning",
            "content":"SYNTHETIC_HIDDEN_CONTENT","raw":"SYNTHETIC_HIDDEN_RAW",
            "item":{"type":"reasoning","text":"SYNTHETIC_LEGACY_TEXT","summary":["SYNTHETIC_LEGACY_SUMMARY"]},
            "evidence":{"raw":{"item":{"type":"reasoning","content":"SYNTHETIC_EVIDENCE"}}}}),
        )];
        let t = synthetic_transcript(rows.clone());
        assert!(t.items.is_empty());
        let decoded = decode(rows[0].to_string().as_bytes()).unwrap().to_string();
        assert!(!decoded.contains("SYNTHETIC_"));
        // Legacy exec has no message: a duration-only row has no drawer text.
        let t = synthetic_transcript(vec![event(
            2,
            "step.completed",
            json!({"stepKind":"reasoning","durationMs":2000,"content":"SYNTHETIC_HIDDEN"}),
        )]);
        assert!(
            matches!(&t.items[0],Item::Thought {text,duration_ms:Some(2000),..} if text.is_empty())
        );
    }

    #[test]
    fn display_strings_and_json_arguments_follow_ts_secret_rules() {
        let t = synthetic_transcript(vec![
            json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":"sec","prompt":"Bearer eyJabc.eyJdef.ghi_signature_here","createdAt":"2026-01-01T00:00:00Z"}}),
            event(
                1,
                "adapter.diagnostic",
                json!({"level":"error","message":format!("token {}-{}", "sk", "abcdefghijklmnopqrstuvwxyz")}),
            ),
            event(
                2,
                "tool.started",
                json!({"itemId":"t","name":"Bash","arguments":{"passwd":"hunter2","password":"my secret phrase","path":"ok.txt"}}),
            ),
            event(
                3,
                "message.delta",
                json!({"itemId":"a","operation":"append","text":"AIzaSyAabcdefghijklmnopqrstuvwxyz012345","final":true}),
            ),
            event(
                4,
                "run.failed",
                json!({"message":"refresh failed password=visible-secret"}),
            ),
        ]);
        assert!(
            matches!(&t.items[0], Item::User(text) if text.contains("Bearer [redacted]") && !text.contains("eyJabc"))
        );
        assert!(
            matches!(&t.items[1], Item::Note { text, .. } if text.contains("[redacted]") && !text.contains(&format!("{}-abcdefghijklmnop", "sk")))
        );
        let Item::Tool { detail, .. } = &t.items[2] else {
            panic!("tool");
        };
        assert!(detail.arguments.contains("\"passwd\": \"[redacted]\""));
        assert!(detail.arguments.contains("\"password\": \"[redacted]\""));
        assert!(detail.arguments.contains("\"path\": \"ok.txt\""));
        assert!(!detail.arguments.contains("hunter2"));
        assert!(!detail.arguments.contains("my secret phrase"));
        assert!(
            matches!(&t.items[3], Item::Assistant { text, .. } if text.contains("[redacted]") && !text.contains("AIzaSyAabcdefghijklmnopqrstuvwxyz012345"))
        );
        assert!(
            matches!(t.items.last(), Some(Item::Note { text, .. }) if text.contains("password=[redacted]"))
        );
    }

    #[test]
    fn tool_keeps_title_when_completion_only_has_command() {
        let t = synthetic_transcript(vec![
            event(
                1,
                "tool.started",
                json!({"itemId":"t","name":"Bash","title":"Check fixture","command":"printf one"}),
            ),
            event(
                2,
                "tool.completed",
                json!({"itemId":"t","name":"Bash","command":"printf two","output":"two"}),
            ),
        ]);
        let Item::Tool {
            summary, detail, ..
        } = &t.items[0]
        else {
            panic!("tool");
        };
        assert_eq!(summary, "Check fixture");
        assert_eq!(detail.arguments, "printf two");
    }

    #[test]
    fn orphaned_parent_item_id_children_are_dropped() {
        let t = synthetic_transcript(vec![
            event(
                1,
                "tool.started",
                json!({"itemId":"own","name":"Read","command":"own.txt"}),
            ),
            event(
                2,
                "tool.started",
                json!({"itemId":"orphan","parentItemId":"missing-helper","name":"Read","command":"secret.txt"}),
            ),
            event(
                3,
                "tool.completed",
                json!({"itemId":"orphan","parentItemId":"missing-helper","name":"Read","output":"should not show"}),
            ),
        ]);
        assert_eq!(t.items.len(), 1);
        assert!(
            matches!(&t.items[0], Item::Tool { detail, .. } if detail.id == "own" && detail.children.is_empty())
        );
    }

    #[test]
    fn step_start_times_clear_so_reused_ids_do_not_inherit_duration() {
        let mut first_start = event(
            1,
            "step.started",
            json!({"itemId":"r","stepKind":"reasoning"}),
        );
        first_start["event"]["occurredAt"] = json!("2026-01-01T00:00:00Z");
        let mut first_end = event(
            2,
            "step.completed",
            json!({"itemId":"r","stepKind":"reasoning","message":"First"}),
        );
        first_end["event"]["occurredAt"] = json!("2026-01-01T00:00:05Z");
        let mut second_start = event(
            3,
            "step.started",
            json!({"itemId":"r","stepKind":"reasoning"}),
        );
        second_start["event"]["occurredAt"] = json!("2026-01-01T00:00:10Z");
        let mut second_end = event(
            4,
            "step.completed",
            json!({"itemId":"r","stepKind":"reasoning","message":"Second"}),
        );
        second_end["event"]["occurredAt"] = json!("2026-01-01T00:00:11Z");
        let t = synthetic_transcript(vec![first_start, first_end, second_start, second_end]);
        let thoughts: Vec<_> = t
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Thought {
                    text, duration_ms, ..
                } => Some((text.as_str(), *duration_ms)),
                _ => None,
            })
            .collect();
        assert_eq!(thoughts, [("First", Some(5000)), ("Second", Some(1000))]);
    }

    #[test]
    fn event_map_is_bounded_and_json_is_capped_before_pretty_print() {
        let mut rows = vec![
            json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":"bound","prompt":"Synthetic","createdAt":"2026-01-01T00:00:00Z"}}),
        ];
        for n in 0..(MAX_EVENTS as u64 + 25) {
            rows.push(event(
                n,
                "adapter.diagnostic",
                json!({"level":"warning","message":format!("n{n}")}),
            ));
        }
        let t = synthetic_transcript(rows);
        assert!(t.events.len() <= MAX_EVENTS);
        assert!(
            !t.items
                .iter()
                .any(|i| matches!(i, Item::Note { text, .. } if text == "n0"))
        );
        let deep = {
            let mut v = json!("leaf");
            for _ in 0..12 {
                v = json!([v]);
            }
            v
        };
        let wide: Value = (0..80)
            .map(|n| (format!("k{n}"), json!(n)))
            .collect::<serde_json::Map<_, _>>()
            .into();
        let rendered = display_value(&json!({"nest": deep, "wide": wide, "passwd": "hunter2"}));
        assert!(rendered.contains("[depth limit]") || rendered.contains("[redacted]"));
        assert!(rendered.contains("[redacted]"));
        assert!(!rendered.contains("hunter2"));
        assert!(rendered.len() < 50_000);
    }

    fn event(seq: u64, kind: &str, payload: Value) -> Value {
        json!({
            "schemaVersion":27,
            "recordType":"mission.event",
            "event":{
                "sequence":seq,
                "occurredAt":"2026-01-01T00:00:01Z",
                "type":kind,
                "payload":payload
            }
        })
    }

    fn delta(seq: u64, text: &str, operation: &str, final_: bool) -> Value {
        event(
            seq,
            "message.delta",
            json!({
                "itemId":"answer",
                "operation":operation,
                "text":text,
                "final":final_
            }),
        )
    }

    fn write(path: &Path, records: &[Value]) {
        let text = records.iter().map(|r| format!("{r}\n")).collect::<String>();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn a_partial_line_completes_on_the_next_poll() {
        let temp = Temp::new();
        let path = temp.path("partial");
        write(&path, &[created("partial", "Synthetic prompt")]);
        let line = format!("{}\n", delta(1, "Synthetic answer", "append", true));
        let middle = line.len() / 2;
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(&line.as_bytes()[..middle]).unwrap();

        let mut tail = Tail::new(&path, 0);
        assert_eq!(tail.poll().len(), 1);

        file.write_all(&line.as_bytes()[middle..]).unwrap();
        assert_eq!(tail.poll().len(), 1);
        assert!(tail.poll().is_empty());
    }

    #[test]
    fn bad_lines_and_two_megabyte_lines_are_skipped() {
        let temp = Temp::new();
        let path = temp.path("bad");
        write(&path, &[created("bad", "Synthetic prompt")]);
        let mut file = OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(b"not json\n").unwrap();
        file.write_all(&vec![b'x'; 2 * MAX_LINE]).unwrap();
        file.write_all(b"\n").unwrap();
        writeln!(file, "{}", delta(1, "kept", "append", true)).unwrap();

        let transcript = read_transcript(&path);
        assert_eq!(transcript.items.len(), 2);
        assert!(matches!(
            transcript.items.last(),
            Some(Item::Assistant { text, .. }) if text == "kept"
        ));
    }

    #[test]
    fn deltas_follow_sequence_and_replace_semantics_without_duplication() {
        let temp = Temp::new();
        let path = temp.path("order");
        write(
            &path,
            &[
                created("order", "Synthetic prompt"),
                delta(4, "B", "append", true),
                delta(2, "A", "replace", false),
                delta(1, "discarded", "append", false),
            ],
        );
        let mut transcript = read_transcript(&path);
        assert!(matches!(
            transcript.items.last(),
            Some(Item::Assistant { text, r#final: true, .. }) if text == "AB"
        ));
        transcript.apply(vec![delta(4, "B", "append", true)]);
        assert!(matches!(
            transcript.items.last(),
            Some(Item::Assistant { text, .. }) if text == "AB"
        ));
    }

    #[test]
    fn running_and_finished_summaries_use_fifteen_minute_window_and_restarts() {
        let temp = Temp::new();
        write(&temp.path("live"), &[created("live", "Live")]);
        write(
            &temp.path("done"),
            &[
                created("done", "Done"),
                event(1, "run.completed", json!({})),
            ],
        );
        write(
            &temp.path("failed"),
            &[
                created("failed", "Failed"),
                event(1, "run.failed", json!({"message":"Synthetic failure"})),
            ],
        );
        write(
            &temp.path("restarted"),
            &[
                created("restarted", "Restarted"),
                event(1, "run.failed", json!({"message":"First failed"})),
                event(2, "run.started", json!({})),
            ],
        );
        write(
            &temp.path("recovered"),
            &[
                created("recovered", "Recovered"),
                event(1, "run.failed", json!({"message":"First failed"})),
                event(2, "run.completed", json!({})),
            ],
        );
        let list = list_recent(&temp.0, 10);
        assert!(
            list.iter()
                .find(|r| r.mission_id == "live")
                .unwrap()
                .running
        );
        assert!(
            !list
                .iter()
                .find(|r| r.mission_id == "done")
                .unwrap()
                .running
        );
        assert!(
            list.iter()
                .find(|r| r.mission_id == "failed")
                .unwrap()
                .failed
        );
        let restarted = list.iter().find(|r| r.mission_id == "restarted").unwrap();
        assert!(restarted.running && !restarted.failed);
        let recovered = list.iter().find(|r| r.mission_id == "recovered").unwrap();
        assert!(!recovered.running && !recovered.failed);

        let restarted_tx = read_transcript(temp.path("restarted"));
        assert!(restarted_tx.running && !restarted_tx.failed);
        let recovered_tx = read_transcript(temp.path("recovered"));
        assert!(!recovered_tx.running && !recovered_tx.failed);

        assert!(fresh(
            Some(SystemTime::now() - Duration::from_secs(121)),
            SystemTime::now(),
        ));
        assert!(!fresh(
            Some(SystemTime::now() - Duration::from_secs(15 * 60 + 1)),
            SystemTime::now(),
        ));
    }

    #[test]
    fn recent_order_is_mtime_order_and_titles_are_bounded() {
        let temp = Temp::new();
        for (id, secs) in [("older", 10), ("newer", 20)] {
            let path = temp.path(id);
            write(&path, &[created(id, &"x".repeat(80))]);
            let file = OpenOptions::new().write(true).open(path).unwrap();
            file.set_times(
                fs::FileTimes::new()
                    .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(secs)),
            )
            .unwrap();
        }
        let list = list_recent(&temp.0, 1);
        assert_eq!(list.len(), 1);
        assert_eq!(list.first().unwrap().mission_id, "newer");
        assert_eq!(list.first().unwrap().title.chars().count(), 60);
    }

    #[test]
    fn truncation_resets_tail_and_assistant_text_is_bounded() {
        let temp = Temp::new();
        let path = temp.path("reset");
        write(
            &path,
            &[created("reset", "Old"), delta(1, "old", "replace", true)],
        );
        let mut tail = Tail::new(&path, 0);
        tail.poll();

        write(&path, &[created("reset", "New")]);
        assert_eq!(tail.poll().len(), 1);
        assert!(tail.reset);

        // Fix 1: an offset smaller than the retained marker length must reset cleanly without underflow.
        tail.offset = 1;
        let records = tail.poll();
        assert_eq!(records.len(), 1);
        assert!(tail.reset);

        // Fix 1: a transient open error after reset must not lose the ability to recover on the next poll.
        let missing = temp.path("missing");
        let mut err_tail = Tail::new(&missing, 500);
        assert!(err_tail.poll().is_empty());
        assert_eq!(err_tail.error, Some(io::ErrorKind::NotFound));
        write(&missing, &[created("missing", "Recovered")]);
        assert_eq!(err_tail.poll().len(), 1);
        assert!(err_tail.reset);

        write(
            &path,
            &[
                created("reset", "New"),
                delta(1, &"x".repeat(MAX_ASSISTANT + 10), "replace", true),
            ],
        );
        let transcript = read_transcript(&path);
        assert!(matches!(
            transcript.items.last(),
            Some(Item::Assistant { text, .. }) if text.len() == MAX_ASSISTANT
        ));
    }

    #[test]
    fn continuation_sessions_include_all_prompts_and_newest_answers() {
        let temp = Temp::new();
        write(
            &temp.path("root"),
            &[
                created("root", "First synthetic prompt"),
                delta(1, "First answer", "replace", true),
                event(2, "run.completed", json!({})),
            ],
        );
        let mut child = created("child", "Second synthetic prompt");
        child["metadata"]["continuesFrom"] = json!({"missionId":"root","reason":"follow-up"});
        write(
            &temp.path("child"),
            &[child, delta(1, "Second answer", "replace", true)],
        );

        let mut session = Session::new(temp.0.clone(), "root".into());
        let snapshot = session.poll().unwrap();
        assert_eq!(snapshot.tip, "child");
        assert_eq!(
            snapshot
                .items
                .iter()
                .filter(|i| matches!(i, Item::User(_)))
                .count(),
            2,
        );
        assert!(snapshot.running);
    }

    #[test]
    fn nonzero_offset_recovers_partial_line_and_sessions_drain_parent_and_follow_anchor_branch() {
        let temp = Temp::new();
        let partial_path = temp.path("offset_partial");
        write(&partial_path, &[created("offset_partial", "Prompt")]);
        let line = format!("{}\n", delta(1, "Completed after offset", "replace", true));
        let half = line.len() / 2;
        let mut file = OpenOptions::new().append(true).open(&partial_path).unwrap();
        file.write_all(&line.as_bytes()[..half]).unwrap();
        let start_offset = file.metadata().unwrap().len();

        // Fix 5 (Finding 7): starting a Tail at EOF of a half-written line completes that line on the next write.
        let mut tail = Tail::new(&partial_path, start_offset);
        assert!(tail.poll().is_empty());
        file.write_all(&line.as_bytes()[half..]).unwrap();
        let records = tail.poll();
        assert_eq!(records.len(), 1);

        // Fix 5: when a parent finishes in the same scan window a child appears, parent tail records are still drained.
        let branch_dir = Temp::new();
        write(&branch_dir.path("root"), &[created("root", "Root prompt")]);
        let mut session = Session::new(branch_dir.0.clone(), "root".into());
        assert_eq!(session.poll().unwrap().items.len(), 1);

        write(
            &branch_dir.path("root"),
            &[
                created("root", "Root prompt"),
                delta(1, "Root final answer", "replace", true),
                event(2, "run.completed", json!({})),
            ],
        );
        let mut branch_a = created("branch_a", "Branch A prompt");
        branch_a["metadata"]["continuesFrom"] = json!({"missionId":"root"});
        write(
            &branch_dir.path("branch_a"),
            &[
                branch_a,
                delta(1, "Branch A answer", "replace", true),
                event(2, "run.completed", json!({})),
            ],
        );
        let mut branch_b = created("branch_b", "Branch B prompt");
        branch_b["metadata"]["continuesFrom"] = json!({"missionId":"root"});
        write(
            &branch_dir.path("branch_b"),
            &[branch_b, delta(1, "Branch B answer", "replace", true)],
        );
        let file_a = OpenOptions::new()
            .write(true)
            .open(branch_dir.path("branch_a"))
            .unwrap();
        file_a
            .set_times(
                fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(30)),
            )
            .unwrap();
        let mut leaf_a = created("leaf_a", "Leaf A prompt");
        leaf_a["metadata"]["continuesFrom"] = json!({"missionId":"branch_a"});
        write(
            &branch_dir.path("leaf_a"),
            &[leaf_a, delta(1, "Leaf A answer", "replace", true)],
        );

        // Poll `session` (anchored at root, after advancing scan window) to verify root's final answer was drained.
        session.scanned = None;
        let root_snap = session.poll().unwrap();
        assert!(root_snap.items.iter().any(|i| matches!(
            i,
            Item::Assistant { text, .. } if text == "Root final answer"
        )));

        // Fix 5: opening `branch_a` must follow `root -> branch_a -> leaf_a`, not sibling `branch_b`.
        let mut anchored = Session::new(branch_dir.0.clone(), "branch_a".into());
        let snap = anchored.poll().unwrap();
        assert_eq!(snap.tip, "leaf_a");
        assert!(snap.items.iter().any(|i| matches!(
            i,
            Item::Assistant { text, .. } if text == "Branch A answer"
        )));
        assert!(snap.items.iter().any(|i| matches!(
            i,
            Item::Assistant { text, .. } if text == "Leaf A answer"
        )));
        assert!(!snap.items.iter().any(|i| matches!(
            i,
            Item::Assistant { text, .. } if text == "Branch B answer"
        )));
    }

    fn set_modified(path: &Path, ago: Duration) {
        let file = OpenOptions::new().write(true).open(path).unwrap();
        file.set_times(fs::FileTimes::new().set_modified(SystemTime::now() - ago))
            .unwrap();
    }

    #[test]
    fn a_quiet_tip_is_not_ended_until_the_ledger_says_so() {
        let temp = Temp::new();
        write(
            &temp.path("quiet"),
            &[
                created("quiet", "Prompt"),
                delta(1, "Partial", "append", false),
            ],
        );
        set_modified(&temp.path("quiet"), ACTIVE_WINDOW + Duration::from_secs(60));

        let mut session = Session::new(temp.0.clone(), "quiet".into());
        let snapshot = session.poll().unwrap();
        // Snapshot::running goes false after the 15-minute window; the run has not ended.
        assert!(!snapshot.running && !snapshot.failed);
        assert!(!session.tip_ended());
        assert!(session.tip_quiet(SystemTime::now()).unwrap() >= ACTIVE_WINDOW);

        write(
            &temp.path("quiet"),
            &[
                created("quiet", "Prompt"),
                delta(1, "Partial", "append", false),
                event(2, "run.completed", json!({})),
            ],
        );
        session.poll().unwrap();
        assert!(session.tip_ended());
        assert!(session.tip_quiet(SystemTime::now()).unwrap() < ACTIVE_WINDOW);

        // A failed run has ended too.
        write(
            &temp.path("failed"),
            &[
                created("failed", "Prompt"),
                event(1, "run.failed", json!({"message":"Synthetic failure"})),
            ],
        );
        let mut failed = Session::new(temp.0.clone(), "failed".into());
        assert!(failed.poll().unwrap().failed);
        assert!(failed.tip_ended());
    }

    #[test]
    fn set_rescan_defers_the_folder_scan_of_a_running_tip() {
        let temp = Temp::new();
        write(&temp.path("root"), &[created("root", "Root prompt")]);
        let mut session = Session::new(temp.0.clone(), "root".into());
        assert_eq!(session.rescan(), RESCAN);
        session.set_rescan(Duration::from_secs(3600));
        assert_eq!(session.poll().unwrap().tip, "root");

        let mut child = created("child", "Follow-up");
        child["metadata"]["continuesFrom"] = json!({"missionId":"root"});
        write(&temp.path("child"), &[child]);
        // Within the interval the running tip is not re-listed, so the child is unseen.
        assert_eq!(session.poll().unwrap().tip, "root");
        session.set_rescan(Duration::ZERO);
        assert_eq!(session.poll().unwrap().tip, "child");
    }

    #[test]
    fn dir_listed_tells_a_missing_folder_from_a_missing_file() {
        let temp = Temp::new();
        let mut nowhere = Session::new(temp.0.join("nope"), "root".into());
        assert!(nowhere.poll().is_err());
        assert!(!nowhere.dir_listed());

        let mut absent = Session::new(temp.0.clone(), "root".into());
        assert_eq!(absent.poll().unwrap_err().kind(), io::ErrorKind::NotFound);
        assert!(absent.dir_listed());
    }

    /// Reads every REAL ledger on this computer and prints only counts, never text. Run by hand:
    /// `cargo test --release real_ledgers -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_ledgers_parse_without_printing_text() {
        assert_eq!(
            std::env::var("LOCUST_REAL_LEDGERS").ok().as_deref(),
            Some("1"),
            "set LOCUST_REAL_LEDGERS=1 to run this real test"
        );
        let root = crate::engine::default_data_folder()
            .expect("Real ledger test requires a data folder")
            .join("mission-ledger");
        let started = Instant::now();
        let (
            mut files,
            mut no_meta,
            mut errors,
            mut users,
            mut assistants,
            mut tools,
            mut plans,
            mut notes,
        ) = (0, 0, 0, 0, 0, 0, 0, 0);
        let (mut biggest, mut running) = (0usize, 0usize);
        for entry in fs::read_dir(&root).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            files += 1;
            let t = read_transcript(&path);
            if t.metadata.is_none() {
                no_meta += 1;
            }
            if t.error.is_some() {
                errors += 1;
            }
            if t.running {
                running += 1;
            }
            for item in &t.items {
                match item {
                    Item::User(_) => users += 1,
                    Item::Assistant { text, .. } => {
                        assistants += 1;
                        biggest = biggest.max(text.chars().count());
                    }
                    Item::Tool { .. } => tools += 1,
                    Item::Plan { .. } => plans += 1,
                    Item::Thought { .. } => notes += 1,
                    Item::Note { .. } => notes += 1,
                    Item::CarryOn { .. } => {}
                }
            }
        }
        let recent = list_recent(&root, 50);
        eprintln!("files {files}, no metadata {no_meta}, read errors {errors}, running {running}");
        eprintln!(
            "items: user {users}, assistant {assistants}, tool {tools}, plan {plans}, note {notes}; longest answer {biggest} chars"
        );
        eprintln!(
            "recent list: {} rows, {} with a title, took {:?}",
            recent.len(),
            recent.iter().filter(|r| r.title != "Untitled").count(),
            started.elapsed()
        );
        assert!(biggest <= MAX_ASSISTANT);
    }
}

#[cfg(test)]
mod carry_tests {
    use super::*;
    use serde_json::json;
    fn transcript(events: Vec<(&str, Value)>) -> Transcript {
        let mut t = Transcript::default();
        let mut records = vec![
            json!({"recordType":"mission.created","metadata":{"missionId":"synthetic-turn","prompt":"Synthetic prompt"}}),
        ];
        records.extend(events.into_iter().enumerate().map(|(i, (kind, payload))| json!({"recordType":"mission.event","event":{"sequence": i, "type":kind,"payload":payload}})));
        t.apply(records);
        t
    }
    fn has_card(t: &Transcript) -> bool {
        t.items.iter().any(|i| matches!(i, Item::CarryOn { .. }))
    }
    #[test]
    fn carry_card_requires_limit_or_provider_failure_and_clears_when_finished_or_carried_on() {
        assert!(has_card(&transcript(vec![(
            "route.limit_detected",
            json!({})
        )])));
        assert!(has_card(&transcript(vec![(
            "run.failed",
            json!({"detail":"Plan limit reached"})
        )])));
        assert!(has_card(&transcript(vec![(
            "run.failed",
            json!({"detail":"Provider unavailable"})
        )])));
        assert!(!has_card(&transcript(vec![(
            "run.failed",
            json!({"detail":"Invalid folder"})
        )])));
        assert!(!has_card(&transcript(vec![("run.cancelled", json!({}))])));
        assert!(!has_card(&transcript(vec![
            ("route.limit_detected", json!({})),
            ("run.completed", json!({}))
        ])));
        assert!(!has_card(&transcript(vec![
            ("run.completed", json!({})),
            ("route.limit_detected", json!({}))
        ])));
        assert!(!has_card(&transcript(vec![
            ("run.cancelled", json!({})),
            ("route.limit_detected", json!({}))
        ])));
        assert!(!has_card(&transcript(vec![
            ("route.limit_detected", json!({})),
            ("run.started", json!({}))
        ])));

        // Non-object event or payload in direct Transcript::apply never panics.
        let mut malformed = Transcript::default();
        malformed.apply(vec![
            json!({"recordType":"mission.created","metadata":{"missionId":"safe","prompt":"Prompt"}}),
            json!({"recordType":"mission.event","event":["not-an-object"]}),
            json!({"recordType":"mission.event","event":{"sequence":1,"type":"message.delta","payload":"not-an-object"}}),
        ]);
        assert_eq!(malformed.items, vec![Item::User("Prompt".into())]);

        // Once a stopped turn is carried on into a child mission file, the parent's CarryOn card is hidden.
        let temp = tests::Temp::new();
        let parent_path = temp.0.join("turn_1.jsonl");
        let child_path = temp.0.join("turn_2.jsonl");
        std::fs::write(&parent_path, format!(
            "{}\n{}\n",
            json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":"turn_1","prompt":"First","runtime":"claude","model":"opus","createdAt":"2026-01-01T00:00:00Z"}}),
            json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":1,"occurredAt":"2026-01-01T00:00:01Z","type":"route.limit_detected","payload":{"message":"Plan limit reached"}}}),
        )).unwrap();
        let mut session = Session::new(temp.0.clone(), "turn_1".into());
        let first = session.poll().unwrap();
        assert!(first.items.iter().any(
            |i| matches!(i, Item::CarryOn { conversation_id } if conversation_id == "turn_1")
        ));
        assert!(!list_recent(&temp.0, 5)[0].running);

        std::fs::write(&child_path, format!(
            "{}\n{}\n",
            json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":"turn_2","prompt":"Carry on","runtime":"codex","model":"gpt-6.1-sol","createdAt":"2026-01-01T00:00:02Z","continuesFrom":{"missionId":"turn_1","reason":"carry-on"}}}),
            json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":1,"occurredAt":"2026-01-01T00:00:03Z","type":"message.delta","payload":{"itemId":"m2","operation":"replace","text":"Continued answer","final":true}}}),
        )).unwrap();
        let second = session.poll().unwrap();
        assert_eq!(second.tip, "turn_2");
        assert!(
            !second
                .items
                .iter()
                .any(|i| matches!(i, Item::CarryOn { .. })),
            "carried-on turn must not keep its CarryOn card"
        );
    }
    #[test]
    fn cancelled_turn_says_stopped() {
        let t = transcript(vec![("run.cancelled", json!({}))]);
        assert!(
            t.items
                .iter()
                .any(|i| matches!(i, Item::Note { text, .. } if text == "Stopped"))
        );
    }
}

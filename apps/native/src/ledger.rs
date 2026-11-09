//! Read-only readers for Locust's append-only mission ledgers.
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

pub const MAX_LINE: usize = 1024 * 1024;
pub const MAX_ASSISTANT: usize = 131_072;
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
    CarryOn { conversation_id: String },
    Assistant {
        item_id: String,
        text: String,
        r#final: bool,
    },
    Tool {
        name: String,
        status: Status,
        summary: String,
    },
    Plan(String),
    Note {
        level: String,
        text: String,
    },
}

fn string(v: &Value, key: &str) -> String {
    v.get(key).and_then(Value::as_str).unwrap_or_default().to_owned()
}

pub fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn metadata(record: &Record) -> Option<Metadata> {
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
        parent: m.get("continuesFrom")
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
    text.split_whitespace().collect::<Vec<_>>()
        .join(" ").chars().take(240).collect()
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
            if let Some(payload) = event.get_mut("payload")
                .and_then(Value::as_object_mut)
            {
                payload.remove("evidence");
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
                if !self.skipping {
                    if let Some(record) = decode(&self.pending) {
                        records.push(record);
                    }
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
}

impl Transcript {
    pub fn apply(&mut self, records: Vec<Record>) {
        if records.is_empty() {
            return;
        }
        for record in records {
            if let Some(m) = metadata(&record) {
                self.metadata = Some(m);
            } else if let Some(event) = record.get("event") {
                if let Some(sequence) = event.get("sequence").and_then(Value::as_u64) {
                    self.events.insert(sequence, event.clone());
                }
            }
        }

        let mut items = Vec::new();
        if let Some(m) = &self.metadata {
            items.push(Item::User(m.prompt.clone()));
        }
        // Fix 8: store (item_index, utf16_units) so streaming deltas count UTF-16 units incrementally.
        let mut messages = HashMap::<String, (usize, usize)>::new();
        let mut tools = HashMap::<String, usize>::new();
        let mut plans = HashMap::<String, usize>::new();
        self.running = self.metadata.is_some();
        self.failed = false;

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
                    if let Some(Item::Assistant { text, r#final, .. }) =
                        items.get_mut(*at)
                    {
                        let piece = string(p, "text");
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
                        *r#final = p.get("final")
                            .and_then(Value::as_bool).unwrap_or(false);
                    }
                }
                "tool.started" | "tool.completed" | "tool.failed"
                | "step.started" | "step.completed" | "step.failed" => {
                    let step = kind.starts_with("step.");
                    let key = format!(
                        "{}:{}",
                        if step { "step" } else { "tool" },
                        if id.is_empty() { sequence.to_string() } else { id },
                    );
                    let at = *tools.entry(key).or_insert_with(|| {
                        items.push(Item::Tool {
                            name: String::new(),
                            status: Status::Running,
                            summary: String::new(),
                        });
                        items.len() - 1
                    });
                    if let Some(Item::Tool { name, status, summary }) =
                        items.get_mut(at)
                    {
                        let candidate = if step {
                            string(p, "stepKind")
                        } else {
                            string(p, "name")
                        };
                        if !candidate.is_empty() {
                            *name = candidate;
                        }
                        *status = if kind.ends_with("failed") {
                            Status::Failed
                        } else if kind.ends_with("completed") {
                            Status::Done
                        } else {
                            Status::Running
                        };
                        let detail = ["title", "command", "message", "output"]
                            .iter().map(|key| string(p, key))
                            .find(|v| !v.trim().is_empty());
                        if let Some(detail) = detail {
                            *summary = one_line(&detail);
                        }
                    }
                }
                "plan.updated" => {
                    let text = match p.get("plan") {
                        Some(Value::String(text)) => text.clone(),
                        Some(Value::Array(rows)) => rows.iter().map(|row| {
                            let step = row.get("step")
                                .or_else(|| row.get("content"))
                                .and_then(Value::as_str).unwrap_or_default();
                            let status = string(row, "status");
                            if status.is_empty() {
                                step.to_owned()
                            } else {
                                format!("{status}: {step}")
                            }
                        }).collect::<Vec<_>>().join("\n"),
                        _ => String::new(),
                    };
                    let key = if id.is_empty() { "plan".into() } else { id };
                    let at = *plans.entry(key).or_insert_with(|| {
                        items.push(Item::Plan(String::new()));
                        items.len() - 1
                    });
                    if let Some(item) = items.get_mut(at) {
                        *item = Item::Plan(text);
                    }
                }
                "adapter.diagnostic" => {
                    let level = string(p, "level");
                    if level == "warning" || level == "error" {
                        items.push(Item::Note {
                            level,
                            text: string(p, "message"),
                        });
                    }
                }
                "route.limit_detected" => {
                    if !finished {
                        carry = true;
                        self.running = false;
                    }
                    let text = string(p, "message");
                    if !text.is_empty() {
                        items.push(Item::Note { level: "warning".into(), text });
                    }
                },
                "run.failed" => {
                    self.running = false;
                    self.failed = true;
                    finished = false;
                    carry |= carry_on_failure(p);
                    let text = string(p, "message");
                    if !text.is_empty() {
                        items.push(Item::Note { level: "error".into(), text });
                    }
                }
                // Fix 4: a later completion or restart clears an earlier completed/failed flag.
                "run.completed" | "run.cancelled" => {
                    carry = false;
                    finished = true;
                    if kind == "run.cancelled" { items.push(Item::Note { level: String::new(), text: "Stopped".into() }); }
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
        if carry && !self.running && !finished && let Some(m) = &self.metadata {
            items.push(Item::CarryOn { conversation_id: m.mission_id.clone() });
        }
        self.items = items;
    }
}

fn carry_on_failure(payload: &Value) -> bool {
    let detail = ["detail", "message", "reason", "error"].iter()
        .filter_map(|key| payload.get(key)).map(Value::to_string).collect::<Vec<_>>().join(" ").to_ascii_lowercase();
    detail.contains("limit") || detail.contains("provider")
}

// Standalone snapshot API; the window uses Session to keep its byte offsets.
#[allow(dead_code)]
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
    modified.is_some_and(|m| {
        now.duration_since(m).unwrap_or_default() < ACTIVE_WINDOW
    })
}

// Convenience API when an unreadable directory need not be distinguished from an empty one.
#[allow(dead_code)]
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
        let Some(m) = records.iter().find_map(metadata) else { continue };
        // Fix 4: at most HEAD + TAIL bytes; skip end_after entirely when head_with_offset already read the whole file.
        if read_bytes < len {
            if let Ok(tail) = end_after(&path, read_bytes, lines) {
                records.extend(tail);
            }
        }
        // Fix 4: replay run lifecycle events in sequence order so a later run.started clears earlier completed/failed state.
        let mut events = BTreeMap::new();
        for record in &records {
            if let Some(event) = record.get("event") {
                if let Some(seq) = event.get("sequence").and_then(Value::as_u64) {
                    events.insert(seq, event);
                }
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
        let updated = events.values().rev()
            .find_map(|e| e.get("occurredAt").and_then(Value::as_str))
            .unwrap_or(&m.created_at).to_owned();
        let title: String = m.prompt.lines().next().unwrap_or_default()
            .trim().chars().take(60).collect();
        summaries.push(Summary {
            mission_id: m.mission_id,
            title: if title.is_empty() { "Untitled".into() } else { title },
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
    pub items: Vec<Item>,
    pub tip: String,
    pub running: bool,
    pub failed: bool,
}

/// A follow-up is a NEW mission file although MCP returns the original conversation ID.
pub struct Session {
    dir: PathBuf,
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
            anchor: mission_id,
            readers: Vec::new(),
            scanned: None,
            headers: HashMap::new(),
            rescan: RESCAN,
            listed: false,
        }
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
        self.readers.last().is_some_and(|r| {
            r.transcript.metadata.is_some() && !r.transcript.running
        })
    }

    /// How long the newest file has gone without being written, as of `now`.
    pub fn tip_quiet(&self, now: SystemTime) -> Option<Duration> {
        let modified = self.readers.last()?.tail.modified?;
        Some(now.duration_since(modified).unwrap_or_default())
    }

    pub fn anchor(&self) -> &str {
        &self.anchor
    }

    #[cfg(test)]
    pub fn cached_headers(&self) -> usize {
        self.headers.len()
    }

    #[cfg(test)]
    pub fn rescan(&self) -> Duration {
        self.rescan
    }

    pub fn poll(&mut self) -> io::Result<Snapshot> {
        if !safe_id(&self.anchor) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        let tip_finished = self.readers.last()
            .is_none_or(|r| !r.transcript.running && r.tail.offset > 0);
        if self.scanned.is_none_or(|at| at.elapsed() >= self.rescan)
            || tip_finished
        {
            let paths = self.chain()?;
            let mut old: HashMap<_, _> = self.readers.drain(..)
                .map(|r| (r.id.clone(), r)).collect();
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
                reader.transcript.items.iter()
                    .filter(|item| is_tip || !matches!(item, Item::CarryOn { .. }))
                    .cloned(),
            );
            snapshot.tip = reader.id.clone();
            snapshot.running = reader.transcript.running
                && fresh(reader.tail.modified, SystemTime::now());
            snapshot.failed = reader.transcript.failed;
        }
        Ok(snapshot)
    }

    fn chain(&mut self) -> io::Result<Vec<(String, PathBuf)>> {
        let mut all = HashMap::new();
        self.listed = false;
        let entries = fs::read_dir(&self.dir)?;
        self.listed = true;
        // Fix 3: skip unreadable directory entries instead of failing the whole session.
        for entry in entries {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_ok_and(|t| t.is_file())
                || entry.path().extension().is_none_or(|e| e != "jsonl")
            {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let path = entry.path();
            let len = meta.len();
            let refresh = self.headers.get(&path)
                .is_none_or(|(cached_len, _)| len < *cached_len);
            if refresh {
                if let Ok(records) = head(&path) {
                    if let Some(m) = records.iter().find_map(metadata) {
                        self.headers.insert(path.clone(), (len, m));
                    } else {
                        self.headers.remove(&path);
                    }
                }
            }
            if let Some((_, m)) = self.headers.get(&path).cloned() {
                all.insert(m.mission_id.clone(), (
                    m,
                    path,
                    meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                ));
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
            let Some((m, path, _)) = all.get(&current) else { break };
            result.push((current.clone(), path.clone()));
            let Some(parent) = m.parent.as_ref() else { break };
            if !all.contains_key(parent) || seen.contains(parent) {
                break;
            }
            current = parent.clone();
        }
        result.reverse();

        let mut tip = self.anchor.clone();
        while let Some(next) = all.values()
            .filter(|v| {
                v.0.parent.as_deref() == Some(tip.as_str())
                    && !seen.contains(&v.0.mission_id)
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
                SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap().as_nanos(),
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
        event(seq, "message.delta", json!({
            "itemId":"answer",
            "operation":operation,
            "text":text,
            "final":final_
        }))
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
        write(&path, &[
            created("order", "Synthetic prompt"),
            delta(4, "B", "append", true),
            delta(2, "A", "replace", false),
            delta(1, "discarded", "append", false),
        ]);
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
        write(&temp.path("done"), &[
            created("done", "Done"),
            event(1, "run.completed", json!({})),
        ]);
        write(&temp.path("failed"), &[
            created("failed", "Failed"),
            event(1, "run.failed", json!({"message":"Synthetic failure"})),
        ]);
        write(&temp.path("restarted"), &[
            created("restarted", "Restarted"),
            event(1, "run.failed", json!({"message":"First failed"})),
            event(2, "run.started", json!({})),
        ]);
        write(&temp.path("recovered"), &[
            created("recovered", "Recovered"),
            event(1, "run.failed", json!({"message":"First failed"})),
            event(2, "run.completed", json!({})),
        ]);
        let list = list_recent(&temp.0, 10);
        assert!(list.iter().find(|r| r.mission_id == "live").unwrap().running);
        assert!(!list.iter().find(|r| r.mission_id == "done").unwrap().running);
        assert!(list.iter().find(|r| r.mission_id == "failed").unwrap().failed);
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
            file.set_times(fs::FileTimes::new().set_modified(
                SystemTime::UNIX_EPOCH + Duration::from_secs(secs),
            )).unwrap();
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
        write(&path, &[
            created("reset", "Old"),
            delta(1, "old", "replace", true),
        ]);
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

        write(&path, &[
            created("reset", "New"),
            delta(1, &"x".repeat(MAX_ASSISTANT + 10), "replace", true),
        ]);
        let transcript = read_transcript(&path);
        assert!(matches!(
            transcript.items.last(),
            Some(Item::Assistant { text, .. }) if text.len() == MAX_ASSISTANT
        ));
    }

    #[test]
    fn continuation_sessions_include_all_prompts_and_newest_answers() {
        let temp = Temp::new();
        write(&temp.path("root"), &[
            created("root", "First synthetic prompt"),
            delta(1, "First answer", "replace", true),
            event(2, "run.completed", json!({})),
        ]);
        let mut child = created("child", "Second synthetic prompt");
        child["metadata"]["continuesFrom"] =
            json!({"missionId":"root","reason":"follow-up"});
        write(&temp.path("child"), &[
            child,
            delta(1, "Second answer", "replace", true),
        ]);

        let mut session = Session::new(temp.0.clone(), "root".into());
        let snapshot = session.poll().unwrap();
        assert_eq!(snapshot.tip, "child");
        assert_eq!(
            snapshot.items.iter().filter(|i| matches!(i, Item::User(_))).count(),
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

        write(&branch_dir.path("root"), &[
            created("root", "Root prompt"),
            delta(1, "Root final answer", "replace", true),
            event(2, "run.completed", json!({})),
        ]);
        let mut branch_a = created("branch_a", "Branch A prompt");
        branch_a["metadata"]["continuesFrom"] = json!({"missionId":"root"});
        write(&branch_dir.path("branch_a"), &[
            branch_a,
            delta(1, "Branch A answer", "replace", true),
            event(2, "run.completed", json!({})),
        ]);
        let mut branch_b = created("branch_b", "Branch B prompt");
        branch_b["metadata"]["continuesFrom"] = json!({"missionId":"root"});
        write(&branch_dir.path("branch_b"), &[
            branch_b,
            delta(1, "Branch B answer", "replace", true),
        ]);
        let file_a = OpenOptions::new().write(true).open(branch_dir.path("branch_a")).unwrap();
        file_a.set_times(fs::FileTimes::new().set_modified(
            SystemTime::now() - Duration::from_secs(30),
        )).unwrap();
        let mut leaf_a = created("leaf_a", "Leaf A prompt");
        leaf_a["metadata"]["continuesFrom"] = json!({"missionId":"branch_a"});
        write(&branch_dir.path("leaf_a"), &[
            leaf_a,
            delta(1, "Leaf A answer", "replace", true),
        ]);

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
        write(&temp.path("quiet"), &[
            created("quiet", "Prompt"),
            delta(1, "Partial", "append", false),
        ]);
        set_modified(&temp.path("quiet"), ACTIVE_WINDOW + Duration::from_secs(60));

        let mut session = Session::new(temp.0.clone(), "quiet".into());
        let snapshot = session.poll().unwrap();
        // Snapshot::running goes false after the 15-minute window; the run has not ended.
        assert!(!snapshot.running && !snapshot.failed);
        assert!(!session.tip_ended());
        assert!(session.tip_quiet(SystemTime::now()).unwrap() >= ACTIVE_WINDOW);

        write(&temp.path("quiet"), &[
            created("quiet", "Prompt"),
            delta(1, "Partial", "append", false),
            event(2, "run.completed", json!({})),
        ]);
        session.poll().unwrap();
        assert!(session.tip_ended());
        assert!(session.tip_quiet(SystemTime::now()).unwrap() < ACTIVE_WINDOW);

        // A failed run has ended too.
        write(&temp.path("failed"), &[
            created("failed", "Prompt"),
            event(1, "run.failed", json!({"message":"Synthetic failure"})),
        ]);
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
        let Some(root) = crate::engine::default_data_folder().map(|f| f.join("mission-ledger")) else { return };
        let started = Instant::now();
        let (mut files, mut no_meta, mut errors, mut users, mut assistants, mut tools, mut plans, mut notes) = (0, 0, 0, 0, 0, 0, 0, 0);
        let (mut biggest, mut running) = (0usize, 0usize);
        for entry in fs::read_dir(&root).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") { continue; }
            files += 1;
            let t = read_transcript(&path);
            if t.metadata.is_none() { no_meta += 1; }
            if t.error.is_some() { errors += 1; }
            if t.running { running += 1; }
            for item in &t.items {
                match item {
                    Item::User(_) => users += 1,
                    Item::Assistant { text, .. } => { assistants += 1; biggest = biggest.max(text.chars().count()); }
                    Item::Tool { .. } => tools += 1,
                    Item::Plan(_) => plans += 1,
                    Item::Note { .. } => notes += 1,
                    Item::CarryOn { .. } => {},
                }
            }
        }
        let recent = list_recent(&root, 50);
        eprintln!("files {files}, no metadata {no_meta}, read errors {errors}, running {running}");
        eprintln!("items: user {users}, assistant {assistants}, tool {tools}, plan {plans}, note {notes}; longest answer {biggest} chars");
        eprintln!("recent list: {} rows, {} with a title, took {:?}", recent.len(), recent.iter().filter(|r| r.title != "Untitled").count(), started.elapsed());
        assert!(biggest <= MAX_ASSISTANT);
    }
}

#[cfg(test)]
mod carry_tests {
    use super::*;
    use serde_json::json;
    fn transcript(events: Vec<(&str, Value)>) -> Transcript {
        let mut t = Transcript::default();
        let mut records = vec![json!({"recordType":"mission.created","metadata":{"missionId":"synthetic-turn","prompt":"Synthetic prompt"}})];
        records.extend(events.into_iter().enumerate().map(|(i, (kind, payload))| json!({"recordType":"mission.event","event":{"sequence": i, "type":kind,"payload":payload}})));
        t.apply(records);
        t
    }
    fn has_card(t: &Transcript) -> bool { t.items.iter().any(|i| matches!(i, Item::CarryOn { .. })) }
    #[test]
    fn carry_card_requires_limit_or_provider_failure_and_clears_when_finished_or_carried_on() {
        assert!(has_card(&transcript(vec![("route.limit_detected", json!({}))])));
        assert!(has_card(&transcript(vec![("run.failed", json!({"detail":"Plan limit reached"}))])));
        assert!(has_card(&transcript(vec![("run.failed", json!({"detail":"Provider unavailable"}))])));
        assert!(!has_card(&transcript(vec![("run.failed", json!({"detail":"Invalid folder"}))])));
        assert!(!has_card(&transcript(vec![("run.cancelled", json!({}))])));
        assert!(!has_card(&transcript(vec![("route.limit_detected", json!({})), ("run.completed", json!({}))])));
        assert!(!has_card(&transcript(vec![("run.completed", json!({})), ("route.limit_detected", json!({}))])));
        assert!(!has_card(&transcript(vec![("run.cancelled", json!({})), ("route.limit_detected", json!({}))])));
        assert!(!has_card(&transcript(vec![("route.limit_detected", json!({})), ("run.started", json!({}))])));

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
        assert!(first.items.iter().any(|i| matches!(i, Item::CarryOn { conversation_id } if conversation_id == "turn_1")));
        assert!(!list_recent(&temp.0, 5)[0].running);

        std::fs::write(&child_path, format!(
            "{}\n{}\n",
            json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":"turn_2","prompt":"Carry on","runtime":"codex","model":"gpt-6.1-sol","createdAt":"2026-01-01T00:00:02Z","continuesFrom":{"missionId":"turn_1","reason":"carry-on"}}}),
            json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":1,"occurredAt":"2026-01-01T00:00:03Z","type":"message.delta","payload":{"itemId":"m2","operation":"replace","text":"Continued answer","final":true}}}),
        )).unwrap();
        let second = session.poll().unwrap();
        assert_eq!(second.tip, "turn_2");
        assert!(!second.items.iter().any(|i| matches!(i, Item::CarryOn { .. })), "carried-on turn must not keep its CarryOn card");
    }
    #[test]
    fn cancelled_turn_says_stopped() {
        let t = transcript(vec![("run.cancelled", json!({}))]);
        assert!(t.items.iter().any(|i| matches!(i, Item::Note { text, .. } if text == "Stopped")));
    }
}

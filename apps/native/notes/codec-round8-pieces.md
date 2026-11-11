The draft below adds bounded, read-only ledger access and main-window streaming. Follow-up mission files are joined through `continuesFrom`.

Syntax, type, and borrow checks passed, including seven synthetic ledger tests. Tests, snapshots, and live tailing were not executed. No files changed.

Ledger rows use runtime labels rather than guessing teammate identities. MCP still resolves the owner when continuing a conversation.

## New `src/ledger.rs`

```rust
//! Read-only readers for Locust's append-only mission ledgers.
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

pub const MAX_LINE: usize = 1024 * 1024;
pub const MAX_ASSISTANT: usize = 131_072;
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

// Match Locust's JavaScript text limit in UTF-16 units without splitting a scalar.
fn capped(text: &str, limit: usize) -> String {
    let mut used = 0;
    text.chars().take_while(|c| {
        used += c.len_utf16();
        used <= limit
    }).collect()
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

#[derive(Default)]
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
        self.reset = false;
        self.error = None;
        match self.read_new() {
            Ok(records) => records,
            Err(error) => {
                self.error = Some(error.kind());
                Vec::new()
            }
        }
    }

    fn read_new(&mut self) -> io::Result<Vec<Record>> {
        // Production code never requests write, append, truncate, or create access.
        let mut file = OpenOptions::new().read(true).open(&self.path)?;
        let meta = file.metadata()?;
        self.modified = meta.modified().ok();

        let mut reset = meta.len() < self.offset;
        if !reset && !self.marker.is_empty() {
            file.seek(SeekFrom::Start(
                self.offset - self.marker.len() as u64,
            ))?;
            let mut actual = vec![0; self.marker.len()];
            reset = file.read_exact(&mut actual).is_err() || actual != self.marker;
        }
        if reset {
            // The marker helps detect truncation followed by regrowth between polls.
            self.offset = 0;
            self.lines = Lines::default();
            self.reset = true;
        }

        if self.first && self.offset > 0 && !reset {
            file.seek(SeekFrom::Start(self.offset - 1))?;
            let mut byte = [0];
            file.read_exact(&mut byte)?;
            self.lines.skipping = byte[0] != b'\n';
        }
        self.first = false;
        file.seek(SeekFrom::Start(self.offset))?;

        // Snapshot EOF so an active writer cannot keep this worker reading forever.
        let mut left = meta.len().saturating_sub(self.offset);
        let mut buffer = [0; 8192];
        let mut records = Vec::new();
        while left > 0 {
            let wanted = left.min(buffer.len() as u64) as usize;
            let n = file.read(&mut buffer[..wanted])?;
            if n == 0 {
                break;
            }
            self.lines.feed(&buffer[..n], &mut records);
            self.offset += n as u64;
            left -= n as u64;
        }

        let count = self.offset.min(64) as usize;
        file.seek(SeekFrom::Start(self.offset - count as u64))?;
        self.marker = vec![0; count];
        file.read_exact(&mut self.marker)?;
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
        let mut messages = HashMap::<String, usize>::new();
        let mut tools = HashMap::<String, usize>::new();
        let mut plans = HashMap::<String, usize>::new();
        self.running = self.metadata.is_some();
        self.failed = false;

        for (sequence, event) in &self.events {
            let kind = string(event, "type");
            let p = &event["payload"];
            let id = string(p, "itemId");
            match kind.as_str() {
                "message.delta" if !id.is_empty() => {
                    let at = *messages.entry(id.clone()).or_insert_with(|| {
                        items.push(Item::Assistant {
                            item_id: id,
                            text: String::new(),
                            r#final: false,
                        });
                        items.len() - 1
                    });
                    if let Some(Item::Assistant { text, r#final, .. }) =
                        items.get_mut(at)
                    {
                        let piece = string(p, "text");
                        if string(p, "operation") == "replace" {
                            *text = capped(&piece, MAX_ASSISTANT);
                        } else {
                            let left = MAX_ASSISTANT
                                .saturating_sub(text.encode_utf16().count());
                            text.push_str(&capped(&piece, left));
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
                "route.limit_detected" => items.push(Item::Note {
                    level: "warning".into(),
                    text: string(p, "message"),
                }),
                "run.failed" => {
                    self.running = false;
                    self.failed = true;
                    let text = string(p, "message");
                    if !text.is_empty() {
                        items.push(Item::Note { level: "error".into(), text });
                    }
                }
                "run.completed" | "run.cancelled" => self.running = false,
                "run.started" => {
                    self.running = true;
                    self.failed = false;
                }
                _ => {}
            }
        }
        self.items = items;
    }
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

fn head(path: &Path) -> io::Result<Vec<Record>> {
    let mut file = OpenOptions::new().read(true).open(path)?.take(HEAD);
    let mut lines = Lines::default();
    let mut records = Vec::new();
    let mut bytes = [0; 8192];
    loop {
        let n = file.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        lines.feed(&bytes[..n], &mut records);
        if records.iter().any(|r| metadata(r).is_some()) {
            break;
        }
    }
    Ok(records)
}

fn end(path: &Path) -> io::Result<Vec<Record>> {
    let mut file = OpenOptions::new().read(true).open(path)?;
    let start = file.metadata()?.len().saturating_sub(TAIL);
    let mut lines = Lines::default();
    if start > 0 {
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
        now.duration_since(m).unwrap_or_default() < Duration::from_secs(120)
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
    for (modified, path, _) in paths {
        if summaries.len() == limit {
            break;
        }
        let Ok(mut records) = head(&path) else { continue };
        let Some(m) = records.iter().find_map(metadata) else { continue };
        // At most HEAD + TAIL bytes; never scan the middle of a large ledger.
        if let Ok(tail) = end(&path) {
            records.extend(tail);
        }
        let finished = records.iter().any(|r| matches!(
            r["event"]["type"].as_str(),
            Some("run.completed" | "run.failed" | "run.cancelled"),
        ));
        let failed = records.iter().any(|r| r["event"]["type"] == "run.failed");
        let updated = records.iter().rev()
            .find_map(|r| r["event"]["occurredAt"].as_str())
            .unwrap_or(&m.created_at).to_owned();
        let title: String = m.prompt.lines().next().unwrap_or_default()
            .trim().chars().take(60).collect();
        summaries.push(Summary {
            mission_id: m.mission_id,
            title: if title.is_empty() { "Untitled".into() } else { title },
            runtime: m.runtime,
            model: m.model,
            created_at: m.created_at,
            running: !finished && fresh(Some(modified), SystemTime::now()),
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
    headers: HashMap<PathBuf, Metadata>,
}

impl Session {
    pub fn new(dir: PathBuf, mission_id: String) -> Self {
        Self {
            dir,
            anchor: mission_id,
            readers: Vec::new(),
            scanned: None,
            headers: HashMap::new(),
        }
    }

    pub fn poll(&mut self) -> io::Result<Snapshot> {
        if !safe_id(&self.anchor) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        if self.scanned.is_none_or(|at| at.elapsed() >= Duration::from_secs(2)) {
            let paths = self.chain()?;
            let mut old: HashMap<_, _> = self.readers.drain(..)
                .map(|r| (r.id.clone(), r)).collect();
            for (id, path) in paths {
                self.readers.push(old.remove(&id).unwrap_or_else(|| Reader {
                    id,
                    tail: Tail::new(path, 0),
                    transcript: Transcript::default(),
                }));
            }
            self.scanned = Some(Instant::now());
        }

        let mut snapshot = Snapshot::default();
        let count = self.readers.len();
        for (index, reader) in self.readers.iter_mut().enumerate() {
            // Completed ancestors are immutable; poll only the tip and new files.
            if index + 1 == count || reader.tail.offset == 0 {
                let records = reader.tail.poll();
                if reader.tail.reset {
                    reader.transcript = Transcript::default();
                }
                reader.transcript.error = reader.tail.error;
                if let Some(kind) = reader.transcript.error {
                    return Err(kind.into());
                }
                reader.transcript.apply(records);
            }
            snapshot.items.extend(reader.transcript.items.iter().cloned());
            snapshot.tip = reader.id.clone();
            snapshot.running = reader.transcript.running
                && fresh(reader.tail.modified, SystemTime::now());
            snapshot.failed = reader.transcript.failed;
        }
        Ok(snapshot)
    }

    fn chain(&mut self) -> io::Result<Vec<(String, PathBuf)>> {
        let mut all = HashMap::new();
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_file()
                || entry.path().extension().is_none_or(|e| e != "jsonl")
            {
                continue;
            }
            let path = entry.path();
            // mission.created never changes in an append-only file.
            if !self.headers.contains_key(&path) {
                if let Ok(records) = head(&path) {
                    if let Some(m) = records.iter().find_map(metadata) {
                        self.headers.insert(path.clone(), m);
                    }
                }
            }
            if let Some(m) = self.headers.get(&path).cloned() {
                all.insert(m.mission_id.clone(), (
                    m,
                    path,
                    entry.metadata()?.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                ));
            }
        }
        if !all.contains_key(&self.anchor) {
            return Err(io::ErrorKind::NotFound.into());
        }

        let mut root = self.anchor.clone();
        let mut seen = HashSet::new();
        while seen.insert(root.clone()) {
            let Some(parent) = all.get(&root).and_then(|v| v.0.parent.as_ref()) else {
                break;
            };
            if !all.contains_key(parent) || seen.contains(parent) {
                break;
            }
            root = parent.clone();
        }

        let mut result = Vec::new();
        seen.clear();
        while seen.insert(root.clone()) {
            let Some((_, path, _)) = all.get(&root) else { break };
            result.push((root.clone(), path.clone()));
            let next = all.values()
                .filter(|v| v.0.parent.as_deref() == Some(root.as_str()))
                .max_by(|a, b| {
                    a.2.cmp(&b.2)
                        .then_with(|| a.0.mission_id.cmp(&b.0.mission_id))
                });
            let Some(next) = next else { break };
            root = next.0.mission_id.clone();
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

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
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
    fn running_and_finished_summaries_use_recent_mtime() {
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
        let list = list_recent(&temp.0, 10);
        assert!(list.iter().find(|r| r.mission_id == "live").unwrap().running);
        assert!(!list.iter().find(|r| r.mission_id == "done").unwrap().running);
        assert!(list.iter().find(|r| r.mission_id == "failed").unwrap().failed);
        assert!(!fresh(
            Some(SystemTime::now() - Duration::from_secs(121)),
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
}
```

## `main.rs`: state additions

Add:

```rust
mod ledger;
```

In `State`, replace the existing `recents` field and add these fields:

```rust
    recents: Latest<Result<RecentBatch, String>>,
    ledger_summaries: HashMap<String, ledger::Summary>,
    ledger_available: bool,
    ledger: Option<LedgerPane>,
    ledger_job: Latest<LedgerAnswer>,
```

In `State::default()`, keep `recents: Latest::default()` and add:

```rust
            ledger_summaries: HashMap::new(),
            ledger_available: false,
            ledger: None,
            ledger_job: Latest::default(),
```

Add these supporting types and functions:

```rust
const LEDGER_TARGET: &str = "@ledger";

struct RecentBatch {
    rows: Vec<ConversationRow>,
    summaries: HashMap<String, ledger::Summary>,
    local: bool,
}

struct LedgerPane {
    header: Teammate,
    conversation: Conversation,
    items: Vec<ledger::Item>,
    dir: std::path::PathBuf,
    worker: Option<ledger::Session>,
    snapshot: ledger::Snapshot,
    due: f64,
    loading: bool,
    // MCP returns the old root ID on follow-up; wait for its new child.
    after_tip: Option<String>,
}

type LedgerAnswer = Result<(ledger::Session, ledger::Snapshot), String>;

fn ledger_dir() -> Option<std::path::PathBuf> {
    engine::default_data_folder().map(|dir| dir.join("mission-ledger"))
}

fn begin_ledger(
    s: &mut State,
    id: String,
    title: String,
    header: Teammate,
    after_tip: Option<String>,
    tx: &mut Tx,
) {
    let Some(dir) = ledger_dir() else {
        s.status = "Locust's data folder could not be found.".into();
        s.status_tone = StatusTone::Red;
        return;
    };
    s.ledger_job.cancel();
    s.ledger = Some(LedgerPane {
        header,
        conversation: Conversation {
            conversation_id: Some(id.clone()),
            turns: vec![("You".into(), title)],
        },
        items: Vec::new(),
        worker: Some(ledger::Session::new(dir.clone(), id)),
        dir,
        snapshot: ledger::Snapshot::default(),
        due: 0.0,
        loading: true,
        after_tip,
    });
    s.status = "Loading conversation…".into();
    s.status_tone = StatusTone::Muted;
    tx.wake_after(Duration::ZERO);
}

fn leave_ledger(s: &mut State) {
    s.ledger_job.cancel();
    if s.flight.as_ref().is_some_and(|f| f.teammate_id == LEDGER_TARGET) {
        detach(s);
    }
    s.ledger = None;
}

fn select_teammate(s: &mut State, id: &str, tx: &mut Tx) {
    leave_ledger(s);
    let Some(index) = s.teammates.iter().position(|t| t.id == id) else {
        return;
    };
    s.selected = index;
    if s.ledger_available {
        let header = s.teammates.get(index).cloned();
        let conversation = s.conversations.get(id)
            .and_then(|c| c.conversation_id.clone());
        if let (Some(header), Some(conversation)) = (header, conversation) {
            begin_ledger(s, conversation, String::new(), header, None, tx);
        }
    }
    tx.scroll_to_bottom("messages");
}

fn ledger_tick(s: &mut State, now: f64, tx: &mut Tx) -> bool {
    let mut changed = false;
    if let Some(result) = s.ledger_job.take() {
        match result {
            Ok((worker, snapshot)) => {
                if let Some(pane) = s.ledger.as_mut() {
                    changed = pane.loading || pane.snapshot != snapshot;
                    let finished = pane.snapshot.running && !snapshot.running;
                    let new_tip = pane.snapshot.tip != snapshot.tip;
                    if pane.after_tip.as_ref()
                        .is_some_and(|tip| tip != &snapshot.tip)
                    {
                        pane.after_tip = None;
                    }
                    pane.items = snapshot.items.clone();
                    pane.loading = false;
                    pane.due = now + if snapshot.running || pane.after_tip.is_some() {
                        0.25
                    } else {
                        2.0
                    };
                    pane.worker = Some(worker);
                    pane.snapshot = snapshot;
                    let active = pane.snapshot.running || pane.after_tip.is_some();
                    s.status = if active { "Working…" } else { "Ready" }.into();
                    s.status_tone = if pane.snapshot.failed {
                        StatusTone::Red
                    } else if active {
                        StatusTone::Green
                    } else {
                        StatusTone::Muted
                    };

                    if active {
                        s.flight = Some(Flight {
                            teammate_id: LEDGER_TARGET.into(),
                            message: None,
                        });
                    } else if s.flight.as_ref()
                        .is_some_and(|f| f.teammate_id == LEDGER_TARGET)
                    {
                        s.flight = None;
                    }

                    if let Some(id) = &pane.conversation.conversation_id {
                        for row in &mut s.recent {
                            if row.id == *id || row.id == pane.snapshot.tip {
                                row.state = if active {
                                    "running"
                                } else if pane.snapshot.failed {
                                    "failed"
                                } else {
                                    "completed"
                                }.into();
                            }
                        }
                    }
                    if finished || new_tip {
                        refresh_recent(s);
                    }
                    // stick_bottom follows new text while the listener stays at the bottom.
                }
            }
            Err(words) => {
                changed = true;
                s.status = words;
                s.status_tone = StatusTone::Red;
                s.flight = None;
                if let Some(pane) = s.ledger.as_mut() {
                    pane.loading = false;
                    pane.due = now + 2.0;
                    if let Some(id) = &pane.conversation.conversation_id {
                        pane.worker = Some(ledger::Session::new(
                            pane.dir.clone(), id.clone(),
                        ));
                    }
                }
            }
        }
    }

    if let Some(pane) = s.ledger.as_mut() {
        if now >= pane.due {
            if let Some(mut worker) = pane.worker.take() {
                s.ledger_job.request((), move |()| {
                    let snapshot = worker.poll()
                        .map_err(|_| "Local conversation could not be read.".to_owned())?;
                    Ok((worker, snapshot))
                });
            }
        } else if pane.worker.is_some() {
            tx.wake_after(Duration::from_secs_f64((pane.due - now).max(0.0)));
        }
    }
    changed
}

fn message_column(items: &[ledger::Item], name: &str, p: &Palette) -> El {
    let mut messages = col().w_full().gap(24.0).padding(24.0);
    let style = markdown_style(p);
    for (index, item) in items.iter().enumerate() {
        let key = format!("ledger-item-{index}");
        let content = match item {
            ledger::Item::User(body) => row().w_full().justify_end()
                .child(
                    row().w_frac(0.7).justify_end().child(
                        text(body.clone()).font(type_::BODY).color(p.text)
                            .shrink().wrap_anywhere().selectable(true)
                            .padding(12.0).radius(10.0).fill(p.your_bubble)
                            .border(1.0, p.hairline),
                    ),
                ),
            ledger::Item::Assistant { text: body, r#final, .. } => {
                col().w_full().gap(8.0)
                    .child(text(name).font(type_::LABEL).color(p.muted))
                    .child(
                        Markdown::new(body.clone()).streaming(!*r#final)
                            .selectable(true).render(&style)
                            .key(format!("{key}-markdown")).w_full(),
                    )
            }
            ledger::Item::Tool { name, status, summary } => {
                let verb = match status {
                    ledger::Status::Running => "Running",
                    ledger::Status::Done => "Ran",
                    ledger::Status::Failed => "Failed",
                };
                text(format!(
                    "{verb}: {name}{}",
                    if summary.is_empty() {
                        String::new()
                    } else {
                        format!(" · {summary}")
                    },
                ))
                .font(type_::LABEL).color(p.muted)
                .w_full().wrap_anywhere().selectable(true)
            }
            ledger::Item::Plan(body) => text(format!("Plan\n{body}"))
                .font(type_::LABEL).color(p.muted)
                .w_full().wrap_anywhere().selectable(true),
            ledger::Item::Note { level, text: body } => {
                text(format!("{level}: {body}"))
                    .font(type_::LABEL).color(p.muted)
                    .w_full().wrap_anywhere().selectable(true)
            }
        };
        messages = messages.child(content.key(key));
    }
    messages
}
```

## `main.rs`: replacement functions

Replace these existing functions in full:

```rust
fn setup(s: &mut State, waker: Waker) {
    s.team.set_waker(waker.clone());
    s.sending.set_waker(waker.clone());
    s.waiting.set_waker(waker.clone());
    s.recents.set_waker(waker.clone());
    s.opening.set_waker(waker.clone());
    s.ledger_job.set_waker(waker);
    load_team(s);
}

fn selected_conversation(s: &State) -> Option<(&Teammate, &Conversation)> {
    if let Some(pane) = &s.ledger {
        return Some((&pane.header, &pane.conversation));
    }
    let teammate = s.teammates.get(s.selected)?;
    Some((teammate, s.conversations.get(&teammate.id)?))
}

fn can_send(s: &State) -> bool {
    !s.demo
        && (s.load_error.is_none() || s.ledger.is_some())
        && !s.team.is_pending()
        && s.flight.is_none()
        && s.opening_teammate.is_none()
        && !s.ledger.as_ref().is_some_and(|p| p.loading)
        && selected_conversation(s).is_some()
        && !s.draft.text.trim().is_empty()
}

fn set_status(s: &mut State, id: &str, message: &str, ink: StatusTone) {
    let name = if id == LEDGER_TARGET {
        s.ledger.as_ref().map(|p| p.header.name.as_str()).unwrap_or("Locust")
    } else {
        s.teammates.iter().find(|t| t.id == id)
            .map(|t| t.name.as_str()).unwrap_or("Locust")
    };
    s.status = format!("{name}: {message}");
    s.status_tone = ink;
}

fn send(s: &mut State, _: &mut Tx) {
    if !can_send(s) {
        return;
    }
    let Some((teammate, conversation)) = selected_conversation(s) else {
        return;
    };
    let teammate_id = teammate.id.clone();
    let conversation_id = conversation.conversation_id.clone();
    let message = s.draft.text.clone();
    let target = if s.ledger.is_some() {
        LEDGER_TARGET.into()
    } else {
        teammate_id.clone()
    };
    s.flight = Some(Flight {
        teammate_id: target.clone(),
        message: Some(message.clone()),
    });

    // An idle ledger result must not clear a new send. Reopen after acceptance.
    if s.ledger.is_some() {
        s.ledger_job.cancel();
        if let Some(pane) = s.ledger.as_mut() {
            pane.worker = None;
        }
    }
    s.draft.clear();
    set_status(s, &target, "Sending…", StatusTone::Muted);

    s.sending.request(
        (teammate_id, conversation_id, message),
        |(id, conversation, message)| {
            let engine = Engine::local()?;
            match conversation {
                Some(id) => engine.send(&id, &message),
                None => engine.start(&id, &message),
            }
        },
    );
}

fn refresh_recent(s: &mut State) {
    if s.demo {
        return;
    }
    s.recents.request((), |()| {
        if let Some(dir) = ledger_dir() {
            if let Ok(summaries) = ledger::try_list_recent(&dir, 12) {
                let rows = summaries.iter().map(|m| ConversationRow {
                    id: m.mission_id.clone(),
                    teammate: engine::runtime_name(&m.runtime).into(),
                    title: m.title.clone(),
                    started_by: "ledger".into(),
                    state: if m.running {
                        "running"
                    } else if m.failed {
                        "failed"
                    } else {
                        "completed"
                    }.into(),
                    updated: m.updated.clone(),
                }).collect();
                return Ok(RecentBatch {
                    rows,
                    summaries: summaries.into_iter()
                        .map(|m| (m.mission_id.clone(), m)).collect(),
                    local: true,
                });
            }
        }
        // An unreadable local folder falls back to MCP and its visibility rules.
        Ok(RecentBatch {
            rows: Engine::local()?.conversations()?,
            summaries: HashMap::new(),
            local: false,
        })
    });
}

fn detach(s: &mut State) {
    s.sending.cancel();
    s.waiting.cancel();
    s.opening.cancel();
    s.ledger_job.cancel();
    s.flight = None;
    s.opening_teammate = None;
    if let Some(pane) = s.ledger.as_mut() {
        pane.worker = None;
        pane.loading = false;
    }
}

fn open_recent(s: &mut State, row: ConversationRow, tx: &mut Tx) {
    if let Some(summary) = s.ledger_summaries.get(&row.id).cloned() {
        detach(s);
        // Ledger metadata has no reliable teammate identity; do not guess from a model.
        let header = Teammate {
            id: LEDGER_TARGET.into(),
            name: engine::runtime_name(&summary.runtime).into(),
            role: "Conversation".into(),
            runtime: Some(summary.runtime),
            model: Some(summary.model),
            busy: summary.running,
            runs_in: String::new(),
            folder: None,
        };
        begin_ledger(s, row.id, row.title, header, None, tx);
        return;
    }

    if s.team.is_pending() {
        return;
    }
    let Some(index) = s.teammates.iter()
        .position(|t| t.name.eq_ignore_ascii_case(&row.teammate))
    else {
        s.status = format!("{} is not in the current team.", row.teammate);
        s.status_tone = StatusTone::Muted;
        return;
    };
    let Some(id) = s.teammates.get(index).map(|t| t.id.clone()) else {
        return;
    };
    detach(s);
    s.ledger = None;
    s.selected = index;
    s.conversations.insert(id.clone(), Conversation {
        conversation_id: Some(row.id.clone()),
        turns: vec![("You".into(), row.title)],
    });
    set_status(s, &id, "Loading…", StatusTone::Muted);
    tx.scroll_to_bottom("messages");

    if !s.demo {
        s.opening_teammate = Some(id);
        s.opening.request(row.id, |id| Engine::local()?.reply(&id, 0));
    }
}

fn fresh_conversation(s: &mut State) {
    let Some(id) = s.teammates.get(s.selected).map(|t| t.id.clone()) else {
        return;
    };
    detach(s);
    s.ledger = None;
    s.conversations.insert(id.clone(), Conversation::default());
    set_status(s, &id, "Ready", StatusTone::Muted);
}

fn on_key(s: &mut State, event: &KeyEvent, tx: &mut Tx) -> bool {
    if event.key == Key::Escape {
        if s.flight.is_some() || s.opening_teammate.is_some() {
            detach(s);
            s.status = "Stopped watching; the turn goes on in Locust".into();
            s.status_tone = StatusTone::Muted;
        } else {
            s.draft.clear();
        }
        return true;
    }
    if event.mods.shift || event.mods.alt {
        return false;
    }
    if event.is_command('n') {
        fresh_conversation(s);
        tx.scroll_to_bottom("messages");
        return true;
    }
    for digit in '1'..='9' {
        if event.is_command(digit) {
            if let Some(id) = s.teammates.get((digit as u8 - b'1') as usize)
                .map(|t| t.id.clone())
            {
                select_teammate(s, &id, tx);
            }
            return true;
        }
    }
    false
}

fn apply_team(s: &mut State, team: Vec<Teammate>) {
    let selected = s.teammates.get(s.selected).map(|t| t.id.clone());
    for t in &team {
        s.conversations.entry(t.id.clone()).or_default();
    }
    s.teammates = team;
    s.selected = selected.as_deref()
        .and_then(|id| s.teammates.iter().position(|t| t.id == id))
        .unwrap_or(0);
    s.load_error = None;

    let active = s.flight.as_ref().map(|f| f.teammate_id.clone())
        .or_else(|| s.opening_teammate.clone());
    if let Some(id) = active {
        if id != LEDGER_TARGET {
            let _ = target_index(s, &id);
        }
    } else if s.ledger.is_none() {
        s.status = if s.teammates.is_empty() {
            "No teammates yet. Add a teammate in Locust."
        } else {
            "Ready"
        }.into();
        s.status_tone = StatusTone::Muted;
    }
}

fn poll(s: &mut State, now: f64, tx: &mut Tx) -> bool {
    if s.demo {
        return false;
    }
    let mut changed = false;

    if let Some(result) = s.team.take() {
        changed = true;
        match result {
            Ok(team) => apply_team(s, team),
            Err(error) => {
                s.status = error.clone();
                s.status_tone = StatusTone::Red;
                s.load_error = Some(error);
            }
        }
    }

    if let Some(result) = s.sending.take() {
        changed = true;
        if let Some(id) = s.flight.as_ref().map(|f| f.teammate_id.clone()) {
            let message = s.flight.as_mut().and_then(|f| f.message.take());
            match result {
                Ok(started) => {
                    if s.ledger_available || s.ledger.is_some() {
                        let header = s.ledger.as_ref().map(|p| p.header.clone())
                            .or_else(|| s.teammates.iter()
                                .find(|t| t.id == id).cloned());
                        let after = s.ledger.as_ref()
                            .map(|p| p.snapshot.tip.clone())
                            .filter(|v| !v.is_empty());

                        if let Some(header) = header {
                            if let Some(conversation) = s.conversations.get_mut(&id) {
                                conversation.conversation_id =
                                    Some(started.conversation_id.clone());
                                if let Some(message) = &message {
                                    conversation.turns.push(("You".into(), message.clone()));
                                }
                            }
                            begin_ledger(
                                s,
                                started.conversation_id,
                                message.unwrap_or_default(),
                                header,
                                after,
                                tx,
                            );
                            s.flight = Some(Flight {
                                teammate_id: LEDGER_TARGET.into(),
                                message: None,
                            });
                        } else {
                            tracking_error(s, &id, "Teammate is no longer in the team.");
                        }
                    } else if let Some(index) = target_index(s, &id) {
                        if let Some(conversation) = s.conversations.get_mut(&id) {
                            conversation.conversation_id = Some(started.conversation_id);
                            if let Some(message) = message {
                                conversation.turns.push(("You".into(), message));
                            }
                            set_status(s, &id, "Working…", StatusTone::Green);
                            request_reply(s, &id, Duration::ZERO);
                            if s.selected == index {
                                tx.scroll_to_bottom("messages");
                            }
                        } else {
                            tracking_error(s, &id, "Conversation is no longer available.");
                        }
                    }
                }
                Err(error) => {
                    if let Some(message) = message {
                        restore_failed_draft(s, message);
                    }
                    set_status(s, &id, &error, StatusTone::Red);
                    s.flight = None;
                    if let Some(pane) = s.ledger.as_mut() {
                        if let Some(id) = &pane.conversation.conversation_id {
                            pane.worker = Some(ledger::Session::new(
                                pane.dir.clone(), id.clone(),
                            ));
                            pane.due = 0.0;
                            tx.wake_after(Duration::ZERO);
                        }
                    }
                }
            }
        }
    }

    if let Some(result) = s.recents.take() {
        changed = true;
        match result {
            Ok(batch) => {
                s.recent = batch.rows;
                s.ledger_summaries = batch.summaries;
                s.ledger_available = batch.local;
                s.recent_error = None;
            }
            Err(words) => {
                s.ledger_available = false;
                s.ledger_summaries.clear();
                s.recent_error = Some(words);
            }
        }
    }

    if let Some(result) = s.opening.take() {
        changed = true;
        if let Some(id) = s.opening_teammate.take() {
            s.flight = Some(Flight {
                teammate_id: id.clone(),
                message: None,
            });
            land_reply(s, &id, result, tx);
        }
    }
    if let Some(result) = s.waiting.take() {
        changed = true;
        if let Some(id) = s.flight.as_ref().map(|f| f.teammate_id.clone()) {
            land_reply(s, &id, result, tx);
        }
    }

    changed | ledger_tick(s, now, tx)
}
```

Replace `view` in full:

```rust
fn view(s: &State, cx: &mut Cx<State>) -> El {
    let p = Palette::new(s.appearance.unwrap_or_else(|| cx.system_dark()));
    let mut rows = col().w_full().gap(6.0).padding(10.0);

    for (index, teammate) in s.teammates.iter().enumerate() {
        let initial = teammate.name.trim_start().chars().next()
            .map(|c| c.to_uppercase().collect::<String>())
            .unwrap_or_else(|| "?".into());
        let selected = s.ledger.as_ref()
            .map(|pane| pane.header.id == teammate.id)
            .unwrap_or(s.selected == index);
        let busy = teammate.busy
            || s.flight.as_ref().is_some_and(|f| f.teammate_id == teammate.id)
            || s.ledger.as_ref().is_some_and(|pane| {
                pane.header.id == teammate.id && pane.snapshot.running
            });

        let mut title = row().w_full().center_y().gap(8.0)
            .child(
                el().size(22.0, 22.0).radius(11.0).center().fill(p.raised)
                    .child(text(initial).font(type_::LABEL).color(p.secondary)),
            )
            .child(
                text(teammate.name.clone()).font(type_::TITLE)
                    .color(p.text).grow(1.0).shrink(),
            );
        if busy {
            title = title.child(
                el().size(6.0, 6.0).radius(3.0).fill(p.green),
            );
        }
        let id = teammate.id.clone();
        rows = rows.child(
            col().key(format!("teammate-{id}")).w_full()
                .padding(12.0).gap(5.0).radius(8.0)
                .fill(if selected { p.selected } else { p.sidebar })
                .child(title)
                .child(
                    text(teammate.role.clone()).font(type_::BODY)
                        .color(p.secondary).w_full().pl(30.0).wrap_anywhere(),
                )
                .child(
                    text(agent(teammate)).font(type_::LABEL)
                        .color(p.muted).w_full().pl(30.0).wrap_anywhere(),
                )
                .on_press_tx(move |s: &mut State, tx| {
                    select_teammate(s, &id, tx);
                }),
        );
    }

    let mut recent = col().w_full().px(10.0).pb(16.0).gap(4.0)
        .child(
            row().w_full().center_y().px(12.0).py(10.0)
                .child(text("Recent").font(type_::LABEL)
                    .color(p.secondary).grow(1.0))
                .child(text("Refresh").key("refresh-recent").font(type_::LABEL)
                    .color(p.muted).on_press(refresh_recent)),
        );
    if let Some(words) = &s.recent_error {
        recent = recent.child(
            text(words.clone()).font(type_::LABEL).color(p.muted)
                .w_full().px(12.0).wrap_anywhere(),
        );
    } else if s.recent.is_empty() {
        recent = recent.child(
            text(if s.recents.is_pending() {
                "Loading…"
            } else {
                "No recent conversations."
            })
            .font(type_::LABEL).color(p.muted).w_full().px(12.0).wrap(),
        );
    } else {
        for item in s.recent.iter().take(12) {
            let picked = selected_conversation(s)
                .and_then(|(_, c)| c.conversation_id.as_deref())
                == Some(item.id.as_str());
            let mut title = row().w_full().center_y().gap(6.0)
                .child(
                    text(item.title.clone()).font(type_::BODY)
                        .color(p.text).grow(1.0).shrink().truncate(),
                );
            let dot = match item.state.as_str() {
                "running" => Some(p.green),
                "failed" => Some(p.red),
                _ => None,
            };
            if let Some(ink) = dot {
                title = title.child(
                    el().size(5.0, 5.0).radius(2.5).fill(ink),
                );
            }
            let item = item.clone();
            recent = recent.child(
                col().key(format!("recent-{}", item.id)).w_full()
                    .padding(12.0).gap(5.0).radius(8.0)
                    .fill(if picked { p.selected } else { p.sidebar })
                    .child(title)
                    .child(
                        text(format!(
                            "{} · {}",
                            item.teammate,
                            when(&item.updated, clock_seconds()),
                        ))
                        .font(type_::LABEL).color(p.muted).w_full().truncate(),
                    )
                    .on_press_tx(move |s: &mut State, tx| {
                        open_recent(s, item.clone(), tx);
                    }),
            );
        }
    }

    let sidebar = col().w(248.0).h_full().fill(p.sidebar)
        .border_sides([0.0, 1.0, 0.0, 0.0], p.hairline)
        .child(
            col().w_full().h(88.0).pt(48.0).pl(52.0).pr(20.0).window_drag()
                .child(text("LOCUST").font(type_::LABEL).color(p.secondary)),
        )
        .child(
            col().key("team-list").w_full().grow(1.0).min_h(0.0)
                .scroll(ScrollSpec { stick_bottom: false })
                .child(col().w_full().child(rows).child(recent)),
        );

    // Local history remains readable even when the MCP engine is unavailable.
    let error = s.load_error.as_ref().filter(|_| s.ledger.is_none());
    let right = if let Some(error) = error {
        col().grow(1.0).shrink().h_full().center().padding(32.0).gap(18.0)
            .child(
                text(error.clone()).font(type_::BODY).color(p.red)
                    .align(Align::Center).w_full().wrap_anywhere().selectable(true),
            )
            .child(
                el().key("retry").h(40.0).px(18.0).center().radius(8.0)
                    .fill(p.raised).border(1.0, p.hairline)
                    .child(text("Try again").font(type_::LABEL).color(p.text))
                    .on_press(load_team),
            )
    } else if let Some((teammate, conversation)) = selected_conversation(s) {
        let fallback;
        let items = if let Some(pane) = &s.ledger {
            &pane.items
        } else {
            fallback = conversation.turns.iter().enumerate()
                .map(|(i, (who, body))| {
                    if who == "You" {
                        ledger::Item::User(body.clone())
                    } else {
                        ledger::Item::Assistant {
                            item_id: format!("legacy-{i}"),
                            text: body.clone(),
                            r#final: true,
                        }
                    }
                }).collect::<Vec<_>>();
            &fallback
        };

        let transcript = if items.is_empty() {
            let title = if let Some(pane) = &s.ledger {
                if pane.loading {
                    "Loading conversation…".into()
                } else {
                    "No transcript items.".into()
                }
            } else {
                format!("Ask {} anything", teammate.name)
            };
            let mut empty = col().key("messages").w_full().grow(1.0).min_h(0.0)
                .center().padding(24.0).gap(10.0)
                .child(
                    text(title).font(type_::TITLE).color(p.text)
                        .w_full().align(Align::Center).wrap(),
                );
            if s.ledger.is_none() {
                empty = empty.child(
                    text(format!(
                        "{} · runs in {}",
                        agent(teammate),
                        mode_word(&teammate.runs_in),
                    ))
                    .font(type_::LABEL).color(p.muted)
                    .w_full().align(Align::Center).wrap_anywhere(),
                );
            }
            empty
        } else {
            col().key("messages").w_full().grow(1.0).min_h(0.0)
                .scroll(ScrollSpec { stick_bottom: true })
                .child(message_column(items, &teammate.name, &p))
        };

        let mut status = row().w_full().center_y().gap(8.0).px(24.0).py(10.0);
        if s.flight.is_some() || s.opening_teammate.is_some() {
            status = status.child(
                el().key("activity-dot").size(6.0, 6.0).radius(3.0)
                    .fill(p.status(s.status_tone))
                    .cycle(Cycle {
                        period: 1.6,
                        phase: 0.0,
                        stops: &[(0.0, 0.35), (0.5, 1.0), (1.0, 0.35)],
                        still: 1.0,
                        fps: 30.0,
                    }),
            );
        }
        status = status.child(
            text(s.status.clone()).font(type_::LABEL)
                .color(p.status(s.status_tone))
                .grow(1.0).shrink().wrap_anywhere().selectable(true),
        );

        let composer = text_input(&s.draft, type_::BODY)
            .key("composer").single_line()
            .placeholder(format!("Message {}", teammate.name), p.muted)
            .color(p.text).grow(1.0).shrink().h(48.0).padding(12.0)
            .radius(9.0).fill(p.field).border(1.0, p.hairline)
            .on_edit(|s: &mut State| &mut s.draft)
            .on_submit(send).on_input_key(on_key);
        let enabled = can_send(s);
        let mut button = el().key("send").h(48.0).px(18.0).center().radius(9.0)
            .fill(p.raised).border(1.0, p.hairline).disabled(!enabled)
            .child(
                text("Send").font(type_::LABEL)
                    .color(if enabled { p.text } else { p.muted }),
            );
        if enabled {
            button = button.on_press_tx(send);
        }

        col().grow(1.0).shrink().h_full()
            .child(
                col().w_full().h(88.0).px(24.0).pt(30.0).gap(5.0).window_drag()
                    .border_sides([0.0, 0.0, 1.0, 0.0], p.hairline)
                    .child(
                        text(teammate.name.clone()).font(type_::TITLE).color(p.text),
                    )
                    .child(text(agent(teammate)).font(type_::LABEL).color(p.muted)),
            )
            .child(transcript)
            .child(status)
            .child(
                row().w_full().gap(10.0).px(24.0).pb(24.0)
                    .child(composer).child(button),
            )
    } else {
        col().grow(1.0).shrink().h_full().center().padding(32.0)
            .child(
                text(s.status.clone()).font(type_::BODY)
                    .color(p.status(s.status_tone)).w_full().wrap(),
            )
    };

    row().w_full().h_full().fill(p.bg).child(sidebar).child(right)
}
```

## Synthetic scenes

Add these functions:

```rust
fn ledger_scene(s: &mut State, live: bool, light: bool) {
    team_scene(s);
    s.appearance = Some(!light);
    let Some(header) = s.teammates.first().cloned() else {
        return;
    };
    let items = vec![
        ledger::Item::User("Explain this synthetic preview.".into()),
        ledger::Item::Plan(
            "completed: Read the input\nin_progress: Summarize the result".into(),
        ),
        ledger::Item::Tool {
            name: "inspect".into(),
            status: ledger::Status::Done,
            summary: "Read a synthetic fixture".into(),
        },
        ledger::Item::Assistant {
            item_id: "synthetic-answer".into(),
            text: if live {
                "### Live answer\n\nThis text is still arriving".into()
            } else {
                "### Complete transcript\n\nThe preview shows **messages**, tool steps, plans, and notes.".into()
            },
            r#final: !live,
        },
        ledger::Item::Note {
            level: "warning".into(),
            text: "Synthetic demonstration note.".into(),
        },
    ];
    s.ledger = Some(LedgerPane {
        header,
        conversation: Conversation {
            conversation_id: Some("synthetic-ledger".into()),
            turns: Vec::new(),
        },
        items: items.clone(),
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: ledger::Snapshot {
            items,
            tip: "synthetic-ledger".into(),
            running: live,
            failed: false,
        },
        due: 0.0,
        loading: false,
        after_tip: None,
    });
    if live {
        s.flight = Some(Flight {
            teammate_id: LEDGER_TARGET.into(),
            message: None,
        });
    }
    s.status = if live { "Working…" } else { "Ready" }.into();
    s.status_tone = if live { StatusTone::Green } else { StatusTone::Muted };
}

fn ledger_transcript_scene(s: &mut State) {
    ledger_scene(s, false, false);
}

fn ledger_live_scene(s: &mut State) {
    ledger_scene(s, true, false);
}

fn ledger_transcript_light_scene(s: &mut State) {
    ledger_scene(s, false, true);
}

fn ledger_live_light_scene(s: &mut State) {
    ledger_scene(s, true, true);
}
```

Append these entries to the ordinary window’s `AppSpec.scenes`:

```rust
            ("ledger-transcript", ledger_transcript_scene),
            ("ledger-live", ledger_live_scene),
            ("ledger-transcript-light", ledger_transcript_light_scene),
            ("ledger-live-light", ledger_live_light_scene),
```

Existing `on_close` already calls `detach`, which now cancels ledger work too. Keep the other functions and Quick Ask dispatch as they are.

Confirmed additional Rune API: `Markdown::streaming` at `crates/rune-kit/src/markdown.rs:277`. **Unconfirmed Rune APIs: none.**

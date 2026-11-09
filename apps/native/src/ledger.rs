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

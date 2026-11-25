//! Schema 27: mission-store createMission / appendEvents. One owned writer per new mission.
use crate::store::Store;
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{self, Write},
    path::PathBuf,
};
pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// one-at-a-time.ts `oneAtATime`: calls of `run` go one after another, never overlapping, in the order they were made.
/// A failed call does not stop the ones behind it -- an `Err` or a panic in one still lets the next one in.
///
/// For writers that read the tail of a record and append after it: two of them at once read the same tail and write
/// the same `ledgerSequence`, which the reader takes for a torn file (the relay's notes were one, the code review's B4).
pub struct OneAtATime {
    turns: std::sync::Mutex<(u64, u64)>,
    next: std::sync::Condvar,
}

impl Default for OneAtATime {
    fn default() -> Self {
        Self::new()
    }
}

impl OneAtATime {
    pub const fn new() -> Self {
        Self { turns: std::sync::Mutex::new((0, 0)), next: std::sync::Condvar::new() }
    }

    fn turns(&self) -> std::sync::MutexGuard<'_, (u64, u64)> {
        self.turns.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Calls taken and not yet finished, the running one included.
    pub fn waiting(&self) -> u64 {
        let turns = self.turns();
        turns.0 - turns.1
    }

    pub fn run<T>(&self, work: impl FnOnce() -> T) -> T {
        struct Done<'a>(&'a OneAtATime);
        impl Drop for Done<'_> {
            fn drop(&mut self) {
                self.0.turns().1 += 1;
                self.0.next.notify_all();
            }
        }
        let mut turns = self.turns();
        let ticket = turns.0;
        turns.0 += 1;
        while turns.1 != ticket {
            turns = self.next.wait(turns).unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        drop(turns);
        let _done = Done(self);
        work()
    }
}

/// The one gate every writer below passes that reads a ledger's tail and appends after it.
static LEDGER_TAIL: OneAtATime = OneAtATime::new();

#[cfg(test)]
thread_local! {
    /// A test's pause between reading the tail and appending, so two writers that were not one at a time would collide.
    static PAUSE_AFTER_TAIL: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn after_tail() {
    #[cfg(test)]
    std::thread::sleep(std::time::Duration::from_millis(PAUSE_AFTER_TAIL.with(std::cell::Cell::get)));
}
/// `continuesFrom` of a follow-up turn, as `codex-mission.ts` writes it: checkpointEpoch 1 (a follow-up reconciles
/// nothing, so epoch 1 is the only one a reader could mean), reason `follow-up`, the resumed thread when there is one.
/// A stopped turn's checkpoint, appended to its own (native) ledger as mission-store's `createCheckpoint` appends it:
/// the next ledger sequence, the ledger's schema version, fsynced. The turn is not running, so no writer holds the file.
pub fn append_checkpoint(path: &std::path::Path, checkpoint: &Value) -> io::Result<()> {
    LEDGER_TAIL.run(|| append_checkpoint_after_tail(path, checkpoint))
}

fn append_checkpoint_after_tail(path: &std::path::Path, checkpoint: &Value) -> io::Result<()> {
    use std::io::Write;
    let text = std::fs::read_to_string(path)?;
    // mission-store's createCheckpoint: an already-broken ledger cannot be checkpointed durably. Recovery stops at the
    // first record it cannot read, so anything appended past that break would be written, fsynced and unreachable.
    // An `unsafe` verdict (the history read short of its end) is the caller's answer, and it is not written either.
    let read = crate::ledger::read_ledger(path)?;
    let broken = checkpoint["resumeSafety"] == "unsafe" || read.unreadable || !read.issues.is_empty();
    if broken {
        return Err(io::Error::other("The ledger has a record its reader stops at, so nothing is written past it"));
    }
    let records: Vec<Value> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let schema = records.first().and_then(|r| r["schemaVersion"].as_u64()).ok_or_else(|| io::Error::other("Ledger has no first record"))?;
    let next = records.iter().filter_map(|r| r["ledgerSequence"].as_u64()).max().unwrap_or(0) + 1;
    after_tail();
    let record = json!({"schemaVersion": schema, "recordType": "mission.checkpoint", "ledgerSequence": next, "occurredAt": checkpoint["createdAt"], "checkpoint": checkpoint});
    let mut line = serde_json::to_vec(&record)?;
    line.push(b'\n');
    if !text.is_empty() && !text.ends_with('\n') {
        return Err(io::Error::other("Ledger does not end on a whole record"));
    }
    let mut file = std::fs::OpenOptions::new().append(true).open(path)?;
    file.write_all(&line)?;
    file.sync_all()
}

/// A line Locust adds to a turn that has ended (`index.ts` noteCarryOn / noteCheckFix): an `adapter.diagnostic` with
/// the next event and ledger sequence, appended to the turn's own native ledger.
pub fn append_events<F>(path: &std::path::Path, make_events: F) -> io::Result<()>
where
    F: FnOnce(&str, &str, &str, u64, &str) -> Vec<Value>,
{
    LEDGER_TAIL.run(|| append_events_after_tail(path, make_events))
}

fn append_events_after_tail<F>(path: &std::path::Path, make_events: F) -> io::Result<()>
where
    F: FnOnce(&str, &str, &str, u64, &str) -> Vec<Value>,
{
    use std::io::Write;
    let text = std::fs::read_to_string(path)?;
    if !text.is_empty() && !text.ends_with('\n') {
        return Err(io::Error::other("Mission ledger is unavailable"));
    }
    let read = crate::ledger::read_ledger(path)?;
    if read.unreadable || !read.issues.is_empty() {
        return Err(io::Error::other("Mission ledger is unavailable"));
    }
    let records: Vec<Value> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let first = records.first().ok_or_else(|| io::Error::other("Ledger has no first record"))?;
    let meta = &first["metadata"];
    let last = records.iter().rev().find_map(|r| r.get("event"));
    let next_seq = last.and_then(|e| e["sequence"].as_u64()).unwrap_or(0) + 1;
    let mut ledger_seq = records.iter().filter_map(|r| r["ledgerSequence"].as_u64()).max().unwrap_or(0) + 1;
    after_tail();
    let at = now();
    let run = meta["runId"].as_str().unwrap_or_default();
    let mission = meta["missionId"].as_str().unwrap_or_default();
    let adapter = last
        .and_then(|e| e["sourceAdapter"].as_str())
        .or(meta["runtime"].as_str())
        .unwrap_or_default();
    let events = make_events(run, mission, adapter, next_seq, &at);
    if events.is_empty() {
        return Ok(());
    }
    // One event that is not this ledger's run would not read back as part of it: refuse the whole batch before any of
    // it is written (mission-store's appendEvents, a-long-answer-is-not-too-much-to-record).
    if let Some(stranger) = events.iter().position(|e| e["runId"] != run || e["missionId"] != mission) {
        return Err(io::Error::other(format!("Event {} of {} is not this mission's run (correlation), so none were written", stranger + 1, events.len())));
    }
    let mut expected_seq = next_seq;
    for e in &events {
        if e["sequence"].as_u64() != Some(expected_seq) {
            return Err(io::Error::other("Mission event sequence is invalid"));
        }
        expected_seq += 1;
    }
    let mission_runtime = meta["runtime"].as_str();
    if events.iter().any(|e| {
        let wrong_runtime = match (mission_runtime, e["sourceAdapter"].as_str()) {
            (Some(rt), Some(ad)) if !rt.is_empty() && !ad.is_empty() => rt != ad,
            _ => false,
        };
        let bad_delta = e["type"] == "message.delta"
            && (e["payload"].get("operation").is_some_and(|op| !matches!(op.as_str(), Some("append" | "replace")))
                || e["payload"].get("itemId").is_some_and(|id| !id.as_str().is_some_and(|s| !s.trim().is_empty() && s.encode_utf16().count() <= 512)));
        wrong_runtime || bad_delta
    }) {
        return Err(io::Error::other("Mission event is not readable by the ledger reader"));
    }
    let mut buf = Vec::new();
    for event in events {
        let record = json!({
            "schemaVersion": first["schemaVersion"],
            "recordType": "mission.event",
            "ledgerSequence": ledger_seq,
            "occurredAt": at,
            "event": event
        });
        ledger_seq += 1;
        serde_json::to_writer(&mut buf, &record)?;
        buf.push(b'\n');
    }
    let mut file = std::fs::OpenOptions::new().append(true).open(path)?;
    file.write_all(&buf)?;
    file.sync_all()
}

pub fn append_note(path: &std::path::Path, code: &str, message: &str) -> io::Result<()> {
    append_note_with_level(path, "warning", code, message)
}

pub fn append_note_with_level(path: &std::path::Path, level: &str, code: &str, message: &str) -> io::Result<()> {
    LEDGER_TAIL.run(|| append_note_with_level_after_tail(path, level, code, message))
}

fn append_note_with_level_after_tail(path: &std::path::Path, level: &str, code: &str, message: &str) -> io::Result<()> {
    use std::io::Write;
    let text = std::fs::read_to_string(path)?;
    if !text.is_empty() && !text.ends_with('\n') {
        return Err(io::Error::other("Ledger does not end on a whole record"));
    }
    let records: Vec<Value> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let first = records.first().ok_or_else(|| io::Error::other("Ledger has no first record"))?;
    let meta = &first["metadata"];
    let last = records.iter().rev().find_map(|r| r.get("event"));
    let sequence = last.and_then(|e| e["sequence"].as_u64()).unwrap_or(0) + 1;
    let ledger = records.iter().filter_map(|r| r["ledgerSequence"].as_u64()).max().unwrap_or(0) + 1;
    after_tail();
    let at = now();
    let run = meta["runId"].as_str().unwrap_or_default();
    let kind_tag = match code {
        "host.check_fix" => "check-fix",
        "host.relay_ended" => "relay",
        crate::worktrees::CHECKPOINT_CODE => "checkpoint",
        _ => "carry-on",
    };
    let mut event = json!({"id": format!("{run}:{kind_tag}:{sequence}"), "runId": run, "missionId": meta["missionId"], "sequence": sequence,
        "occurredAt": at, "sourceAdapter": last.and_then(|e| e["sourceAdapter"].as_str()).or(meta["runtime"].as_str()).unwrap_or_default()});
    for key in ["requestedRouteId", "resolvedRouteId"] {
        if let Some(route) = last.and_then(|e| e.get(key)) {
            event[key] = route.clone();
        }
    }
    event["type"] = json!("adapter.diagnostic");
    event["payload"] = json!({"level": level, "code": code, "message": message, "terminal": false, "evidence": {"redacted": true}});
    let record = json!({"schemaVersion": first["schemaVersion"], "recordType": "mission.event", "ledgerSequence": ledger, "occurredAt": at, "event": event});
    let mut line = serde_json::to_vec(&record)?;
    line.push(b'\n');
    let mut file = std::fs::OpenOptions::new().append(true).open(path)?;
    file.write_all(&line)?;
    file.sync_all()
}

/// mission-store's `requireText`: nonblank, at most `maximum` UTF-16 units, no NUL.
fn require_text(value: &Value, label: &str, maximum: usize) -> io::Result<()> {
    match value.as_str() {
        Some(s) if !s.trim().is_empty() && s.encode_utf16().count() <= maximum && !s.contains('\0') => Ok(()),
        _ => Err(io::Error::other(format!("{label} is invalid"))),
    }
}
/// mission-store's `requireTimestamp`: text of at most 128 that reads as a time.
fn require_timestamp(value: &Value, label: &str) -> io::Result<()> {
    require_text(value, label, 128)?;
    match chrono::DateTime::parse_from_rfc3339(value.as_str().unwrap_or_default()) {
        Ok(_) => Ok(()),
        Err(_) => Err(io::Error::other(format!("{label} is invalid"))),
    }
}
/// The most lines, and the longest line, a check result's record holds (mission-store's `MAX_EDIT_CHECK_LINES`,
/// `MAX_EDIT_CHECK_LINE_LENGTH`). The checker's own bound (`edit_check::MAX_NEW_LINES`, 30 by 300) is tighter.
pub const EDIT_CHECK_LINES: usize = 60;
pub const EDIT_CHECK_LINE: usize = 500;
fn utf16_cut(text: &str, limit: usize) -> String {
    let mut units = 0;
    text.chars().take_while(|c| { units += c.len_utf16(); units <= limit }).collect()
}
/// mission-store's `validateEditCheck`: the writer refuses what the reader would.
pub fn validate_edit_check(check: &Value) -> io::Result<()> {
    if !matches!(check["outcome"].as_str(), Some("passed" | "failed" | "timed-out" | "could-not-run")) {
        return Err(io::Error::other("Edit check outcome is invalid"));
    }
    require_text(&check["command"], "Edit check command", 500)?;
    let Some(lines) = check["newLines"].as_array().filter(|l| l.len() <= EDIT_CHECK_LINES) else {
        return Err(io::Error::other("Edit check lines are invalid"));
    };
    if !lines.iter().all(|l| l.as_str().is_some_and(|l| l.encode_utf16().count() <= EDIT_CHECK_LINE)) {
        return Err(io::Error::other("Edit check line is invalid"));
    }
    if !check["unchanged"].is_boolean() || !check["first"].is_boolean() {
        return Err(io::Error::other("Edit check flags are invalid"));
    }
    require_timestamp(&check["occurredAt"], "Edit check timestamp")
}
/// mission-store's `boundedEditCheck`: a check result cut to what the record holds, the first lines each at most a
/// line's worth, so the host never writes one the reader would refuse.
pub fn bounded_edit_check(check: &Value) -> Value {
    let lines: Vec<String> = check["newLines"].as_array().into_iter().flatten().take(EDIT_CHECK_LINES)
        .map(|l| utf16_cut(l.as_str().unwrap_or_default(), EDIT_CHECK_LINE)).collect();
    json!({
        "command": utf16_cut(check["command"].as_str().unwrap_or_default(), 500),
        "outcome": check["outcome"],
        "newLines": lines,
        "unchanged": check["unchanged"],
        "first": check["first"],
        "occurredAt": check["occurredAt"],
    })
}
/// mission-store's `validateApproval`: a card's answer the reader would refuse is never written.
pub fn validate_approval(approval: &Value) -> io::Result<()> {
    require_text(&approval["approvalId"], "Approval id", 200)?;
    if !matches!(approval["kind"].as_str(), Some("command" | "file-change" | "question" | "connector")) {
        return Err(io::Error::other("Approval kind is invalid"));
    }
    require_text(&approval["asked"], "Approval question", 2_000)?;
    if !matches!(approval["answer"].as_str(), Some("allowed" | "allowed-always" | "denied" | "answered")) {
        return Err(io::Error::other("Approval answer is invalid"));
    }
    if !matches!(approval["by"].as_str(), Some("card" | "card-saving-a-rule" | "saved-rule" | "earlier-always")) {
        return Err(io::Error::other("Approval answerer is invalid"));
    }
    if let Some(words) = approval.get("words")
        && !words.as_str().is_some_and(|w| w.encode_utf16().count() <= 1_000)
    {
        return Err(io::Error::other("Approval words are invalid"));
    }
    require_timestamp(&approval["askedAt"], "Approval asked timestamp")?;
    require_timestamp(&approval["occurredAt"], "Approval timestamp")
}

/// A post-edit check result, appended to its turn's native ledger as `mission-store`'s `appendEditCheck` appends it.
pub fn append_edit_check(path: &std::path::Path, check: &Value) -> io::Result<()> {
    LEDGER_TAIL.run(|| append_edit_check_after_tail(path, check))
}

fn append_edit_check_after_tail(path: &std::path::Path, check: &Value) -> io::Result<()> {
    use std::io::Write;
    validate_edit_check(check)?;
    let text = std::fs::read_to_string(path)?;
    if !text.is_empty() && !text.ends_with('\n') {
        return Err(io::Error::other("Ledger does not end on a whole record"));
    }
    let records: Vec<Value> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let first = records.first().ok_or_else(|| io::Error::other("Ledger has no first record"))?;
    let schema = first["schemaVersion"].as_u64().ok_or_else(|| io::Error::other("Ledger has no schemaVersion"))?;
    if schema < 17 {
        return Err(io::Error::other("Mission ledger version cannot hold check results"));
    }
    let next = records.iter().filter_map(|r| r["ledgerSequence"].as_u64()).max().unwrap_or(0) + 1;
    after_tail();
    let record = json!({
        "schemaVersion": schema,
        "recordType": "mission.edit_check",
        "ledgerSequence": next,
        "occurredAt": check["occurredAt"],
        "check": check,
    });
    let mut line = serde_json::to_vec(&record)?;
    line.push(b'\n');
    let mut file = std::fs::OpenOptions::new().append(true).open(path)?;
    file.write_all(&line)?;
    file.sync_all()
}

/// The longest argument the record keeps whole, in UTF-16 units (command-record.ts `MAX_ARGUMENT_LENGTH`).
pub const MAX_ARGUMENT_LENGTH: usize = 512;
/// The most arguments the record keeps (`MAX_ARGUMENTS`); the rest are counted.
pub const MAX_ARGUMENTS: usize = 64;

/// The command's arguments as the record keeps them (command-record.ts `recordableCommand`): the flags, which are the
/// point; never the prompt, whatever flag introduced it (compared whole after trimming, so a flag that merely contains
/// it stays); an overlong argument shortened with an ellipsis rather than dropped; and at most 64, the rest counted.
pub fn recordable_args(args: &[String], prompt: &str) -> Vec<String> {
    let trimmed = prompt.trim();
    let mut kept: Vec<String> = args
        .iter()
        .take(MAX_ARGUMENTS)
        .map(|argument| {
            if !trimmed.is_empty() && argument.trim() == trimmed {
                return "<prompt>".to_owned();
            }
            let units: Vec<u16> = argument.encode_utf16().collect();
            if units.len() <= MAX_ARGUMENT_LENGTH {
                argument.clone()
            } else {
                format!("{}…", String::from_utf16_lossy(&units[..MAX_ARGUMENT_LENGTH - 1]))
            }
        })
        .collect();
    if args.len() > MAX_ARGUMENTS {
        kept.push(format!("… {} more", args.len() - MAX_ARGUMENTS));
    }
    kept
}

pub struct Continuation {
    pub mission_id: String,
    pub thread: Option<String>,
}
pub struct LedgerWriter {
    file: File,
    pub path: PathBuf,
    pub mission: String,
    pub run: String,
    pub route: String,
    pub resolved_route: String,
    runtime: String,
    /// Next `event.sequence` (1-based). Approvals do not consume this.
    sequence: u64,
    /// Next `ledgerSequence` (created is 1; next record is 2).
    ledger_seq: u64,
    poisoned: bool,
    bytes: u64,
    /// A teammate's keys given to this run: their values are taken out of every record before it is written.
    keys: Vec<crate::teammate_keys::RunKey>,
    /// Called once, the first time the runtime is seen to begin (`began`): what waits on the run being live.
    on_began: Vec<Box<dyn FnOnce() + Send>>,
}

/// An event that says the runtime really began, not one that only reports why it could not (codex-mission.ts marks a
/// relayed message delivered on the run's start, antigravity-mission.ts on its first step; a diagnostic is neither).
pub fn began(kind: &str) -> bool {
    kind == "run.started" || kind.starts_with("step.") || kind.starts_with("tool.")
}

/// Electron's `schemaVersionWritten(runtime)`.
/// The part of mission-store's `parsedEvent` a native writer can get wrong: a message's operation is one the reader
/// knows (`append` or `replace`, never a word nobody defined), its `itemId` is nonblank, and a tool call's helper
/// (`parentItemId`, v22) names a row -- absent, or text of 1 to 512 characters, never empty, so a reader never has to
/// guess at one.
pub fn readable_event(event: &Value) -> bool {
    let payload = &event["payload"];
    match event["type"].as_str().unwrap_or("") {
        "message.delta" => {
            matches!(payload["operation"].as_str(), Some("append" | "replace"))
                && payload
                    .get("itemId")
                    .is_none_or(|id| id.as_str().is_some_and(|s| !s.trim().is_empty() && s.encode_utf16().count() <= 512))
        }
        "tool.started" | "tool.completed" | "tool.failed" => match payload.get("parentItemId") {
            None => true,
            Some(parent) => parent.as_str().is_some_and(|p| !p.trim().is_empty() && p.encode_utf16().count() <= 512),
        },
        _ => true,
    }
}

/// Append a `mission.host_failure` record to a ledger (`mission-store` `appendHostFailure`).
pub fn append_host_failure(path: &std::path::Path, code: &str, message: &str, occurred_at: &str) -> io::Result<()> {
    if code != "runtime-start-failed" && code != "runtime-transport-failed" {
        return Err(io::Error::other("Host failure code is invalid"));
    }
    require_text(&json!(message), "Host failure message", 1_024)?;
    require_timestamp(&json!(occurred_at), "Host failure timestamp")?;
    LEDGER_TAIL.run(|| append_host_failure_after_tail(path, code, message, occurred_at))
}

fn append_host_failure_after_tail(path: &std::path::Path, code: &str, message: &str, occurred_at: &str) -> io::Result<()> {
    use std::io::Write;
    let text = std::fs::read_to_string(path)?;
    if !text.is_empty() && !text.ends_with('\n') {
        return Err(io::Error::other("Mission ledger is unavailable"));
    }
    let read = crate::ledger::read_ledger(path)?;
    if read.unreadable || !read.issues.is_empty() {
        return Err(io::Error::other("Mission ledger is unavailable"));
    }
    let records: Vec<Value> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let first = records.first().ok_or_else(|| io::Error::other("Ledger has no first record"))?;
    let schema = first["schemaVersion"].as_u64().ok_or_else(|| io::Error::other("Ledger has no schemaVersion"))?;
    let next = records.iter().filter_map(|r| r["ledgerSequence"].as_u64()).max().unwrap_or(0) + 1;
    after_tail();
    let record = json!({
        "schemaVersion": schema,
        "recordType": "mission.host_failure",
        "ledgerSequence": next,
        "occurredAt": occurred_at,
        "failure": {
            "code": code,
            "message": message,
            "occurredAt": occurred_at,
        }
    });
    let mut line = serde_json::to_vec(&record)?;
    line.push(b'\n');
    let mut file = std::fs::OpenOptions::new().append(true).open(path)?;
    file.write_all(&line)?;
    file.sync_all()
}

pub fn schema_written(runtime: &str) -> u64 {
    if runtime == "apple" { 28 } else { 27 }
}

impl LedgerWriter {
    /// The record version this run writes, as Electron's `schemaVersionWritten`: 28 for an Apple Intelligence run,
    /// whose records a reader before 28 cannot know, and 27 for every other run, so older readers keep reading them.
    fn schema(&self) -> u64 {
        schema_written(&self.runtime)
    }

    /// Run `then` once, when this run's runtime first begins; never when it is refused before it does.
    pub fn when_began(&mut self, then: Box<dyn FnOnce() + Send>) {
        self.on_began.push(then);
    }
    /// Take these keys' values out of everything this writer writes from now on (teammate_keys, scrubKeyValues).
    pub fn take_out(&mut self, keys: Vec<crate::teammate_keys::RunKey>) {
        self.keys = keys;
    }
    pub fn create(
        store: &Store,
        id: &str,
        prompt: &str,
        model: &str,
        executable: &str,
    ) -> io::Result<Self> {
        Self::create_continuing(store, id, prompt, model, executable, None)
    }
    pub fn create_continuing(
        store: &Store,
        id: &str,
        prompt: &str,
        model: &str,
        executable: &str,
        continues: Option<&Continuation>,
    ) -> io::Result<Self> {
        Self::create_runtime(
            store,
            id,
            prompt,
            model,
            executable,
            "codex",
            &["app-server".into()],
            continues,
        )
    }
    /// New mission for any runtime (`codex`, `claude`, …). Args are the CLI argv after the executable.
    pub fn create_runtime(
        store: &Store,
        id: &str,
        prompt: &str,
        model: &str,
        executable: &str,
        runtime: &str,
        args: &[String],
        continues: Option<&Continuation>,
    ) -> io::Result<Self> {
        Self::create_runtime_with(store, id, prompt, model, executable, runtime, args, continues, &Value::Null)
    }

    /// `create_runtime`, with more of the turn recorded in its `mission.created` metadata: `extra`'s keys (the mode and
    /// sandbox it really runs in, who started it) laid over the defaults, and `cwd` put inside `command`, as Electron's
    /// host records where a turn stood.
    #[allow(clippy::too_many_arguments)]
    pub fn create_runtime_with(
        store: &Store,
        id: &str,
        prompt: &str,
        model: &str,
        executable: &str,
        runtime: &str,
        args: &[String],
        continues: Option<&Continuation>,
        extra: &Value,
    ) -> io::Result<Self> {
        if prompt.trim().is_empty() || prompt.contains('\0') || prompt.encode_utf16().count() > 8000
        {
            return Err(io::Error::other("Prompt must be nonempty text without NUL"));
        }
        if model.is_empty() || model.len() > 250 || model.contains('\0') {
            return Err(io::Error::other("Invalid model ID"));
        }
        if !matches!(
            runtime,
            "codex"
                | "claude"
                | "gemini"
                | "acp"
                | "opencode"
                | "kilo"
                | "cursor"
                | "copilot"
                | "antigravity"
                | "muse"
                | "apple"
        ) {
            return Err(io::Error::other("Unsupported runtime"));
        }
        // Validated before anything is created: a bad link leaves no empty ledger.
        if let Some(c) = continues {
            let safe = |v: &str| {
                !v.is_empty()
                    && v.len() <= 128
                    && v.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            };
            if !safe(&c.mission_id)
                || c.thread
                    .as_deref()
                    .is_some_and(|t| t.is_empty() || t.len() > 2048)
            {
                return Err(io::Error::other("Invalid continuation"));
            }
        }
        let route = format!("{runtime}:{model}");
        let resolved_route = extra
            .get("resolvedRouteId")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("{route}-account:default"));
        let run = format!("run_{id}");
        let at = now();
        let continuation = continues.map(|c| {
            let mut link =
                json!({"missionId":c.mission_id,"checkpointEpoch":1,"reason":"follow-up"});
            if let Some(thread) = &c.thread {
                link["runtimeThreadId"] = json!(thread);
            }
            link
        });
        let mut record = json!({"schemaVersion":schema_written(runtime),"recordType":"mission.created","ledgerSequence":1,"occurredAt":at,"metadata":{
            "missionId":id,"runId":run,"prompt":prompt,"runtime":runtime,"model":model,
            "requestedRouteId":route,"resolvedRouteId":resolved_route,"cliVersion":null,"workspaceId":"native",
            "sandbox":"read-only","mode":"ask","executionPolicyVersion":1,"createdAt":at,
            "command":{"executablePath":executable,"args":recordable_args(args, prompt)}
        }});
        if let Some(link) = continuation {
            record["metadata"]["continuesFrom"] = link;
        }
        if let Some(fields) = extra.as_object() {
            for (key, value) in fields {
                if key == "cwd" {
                    record["metadata"]["command"]["cwd"] = value.clone();
                } else {
                    record["metadata"][key] = value.clone();
                }
            }
        }
        crate::ledger::validate_metadata_fields(&record["metadata"]).map_err(io::Error::other)?;
        let path = store.write_path(id)?;
        // The directory is created and walked without following any symlink (`Store::create_new_file_in`), so the
        // ancestor race closes: no file is ever created through a swapped component.
        let file = open_checked(store, &path)?;
        let mut writer = Self {
            file,
            path,
            mission: id.into(),
            run,
            route,
            resolved_route,
            runtime: runtime.into(),
            sequence: 0,
            ledger_seq: 2,
            poisoned: false,
            bytes: 0,
            keys: Vec::new(),
            on_began: Vec::new(),
        };
        writer.line(record)?;
        if let Ok(dir) = File::open(
            writer
                .path
                .parent()
                .ok_or_else(|| io::Error::other("Ledger path has no parent directory"))?,
        ) {
            let _ = dir.sync_all();
        }
        Ok(writer)
    }
    fn line(&mut self, value: Value) -> io::Result<()> {
        self.lines(vec![value])
    }
    /// Records in one write and one sync (a streamed batch, long_answer_batch.rs); each keeps the one-record limit.
    fn lines(&mut self, values: Vec<Value>) -> io::Result<()> {
        if self.poisoned {
            return Err(io::Error::other(
                // mission-store's words for a ledger it will not extend.
                "Mission ledger is unavailable",
            ));
        }
        let mut bytes = Vec::new();
        for value in values {
            let value = if self.keys.is_empty() { value } else { crate::teammate_keys::scrub_key_values(&value, &self.keys) };
            let mut one = serde_json::to_vec(&value)?;
            one.push(b'\n');
            if one.len() > 512 * 1024 || self.bytes + (bytes.len() + one.len()) as u64 > 64 * 1024 * 1024 {
                return Err(io::Error::other(
                    "Ledger exceeds Electron's record/file safety limit",
                ));
            }
            bytes.extend(one);
        }
        if self.file.metadata()?.len() != self.bytes {
            self.poisoned = true;
            return Err(io::Error::other("Mission ledger changed outside the active writer"));
        }
        let result = self
            .file
            .write_all(&bytes)
            .and_then(|_| self.file.sync_all());
        if result.is_err() {
            self.poisoned = true;
        }
        if result.is_ok() {
            self.bytes += bytes.len() as u64;
        }
        result
    }
    pub fn append(
        &mut self,
        kind: &str,
        payload: Value,
        thread: Option<&str>,
    ) -> io::Result<Value> {
        let seq = self.sequence + 1;
        let ledger = self.ledger_seq;
        let at = now();
        let mut event = json!({"id":format!("{}:app:{seq}",self.run),"runId":self.run,"missionId":self.mission,
            "sequence":seq,"occurredAt":at,"sourceAdapter":self.runtime,"requestedRouteId":self.route,"resolvedRouteId":self.resolved_route});
        if let Some(thread) = thread {
            event["runtimeThreadId"] = json!(thread);
        }
        event["type"] = json!(kind);
        event["payload"] = payload;
        // Kept shortened rather than a record too large to write (mission-store fitEventToLedger).
        let event = crate::ledger_fit::fit_event(event);
        // Writer strictness is defined by the reader (mission-store appendEvents): a record recovery would stop at is
        // never written, or every later record of the mission would be lost with it.
        if !readable_event(&event) {
            return Err(io::Error::other("Mission event is not readable by the ledger reader"));
        }
        self.line(json!({"schemaVersion":self.schema(),"recordType":"mission.event","ledgerSequence":ledger,"occurredAt":at,"event":event}))?;
        self.sequence = seq;
        self.ledger_seq = ledger + 1;
        if began(kind) {
            for then in std::mem::take(&mut self.on_began) {
                then();
            }
        }
        Ok(event)
    }
    /// A streamed batch (mission-store `appendEvents`): each event its own record, as `append` makes it, and all of
    /// them in one write and one sync, so the batch is on disk whole before anything shows it.
    pub fn append_all(&mut self, events: &[crate::runtime::Event]) -> io::Result<Vec<Value>> {
        let (first_seq, first_ledger) = (self.sequence, self.ledger_seq);
        let mut written = Vec::new();
        let mut records = Vec::new();
        for (i, one) in events.iter().enumerate() {
            let (seq, ledger, at) = (first_seq + 1 + i as u64, first_ledger + i as u64, now());
            let mut event = json!({"id":format!("{}:app:{seq}",self.run),"runId":self.run,"missionId":self.mission,
                "sequence":seq,"occurredAt":at,"sourceAdapter":self.runtime,"requestedRouteId":self.route,"resolvedRouteId":self.resolved_route});
            if let Some(thread) = &one.thread {
                event["runtimeThreadId"] = json!(thread);
            }
            event["type"] = json!(one.kind);
            event["payload"] = one.payload.clone();
            let event = crate::ledger_fit::fit_event(event);
            if !readable_event(&event) {
                return Err(io::Error::other("Mission event is not readable by the ledger reader"));
            }
            records.push(json!({"schemaVersion":self.schema(),"recordType":"mission.event","ledgerSequence":ledger,"occurredAt":at,"event":event}));
            written.push(event);
        }
        self.lines(records)?;
        self.sequence = first_seq + events.len() as u64;
        self.ledger_seq = first_ledger + events.len() as u64;
        if events.iter().any(|e| began(&e.kind)) {
            for then in std::mem::take(&mut self.on_began) {
                then();
            }
        }
        Ok(written)
    }
    /// `mission-store` appendApproval: a card the person answered (schema ≥ 20).
    pub fn append_approval(&mut self, approval: &Value) -> io::Result<()> {
        validate_approval(approval)?;
        let ledger = self.ledger_seq;
        let at = approval["occurredAt"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(now);
        self.line(json!({
            "schemaVersion": self.schema(),
            "recordType": "mission.approval",
            "ledgerSequence": ledger,
            "occurredAt": at,
            "approval": approval,
        }))?;
        self.ledger_seq = ledger + 1;
        Ok(())
    }

    pub fn write_peer_link(
        &mut self,
        direction: &str,
        message_id: &str,
        peer_teammate_id: &str,
        occurred_at: &str,
    ) -> io::Result<()> {
        let ledger = self.ledger_seq;
        self.line(json!({
            "schemaVersion": self.schema(),
            "recordType": "mission.peer",
            "ledgerSequence": ledger,
            "occurredAt": occurred_at,
            "link": {
                "direction": direction,
                "messageId": message_id,
                "peerTeammateId": peer_teammate_id,
                "occurredAt": occurred_at,
            }
        }))?;
        self.ledger_seq = ledger + 1;
        Ok(())
    }
}

/// Append `mission.peer` link records to a finished turn's ledger (`peer-exchange.ts` / `mission-store`).
pub fn append_peer_links(path: &std::path::Path, links: &[(&str, &str, &str, &str)]) -> io::Result<()> {
    LEDGER_TAIL.run(|| append_peer_links_after_tail(path, links))
}

fn append_peer_links_after_tail(path: &std::path::Path, links: &[(&str, &str, &str, &str)]) -> io::Result<()> {
    if links.is_empty() {
        return Ok(());
    }
    // mission-store's validatePeerLink, before the file is read.
    for (direction, message_id, peer_teammate_id, occurred_at) in links {
        if *direction != "received" && *direction != "posted" {
            return Err(io::Error::other("Peer link direction is invalid"));
        }
        if !crate::trash::is_safe_id(message_id) {
            return Err(io::Error::other("messageId is invalid"));
        }
        if !crate::trash::is_safe_id(peer_teammate_id) {
            return Err(io::Error::other("peerTeammateId is invalid"));
        }
        require_timestamp(&json!(occurred_at), "Peer link timestamp")?;
    }
    use std::io::Write;
    let text = std::fs::read_to_string(path)?;
    if !text.is_empty() && !text.ends_with('\n') {
        return Err(io::Error::other("Ledger does not end on a whole record"));
    }
    let records: Vec<Value> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
    let first = records.first().ok_or_else(|| io::Error::other("Ledger has no first record"))?;
    let schema = first["schemaVersion"].as_u64().unwrap_or(27);
    if schema < 5 {
        // The file's version is fixed by its header: a pre-v5 mission cannot take a record its own readers stop at.
        return Err(io::Error::other("Mission ledger version cannot hold peer links"));
    }
    let mut ledger_seq = records.iter().filter_map(|r| r["ledgerSequence"].as_u64()).max().unwrap_or(0);
    after_tail();
    let mut file = std::fs::OpenOptions::new().append(true).open(path)?;
    for (direction, message_id, peer_teammate_id, occurred_at) in links {
        ledger_seq += 1;
        let record = json!({
            "schemaVersion": schema,
            "recordType": "mission.peer",
            "ledgerSequence": ledger_seq,
            "occurredAt": occurred_at,
            "link": {
                "direction": direction,
                "messageId": message_id,
                "peerTeammateId": peer_teammate_id,
                "occurredAt": occurred_at,
            }
        });
        let mut line = serde_json::to_vec(&record)?;
        line.push(b'\n');
        file.write_all(&line)?;
    }
    file.sync_all()
}

// Keep opening and descriptor validation together: no ledger byte precedes this check. The file is created by
// `openat` relative to a directory descriptor walked without following symlinks, so nothing is created through a
// swapped path component; `check_open_file` then confirms the descriptor's own location (defence in depth).
fn open_checked(store: &Store, path: &std::path::Path) -> io::Result<File> {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name().and_then(|n| n.to_str())) else {
        return Err(io::Error::other("Invalid ledger path"));
    };
    let file = store.create_new_file_in(dir, name)?;
    store.check_open_file(&file)?;
    Ok(file)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    fn temp(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("locust-open-{tag}-{}", std::process::id()))
    }
    #[test]
    fn swapped_ancestor_is_refused_and_nothing_is_created_in_electrons_folder() {
        let temp = temp("race");
        let _ = std::fs::remove_dir_all(&temp);
        let native = temp.join("native");
        let electron = temp.join("electron");
        std::fs::create_dir_all(native.join("mission-ledger")).unwrap();
        std::fs::create_dir_all(electron.join("mission-ledger")).unwrap();
        let store = Store::new(native.clone(), electron.clone()).unwrap();
        // The path was checked while it was good ...
        let checked = store.write_path("race").unwrap();
        // ... then an ancestor is swapped for a symlink into Electron's folder.
        std::fs::rename(&native, temp.join("old-native")).unwrap();
        std::os::unix::fs::symlink(&electron, &native).unwrap();
        let error = open_checked(&store, &checked).unwrap_err().to_string();
        assert!(
            error.contains("nothing was created") || error.contains("Refusing"),
            "{error}"
        );
        let leaked = electron.join("mission-ledger/race.jsonl");
        assert!(
            !leaked.exists(),
            "nothing may be created inside Electron's folder"
        );
        assert_eq!(
            std::fs::read_dir(electron.join("mission-ledger"))
                .unwrap()
                .count(),
            0
        );
        std::fs::remove_dir_all(temp).unwrap();
    }
    #[test]
    fn a_symlink_in_the_final_directory_or_the_file_name_is_refused_and_creates_nothing() {
        let temp = temp("final");
        let _ = std::fs::remove_dir_all(&temp);
        let native = temp.join("native");
        let electron = temp.join("electron");
        std::fs::create_dir_all(&native).unwrap();
        std::fs::create_dir_all(electron.join("mission-ledger")).unwrap();
        let store = Store::new(native.clone(), electron.clone()).unwrap();
        // mission-ledger itself is a symlink into Electron's folder.
        let checked = store.write_path("x").unwrap();
        std::os::unix::fs::symlink(
            electron.join("mission-ledger"),
            native.join("mission-ledger"),
        )
        .unwrap();
        assert!(open_checked(&store, &checked).is_err());
        assert_eq!(
            std::fs::read_dir(electron.join("mission-ledger"))
                .unwrap()
                .count(),
            0
        );
        // A final-component symlink (even a dangling one) is never followed or created through.
        std::fs::remove_file(native.join("mission-ledger")).unwrap();
        std::fs::create_dir_all(native.join("mission-ledger")).unwrap();
        let target = electron.join("mission-ledger/victim.jsonl");
        std::os::unix::fs::symlink(&target, native.join("mission-ledger/link.jsonl")).unwrap();
        assert!(open_checked(&store, &native.join("mission-ledger/link.jsonl")).is_err());
        assert!(!target.exists());
        // A good path still works, makes missing directories with mode 0700, and refuses a second create.
        let good = store.write_path("good").unwrap();
        std::fs::remove_dir_all(native.join("mission-ledger")).unwrap();
        let file = open_checked(&store, &good).unwrap();
        drop(file);
        assert!(open_checked(&store, &good).is_err(), "exclusive");
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(native.join("mission-ledger"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(&good).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let err = LedgerWriter::create_runtime_with(
            &store,
            "m_bad",
            "p",
            "account-default",
            "codex",
            "codex",
            &[],
            None,
            &json!({"mode": "whatever"}),
        );
        assert!(err.is_err());
        let w = LedgerWriter::create_runtime_with(
            &store,
            "m_ok",
            "p",
            "account-default",
            "codex",
            "codex",
            &[],
            None,
            &Value::Null,
        )
        .unwrap();
        let path = w.path.clone();
        drop(w);
        assert!(append_events(&path, |run, mission, _ad, seq, at| {
            vec![json!({"id":format!("{run}:c:{seq}"),"runId":run,"missionId":mission,"sequence":seq,"occurredAt":at,"sourceAdapter":"claude","type":"run.started","payload":{}})]
        })
        .is_err());
        assert!(append_events(&path, |run, mission, ad, seq, at| {
            vec![json!({"id":format!("{run}:c:{seq}"),"runId":run,"missionId":mission,"sequence":seq + 1,"occurredAt":at,"sourceAdapter":ad,"type":"run.started","payload":{}})]
        })
        .is_err());
        assert!(!readable_event(&json!({"type":"message.delta","payload":{"operation":"append","itemId":"   "}})));
        append_host_failure(&path, "runtime-start-failed", "Could not start", "2026-08-31T16:00:00.000Z").unwrap();
        assert!(append_host_failure(&path, "unknown", "Could not start", "2026-08-31T16:00:00.000Z").is_err());
        std::fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"{\"half\":").unwrap();
        assert!(append_events(&path, |run, mission, ad, seq, at| {
            vec![json!({"id":format!("{run}:c:{seq}"),"runId":run,"missionId":mission,"sequence":seq,"occurredAt":at,"sourceAdapter":ad,"type":"run.started","payload":{}})]
        })
        .is_err());
        std::fs::remove_dir_all(temp).unwrap();
    }
}

#[cfg(test)]
#[path = "long_answer_tests.rs"]
mod long_answer_tests;

#[cfg(test)]
#[path = "command_record_tests.rs"]
mod command_record_tests;

#[cfg(test)]
#[path = "one_at_a_time_tests.rs"]
mod one_at_a_time_tests;

#[cfg(test)]
#[path = "ledger_readable_case_tests.rs"]
mod ledger_readable_case_tests;

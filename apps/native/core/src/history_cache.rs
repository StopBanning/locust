//! `(size, mtime)`-cached mission history reader (Electron `main/mission-history.ts`).
//!
//! Avoids re-reading unchanged `.jsonl` ledger files on every history poll while keeping
//! field-for-field parity with `ledger::try_list_recent`, preserving `event_count` and
//! `waiting_question` on light history rows (older than the newest `MAX_FULL_MISSIONS`).

use crate::invariants::lock;
use crate::ledger::{self, PeerLink, Summary};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// How many newest missions carry full events; older rows are light (`events: []`) while
/// keeping their `event_count` and `waiting_question`.
pub const MAX_FULL_MISSIONS: usize = 20;

#[derive(Clone, Debug, PartialEq)]
pub struct HistoryRow {
    pub summary: Summary,
    pub event_count: usize,
    pub events: Vec<Value>,
    pub waiting_question: Option<String>,
}

#[derive(Clone)]
struct CachedFile {
    stamp: (u64, u128),
    /// The run's own lifecycle says it is running; whether it still counts as running is decided at each read,
    /// from the file's time, as `ledger::try_list_recent` decides it.
    running: bool,
    modified: SystemTime,
    summary: Option<Summary>,
    event_count: usize,
    waiting_question: Option<String>,
    events: Option<Vec<Value>>,
    /// What the strict reader found wrong with the file (`ledger::recover`).
    issues: Vec<ledger::Issue>,
    /// No conversation could be read from it at all (mission-store's `unreadableCount` counts these).
    unreadable: bool,
}

/// What a folder's records say about their own damage, counted over every file in it, not only the listed ones.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Damage {
    /// Files no conversation could be read from (a bad header, too large to read, a file that could not be opened).
    pub unreadable: usize,
    /// Conversations kept up to a bad record: listed, with a gap at their end (mission-store counts each one).
    pub incomplete: HashSet<String>,
    /// Those of them whose one fault is a last line with no end, which is also how a turn being written looks for
    /// the moment of a write: `native::list_recent` leaves out the ones this window is writing right now.
    pub cut_tail_only: HashSet<String>,
}

/// Process-wide cache shared by `native::list_recent`.
pub fn global() -> &'static HistoryCache {
    static CACHE: OnceLock<HistoryCache> = OnceLock::new();
    CACHE.get_or_init(HistoryCache::new)
}

#[derive(Default)]
pub struct HistoryCache {
    entries: Mutex<HashMap<PathBuf, CachedFile>>,
}

impl HistoryCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lists up to `limit` recent `Summary` entries from `dir`, re-reading only files whose
    /// `(size, modified)` changed since the last call.
    /// Every conversation id the folder keeps, from file names alone (nothing is read): what a capped list counts
    /// its total from.
    pub fn mission_ids(&self, dir: &Path) -> io::Result<HashSet<String>> {
        let mut ids = HashSet::new();
        for entry in fs::read_dir(dir)? {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if !entry.file_type().is_ok_and(|t| t.is_file()) || path.extension().is_none_or(|e| e != "jsonl") {
                continue;
            }
            if let Some(id) = path.file_stem().and_then(|v| v.to_str()).filter(|id| safe_id(id)) {
                ids.insert(id.to_owned());
            }
        }
        Ok(ids)
    }

    pub fn list_recent_summaries(&self, dir: &Path, limit: usize) -> io::Result<Vec<Summary>> {
        self.list_recent_with_damage(dir, limit).map(|(summaries, _)| summaries)
    }

    /// `list_recent_summaries`, and the folder's `Damage` over every file in it.
    pub fn list_recent_with_damage(&self, dir: &Path, limit: usize) -> io::Result<(Vec<Summary>, Damage)> {
        let (rows, _, damage) = self.read_history_at(dir, limit, SystemTime::now())?;
        Ok((rows.into_iter().map(|r| r.summary).collect(), damage))
    }

    /// Reads mission history from `dir` with `(size, mtime)` caching, keeping full events only
    /// on the newest `MAX_FULL_MISSIONS` rows and preserving `event_count` and `waiting_question`
    /// on light rows. Returns `(rows, mission_ids_read_from_disk)`.
    pub fn read_history_tracked(&self, dir: &Path, limit: usize) -> io::Result<(Vec<HistoryRow>, Vec<String>)> {
        self.read_history_at(dir, limit, SystemTime::now()).map(|(rows, read, _)| (rows, read))
    }

    /// `read_history_tracked` as of `now`, which decides whether a running mission still counts as running.
    fn read_history_at(&self, dir: &Path, limit: usize, now: SystemTime) -> io::Result<(Vec<HistoryRow>, Vec<String>, Damage)> {
        let mut files = Vec::new();
        let mut present = HashSet::new();
        for entry in fs::read_dir(dir)? {
            let Ok(entry) = entry else { continue };
            if !entry.file_type().is_ok_and(|t| t.is_file())
                || entry.path().extension().is_none_or(|e| e != "jsonl")
            {
                continue;
            }
            let path = entry.path();
            let Some(id) = path.file_stem().and_then(|v| v.to_str()).map(str::to_owned) else {
                continue;
            };
            if !safe_id(&id) {
                continue;
            }
            let Ok(meta) = entry.metadata() else { continue };
            let modified = meta.modified().unwrap_or(UNIX_EPOCH);
            let stamp = (
                meta.len(),
                modified.duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos(),
            );
            present.insert(path.clone());
            files.push((modified, path, id, stamp));
        }

        // Forget deleted files so the cache never grows unboundedly.
        {
            let mut cache = lock(&self.entries);
            cache.retain(|p, _| !p.starts_with(dir) || present.contains(p));
        }

        // Chosen as `ledger::try_list_recent` chooses: newest file first, the first `limit` that are conversations.
        files.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        let mut read_ids = Vec::new();
        let mut all_rows: Vec<(HistoryRow, PathBuf)> = Vec::new();
        let mut damage = Damage::default();
        // Every file is counted, past the list's end too: an older file that recovered is never "could not be read",
        // and one that did not is said however old it is. Each is read once, then only when its size or time moves.
        for (modified, path, id, stamp) in files {
            let listing = all_rows.len() < limit;
            let cached = {
                let cache = lock(&self.entries);
                cache.get(&path).filter(|c| c.stamp == stamp).cloned()
            };
            let entry = match cached {
                Some(hit) => hit,
                None => {
                    read_ids.push(id);
                    let mut parsed = parse_file(&path, modified, stamp);
                    if !listing {
                        parsed.events = None;
                    }
                    lock(&self.entries).insert(path.clone(), parsed.clone());
                    parsed
                }
            };
            if entry.unreadable {
                damage.unreadable += 1;
            } else if let Some(summary) = &entry.summary
                && !entry.issues.is_empty()
            {
                damage.incomplete.insert(summary.mission_id.clone());
                if entry.issues == [ledger::Issue::TruncatedTail] {
                    damage.cut_tail_only.insert(summary.mission_id.clone());
                }
            }
            if !listing {
                continue;
            }
            if let Some(mut summary) = entry.summary {
                summary.running = entry.running && fresh(entry.modified, now);
                all_rows.push((
                    HistoryRow {
                        summary,
                        event_count: entry.event_count,
                        events: entry.events.unwrap_or_default(),
                        waiting_question: entry.waiting_question,
                    },
                    path,
                ));
            }
        }

        // Newest updated first, tie-breaking by mission_id (matching Electron & native::list_recent).
        all_rows.sort_by(|(a, _), (b, _)| {
            b.summary
                .updated
                .cmp(&a.summary.updated)
                .then_with(|| a.summary.mission_id.cmp(&b.summary.mission_id))
        });

        // Keep full events only on the newest MAX_FULL_MISSIONS rows; older rows stay light.
        let mut cache = lock(&self.entries);
        let mut out = Vec::with_capacity(all_rows.len());
        for (idx, (mut row, path)) in all_rows.into_iter().enumerate() {
            if idx >= MAX_FULL_MISSIONS {
                row.events.clear();
                if let Some(c) = cache.get_mut(&path) {
                    c.events = None;
                }
            }
            out.push(row);
        }
        Ok((out, read_ids, damage))
    }
}

/// `ledger::fresh`: a running mission whose file has not moved for `ACTIVE_WINDOW` is not running any more.
fn fresh(modified: SystemTime, now: SystemTime) -> bool {
    modified != UNIX_EPOCH && now.duration_since(modified).unwrap_or_default() < ledger::ACTIVE_WINDOW
}

fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}

/// `ledger::parse_peer_link`: a `mission.peer` record's `link`, its time falling back to the record's.
fn peer_link(record: &Value) -> Option<PeerLink> {
    if record["recordType"] != "mission.peer" {
        return None;
    }
    let link = record.get("link")?;
    Some(PeerLink {
        direction: link.get("direction")?.as_str()?.to_owned(),
        message_id: link.get("messageId")?.as_str()?.to_owned(),
        peer_teammate_id: link.get("peerTeammateId")?.as_str()?.to_owned(),
        occurred_at: link
            .get("occurredAt")
            .and_then(Value::as_str)
            .or_else(|| record.get("occurredAt").and_then(Value::as_str))
            .unwrap_or_default()
            .to_owned(),
    })
}

fn parse_file(path: &Path, modified: SystemTime, stamp: (u64, u128)) -> CachedFile {
    let nothing = |issues: Vec<ledger::Issue>, unreadable: bool| CachedFile {
        stamp,
        running: false,
        modified,
        summary: None,
        event_count: 0,
        waiting_question: None,
        events: None,
        issues,
        unreadable,
    };
    // A file gone between listing and reading is simply not there; one that could not be opened is unreadable.
    let read = match ledger::read_ledger(path) {
        Ok(read) => read,
        Err(error) => return nothing(Vec::new(), error.kind() != io::ErrorKind::NotFound),
    };
    if read.unreadable {
        return nothing(read.issues, true);
    }
    let issues = read.issues;
    let records = read.records;
    let Some(m) = records.first().and_then(ledger::metadata) else {
        return nothing(issues, true);
    };
    if m.side || m.started_by_kind.as_deref() == Some("scout") {
        return nothing(Vec::new(), false);
    }

    let mut events_map: BTreeMap<u64, Value> = BTreeMap::new();
    let mut peer_links: Vec<PeerLink> = Vec::new();
    for record in &records {
        if let Some(link) = peer_link(record)
            && !peer_links
                .iter()
                .any(|l: &PeerLink| l.direction == link.direction && l.message_id == link.message_id)
        {
            peer_links.push(link);
        }
        if let Some(event) = record.get("event")
            && let Some(seq) = event.get("sequence").and_then(Value::as_u64)
        {
            events_map.insert(seq, event.clone());
        }
    }

    let ordered_events: Vec<Value> = events_map.into_values().collect();
    let event_count = ordered_events.len();
    let cost = crate::relay::mission_cost_from_events(&ordered_events);
    let mut running = true;
    let mut failed = false;
    let mut completed = false;
    let mut cancelled = false;
    let mut assistant_text = String::new();
    for event in &ordered_events {
        match event.get("type").and_then(Value::as_str) {
            Some("run.started") => {
                running = true;
                failed = false;
                completed = false;
                cancelled = false;
            }
            Some("route.limit_detected") => {
                running = false;
            }
            Some("run.failed") => {
                running = false;
                failed = true;
                completed = false;
                cancelled = false;
            }
            Some("run.completed") => {
                running = false;
                failed = false;
                completed = true;
                cancelled = false;
            }
            Some("run.cancelled") => {
                running = false;
                failed = false;
                completed = false;
                cancelled = true;
            }
            Some("message.delta") => {
                if let Some(p) = event.get("payload")
                    && let Some(t) = p.get("text").and_then(Value::as_str)
                {
                    if p.get("operation").and_then(Value::as_str) == Some("replace") {
                        assistant_text = t.to_owned();
                    } else {
                        assistant_text.push_str(t);
                    }
                }
            }
            _ => {}
        }
    }
    let waiting_question = if completed {
        crate::protocol_tags::parse_decision(&assistant_text).map(|d| d.question)
    } else {
        None
    };

    let updated = ledger::last_activity_at(&m.created_at, ordered_events.iter());
    let first = m.prompt.lines().next().unwrap_or_default().trim();
    let title: String = crate::path_title::path_only_title(first)
        .as_deref()
        .unwrap_or(first)
        .chars()
        .take(60)
        .collect();
    let summary = Summary {
        reported_model: crate::model_alias::reported_model(&ordered_events, &m.model),
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
        running,
        failed,
        run_end: ledger::RunEnd::latest(ordered_events.iter()),
        cancelled,
        updated,
        started_by_kind: m.started_by_kind,
        started_by_hop: m.started_by_hop,
        relay: m.relay,
        cost,
        peer_links,
        continues_reason: m.continues_reason,
        edited: m.edited,
    };

    CachedFile {
        stamp,
        running,
        modified,
        summary: Some(summary),
        event_count,
        waiting_question,
        events: Some(ordered_events),
        issues,
        unreadable: false,
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
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn temp(prefix: &str) -> Temp {
        let p = std::env::temp_dir().join(format!(
            "{prefix}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&p).unwrap();
        Temp(p)
    }

    fn at(minute: usize) -> String {
        format!("2026-09-22T12:{minute:02}:00.000Z")
    }

    fn write_mission(dir: &Path, index: usize) {
        let id = format!("mission_{index:03}");
        let mut f = fs::File::create(dir.join(format!("{id}.jsonl"))).unwrap();
        let meta = json!({
            "schemaVersion": 27,
            "recordType": "mission.created",
            "metadata": {
                "missionId": id,
                "runId": format!("run_{index}"),
                "prompt": format!("Question {index}"),
                "runtime": "opencode",
                "model": "opencode/muse-spark-1.3-contributor-free",
                "requestedRouteId": "opencode",
                "resolvedRouteId": "opencode-account:default",
                "cliVersion": "test",
                "workspaceId": "ws_test",
                "sandbox": "read-only",
                "executionPolicyVersion": 1,
                "createdAt": at(index),
            }
        });
        writeln!(f, "{meta}").unwrap();
        for seq in 1..=2 {
            let ev = json!({
                "schemaVersion": 27,
                "recordType": "mission.event",
                "event": {
                    "id": format!("event_{index}_{seq}"),
                    "runId": format!("run_{index}"),
                    "missionId": id,
                    "sequence": seq,
                    "occurredAt": at(index),
                    "sourceAdapter": "opencode",
                    "type": "message.delta",
                    "payload": {
                        "itemId": format!("answer_{index}"),
                        "operation": "append",
                        "text": format!("part {seq} "),
                        "final": false,
                    }
                }
            });
            writeln!(f, "{ev}").unwrap();
        }
    }

    fn ledger_with_missions() -> Temp {
        let dir = temp("locust-history-cache");
        for index in 1..=25 {
            write_mission(&dir.0, index);
        }
        // A conversation that messaged a teammate carries the link, as the relay writes it.
        let mut f = fs::OpenOptions::new().append(true).open(dir.0.join("mission_002.jsonl")).unwrap();
        let link = json!({
            "schemaVersion": 27,
            "recordType": "mission.peer",
            "occurredAt": at(2),
            "link": { "direction": "posted", "messageId": "wm_1", "peerTeammateId": "tm_pip" }
        });
        writeln!(f, "{link}").unwrap();
        dir
    }

    fn append_event(dir: &Path, index: usize, seq: u64, minute: usize, kind: &str, payload: Value) {
        let id = format!("mission_{index:03}");
        let mut f = fs::OpenOptions::new()
            .append(true)
            .open(dir.join(format!("{id}.jsonl")))
            .unwrap();
        let ev = json!({
            "schemaVersion": 27,
            "recordType": "mission.event",
            "event": {
                "id": format!("event_{index}_{seq}"),
                "runId": format!("run_{index}"),
                "missionId": id,
                "sequence": seq,
                "occurredAt": at(minute),
                "sourceAdapter": "opencode",
                "type": kind,
                "payload": payload,
            }
        });
        writeln!(f, "{ev}").unwrap();
    }

    /// A stopped run is told from a finished one, by the cache and by the whole listing alike: only a run that finished
    /// counts as one a free model answered (a-free-model-that-answered-comes-first).
    #[test]
    fn a_stopped_run_is_marked_cancelled_and_a_finished_one_is_not() {
        let dir = ledger_with_missions();
        append_event(&dir.0, 3, 900, 59, "run.cancelled", json!({}));
        append_event(&dir.0, 4, 900, 59, "run.completed", json!({}));
        let cache = HistoryCache::new();
        let (kept, _) = cache.read_history_tracked(&dir.0, 100).unwrap();
        let whole = ledger::try_list_recent(&dir.0, 100).unwrap();
        for (said, list) in [("cache", kept.iter().map(|r| r.summary.clone()).collect::<Vec<_>>()), ("listing", whole)] {
            let of = |id: &str| list.iter().find(|s| s.mission_id == id).unwrap_or_else(|| panic!("{said}: {id}"));
            assert!(of("mission_003").cancelled && !of("mission_003").failed && !of("mission_003").running, "{said}");
            assert!(!of("mission_004").cancelled, "{said}");
        }
    }

    // ---- Electron main/the-history-reads-only-what-changed.test.ts -------------------------------------------------

    // case: answers exactly what the whole listing answered
    #[test]
    fn answers_exactly_what_the_whole_listing_answered() {
        let dir = ledger_with_missions();
        let cache = HistoryCache::new();
        let (kept, _) = cache.read_history_tracked(&dir.0, 100).unwrap();
        let mut whole = ledger::try_list_recent(&dir.0, 100).unwrap();
        whole.sort_by(|a, b| b.updated.cmp(&a.updated).then_with(|| a.mission_id.cmp(&b.mission_id)));
        let kept_summaries: Vec<Summary> = kept.iter().map(|r| r.summary.clone()).collect();
        // Every field, peer links and running included, on the first read and on the cached one.
        assert_eq!(kept_summaries, whole);
        let (again, _) = cache.read_history_tracked(&dir.0, 100).unwrap();
        assert_eq!(again.iter().map(|r| r.summary.clone()).collect::<Vec<_>>(), whole);
        assert_eq!(
            kept_summaries.iter().find(|s| s.mission_id == "mission_002").map(|s| s.peer_links.len()),
            Some(1)
        );
        // A smaller limit picks the same conversations the whole listing picks.
        let mut few: Vec<String> = cache.list_recent_summaries(&dir.0, 7).unwrap().into_iter().map(|s| s.mission_id).collect();
        let mut few_whole: Vec<String> = ledger::try_list_recent(&dir.0, 7).unwrap().into_iter().map(|s| s.mission_id).collect();
        few.sort();
        few_whole.sort();
        assert_eq!(few, few_whole);
        // Premise: some rows came WITHOUT events, and still count them.
        let light: Vec<&HistoryRow> = kept.iter().filter(|r| r.events.is_empty()).collect();
        assert_eq!(light.len(), 5);
        assert!(light.iter().all(|r| r.event_count > 0));
    }

    #[test]
    fn a_cached_run_stops_counting_as_running_once_its_file_has_been_quiet_for_the_window() {
        let dir = ledger_with_missions();
        append_event(&dir.0, 4, 3, 4, "run.started", json!({}));
        let cache = HistoryCache::new();
        let now = SystemTime::now();
        let running_of = |rows: &[HistoryRow]| rows.iter().find(|r| r.summary.mission_id == "mission_004").map(|r| r.summary.running);
        let (first, _, _) = cache.read_history_at(&dir.0, 100, now).unwrap();
        assert_eq!(running_of(&first), Some(true));
        let (later, reads, _) = cache.read_history_at(&dir.0, 100, now + ledger::ACTIVE_WINDOW + std::time::Duration::from_secs(60)).unwrap();
        assert!(reads.is_empty(), "nothing changed on disk: {reads:?}");
        assert_eq!(running_of(&later), Some(false));
    }

    // case: reads no file twice when nothing changed
    #[test]
    fn reads_no_file_twice_when_nothing_changed() {
        let dir = ledger_with_missions();
        let cache = HistoryCache::new();
        let (_, first_reads) = cache.read_history_tracked(&dir.0, 100).unwrap();
        assert_eq!(first_reads.len(), 25);
        let (_, second_reads) = cache.read_history_tracked(&dir.0, 100).unwrap();
        assert!(second_reads.is_empty(), "unchanged files must not be re-read: {second_reads:?}");
    }

    // case: reads the one file that changed, and the answer carries the change
    #[test]
    fn reads_the_one_file_that_changed_and_the_answer_carries_the_change() {
        let dir = ledger_with_missions();
        let cache = HistoryCache::new();
        cache.read_history_tracked(&dir.0, 100).unwrap();
        append_event(
            &dir.0,
            3,
            3,
            59,
            "message.delta",
            json!({"itemId": "answer_3", "operation": "append", "text": "part 3 ", "final": false}),
        );
        let (after, read_ids) = cache.read_history_tracked(&dir.0, 100).unwrap();
        assert_eq!(read_ids, vec!["mission_003"]);
        assert_eq!(after[0].summary.mission_id, "mission_003");
        assert_eq!(after[0].event_count, 3);
    }

    // case: keeps its question on light history rows on the first and cached reads
    #[test]
    fn keeps_its_question_on_light_history_rows_on_the_first_and_cached_reads() {
        let dir = ledger_with_missions();
        append_event(
            &dir.0,
            1,
            3,
            1,
            "message.delta",
            json!({
                "itemId": "ask",
                "operation": "append",
                "final": true,
                "text": "<locust-ask>\nWhich database?\n- SQLite :: local\n- Postgres :: server\n</locust-ask>"
            }),
        );
        append_event(&dir.0, 1, 4, 1, "run.completed", json!({}));
        let cache = HistoryCache::new();
        for pass in 0..2 {
            let (rows, _) = cache.read_history_tracked(&dir.0, 100).unwrap();
            let old = rows
                .iter()
                .find(|r| r.summary.mission_id == "mission_001")
                .expect("mission_001 present");
            assert!(old.events.is_empty(), "pass {pass}: old mission must be light");
            assert_eq!(
                old.waiting_question.as_deref(),
                Some("Which database?"),
                "pass {pass}: waiting_question must survive"
            );
        }
    }
}

#[cfg(test)]
#[path = "torn_listing_tests.rs"]
mod torn_listing_tests;

#[cfg(test)]
#[path = "unreadable_count_tests.rs"]
mod unreadable_count_tests;

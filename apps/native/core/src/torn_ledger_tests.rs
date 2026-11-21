//! Electron's `main/torn-ledger.test.ts`, the reader half ("physical torn ledger: reader contract"): a ledger read
//! whole keeps exactly the records before its first bad one and names what was wrong (mission-store's
//! `readLedgerFile`). The records are written by hand here, as Electron's are, so a writer bug cannot cancel a
//! reader bug. The fixture is shared with the listing's tests (`torn_listing_tests`, `native/torn_words_tests`).
//!
//! The live `Tail` a running turn is followed with is not this reader: there a half-written last line is the record
//! still arriving. The last test here holds that apart.
use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) const NOW: &str = "2026-09-08T12:00:00.000Z";
pub(crate) const ID: &str = "mission_torn";

pub(crate) fn header() -> Value {
    json!({
        "schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": NOW,
        "metadata": {
            "missionId": ID, "runId": "run_torn", "prompt": "Read the scratch file.", "runtime": "codex",
            "model": "account-default", "requestedRouteId": "codex", "resolvedRouteId": "codex-account:default",
            "cliVersion": "test", "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1,
            "createdAt": NOW
        }
    })
}

pub(crate) fn event(sequence: u64) -> Value {
    json!({
        "id": format!("event_{sequence}"), "runId": "run_torn", "missionId": ID, "sequence": sequence, "occurredAt": NOW,
        "sourceAdapter": "codex", "type": "message.delta",
        "payload": { "itemId": format!("answer_{sequence}"), "operation": "append", "text": format!("record {sequence}"),
            "final": false, "evidence": { "redacted": true } }
    })
}

pub(crate) fn record(sequence: u64) -> Value {
    json!({ "schemaVersion": 27, "recordType": "mission.event", "ledgerSequence": sequence + 1, "occurredAt": NOW,
        "event": event(sequence) })
}

pub(crate) fn line(value: &Value) -> String {
    format!("{value}\n")
}

pub(crate) fn clean() -> String {
    line(&header()) + &line(&record(1)) + &line(&record(2)) + &line(&record(3))
}

pub(crate) fn prefix() -> String {
    line(&header()) + &line(&record(1))
}

/// One of Electron's damaged shapes: the file's text, the issues it must raise, and how many events survive
/// (`None`: no conversation at all).
pub(crate) struct Shape {
    pub text: String,
    pub codes: Vec<Issue>,
    pub surviving: Option<usize>,
    pub oversized: bool,
}

pub(crate) fn shape(name: &str) -> Shape {
    use Issue::*;
    let with = |mut value: Value, key: &str, to: Value| {
        value[key] = to;
        value
    };
    let rest = line(&record(1)) + &line(&record(2)) + &line(&record(3));
    let (text, codes, surviving, oversized) = match name {
        "truncated final JSON" => (prefix() + r#"{"recordType":"mission.event","event":"#, vec![TruncatedTail], Some(1), false),
        "well-formed JSON with invalid record type" => (
            prefix() + &line(&with(record(2), "recordType", json!("not.a.record"))) + &line(&record(3)),
            vec![InvalidRecord],
            Some(1),
            false,
        ),
        "noncontiguous ledgerSequence" => (
            prefix() + &line(&with(record(2), "ledgerSequence", json!(4))) + &line(&record(3)),
            vec![InvalidRecord],
            Some(1),
            false,
        ),
        "malformed body followed by otherwise valid tail" => {
            (prefix() + "{broken\n" + &line(&record(2)) + &line(&record(3)), vec![InvalidRecord], Some(1), false)
        }
        "malformed header" => ("{broken\n".to_owned() + &rest, vec![InvalidRecord], None, false),
        "over MAX_LEDGER_BYTES" => (clean(), vec![FileTooLarge], None, true),
        "well-formed invalid header" => (line(&with(header(), "ledgerSequence", json!(2))) + &rest, vec![InvalidRecord], None, false),
        "truncated header" => (r#"{"schemaVersion":"#.to_owned(), vec![TruncatedTail, InvalidRecord], None, false),
        other => panic!("no shape {other}"),
    };
    Shape { text, codes, surviving, oversized }
}

pub(crate) const RECOVERING: [&str; 4] = [
    "truncated final JSON",
    "well-formed JSON with invalid record type",
    "noncontiguous ledgerSequence",
    "malformed body followed by otherwise valid tail",
];
pub(crate) const UNREADABLE: [&str; 4] = ["malformed header", "over MAX_LEDGER_BYTES", "well-formed invalid header", "truncated header"];

static SEQ: AtomicU64 = AtomicU64::new(0);

/// A folder of its own under the system temp folder, removed when dropped.
pub(crate) struct Folder(pub PathBuf);
impl Folder {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "locust-torn-proof-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    /// Writes `<id>.jsonl` and checks it holds what was meant; an oversized one is grown to 64 MiB and a byte, as
    /// Electron's is (a number of its own, so a limit moved in the reader is caught).
    pub fn write(&self, id: &str, text: &str, oversized: bool) -> PathBuf {
        let path = self.0.join(format!("{id}.jsonl"));
        fs::write(&path, text).unwrap();
        if oversized {
            OpenOptions::new().write(true).open(&path).unwrap().set_len(64 * 1024 * 1024 + 1).unwrap();
            assert_eq!(fs::metadata(&path).unwrap().len(), 64 * 1024 * 1024 + 1);
        } else {
            assert_eq!(fs::read_to_string(&path).unwrap(), text);
        }
        path
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn events_of(records: &[Value]) -> Vec<Value> {
    records.iter().filter(|r| r["recordType"] == "mission.event").map(|r| r["event"].clone()).collect()
}

fn reads_as_electron_says(name: &str) {
    let shape = shape(name);
    let folder = Folder::new();
    let path = folder.write(ID, &shape.text, shape.oversized);
    let read = read_ledger(&path).unwrap();
    assert_eq!(read.issues, shape.codes, "{name}");
    let transcript = read_transcript(&path);
    assert_eq!(transcript.issues, shape.codes, "{name}: the turn read whole says the same");
    match shape.surviving {
        None => {
            assert!(read.unreadable, "{name}");
            assert!(read.records.is_empty(), "{name}");
            assert!(transcript.metadata.is_none(), "{name}: no conversation is made of it");
        }
        Some(surviving) => {
            assert!(!read.unreadable, "{name}");
            let wanted: Vec<Value> = (1..=surviving as u64).map(event).collect();
            assert_eq!(events_of(&read.records), wanted, "{name}: exactly the prefix");
            assert_eq!(read.records.len(), surviving + 1, "{name}: nothing after the first bad record");
            assert_eq!(read.records.last().unwrap()["ledgerSequence"], json!(surviving as u64 + 1), "{name}");
            assert_eq!(transcript.metadata.as_ref().map(|m| m.mission_id.as_str()), Some(ID), "{name}");
            assert!(transcript.peer_links.is_empty(), "{name}");
        }
    }
}

// case: clean control: all records survive with zero snapshot AND mission issues
#[test]
fn a_clean_ledger_keeps_every_record_and_says_nothing_is_wrong() {
    let folder = Folder::new();
    let path = folder.write(ID, &clean(), false);
    let read = read_ledger(&path).unwrap();
    assert_eq!(read.issues, []);
    assert!(!read.unreadable);
    assert_eq!(read.records[0], header());
    assert_eq!(metadata(&read.records[0]).map(|m| m.mission_id), Some(ID.to_owned()));
    assert_eq!(events_of(&read.records), (1..=3).map(event).collect::<Vec<_>>());
    assert_eq!(read.records.last().unwrap()["ledgerSequence"], json!(4));
    let transcript = read_transcript(&path);
    assert_eq!(transcript.issues, []);
    assert_eq!(transcript.error, None);
}

// case: $name: exact prefix and exact issue codes, including rejected tail
#[test]
fn truncated_final_json_keeps_the_prefix_and_says_truncated_tail() {
    reads_as_electron_says("truncated final JSON");
}

// case: $name: exact prefix and exact issue codes, including rejected tail
#[test]
fn an_invalid_record_type_ends_what_is_kept_though_a_good_record_follows() {
    reads_as_electron_says("well-formed JSON with invalid record type");
}

// case: $name: exact prefix and exact issue codes, including rejected tail
#[test]
fn a_noncontiguous_ledger_sequence_ends_what_is_kept() {
    reads_as_electron_says("noncontiguous ledgerSequence");
}

// case: $name: exact prefix and exact issue codes, including rejected tail
#[test]
fn a_malformed_body_line_ends_what_is_kept_though_a_valid_tail_follows() {
    reads_as_electron_says("malformed body followed by otherwise valid tail");
}

// case: $name: exact prefix and exact issue codes, including rejected tail
#[test]
fn a_malformed_header_keeps_no_conversation() {
    reads_as_electron_says("malformed header");
}

// case: $name: exact prefix and exact issue codes, including rejected tail
#[test]
fn a_ledger_over_max_ledger_bytes_is_not_read_and_says_file_too_large() {
    reads_as_electron_says("over MAX_LEDGER_BYTES");
}

// case: $name: exact prefix and exact issue codes, including rejected tail
#[test]
fn a_well_formed_header_out_of_sequence_keeps_no_conversation() {
    reads_as_electron_says("well-formed invalid header");
}

// case: $name: exact prefix and exact issue codes, including rejected tail
#[test]
fn a_truncated_header_says_both_truncated_tail_and_invalid_record() {
    reads_as_electron_says("truncated header");
}

/// mission-store's `MAX_RECORD_BYTES`: a record longer than it is not kept, and nothing after it is.
#[test]
fn a_record_past_max_record_bytes_ends_what_is_kept() {
    let mut big = record(2);
    big["event"]["payload"]["text"] = json!("x".repeat(512 * 1024));
    let folder = Folder::new();
    let path = folder.write(ID, &(prefix() + &line(&big) + &line(&record(3))), false);
    let read = read_ledger(&path).unwrap();
    assert_eq!(read.issues, [Issue::InvalidRecord]);
    assert_eq!(events_of(&read.records), [event(1)]);
    // One just under the limit is kept.
    let mut fits = record(2);
    let room = 512 * 1024 - line(&record(2)).len() - 16;
    fits["event"]["payload"]["text"] = json!(format!("record 2{}", " ".repeat(room)));
    let path = folder.write("mission_fits", &(prefix() + &line(&fits) + &line(&record(3))), false);
    let read = read_ledger(&path).unwrap();
    assert_eq!(read.issues, []);
    assert_eq!(read.records.len(), 4);
}

/// A running turn's file is followed by the live `Tail`, which waits for a half-written last line to be finished
/// and calls nothing torn; only a ledger read whole names the cut.
#[test]
fn the_live_tail_waits_for_a_half_written_last_line_and_calls_nothing_torn() {
    let folder = Folder::new();
    let whole = line(&record(2));
    let (half, other_half) = whole.split_at(whole.len() / 2);
    let path = folder.write(ID, &(prefix() + half), false);
    let mut tail = Tail::new(&path, 0);
    assert_eq!(tail.poll().len(), 2, "the header and the first record");
    assert_eq!(tail.error, None);
    std::io::Write::write_all(&mut OpenOptions::new().append(true).open(&path).unwrap(), other_half.as_bytes()).unwrap();
    let arrived = tail.poll();
    assert_eq!(tail.error, None);
    assert_eq!(arrived.len(), 1, "the record arrives whole once it is finished");
    assert_eq!(arrived[0]["event"]["sequence"], json!(2));
    // Read whole while it was half-written, the same file said what was cut.
    assert_eq!(recover((prefix() + half).as_bytes()).issues, [Issue::TruncatedTail]);
}

/// A record must share its header's schema version (mission-store's reader); one that does not ends what is kept.
#[test]
fn a_record_of_another_schema_version_ends_what_is_kept() {
    let mut other = record(2);
    other["schemaVersion"] = json!(26);
    let read = recover((prefix() + &line(&other) + &line(&record(3))).as_bytes());
    assert_eq!(read.issues, [Issue::InvalidRecord]);
    assert_eq!(events_of(&read.records), [event(1)]);
}

/// mission-store's `MissionLedgerIssueCode`s, spelled as Electron spells them.
#[test]
fn the_issue_codes_are_electrons() {
    let codes = [Issue::TruncatedTail, Issue::InvalidRecord, Issue::FileTooLarge].map(Issue::code);
    assert_eq!(codes, ["truncated-tail", "invalid-record", "file-too-large"]);
}

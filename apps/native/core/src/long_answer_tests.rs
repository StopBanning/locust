//! Electron's `a-long-answer-is-not-too-much-to-record.test.ts` (4 cases). Electron refused a burst of more than a
//! hundred events in one append and threw away a finished run; Rust writes each event as its own line, so a long
//! answer is only ever many small writes, and its one batch path refuses a stranger's event before writing any.
use super::*;

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn writer() -> (Temp, LedgerWriter) {
    let root = std::env::temp_dir().join(format!("locust-long-answer-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("native")).unwrap();
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let w = LedgerWriter::create_runtime(&store, "mission_long", "check robinhood for me, specifically my roth ira, and all associated news", "grok-4.6", "cursor-agent", "cursor", &[], None).unwrap();
    (Temp(root), w)
}

fn delta(w: &mut LedgerWriter, text: &str) {
    w.append("message.delta", json!({"itemId": "item_1", "operation": "append", "text": text, "final": false}), None).unwrap();
}

fn records(path: &std::path::Path) -> Vec<Value> {
    std::fs::read_to_string(path).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn event_sequences(path: &std::path::Path) -> Vec<u64> {
    records(path).iter().filter_map(|r| r.get("event")).filter_map(|e| e["sequence"].as_u64()).collect()
}

// case: accepts a burst of more than a hundred events, which used to be refused
#[test]
fn accepts_a_burst_of_more_than_a_hundred_events() {
    let (_t, mut w) = writer();
    for i in 0..250 {
        delta(&mut w, &format!("fragment {i} "));
    }
    assert_eq!(event_sequences(&w.path), (1..=250).collect::<Vec<_>>());
    let read = crate::ledger::read_transcript(&w.path);
    assert_eq!(read.error, None);
    assert!(!read.failed);
}

// case: writes them as one unbroken run, and the next append continues it
#[test]
fn writes_them_as_one_unbroken_run_and_the_next_append_continues_it() {
    let (_t, mut w) = writer();
    for _ in 0..150 {
        delta(&mut w, "x");
    }
    delta(&mut w, "and one more");
    let seqs = event_sequences(&w.path);
    assert_eq!((seqs.len(), seqs.last().copied()), (151, Some(151)));
    // The ledger's own numbering is unbroken, created included: a gap reads as a torn file.
    let ledger: Vec<u64> = records(&w.path).iter().filter_map(|r| r["ledgerSequence"].as_u64()).collect();
    assert_eq!(ledger, (1..=152).collect::<Vec<_>>());
}

// case: splits on total size too, not only on count
#[test]
fn a_long_answer_by_size_is_recorded_whole() {
    // 200 deltas at their 16KB cap is ~3.2MB, past the 2MB one Electron append took.
    let (_t, mut w) = writer();
    let big = "y".repeat(16 * 1024);
    for _ in 0..200 {
        delta(&mut w, &big);
    }
    assert_eq!(event_sequences(&w.path).len(), 200);
}

// case: still refuses an event it could not read back, before writing any of them
#[test]
fn still_refuses_an_event_it_could_not_read_back_before_writing_any_of_them() {
    let (_t, w) = writer();
    let before = std::fs::read_to_string(&w.path).unwrap();
    let refused = append_events(&w.path, |run, mission, adapter, next, at| {
        let mut burst: Vec<Value> = (0..120)
            .map(|i| json!({"id": format!("{run}:{}", next + i), "runId": run, "missionId": mission, "sequence": next + i, "type": "message.delta", "occurredAt": at, "sourceAdapter": adapter, "payload": {"text": "ok"}}))
            .collect();
        burst.push(json!({"id": "x", "runId": "run_somebody_else", "missionId": mission, "sequence": next + 120, "type": "message.delta", "occurredAt": at, "sourceAdapter": adapter, "payload": {"text": "bad"}}));
        burst
    });
    let message = refused.unwrap_err().to_string();
    assert!(message.contains("correlation"), "{message}");
    assert_eq!(std::fs::read_to_string(&w.path).unwrap(), before, "nothing of the burst is written");
    // The same burst without the stranger is written whole.
    append_events(&w.path, |run, mission, adapter, next, at| {
        (0..120).map(|i| json!({"id": format!("{run}:{}", next + i), "runId": run, "missionId": mission, "sequence": next + i, "type": "message.delta", "occurredAt": at, "sourceAdapter": adapter, "payload": {"text": "ok"}})).collect()
    })
    .unwrap();
    assert_eq!(event_sequences(&w.path).len(), 120);
}

/// a-streamed-batch-is-written-before-it-is-shown.test.ts, the part Rust shares: what the window shows is read from
/// the ledger, and every event is on disk, whole, the moment `append` returns (each is written and synced on its own,
/// so there is no batch to lose in a crash and no timer to time). Electron's batcher cases have no Rust counterpart.
#[test]
fn every_streamed_event_is_on_disk_when_append_returns() {
    let (_t, mut w) = writer();
    for n in 1..=7 {
        delta(&mut w, &format!("word {n} "));
        assert_eq!(event_sequences(&w.path).len(), n, "event {n} is on disk before anything could show it");
    }
}

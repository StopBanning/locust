//! Electron's `one-at-a-time.test.ts` (2 cases). Rust's ledger writers that read the tail `ledgerSequence` and append
//! after it ran with no lock, so two notes at once could write the same sequence; they now pass one gate, in turn.
use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn ledger(tag: &str) -> (Temp, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("locust-one-at-a-time-{tag}-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("native")).unwrap();
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let w = LedgerWriter::create_runtime(&store, "mission_turns", "Write the notes.", "gpt-5.5", "codex", "codex", &[], None).unwrap();
    let path = w.path.clone();
    drop(w);
    (Temp(root), path)
}

fn records(path: &std::path::Path) -> Vec<Value> {
    std::fs::read_to_string(path).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

fn wait_for(gate: &OneAtATime, taken: u64) {
    for _ in 0..2000 {
        if gate.waiting() == taken {
            return;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("the gate never had {taken} calls in it");
}

// case: run one at a time, in order, so no two read the same tail
#[test]
fn run_one_at_a_time_in_order_so_no_two_read_the_same_tail() {
    let gate = Arc::new(OneAtATime::new());
    let record = Arc::new(Mutex::new(Vec::<u32>::new()));
    let in_flight = Arc::new(AtomicUsize::new(0));
    let overlapped = Arc::new(AtomicBool::new(false));
    let mut calls = Vec::new();
    for value in [1u32, 2, 3] {
        let (gate2, record, in_flight, overlapped) = (gate.clone(), record.clone(), in_flight.clone(), overlapped.clone());
        calls.push(std::thread::spawn(move || {
            gate2.run(|| {
                if in_flight.fetch_add(1, Ordering::SeqCst) + 1 > 1 {
                    overlapped.store(true, Ordering::SeqCst);
                }
                let tail = record.lock().unwrap().len();
                std::thread::sleep(Duration::from_millis(if value == 1 { 40 } else { 5 }));
                record.lock().unwrap().insert(tail, value);
                in_flight.fetch_sub(1, Ordering::SeqCst);
            })
        }));
        // Each call is made only once the one before it is in the gate, so the order they were made is 1, 2, 3.
        wait_for(&gate, u64::from(value));
    }
    for call in calls {
        call.join().unwrap();
    }
    assert!(!overlapped.load(Ordering::SeqCst));
    assert_eq!(*record.lock().unwrap(), vec![1, 2, 3]);
    assert_eq!(gate.waiting(), 0);
}

// case: run one at a time, in order, so no two read the same tail
/// The ledger's own writers, all five of them, at once on one file and each pausing between reading the tail and
/// appending: every record has its own `ledgerSequence`, unbroken, and every event its own `sequence`.
#[test]
fn every_ledger_writer_that_reads_the_tail_waits_its_turn() {
    let (_t, path) = ledger("writers");
    let writers: Vec<Box<dyn FnOnce(&std::path::Path) + Send>> = vec![
        Box::new(|p| append_note(p, "host.relay_ended", "The relay ended.").unwrap()),
        Box::new(|p| append_note(p, "host.check_fix", "The check was fixed.").unwrap()),
        Box::new(|p| append_note_with_level(p, "info", "host.carry_on", "Carried on.").unwrap()),
        Box::new(|p| {
            append_events(p, |run, mission, adapter, next, at| {
                vec![json!({"id": format!("{run}:observed:{next}"), "runId": run, "missionId": mission, "sequence": next,
                    "occurredAt": at, "sourceAdapter": adapter, "type": "adapter.diagnostic",
                    "payload": {"level": "info", "code": "host.observed", "message": "Seen.", "terminal": false, "evidence": {"redacted": true}}})]
            })
            .unwrap()
        }),
        Box::new(|p| append_checkpoint(p, &json!({"createdAt": now(), "epoch": 1})).unwrap()),
        Box::new(|p| {
            append_edit_check(p, &json!({"outcome": "passed", "command": "npm test", "newLines": [], "unchanged": false, "first": true, "occurredAt": now()}))
                .unwrap()
        }),
        Box::new(|p| append_peer_links(p, &[("posted", "message_1", "teammate_1", &now())]).unwrap()),
        Box::new(|p| append_note(p, "host.relay_ended", "The relay ended again.").unwrap()),
    ];
    let count = writers.len() as u64;
    let threads: Vec<_> = writers
        .into_iter()
        .map(|write| {
            let path = path.clone();
            std::thread::spawn(move || {
                PAUSE_AFTER_TAIL.with(|pause| pause.set(30));
                write(&path);
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let all = records(&path);
    let ledger: Vec<u64> = all.iter().map(|r| r["ledgerSequence"].as_u64().unwrap()).collect();
    assert_eq!(ledger, (1..=count + 1).collect::<Vec<_>>());
    let events: Vec<u64> = all.iter().filter_map(|r| r.get("event")).map(|e| e["sequence"].as_u64().unwrap()).collect();
    assert_eq!(events, (1..=events.len() as u64).collect::<Vec<_>>());
    assert_eq!(events.len(), 5);
    assert_eq!(crate::ledger::read_transcript(&path).error, None);
}

// case: keeps going after one fails
#[test]
fn keeps_going_after_one_fails() {
    let gate = OneAtATime::new();
    let seen = Mutex::new(Vec::new());
    let write = |value: u32| -> io::Result<()> {
        gate.run(|| {
            if value == 1 {
                return Err(io::Error::other("refused"));
            }
            seen.lock().unwrap().push(value);
            Ok(())
        })
    };
    assert_eq!(write(1).unwrap_err().to_string(), "refused");
    write(2).unwrap();
    assert_eq!(*seen.lock().unwrap(), vec![2]);
}

// case: keeps going after one fails
/// A call that panics does not take the gate down with it: the next call still runs, and the gate is empty after.
#[test]
fn keeps_going_after_one_panics() {
    let gate = Arc::new(OneAtATime::new());
    let gate2 = gate.clone();
    let panicked = std::thread::spawn(move || gate2.run(|| panic!("refused"))).join();
    assert!(panicked.is_err());
    let after = Arc::new(Mutex::new(Vec::new()));
    let (gate3, after2) = (gate.clone(), after.clone());
    let next = std::thread::spawn(move || gate3.run(|| after2.lock().unwrap().push(2)));
    for _ in 0..2000 {
        if next.is_finished() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(next.is_finished(), "the call after a panic never ran");
    next.join().unwrap();
    assert_eq!(*after.lock().unwrap(), vec![2]);
    assert_eq!(gate.waiting(), 0);
}

// case: keeps going after one fails
/// On the ledger: a note refused (no file) lets the next note in, and it is written.
#[test]
fn a_refused_note_lets_the_next_one_in() {
    let (_t, path) = ledger("refused");
    let missing = path.with_file_name("mission_missing.jsonl");
    assert!(append_note(&missing, "host.relay_ended", "Nowhere.").is_err());
    append_note(&path, "host.relay_ended", "Here.").unwrap();
    let all = records(&path);
    assert_eq!(all.last().unwrap()["event"]["payload"]["message"], "Here.");
    assert_eq!(all.last().unwrap()["ledgerSequence"], 2);
}

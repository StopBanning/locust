//! Electron's `main/an-unwritten-card-answer-is-said-in-the-record.test.ts`: the note for an answer that could not be
//! written down, and the run-end step that writes a refused answer after all or says so. The host's case (a real run
//! whose card's answer the ledger refused) is in `native/unwritten_answer_tests.rs`.
use super::*;

fn approval() -> Value {
    json!({"approvalId": "ap_1", "kind": "command", "asked": "Run a command\nnpm test", "answer": "allowed", "by": "card",
        "askedAt": "2026-10-04T11:59:00.000Z", "occurredAt": "2026-10-04T12:00:00.000Z"})
}

fn mission() -> Held {
    Held { last_sequence: Some(7), run_id: "run_1".into(), runtime: "codex".into() }
}

const NOW: &str = "2026-10-04T12:00:00.000Z";

// case: names the card, the answer and the ledger's reason, one past the last sequence
#[test]
fn names_the_card_the_answer_and_the_ledgers_reason_one_past_the_last_sequence() {
    let note = approval_not_recorded_note(&mission(), "run_1", "mission_1", "command", "allowed", "EACCES: permission denied, open ledger\nat Object.openSync", NOW).unwrap();
    assert_eq!(note["id"], "run_1:approval-not-recorded:8");
    assert_eq!((note["runId"].as_str(), note["missionId"].as_str(), note["sequence"].as_u64()), (Some("run_1"), Some("mission_1"), Some(8)));
    assert_eq!((note["type"].as_str(), note["occurredAt"].as_str(), note["sourceAdapter"].as_str()), (Some("adapter.diagnostic"), Some(NOW), Some("codex")));
    assert_eq!(
        note["payload"],
        json!({"level": "warning", "code": APPROVAL_NOT_RECORDED, "message": "The answer to a card (command, allowed) could not be written to this record: EACCES: permission denied, open ledger", "terminal": false, "evidence": {"redacted": true}})
    );
    // A reason with no words of its own is still said.
    let blank = approval_not_recorded_note(&mission(), "run_1", "mission_1", "command", "allowed", "  \nat x", NOW).unwrap();
    assert!(blank["payload"]["message"].as_str().unwrap().ends_with("could not be written to this record: the ledger refused the write"));
}

// case: leaves nothing on a turn too old to hold cards (control), and starts at 1 on an empty mission
#[test]
fn leaves_nothing_on_a_turn_too_old_to_hold_cards_control_and_starts_at_1_on_an_empty_mission() {
    assert_eq!(approval_not_recorded_note(&mission(), "run_1", "mission_1", "command", "denied", "Mission ledger version cannot hold approvals", NOW), None);
    let fresh = Held { last_sequence: None, run_id: "run_2".into(), runtime: "claude".into() };
    let note = approval_not_recorded_note(&fresh, "run_2", "mission_2", "question", "answered", "disk full", NOW).unwrap();
    assert_eq!((note["sequence"].as_u64(), note["sourceAdapter"].as_str()), (Some(1), Some("claude")));
}

/// A ledger that refuses the answer's record a set number of times, and holds the mission's tail.
struct Fake {
    refusals: usize,
    tail_taken_once: bool,
    tail: u64,
    reads: usize,
    approvals: Vec<Value>,
    events: Vec<Value>,
}

fn fake(refusals: usize, tail_taken_once: bool) -> Fake {
    Fake { refusals, tail_taken_once, tail: 7, reads: 0, approvals: Vec::new(), events: Vec::new() }
}

impl AnswerLedger for Fake {
    fn append_approval(&mut self, _: &str, approval: &Value) -> Result<(), String> {
        if self.refusals > 0 {
            self.refusals -= 1;
            return Err("EBUSY: resource busy or locked".into());
        }
        self.approvals.push(approval.clone());
        Ok(())
    }
    fn get_mission(&mut self, _: &str) -> Option<Held> {
        self.reads += 1;
        Some(Held { last_sequence: Some(self.tail), run_id: "run_1".into(), runtime: "codex".into() })
    }
    fn append_events(&mut self, _: &str, events: &[Value]) -> Result<(), String> {
        if self.tail_taken_once {
            // Another run-end writer appended first: the tail moved on, the ledger refused.
            self.tail_taken_once = false;
            self.tail += 1;
            return Err("Mission event sequence is invalid".into());
        }
        self.events.extend(events.iter().cloned());
        Ok(())
    }
}

fn ended(unwritten: &mut UnwrittenAnswers, mission_id: &str, ledger: &mut Fake, now: &str) -> Result<(), String> {
    let now = now.to_owned();
    match unwritten.take(mission_id) {
        Some(list) => write_at_run_end(list, mission_id, ledger, &move || now.clone()),
        None => Ok(()),
    }
}

// case: is written after all when the ledger takes it the second time, and no note is left
#[test]
fn is_written_after_all_when_the_ledger_takes_it_the_second_time_and_no_note_is_left() {
    let mut f = fake(1, false);
    let mut unwritten = UnwrittenAnswers::new();
    assert_eq!(f.append_approval("mission_1", &approval()).unwrap_err(), "EBUSY: resource busy or locked");
    unwritten.remember("mission_1", approval(), "EBUSY: resource busy or locked");
    assert_eq!(unwritten.pending("mission_1"), 1);
    ended(&mut unwritten, "mission_1", &mut f, NOW).unwrap();
    assert_eq!(f.approvals, vec![approval()]);
    assert!(f.events.is_empty());
    assert_eq!(unwritten.pending("mission_1"), 0);
}

// case: still refused, leaves the note one past the run's last event, with the ledger's latest words
#[test]
fn still_refused_leaves_the_note_one_past_the_runs_last_event_with_the_ledgers_latest_words() {
    let mut f = fake(5, false);
    let mut unwritten = UnwrittenAnswers::new();
    unwritten.remember("mission_1", approval(), "disk full");
    let mut second = approval();
    second["approvalId"] = json!("ap_2");
    second["kind"] = json!("file-change");
    second["answer"] = json!("denied");
    unwritten.remember("mission_1", second, "disk full");
    ended(&mut unwritten, "mission_1", &mut f, "2026-10-04T12:05:00.000Z").unwrap();
    assert!(f.approvals.is_empty());
    let said: Vec<(u64, String)> = f.events.iter().map(|e| (e["sequence"].as_u64().unwrap(), e["payload"]["message"].as_str().unwrap().to_owned())).collect();
    assert_eq!(
        said,
        vec![
            (8, "The answer to a card (command, allowed) could not be written to this record: EBUSY: resource busy or locked".to_owned()),
            (8, "The answer to a card (file-change, denied) could not be written to this record: EBUSY: resource busy or locked".to_owned()),
        ]
    );
    let first = &f.events[0];
    assert_eq!((first["runId"].as_str(), first["missionId"].as_str(), first["sourceAdapter"].as_str()), (Some("run_1"), Some("mission_1"), Some("codex")));
    assert_eq!((first["occurredAt"].as_str(), first["payload"]["code"].as_str()), (Some("2026-10-04T12:05:00.000Z"), Some(APPROVAL_NOT_RECORDED)));
}

// case: reads the tail again when another writer took the sequence first
#[test]
fn reads_the_tail_again_when_another_writer_took_the_sequence_first() {
    let mut f = fake(5, true);
    let mut unwritten = UnwrittenAnswers::new();
    unwritten.remember("mission_1", approval(), "disk full");
    ended(&mut unwritten, "mission_1", &mut f, NOW).unwrap();
    assert_eq!(f.reads, 2);
    assert_eq!(f.events.iter().map(|e| e["sequence"].as_u64().unwrap()).collect::<Vec<_>>(), vec![9]);
}

// case: remembers nothing for a turn too old to hold cards, and touches a run with nothing remembered not at all (controls)
#[test]
fn remembers_nothing_for_a_turn_too_old_to_hold_cards_and_touches_a_run_with_nothing_remembered_not_at_all() {
    let mut f = fake(5, false);
    let mut unwritten = UnwrittenAnswers::new();
    unwritten.remember("mission_old", approval(), "Mission ledger version cannot hold approvals");
    assert_eq!(unwritten.pending("mission_old"), 0);
    ended(&mut unwritten, "mission_old", &mut f, NOW).unwrap();
    ended(&mut unwritten, "mission_never", &mut f, NOW).unwrap();
    assert_eq!(f.reads, 0);
    assert!(f.approvals.is_empty() && f.events.is_empty());
}

/// Rust's own: the turn's ledger file, as the run-end step writes to it, refuses as mission-store does (a ledger too old
/// to hold cards, a note out of sequence) and writes nothing then.
#[test]
fn the_turns_own_ledger_file_takes_an_answer_and_a_note_as_mission_store_does() {
    let dir = std::env::temp_dir().join(format!("locust-unwritten-{}-{}", std::process::id(), crate::ledger_write::now().replace([':', '.'], "-")));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("mission_1.jsonl");
    let ledger_of = |schema: u64| {
        let created = json!({"schemaVersion": schema, "recordType": "mission.created", "ledgerSequence": 1, "metadata": {"missionId": "mission_1", "runId": "run_1", "runtime": "codex"}});
        let event = json!({"schemaVersion": schema, "recordType": "mission.event", "ledgerSequence": 2, "event": {"id": "run_1:app:1", "runId": "run_1", "missionId": "mission_1", "sequence": 1, "type": "run.completed", "sourceAdapter": "codex", "payload": {}}});
        std::fs::write(&path, format!("{created}\n{event}\n")).unwrap();
    };
    let mut file = FileLedger { path: path.clone() };
    ledger_of(19);
    assert_eq!(file.append_approval("mission_1", &approval()).unwrap_err(), "Mission ledger version cannot hold approvals");
    ledger_of(27);
    let before = std::fs::read_to_string(&path).unwrap();
    let held = file.get_mission("mission_1").unwrap();
    assert_eq!((held.last_sequence, held.run_id.as_str(), held.runtime.as_str()), (Some(1), "run_1", "codex"));
    let late = approval_not_recorded_note(&Held { last_sequence: Some(0), ..held.clone() }, "run_1", "mission_1", "command", "allowed", "disk full", NOW).unwrap();
    assert_eq!(file.append_events("mission_1", &[late]).unwrap_err(), "Mission event sequence is invalid");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), before, "nothing was written");
    let note = approval_not_recorded_note(&held, "run_1", "mission_1", "command", "allowed", "disk full", NOW).unwrap();
    file.append_events("mission_1", &[note]).unwrap();
    file.append_approval("mission_1", &approval()).unwrap();
    let rows: Vec<Value> = std::fs::read_to_string(&path).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(rows[2]["event"]["sequence"], 2);
    assert_eq!((rows[3]["recordType"].as_str(), rows[3]["ledgerSequence"].as_u64()), (Some("mission.approval"), Some(4)));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keeps_at_most_fifty_answers_a_mission_and_two_hundred_missions() {
    let mut unwritten = UnwrittenAnswers::new();
    for n in 0..60 {
        unwritten.remember("mission_many", json!({"n": n}), "EBUSY");
    }
    assert_eq!(unwritten.pending("mission_many"), 50, "a ledger that refuses everything cannot grow memory without end");
    for m in 0..205 {
        unwritten.remember(&format!("mission_{m}"), json!({}), "EBUSY");
    }
    assert_eq!(unwritten.pending("mission_many"), 0, "the oldest mission goes first");
    assert_eq!(unwritten.pending("mission_204"), 1);
    assert_eq!(unwritten.pending("mission_5"), 1);
    assert_eq!(unwritten.pending("mission_4"), 0);
}

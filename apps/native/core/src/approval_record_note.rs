//! approval-record-note.ts: a card's answer the ledger refused is tried again when the run ends, then said (0.587).
//!
//! Every card answered is written to the turn's ledger as its own record. A write the ledger refused was dropped, so
//! the saved record could read complete when the host knew it was not (QA's Q2). The answer cannot be noted while the
//! run is live (a host note slipped in mid-run would take a number the run's own next event carries), so a refused
//! answer waits for the run to end: the write is tried once more, since a disk that was busy is often free by then;
//! still refused, a note goes after the run's last event, where the export prints it under "How it ended".
//!
//! One exception: a turn written before the ledger held cards (schema < 20) refuses the write by design, and the export
//! already says such a turn cannot show cards; nothing is remembered.
use serde_json::{Value, json};
use std::sync::Mutex;

pub const APPROVAL_NOT_RECORDED: &str = "host.approval-not-recorded";

/// mission-store's words for a ledger too old to hold cards (`CANNOT_HOLD_CARDS`, matched without case).
pub fn cannot_hold_cards(why: &str) -> bool {
    why.to_lowercase().contains("cannot hold approvals")
}

/// The ledger's word when another writer took the sequence read a moment ago (`SEQUENCE_TAKEN`).
fn sequence_taken(why: &str) -> bool {
    why.to_lowercase().contains("sequence is invalid")
}

/// Bounds, so a ledger that refuses everything cannot grow the host's memory without end.
const MAX_PER_MISSION: usize = 50;
const MAX_MISSIONS: usize = 200;
const TAIL_ATTEMPTS: usize = 3;

/// What the note needs of the mission as the ledger holds it: its last event's sequence, its run and its runtime.
#[derive(Clone, Debug, Default)]
pub struct Held {
    pub last_sequence: Option<u64>,
    pub run_id: String,
    pub runtime: String,
}

/// `approvalNotRecordedNote`: the note one past the mission's last event, naming the card's kind, the answer and the
/// first line of the ledger's reason. None for a turn too old to hold cards.
#[allow(clippy::too_many_arguments)]
pub fn approval_not_recorded_note(held: &Held, run_id: &str, mission_id: &str, kind: &str, answer: &str, why: &str, now: &str) -> Option<Value> {
    if cannot_hold_cards(why) {
        return None;
    }
    // The ledger requires contiguous sequences: one past the mission's last.
    let sequence = held.last_sequence.unwrap_or(0) + 1;
    let first_line = why.split('\n').next().unwrap_or("").trim_end_matches('\r').trim();
    let reason = if first_line.is_empty() { "the ledger refused the write" } else { first_line };
    Some(json!({
        "id": format!("{run_id}:approval-not-recorded:{sequence}"),
        "runId": run_id,
        "missionId": mission_id,
        "sequence": sequence,
        "type": "adapter.diagnostic",
        "occurredAt": now,
        "sourceAdapter": held.runtime,
        "payload": {
            "level": "warning",
            "code": APPROVAL_NOT_RECORDED,
            "message": format!("The answer to a card ({kind}, {answer}) could not be written to this record: {reason}"),
            "terminal": false,
            "evidence": {"redacted": true}
        }
    }))
}

/// The ledger, as the run-end step uses it (mission-store's appendApproval, getMission and appendEvents).
pub trait AnswerLedger {
    fn append_approval(&mut self, mission_id: &str, approval: &Value) -> Result<(), String>;
    fn get_mission(&mut self, mission_id: &str) -> Option<Held>;
    fn append_events(&mut self, mission_id: &str, events: &[Value]) -> Result<(), String>;
}

/// `createUnwrittenAnswers`: answers the ledger refused, by mission, oldest mission first.
#[derive(Default)]
pub struct UnwrittenAnswers {
    waiting: Vec<(String, Vec<(Value, String)>)>,
}

impl UnwrittenAnswers {
    pub const fn new() -> Self {
        Self { waiting: Vec::new() }
    }

    /// The ledger refused this answer's record; it is tried again when the run ends.
    pub fn remember(&mut self, mission_id: &str, approval: Value, why: &str) {
        if cannot_hold_cards(why) {
            return;
        }
        let at = match self.waiting.iter().position(|(m, _)| m == mission_id) {
            Some(at) => at,
            None => {
                self.waiting.push((mission_id.to_owned(), Vec::new()));
                self.waiting.len() - 1
            }
        };
        let list = &mut self.waiting[at].1;
        if list.len() >= MAX_PER_MISSION {
            return;
        }
        list.push((approval, why.to_owned()));
        while self.waiting.len() > MAX_MISSIONS {
            self.waiting.remove(0);
        }
    }

    /// How many answers wait for this mission's end.
    pub fn pending(&self, mission_id: &str) -> usize {
        self.waiting.iter().find(|(m, _)| m == mission_id).map_or(0, |(_, l)| l.len())
    }

    /// What was remembered for this mission, taken out to be written when its run ends.
    pub fn take(&mut self, mission_id: &str) -> Option<Vec<(Value, String)>> {
        let at = self.waiting.iter().position(|(m, _)| m == mission_id)?;
        Some(self.waiting.remove(at).1)
    }
}

/// `onRunEnded`: writes what was remembered, or says in the record that it could not. The last refusal of a note is
/// returned, for the caller to say in its run-end log; nothing else can be done for it.
pub fn write_at_run_end(list: Vec<(Value, String)>, mission_id: &str, ledger: &mut dyn AnswerLedger, now: &dyn Fn() -> String) -> Result<(), String> {
    let mut last_error = None;
    for (approval, _why) in list {
        let still_why = match ledger.append_approval(mission_id, &approval) {
            Ok(()) => continue,
            Err(why) => why,
        };
        // Still refused: say so in the record, after the run's last event. Another run-end writer may take the tail
        // between the read and the write; the ledger refuses before writing anything, so the tail is read again.
        for _ in 0..TAIL_ATTEMPTS {
            let Some(held) = ledger.get_mission(mission_id) else { break };
            let kind = approval["kind"].as_str().unwrap_or_default();
            let answer = approval["answer"].as_str().unwrap_or_default();
            let Some(note) = approval_not_recorded_note(&held, &held.run_id, mission_id, kind, answer, &still_why, &now()) else { break };
            match ledger.append_events(mission_id, &[note]) {
                Ok(()) => {
                    last_error = None;
                    break;
                }
                Err(why) => {
                    let taken = sequence_taken(&why);
                    last_error = Some(why);
                    if !taken {
                        break;
                    }
                }
            }
        }
    }
    last_error.map_or(Ok(()), Err)
}

/// Tests only: the ledger refuses this many answers' records, mid-run and at the run's end alike, in a busy disk's words.
#[cfg(any(test, feature = "test-hooks"))]
pub static REFUSE_ANSWERS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Whether a test has the ledger refuse this answer's record (never outside tests).
pub fn refused_for_test() -> Option<String> {
    #[cfg(any(test, feature = "test-hooks"))]
    {
        use std::sync::atomic::Ordering;
        if REFUSE_ANSWERS.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1)).is_ok() {
            return Some("EBUSY: resource busy or locked".into());
        }
    }
    None
}

/// The host's answers waiting for their run's end (`unwrittenAnswers` in main/index.ts).
pub static UNWRITTEN: Mutex<UnwrittenAnswers> = Mutex::new(UnwrittenAnswers::new());

/// The run-end step (`{ label: 'unwritten answers' }`): a turn with answers remembered has them written to its ledger
/// file, or the note said; a turn with nothing remembered is not touched at all.
pub fn on_run_ended(store: &crate::store::Store, mission_id: &str) {
    let Some(list) = crate::invariants::lock(&UNWRITTEN).take(mission_id) else { return };
    let Ok(path) = store.write_path(mission_id) else { return };
    let mut ledger = FileLedger { path };
    if let Err(why) = write_at_run_end(list, mission_id, &mut ledger, &crate::ledger_write::now) {
        eprintln!("Run-end step unwritten answers failed: {why}");
    }
}

/// The turn's own ledger file, opened afresh: the run's writer has let go of it by the run's end, and a writer that
/// refused once refuses everything after.
pub struct FileLedger {
    pub path: std::path::PathBuf,
}

fn read_records(path: &std::path::Path) -> Result<Vec<Value>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    Ok(text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect())
}

impl AnswerLedger for FileLedger {
    /// mission-store's appendApproval, on the file: a ledger too old to hold cards refuses it in mission-store's words.
    fn append_approval(&mut self, _mission_id: &str, approval: &Value) -> Result<(), String> {
        if let Some(why) = refused_for_test() {
            return Err(why);
        }
        crate::ledger_write::validate_approval(approval).map_err(|e| e.to_string())?;
        let text = std::fs::read_to_string(&self.path).map_err(|e| e.to_string())?;
        if !text.is_empty() && !text.ends_with('\n') {
            return Err("Ledger does not end on a whole record".into());
        }
        let records: Vec<Value> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
        let first = records.first().ok_or("Ledger has no first record")?;
        if first["schemaVersion"].as_u64().is_none_or(|v| v < 20) {
            return Err("Mission ledger version cannot hold approvals".into());
        }
        let ledger = records.iter().filter_map(|r| r["ledgerSequence"].as_u64()).max().unwrap_or(0) + 1;
        let at = approval["occurredAt"].as_str().map(str::to_owned).unwrap_or_else(crate::ledger_write::now);
        let record = json!({"schemaVersion": first["schemaVersion"], "recordType": "mission.approval", "ledgerSequence": ledger, "occurredAt": at, "approval": approval});
        let mut line = serde_json::to_vec(&record).map_err(|e| e.to_string())?;
        line.push(b'\n');
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new().append(true).open(&self.path).map_err(|e| e.to_string())?;
        file.write_all(&line).and_then(|_| file.sync_all()).map_err(|e| e.to_string())
    }
    fn get_mission(&mut self, _mission_id: &str) -> Option<Held> {
        let records = read_records(&self.path).ok()?;
        let meta = &records.first()?["metadata"];
        Some(Held {
            last_sequence: records.iter().rev().find_map(|r| r.get("event")).and_then(|e| e["sequence"].as_u64()),
            run_id: meta["runId"].as_str().unwrap_or_default().to_owned(),
            runtime: meta["runtime"].as_str().unwrap_or_default().to_owned(),
        })
    }
    fn append_events(&mut self, _mission_id: &str, events: &[Value]) -> Result<(), String> {
        // mission-store's appendEvents: the first event must be the one after the ledger's last, or nothing is written.
        let mut taken = false;
        crate::ledger_write::append_events(&self.path, |_, _, _, next, _| {
            if events.first().and_then(|e| e["sequence"].as_u64()) != Some(next) {
                taken = true;
                return Vec::new();
            }
            events.to_vec()
        })
        .map_err(|e| e.to_string())?;
        if taken { Err("Mission event sequence is invalid".into()) } else { Ok(()) }
    }
}

#[cfg(test)]
#[path = "approval_record_note_tests.rs"]
mod tests;

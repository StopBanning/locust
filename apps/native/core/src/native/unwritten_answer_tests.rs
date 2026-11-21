//! Electron's `main/an-unwritten-card-answer-is-said-in-the-record.test.ts`, the host's case, through a real run: a
//! card's answer the ledger refused while the run was live is written when the run ends, or said in a note after the
//! run's last event. The note and the run-end step themselves are in `approval_record_note_tests.rs`.
use super::*;
use crate::approval_record_note::{APPROVAL_NOT_RECORDED, REFUSE_ANSWERS};

/// One Approve-each turn whose card is allowed, with the ledger refusing the answer's record `refusals` times; the
/// ledger's records once the turn's ending, run-end steps included, is done.
fn answered_with_refusals(refusals: u64) -> Vec<Value> {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("approve-allow"));
    REFUSE_ANSWERS.store(refusals, Ordering::SeqCst);
    let mut req = job(&t, None, "Synthetic prompt");
    req.teammate.runs_in = "approve-each".into();
    let started = send(req).unwrap();
    let mission = real_tip(&started.conversation_id).unwrap_or(started.conversation_id.clone());
    wait("the card", || !waiting_approvals_for(&mission).is_empty());
    let card = waiting_approvals_for(&mission).remove(0);
    decide_approval(&card.approval_id, Decision::ApproveOnce, None).unwrap();
    finished(&started.conversation_id);
    wait("the turn's ending", || TURN_THREADS.load(Ordering::SeqCst) == 0);
    REFUSE_ANSWERS.store(0, Ordering::SeqCst);
    records(&path_of(&t, &mission))
}

fn notes(rows: &[Value]) -> Vec<String> {
    rows.iter()
        .filter(|r| r["event"]["payload"]["code"] == APPROVAL_NOT_RECORDED)
        .map(|r| r["event"]["payload"]["message"].as_str().unwrap_or_default().to_owned())
        .collect()
}

// case: remembers a refused answer where every answer is written, and writes it when the run ends
#[test]
fn remembers_a_refused_answer_where_every_answer_is_written_and_writes_it_when_the_run_ends() {
    // Refused once, while the run was live: written when it ended, after its last event, and no note.
    let rows = answered_with_refusals(1);
    let approvals: Vec<&Value> = rows.iter().filter(|r| r["recordType"] == "mission.approval").collect();
    assert_eq!(approvals.len(), 1, "{rows:?}");
    assert_eq!(approvals[0]["approval"]["answer"], "allowed");
    let ended_at = rows.iter().position(|r| r["event"]["type"] == "run.completed").expect("the run completed");
    assert!(rows.iter().position(|r| r["recordType"] == "mission.approval").unwrap() > ended_at);
    assert!(notes(&rows).is_empty(), "{rows:?}");
    // Refused again at the run's end: the note says so, one past the run's last event.
    let rows = answered_with_refusals(2);
    assert!(!rows.iter().any(|r| r["recordType"] == "mission.approval"), "{rows:?}");
    let said = notes(&rows);
    assert_eq!(said.len(), 1, "{rows:?}");
    assert!(said[0].starts_with("The answer to a card (") && said[0].ends_with(", allowed) could not be written to this record: EBUSY: resource busy or locked"), "{}", said[0]);
    let sequences: Vec<u64> = rows.iter().filter_map(|r| r["event"]["sequence"].as_u64()).collect();
    assert!(sequences.windows(2).all(|w| w[1] == w[0] + 1), "{sequences:?}");
    assert_eq!(rows.last().unwrap()["event"]["payload"]["code"], APPROVAL_NOT_RECORDED);
}

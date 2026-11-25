//! Electron's `main/approvals-come-only-from-the-window.test.ts`, the window's card (M31): a card no run can take is not
//! offered. Electron's App drops it with `approvalsOfLiveRuns` on every change of runs; native's window draws the cards
//! the process's channel holds (`waiting_approvals`), and a run's end releases its cards from that channel, so a
//! stopped run's card leaves the window and an answer to it is refused.
use super::*;

// case: is dropped when its run ends: App applies approvalsOfLiveRuns on every change of runs
#[test]
fn a_card_whose_run_ended_leaves_the_window_and_cannot_be_answered() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("approve-allow"));
    let mut req = job(&t, None, "Synthetic prompt");
    req.teammate.runs_in = "approve-each".into();
    let started = send(req).unwrap();
    let mission = real_tip(&started.conversation_id).unwrap_or(started.conversation_id.clone());
    wait("the card", || !waiting_approvals_for(&mission).is_empty());
    let card = waiting_approvals_for(&mission).remove(0);
    assert!(waiting_approvals().iter().any(|c| c.approval_id == card.approval_id), "the window shows the live run's card");
    stop(&started.conversation_id).unwrap();
    finished(&started.conversation_id);
    assert!(!waiting_approvals().iter().any(|c| c.approval_id == card.approval_id), "the ended run's card is not offered");
    assert!(decide_approval(&card.approval_id, Decision::ApproveOnce, None).is_err(), "and an answer to it is refused");
}

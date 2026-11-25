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

// A run that ends on its own, with no Stop pressed, still closes its approvals: the run's `finish` releases the
// mission from the channel, so a request that arrives after the run is refused rather than raising a card no run can
// take (Electron: approval-channel.ts releaseRun on every run end, not only on cancel). Merge check, Loose ends 1.
#[test]
fn a_run_that_finished_on_its_own_refuses_a_late_approval_request() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    let started = send(job(&t, None, "Synthetic prompt")).unwrap();
    finished(&started.conversation_id);
    let mission = real_tip(&started.conversation_id).unwrap_or(started.conversation_id.clone());
    // The cancel only gives up after a while, so a missing release shows as a waiting card, not a hang.
    let give_up = Instant::now() + Duration::from_secs(3);
    let late = approvals_channel().ask_claude("late-run", &mission, "late", "Bash", &serde_json::json!({"command": "ls"}), "/tmp", &|| Instant::now() >= give_up);
    let why = late.expect_err("a request after the run finished is refused, not left waiting for a card");
    assert!(why.contains("already ended"), "{why}");
    assert!(!waiting_approvals().iter().any(|c| c.mission_id == mission), "and no card is offered for it");
}

// The same for a run whose agent never started (its program is missing): the run's `finish` still releases the
// mission, so nothing can raise a card for it afterwards. Merge check, Loose ends 1.
#[test]
fn a_run_whose_agent_never_started_refuses_a_late_approval_request() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), Some((PathBuf::from("/nonexistent/locust-no-such-agent"), Vec::new())));
    let started = send(job(&t, None, "Synthetic prompt")).unwrap();
    finished(&started.conversation_id);
    let mission = real_tip(&started.conversation_id).unwrap_or(started.conversation_id.clone());
    let give_up = Instant::now() + Duration::from_secs(3);
    let late = approvals_channel().ask_claude("late-run", &mission, "late", "Bash", &serde_json::json!({"command": "ls"}), "/tmp", &|| Instant::now() >= give_up);
    let why = late.expect_err("a request after a run that never started is refused");
    assert!(why.contains("already ended"), "{why}");
}

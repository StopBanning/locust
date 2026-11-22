// Electron's `renderer/src/a-lead-shows-what-it-handed-out.test.tsx`, the parts that are words and states. The card
// itself is drawn and pressed in the window's own tests (src/ui/handed_out_tests.rs).
use super::*;
use crate::ledger::PeerLink;

fn link(direction: &str, message_id: &str) -> PeerLink {
    PeerLink { direction: direction.into(), message_id: message_id.into(), peer_teammate_id: "tm_bro".into(), occurred_at: String::new() }
}

#[test]
fn a_to_dos_state_reads_the_run_then_the_record_and_needs_you_outranks_both() {
    assert_eq!(todo_state(Some("running"), None, false), TodoState::Working);
    assert_eq!(todo_state(Some("cancelling"), None, false), TodoState::Working);
    assert_eq!(todo_state(Some("running"), None, true), TodoState::NeedsYou);
    assert_eq!(todo_state(None, Some("completed"), true), TodoState::NeedsYou);
    assert_eq!(todo_state(None, Some("completed"), false), TodoState::Done);
    assert_eq!(todo_state(Some("running"), Some("completed"), false), TodoState::Working);
    assert_eq!(todo_state(None, Some("failed"), false), TodoState::Failed);
    assert_eq!(todo_state(None, Some("cancelled"), false), TodoState::Stopped);
    assert_eq!(todo_state(None, Some("interrupted"), false), TodoState::Stopped);
    assert_eq!(todo_state(None, None, false), TodoState::Starting);
    assert_eq!(TodoState::Working.words(), ("Working", Tone::Blue));
    assert_eq!(TodoState::NeedsYou.words(), ("Needs you", Tone::Amber));
    assert_eq!(TodoState::Done.words(), ("Done", Tone::Green));
    assert_eq!(TodoState::Starting.words(), ("Starting", Tone::Quiet));
    assert_eq!(TodoState::Failed.words(), ("Failed", Tone::Red));
    assert_eq!(TodoState::Stopped.words(), ("Stopped", Tone::Quiet));
}

#[test]
fn the_count_is_said_in_words_that_agree_with_themselves() {
    assert_eq!(todo_count(1), "1 to-do");
    assert_eq!(todo_count(2), "2 to-dos");
}

#[test]
fn only_what_the_turn_posted_is_handed_out_each_ask_its_own_line() {
    let messages = [
        ("msg_fix", "posted", "Casper", Some("Fix the login test.")),
        ("msg_in", "received", "Bro", Some("Thanks")),
        ("msg_more", "posted", "Casper", Some("Also check the signed-out case")),
        ("msg_gone", "posted", "Toaster", None),
    ];
    let handed = handed_out(messages, |id| (id != "msg_gone").then(|| "mis_fix".to_owned()));
    assert_eq!(handed.iter().map(|h| h.message_id.as_str()).collect::<Vec<_>>(), ["msg_fix", "msg_more", "msg_gone"]);
    assert_eq!(handed[0].mission_id.as_deref(), Some("mis_fix"));
    assert_eq!(handed[1].mission_id.as_deref(), Some("mis_fix"));
    assert_eq!(handed[2].mission_id, None);
    assert_eq!(handed[2].task, GONE);
    assert_eq!(handed_out_label(&handed).as_deref(), Some("Handed out \u{b7} 3 to-dos"));
    assert_eq!(handed_out_label(&handed[..1]).as_deref(), Some("Handed out \u{b7} 1 to-do"));
    assert_eq!(handed_out_label(&[]), None);
    assert!(handed_out([("msg_in", "received", "Bro", Some("x"))], |_| None).is_empty());
}

#[test]
fn uses_the_receiving_live_mission_before_a_history_refresh_never_the_sender() {
    // case: uses the receiving live mission before a history refresh, never the sender
    let sender = [link("posted", "msg_fix")];
    let target = [link("received", "msg_fix")];
    let missions = [("mis_lead", &sender[..]), ("mis_fix", &target[..])];
    assert_eq!(peer_run_for("msg_fix", missions), Some("mis_fix"));
    assert_eq!(peer_run_for("msg_fix", [("mis_lead", &sender[..])]), None);
    assert_eq!(peer_run_for("", missions), None);
}

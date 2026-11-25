//! what-happens-in-the-terminal-comes-back.test.ts, at the window (App.tsx 0.391): what the person did in the terminal
//! is asked for when a settled Claude Code or Codex conversation is shown, and again whenever the window comes back into
//! focus on it, without a press; a running turn, or another agent's conversation, is not asked about.
use super::*;

fn meta(mission_id: &str, runtime: &str) -> ledger::Metadata {
    ledger::Metadata { mission_id: mission_id.into(), prompt: "Ask".into(), runtime: runtime.into(), ..Default::default() }
}

fn showing(tip: &str, runtime: &str, running: bool) -> State {
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: Teammate { id: "tm_wren".into(), name: "Wren".into(), role: String::new(), runtime: Some(runtime.into()), model: None,
            account: None, busy: false, runs_in: "ask".into(), folder: None },
        conversation: Conversation { conversation_id: Some(tip.into()), turns: vec![] },
        items: vec![], dir: std::path::PathBuf::new(), worker: None,
        snapshot: ledger::Snapshot { tip: tip.into(), turn_metas: vec![meta(tip, runtime)], running, ..Default::default() },
        due: 0.0, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
    });
    s
}

/// Polls until the asked catch-up has answered and its answer is taken.
fn settle(s: &mut State) {
    for _ in 0..400 {
        poll(s);
        if !s.terminal.catchup.is_pending() {
            poll(s);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the catch-up never answered");
}

// Not an Electron case of its own: the window side of the row, the call site that keeps the terminal offer's promise.
#[test]
fn a_settled_conversation_asks_for_the_terminal_when_shown_and_again_on_focus() {
    let mut s = showing("mission_shown", "claude", false);
    poll(&mut s);
    assert_eq!(s.terminal.asks, 1, "asked when the conversation is shown");
    assert_eq!(s.terminal.asked.as_deref(), Some("mission_shown"));
    settle(&mut s);
    poll(&mut s);
    assert_eq!(s.terminal.asks, 1, "not asked again while nothing changed");
    assert_eq!(s.composer_note, None, "asked on its own, it says nothing when nothing came back");
    s.window_focused = false;
    poll(&mut s);
    s.window_focused = true;
    poll(&mut s);
    assert_eq!(s.terminal.asks, 2, "asked again when the window comes back into focus on it");
    settle(&mut s);
    // A new turn on screen (one the terminal brought back, or the next reply) is asked about too.
    s.ledger.as_mut().unwrap().snapshot.tip = "mission_next".into();
    poll(&mut s);
    assert_eq!((s.terminal.asks, s.terminal.asked.as_deref()), (3, Some("mission_next")));
    settle(&mut s);

    let mut codex = showing("mission_codex", "codex", false);
    poll(&mut codex);
    assert_eq!(codex.terminal.asks, 1, "Codex's rollout is read the same way");
    settle(&mut codex);
}

#[test]
fn a_running_turn_or_another_agents_conversation_is_not_asked_about() {
    let mut running = showing("mission_running", "claude", true);
    poll(&mut running);
    assert_eq!(running.terminal.asks, 0, "a running turn is its conversation's newest: nothing continues one");
    let mut other = showing("mission_cursor", "cursor", false);
    poll(&mut other);
    assert_eq!(other.terminal.asks, 0, "only a runtime whose sessions the host can read");
}

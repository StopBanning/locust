//! Open in terminal from the window goes through Electron's `terminalRequestFor` (`runtime_terminal::terminal_request_for`):
//! a running conversation is refused first, in Electron's words, and a conversation the ledger does not hold is refused
//! as one (main/a-conversation-opens-in-its-own-terminal.test.ts, "which conversation, and where").
use super::*;

/// Wren's conversation on Claude Code, finished or still running, whose ledger tip is `tip`.
fn conversation(tip: &str, running: bool) -> State {
    let mut s = State::default();
    let header = Teammate { id: "tm".into(), name: "Wren".into(), role: "Custom".into(), model: None, runtime: Some("claude".into()), folder: None, runs_in: "ask".into(), account: None, busy: running };
    s.teammates.push(header.clone());
    s.conversations.insert("tm".into(), Default::default());
    if running {
        s.opening_teammate = Some("tm".into());
    }
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header,
        conversation: Conversation { conversation_id: Some(tip.into()), turns: Vec::new() },
        items: Vec::new(),
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: ledger::Snapshot {
            tip: tip.into(),
            turn_metas: vec![ledger::Metadata { mission_id: tip.into(), runtime: "claude".into(), model: "opus".into(), ..Default::default() }],
            ..Default::default()
        },
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    s
}

#[test]
fn opening_a_running_conversation_in_a_terminal_is_refused_and_says_why() {
    let mut s = conversation("mission_running", true);
    open_in_terminal(&mut s);
    assert_eq!(
        s.composer_note.as_deref(),
        Some("This conversation is still running. Open it in a terminal once the run finishes: two programs on one session at once would talk over each other.")
    );
}

#[test]
fn opening_a_conversation_the_ledger_does_not_hold_is_refused_as_one() {
    let mut s = conversation("mission_nowhere_0000", false);
    open_in_terminal(&mut s);
    assert_eq!(s.composer_note.as_deref(), Some("That conversation is not in the ledger."));
}

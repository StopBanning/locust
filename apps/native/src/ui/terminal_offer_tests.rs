//! a-conversation-offers-its-terminal.test.tsx through the window: the offer (`terminal_offer`, as `shown_terminal`
//! makes it for the conversation on screen) and where it is drawn, the head of the More actions menu
//! (`header_menu::items`) in the whole window. Expected words are Electron's (OpenInTerminal.tsx's test).
use super::*;
use rune::{Env, Runtime};

fn offer(runtime: &str) -> Option<TerminalOffer> {
    terminal_offer(Some(runtime), Some("opus"), Some("m1"), false, Some("Wren"), false)
}

/// The whole window on a finished (or running) conversation of Wren's, on `runtime`.
fn window(runtime: &str, running: bool) -> Runtime<State> {
    let mut s = State::default();
    let header = Teammate { id: "tm".into(), name: "Wren".into(), role: "Custom".into(), model: None, runtime: Some(runtime.into()), folder: None, runs_in: "ask".into(), account: None, busy: running };
    s.teammates.push(header.clone());
    s.conversations.insert("tm".into(), Default::default());
    if running {
        s.opening_teammate = Some("tm".into());
    }
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header,
        conversation: Conversation { conversation_id: Some("mission_1".into()), turns: Vec::new() },
        items: vec![ledger::Item::User("Check the migration.".into()), ledger::Item::Assistant { item_id: "a1".into(), text: "Done.".into(), r#final: true }],
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: ledger::Snapshot {
            tip: "mission_1".into(),
            turn_metas: vec![ledger::Metadata { mission_id: "mission_1".into(), runtime: runtime.into(), model: "opus".into(), ..Default::default() }],
            ..Default::default()
        },
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    let mut rt = Runtime::new(s, crate::view, (1400.0, 900.0), Env::new(true, 1.0));
    for _ in 0..10 {
        rt.frame(0.05);
    }
    rt
}

fn open_menu(rt: &mut Runtime<State>) -> Vec<String> {
    for _ in 0..5 {
        rt.frame(0.05);
    }
    let (x, y) = rt.find("thread-header-more").expect("the More actions button");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..5 {
        rt.frame(0.05);
    }
    rt.state.row_menu.as_ref().map(|m| m.items.iter().map(|i| i.label.clone()).collect()).unwrap_or_default()
}

#[test]
fn names_the_runtime_whose_interface_opens_says_what_happens_there_comes_back_and_that_its_permissions_are_not_this_mode() {
    // case: names the runtime whose interface opens, says what happens there comes back, and that its permissions are not this mode
    let o = offer("claude").expect("an offer");
    assert_eq!(o.label, "Open in Claude Code, in a terminal");
    assert!(o.title.contains("The same session, in Wren's folder."), "{}", o.title);
    assert!(o.title.contains("What you do there comes back into this conversation"), "{}", o.title);
    assert!(o.title.contains("Claude Code's own permissions, not this conversation's mode"), "{}", o.title);
    assert_eq!(o.opened("Windows Terminal"), "Opened in Windows Terminal. What you do there comes back here when you return.");
    assert_eq!(o.disabled, None);
    assert!(offer("codex").unwrap().title.contains("What you do there comes back into this conversation"));
}

#[test]
fn says_locust_wont_see_it_for_a_runtime_whose_sessions_it_does_not_read_back() {
    // case: says Locust won't see it for a runtime whose sessions it does not read back (0.391)
    for runtime in ["copilot", "cursor", "opencode", "muse"] {
        let o = offer(runtime).unwrap_or_else(|| panic!("{runtime}"));
        assert!(o.title.contains("Locust won't see what you do there"), "{runtime}: {}", o.title);
        assert_eq!(o.opened("Windows Terminal"), "Opened in Windows Terminal. Locust won't see what happens there.", "{runtime}");
    }
}

#[test]
fn is_held_while_the_run_is_going_saying_until_when_and_why() {
    // case: is held while the run is going, saying until when and why
    let held = terminal_offer(Some("claude"), Some("opus"), Some("m1"), true, Some("Wren"), false).unwrap();
    assert_eq!(held.disabled.as_deref(), Some("Available when Wren finishes: Locust's run and a terminal can't share one session at once."));
    // In the window the menu has no entry while the run holds the session (App.tsx pushes it only when not disabled).
    let mut rt = window("claude", true);
    assert_eq!(shown_terminal(&rt.state).and_then(|o| o.disabled), Some("Available when Wren finishes: Locust's run and a terminal can't share one session at once.".into()));
    assert!(!open_menu(&mut rt).iter().any(|l| l.starts_with("Open in terminal")));
}

#[test]
fn is_not_made_where_there_is_no_terminal_to_open() {
    // case: is not made where there is no terminal to open
    assert_eq!(offer("antigravity"), None);
    let through_cli = terminal_offer(Some("antigravity"), Some("opus"), Some("m1"), false, Some("Wren"), true).unwrap();
    assert_eq!(through_cli.label, "Open in Antigravity, in a terminal");
    assert_eq!(offer("gemini"), None);
    assert_eq!(terminal_offer(Some("opencode"), Some("own-1a2b3c4d/acme-70b"), Some("m1"), false, Some("Wren"), false), None);
    assert_eq!(terminal_offer(Some("claude"), Some("opus"), None, false, Some("Wren"), false), None);
    for runtime in ["codex", "copilot", "cursor", "opencode", "muse"] {
        assert!(offer(runtime).is_some(), "{runtime}");
    }
    // In the window: a Gemini conversation's menu has no entry.
    let mut rt = window("gemini", false);
    assert!(!open_menu(&mut rt).iter().any(|l| l.starts_with("Open in terminal")));
}

#[test]
fn heads_the_more_menu_and_is_no_longer_a_header_button_of_its_own() {
    // case: heads the ... menu, and is no longer a header button of its own (0.797: the header is a row of icons)
    let mut rt = window("claude", false);
    let header_words: Vec<String> = rt.elements().into_iter().flat_map(|e| [e.label, e.help]).flatten().collect();
    assert!(!header_words.iter().any(|w| w.starts_with("Open in") || w.contains("in a terminal")), "{header_words:?}");
    assert!(header_words.iter().any(|w| w == "More actions for this conversation"), "{header_words:?}");
    let labels = open_menu(&mut rt);
    assert_eq!(labels, ["Verbose", "Open in terminal (Claude Code)", "Send feedback", "Delete conversation"]);
}

//! Opening a conversation whose record is still being read says "Loading conversation…" once, in the thread, and not
//! again in the status line over the message box. Follows Electron's Composer.tsx (its line over the field has no words
//! for a conversation being opened) and App.tsx:7521 `openMission` (opening draws the thread; nothing else says so).
use super::*;

fn mate() -> Teammate {
    Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Code & Migrations".into(), runtime: Some("claude".into()), model: None,
        account: None, busy: false, runs_in: "ask".into(), folder: None }
}

/// The window as `begin_ledger` leaves it for a conversation with nothing read yet: the pane loading, and its status.
fn opening() -> State {
    let mut s = State::default();
    s.teammates = vec![mate()];
    s.ledger = Some(LedgerPane {
        header: mate(),
        conversation: Conversation { conversation_id: Some("m_open".into()), turns: vec![("You".into(), "Fix the cart".into())] },
        items: Vec::new(),
        dir: std::env::temp_dir(),
        worker: None,
        snapshot: ledger::Snapshot::default(),
        cold_tip: None,
        due: 0.0,
        loading: true,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    s.status = "Loading conversation\u{2026}".into();
    s.status_tone = StatusTone::Muted;
    s
}

#[test]
fn a_conversation_being_opened_says_loading_once_in_the_thread_not_again_over_the_message_box() {
    let mut rt = rune::Runtime::new(opening(), |s, _| thread_view(s, &Palette::new(true)), (1000.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let all: Vec<_> = rt.elements().into_iter().filter(|e| !e.exiting).collect();
    let said: Vec<_> = all.iter().filter(|e| e.text.as_deref().is_some_and(|t| t.contains("Loading conversation"))).collect();
    assert_eq!(said.len(), 1, "said once: {:?}", said.iter().map(|e| e.rect).collect::<Vec<_>>());
    let field = all.iter().find(|e| e.key.as_deref() == Some("composer")).expect("the message box").rect;
    let messages = all.iter().find(|e| e.key.as_deref() == Some("messages")).expect("the thread").rect;
    let at = said[0].rect;
    assert!(at[1] >= messages[1] && at[1] + at[3] <= messages[1] + messages[3] + 0.5, "in the thread: {at:?} {messages:?}");
    assert!(at[1] + at[3] < field[1] - 40.0, "well above the message box, not its status line: {at:?} {field:?}");
}

#[test]
fn any_other_status_is_still_said_over_the_message_box() {
    let mut s = opening();
    s.status = "Locust's data folder could not be found.".into();
    s.status_tone = StatusTone::Red;
    let mut rt = rune::Runtime::new(s, |s, _| thread_view(s, &Palette::new(true)), (1000.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("Locust's data folder could not be found.")));
}

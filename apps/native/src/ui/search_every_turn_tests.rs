//! search-finds-every-turn.test.tsx through the window: a conversation of three turns and another of Atlas's, read as
//! the window reads them (`recent`, `ledger_summaries`), searched in the sidebar's box (`sidebar_missions_now`, the
//! sidebar drawn), Escape pressed there (`on_key`), and Ctrl+K's palette opened, typed into and drawn. Words and ids are
//! Electron's.
use super::*;
use locust_core::engine::ConversationRow;

/// Three turns of one conversation (Wren's), and Atlas's own: the ids, titles and words of Electron's test.
fn window() -> State {
    let mut s = State::default();
    s.demo = true;
    let turns = [
        ("mission_login", None, "Fix the login redirect loop", "Fix the login redirect loop", "Wren", "01"),
        ("mission_signup", Some("mission_login"), "Fix the login redirect loop", "Also check the signup form does the same thing", "Wren", "02"),
        ("mission_ship", Some("mission_signup"), "Fix the login redirect loop", "Good, ship it", "Wren", "03"),
        ("mission_crm", None, "Summarize the CRM quotes", "Summarize the CRM quotes", "Atlas", "00"),
    ];
    for (id, parent, title, words, owner, time) in turns {
        let at = format!("2026-10-01T10:00:{time}Z");
        s.recent.push(ConversationRow { id: id.into(), title: title.into(), teammate: owner.into(), started_by: "native".into(), state: "completed".into(), updated: at.clone() });
        s.ledger_summaries.insert(id.into(), ledger::Summary {
            mission_id: id.into(), title: title.into(), parent: parent.map(str::to_owned), words: words.into(),
            runtime: "codex".into(), created_at: at.clone(), updated: at, ..Default::default()
        });
    }
    s
}

fn key(key: Key) -> KeyEvent {
    KeyEvent { key, mods: Default::default() }
}

fn ctrl_k() -> KeyEvent {
    KeyEvent { key: Key::Command('k'), mods: rune::Mods { ctrl: true, ..Default::default() } }
}

fn sidebar_words(s: State) -> Vec<String> {
    let mut rt = rune::Runtime::new(s, |s, _| crate::sidebar::sidebar(s, &Palette::new(true)), (320.0, 1200.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect()
}

/// The palette opened with Ctrl+K and `typed` into its box; the conversations it lists, drawn.
fn palette_conversations(mut s: State, typed: &str) -> Vec<String> {
    assert!(on_key(&mut s, &ctrl_k(), &mut Tx::default()));
    assert!(s.palette_open);
    s.palette_query.replace_text(typed);
    let mut rt = rune::Runtime::new(s, |s, _| command_palette(s, &Palette::new(true)), (600.0, 900.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.key).filter_map(|k| k.strip_prefix("palette-conv:").map(str::to_owned)).collect()
}

// case: keeps every turn’s words on the conversation’s one row
#[test]
fn keeps_every_turns_words_on_the_conversations_one_row() {
    let rows = crate::sidebar::sidebar_missions_now(&window());
    assert_eq!(rows.len(), 2);
    let login = rows.iter().find(|r| r.title == "Fix the login redirect loop").expect("the login conversation");
    assert_eq!(login.said, "Fix the login redirect loop · Also check the signup form does the same thing · Good, ship it");
}

// case: finds a conversation by a word only its second turn said, as one row under its own name
#[test]
fn finds_a_conversation_by_a_word_only_its_second_turn_said_as_one_row_under_its_own_name() {
    let mut s = window();
    s.sidebar_query.replace_text("signup");
    let found = crate::sidebar::sidebar_missions_now(&s);
    assert_eq!(found.iter().map(|r| r.title.as_str()).collect::<Vec<_>>(), ["Fix the login redirect loop"]);
    // The row is the conversation's newest turn, so opening it opens the conversation where it is.
    assert_eq!(found[0].mission_id, "mission_ship");
    let drawn = sidebar_words(s);
    assert!(drawn.iter().any(|w| w == "Fix the login redirect loop"), "{drawn:?}");
    assert!(!drawn.iter().any(|w| w == "Summarize the CRM quotes"), "{drawn:?}");
}

// case: finds a teammate’s conversations by the teammate’s name
#[test]
fn finds_a_teammates_conversations_by_the_teammates_name() {
    let mut s = window();
    s.sidebar_query.replace_text("atlas");
    assert_eq!(crate::sidebar::sidebar_missions_now(&s).iter().map(|r| r.mission_id.as_str()).collect::<Vec<_>>(), ["mission_crm"]);
    // A name no teammate has finds nothing.
    s.sidebar_query.replace_text("booty");
    assert!(crate::sidebar::sidebar_missions_now(&s).is_empty());
}

// case: empties it, and lets Escape through when it is already empty
#[test]
fn empties_it_and_lets_escape_through_when_it_is_already_empty() {
    let mut s = window();
    s.sidebar_query.replace_text("signup");
    s.draft.replace_text("half a message");
    assert!(on_key(&mut s, &key(Key::Escape), &mut Tx::default()));
    assert_eq!(s.sidebar_query.text, "", "the first Escape empties the search");
    assert_eq!(s.draft.text, "half a message", "and does nothing else");
    // Already empty, Escape goes on to whatever else it does: here, the message box's.
    assert!(on_key(&mut s, &key(Key::Escape), &mut Tx::default()));
    assert_eq!(s.draft.text, "");
}

// case: every list the search narrows is narrowed by the teammate’s name too
#[test]
fn every_list_the_search_narrows_is_narrowed_by_the_teammates_name_too() {
    let mut s = window();
    s.sidebar_query.replace_text("atlas");
    let drawn = sidebar_words(s);
    assert!(drawn.iter().any(|w| w == "Summarize the CRM quotes"), "{drawn:?}");
    assert!(!drawn.iter().any(|w| w == "Fix the login redirect loop"), "{drawn:?}");
    assert!(!drawn.iter().any(|w| w == "No conversations match that."), "the name counts when the list says nothing matched: {drawn:?}");
    // The palette's list is narrowed by it too.
    assert_eq!(palette_conversations(window(), "atlas"), ["mission_crm"]);
}

// case: lists conversations, found by every turn and the teammate -- the only search in the rail layout
#[test]
fn lists_conversations_found_by_every_turn_and_the_teammate_the_only_search_in_the_rail_layout() {
    assert_eq!(palette_conversations(window(), "signup"), ["mission_ship"]);
    assert_eq!(palette_conversations(window(), "wren"), ["mission_ship"]);
    // Pressed, it opens that conversation.
    let mut s = window();
    assert!(on_key(&mut s, &ctrl_k(), &mut Tx::default()));
    s.palette_query.replace_text("atlas");
    let at = palette_matches(&s.palette_actions, &s.palette_query.text).iter().position(|a| a.id == "conv:mission_crm").expect("listed");
    assert_eq!(palette_matches(&s.palette_actions, &s.palette_query.text)[at].group, "Conversations");
}

/// Seven conversations, one turn each, the newest last.
fn seven() -> State {
    let mut s = State::default();
    s.demo = true;
    for i in 0..7 {
        let id = format!("mission_{i}");
        let at = format!("2026-10-01T10:00:0{i}Z");
        s.recent.push(ConversationRow { id: id.clone(), title: format!("Task {i}"), teammate: "Wren".into(), started_by: "native".into(), state: "completed".into(), updated: at.clone() });
        s.ledger_summaries.insert(id.clone(), ledger::Summary { mission_id: id, title: format!("Task {i}"), words: format!("Task {i}"), runtime: "codex".into(), created_at: at.clone(), updated: at, ..Default::default() });
    }
    s
}

// case: shows only the five newest before anything is typed
#[test]
fn shows_only_the_five_newest_before_anything_is_typed() {
    let shown = palette_conversations(seven(), "");
    assert_eq!(shown.len(), 5, "{shown:?}");
    assert!(!shown.contains(&"mission_0".to_owned()) && !shown.contains(&"mission_1".to_owned()), "the two oldest wait for typing: {shown:?}");
    assert_eq!(palette_conversations(seven(), "Task 0"), ["mission_0"], "typed, the older ones are found");
}

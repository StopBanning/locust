//! Electron's `a-proposed-change-says-what-it-changes.test.tsx`, through the window: the Memory screen as the People
//! panel draws it (`memory_screen`), the memory card under a turn (`memory_card::view`), and the screen's notice as a
//! finished reply's memory update sets it (`heard_memory_changed`), with Electron's values.
use super::*;
use locust_core::people::MemoryChanged;
use rune::{Env, Runtime};

fn kept() -> Value {
    json!({"memoryId": "mem_kept", "text": "Deploys go out on Fridays.", "scope": "workspace", "workspaceId": "ws_shop",
        "workspaceName": "shop", "by": {"name": "Yurt"}, "createdAt": "2026-09-01T10:00:00.000Z", "status": "kept",
        "enabled": true, "name": "deploy-day"})
}
fn with(fields: Value) -> Value {
    let mut memory = kept();
    for (key, value) in fields.as_object().expect("fields") {
        memory[key] = value.clone();
    }
    memory
}
fn rewrite() -> Value {
    with(json!({"memoryId": "mem_change", "text": "Deploys go out on Thursdays.", "status": "proposed", "replaces": "mem_kept", "missionId": "mission_2"}))
}
fn forget() -> Value {
    with(json!({"memoryId": "mem_forget", "status": "proposed", "forgets": "mem_kept", "missionId": "mission_2"}))
}
fn fresh() -> Value {
    with(json!({"memoryId": "mem_new", "text": "Tests run with pnpm test.", "status": "proposed", "missionId": "mission_2"}))
}

fn with_memories(memories: Vec<Value>) -> State {
    let mut s = State::default();
    s.people_panel.open = true;
    s.people_panel.tab = 1;
    s.people_panel.memory_tab = true;
    s.people_panel.memory_mode = "ask".to_owned();
    s.people_panel.memories = json!({"memories": memories, "forgotten": []});
    s.home.folder_path = Some("/tmp/shop".into());
    s
}
fn screen(memories: Vec<Value>) -> Vec<String> {
    let mut rt = Runtime::new(with_memories(memories), |s, _| memory_screen(s, &Palette::new(true)), (900.0, 2000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}
/// The card under the turn `mission_2`, its one line (the summary) as it is drawn.
fn card(memories: Vec<Value>) -> Vec<String> {
    let mut rt = Runtime::new(with_memories(memories), |s, _| {
        rune::col().children(crate::memory_card::view(s, Some("mission_2"), &Palette::new(true)))
    }, (900.0, 400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}
fn card_open(memories: Vec<Value>) -> Vec<String> {
    let mut s = with_memories(memories);
    s.activity_open.insert("turn-memory-mission_2".into(), true);
    let mut rt = Runtime::new(s, |s, _| {
        rune::col().children(crate::memory_card::view(s, Some("mission_2"), &Palette::new(true)))
    }, (900.0, 400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}
fn has(words: &[String], said: &str) -> bool {
    words.iter().any(|w| w == said)
}

// a proposed change on the Memory screen
#[test]
fn a_rewrite_shows_the_new_words_the_ones_kept_now_and_keep_the_change_keep_the_old_one() {
    let words = screen(vec![kept(), rewrite()]);
    assert!(has(&words, "Wants to change this to: ") && has(&words, "Deploys go out on Thursdays."), "{words:?}");
    assert!(has(&words, "Now: Deploys go out on Fridays."), "{words:?}");
    assert!(has(&words, "Keep the change") && has(&words, "Keep the old one"), "{words:?}");
}

#[test]
fn a_forget_says_so_and_its_buttons_are_forget_it_keep_it() {
    let words = screen(vec![kept(), forget()]);
    assert!(has(&words, "Wants to forget this: ") && has(&words, "Deploys go out on Fridays."), "{words:?}");
    assert!(has(&words, "Forget it") && has(&words, "Keep it"), "{words:?}");
}

#[test]
fn a_new_memory_is_asked_about_as_it_always_was() {
    let words = screen(vec![fresh()]);
    assert!(has(&words, "Keep") && has(&words, "Forget"), "{words:?}");
    assert!(!words.iter().any(|w| w.starts_with("Wants to ")), "{words:?}");
}

// a proposed change on the memory card
#[test]
fn is_named_for_what_it_asks_not_as_something_to_remember() {
    let words = card(vec![kept(), rewrite(), forget(), fresh()]);
    assert!(has(&words, "Yurt wants to remember 1 thing and wants to change 1 thing it already knew and wants to forget 1 thing — answer it on the Memory screen"), "{words:?}");
    // Opened, each line says what it asks: a rewrite, a forget, and the new one as itself.
    let words = card_open(vec![kept(), rewrite(), forget(), fresh()]);
    assert!(has(&words, "Change to: Deploys go out on Thursdays. · waiting for you on the Memory screen"), "{words:?}");
    assert!(has(&words, "Forget: Deploys go out on Fridays. · waiting for you on the Memory screen"), "{words:?}");
    assert!(has(&words, "Tests run with pnpm test. · waiting for you on the Memory screen"), "{words:?}");
}

#[test]
fn a_card_of_new_proposals_only_reads_as_it_always_did() {
    assert!(has(&card(vec![fresh()]), "Yurt wants to remember 1 thing — keep or forget it on the Memory screen"));
}

// the notice on the Memory screen
fn notice(changed: MemoryChanged) -> Option<String> {
    let mut s = State::default();
    heard_memory_changed(&mut s, &changed);
    s.people_panel.memory_notice
}

#[test]
fn names_each_change_for_what_it_is_in_sentences_no_after_a_quoted_memory() {
    let said = notice(MemoryChanged {
        by: "Booty".into(),
        proposed_changes: vec!["Deploys go out on Thursdays.".into()],
        proposed_forgets: vec!["The API is on port 3000.".into()],
        ..Default::default()
    });
    assert_eq!(said.as_deref(), Some("Booty wants to change a memory to \"Deploys go out on Thursdays.\" Booty wants to forget \"The API is on port 3000.\""));
}

#[test]
fn ends_a_clause_whose_memory_has_no_full_stop_of_its_own() {
    let said = notice(MemoryChanged { by: "Wren".into(), kept: vec!["Build with pnpm build".into()], ..Default::default() });
    assert_eq!(said.as_deref(), Some("Wren remembered \"Build with pnpm build\"."));
}

#[test]
fn is_nothing_at_all_when_nothing_happened() {
    assert_eq!(notice(MemoryChanged { by: "Wren".into(), ..Default::default() }), None);
}

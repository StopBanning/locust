//! Electron's `a-conversation-keeps-its-name-and-group.test.ts`, case by case: the ids a conversation has worn
//! (conversation_list.rs `conversation_keys`), what was saved against any of them (`held_for`) as the sidebar reads it
//! (`recent_as_missions`), and the header that says the sidebar's name (`conversation_title_for`, thread_view.rs
//! `header_name`).
use super::*;
use crate::conversation_list::{conversation_keys, conversation_title_for, held_for};

/// A three-turn conversation root -> mid -> leaf (named "handoff-export-polish.md" by its first words) and another.
fn conversation() -> State {
    let mut s = State::default();
    s.demo = true;
    for (id, parent, words, second) in [("leaf", Some("mid"), "continue mb", "03"), ("mid", Some("root"), "and the export", "02"),
        ("root", None, "handoff-export-polish.md", "01"), ("other", None, "Something else", "00")] {
        let at = format!("2026-11-03T10:00:{second}Z");
        s.recent.push(ConversationRow { id: id.into(), title: words.into(), teammate: String::new(), started_by: "native".into(),
            state: "completed".into(), updated: at.clone() });
        s.ledger_summaries.insert(id.into(), ledger::Summary { mission_id: id.into(), title: words.into(), words: words.into(),
            parent: parent.map(Into::into), runtime: "claude".into(), created_at: at.clone(), updated: at, ..Default::default() });
    }
    s
}

fn the_row(s: &State) -> SidebarMission {
    recent_as_missions(s).into_iter().find(|m| m.member_ids.contains(&"leaf".to_owned())).expect("the conversation's row")
}

fn group(id: &str) -> org::GroupMembership {
    org::GroupMembership { group_id: id.into(), at: None }
}

fn mission(id: &str, root: Option<&str>, members: &[&str]) -> SidebarMission {
    SidebarMission { mission_id: id.into(), title: "t".into(), phase: "completed".into(), last_at: None, root_id: root.map(Into::into),
        member_ids: members.iter().map(|m| (*m).to_owned()).collect(), owner_id: None, said: String::new(), nested_under: None,
        integrity_issue_count: 0, needs_you: false, app_started: false }
}

#[test]
fn its_keys_hold_the_root_the_row_and_every_turn() {
    // case: includes the root, the row and every turn
    let keys = conversation_keys(&the_row(&conversation()));
    for id in ["root", "mid", "leaf"] {
        assert!(keys.contains(&id.to_owned()), "{id} in {keys:?}");
    }
}

#[test]
fn its_keys_list_each_id_once() {
    // case: lists each one once
    assert_eq!(conversation_keys(&mission("a", Some("a"), &["a"])), ["a"]);
    let mut s = conversation();
    s.recent.retain(|r| r.id == "other");
    assert_eq!(conversation_keys(&recent_as_missions(&s)[0]), ["other"]);
}

#[test]
fn its_keys_start_with_the_root() {
    // case: prefers the root, which is the stable identity
    assert_eq!(conversation_keys(&mission("leaf", Some("root"), &[]))[0], "root");
    assert_eq!(conversation_keys(&the_row(&conversation()))[0], "root");
}

#[test]
fn a_name_typed_on_a_follow_up_turn_is_found() {
    // case: finds a name stored against a follow-up turn
    let mut s = conversation();
    s.org.titles.insert("leaf".into(), "code".into());
    assert_eq!(the_row(&s).title, "code");
}

#[test]
fn a_group_stored_against_the_root_is_found() {
    // case: finds a group stored against the root
    let mut s = conversation();
    s.org.members.insert("root".into(), group("grp_locust"));
    assert_eq!(held_for(&the_row(&s), &s.org.members).map(|m| m.group_id.as_str()), Some("grp_locust"));
}

#[test]
fn the_roots_name_wins_when_both_were_written() {
    // case: prefers the root when both were written, so the newer identity wins
    let mut s = conversation();
    s.org.titles.insert("root".into(), "from the root".into());
    s.org.titles.insert("leaf".into(), "from the leaf".into());
    assert_eq!(the_row(&s).title, "from the root");
}

#[test]
fn nothing_saved_for_any_turn_is_nothing() {
    // case: answers undefined when nothing was saved for any of them
    let mut s = conversation();
    s.org.titles.insert("other_conversation".into(), "x".into());
    assert_eq!(held_for(&the_row(&s), &s.org.titles), None);
    assert_eq!(the_row(&s).title, "handoff-export-polish.md", "the derived title stays");
}

#[test]
fn a_single_turn_conversation_with_no_root_is_found_by_its_own_id() {
    // case: works for a single-turn conversation with no root recorded
    assert_eq!(held_for(&mission("only", None, &[]), &HashMap::from([("only".to_owned(), "named")])), Some(&"named"));
    let mut s = conversation();
    s.org.titles.insert("other".into(), "named".into());
    assert!(recent_as_missions(&s).iter().any(|m| m.mission_id == "other" && m.title == "named"));
}

#[test]
fn the_header_finds_the_row_by_any_turn() {
    // case: finds the row by any turn the conversation has had
    let rows = recent_as_missions(&conversation());
    for id in ["leaf", "mid", "root"] {
        assert_eq!(conversation_title_for(&rows, Some(id)), Some("handoff-export-polish.md"), "{id}");
    }
}

#[test]
fn a_run_no_row_holds_has_no_name_to_give() {
    // case: says nothing for a run no row holds, so the header falls back to the prompt
    let rows = recent_as_missions(&conversation());
    assert_eq!(conversation_title_for(&rows, Some("pending:3")), None);
    assert_eq!(conversation_title_for(&rows, None), None);
}

/// The conversation's header for the turn `tip`, opened as a conversation of `owner` (none: a teammate-less one).
fn header(s: &mut State, tip: &str, owner: Option<&str>, prompt: &str) -> String {
    let header = match owner {
        Some(name) => Teammate { id: "tm_wren".into(), name: name.into(), role: "Code".into(), runtime: Some("claude".into()), model: None,
            account: None, busy: false, runs_in: "ask".into(), folder: None },
        None => Teammate { id: LEDGER_TARGET.into(), name: "Claude".into(), role: "Conversation".into(), runtime: Some("claude".into()),
            model: None, account: None, busy: false, runs_in: String::new(), folder: None },
    };
    s.teammates = owner.map(|_| vec![header.clone()]).unwrap_or_default();
    let mut snap = ledger::Snapshot::default();
    snap.tip = tip.into();
    let conversation = || Conversation { conversation_id: Some(tip.into()), turns: vec![("You".into(), prompt.into())] };
    s.conversations.insert(header.id.clone(), conversation());
    s.ledger = Some(LedgerPane { cold_tip: None, header, conversation: conversation(), items: vec![], dir: std::path::PathBuf::new(), worker: None,
        snapshot: snap, due: f64::INFINITY, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0 });
    let state = std::mem::take(s);
    let mut rt = rune::Runtime::new(state, |s, _| crate::thread_view(s, &Palette::new(true)), (1000.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let name = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("thread-header-name")).and_then(|e| e.text).unwrap_or_default();
    *s = rt.state;
    name
}

#[test]
fn the_header_says_the_sidebars_name_before_the_prompt_after_a_teammate() {
    // case: the header reads it before the prompt, after a teammate
    let mut s = conversation();
    s.org.titles.insert("mid".into(), "Export polish".into());
    // A conversation of no teammate: the sidebar's name, not the newest turn's words.
    assert_eq!(header(&mut s, "leaf", None, "continue mb"), "Export polish");
    // A teammate's conversation: the teammate's name comes first.
    assert_eq!(header(&mut s, "leaf", Some("Wren"), "continue mb"), "Wren");
    // A run no row holds yet: the words it was opened with.
    assert_eq!(header(&mut s, "pending:3", None, "continue mb"), "continue mb");
}

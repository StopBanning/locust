//! Electron's `a-waiting-conversation-is-first-in-the-sidebar.test.tsx`, case by case, on the drawn sidebar: a
//! conversation waiting on the person stands first under Needs you, with the amber mark and words, drawn once.
use super::*;

const AT: &str = "2026-10-11T05:00:00.000Z";

fn add(s: &mut State, id: &str, phase: &str, nested_under: Option<&str>) {
    s.recent.push(ConversationRow { id: id.into(), title: format!("Conversation {id}"), teammate: "Wren".into(),
        started_by: "native".into(), state: phase.into(), updated: AT.into() });
    s.ledger_summaries.insert(id.into(), ledger::Summary {
        mission_id: id.into(), title: format!("Conversation {id}"), runtime: "codex".into(),
        created_at: AT.into(), updated: AT.into(),
        // Started by a teammate from `nested_under`: the sidebar nests it there (relay.rs `nested_under`).
        started_by_kind: Some(if nested_under.is_some() { "relay" } else { "native" }.into()),
        relay: nested_under.map(|parent| locust_core::relay::RelayOrigin { hop: 1, root_mission_id: Some(parent.into()), ..Default::default() }),
        ..Default::default()
    });
}

fn draw(s: State) -> rune::Runtime<State> {
    let mut rt = rune::Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (280.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(16.0);
    rt
}

fn drawn(rt: &rune::Runtime<State>, key: &str) -> usize {
    rt.elements().iter().filter(|e| !e.exiting && e.key.as_deref() == Some(key)).count()
}

fn wren() -> State {
    let mut s = State::default();
    s.demo = true;
    s.teammates.push(Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Code & Migrations".into(), runtime: None,
        model: None, account: None, busy: false, runs_in: "ask".into(), folder: None });
    s
}

#[test]
fn keeps_a_waiting_parent_and_its_nested_conversations_visible_exactly_once() {
    // case: keeps a waiting parent and its nested conversations visible exactly once
    let mut s = wren();
    add(&mut s, "waiting", "completed", None);
    add(&mut s, "child", "completed", Some("waiting"));
    s.home.waiting_for.insert("waiting".into(), "A decision".into());
    let rt = draw(s);
    assert_eq!(drawn(&rt, "recent-waiting"), 1);
    assert_eq!(drawn(&rt, "recent-child"), 1);
    assert_eq!(drawn(&rt, "nested-child"), 1, "the child is drawn nested under the waiting one");
    assert_eq!(drawn(&rt, "sidebar-needs-you"), 1);
    let (_, needs) = rt.find("sidebar-needs-you").unwrap();
    let (_, parent) = rt.find("recent-waiting").unwrap();
    let (_, child) = rt.find("recent-child").unwrap();
    assert!(needs < parent && parent < child, "Needs you, then the waiting one, then its child: {needs} {parent} {child}");
}

#[test]
fn stands_above_a_running_row_with_an_amber_mark_and_words_even_when_nested() {
    // case: stands above a running row with an amber mark and words, even when nested
    let mut s = wren();
    add(&mut s, "working", "running", None);
    add(&mut s, "waiting", "completed", Some("working"));
    s.home.waiting_for.insert("waiting".into(), "A decision".into());
    let rt = draw(s);
    let (_, waiting) = rt.find("recent-waiting").unwrap();
    let (_, working) = rt.find("recent-working").unwrap();
    assert!(waiting < working, "the waiting one stands above the running one: {waiting} {working}");
    assert_eq!(drawn(&rt, "conversation-needs-you-dot-waiting"), 1, "the amber mark");
    let words = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("conversation-needs-you-waiting")).unwrap();
    assert_eq!(words.text.as_deref(), Some("needs you"));
    assert_eq!(words.text_color, Some(Palette::new(true).amber));
    let dot = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("conversation-needs-you-dot-waiting")).unwrap();
    assert_eq!(dot.fill, Palette::new(true).amber);
    assert_eq!(drawn(&rt, "recent-waiting"), 1);
    assert_eq!(drawn(&rt, "nested-waiting"), 0, "a waiting one is never only a nested child");
    // The running one wears neither: the mark is for what waits.
    assert_eq!(drawn(&rt, "conversation-needs-you-dot-working"), 0);
    assert_eq!(drawn(&rt, "conversation-needs-you-working"), 0);
}

#[test]
fn a_settled_conversation_that_waits_is_still_listed_first_and_the_keys_walk_it_first() {
    // Sidebar.tsx's filter (`mission.needsYou || ... || !isPutAway`) and the keyboard walking the order drawn.
    let mut s = wren();
    add(&mut s, "newer", "completed", None);
    add(&mut s, "settled", "completed", None);
    s.recent[1].updated = "2026-10-10T05:00:00.000Z".into();
    s.org.settled.insert("settled".into(), crate::settled::SettledEntry { at: "2026-10-12T05:00:00.000Z".into(), until: None });
    let rt = draw(s);
    assert_eq!(drawn(&rt, "recent-settled"), 0, "settled and not waiting: out of the list");
    let mut s = rt.state;
    s.home.waiting_for.insert("settled".into(), "A decision".into());
    let rt = draw(s);
    assert_eq!(drawn(&rt, "recent-settled"), 1, "settled but waiting: still listed");
    let (_, waiting) = rt.find("recent-settled").unwrap();
    let (_, newer) = rt.find("recent-newer").unwrap();
    assert!(waiting < newer, "the older one that waits stands above the newer one");
    // Alt+Up/Down walk the rows in the order drawn: the waiting one first.
    let order: Vec<_> = drawn_missions(&rt.state).into_iter().map(|m| m.mission_id).collect();
    assert_eq!(order, ["settled", "newer"]);
}

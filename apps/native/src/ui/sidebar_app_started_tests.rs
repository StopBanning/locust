//! Electron's `an-app-started-conversation-says-so-in-the-sidebar.test.tsx`, case by case, on the drawn sidebar and
//! conversation_list.rs's `conversation_source_matches` and `app_started_conversation`.
use super::*;
use crate::conversation_list::{ConversationSource, app_started_conversation, conversation_source_matches};

const AT: &str = "2026-10-11T05:00:00.000Z";

/// A turn of a conversation: `kind` is its `startedBy.kind`, `parent` the turn it continues.
fn turn(s: &mut State, id: &str, kind: Option<&str>, parent: Option<&str>, at: &str) {
    s.recent.push(ConversationRow { id: id.into(), title: format!("Conversation {id}"), teammate: "Wren".into(),
        started_by: kind.unwrap_or("native").into(), state: "completed".into(), updated: at.into() });
    s.ledger_summaries.insert(id.into(), ledger::Summary {
        mission_id: id.into(), title: format!("Conversation {id}"), runtime: "codex".into(), parent: parent.map(Into::into),
        created_at: at.into(), updated: at.into(), started_by_kind: kind.map(Into::into), ..Default::default()
    });
}

fn draw(s: State) -> rune::Runtime<State> {
    let mut rt = rune::Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (280.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.016);
    rt
}

/// The whole window, wide enough for the sidebar: the source list opens over it (main.rs `view`).
fn window(s: State) -> rune::Runtime<State> {
    let mut rt = rune::Runtime::new(s, crate::view, (1300.0, 900.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.016);
    rt
}

fn marks(rt: &rune::Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting && e.role == "img" && e.key.as_deref().is_some_and(|k| k.starts_with("conversation-app-")))
        .filter_map(|e| e.key).collect()
}

fn said(rt: &rune::Runtime<State>) -> String {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text.or(e.label)).collect::<Vec<_>>().join("\n")
}

fn listed(rt: &rune::Runtime<State>) -> Vec<String> {
    let mut rows: Vec<_> = rt.elements().into_iter()
        .filter(|e| !e.exiting && e.key.as_deref().is_some_and(|k| k.starts_with("recent-")))
        .map(|e| (e.rect[1], e.key.unwrap().trim_start_matches("recent-").to_owned())).collect();
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    rows.into_iter().map(|(_, id)| id).collect()
}

fn press(rt: &mut rune::Runtime<State>, x: f32, y: f32) {
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.032);
    rt.frame(1.0);
}

fn app_and_person() -> State {
    let mut s = State::default();
    s.demo = true;
    turn(&mut s, "from_app", Some("mcp"), None, AT);
    turn(&mut s, "from_person", None, None, "2026-10-11T04:00:00.000Z");
    s
}

#[test]
fn has_a_quiet_accessible_origin_mark_and_offers_the_source_filter() {
    // case: has a quiet accessible origin mark and offers the source filter
    let rt = draw(app_and_person());
    assert_eq!(marks(&rt), ["conversation-app-from_app"], "one plug, on the app-started row");
    let plug = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("conversation-app-from_app")).unwrap();
    assert_eq!((plug.role, plug.label.as_deref()), ("img", Some("Started by another app through Locust")));
    // Before the title, as a routine's clock is.
    let (plug_x, _) = rt.find("conversation-app-from_app").unwrap();
    let (title_x, _) = rt.find("conversation-title-from_app").unwrap();
    assert!(plug_x < title_x);
    assert!(!said(&rt).to_lowercase().contains("via app"), "no words after the title");
    let menu = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("sidebar-source-trigger")).expect("the source menu");
    assert_eq!((menu.role, menu.label.as_deref()), ("combobox", Some("Conversation source")));
    assert!(said(&rt).contains("All conversations"));
    // It opens a list of the three sources (`aria-haspopup="listbox"`), over the window.
    let mut rt = window(app_and_person());
    let (x, y) = rt.find("sidebar-source-trigger").unwrap();
    press(&mut rt, x, y);
    assert!(rt.elements().iter().any(|e| e.role == "listbox" && e.label.as_deref() == Some("Conversation source")));
    let options: Vec<_> = rt.elements().into_iter().filter(|e| e.role == "option").filter_map(|e| e.label.or(e.text)).collect();
    assert_eq!(options, ["All conversations", "Started here", "Via apps"]);
}

#[test]
fn leaves_ordinary_rows_quiet_and_offers_no_unnecessary_filter() {
    // case: leaves ordinary rows quiet and offers no unnecessary filter
    let mut s = State::default();
    s.demo = true;
    turn(&mut s, "person", None, None, AT);
    let rt = draw(s);
    assert!(marks(&rt).is_empty());
    assert!(!said(&rt).contains("Started by another app"));
    assert!(!said(&rt).contains("Conversation source"));
    assert!(rt.find("sidebar-source-trigger").is_none());
}

fn mission(id: &str, app_started: bool) -> crate::conversation_list::SidebarMission {
    crate::conversation_list::SidebarMission { mission_id: id.into(), title: id.into(), phase: "completed".into(),
        last_at: Some(AT.into()), root_id: None, member_ids: vec![], owner_id: None, said: String::new(), nested_under: None,
        integrity_issue_count: 0, needs_you: false, app_started }
}

#[test]
fn filters_app_starts_independently_of_search_and_teammate_selection() {
    // case: filters app starts independently of search and teammate selection
    let (app, person) = (mission("app", true), mission("person", false));
    let kept = |source| [&app, &person].into_iter().filter(|m| conversation_source_matches(m, source)).map(|m| m.mission_id.as_str()).collect::<Vec<_>>();
    assert_eq!(kept(ConversationSource::All), ["app", "person"]);
    assert_eq!(kept(ConversationSource::App), ["app"]);
    assert_eq!(kept(ConversationSource::Person), ["person"]);
    // In the window: choosing a source narrows the list and says so, and a search narrows within it.
    let mut rt = window(app_and_person());
    assert_eq!(listed(&rt), ["from_app", "from_person"]);
    let choose = |rt: &mut rune::Runtime<State>, words: &str| {
        let (x, y) = rt.find("sidebar-source-trigger").unwrap();
        press(rt, x, y);
        let option = rt.elements().into_iter().find(|e| e.role == "option" && e.label.as_deref().or(e.text.as_deref()) == Some(words)).unwrap();
        press(rt, option.rect[0] + option.rect[2] / 2.0, option.rect[1] + option.rect[3] / 2.0);
    };
    choose(&mut rt, "Via apps");
    assert_eq!(rt.state.sidebar_source, ConversationSource::App);
    assert_eq!(listed(&rt), ["from_app"]);
    assert!(said(&rt).contains("Via apps · 1 of 2 conversations"), "{}", said(&rt));
    choose(&mut rt, "Started here");
    assert_eq!(listed(&rt), ["from_person"]);
    // The menu stays while it narrows, even once no app-started row is left to list.
    assert!(rt.find("sidebar-source-trigger").is_some());
    // A search inside Started here never brings an app's row back, and finds the person's own.
    let found = |s: &State| sidebar_missions_now(s).into_iter().map(|m| m.mission_id).collect::<Vec<_>>();
    rt.state.sidebar_query = TextEdit::new("from_app");
    assert!(found(&rt.state).is_empty());
    rt.state.sidebar_query = TextEdit::new("Conversation");
    assert_eq!(found(&rt.state), ["from_person"]);
    rt.state.sidebar_query = TextEdit::new("");
    choose(&mut rt, "All conversations");
    assert_eq!(listed(&rt), ["from_app", "from_person"]);
}

#[test]
fn keeps_the_first_turn_origin_when_either_the_app_or_person_follows_up_live_and_restored() {
    // case: keeps the first turn origin when either the app or person follows up, live and restored
    assert!(app_started_conversation(Some("mcp"), None, None));
    assert!(!app_started_conversation(None, None, None));
    assert!(!app_started_conversation(Some("mcp"), None, Some(None)));
    assert!(app_started_conversation(None, None, Some(Some("mcp"))));
    assert!(app_started_conversation(None, Some(Some("mcp")), None));
    assert!(!app_started_conversation(Some("mcp"), Some(None), None));
    assert!(!app_started_conversation(Some("routine"), None, None));
    // In the window: an app's conversation the person followed up keeps its plug; the person's one an app followed
    // up has none.
    let mut s = State::default();
    s.demo = true;
    turn(&mut s, "app_root", Some("mcp"), None, "2026-10-11T05:00:00.000Z");
    turn(&mut s, "app_reply", None, Some("app_root"), "2026-10-11T05:01:00.000Z");
    turn(&mut s, "person_root", None, None, "2026-10-11T04:00:00.000Z");
    turn(&mut s, "person_reply", Some("mcp"), Some("person_root"), "2026-10-11T04:01:00.000Z");
    let rt = draw(s);
    assert_eq!(listed(&rt), ["app_reply", "person_reply"], "one row each, on its latest turn");
    assert_eq!(marks(&rt), ["conversation-app-app_reply"]);
}

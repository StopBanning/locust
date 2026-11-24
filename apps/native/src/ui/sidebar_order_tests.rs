//! Electron's `the-flat-sidebar.test.ts`, case by case, on the sidebar the window draws: the order the list claims
//! (newest first, a live one above, no age last, the rail flyout's order, uncapped, the given rows left alone, a
//! comparator that does not reshuffle), who a row belongs to (`row_owner`), and the sentence above a narrowed list.
use super::*;
use rune::{Env, Runtime};

fn mate(id: &str, name: &str) -> Teammate {
    Teammate { id: id.into(), name: name.into(), role: "Custom".into(), runtime: None, model: None, account: None, busy: false, runs_in: "ask".into(), folder: None }
}

fn conv(id: &str, phase: &str, at: Option<&str>, teammate: &str) -> ConversationRow {
    ConversationRow {
        id: id.into(),
        title: format!("Conversation {id}"),
        teammate: teammate.into(),
        started_by: "native".into(),
        state: phase.into(),
        updated: at.unwrap_or_default().into(),
    }
}

fn state(teammates: Vec<Teammate>, recent: Vec<ConversationRow>) -> State {
    let mut s = State::default();
    s.demo = true;
    s.teammates = teammates;
    s.recent = recent;
    s
}

fn draw(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (crate::window_glass::SIDEBAR_WIDTH, 8000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

/// The ids of the rows with this key prefix, top to bottom.
fn order(rt: &Runtime<State>, prefix: &str) -> Vec<String> {
    let mut found: Vec<(f32, String)> = rt
        .elements()
        .into_iter()
        .filter(|e| !e.exiting)
        .filter_map(|e| e.key.and_then(|k| k.strip_prefix(prefix).map(str::to_owned)).map(|id| (e.rect[1], id)))
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.dedup_by(|a, b| a.1 == b.1);
    found.into_iter().map(|(_, id)| id).collect()
}

fn listed(recent: Vec<ConversationRow>) -> Vec<String> {
    order(&draw(state(vec![], recent)), "recent-")
}

// --- the order the list claims ----------------------------------------------------------------------------------

#[test]
fn puts_the_newest_first() {
    // case: puts the newest first
    let shown = listed(vec![
        conv("old", "completed", Some("2026-09-10T00:00:00.000Z"), ""),
        conv("new", "completed", Some("2026-09-15T00:00:00.000Z"), ""),
        conv("mid", "completed", Some("2026-09-12T00:00:00.000Z"), ""),
    ]);
    assert_eq!(shown, ["new", "mid", "old"]);
}

#[test]
fn puts_a_live_conversation_above_a_newer_finished_one() {
    // case: puts a live conversation above a newer finished one
    let shown = listed(vec![
        conv("newer", "completed", Some("2026-09-15T12:00:00.000Z"), ""),
        conv("live", "running", Some("2026-09-15T09:00:00.000Z"), ""),
    ]);
    assert_eq!(shown, ["live", "newer"]);
}

#[test]
fn sorts_a_row_with_no_timestamp_last_not_first() {
    // case: sorts a row with no timestamp last, not first
    let shown = listed(vec![conv("unknown", "completed", None, ""), conv("known", "completed", Some("2026-09-01T00:00:00.000Z"), "")]);
    assert_eq!(shown, ["known", "unknown"]);
}

#[test]
fn is_the_same_order_the_rail_flyout_uses() {
    // case: is the same order the rail flyout uses
    let recent = vec![
        conv("a", "completed", Some("2026-09-11T00:00:00.000Z"), "Wren"),
        conv("b", "running", Some("2026-09-01T00:00:00.000Z"), "Wren"),
        conv("c", "completed", Some("2026-09-14T00:00:00.000Z"), "Wren"),
    ];
    let sidebar_order = order(&draw(state(vec![mate("tm_wren", "Wren")], recent.clone())), "recent-");
    let mut s = state(vec![mate("tm_wren", "Wren")], recent);
    s.rail_open = Some("tm_wren".into());
    let mut rt = Runtime::new(s, |s, _| crate::rail::flyout(s, &Palette::new(true), 0.0).expect("the flyout is open"), (360.0, 1200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let rail_order = order(&rt, "rail-row-");
    assert_eq!(rail_order, ["b", "c", "a"]);
    assert_eq!(rail_order, sidebar_order);
}

#[test]
fn does_not_cap_the_list_because_the_sidebar_is_not_a_popover() {
    // case: does not cap the list, because the sidebar is not a popover
    let many = (0..40).map(|i| conv(&format!("m{i}"), "completed", None, "")).collect();
    assert_eq!(listed(many).len(), 40);
}

#[test]
fn leaves_the_rows_it_was_given_alone() {
    // case: leaves the rows it was given alone
    let rt = draw(state(vec![], vec![conv("b", "completed", None, ""), conv("a", "completed", Some("2026-09-14T00:00:00.000Z"), "")]));
    assert_eq!(order(&rt, "recent-"), ["a", "b"], "the list is ordered");
    assert_eq!(rt.state.recent[0].id, "b", "the rows the window holds are not");
}

#[test]
fn is_antisymmetric_so_the_list_does_not_reshuffle_between_renders() {
    // case: is antisymmetric, so the list does not reshuffle between renders
    use std::cmp::Ordering;
    let s = state(vec![], vec![conv("x", "completed", Some("2026-09-14T00:00:00.000Z"), ""), conv("y", "completed", Some("2026-09-10T00:00:00.000Z"), "")]);
    let rows = recent_as_missions(&s);
    let newer = rows.iter().find(|m| m.mission_id == "x").unwrap();
    let older = rows.iter().find(|m| m.mission_id == "y").unwrap();
    assert_eq!(crate::conversation_list::by_live_then_recent(newer, older), Ordering::Less);
    assert_eq!(crate::conversation_list::by_live_then_recent(older, newer), Ordering::Greater);
    assert_eq!(crate::conversation_list::by_live_then_recent(newer, newer), Ordering::Equal);
    // Two rows it cannot separate keep their places from one render to the next.
    let mut rt = draw(state(vec![], vec![conv("p", "completed", None, ""), conv("q", "completed", None, ""), conv("r", "completed", Some("2026-09-14T00:00:00.000Z"), "")]));
    let first = order(&rt, "recent-");
    rt.frame(0.5);
    assert_eq!(order(&rt, "recent-"), first);
    assert_eq!(first, ["r", "p", "q"]);
}

// --- who a conversation belongs to ------------------------------------------------------------------------------

#[test]
fn asks_the_shell_first_and_the_host_second() {
    // case: asks the shell first and the host second
    let temp = std::env::temp_dir().join(format!("locust-sb-owner-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(temp.join("electron")).unwrap();
    std::fs::create_dir_all(temp.join("native")).unwrap();
    let store = crate::store::Store::new(temp.join("native"), temp.join("electron")).unwrap();
    store
        .save_json("teammates.json", &serde_json::json!({"schemaVersion": 1, "teammates": [], "missionOwners": {"m": "tm_host"}, "settings": {}}))
        .unwrap();
    let mut s = state(vec![], vec![conv("m", "completed", None, ""), conv("n", "completed", None, "")]);
    s.data = Some(store);
    let rows = recent_as_missions(&s);
    let mut m = rows.iter().find(|r| r.mission_id == "m").unwrap().clone();
    // The host's record, when the shell knows nothing.
    assert_eq!(row_owner(&s, &m).as_deref(), Some("tm_host"));
    // The shell's own owner wins over it.
    m.owner_id = Some("tm_live".into());
    assert_eq!(row_owner(&s, &m).as_deref(), Some("tm_live"));
    // Neither knows: nobody.
    let n = rows.iter().find(|r| r.mission_id == "n").unwrap();
    assert_eq!(row_owner(&s, n), None);
    let _ = std::fs::remove_dir_all(&temp);
}

// --- the sentence above a narrowed list -------------------------------------------------------------------------

#[test]
fn counts_against_the_face_when_a_face_is_on_and_against_the_folder_when_only_search_is() {
    // case: counts against the face when a face is on, and against the folder when only search is
    let mut recent = vec![conv("a0", "completed", None, "Atlas")];
    recent[0].title = "Invoice totals".into();
    recent.extend((1..5).map(|i| conv(&format!("a{i}"), "completed", None, "Atlas")));
    recent.extend((0..15).map(|i| conv(&format!("o{i}"), "completed", None, "Sable")));
    recent[5].title = "Chase the invoice".into();
    recent[6].title = "invoice numbers".into();
    let said = |face: Option<&str>, query: &str| {
        let mut s = state(vec![mate("tm_atlas", "Atlas"), mate("tm_sable", "Sable")], recent.clone());
        s.sidebar_face_filter = face.map(Into::into);
        s.sidebar_query = TextEdit::new(query);
        let rt = draw(s);
        rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some("sidebar-faces-clearsaid")).and_then(|e| e.text)
    };
    assert_eq!(said(Some("tm_atlas"), "").as_deref(), Some("Atlas — 5 of 5"));
    assert_eq!(said(Some("tm_atlas"), "invoice").as_deref(), Some("Atlas · \"invoice\" — 1 of 5"));
    assert_eq!(said(None, "invoice").as_deref(), Some("\"invoice\" — 3 of 20"));
    assert_eq!(said(None, "  "), None);
}

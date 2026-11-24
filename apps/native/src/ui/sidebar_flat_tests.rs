//! THE FLAT SIDEBAR, AS IT IS WRITTEN (`main/the-flat-sidebar-is-written-down.test.ts`): one-line conversation rows
//! whose marks all take the owner's face footprint, the roster as one capped row of faces (+N, Team at the end) in the
//! wide sidebar only, one sentence and one Clear for the face filter and the search together, and a way in for
//! Conversations, Rooms, Routines, Settings and the roster. Electron reads its stylesheet; these draw the sidebar and
//! measure it. The title cases ("the title is cut by the column") are core's `ledger::mission_title_tests`.
use super::*;
use rune::{Env, Runtime};

/// Drawn at the sidebar's own width, as the window gives it (the root takes the whole viewport).
const SIDEBAR_W: f32 = crate::window_glass::SIDEBAR_WIDTH;

fn mate(id: &str, name: &str) -> Teammate {
    Teammate {
        id: id.into(),
        name: name.into(),
        role: "Custom".into(),
        runtime: None,
        model: None,
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    }
}

fn conv(id: &str, title: &str, teammate: &str, minutes_ago: i64) -> ConversationRow {
    let at = chrono::DateTime::parse_from_rfc3339("2026-11-20T12:00:00Z").unwrap() - chrono::Duration::minutes(minutes_ago);
    ConversationRow {
        id: id.into(),
        title: title.into(),
        teammate: teammate.into(),
        started_by: "native".into(),
        state: "completed".into(),
        updated: at.to_rfc3339(),
    }
}

/// A team of `n` named t0..t(n-1), made in that order.
fn team(n: usize) -> Vec<Teammate> {
    (0..n).map(|i| mate(&format!("tm_{i}"), &format!("T{i}"))).collect()
}

fn state(teammates: Vec<Teammate>, recent: Vec<ConversationRow>) -> State {
    let mut s = State::default();
    s.demo = true;
    s.teammates = teammates;
    s.recent = recent;
    s
}

fn draw(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (SIDEBAR_W, 1400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

/// The whole window, wide or compact as asked.
fn window(mut s: State, layout: &str) -> Runtime<State> {
    s.layout_pref = layout.into();
    let mut rt = Runtime::new(s, crate::view, (1400.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.05);
    rt
}

fn layout(rt: &Runtime<State>, key: &str) -> Option<[f32; 4]> {
    rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some(key)).map(|e| e.rect)
}

fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    layout(rt, key).unwrap_or_else(|| panic!("{key} is drawn"))
}

fn press(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("{key} is drawn"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
}

fn keys_starting(rt: &Runtime<State>, prefix: &str) -> Vec<String> {
    let mut found: Vec<(f32, f32, String)> = rt
        .elements()
        .into_iter()
        .filter(|e| !e.exiting)
        .filter_map(|e| e.key.filter(|k| k.starts_with(prefix)).map(|k| (e.rect[0], e.rect[1], k)))
        .collect();
    found.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.total_cmp(&b.0)));
    found.dedup_by(|a, b| a.2 == b.2);
    found.into_iter().map(|(_, _, k)| k).collect()
}

/// The faces in the strip, left to right.
fn strip_faces(rt: &Runtime<State>) -> Vec<String> {
    let strip = rect(rt, "sidebar-faces");
    let mut faces: Vec<(f32, String)> = rt
        .elements()
        .into_iter()
        .filter_map(|e| {
            let key = e.key?;
            let id = key.strip_prefix("teammate-")?.to_owned();
            (e.rect[1] >= strip[1] && e.rect[1] < strip[1] + strip[3]).then_some((e.rect[0], id))
        })
        .collect();
    faces.sort_by(|a, b| a.0.total_cmp(&b.0));
    faces.into_iter().map(|(_, id)| id).collect()
}

fn texts(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

const LONG: &str = "Go through every file under apps/native/src/ui and tell me which of them still draws the old two-line conversation row, then fix each";

// --- the conversation row ---------------------------------------------------------------------------------------

#[test]
fn lets_only_the_title_give_so_nothing_is_pushed_off_the_edge() {
    // case: lets only the title give, so nothing is pushed off the edge
    let mut s = state(vec![mate("tm_wren", "Wren")], vec![conv("m_long", LONG, "Wren", 0)]);
    // Picked, so the row carries its actions at the end, where a long title would push them.
    let mut open = Conversation::default();
    open.conversation_id = Some("m_long".into());
    s.conversations.insert("tm_wren".into(), open);
    let rt = draw(s);
    let row = rect(&rt, "recent-m_long");
    let title = rect(&rt, "conversation-title-m_long");
    let menu = rect(&rt, "org-menu-open-m_long");
    assert!(menu[0] + menu[2] <= row[0] + row[2] + 0.5, "the actions stay inside the row: {menu:?} in {row:?}");
    assert!(row[0] + row[2] <= SIDEBAR_W, "the row stays inside the column: {row:?}");
    assert!(title[0] + title[2] <= menu[0], "the title gives way to the actions: {title:?} then {menu:?}");
    assert!(title[2] < 200.0, "the title is cut, not drawn at its full length: {title:?}");
}

#[test]
fn is_one_line_never_two() {
    // case: is one line, never two
    let s = state(vec![mate("tm_wren", "Wren")], vec![conv("m_long", LONG, "Wren", 0), conv("m_short", "Hi", "Wren", 1)]);
    let rt = draw(s);
    for key in ["recent-m_long", "recent-m_short"] {
        assert_eq!(rect(&rt, key)[3], 32.0, "{key} is `.lc-conv`'s 32 high");
    }
    // No second line: nothing under the title says who or when (the old subtitle "Wren · native · …").
    assert!(!texts(&rt).iter().any(|t| t.contains(" · native")), "{:?}", texts(&rt));
}

#[test]
fn keeps_the_unowned_row_aligned_with_every_other_one() {
    // case: keeps the unowned row aligned with every other one
    let s = state(vec![mate("tm_wren", "Wren")], vec![conv("m_owned", "Owned", "Wren", 0), conv("m_nobody", "Nobody's", "", 1)]);
    let rt = draw(s);
    let nobody = rect(&rt, "conversation-nobody-m_nobody");
    assert_eq!(nobody[2], 16.0, "`.lc-conv__nobody` is 16 wide");
    assert_eq!(rect(&rt, "conversation-title-m_nobody")[0], rect(&rt, "conversation-title-m_owned")[0]);
}

#[test]
fn gives_every_rows_mark_the_owners_face_footprint_whatever_size_the_face_is() {
    // case: gives every row's mark the owner's face footprint, whatever size the face is (0.561)
    let mut s = state(
        vec![mate("tm_wren", "Wren")],
        vec![conv("m_owned", "Owned", "Wren", 0), conv("m_model", "A model's", "", 1), conv("m_nobody", "Nobody's", "", 2)],
    );
    s.ledger_summaries.insert("m_model".into(), locust_core::ledger::Summary {
        mission_id: "m_model".into(),
        runtime: "claude".into(),
        model: "opus".into(),
        ..Default::default()
    });
    let rt = draw(s);
    // `BOT_SIZE.conversationOwner` is 20 (botSizes.ts), more than the nobody box's 16.
    assert_eq!(rect(&rt, "conversation-owner-m_owned")[2], 20.0);
    assert_eq!(rect(&rt, "conversation-runtime-m_model")[2], 20.0);
    let nobody = rect(&rt, "conversation-nobody-m_nobody");
    let owned_mark = rect(&rt, "conversation-owner-m_owned");
    assert_eq!(nobody[0] - owned_mark[0], 2.0, "16 wide and 2 either side make the face's 20");
    let x = rect(&rt, "conversation-title-m_owned")[0];
    for id in ["m_model", "m_nobody"] {
        assert_eq!(rect(&rt, &format!("conversation-title-{id}"))[0], x, "{id}'s title starts where the owned one's does");
    }
}

// --- the roster is one row, and the rail is left alone -------------------------------------------------------------

#[test]
fn draws_the_faces_only_in_the_wide_sidebar() {
    // case: draws the faces only in the wide sidebar
    let wide = window(state(team(3), vec![]), "wide");
    assert_eq!(strip_faces(&wide).len(), 3);
    let compact = window(state(team(3), vec![]), "compact");
    assert!(layout(&compact, "sidebar-faces").is_none(), "the rail draws no faces strip");
    assert_eq!(keys_starting(&compact, "rail-tm_").len(), 3, "the rail draws its own faces instead");
    // And none at all with no team (`teammates.length > 0`).
    assert!(layout(&draw(state(vec![], vec![])), "sidebar-faces").is_none());
}

#[test]
fn keeps_the_way_to_the_full_roster_in_a_fixed_place() {
    // case: keeps the way to the full roster in a fixed place
    let at = |n: usize| rect(&draw(state(team(n), vec![])), "sidebar-faces-team");
    let one = at(1);
    for n in [3, 5, 12] {
        let team_at = at(n);
        assert_eq!((team_at[0], team_at[1]), (one[0], one[1]), "Team does not move with {n} teammates");
    }
    // At the row's end (`margin-left: auto`), inside the column.
    let strip = rect(&draw(state(team(1), vec![])), "sidebar-faces");
    assert!((one[0] + one[2] - (strip[0] + strip[2] - 10.0)).abs() < 0.5, "Team ends at the strip's padding: {one:?} {strip:?}");
}

/// Wren owns two conversations, Sable one; one of Wren's and Sable's say "parser".
fn filtered(face: Option<&str>, query: &str) -> Runtime<State> {
    let mut s = state(
        vec![mate("tm_wren", "Wren"), mate("tm_sable", "Sable")],
        vec![conv("m_w1", "Fix the parser", "Wren", 0), conv("m_w2", "Tidy the docs", "Wren", 1), conv("m_s1", "Parser tests", "Sable", 2)],
    );
    s.sidebar_face_filter = face.map(Into::into);
    s.sidebar_query = TextEdit::new(query);
    draw(s)
}

fn said(rt: &Runtime<State>) -> Option<String> {
    rt.elements().into_iter().find(|e| !e.exiting && e.key.as_deref() == Some("sidebar-faces-clearsaid")).and_then(|e| e.text)
}

#[test]
fn says_when_the_list_is_filtered_in_one_sentence_covering_both_filters() {
    // case: says when the list is filtered, in one sentence covering both filters
    // conversationList.ts `narrowingLine`: `${narrowedBy.join(' · ')} — ${shown} of ${of}`.
    assert_eq!(said(&filtered(Some("tm_wren"), "parser")).as_deref(), Some("Wren · \"parser\" — 1 of 2"));
    assert_eq!(said(&filtered(Some("tm_wren"), "")).as_deref(), Some("Wren — 2 of 2"));
    assert_eq!(said(&filtered(None, "parser")).as_deref(), Some("\"parser\" — 2 of 3"));
    // An unfiltered list says nothing.
    assert_eq!(said(&filtered(None, "")), None);
    // And the face's filter narrows the rows themselves.
    let rt = filtered(Some("tm_wren"), "");
    assert_eq!(keys_starting(&rt, "recent-m_"), ["recent-m_w1", "recent-m_w2"]);
}

#[test]
fn carries_the_count_which_is_what_separates_filtered_from_empty() {
    // case: carries the count, which is what separates filtered from empty
    // Nothing of Sable's matches "docs": filtered, not empty, and the count says so.
    let mut s = state(
        vec![mate("tm_wren", "Wren"), mate("tm_sable", "Sable")],
        vec![conv("m_w1", "Tidy the docs", "Wren", 0), conv("m_s1", "Parser tests", "Sable", 1), conv("m_s2", "Ship it", "Sable", 2)],
    );
    s.sidebar_face_filter = Some("tm_sable".into());
    s.sidebar_query = TextEdit::new("docs");
    let rt = draw(s);
    assert_eq!(said(&rt).as_deref(), Some("Sable · \"docs\" — 0 of 2"));
    assert!(keys_starting(&rt, "recent-m_").is_empty());
}

#[test]
fn clears_both_at_once_since_it_offered_one_way_out_of_two_filters() {
    // case: clears both at once, since it offered one way out of two filters
    let mut rt = filtered(Some("tm_wren"), "parser");
    press(&mut rt, "sidebar-faces-clearlink");
    assert_eq!(rt.state.sidebar_face_filter, None);
    assert_eq!(rt.state.sidebar_query.text, "");
    rt.frame(0.05);
    assert_eq!(said(&rt), None);
    assert_eq!(keys_starting(&rt, "recent-m_").len(), 3);
}

#[test]
fn still_gives_the_rail_the_layout_it_already_had() {
    // case: still gives the rail the layout it already had
    let rt = window(state(team(3), vec![conv("m_1", "One", "T0", 0)]), "compact");
    let faces = keys_starting(&rt, "rail-tm_");
    assert_eq!(faces, ["rail-tm_0", "rail-tm_1", "rail-tm_2"], "a column of faces, in roster order");
    let xs: Vec<f32> = faces.iter().map(|k| rect(&rt, k)[0]).collect();
    assert!(xs.windows(2).all(|w| w[0] == w[1]), "one above another: {xs:?}");
    assert!(layout(&rt, "recent-m_1").is_none(), "the rail draws no conversation rows");
}

// --- nothing loses its way in ---------------------------------------------------------------------------------------

fn place(name: &str) -> State {
    let mut rt = draw(state(team(1), vec![conv("m_1", "One", "T0", 0)]));
    let key = format!("sidebar-place-{}", name.to_lowercase());
    // In the places list, above the conversations.
    let places = rect(&rt, "sidebar-places");
    let at = rect(&rt, &key);
    assert!(at[1] >= places[1] && at[1] + at[3] <= places[1] + places[3] + 0.5, "{name} is in the places list");
    assert!(at[1] < rect(&rt, "recent-m_1")[1], "{name} is above the conversations");
    assert!(texts(&rt).iter().any(|t| t == name), "{name} is said");
    press(&mut rt, &key);
    rt.state
}

#[test]
fn conversations_has_a_button_in_the_places_list_at_the_top() {
    // case: Conversations has a button in the places list at the top
    // case: ${name} has a button in the places list at the top
    assert!(place("Conversations").missions.open);
}

#[test]
fn rooms_has_a_button_in_the_places_list_at_the_top() {
    // case: Rooms has a button in the places list at the top
    assert!(matches!(place("Rooms").rooms.active, crate::room_screen::RoomView::List));
}

#[test]
fn routines_has_a_button_in_the_places_list_at_the_top() {
    // case: Routines has a button in the places list at the top
    assert!(place("Routines").routines.open);
}

#[test]
fn settings_has_a_button_in_the_footer() {
    // case: Settings has a button in the footer
    let mut rt = draw(state(team(1), vec![]));
    let footer = rect(&rt, "sidebar-footer");
    let settings = rect(&rt, "sidebar-footer-settings");
    assert!(settings[1] >= footer[1], "{settings:?} in {footer:?}");
    press(&mut rt, "sidebar-footer-settings");
    assert!(rt.state.settings_panel.open);
}

#[test]
fn the_roster_is_reachable_too_from_the_faces_row_rather_than_the_footer() {
    // case: the roster is reachable too, from the faces row rather than the footer
    let mut rt = draw(state(team(2), vec![]));
    let strip = rect(&rt, "sidebar-faces");
    let team_at = rect(&rt, "sidebar-faces-team");
    assert!(team_at[1] >= strip[1] && team_at[1] < strip[1] + strip[3], "Team is in the faces row");
    press(&mut rt, "sidebar-faces-team");
    assert!(rt.state.people_panel.open);
}

#[test]
fn gives_the_roster_its_full_word_back_which_the_narrow_row_could_not_hold() {
    // case: gives the roster its full word back, which the narrow row could not hold
    let rt = draw(state(team(2), vec![]));
    let footer = rect(&rt, "sidebar-footer");
    let in_footer: Vec<String> = rt
        .elements()
        .into_iter()
        .filter(|e| e.rect[1] >= footer[1] && e.rect[1] < footer[1] + footer[3])
        .filter_map(|e| e.text)
        .collect();
    assert!(in_footer.iter().any(|t| t == "Settings"), "{in_footer:?}");
    assert!(!in_footer.iter().any(|t| t == "Team" || t == "Teammates"), "the footer no longer carries the roster: {in_footer:?}");
    // The faces row's own Team is not the footer's.
    assert!(texts(&rt).iter().any(|t| t == "Team"));
}

#[test]
fn keeps_every_footer_cell_sharing_the_width_rather_than_claiming_it() {
    // case: keeps every footer cell sharing the width rather than claiming it
    let rt = draw(state(team(1), vec![]));
    let settings = rect(&rt, "sidebar-footer-settings");
    let board = rect(&rt, "sidebar-footer-board");
    assert_eq!(settings[2], board[2], "equal cells, whatever their words");
    assert!(board[0] + board[2] <= SIDEBAR_W);
}

// --- the roster strip at twelve teammates ----------------------------------------------------------------------------

#[test]
fn caps_the_faces_rather_than_letting_the_strip_run_past_the_column() {
    // case: caps the faces rather than letting the strip run past the column
    // Sidebar.tsx: FACES_WITHOUT_CHIP = 5, FACES_WITH_CHIP = 4.
    for (n, drawn) in [(1, 1), (5, 5), (6, 4), (12, 4)] {
        assert_eq!(shown_face_count(n), drawn, "{n} teammates");
        let rt = draw(state(team(n), vec![]));
        assert_eq!(strip_faces(&rt).len(), drawn, "{n} teammates draw {drawn} faces");
        assert_eq!(layout(&rt, "sidebar-faces-more").is_some(), n > 5, "{n} teammates: the +N chip");
    }
}

#[test]
fn counts_what_it_does_not_draw_instead_of_hiding_it() {
    // case: counts what it does not draw instead of hiding it
    let rt = draw(state(team(12), vec![]));
    assert!(texts(&rt).iter().any(|t| t == "+8"), "{:?}", texts(&rt));
    let more = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("sidebar-faces-more")).unwrap();
    assert_eq!(more.label.as_deref(), Some("8 more on the team — open the roster"));
    let mut rt = rt;
    press(&mut rt, "sidebar-faces-more");
    assert!(rt.state.people_panel.open);
}

#[test]
fn never_scrolls_sideways() {
    // case: never scrolls sideways
    // Fifty-nine teammates make the widest count Electron worried about, `+55`; everything still ends in the column.
    let rt = draw(state(team(59), vec![]));
    assert!(texts(&rt).iter().any(|t| t == "+55"));
    let width = crate::window_glass::sidebar_width(&rt.state);
    let mut keys = strip_faces(&rt).into_iter().map(|id| format!("teammate-{id}")).collect::<Vec<_>>();
    keys.extend(["sidebar-faces-more".to_owned(), "sidebar-faces-team".to_owned()]);
    for key in keys {
        let r = rect(&rt, &key);
        assert!(r[0] >= 0.0 && r[0] + r[2] <= width, "{key} {r:?} ends inside the {width} column");
    }
}

#[test]
fn shows_the_ones_being_worked_with_not_the_ones_made_first() {
    // case: shows the ones being worked with, not the ones made first
    // T5 has the newest conversation, T3 the next; the rest have none and keep the roster's order.
    let s = state(team(7), vec![conv("m_a", "Newest", "T5", 0), conv("m_b", "Older", "T3", 30), conv("m_c", "Oldest", "T3", 90)]);
    let rt = draw(s);
    assert_eq!(strip_faces(&rt), ["tm_5", "tm_3", "tm_0", "tm_1"]);
    // A row whose time is not a time sorts as no work (`Number.isNaN(at) ? held`).
    let team = team(3);
    let owned = vec![(Some("tm_2".to_owned()), Some("not a time".to_owned())), (Some("tm_1".to_owned()), Some("2026-11-20T12:00:00Z".to_owned()))];
    let order: Vec<&str> = faces_by_recency(&team, &owned).iter().map(|m| m.id.as_str()).collect();
    assert_eq!(order, ["tm_1", "tm_0", "tm_2"]);
}

#[test]
fn leaves_the_rail_listing_everybody_which_is_a_different_shape() {
    // case: leaves the rail listing everybody, which is a different shape
    let rt = window(state(team(12), vec![]), "compact");
    assert_eq!(keys_starting(&rt, "rail-tm_").len(), 12);
    assert!(!texts(&rt).iter().any(|t| t == "+8"), "the rail carries no count");
}

// --- through the window: a face's card filters the list --------------------------------------------------------------

#[test]
fn a_faces_card_filters_the_list_to_that_teammate_and_clear_puts_everyone_back() {
    let s = state(
        vec![mate("tm_wren", "Wren"), mate("tm_sable", "Sable")],
        vec![conv("m_w1", "Fix the parser", "Wren", 0), conv("m_s1", "Parser tests", "Sable", 1)],
    );
    let mut rt = window(s, "wide");
    let (x, y) = rt.find("teammate-tm_wren").expect("Wren's face in the strip");
    rt.pointer_move(x, y);
    rt.frame(0.05);
    rt.frame(0.05);
    // The card opens beside the face (main.rs's wide-sidebar flyout line), with the list filter in it.
    let face = rect(&rt, "teammate-tm_wren");
    let filter = rect(&rt, "rail-filter");
    assert!(filter[0] >= crate::window_glass::sidebar_width(&rt.state), "the card sits beside the sidebar: {filter:?}");
    assert!(filter[1] + filter[3] > face[1] - 6.0, "the card's top is at the face's: {filter:?} {face:?}");
    let filter_words = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("rail-filter")).and_then(|e| e.label);
    assert_eq!(filter_words.as_deref(), Some("Only Wren’s in the list"));
    let (fx, fy) = rt.find("rail-filter").unwrap();
    rt.pointer_down(fx, fy);
    rt.pointer_up(fx, fy);
    rt.frame(0.05);
    rt.frame(0.05);
    assert_eq!(rt.state.sidebar_face_filter.as_deref(), Some("tm_wren"));
    assert_eq!(keys_starting(&rt, "recent-m_"), ["recent-m_w1"]);
    assert_eq!(said(&rt).as_deref(), Some("Wren — 1 of 1"));
    press(&mut rt, "sidebar-faces-clearlink");
    rt.frame(0.05);
    assert_eq!(keys_starting(&rt, "recent-m_"), ["recent-m_w1", "recent-m_s1"]);
}

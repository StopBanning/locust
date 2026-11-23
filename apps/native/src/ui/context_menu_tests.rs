//! a-context-menu-stays-in-the-window.test.tsx, "what a menu draws", drawn headless; and the menu opened by a real
//! right-click on a sidebar row, placed by `place_menu` in the window it is in.
use super::*;
use rune::{Env, Key, Runtime};

const VIEW: (f32, f32) = (1477.0, 920.0);

/// The Electron test's five rows, at (10, 10), titled for what was clicked.
fn electron_items() -> Vec<Item> {
    vec![
        Item::new("Open", "open").key('o'),
        Item::new("Rename", "rename").key('r'),
        Item::new("Move to group", "").divided().list(vec![Item::new("Work", "group:g_work").checked(true)]),
        Item::new("Assign to", "")
            .refused(Some("Wait for the run to finish before handing it over."))
            .list(vec![Item::new("Yurt", "assign:yurt")]),
        Item::new("Delete", "delete").key('d').divided().danger("Delete for good?"),
    ]
}

fn drawn() -> Runtime<State> {
    let mut s = State::default();
    s.row_menu = Some(MenuState {
        x: 10.0,
        y: 10.0,
        title: "Say hello to my friend Kevin".into(),
        target: "m_kevin".into(),
        anchor: None,
        items: electron_items(),
        armed: None,
        open_sub: None,
    });
    let mut rt = Runtime::new(
        s,
        |s, cx| {
            let measured = cx.layout("row-menu").map(|r| (r[2], r[3]));
            col().w_full().h_full().children(view(s, &Palette::new(true), VIEW, measured))
        },
        VIEW,
        Env::new(true, 1.0),
    );
    for _ in 0..3 {
        rt.frame(0.016);
    }
    rt
}

fn element(rt: &Runtime<State>, key: &str) -> rune::ElementInfo {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no element {key}"))
}

fn has(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

fn press(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
}

fn type_key(rt: &mut Runtime<State>, c: &str) {
    rt.key(Key::Text(c.into()).into());
    rt.frame(0.016);
}

#[test]
fn has_no_title_bar_what_was_clicked_is_the_menus_name() {
    let rt = drawn();
    let menu = element(&rt, "row-menu");
    assert_eq!(menu.role, "menu");
    assert_eq!(menu.label.as_deref(), Some("Say hello to my friend Kevin"));
    assert!(
        !rt.elements().iter().any(|e| e.text.as_deref() == Some("Say hello to my friend Kevin")),
        "the title is the menu's name, never drawn as a bar"
    );
}

#[test]
fn shows_each_rows_key_at_its_end() {
    let rt = drawn();
    for (row, key) in [("open", "O"), ("delete", "D")] {
        let shown = element(&rt, &format!("row-menu-item-{row}-key"));
        assert_eq!(shown.text.as_deref(), Some(key));
        let label = element(&rt, &format!("row-menu-item-{row}-label"));
        let item = element(&rt, &format!("row-menu-item-{row}"));
        assert!(shown.rect[0] > label.rect[0] + label.rect[2] - 0.5, "{row}: the key sits after the label");
        assert!(shown.rect[0] + shown.rect[2] > item.rect[0] + item.rect[2] - 16.0, "{row}: the key sits at the row's end");
    }
}

#[test]
fn draws_a_hairline_between_kinds_of_action_and_red_for_the_one_that_deletes() {
    let rt = drawn();
    let separators = rt.elements().into_iter().filter(|e| e.role == "separator" && !e.exiting).count();
    assert_eq!(separators, 2);
    let l = crate::look::controls::Look::new(true);
    assert_eq!(element(&rt, "row-menu-item-delete-label").text_color, Some(l.c("red-text")));
    assert_ne!(element(&rt, "row-menu-item-open-label").text_color, Some(l.c("red-text")));
}

#[test]
fn puts_a_list_behind_one_row_and_says_why_when_it_cannot_open() {
    let rt = drawn();
    let moving = element(&rt, "row-menu-item-move-to-group");
    assert_eq!(moving.expanded, Some(false), "a row with a list behind it says it can open");
    assert_eq!(element(&rt, "row-menu-item-move-to-group-more").text.as_deref(), Some("›"));
    // A list that cannot be used now is a greyed row with its reason, not an arrow into a list that would refuse.
    let assign = element(&rt, "row-menu-item-assign-to");
    assert!(!assign.focusable, "refused, it cannot be chosen");
    assert_eq!(assign.expanded, None);
    assert!(!has(&rt, "row-menu-item-assign-to-more"));
    assert_eq!(
        element(&rt, "row-menu-item-assign-to-why").text.as_deref(),
        Some("Wait for the run to finish before handing it over.")
    );
}

#[test]
fn a_letter_chooses_its_row_and_delete_asks_in_place_before_it_acts() {
    let mut rt = drawn();
    press(&mut rt, "row-menu-item-rename-label");
    // Rename ran (against no data folder it says so) and the menu closed.
    assert!(rt.state.row_menu.is_none());

    let mut rt = drawn();
    type_key(&mut rt, "d");
    assert_eq!(rt.state.row_menu.as_ref().and_then(|m| m.armed.as_deref()), Some("Delete"));
    assert_eq!(element(&rt, "row-menu-item-delete-label").text.as_deref(), Some("Delete for good?"));
    type_key(&mut rt, "D");
    assert!(rt.state.row_menu.is_none(), "the second press acts and closes the menu");
    assert_eq!(rt.state.status, "No native data folder for conversation organization.");
}

#[test]
fn a_list_opens_from_its_row_and_its_entry_is_chosen_there() {
    let mut rt = drawn();
    press(&mut rt, "row-menu-item-move-to-group");
    assert_eq!(rt.state.row_menu.as_ref().and_then(|m| m.open_sub.as_deref()), Some("Move to group"));
    let sub = element(&rt, "row-menu-sub-move-to-group");
    let menu = element(&rt, "row-menu");
    assert!(sub.rect[0] >= menu.rect[0] + menu.rect[2] - 2.0, "the list opens to the right of its row");
    assert!(has(&rt, "row-menu-tick-work"), "the tick says which one it is already in");
    press(&mut rt, "row-menu-sub-item-work");
    assert!(rt.state.row_menu.is_none());
}

#[test]
fn escape_or_a_press_anywhere_else_closes_it() {
    let mut rt = drawn();
    rt.key(Key::Escape.into());
    rt.frame(0.016);
    assert!(rt.state.row_menu.is_none());

    let mut rt = drawn();
    rt.pointer_down(900.0, 700.0);
    rt.pointer_up(900.0, 700.0);
    rt.frame(0.016);
    assert!(rt.state.row_menu.is_none());
}

/// The window, wide enough for the full sidebar (1199 points or less draws the rail), with one conversation in it.
fn window(size: (f32, f32)) -> Runtime<State> {
    let mut s = State::default();
    s.demo = true;
    s.teammates.push(Teammate { id: "tm_wren".into(), name: "Wren".into(), role: "Custom".into(), runtime: None, model: None, account: None, busy: false, runs_in: "ask".into(), folder: None });
    s.recent = vec![ConversationRow {
        id: "m_kevin".into(),
        teammate: "Wren".into(),
        title: "Say hello to my friend Kevin".into(),
        started_by: "native".into(),
        state: "completed".into(),
        updated: "2026-11-18T10:00:00Z".into(),
    }];
    let mut rt = Runtime::new(s, crate::view, size, Env::new(true, 1.0));
    for _ in 0..10 {
        rt.frame(0.05);
    }
    rt
}

fn right_click(rt: &mut Runtime<State>, x: f32, y: f32) {
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    for _ in 0..10 {
        rt.frame(0.05);
    }
}

#[test]
fn a_right_click_on_a_conversation_opens_its_menu_at_the_pointer() {
    let mut rt = window((1400.0, 780.0));
    let (x, y) = rt.find("recent-m_kevin").expect("the conversation's row");
    right_click(&mut rt, x, y);
    let menu = rt.state.row_menu.as_ref().expect("the menu opened");
    assert_eq!(menu.title, "Say hello to my friend Kevin");
    assert_eq!(menu.target, "m_kevin");
    let ([left, top, _, _], _) = rt.presented("row-menu").expect("the menu is drawn");
    assert_eq!((left, top), (x, y), "it fits there, so it opens at the pointer");
    assert!(has(&rt, "row-menu-item-open-key"));
}

#[test]
fn a_menu_opened_low_in_a_short_window_stays_inside_it() {
    let mut rt = window((1400.0, 440.0));
    let row = element(&rt, "recent-m_kevin");
    let (x, y) = (row.visible[0] + 20.0, row.visible[1] + 4.0);
    right_click(&mut rt, x, y);
    let ([left, top, w, h], _) = rt.presented("row-menu").expect("the menu is drawn");
    let ([bx, by, bw, bh], _) = rt.presented("app-body").expect("the window body");
    let win = (bx + bw, by + bh);
    assert_eq!((left, top), place_menu((x, y), (w, h), win));
    assert!(top + h <= win.1 - 8.0 + 0.5, "all of it inside the window: {top} + {h} in {}", win.1);
    assert!(top >= 8.0 - 0.5);
}

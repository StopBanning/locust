//! Carries `apps/desktop/src/renderer/src/a-pointed-part-sits-in-the-chat-box.test.tsx`, drawn headless: the chat box
//! holds a pointed part until Send, the button says so when the page will not run the picker, and Tab reaches the note
//! and the remove button. Then the window's own half: a press asks the open tab to point, the answer becomes a chip
//! with its picture kept, and Send carries the parts after the person's words and empties the box.
use super::*;
use locust_core::browser_point::{BrowserPoint, POINT_BLOCKED, PAGE_POINT_MARK, read_browser_point};
use serde_json::{Value, json};
use std::path::PathBuf;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp() -> Temp {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!("locust-page-points-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    Temp(root)
}

const URL: &str = "http://127.0.0.1:9786/";

/// A part, read the only way one is read: from what the picker answered.
fn read(raw: Value) -> BrowserPoint {
    read_browser_point(&raw, URL).expect("a pick").point
}

fn save_button() -> BrowserPoint {
    read(json!({
        "selector": "button#save", "html": "<button id=\"save\">Save</button>", "htmlLength": 31,
        "rect": {"x": 4, "y": 8, "width": 180, "height": 40}, "viewport": {"width": 800, "height": 600},
        "styles": {"width": "180px", "color": "rgb(11, 22, 33)"},
        "source": {"file": "src/SaveButton.tsx", "line": 18}
    }))
}

fn password() -> BrowserPoint {
    read(json!({
        "selector": "input#pw", "html": "<input id=\"pw\" type=\"password\" value=\"s3cret-pass\">", "htmlLength": 52,
        "rect": {"x": 4, "y": 8, "width": 200, "height": 30}, "viewport": {"width": 800, "height": 600},
        "styles": {"width": "200px"}, "sensitive": "password"
    }))
}

/// The two chips of Electron's test: the Save button with a note and its picture, and a password field.
fn with_chips(s: &mut State) {
    s.page_points.chips = vec![
        PagePointChip { id: "save".into(), note: String::new(), attachment: Some("page-part-1.png".into()), point: save_button() },
        PagePointChip { id: "pw".into(), note: String::new(), attachment: None, point: password() },
    ];
    let mut note = TextEdit::default();
    note.replace_text("the spacing is tight".to_owned());
    s.page_points.notes.insert("save".into(), note);
    s.page_points.notes.insert("pw".into(), TextEdit::default());
}

fn box_drawn(s: State) -> rune::Runtime<State> {
    let mut rt = rune::Runtime::new(s, |s, _| page_points_ui::chips(s, &Palette::new(true)).unwrap_or_else(col), (700.0, 900.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn texts(rt: &rune::Runtime<State>) -> String {
    rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

fn node<'a>(tree: &'a Value, key: &str) -> Option<&'a Value> {
    if tree["key"] == key {
        return Some(tree);
    }
    tree["children"].as_array()?.iter().find_map(|child| node(child, key))
}

/// The browser panel open on `url`, its point button as assistive technology reads it.
fn panel(s: State) -> rune::Runtime<State> {
    let mut rt = rune::Runtime::new(s, |s, _| browser_panel::view(s, &Palette::new(true)).unwrap_or_else(col), (600.0, 900.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn open_on(s: &mut State, url: &str) -> u64 {
    let mut host = crate::invariants::lock(&s.browser.host);
    let id = host.add(crate::browser::Profile(None)).unwrap();
    host.tabs[0].url = url.into();
    id
}

// case: a blocked page says it does not allow pointing
#[test]
fn a_blocked_page_says_it_does_not_allow_pointing() {
    let mut s = State::default();
    open_on(&mut s, URL);
    s.page_points.blocked = Some(URL.into());
    let rt = panel(s);
    let tree = rt.accessibility_json();
    let button = node(&tree, "browser-point").unwrap_or_else(|| panic!("no point button: {tree}"));
    assert_eq!(button["name"], POINT_BLOCKED, "{button}");
    assert!(texts(&rt).contains(POINT_BLOCKED));
    assert!(button["states"].as_array().is_some_and(|all| all.iter().any(|s| s == "disabled")), "{button}");
    assert_eq!(button["shortcut"], "Control+Shift+C", "{button}");
    // The control: the same page, not blocked, can be pointed at.
    let mut s = State::default();
    open_on(&mut s, URL);
    let tree = panel(s).accessibility_json();
    let button = node(&tree, "browser-point").unwrap();
    assert_eq!(button["name"], "Point at a part of the page");
    assert!(!button["states"].as_array().is_some_and(|all| all.iter().any(|s| s == "disabled")), "{button}");
}

#[test]
fn a_pointed_part_is_a_chip_with_a_note_its_styles_and_a_way_to_remove_it() {
    let mut s = State::default();
    with_chips(&mut s);
    let rt = box_drawn(s);
    let said = texts(&rt);
    assert!(said.contains("button#save"), "{said}");
    assert!(said.contains("src/SaveButton.tsx:18"), "{said}");
    assert!(said.contains("width: 180px"), "{said}");
    assert!(said.contains("the spacing is tight"), "the note is in its field: {said}");
    let all = rt.elements();
    let by_label = |label: &str| all.iter().find(|e| e.label.as_deref() == Some(label)).unwrap_or_else(|| panic!("no {label}"));
    assert!(by_label("Picture of button#save").key.as_deref() == Some("save/picture"));
    // Tab reaches the note and the remove button.
    assert!(by_label("Note on button#save").focusable);
    assert!(by_label("Remove button#save").focusable);
}

// case: a password field's chip has no picture of its contents
#[test]
fn a_password_fields_chip_has_no_picture_of_its_contents() {
    let mut s = State::default();
    with_chips(&mut s);
    let rt = box_drawn(s);
    let said = texts(&rt);
    assert!(said.contains("No picture: a password field is not photographed with its contents."), "{said}");
    let keys: Vec<String> = rt.elements().into_iter().filter_map(|e| e.key).collect();
    assert!(!keys.iter().any(|k| k == "pw/picture"), "{keys:?}");
    assert!(keys.iter().any(|k| k == "save/picture"), "the control: {keys:?}");
    let tree = rt.accessibility_json().to_string();
    assert!(!said.contains("s3cret-pass") && !tree.contains("s3cret-pass"));
}

fn store_in(t: &Temp) -> crate::store::Store {
    crate::store::Store::new(t.0.join("native"), t.0.join("electron")).unwrap()
}

#[test]
fn points_at_the_open_page_when_pressed_and_puts_the_part_in_the_box_with_its_picture() {
    let t = temp();
    let mut s = State::default();
    s.data = Some(store_in(&t));
    open_on(&mut s, URL);
    let mut rt = panel(s);
    let (x, y) = rt.find("browser-point").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    let reply = {
        let mut host = crate::invariants::lock(&rt.state.browser.host);
        match host.tabs[0].commands.pop_front() {
            Some(crate::browser::Command::Point { reply }) => reply,
            _ => panic!("the open tab was not asked to point"),
        }
    };
    assert!(page_points_ui::control(&rt.state).pressed, "pressed while it points");
    reply.send(PointAnswer::Picked { point: save_button(), png: Some(vec![137, 80, 78, 71]) }).unwrap();
    assert!(page_points_ui::poll(&mut rt.state));
    let chips = &rt.state.page_points.chips;
    assert_eq!(chips.len(), 1);
    let picture = chips[0].attachment.clone().expect("its picture is kept");
    assert!(picture.starts_with(&t.0.join("native").to_string_lossy().into_owned()) && picture.contains("page-parts"), "{picture}");
    assert_eq!(std::fs::read(&picture).unwrap(), [137, 80, 78, 71]);
    assert!(!page_points_ui::control(&rt.state).pressed);
}

#[test]
fn remembers_a_page_that_will_not_run_the_picker_and_says_so_on_its_button() {
    let mut s = State::default();
    open_on(&mut s, URL);
    page_points_ui::take_answer(&mut s, PointAnswer::Refused { message: POINT_BLOCKED.into(), blocked: true });
    assert_eq!(s.status, POINT_BLOCKED);
    let c = page_points_ui::control(&s);
    assert!(c.disabled && c.label == POINT_BLOCKED);
}

#[test]
fn removing_a_part_takes_its_picture_off_the_message_and_the_disk() {
    let t = temp();
    let picture = t.0.join("part.png");
    std::fs::write(&picture, [1u8]).unwrap();
    let mut s = State::default();
    with_chips(&mut s);
    s.page_points.chips[0].attachment = Some(picture.to_string_lossy().into_owned());
    let mut rt = box_drawn(s);
    let (x, y) = rt.find("save/remove").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
    assert_eq!(rt.state.page_points.chips.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["pw"]);
    assert!(!picture.exists());
}

#[test]
fn sends_each_part_after_the_words_with_its_note_and_picture_and_empties_the_box() {
    let mut s = State::default();
    with_chips(&mut s);
    let message = page_points_ui::take_into(&mut s, "Fix these");
    assert!(message.starts_with("Fix these\n\n"), "{message}");
    assert_eq!(message.matches(PAGE_POINT_MARK).count(), 2, "{message}");
    assert!(message.contains("Part: button#save") && message.contains("Note: the spacing is tight"), "{message}");
    assert!(message.contains("No picture: a password field is not photographed with its contents."));
    assert!(!message.contains("s3cret-pass"), "{message}");
    assert_eq!(s.attached, ["page-part-1.png"]);
    assert!(s.page_points.chips.is_empty() && s.page_points.notes.is_empty());
    // Nothing waiting: the words go as they are.
    assert_eq!(page_points_ui::take_into(&mut s, "Just words"), "Just words");
}

#[test]
fn a_part_waits_with_a_message_queued_behind_a_running_turn_and_leaves_the_box_only_then() {
    let t = temp();
    let mut s = State::default();
    s.data = Some(store_in(&t));
    s.teammates.push(crate::Teammate { id: "tm".into(), name: "Codec".into(), role: "Custom".into(), model: None, runtime: Some("claude".into()), folder: Some("/tmp".into()), runs_in: "ask".into(), account: None, busy: true });
    s.conversations.insert("tm".into(), Conversation { conversation_id: Some("mission_running".into()), turns: Vec::new() });
    s.opening_teammate = Some("tm".into());
    with_chips(&mut s);
    s.draft.replace_text("And this".to_owned());
    let mut rt = rune::Runtime::new(s, |_, _| col(), (100.0, 100.0), rune::Env::new(true, 1.0));
    rt.update(|s, tx| crate::composer_wiring::submit_composer(s, tx, false));
    let row = rt.state.queue_rows.first().expect("the message is queued");
    assert!(row.text.starts_with("And this\n\n") && row.text.contains("Part: button#save"), "{}", row.text);
    assert_eq!(row.attachments, ["page-part-1.png"]);
    assert!(rt.state.page_points.chips.is_empty(), "the parts went with it");
    // A second message while one waits is not queued, and its parts stay in the box.
    rt.update(|s, _| with_chips(s));
    rt.update(|s, _| s.draft.replace_text("Another".to_owned()));
    rt.update(|s, tx| crate::composer_wiring::submit_composer(s, tx, false));
    assert_eq!(rt.state.queue_rows.len(), 1);
    assert_eq!(rt.state.page_points.chips.len(), 2);
}

#[path = "page_points_chip_sweep_tests.rs"]
mod page_points_chip_sweep_tests;

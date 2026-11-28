//! Carries `apps/desktop/src/renderer/src/a-pointed-part-sits-in-the-chat-box.test.tsx` (T767) and verifies
//! real screenshot thumbnail rendering in the chat box (R93), no leaked PNG on 8-chip refusal, and timeout cancellation.
use super::*;
use locust_core::browser_point::{
    BrowserPoint, EIGHT_IN_THE_BOX, MAX_PAGE_POINTS, POINT_BLOCKED, read_browser_point,
};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Arc;

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp(name: &str) -> Temp {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "locust-page-points-parity-{name}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("native")).unwrap();
    std::fs::create_dir_all(root.join("electron")).unwrap();
    Temp(root)
}

/// Valid 1x1 red PNG bytes (decoded by Rune's ImageCache).
const TINY_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 2, 0,
    0, 0, 144, 119, 83, 222, 0, 0, 0, 12, 73, 68, 65, 84, 8, 215, 99, 248, 207, 192, 0, 0, 3, 1, 1,
    0, 24, 221, 141, 176, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

const URL: &str = "http://127.0.0.1:9786/";

fn read(raw: Value) -> BrowserPoint {
    read_browser_point(&raw, URL).expect("a pick").point
}

fn save_button() -> BrowserPoint {
    read(json!({
        "selector": "button#save",
        "html": "<button id=\"save\">Save</button>",
        "htmlLength": 31,
        "rect": {"x": 4, "y": 8, "width": 180, "height": 40},
        "viewport": {"width": 800, "height": 600},
        "styles": {"width": "180px", "color": "rgb(11, 22, 33)"},
        "source": {"file": "src/SaveButton.tsx", "line": 18}
    }))
}

fn password_field() -> BrowserPoint {
    read(json!({
        "selector": "input#pw",
        "html": "<input id=\"pw\" type=\"password\" value=\"s3cret-pass\">",
        "htmlLength": 52,
        "rect": {"x": 4, "y": 8, "width": 200, "height": 30},
        "viewport": {"width": 800, "height": 600},
        "styles": {"width": "200px"},
        "sensitive": "password"
    }))
}

fn node<'a>(tree: &'a Value, key: &str) -> Option<&'a Value> {
    if tree["key"] == key {
        return Some(tree);
    }
    tree["children"].as_array()?.iter().find_map(|child| node(child, key))
}

fn texts(rt: &rune::Runtime<State>) -> String {
    rt.elements().iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

// case: a blocked page says it does not allow pointing
#[test]
fn parity_a_blocked_page_says_it_does_not_allow_pointing() {
    let mut s = State::default();
    {
        let mut host = crate::invariants::lock(&s.browser.host);
        host.add(crate::browser::Profile(None)).unwrap();
        host.tabs[0].url = URL.into();
    }
    s.page_points.blocked = Some(URL.into());
    let mut rt = rune::Runtime::new(
        s,
        |s, _| browser_panel::view(s, &Palette::new(true)).unwrap_or_else(col),
        (600.0, 900.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let tree = rt.accessibility_json();
    let button = node(&tree, "browser-point").expect("point button");
    assert_eq!(button["name"], POINT_BLOCKED);
    assert!(button["states"].as_array().is_some_and(|all| all.iter().any(|st| st == "disabled")));
    assert_eq!(button["shortcut"], "Control+Shift+C");
}

// case: a pointed part is a chip with a note, its styles, and a way to remove it
#[test]
fn parity_a_pointed_part_is_a_chip_with_a_note_its_styles_and_a_way_to_remove_it() {
    let t = temp("chip-thumb");
    let shot_path = t.0.join("page-part-1.png");
    std::fs::write(&shot_path, TINY_PNG).unwrap();
    let mut s = State::default();
    s.image_cache = Some(Arc::new(rune::ImageCache::new(Arc::new(|| {}))));
    s.page_points.chips = vec![PagePointChip {
        id: "save".into(),
        note: String::new(),
        attachment: Some(shot_path.to_string_lossy().into_owned()),
        point: save_button(),
    }];
    let mut note = TextEdit::default();
    note.replace_text("the spacing is tight".to_owned());
    s.page_points.notes.insert("save".into(), note);

    let mut rt = rune::Runtime::new(
        s,
        |s, _| page_points_ui::chips(s, &Palette::new(true)).unwrap_or_else(col),
        (700.0, 900.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let said = texts(&rt);
    assert!(said.contains("button#save"), "{said}");
    assert!(said.contains("src/SaveButton.tsx:18"), "{said}");
    assert!(said.contains("width: 180px"), "{said}");
    assert!(said.contains("the spacing is tight"), "{said}");
    // Draws the real screenshot thumbnail rather than the "Picture" text placeholder.
    assert!(!said.lines().any(|l| l == "Picture"), "decoded PNG must render pixels, not placeholder text: {said}");
    let all = rt.elements();
    let picture = all.iter().find(|e| e.key.as_deref() == Some("save/picture")).expect("thumbnail element");
    assert_eq!(picture.label.as_deref(), Some("Picture of button#save"));
    assert!(picture.rect[2] > 0.0 && picture.rect[3] > 0.0, "thumbnail rect: {:?}", picture.rect);
    let by_label = |label: &str| all.iter().find(|e| e.label.as_deref() == Some(label)).unwrap_or_else(|| panic!("no {label}"));
    assert!(by_label("Note on button#save").focusable);
    assert!(by_label("Remove button#save").focusable);
}

// case: a password field's chip has no picture of its contents
#[test]
fn parity_a_password_fields_chip_has_no_picture_of_its_contents() {
    let mut s = State::default();
    s.image_cache = Some(Arc::new(rune::ImageCache::new(Arc::new(|| {}))));
    s.page_points.chips = vec![PagePointChip {
        id: "pw".into(),
        note: String::new(),
        attachment: None,
        point: password_field(),
    }];
    let mut rt = rune::Runtime::new(
        s,
        |s, _| page_points_ui::chips(s, &Palette::new(true)).unwrap_or_else(col),
        (700.0, 900.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let said = texts(&rt);
    assert!(said.contains("No picture: a password field is not photographed with its contents."), "{said}");
    assert!(!rt.elements().iter().any(|e| e.key.as_deref() == Some("pw/picture")));
    assert!(!said.contains("s3cret-pass"));
}

#[test]
fn ninth_pointed_part_is_refused_without_writing_an_orphaned_picture_to_disk() {
    let t = temp("nine");
    let store = crate::store::Store::new(t.0.join("native"), t.0.join("electron")).unwrap();
    let mut s = State::default();
    s.data = Some(store.clone());
    for i in 0..MAX_PAGE_POINTS {
        s.page_points.chips.push(PagePointChip {
            id: format!("p-{i}"),
            note: String::new(),
            attachment: None,
            point: save_button(),
        });
    }
    take_answer(&mut s, PointAnswer::Picked { point: save_button(), png: Some(TINY_PNG.to_vec()) });
    assert_eq!(s.page_points.chips.len(), MAX_PAGE_POINTS);
    assert_eq!(s.status, EIGHT_IN_THE_BOX);
    let parts_dir = store.root().join(PAGE_PARTS_FOLDER);
    let written = std::fs::read_dir(&parts_dir).map(|d| d.count()).unwrap_or(0);
    assert_eq!(written, 0, "no orphaned PNG should be written when 8 parts are already in the box");
}

// case: reads a pick, and lets go of one that never comes
#[test]
fn reads_a_pick_and_lets_go_of_one_that_never_comes() {
    use locust_core::browser_point::point_answer;
    // Reading a valid pick yields Picked; a null pick yields Nothing.
    let answer = json!({
        "selector": "main > h1",
        "html": "<h1>Hi</h1>",
        "htmlLength": 11,
        "rect": {"x": 1, "y": 2, "width": 30, "height": 10},
        "viewport": {"width": 800, "height": 600}
    });
    let picked = point_answer(Some(&json!(true)), Ok(Some(&answer)), false, "locust-page://aaa/x.html", |_| None);
    assert!(matches!(picked, PointAnswer::Picked { ref point, .. } if point.selector() == "main > h1" && point.html() == "<h1>Hi</h1>"));
    assert_eq!(
        point_answer(Some(&json!(true)), Ok(Some(&Value::Null)), false, "locust-page://aaa/x.html", |_| None),
        PointAnswer::Nothing
    );
    // A pick that never arrives times out past PICK_WAIT, clears waiting, and queues CancelPoint.
    let mut s = State::default();
    let tab_id = {
        let mut host = crate::invariants::lock(&s.browser.host);
        let id = host.add(crate::browser::Profile(None)).unwrap();
        host.tabs[0].url = "locust-page://aaa/x.html".into();
        id
    };
    let (_tx, rx) = std::sync::mpsc::sync_channel::<PointAnswer>(1);
    let expired = Instant::now().checked_sub(PICK_WAIT + Duration::from_secs(1)).unwrap();
    s.page_points.waiting = Some((rx, expired, tab_id));
    assert!(poll(&mut s), "expired pick should be cancelled on poll");
    assert!(s.page_points.waiting.is_none());
    let host = crate::invariants::lock(&s.browser.host);
    assert!(
        host.tabs[0].commands.iter().any(|c| matches!(c, crate::browser::Command::CancelPoint)),
        "expected CancelPoint to be sent to the tab"
    );
}

#[test]
fn browser_policy_false_refuses_pointing_in_ui_toggle() {
    let mut s = State::default();
    {
        let mut host = crate::invariants::lock(&s.browser.host);
        host.add(crate::browser::Profile(None)).unwrap();
        host.tabs[0].url = URL.into();
        host.refusal = crate::browser::policy_refusal(
            &json!({"browser": false}),
            std::path::Path::new("/etc/locust/policy.json"),
        );
    }
    toggle(&mut s);
    assert!(s.page_points.waiting.is_none());
    assert!(s.status.contains("The browser panel is off."), "{}", s.status);
    assert!(crate::invariants::lock(&s.browser.host).tabs[0].commands.is_empty());
}

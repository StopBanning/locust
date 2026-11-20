//! the-header-is-a-row-of-icons.test.tsx (4 cases). The Browser and Terminal toggles are icons named for a screen
//! reader and for hover, with no words drawn; open, each reads as on (pressed); turned off by the organization, each is
//! disabled and says so in its name and its hover. Read from Rune's accessibility tree, as a reader hears them (a toggle button's
//! `aria-pressed` is AccessKit's toggled state, dumped as "checked"/"unchecked").
use super::*;
use serde_json::Value;

fn node<'a>(tree: &'a Value, key: &str) -> Option<&'a Value> {
    if tree["key"] == key {
        return Some(tree);
    }
    tree["children"].as_array()?.iter().find_map(|c| node(c, key))
}

fn states(n: &Value) -> Vec<String> {
    n["states"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

fn drawn(open: bool) -> rune::Runtime<State> {
    let mut s = State::default();
    s.terminal.open = open;
    crate::invariants::lock(&s.browser.host).open = open;
    let mut rt = rune::Runtime::new(
        s,
        |s, _| row().child(crate::browser_panel::toggle(s, &Palette::new(true))).child(crate::terminal_panel::toggle(s, &Palette::new(true))),
        (400.0, 100.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

fn words(rt: &rune::Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.text).filter(|t| !t.trim().is_empty()).collect()
}

fn off(settings: &[&str]) {
    let policy = settings.iter().map(|k| (k.to_string(), serde_json::json!(false))).collect();
    locust_core::org_policy::use_policy_on_this_thread(Some(locust_core::org_policy::PolicyState {
        path: "/test/policy.json".into(),
        present: true,
        policy,
        error: None,
    }));
}

struct Back;
impl Drop for Back {
    fn drop(&mut self) {
        locust_core::org_policy::use_policy_on_this_thread(None);
    }
}

// case: the browser is an icon, named for hover and screen readers
#[test]
fn the_browser_is_an_icon_named_for_hover_and_screen_readers() {
    let _back = Back;
    off(&[]);
    let rt = drawn(false);
    let e = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("browser-toggle")).expect("the browser toggle");
    assert_eq!(e.role, "button");
    assert_eq!(e.label.as_deref(), Some("Browser"));
    assert_eq!(e.help.as_deref(), Some("Browser"));
    assert!(words(&rt).is_empty(), "no words are drawn: {:?}", words(&rt));
    let tree = rt.accessibility_json();
    assert!(states(node(&tree, "browser-toggle").unwrap()).iter().any(|s| s == "unchecked"), "closed, it reads as off");
}

// case: open, it reads as on
#[test]
fn open_it_reads_as_on() {
    let _back = Back;
    off(&[]);
    let tree = drawn(true).accessibility_json();
    let browser = states(node(&tree, "browser-toggle").expect("in the tree"));
    let terminal = states(node(&tree, "terminal-toggle").expect("in the tree"));
    assert!(browser.iter().any(|s| s == "checked"), "{browser:?}");
    assert!(terminal.iter().any(|s| s == "checked"), "{terminal:?}");
}

// case: the terminal is an icon too, with its shortcut on hover
#[test]
fn the_terminal_is_an_icon_with_its_shortcut_on_hover() {
    let _back = Back;
    off(&[]);
    let rt = drawn(false);
    let e = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("terminal-toggle")).expect("the terminal toggle");
    assert_eq!(e.label.as_deref(), Some("Terminal"));
    assert_eq!(e.help.as_deref(), Some("Terminal (Ctrl+`)"));
    assert!(words(&rt).is_empty());
}

// case: turned off by the organization, each says so where the word used to be
#[test]
fn turned_off_by_the_organization_each_says_so() {
    let _back = Back;
    off(&["browser", "terminal"]);
    let rt = drawn(true);
    let find = |key: &str| rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap();
    let browser = find("browser-toggle");
    assert_eq!(browser.label.as_deref(), Some("Browser, set by your organization"));
    assert!(browser.help.as_deref().is_some_and(|h| h.starts_with("The browser panel is off.")), "{:?}", browser.help);
    let terminal = find("terminal-toggle");
    assert_eq!(terminal.label.as_deref(), Some("Terminal, set by your organization"));
    assert!(terminal.help.as_deref().is_some_and(|h| h.starts_with("The terminal panel is off.")), "{:?}", terminal.help);
    let tree = rt.accessibility_json();
    for key in ["browser-toggle", "terminal-toggle"] {
        let st = states(node(&tree, key).unwrap());
        assert!(st.iter().any(|s| s == "disabled"), "{key}: {st:?}");
        assert!(!st.iter().any(|s| s == "checked"), "a refused panel never reads as on: {key}: {st:?}");
    }
}

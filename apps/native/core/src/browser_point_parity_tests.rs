//! Carries all 10 cases of `apps/desktop/src/main/a-part-of-the-browser-can-be-pointed-at.test.ts` (T160).
//! Expected values come from Electron's `shared/browser-point.ts`, `main/browser-point.ts`, and test cases.
use super::*;
use serde_json::json;
use std::cell::Cell;
use std::process::Command;

const URL: &str = "http://127.0.0.1:9786/";

fn picked(selector: &str, html: &str) -> Value {
    json!({
        "selector": selector,
        "html": html,
        "htmlLength": html.chars().count(),
        "rect": {"x": 4, "y": 8, "width": 180, "height": 40},
        "viewport": {"width": 800, "height": 600},
        "styles": {"width": "180px", "height": "40px", "display": "flex", "color": "rgb(11, 22, 33)"}
    })
}

// case: the picker script parses
#[test]
fn parity_the_picker_script_parses() {
    for script in BROWSER_POINT_SCRIPTS {
        let probe = format!(
            "new (require('vm').Script)({}); process.stdout.write('ok')",
            serde_json::to_string(script).unwrap()
        );
        let out = match Command::new("node").arg("-e").arg(&probe).output() {
            Ok(out) => out,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(e) => panic!("node failed: {e}"),
        };
        assert!(out.status.success() && out.stdout == b"ok", "{}", String::from_utf8_lossy(&out.stderr));
    }
}

// case: the picker request goes through main only
#[test]
fn parity_the_picker_request_goes_through_main_only() {
    let panel = include_str!("../../src/ui/browser_panel.rs");
    let points_ui = include_str!("../../src/ui/page_points_ui.rs");
    let mac = include_str!("../../src/browser_mac.rs");
    assert!(!panel.contains("evaluateJavaScript") && !panel.contains("callAsyncJavaScript"));
    assert!(!points_ui.contains("evaluateJavaScript") && !points_ui.contains("callAsyncJavaScript"));
    assert!(points_ui.contains("Command::Point { reply }"));
    assert!(mac.contains("Command::Point { reply } => point(&p.view, reply, mtm)"));
    let captures = Cell::new(0);
    let answer = point_answer(
        Some(&json!(true)),
        Ok(Some(&picked("button#save", "<button id=\"save\">Save</button>"))),
        false,
        URL,
        |_| {
            captures.set(captures.get() + 1);
            Some(vec![1, 2, 3, 4])
        },
    );
    assert_eq!(captures.get(), 1);
    let PointAnswer::Picked { png, .. } = answer else { panic!("{answer:?}") };
    assert_eq!(png, Some(vec![1, 2, 3, 4]));
}

// case: form values never appear in a pick
#[test]
fn parity_form_values_never_appear_in_a_pick() {
    let html = r#"<form><input type="text" value="typed-in"><input type="password" value="s3cret-pass"><textarea>typed words</textarea><select><option value="secret-opt" selected>Shown</option></select></form>"#;
    let read = read_browser_point(&picked("form", html), URL).expect("pick");
    let quoted = read.point.html();
    for secret in ["s3cret-pass", "typed words", "typed-in", "secret-opt", "selected"] {
        assert!(!quoted.contains(secret), "{secret} in {quoted}");
    }
    assert!(quoted.contains("Shown"));
}

// case: lists the styles that matter for a visual fix
#[test]
fn parity_lists_the_styles_that_matter_for_a_visual_fix() {
    let raw = json!({
        "width": "180px", "height": "40px",
        "margin-top": "0px", "margin-right": "0px", "margin-bottom": "0px", "margin-left": "0px",
        "padding-top": "8px", "padding-right": "16px", "padding-bottom": "8px", "padding-left": "16px",
        "display": "flex", "flex-direction": "row", "position": "static", "top": "10px",
        "color": "rgb(11, 22, 33)",
        "border-top-width": "1px", "border-right-width": "1px", "border-bottom-width": "1px", "border-left-width": "1px",
        "border-top-style": "solid", "border-top-color": "rgb(1, 2, 3)",
        "border-top-left-radius": "6px", "border-top-right-radius": "6px",
        "border-bottom-right-radius": "6px", "border-bottom-left-radius": "6px",
        "opacity": "1"
    });
    let styles = compact_styles(raw.as_object().unwrap());
    let map: std::collections::HashMap<_, _> = styles.iter().map(|s| (s.name.as_str(), s.value.as_str())).collect();
    assert_eq!(map["color"], "rgb(11, 22, 33)");
    assert_eq!(map["display"], "flex");
    assert_eq!(map["flex-direction"], "row");
    assert_eq!(map["padding"], "8px 16px");
    assert_eq!(map["margin"], "0px");
    assert!(!map.contains_key("top"));
    assert_eq!(map["border"], "1px solid rgb(1, 2, 3)");
    assert_eq!(map["border-radius"], "6px");
    assert_eq!(map["opacity"], "1");
}

// case: a trimmed HTML says it was trimmed
#[test]
fn parity_a_trimmed_html_says_it_was_trimmed() {
    let html = format!("x{}", "y".repeat(MAX_POINTED_HTML + 40));
    let read = read_browser_point(&picked("div", &html), URL).expect("pick");
    let block = browser_point_block(&read.point, "");
    assert!(block.contains("more characters of its code not shown"), "{block}");
    assert!(block.contains(PAGE_POINT_MARK), "{block}");
}

// case: several picks become several blocks
#[test]
fn parity_several_picks_become_several_blocks() {
    let one = read_browser_point(&picked("button#one", "<button id=\"one\">one</button>"), URL).unwrap().point;
    let two = read_browser_point(&picked("button#two", "<button id=\"two\">two</button>"), URL).unwrap().point;
    let chips = [
        PagePointChip { id: "one".into(), note: "the spacing is tight".into(), attachment: None, point: one },
        PagePointChip { id: "two".into(), note: String::new(), attachment: None, point: two },
    ];
    let (text, _) = message_of_page_points(&chips);
    assert_eq!(text.split(PAGE_POINT_MARK).count(), 3);
    assert!(text.contains("Part: button#one"));
    assert!(text.contains("Part: button#two"));
    assert!(text.contains("Note: the spacing is tight"));
    assert!(text.contains("width: 180px"));
}

// case: removing a chip removes its picture
#[test]
fn parity_removing_a_chip_removes_its_picture() {
    let a = read_browser_point(&picked("button#save", "<button>Save</button>"), URL).unwrap().point;
    let b = read_browser_point(&picked("p", "<p>Hi</p>"), URL).unwrap().point;
    let mut chips = vec![
        PagePointChip { id: "a".into(), note: String::new(), attachment: Some("shot-a.png".into()), point: a },
        PagePointChip { id: "b".into(), note: String::new(), attachment: Some("shot-b.png".into()), point: b },
    ];
    assert_eq!(message_of_page_points(&chips).1, ["shot-a.png", "shot-b.png"]);
    without_page_point(&mut chips, "a");
    assert_eq!(chips.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["b"]);
    assert_eq!(message_of_page_points(&chips).1, ["shot-b.png"]);
}

// case: browser: false turns pointing off too
#[test]
fn parity_browser_false_turns_pointing_off_too() {
    let refusal = crate::browser::policy_refusal(&json!({"browser": false}), std::path::Path::new("C:/policy.json"));
    let err = begin_browser_point(refusal.as_deref()).unwrap_err();
    assert!(err.contains("The browser panel is off."), "{err}");
    assert!(begin_browser_point(None).is_ok());
}

// case: a password field is not pictured with its contents
#[test]
fn parity_a_password_field_is_not_pictured_with_its_contents() {
    let html = r#"<input id="pw" type="password" value="s3cret-pass">"#;
    let mut raw = picked("input#pw", html);
    raw["sensitive"] = json!("password");
    let captures = Cell::new(0);
    let answer = point_answer(Some(&json!(true)), Ok(Some(&raw)), false, URL, |_| {
        captures.set(captures.get() + 1);
        Some(vec![1])
    });
    assert_eq!(captures.get(), 0);
    let PointAnswer::Picked { point, png } = answer else { panic!("{answer:?}") };
    assert_eq!(png, None);
    assert!(point.picture_note().is_some_and(|n| n.contains("password")));
    assert!(!point.html().contains("s3cret-pass"));
}

// case: says when the page does not allow pointing
#[test]
fn parity_says_when_the_page_does_not_allow_pointing() {
    let raw = picked("button#save", "<button id=\"save\">Save</button>");
    let expected = PointAnswer::Refused { message: POINT_BLOCKED.into(), blocked: true };
    assert_eq!(point_answer(Some(&json!(false)), Ok(Some(&raw)), false, URL, |_| None), expected);
    assert_eq!(point_answer(None, Ok(Some(&raw)), false, URL, |_| None), expected);
}

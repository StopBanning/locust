//! Carries `apps/desktop/src/main/a-part-of-the-browser-can-be-pointed-at.test.ts` (the parts that need no window) and
//! the two promises pointing keeps: what is typed into a form never comes along, and a password or card field is
//! never pictured.
use super::*;
use serde_json::json;
use std::cell::Cell;
use std::process::Command;

const URL: &str = "http://127.0.0.1:9786/";

fn pick(selector: &str, html: &str) -> Value {
    json!({
        "selector": selector, "html": html, "htmlLength": html.chars().count(),
        "rect": {"x": 4, "y": 8, "width": 180, "height": 40}, "viewport": {"width": 800, "height": 600},
        "styles": {"width": "180px", "height": "40px", "display": "flex", "color": "rgb(11, 22, 33)"}
    })
}

/// A point, made the only way one is made: read from what a picker answered.
pub(crate) fn point(selector: &str, html: &str) -> BrowserPoint {
    read_browser_point(&pick(selector, html), URL).expect("a pick").point
}

fn chip(id: &str, note: &str, attachment: Option<&str>, point: BrowserPoint) -> PagePointChip {
    PagePointChip { id: id.into(), point, note: note.into(), attachment: attachment.map(str::to_owned) }
}

// case: the picker script parses
#[test]
fn the_picker_script_parses() {
    for script in BROWSER_POINT_SCRIPTS {
        let probe = format!("new (require('vm').Script)({}); process.stdout.write('ok')", serde_json::to_string(script).unwrap());
        let out = match Command::new("node").arg("-e").arg(&probe).output() {
            Ok(out) => out,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(e) => panic!("node failed: {e}"),
        };
        assert!(out.status.success() && out.stdout == b"ok", "{}", String::from_utf8_lossy(&out.stderr));
    }
    // The window runs the picker as a function body that returns its promise (callAsyncJavaScript): it starts with
    // the expression, never a comment that would end the return.
    assert!(BROWSER_PICKER_SCRIPT.starts_with("(() => new Promise("));
}

#[test]
fn the_picker_request_goes_through_main_only() {
    // The window asks; the browser host runs Locust's own scripts and nothing a page or the window hands in.
    let host = include_str!("../../src/browser_mac.rs");
    assert!(host.contains("Command::Point { reply } => point(&p.view, reply, mtm)"), "the command carries no script");
    let point_fn = &host[host.find("\nfn point(").expect("the host runs pointing")..];
    let point_fn = &point_fn[..point_fn.find("\n}\n").unwrap()];
    assert!(point_fn.contains("BROWSER_POINT_PROBE") && point_fn.contains("BROWSER_PICKER_SCRIPT"));
    // It runs two scripts, both its own, and photographs only where point_answer gives it a place.
    assert_eq!(point_fn.matches("callAsyncJavaScript").count(), 2);
    assert!(!point_fn.contains("evaluateJavaScript"));
    assert!(point_fn.contains("point_answer("));
    let photographed = Cell::new(0);
    let answer = point_answer(Some(&json!(true)), Ok(Some(&pick("button#save", "<button id=\"save\">Save</button>"))), false, URL, |_| {
        photographed.set(photographed.get() + 1);
        Some(vec![1, 2, 3, 4])
    });
    let PointAnswer::Picked { point, png } = answer else { panic!("{answer:?}") };
    assert_eq!(point.selector(), "button#save");
    assert_eq!(png, Some(vec![1, 2, 3, 4]));
    assert_eq!(photographed.get(), 1);
}

// case: form values never appear in a pick
#[test]
fn form_values_never_appear_in_a_pick() {
    let html = r#"<form><input type="text" value="typed-in"><input type="password" value="s3cret-pass"><textarea>typed words</textarea><select><option value="secret-opt" selected>Shown</option></select><input value='single' /><input value=bare></form>"#;
    let read = read_browser_point(&pick("form", html), URL).expect("a pick");
    let quoted = read.point.html();
    for secret in ["s3cret-pass", "typed words", "typed-in", "secret-opt", "selected", "single", "bare"] {
        assert!(!quoted.contains(secret), "{secret} in {quoted}");
    }
    assert!(quoted.contains("Shown"), "{quoted}");
    // And so nowhere a point goes: the chip's block, and the message.
    let message = message_of_page_points(&[chip("f", "", None, read.point.clone())]).0;
    for secret in ["s3cret-pass", "typed words", "typed-in", "secret-opt"] {
        assert!(!message.contains(secret), "{secret} in {message}");
    }
}

// case: lists the styles that matter for a visual fix
#[test]
fn lists_the_styles_that_matter_for_a_visual_fix() {
    let raw = json!({
        "width": "180px", "height": "40px",
        "margin-top": "0px", "margin-right": "0px", "margin-bottom": "0px", "margin-left": "0px",
        "padding-top": "8px", "padding-right": "16px", "padding-bottom": "8px", "padding-left": "16px",
        "display": "flex", "flex-direction": "row", "position": "static", "top": "10px", "color": "rgb(11, 22, 33)",
        "border-top-width": "1px", "border-right-width": "1px", "border-bottom-width": "1px", "border-left-width": "1px",
        "border-top-style": "solid", "border-top-color": "rgb(1, 2, 3)",
        "border-top-left-radius": "6px", "border-top-right-radius": "6px", "border-bottom-right-radius": "6px", "border-bottom-left-radius": "6px",
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
fn a_trimmed_html_says_it_was_trimmed() {
    let html = format!("x{}", "y".repeat(MAX_POINTED_HTML + 40));
    let block = browser_point_block(&point("div", &html), "");
    assert!(block.contains("(41 more characters of its code not shown)"), "{block}");
    assert!(block.starts_with(PAGE_POINT_MARK));
    // A fence in the page's code cannot close the quote.
    assert!(browser_point_block(&point("pre", "<pre>```js</pre>"), "").contains("````html"));
}

// case: several picks become several blocks
#[test]
fn several_picks_become_several_blocks() {
    let chips = [chip("one", "the spacing is tight", None, point("button#one", "<button id=\"one\">one</button>")), chip("two", "", None, point("button#two", "<button id=\"two\">two</button>"))];
    let (text, _) = message_of_page_points(&chips);
    assert_eq!(text.split(PAGE_POINT_MARK).count(), 3);
    assert!(text.contains("Part: button#one"));
    assert!(text.contains("Part: button#two"));
    assert!(text.contains("Note: the spacing is tight"));
    assert!(text.contains("width: 180px"));
}

// case: removing a chip removes its picture
#[test]
fn removing_a_chip_removes_its_picture() {
    let mut chips = vec![chip("a", "", Some("shot-a.png"), point("button#save", "<button>Save</button>")), chip("b", "", Some("shot-b.png"), point("p", "<p>Hi</p>"))];
    assert_eq!(message_of_page_points(&chips).1, ["shot-a.png", "shot-b.png"]);
    without_page_point(&mut chips, "a");
    assert_eq!(chips.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(), ["b"]);
    assert_eq!(message_of_page_points(&chips).1, ["shot-b.png"]);
}

#[test]
fn browser_false_turns_pointing_off_too() {
    let refusal = crate::browser::policy_refusal(&json!({"browser": false}), std::path::Path::new("/synthetic/policy.json"));
    let refused = begin_browser_point(refusal.as_deref()).unwrap_err();
    assert!(refused.contains("The browser panel is off."), "{refused}");
    assert!(begin_browser_point(None).is_ok());
    // The panel's button says the same sentence, and cannot be pressed.
    let control = browser_point_control(refusal.as_deref(), false, false, true);
    assert!(control.disabled && control.sentence && control.label == refused);
}

// case: a password field is not pictured with its contents
#[test]
fn a_password_field_is_not_pictured_with_its_contents() {
    let html = r#"<input id="pw" type="password" value="s3cret-pass">"#;
    let photographed = Cell::new(0);
    let mut raw = pick("input#pw", html);
    raw["sensitive"] = json!("password");
    let answer = point_answer(Some(&json!(true)), Ok(Some(&raw)), false, URL, |_| {
        photographed.set(photographed.get() + 1);
        Some(vec![1])
    });
    assert_eq!(photographed.get(), 0);
    let PointAnswer::Picked { point, png } = answer else { panic!("{answer:?}") };
    assert_eq!(png, None);
    assert!(point.picture_note().is_some_and(|n| n.contains("password")));
    assert!(!point.html().contains("s3cret-pass"));
    // Read from its code when the picker did not say so.
    let read = read_browser_point(&pick("input#pw", html), URL).unwrap();
    assert_eq!(picture_rect(&read, 1.0), None);
}

#[test]
fn a_card_or_one_time_code_field_is_never_pictured_either() {
    for (html, word) in [
        (r#"<input autocomplete="cc-number" name="card">"#, "payment"),
        (r#"<label>CVV <input id="cvv"></label>"#, "payment"),
        (r#"<input autocomplete="one-time-code">"#, "one-time-code"),
    ] {
        let read = read_browser_point(&pick("input", html), URL).unwrap();
        assert_eq!(picture_rect(&read, 1.0), None, "{html}");
        assert!(read.point.picture_note().is_some_and(|n| n.contains(word)), "{html}");
        let answer = point_answer(Some(&json!(true)), Ok(Some(&pick("input", html))), false, URL, |_| panic!("photographed {html}"));
        assert!(matches!(answer, PointAnswer::Picked { png: None, .. }));
    }
    // A plain part is pictured, inside what the page shows.
    let read = read_browser_point(&pick("button", "<button>Save</button>"), URL).unwrap();
    assert_eq!(picture_rect(&read, 1.0), Some(Rect { x: 4.0, y: 8.0, width: 180.0, height: 40.0 }));
}

// case: says when the page does not allow pointing
#[test]
fn says_when_the_page_does_not_allow_pointing() {
    let blocked = PointAnswer::Refused { message: POINT_BLOCKED.into(), blocked: true };
    let raw = pick("button#save", "<button>Save</button>");
    assert_eq!(point_answer(Some(&json!(false)), Ok(Some(&raw)), false, URL, |_| None), blocked);
    assert_eq!(point_answer(None, Ok(Some(&raw)), false, URL, |_| None), blocked);
    assert_eq!(point_answer(Some(&json!(true)), Err(()), false, URL, |_| None), blocked);
    // Escape or no answer adds nothing; a page that closed says so.
    assert_eq!(point_answer(Some(&json!(true)), Ok(None), false, URL, |_| None), PointAnswer::Nothing);
    assert_eq!(point_answer(Some(&json!(true)), Ok(Some(&Value::Null)), false, URL, |_| None), PointAnswer::Nothing);
    assert_eq!(point_answer(Some(&json!(true)), Err(()), true, URL, |_| None), PointAnswer::Refused { message: POINT_PAGE_CLOSED.into(), blocked: false });
}

#[test]
fn reads_only_a_pick_and_keeps_it_to_its_bounds() {
    assert!(read_browser_point(&Value::Null, URL).is_none());
    assert!(read_browser_point(&json!({"selector": "h1", "html": "<h1>"}), URL).is_none());
    let mut nan = pick("h1", "<h1>");
    nan["viewport"]["width"] = json!(0);
    assert!(read_browser_point(&nan, URL).is_none());
    let mut huge = pick(&"s".repeat(900), &"h".repeat(9_000));
    huge["htmlLength"] = json!(3);
    huge["rect"]["width"] = json!(-5);
    let read = read_browser_point(&huge, URL).unwrap();
    assert_eq!(read.point.selector().len(), 300);
    assert_eq!(read.point.html().len(), 4_000);
    assert_eq!(read.point.html_length(), 4_000);
    assert_eq!(read.rect.width, 0.0);
    // A file the page names, and nothing guessed.
    assert_eq!(read_named_source(&json!({"file": "src/SaveButton.tsx", "line": 18})), Some(Source { file: "src/SaveButton.tsx".into(), line: Some(18) }));
    assert_eq!(read_named_source(&json!({"file": "Button"})), None);
    assert_eq!(read_named_source(&json!({"file": "a b.tsx"})), None);
}

#[test]
fn holds_eight_parts_and_takes_a_note_of_five_hundred_characters() {
    let mut chips = Vec::new();
    for i in 0..MAX_PAGE_POINTS {
        add_page_point(&mut chips, chip(&i.to_string(), "", None, point("p", "<p>Hi</p>"))).unwrap();
    }
    assert_eq!(add_page_point(&mut chips, chip("9", "", None, point("p", "<p>Hi</p>"))), Err(EIGHT_IN_THE_BOX));
    note_on_page_point(&mut chips, "0", &"n".repeat(600));
    assert_eq!(chips[0].note.len(), 500);
}

#[test]
fn takes_ctrl_or_cmd_with_shift_and_c_and_says_so_on_the_button() {
    assert!(is_browser_point_shortcut(true, false, true, false, 'c'));
    assert!(is_browser_point_shortcut(false, true, true, false, 'C'));
    assert!(!is_browser_point_shortcut(true, false, false, false, 'c'));
    assert!(!is_browser_point_shortcut(true, false, true, true, 'c'));
    assert_eq!(browser_point_control(None, false, false, false).label, "Open a page to point at it.");
    assert_eq!(browser_point_control(None, true, false, true), PointControl { label: POINT_BLOCKED.into(), disabled: true, pressed: false, sentence: true });
    assert!(browser_point_control(None, false, true, true).pressed);
}

// a-part-of-a-page-can-be-pointed-at.test.ts, the picture of it (captureRectOf), with Electron's numbers.

#[test]
fn is_the_part_inside_the_frame_in_the_windows_pixels_at_its_zoom() {
    let frame = Rect { x: 600.0, y: 100.0, width: 400.0, height: 300.0 };
    let part = Rect { x: 10.0, y: 20.0, width: 100.0, height: 50.0 };
    assert_eq!(capture_rect_of(frame, part, 1.0), Some(Rect { x: 610.0, y: 120.0, width: 100.0, height: 50.0 }));
    assert_eq!(capture_rect_of(frame, part, 1.25), Some(Rect { x: 763.0, y: 150.0, width: 125.0, height: 63.0 }));
}

#[test]
fn is_cut_to_what_the_frame_shows_and_is_none_when_nothing_of_it_is() {
    let frame = Rect { x: 600.0, y: 100.0, width: 400.0, height: 300.0 };
    assert_eq!(capture_rect_of(frame, Rect { x: -50.0, y: 250.0, width: 100.0, height: 200.0 }, 1.0), Some(Rect { x: 600.0, y: 350.0, width: 50.0, height: 50.0 }));
    assert_eq!(capture_rect_of(frame, Rect { x: 10.0, y: 400.0, width: 100.0, height: 50.0 }, 1.0), None);
}

//! Carries `apps/desktop/src/main/a-part-of-a-page-can-be-pointed-at.test.ts` (T159).
//! In native, pages run in the browser panel (no inline viewer iframe), so page pointing uses `browser_point`.
//! Case 2 (`finds the viewer's frame showing the page, and no other`) applies only to an inline iframe.
use super::*;
use serde_json::json;
use std::process::Command;

const PAGE_URL: &str = "locust-page://0123456789abcdef01234567/index.html";

// case: is a script that parses, and so is its cancel
#[test]
fn is_a_script_that_parses_and_so_is_its_cancel() {
    for script in [BROWSER_PICKER_SCRIPT, BROWSER_POINT_CANCEL] {
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

// case: is kept to its shape and its bounds
#[test]
fn is_kept_to_its_shape_and_its_bounds() {
    assert!(read_browser_point(&Value::Null, PAGE_URL).is_none());
    assert!(read_browser_point(&json!({"selector": "h1", "html": "<h1>", "htmlLength": 4}), PAGE_URL).is_none());
    assert!(
        read_browser_point(
            &json!({"selector": "h1", "html": "<h1>", "htmlLength": 4, "rect": {"x": null, "y": 0, "width": 1, "height": 1}, "viewport": {"width": 800, "height": 600}}),
            PAGE_URL
        )
        .is_none()
    );
    let huge = read_browser_point(
        &json!({
            "selector": "s".repeat(900),
            "html": "h".repeat(9_000),
            "htmlLength": 3,
            "rect": {"x": 0, "y": 0, "width": -5, "height": 4},
            "viewport": {"width": 800, "height": 600}
        }),
        PAGE_URL,
    )
    .expect("bounded pick");
    assert_eq!(huge.point.selector().len(), 300);
    assert_eq!(huge.point.html().len(), 4_000);
    assert_eq!(huge.rect.width, 0.0);
}

// case: names the file and the part, and quotes its code
#[test]
fn names_the_file_and_the_part_and_quotes_its_code() {
    let html = "<h1 class=\"title\">\n  Fresh bread\n</h1>";
    let read = read_browser_point(
        &json!({
            "selector": "main > section.hero > h1",
            "html": html,
            "htmlLength": 36,
            "rect": {"x": 1, "y": 2, "width": 30, "height": 10},
            "viewport": {"width": 800, "height": 600},
            "source": {"file": "index.html"}
        }),
        PAGE_URL,
    )
    .expect("pick");
    let block = browser_point_block(&read.point, "");
    assert!(block.contains("Source: index.html (named by the page)"), "{block}");
    assert!(block.contains("Part: main > section.hero > h1"), "{block}");
    assert!(block.contains("```html\n<h1 class=\"title\">\n  Fresh bread\n</h1>\n```"), "{block}");
}

// case: counts what it leaves out, and a fence in the code cannot close the quote
#[test]
fn counts_what_it_leaves_out_and_a_fence_in_the_code_cannot_close_the_quote() {
    let long_read = read_browser_point(
        &json!({
            "selector": "div",
            "html": "x".repeat(MAX_POINTED_HTML + 40),
            "htmlLength": MAX_POINTED_HTML + 540,
            "rect": {"x": 1, "y": 2, "width": 30, "height": 10},
            "viewport": {"width": 800, "height": 600}
        }),
        PAGE_URL,
    )
    .expect("long pick");
    let long = browser_point_block(&long_read.point, "");
    assert!(long.contains("(540 more characters of its code not shown)"), "{long}");

    let fenced_read = read_browser_point(
        &json!({
            "selector": "pre",
            "html": "<pre>```js</pre>",
            "htmlLength": 16,
            "rect": {"x": 1, "y": 2, "width": 30, "height": 10},
            "viewport": {"width": 800, "height": 600}
        }),
        PAGE_URL,
    )
    .expect("fenced pick");
    let fenced = browser_point_block(&fenced_read.point, "");
    assert!(fenced.contains("````html"), "{fenced}");
}

// case: is the part inside the frame, in the window’s pixels at its zoom
#[test]
fn part_inside_frame_in_windows_pixels_at_its_zoom() {
    let frame = Rect { x: 600.0, y: 100.0, width: 400.0, height: 300.0 };
    let part = Rect { x: 10.0, y: 20.0, width: 100.0, height: 50.0 };
    assert_eq!(capture_rect_of(frame, part, 1.0), Some(Rect { x: 610.0, y: 120.0, width: 100.0, height: 50.0 }));
    assert_eq!(capture_rect_of(frame, part, 1.25), Some(Rect { x: 763.0, y: 150.0, width: 125.0, height: 63.0 }));
}

// case: is cut to what the frame shows, and is none when nothing of it is
#[test]
fn cut_to_what_the_frame_shows_and_none_when_nothing_of_it_is() {
    let frame = Rect { x: 600.0, y: 100.0, width: 400.0, height: 300.0 };
    assert_eq!(
        capture_rect_of(frame, Rect { x: -50.0, y: 250.0, width: 100.0, height: 200.0 }, 1.0),
        Some(Rect { x: 600.0, y: 350.0, width: 50.0, height: 50.0 })
    );
    assert_eq!(
        capture_rect_of(frame, Rect { x: 10.0, y: 400.0, width: 100.0, height: 50.0 }, 1.0),
        None
    );
}

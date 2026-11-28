use super::*;
use locust_core::browser_point::{PagePointChip, read_browser_point};

fn selected_part(selector: &str, html: &str) -> locust_core::browser_point::BrowserPoint {
    read_browser_point(
        &serde_json::json!({
            "selector": selector, "html": html,
            "rect": {"x": 16, "y": 16, "width": 180, "height": 40},
            "viewport": {"width": 1000, "height": 800},
            "styles": {"width": "180px", "color": "rgb(11, 22, 33)"},
            "source": {"file": "src/SaveButton.tsx", "line": 18}
        }),
        "http://127.0.0.1:9786/",
    )
    .unwrap()
    .point
}

// case: a pointed part is a chip with a note, its styles, and a way to remove it
#[test]
fn composer_pointed_part_note_and_remove_act_on_the_visible_chip() {
    let (root, mut state) = fresh();
    let picture = root.0.join("page-part-1.png");
    std::fs::write(&picture, b"fixture").unwrap();
    state.draft.replace_text("Keep this draft.");
    state.page_points.chips.push(PagePointChip {
        id: "save".into(),
        point: selected_part("button#save", "<button id=\"save\">Save</button>"),
        note: "the spacing is tight".into(),
        attachment: Some(picture.to_string_lossy().into_owned()),
    });
    state
        .page_points
        .notes
        .entry("save".into())
        .or_default()
        .replace_text("the spacing is tight");
    let mut rt = window(state);
    let shown = words(&rt);
    for word in [
        "button#save",
        "src/SaveButton.tsx:18",
        "width: 180px",
        "the spacing is tight",
        "Note on button#save",
        "Remove button#save",
    ] {
        assert!(shown.contains(word), "missing {word}");
    }
    assert!(
        rt.elements()
            .iter()
            .any(|e| e.key.as_deref() == Some("save/picture")
                && e.help.as_deref() == picture.to_str())
    );
    click(&mut rt, "save/note");
    rt.paste(" check");
    frames(&mut rt);
    assert!(rt.state.page_points.notes["save"].text.contains("check"));
    click(&mut rt, "save/remove");
    assert!(rt.state.page_points.chips.is_empty());
    assert!(!picture.exists());
    assert!(!rt.state.page_points.notes.contains_key("save"));
    assert_eq!(rt.state.draft.text, "Keep this draft.");
    assert!(rt.state.flight.is_none());
}

// case: a password field's chip has no picture of its contents
#[test]
fn composer_password_chip_says_why_it_has_no_picture_and_shows_no_value() {
    let (_root, mut state) = fresh();
    let point = selected_part(
        "input#pw",
        "<input id=\"pw\" type=\"password\" value=\"s3cret-pass\">",
    );
    assert!(!point.html().contains("s3cret-pass"));
    state.page_points.chips.push(PagePointChip {
        id: "pw".into(),
        point,
        note: String::new(),
        attachment: None,
    });
    let rt = window(state);
    let shown = words(&rt);
    assert!(shown.contains("No picture: a password field is not photographed with its contents."));
    assert!(!shown.contains("s3cret-pass"));
    assert!(rt.find("pw/picture").is_none());
}

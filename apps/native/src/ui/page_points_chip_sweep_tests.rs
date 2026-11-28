use super::*;

// case: a pointed part is a chip with a note, its styles, and a way to remove it
#[test]
fn a_pointed_chip_shows_its_file_and_note_and_the_remove_button_discards_it() {
    let fixture = temp();
    let picture = fixture.0.join("page-part-1.png");
    std::fs::write(&picture, b"test screenshot").unwrap();
    let mut s = State::default();
    with_chips(&mut s);
    s.page_points.chips[0].attachment = Some(picture.to_str().unwrap().to_owned());

    let mut rt = box_drawn(s);
    let said = texts(&rt);
    for expected in ["button#save", "src/SaveButton.tsx:18", "width: 180px", "the spacing is tight"] {
        assert!(said.contains(expected), "{said}");
    }
    let elements = rt.elements();
    let shown = elements.iter().find(|e| e.key.as_deref() == Some("save/picture")).unwrap();
    assert_eq!(shown.help.as_deref(), picture.to_str());
    for name in ["Note on button#save", "Remove button#save"] {
        assert!(elements.iter().any(|e| e.label.as_deref() == Some(name) && e.focusable), "{name}");
    }

    let (x, y) = rt.find("save/remove").expect("the chip's remove button");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert_eq!(rt.state.page_points.chips.len(), 1);
    assert_eq!(rt.state.page_points.chips[0].point.selector(), "input#pw");
    assert!(!rt.state.page_points.notes.contains_key("save"));
    assert!(!picture.exists());
}

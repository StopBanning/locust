use super::*;

#[test]
fn diagram_code_tabs_report_selection_and_move_it_on_press() {
    let mut rt = rune::Runtime::new(
        State::default(),
        |s, _| reply_markdown_for_tests(s, "```mermaid\ngraph LR\n  A[Push] --> B[Tests]\n```", &Palette::new(false)),
        (800.0, 1000.0), rune::Env::new(false, 1.0),
    );
    rt.frame(0.0);
    let selected = |rt: &rune::Runtime<State>, label: &str| {
        let tab = rt.elements().into_iter().find(|e| !e.exiting && e.label.as_deref() == Some(label) && e.role == "tab")
            .unwrap_or_else(|| panic!("no tab labelled {label}"));
        assert!(tab.focusable, "{label} tab must be keyboard reachable");
        tab.item_selected
    };
    assert!(selected(&rt, "Diagram"));
    assert!(!selected(&rt, "Code"));
    let (x, y) = rt.find("diagram-0/tab-code").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert!(!selected(&rt, "Diagram"));
    assert!(selected(&rt, "Code"));
}

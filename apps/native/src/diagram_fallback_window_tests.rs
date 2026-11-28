use crate::{Palette, State};

#[test]
fn an_undrawable_diagram_keeps_its_source_and_caption_after_a_diff() {
    const REPLY: &str = "Here is the change:\n\n```diff\n-old\n+new\n```\n\n```mermaid\nkanban\n  Todo\n```\n\n```js\nconst a = 1\n```";
    let mut window = rune::Runtime::new(
        State::default(),
        |s, _| crate::reply_actions::reply_markdown_for_tests(s, REPLY, &Palette::new(false)),
        (900.0, 1500.0),
        rune::Env::new(false, 1.0),
    );
    window.frame(0.0);
    let elements = window.elements();
    let caption = "Not drawn as a diagram (Locust could not draw it). Its text is above, as written.";
    // Rune gives the parent a Markdown block key; the accessible figure role survives that wrapper.
    let figure = elements.iter().find(|e| e.role == "figure")
        .expect("the unreadable diagram has a source figure").rect;
    let inside = |wanted: &str| elements.iter().any(|e| {
        let y = e.rect[1] + e.rect[3] / 2.0;
        y >= figure[1] && y <= figure[1] + figure[3] && e.text.as_deref().is_some_and(|t| t.contains(wanted))
    });
    assert!(inside("kanban"));
    assert!(inside(caption));
    assert!(!inside("const a = 1"));
    assert!(elements.iter().any(|e| e.text.as_deref().is_some_and(|t| t.contains("const a = 1"))));
    assert_eq!(elements.iter().filter(|e| e.text.as_deref() == Some(caption)).count(), 1);
}

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

#[test]
fn a_code_fence_inside_a_quote_does_not_steal_the_next_diagrams_caption() {
    const REPLY: &str = "> A quoted example:\n>\n> ```text\n> quoted_source\n> ```\n\n```mermaid\nkanban\n  Todo\n```\n";
    const CAPTION: &str = "Not drawn as a diagram (Locust could not draw it). Its text is above, as written.";
    let mut window = rune::Runtime::new(
        State::default(),
        |s, _| crate::reply_actions::reply_markdown_for_tests(s, REPLY, &Palette::new(false)),
        (900.0, 1500.0),
        rune::Env::new(false, 1.0),
    );
    window.frame(0.0);
    let elements = window.elements();
    let quote = elements.iter().find(|e| e.role == "blockquote").expect("the rendered quote");
    let diagram = elements.iter().find(|e| e.role == "figure").expect("the diagram source figure");
    let source = |needle: &str| elements.iter().find(|e| {
        !e.exiting && e.text.as_deref().is_some_and(|t| t.contains(needle))
    }).unwrap_or_else(|| panic!("missing rendered text: {needle}"));
    let in_box = |child: &rune::ElementInfo, parent: &rune::ElementInfo| {
        let y = child.rect[1] + child.rect[3] / 2.0;
        y >= parent.rect[1] && y <= parent.rect[1] + parent.rect[3]
    };
    assert!(in_box(source("quoted_source"), quote));
    assert!(in_box(source("kanban"), diagram), "the captioned figure must contain the diagram source");
    assert!(in_box(source(CAPTION), diagram));
    assert!(!in_box(source(CAPTION), quote), "the earlier quoted fence must not get the caption");
    assert_eq!(elements.iter().filter(|e| e.text.as_deref() == Some(CAPTION)).count(), 1);
}

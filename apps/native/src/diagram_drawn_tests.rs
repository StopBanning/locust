//! a-diagram-in-a-reply-is-drawn.test.ts through the window: a reply's Mermaid fence drawn headless by `reply_markdown`
//! (which prepares it with `prepare_reply_markdown` and draws it with rune-mermaid), its kind named in the card's bar,
//! and the caption under a fence it does not draw. Expected words and sizes are Electron's (shared/diagram.ts's test).
//!
//! Electron draws a diagram in a hidden web view that answers with an SVG; the window draws it in-process with
//! rune-mermaid, so the cases about the page's answer, the view (drawn once, kept, a burst, a stall, a page that did not
//! load) and Mermaid's own parse errors have no Rust counterpart: see the ledger note.
use super::*;

/// The reply drawn headless: each element's (key, role, text, label).
fn drawn(body: &str) -> Vec<(Option<String>, &'static str, String, String)> {
    let mut s = crate::State::default();
    s.image_cache = Some(std::sync::Arc::new(rune::ImageCache::new(std::sync::Arc::new(|| {}))));
    let mut rt = rune::Runtime::new(
        (s, body.to_owned()),
        |(s, body), _| crate::reply_actions::reply_markdown_for_tests(s, body, &crate::Palette::new(false)),
        (800.0, 3000.0),
        rune::Env::new(false, 1.0),
    );
    rt.frame(0.0);
    rt.elements().into_iter().map(|e| (e.key, e.role, e.text.unwrap_or_default(), e.label.unwrap_or_default())).collect()
}

fn fence(language: &str, code: &str) -> String {
    format!("Here it is:\n\n```{language}\n{code}\n```\n")
}

/// Whether the reply drew a diagram: the figure Electron's ReplyDiagram is.
fn has_diagram(d: &[(Option<String>, &'static str, String, String)]) -> bool {
    d.iter().any(|(_, role, _, label)| *role == "figure" && label == "A diagram in this reply")
}

fn texts(d: &[(Option<String>, &'static str, String, String)]) -> Vec<String> {
    d.iter().map(|(_, _, t, _)| t.clone()).filter(|t| !t.is_empty()).collect()
}

#[test]
fn is_a_block_the_model_named_mermaid_with_something_in_it_of_a_size_locust_draws() {
    // case: is a block the model named Mermaid, with something in it, of a size Locust draws
    let graph = "graph TD\n  A --> B";
    assert!(is_diagram(graph, Some("mermaid")));
    assert!(is_diagram(graph, Some("Mermaid ")));
    assert!(is_diagram(graph, Some("mmd")));
    assert!(!is_diagram(graph, Some("js")));
    assert!(!is_diagram(graph, None));
    assert!(!is_diagram("   \n", Some("mermaid")));
    let huge = format!("graph TD\n{}", "  A --> B\n".repeat(MAX_DIAGRAM_CHARS / 10));
    assert!(!is_diagram(&huge, Some("mermaid")));
    // Through the reply: named Mermaid (or mmd), it is drawn; named js, or with no name, it stays code.
    assert!(has_diagram(&drawn(&fence("mermaid", graph))));
    assert!(has_diagram(&drawn(&fence("mmd", graph))));
    assert!(!has_diagram(&drawn(&fence("js", graph))));
    assert!(!has_diagram(&drawn(&fence("", graph))));
    // Past the size Locust draws: its text stays, with the caption saying so.
    let d = drawn(&fence("mermaid", &huge));
    assert!(!has_diagram(&d));
    assert!(texts(&d).iter().any(|t| t == "Not drawn as a diagram (Locust could not draw it). Its text is above, as written."), "{:?}", texts(&d));
}

#[test]
fn says_mermaids_reason_as_one_short_line() {
    // case: says Mermaid's reason as one short line
    assert_eq!(reason_of("Parse error on line 3:\n...A --> \n-------^\nExpecting 'NODE_STRING', got 'EOF'"), "Parse error on line 3");
    assert_eq!(reason_of(""), "Mermaid could not read it");
    assert_eq!(reason_of(&"x".repeat(400)).chars().count(), 160);
    // Through the reply: what it could not draw is said in one line under its text.
    let d = drawn(&fence("mermaid", "this is not one Mermaid knows"));
    let caption = texts(&d).into_iter().find(|t| t.starts_with("Not drawn as a diagram")).expect("a caption");
    assert!(!caption.contains('\n'), "{caption}");
}

#[test]
fn names_the_kind_as_a_person_would_and_shows_the_picture_as_data() {
    // case: names the kind as a person would, and shows the picture as data
    assert_eq!(diagram_kind_words(Some("flowchart-v2")), "Flowchart");
    assert_eq!(diagram_kind_words(Some("sequence")), "Sequence diagram");
    assert_eq!(diagram_kind_words(Some("somethingNew")), "Diagram");
    assert_eq!(diagram_kind_words(None), "Diagram");
    // Through the reply: the card's bar names it. (There is no SVG to carry as a data address: rune-mermaid draws the
    // diagram as the window's own shapes, and nothing of it is handed to a page.)
    let d = drawn(&fence("mermaid", "graph TD\n  A --> B"));
    assert!(texts(&d).iter().any(|t| t == "Flowchart"), "{:?}", texts(&d));
    let d = drawn(&fence("mermaid", "sequenceDiagram\n  Alice->>Bob: Hi"));
    assert!(texts(&d).iter().any(|t| t == "Sequence diagram"), "{:?}", texts(&d));
}

#[test]
fn keeps_a_mistake_too_and_draws_nothing_that_is_not_a_diagram() {
    // case: keeps a mistake too, and draws nothing that is not a diagram
    // A fence named Mermaid with nothing in it is not a diagram: nothing is drawn, and the caption says why in
    // Electron's words. (What is kept between draws is rune's memo, not a cache of the window's own.)
    let d = drawn("Here:\n\n```mermaid\n   \n```\n");
    assert!(!has_diagram(&d));
    assert!(texts(&d).iter().any(|t| t == "Not drawn as a diagram (it is not a diagram Locust draws). Its text is above, as written."), "{:?}", texts(&d));
    // The same mistake drawn again says the same thing.
    let again = drawn("Here:\n\n```mermaid\n   \n```\n");
    assert_eq!(texts(&again), texts(&d));
}

//! a-diagram-in-a-reply-shows-as-one.test.tsx through the window: a reply drawn headless by `reply_in_thread`, the
//! call `messages.rs` makes for each agent reply, with `live` as the thread passes it while a turn is still arriving.
//! A closed ```mermaid fence is framed as a diagram with its text a tab away; one still being written is its text,
//! as code, until the fence closes; every other fence stays the code it was. Expected words are Electron's.
//!
//! Electron says "Drawing the diagram…" until its web view answers. The window draws in-process (rune-mermaid), in
//! the frame the fence closes, so there is no wait to say: that line of the first case does not apply here.

/// The reply drawn headless as the thread draws it: each element's (role, text, label).
fn drawn_reply(body: &str, live: bool) -> Vec<(&'static str, String, String)> {
    let mut s = crate::State::default();
    s.image_cache = Some(std::sync::Arc::new(rune::ImageCache::new(std::sync::Arc::new(|| {}))));
    let mut rt = rune::Runtime::new(
        (s, body.to_owned(), live),
        |(s, body, live), _| crate::reply_actions::reply_in_thread(s, body, *live, true, &crate::Palette::new(false)),
        (800.0, 3000.0),
        rune::Env::new(false, 1.0),
    );
    rt.frame(0.0);
    rt.elements()
        .into_iter()
        .filter(|e| !e.exiting)
        .map(|e| (e.role, e.text.unwrap_or_default(), e.label.unwrap_or_default()))
        .collect()
}

/// Whether the reply framed a diagram: the figure Electron's ReplyDiagram (`lc-replydiagram`) is.
fn framed_as_diagram(d: &[(&'static str, String, String)]) -> bool {
    d.iter().any(|(role, _, label)| *role == "figure" && label == "A diagram in this reply")
}

fn shows_text(d: &[(&'static str, String, String)], words: &str) -> bool {
    d.iter().any(|(_, text, _)| text.contains(words))
}

fn reply_lines(rows: &[&str]) -> String {
    rows.join("\n")
}

fn flow() -> String {
    reply_lines(&["Here is how a deploy runs:", "", "```mermaid", "graph LR", "  A[Push] --> B[Tests] --> C[Staging]", "```", "", "Staging goes first."])
}

#[test]
fn is_framed_as_a_diagram_once_its_block_has_closed_its_text_a_tab_away() {
    // case: is framed as a diagram once its block has closed, its text a tab away
    let d = drawn_reply(&flow(), false);
    assert!(framed_as_diagram(&d), "{d:?}");
    assert!(d.iter().any(|(role, _, label)| *role == "tablist" && label == "Show the diagram or its text"), "{d:?}");
    assert!(d.iter().any(|(_, text, _)| text == "Diagram"), "{d:?}");
    assert!(d.iter().any(|(_, text, _)| text == "Code"), "{d:?}");
    // The prose around it is untouched.
    assert!(shows_text(&d, "Here is how a deploy runs:"), "{d:?}");
    assert!(shows_text(&d, "Staging goes first."), "{d:?}");
    // Still streaming, but the fence has closed: framed too.
    let d = drawn_reply(&reply_lines(&["Here is how a deploy runs:", "", "```mermaid", "graph LR", "  A --> B", "```", "", "Stag"]), true);
    assert!(framed_as_diagram(&d), "{d:?}");
}

#[test]
fn is_text_while_it_is_still_being_written() {
    // case: is text while it is still being written
    let d = drawn_reply(&reply_lines(&["Here it comes:", "", "```mermaid", "graph LR", "  A --> B"]), true);
    assert!(!framed_as_diagram(&d), "{d:?}");
    assert!(shows_text(&d, "graph LR"), "{d:?}");
    // Every token that arrives before the fence closes leaves it text: no diagram flickers in and out.
    let d = drawn_reply(&reply_lines(&["Here it comes:", "", "```mermaid", "graph LR", "  A --> B", "  B --> C", ""]), true);
    assert!(!framed_as_diagram(&d), "{d:?}");
    assert!(shows_text(&d, "graph LR"), "{d:?}");
    // The fence closes: now it is drawn.
    let d = drawn_reply(&reply_lines(&["Here it comes:", "", "```mermaid", "graph LR", "  A --> B", "  B --> C", "```"]), true);
    assert!(framed_as_diagram(&d), "{d:?}");
}

#[test]
fn leaves_every_other_block_as_the_code_it_was() {
    // case: leaves every other block as the code it was
    let d = drawn_reply(&reply_lines(&["```js", "const a = 1", "```"]), false);
    assert!(!framed_as_diagram(&d), "{d:?}");
    assert!(!d.iter().any(|(role, _, _)| *role == "figure"), "{d:?}");
    assert!(shows_text(&d, "const a = 1"), "{d:?}");
}

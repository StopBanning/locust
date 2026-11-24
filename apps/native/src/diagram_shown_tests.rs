//! Electron's `renderer/src/a-diagram-in-a-reply-shows-as-one.test.tsx` (0.770), through the window: a closed Mermaid
//! block is framed as a diagram with its text a tab away; one still being written is text until its fence closes; and
//! every other block is the code it was. Each reply is drawn headless by `reply_markdown`, which prepares it with
//! `diagram::prepare_reply_markdown` (`markdown_prep::prepare_reply`) as the message column does.
//!
//! Electron says "Drawing the diagram…" while a hidden page draws the picture; the window draws it in-process with
//! rune-mermaid, in the same frame, so there is no wait to say.
use crate::{Palette, State};

const FLOW: &str = "Here is how a deploy runs:\n\n```mermaid\ngraph LR\n  A[Push] --> B[Tests] --> C[Staging]\n```\n\nStaging goes first.";

thread_local! {
    /// The reply this test's window draws, and whether it is still arriving (rune's view is a plain `fn`).
    static REPLY: std::cell::Cell<(&'static str, bool)> = const { std::cell::Cell::new(("", false)) };
}

/// A window drawing one reply, finished (`live` false) or still arriving.
fn window(body: &'static str, live: bool) -> rune::Runtime<State> {
    REPLY.with(|r| r.set((body, live)));
    let mut s = State::default();
    s.image_cache = Some(std::sync::Arc::new(rune::ImageCache::new(std::sync::Arc::new(|| {}))));
    let mut rt = rune::Runtime::new(
        s,
        |s, _| {
            let (body, live) = REPLY.with(|r| r.get());
            let p = Palette::new(false);
            if live {
                crate::reply_actions::live_reply_markdown_for_tests(s, body, &p)
            } else {
                crate::reply_actions::reply_markdown_for_tests(s, body, &p)
            }
        },
        (800.0, 3000.0),
        rune::Env::new(false, 1.0),
    );
    rt.frame(0.0);
    rt
}

/// Each drawn element's (key, role, text, label).
fn drawn(rt: &rune::Runtime<State>) -> Vec<(Option<String>, &'static str, String, String)> {
    rt.elements().into_iter().map(|e| (e.key, e.role, e.text.unwrap_or_default(), e.label.unwrap_or_default())).collect()
}

fn has_diagram(rt: &rune::Runtime<State>) -> bool {
    drawn(rt).iter().any(|(_, role, _, label)| *role == "figure" && label == "A diagram in this reply")
}

fn shows(rt: &rune::Runtime<State>, words: &str) -> bool {
    drawn(rt).iter().any(|(_, _, text, _)| text.contains(words))
}

fn press(rt: &mut rune::Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    // Two seconds of frames, so what the press took away has finished going.
    for _ in 0..40 {
        rt.frame(0.05);
    }
}

// case: is framed as a diagram once its block has closed, its text a tab away
#[test]
fn is_framed_as_a_diagram_once_its_block_has_closed_its_text_a_tab_away() {
    let mut rt = window(FLOW, false);
    assert!(has_diagram(&rt), "{:?}", drawn(&rt));
    let d = drawn(&rt);
    assert!(d.iter().any(|(_, role, _, label)| *role == "tablist" && label == "Show the diagram or its text"), "{d:?}");
    assert!(d.iter().any(|(key, _, _, _)| key.as_deref() == Some("diagram-0/tab-diagram")), "{d:?}");
    assert!(d.iter().any(|(_, _, text, _)| text == "Diagram") && d.iter().any(|(_, _, text, _)| text == "Code"), "{d:?}");
    // The prose around it is untouched, and the diagram's text is not shown beside its picture.
    assert!(shows(&rt, "Here is how a deploy runs:") && shows(&rt, "Staging goes first."));
    assert!(!shows(&rt, "A[Push] --> B[Tests]"), "{:?}", drawn(&rt));
    // A tab away: its text, as written.
    press(&mut rt, "diagram-0/tab-code");
    assert!(shows(&rt, "A[Push] --> B[Tests] --> C[Staging]"), "{:?}", drawn(&rt));
    press(&mut rt, "diagram-0/tab-diagram");
    assert!(!shows(&rt, "A[Push] --> B[Tests]"), "{:?}", drawn(&rt));
}

// case: is text while it is still being written
#[test]
fn is_text_while_it_is_still_being_written() {
    let rt = window("Here it comes:\n\n```mermaid\ngraph LR\n  A --> B", true);
    assert!(!has_diagram(&rt), "{:?}", drawn(&rt));
    assert!(shows(&rt, "graph LR"), "{:?}", drawn(&rt));
    // Once its fence closes, it is the diagram.
    let rt = window("Here it comes:\n\n```mermaid\ngraph LR\n  A --> B\n```\n", true);
    assert!(has_diagram(&rt), "{:?}", drawn(&rt));
}

// case: leaves every other block as the code it was
#[test]
fn leaves_every_other_block_as_the_code_it_was() {
    let rt = window("```js\nconst a = 1\n```", false);
    assert!(!has_diagram(&rt), "{:?}", drawn(&rt));
    assert!(shows(&rt, "const a = 1"), "{:?}", drawn(&rt));
    // Code, not a diagram it could not draw: no caption under it.
    assert!(!shows(&rt, "Not drawn as a diagram"), "{:?}", drawn(&rt));
}

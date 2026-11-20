//! a-note-between-steps-renders-its-code.test.tsx (2 cases): the words a runtime says between its steps are drawn by
//! the reply's Markdown renderer, so a fenced block is code and no fence is left as text. In Rust those words are the
//! step's thought, drawn with `Markdown` inside the step line (lone or beside other steps).

use super::*;

const NOTE: &str = "Found the bug. In `cart.js:3`, the discount is subtracted as a flat amount:\n```javascript\nreturn subtotal - discountPercent\n```\nShould be a percentage.";

fn opened(items: Vec<ledger::Item>) -> Vec<rune::ElementInfo> {
    let mut s = State::default();
    s.activity_open.insert(format!("conv/group/{}", activity_id(&items[0])), true);
    let mut rt = rune::Runtime::new((s, items), |(s, items), _| activity_group(items, &Palette::new(true), s, "conv"), (700.0, 600.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements()
}

fn note() -> ledger::Item {
    ledger::Item::Thought { id: "n1".into(), text: NOTE.into(), duration_ms: None }
}

fn read_tool() -> ledger::Item {
    ledger::Item::Tool { name: "Read".into(), summary: String::new(), status: ledger::Status::Done,
        detail: Box::new(ledger::ToolDetail { id: "t1".into(), arguments: "cart.js".into(), finished: true, ..Default::default() }) }
}

// case: is drawn by the reply’s renderer
#[test]
fn the_note_is_drawn_by_the_replys_renderer() {
    let source = include_str!("activity_drawers.rs");
    // Electron's old splitInlineCode row (lc-filerow--said) has no Rust counterpart: the words go through Markdown.
    assert!(!source.contains("split_inline_code") && !source.contains("filerow--said"));
    assert!(source.contains("Markdown::new(without_headline(words)"));
}

// case: which turns a fenced block into code, with no fences left as text
#[test]
fn a_fenced_block_is_code_with_no_fences_left_as_text() {
    for items in [vec![note()], vec![note(), read_tool()]] {
        let els = opened(items);
        let words: Vec<&str> = els.iter().filter_map(|e| e.text.as_deref()).collect();
        assert!(!words.iter().any(|w| w.contains("```")), "{words:?}");
        let code = els.iter().find(|e| e.text.as_deref().is_some_and(|t| t.contains("return subtotal - discountPercent"))).expect("the code is drawn");
        // Its own block: the line alone, not run into the sentence around it.
        assert!(!code.text.as_deref().unwrap().contains("Should be a percentage"), "{words:?}");
        assert!(words.iter().any(|w| w.contains("Should be a percentage")), "{words:?}");
    }
}

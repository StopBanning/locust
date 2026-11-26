//! `apps/desktop/src/renderer/src/settled-text.test.ts`: text a person has read must not change shape after they read it.
//!
//! Electron gets there with `splitSettled`: while streaming, only a settled prefix is parsed and the tail is drawn plain.
//! The window does not port `splitSettled`. `reply_markdown` draws a live reply with rune-kit's `Markdown::streaming`
//! (`src/ui/reply_actions.rs`), which parses the tail as what it is about to become, so nothing settled has to wait and
//! no block changes kind under the reader. Each case below drives Electron's own text and stages through that, drawn
//! headless, and holds it to the case's promise: what was drawn keeps its kind and its words as more arrives, nothing
//! is lost, and a block is never drawn as something it is not.
use crate::{Palette, State};

thread_local! {
    static REPLY: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}

const FENCE: &str = "```";

fn join(lines: &[&str]) -> String {
    lines.join("\n")
}

/// Each drawn element (key, role, text) of `body` drawn while it is still arriving.
fn drawn(body: &str) -> Vec<(Option<String>, &'static str, String)> {
    REPLY.with(|r| *r.borrow_mut() = body.to_owned());
    let mut s = State::default();
    s.image_cache = Some(std::sync::Arc::new(rune::ImageCache::new(std::sync::Arc::new(|| {}))));
    let mut rt = rune::Runtime::new(
        s,
        |s, _| REPLY.with(|r| crate::reply_actions::live_reply_markdown_for_tests(s, &r.borrow(), &Palette::new(false))),
        (800.0, 3000.0),
        rune::Env::new(false, 1.0),
    );
    rt.frame(0.0);
    rt.elements().into_iter().map(|e| (e.key, e.role, e.text.unwrap_or_default())).collect()
}

/// The reply's top-level blocks as drawn: (kind, the words drawn in it). A code block is drawn as a group holding a
/// `code` card, so its kind is said as "code".
fn blocks(body: &str) -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = Vec::new();
    for (key, role, text) in drawn(body) {
        if key.as_deref() == Some(format!("b{}", out.len()).as_str()) {
            out.push((role, text));
            continue;
        }
        if let Some(last) = out.last_mut() {
            if role == "code" {
                last.0 = "code";
            }
            if !text.is_empty() {
                if !last.1.is_empty() {
                    last.1.push('\n');
                }
                last.1.push_str(&text);
            }
        }
    }
    out
}

fn kinds(body: &str) -> Vec<&'static str> {
    blocks(body).into_iter().map(|(kind, _)| kind).collect()
}

/// As text arrives, every block drawn before the last keeps its kind and words, and the last keeps its kind.
fn moves_forward(stages: &[String]) {
    let mut before: Vec<(&'static str, String)> = Vec::new();
    for stage in stages {
        let now = blocks(stage);
        assert!(now.len() >= before.len(), "a block went away at {stage:?}: {before:?} -> {now:?}");
        for (i, was) in before.iter().enumerate() {
            assert_eq!(now[i].0, was.0, "block {i} changed kind at {stage:?}: {before:?} -> {now:?}");
            if i + 1 < before.len() {
                assert_eq!(now[i].1, was.1, "block {i} changed its words at {stage:?}");
            }
        }
        before = now;
    }
}

fn every_prefix(whole: &str) -> Vec<String> {
    whole.char_indices().map(|(at, c)| whole[..at + c.len_utf8()].to_owned()).collect()
}

/// Every word of the text is drawn somewhere.
fn draws_every_word(text: &str) {
    let shown: String = drawn(text).into_iter().map(|(_, _, t)| t).collect::<Vec<_>>().join("\n");
    for word in text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
        assert!(shown.contains(word), "{word:?} of {text:?} is not drawn: {shown:?}");
    }
}

#[test]
fn settles_a_paragraph_at_the_blank_line_that_ends_it_and_not_before() {
    let text = join(&["First paragraph, complete.", "", "Second paragraph, still being writ"]);
    assert_eq!(
        blocks(&text),
        [("text", "First paragraph, complete.".to_owned()), ("text", "Second paragraph, still being writ".to_owned())]
    );
    // Until the blank line, the second line belongs to the first paragraph; from it on, the first stands as written.
    moves_forward(&every_prefix(&text)[text.find('\n').unwrap() + 1..]);
}

#[test]
fn leaves_everything_as_tail_while_the_first_paragraph_is_still_arriving() {
    // Electron draws it plain; the window draws it as the paragraph it is, and it stays one as it grows.
    let text = "Worth flagging upfront: Alphabet is one of";
    assert_eq!(blocks(text), [("text", text.to_owned())]);
    moves_forward(&every_prefix(text));
}

#[test]
fn never_splits_an_open_fence_whatever_blank_lines_it_holds() {
    let text = join(&["Here is the diff:", "", &format!("{FENCE}ts"), "const a = 1", "", "const b = 2"]);
    let drawn = blocks(&text);
    assert_eq!(drawn.iter().map(|b| b.0).collect::<Vec<_>>(), ["text", "code"], "{drawn:?}");
    assert!(drawn[1].1.contains("const a = 1") && drawn[1].1.contains("const b = 2"), "{drawn:?}");
    assert!(!drawn.iter().any(|b| b.1.contains(FENCE)), "the opener is drawn as words: {drawn:?}");
    let opened = text.find(FENCE).unwrap() + FENCE.len();
    moves_forward(&every_prefix(&text)[opened..]);
}

#[test]
fn settles_a_fence_the_moment_it_closes_blank_line_or_not() {
    let text = join(&[FENCE, "x", FENCE, "And then prose that is still going"]);
    assert_eq!(blocks(&text), [("code", "x".to_owned()), ("text", "And then prose that is still going".to_owned())]);
}

#[test]
fn loses_nothing_settled_plus_tail_is_the_text() {
    assert!(blocks("").is_empty());
    for text in [
        "one line".to_owned(),
        join(&["a", "", "b", "", "c"]),
        join(&["p", "", FENCE, "code", "", "more", FENCE, "", "after"]),
        join(&["p", "", &format!("{FENCE}js"), "unterminated"]),
    ] {
        draws_every_word(&text);
    }
}

#[test]
fn only_ever_moves_forward_as_text_arrives() {
    let stages = [
        "Alpha.".to_owned(),
        join(&["Alpha.", ""]),
        join(&["Alpha.", "", "Beta"]),
        join(&["Alpha.", "", "Beta is longer now."]),
        join(&["Alpha.", "", "Beta is longer now.", ""]),
        join(&["Alpha.", "", "Beta is longer now.", "", "Gamma"]),
    ];
    moves_forward(&stages);
}

#[test]
fn settles_a_table_from_its_rule_on_row_by_finished_row() {
    let text = join(&["Intro.", "", "| File | Why |", "|---|---|", "| a.ts | one |", "| b.ts | tw"]);
    let drawn = blocks(&text);
    assert_eq!(drawn.iter().map(|b| b.0).collect::<Vec<_>>(), ["text", "table"], "{drawn:?}");
    assert_eq!(drawn[1].1.lines().take(4).collect::<Vec<_>>(), ["File", "Why", "a.ts", "one"], "{drawn:?}");
}

#[test]
fn holds_a_header_row_back_until_the_rule_under_it_has_arrived() {
    for text in [join(&["Intro.", "", "| File | Why |", "|---|"]), join(&["Intro.", "", "| File | Why |", ""])] {
        // Not laid out as a paragraph of pipes first, and not a table yet.
        let drawn = blocks(&text);
        assert_eq!(drawn.first(), Some(&("text", "Intro.".to_owned())), "{text:?}: {drawn:?}");
        assert!(!drawn.iter().any(|b| b.1.contains('|')), "{text:?}: {drawn:?}");
    }
}

#[test]
fn settles_a_reply_that_opens_with_a_table() {
    let text = join(&["| A | B |", "| - | - |", "| 1 | 2 |", ""]);
    assert_eq!(blocks(&text), [("table", join(&["A", "B", "1", "2"]))]);
}

#[test]
fn settles_each_finished_list_item_and_heading_never_the_line_still_being_written() {
    let text = join(&["## Findings", "- first, done", "- second, done", "1. third, done", "- fourth is still"]);
    let drawn = blocks(&text);
    assert_eq!(drawn.first(), Some(&("heading", "Findings".to_owned())), "{drawn:?}");
    assert!(drawn[1..].iter().all(|b| b.0 == "list"), "{drawn:?}");
    let shown: String = drawn.iter().map(|b| b.1.as_str()).collect::<Vec<_>>().join("\n");
    for item in ["first, done", "second, done", "third, done", "fourth is still"] {
        assert!(shown.contains(item), "{item}: {drawn:?}");
    }
    // From the first item on, nothing drawn changes kind or words as the rest arrives.
    moves_forward(&every_prefix(&text)[text.find("\n- ").unwrap() + 3..]);
}

#[test]
fn stops_at_prose_a_paragraph_line_still_waits_for_its_blank_line() {
    let text = join(&["- an item", "A sentence after it", "that runs on"]);
    // The item stays an item as the prose under it arrives, and loses none of it.
    assert_eq!(kinds(&text), ["list"]);
    moves_forward(&every_prefix(&text)[2..]);
    draws_every_word(&text);
}

#[test]
fn settles_the_rows_after_a_closed_fence_too() {
    let text = join(&[FENCE, "x", FENCE, "- after the code", "- still writ"]);
    let drawn = blocks(&text);
    assert_eq!(drawn.iter().map(|b| b.0).collect::<Vec<_>>(), ["code", "list"], "{drawn:?}");
    assert!(drawn[1].1.contains("after the code") && drawn[1].1.contains("still writ"), "{drawn:?}");
    let closed = text.rfind(FENCE).unwrap() + FENCE.len();
    moves_forward(&every_prefix(&text)[closed..]);
}

#[test]
fn only_moves_forward_as_a_table_arrives_a_character_at_a_time() {
    let whole = join(&["Intro.", "", "| A | B |", "|---|---|", "| 1 | 2 |", "| 3 | 4 |", "", "After."]);
    moves_forward(&every_prefix(&whole));
    assert_eq!(kinds(&whole), ["text", "table", "text"]);
    draws_every_word(&whole);
}

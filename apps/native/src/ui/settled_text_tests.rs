//! settled-text.test.ts (13 cases): text a person has read must not change shape after they read it.
//!
//! Electron gets there by splitting a live reply (`splitSettled`): the settled prefix is parsed as Markdown and the
//! tail is drawn plain until it settles. The window does not split: `reply_markdown` draws a live reply with rune-kit's
//! `Markdown::streaming`, which parses the tail as what it is about to become, so no block changes kind under the
//! reader. Each case here takes Electron's own text, draws it headless through `reply_markdown` as live, and holds
//! Electron's claim the window's way: what Electron calls settled is drawn now exactly as the finished reply draws it,
//! nothing is lost, an open fence is one code block, and as the text grows no drawn block ever changes kind. The tail
//! Electron leaves plain is drawn as what it becomes (a table row as a row, an item as an item), never as raw markup.
use crate::{Palette, State};

const FENCE: &str = "```";

fn join(lines: &[&str]) -> String {
    lines.join("\n")
}

/// One drawn block of the reply: its kind (the block's role, or what a framed block holds) and every text under it.
#[derive(Clone, Debug, PartialEq)]
struct Drawn {
    kind: &'static str,
    texts: Vec<String>,
}

/// The reply drawn headless by `reply_markdown`, live (still arriving) or finished: one entry per Markdown block.
fn draw(text: &str, live: bool) -> Vec<Drawn> {
    let mut s = State::default();
    s.image_cache = Some(std::sync::Arc::new(rune::ImageCache::new(std::sync::Arc::new(|| {}))));
    let mut rt = rune::Runtime::new(
        (s, text.to_owned(), live),
        |(s, body, live), _| {
            let p = Palette::new(false);
            if *live { super::live_reply_markdown_for_tests(s, body, &p) } else { super::reply_markdown_for_tests(s, body, &p) }
        },
        (700.0, 3000.0),
        rune::Env::new(false, 1.0),
    );
    rt.frame(0.0);
    let els = rt.elements();
    let is_block = |path: &str| path.strip_prefix("#0/#0/b").is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
    els.iter()
        .filter(|e| is_block(&e.path))
        .map(|block| {
            let under: Vec<_> = els.iter().filter(|e| e.path == block.path || e.path.starts_with(&format!("{}/", block.path))).collect();
            let kind = match block.role {
                "group" | "" => ["code", "table"].into_iter().find(|k| under.iter().any(|e| e.role == *k)).unwrap_or("group"),
                role => role,
            };
            let texts = under
                .iter()
                .filter(|e| e.role == "text" || e.path == block.path)
                .filter_map(|e| e.text.clone())
                .filter(|t| !t.is_empty() && t != "•")
                .collect();
            Drawn { kind, texts }
        })
        .collect()
}

fn live(text: &str) -> Vec<Drawn> {
    draw(text, true)
}

fn finished(text: &str) -> Vec<Drawn> {
    draw(text, false)
}

fn kinds(blocks: &[Drawn]) -> Vec<&'static str> {
    blocks.iter().map(|b| b.kind).collect()
}

/// What Electron settles is drawn now as the finished reply draws it: the live reply starts with those very blocks.
/// The last of them may be one the tail joins (a list gaining the item still arriving): it keeps its kind and its words.
fn settled_part_holds(text: &str, settled: &str) {
    let now = live(text);
    let done = finished(settled);
    assert!(!done.is_empty() && now.len() >= done.len(), "{settled:?} drew {done:?}; live {text:?} drew {now:?}");
    let last = done.len() - 1;
    assert_eq!(now[..last], done[..last], "the settled part of {text:?}");
    assert_eq!(now[last].kind, done[last].kind, "the settled part of {text:?}");
    assert!(now[last].texts.starts_with(&done[last].texts), "{text:?}: {:?} then {:?}", done[last].texts, now[last].texts);
}

/// Every word of `text` (not the fence markers or a fence's language) is on screen.
fn loses_nothing(text: &str) {
    let shown = live(text).iter().flat_map(|b| b.texts.clone()).collect::<Vec<_>>().join("\n");
    for line in text.lines().filter(|l| !l.trim_start().starts_with(FENCE)) {
        for word in line.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
            assert!(shown.contains(word), "{word:?} of {text:?} is not drawn: {shown:?}");
        }
    }
}

/// Grown one character at a time to `whole`, no drawn block ever changes kind, and a block above the one still
/// arriving is never redrawn differently: what is settled stays settled.
fn only_moves_forward(stages: &[String]) {
    let end = live(stages.last().expect("a stage"));
    let whole = kinds(&end);
    // Pipes are prose only where the finished text is prose with pipes in it.
    let piped_prose = end.iter().any(|b| b.kind == "text" && b.texts.iter().any(|t| t.contains('|')));
    let mut before: Vec<Drawn> = Vec::new();
    for stage in stages {
        let now = live(stage);
        for (i, block) in now.iter().enumerate() {
            assert_eq!(Some(&block.kind), whole.get(i), "block {i} of {stage:?} is {:?}, it ends as {whole:?}", block.kind);
            let pipes = block.kind == "text" && block.texts.iter().any(|t| t.contains('|'));
            assert!(piped_prose || !pipes, "pipes drawn as prose in {stage:?}");
        }
        // Every block but the last one drawn before is drawn the same now.
        let kept = before.len().saturating_sub(1);
        assert!(now.len() >= kept && now[..kept] == before[..kept], "a settled block changed: {before:?} -> {now:?} at {stage:?}");
        before = now;
    }
}

fn by_character(whole: &str) -> Vec<String> {
    (1..=whole.len()).filter(|&i| whole.is_char_boundary(i)).map(|i| whole[..i].to_owned()).collect()
}

// what has settled in a streaming reply

#[test]
fn settles_a_paragraph_at_the_blank_line_that_ends_it_and_not_before() {
    let text = join(&["First paragraph, complete.", "", "Second paragraph, still being writ"]);
    settled_part_holds(&text, "First paragraph, complete.");
    let now = live(&text);
    assert_eq!(kinds(&now), ["text", "text"]);
    assert_eq!(now[1].texts, ["Second paragraph, still being writ"]);
    // Before the blank line it is one paragraph still growing, not two.
    assert_eq!(kinds(&live(&join(&["First paragraph, complete.", "Second"]))), ["text"]);
}

#[test]
fn leaves_everything_as_tail_while_the_first_paragraph_is_still_arriving() {
    // Electron: nothing settled, all tail. The window draws it as the paragraph it is, with no markup to flip.
    let text = "Worth flagging upfront: Alphabet is one of";
    assert_eq!(live(text), finished(text));
    assert_eq!(live(text), [Drawn { kind: "text", texts: vec![text.to_owned()] }]);
}

#[test]
fn never_splits_an_open_fence_whatever_blank_lines_it_holds() {
    let text = join(&["Here is the diff:", "", &format!("{FENCE}ts"), "const a = 1", "", "const b = 2"]);
    settled_part_holds(&text, &join(&["Here is the diff:", ""]));
    let now = live(&text);
    assert_eq!(kinds(&now), ["text", "code"], "{now:?}");
    let code = now[1].texts.join("\n");
    assert!(code.contains("const a = 1") && code.contains("const b = 2"), "{code:?}");
    assert!(!now.iter().any(|b| b.texts.iter().any(|t| t.contains(FENCE))), "the opener never reads as three backticks");
}

#[test]
fn settles_a_fence_the_moment_it_closes_blank_line_or_not() {
    let text = join(&[FENCE, "x", FENCE, "And then prose that is still going"]);
    settled_part_holds(&text, &join(&[FENCE, "x", FENCE]));
    let now = live(&text);
    assert_eq!(kinds(&now), ["code", "text"]);
    assert_eq!(now[1].texts, ["And then prose that is still going"]);
}

#[test]
fn loses_nothing_settled_plus_tail_is_the_text() {
    for text in [
        String::new(),
        "one line".to_owned(),
        join(&["a", "", "b", "", "c"]),
        join(&["p", "", FENCE, "code", "", "more", FENCE, "", "after"]),
        join(&["p", "", &format!("{FENCE}js"), "unterminated"]),
    ] {
        loses_nothing(&text);
    }
}

#[test]
fn only_ever_moves_forward_as_text_arrives() {
    let stages: Vec<String> = [
        &["Alpha."][..],
        &["Alpha.", ""],
        &["Alpha.", "", "Beta"],
        &["Alpha.", "", "Beta is longer now."],
        &["Alpha.", "", "Beta is longer now.", ""],
        &["Alpha.", "", "Beta is longer now.", "", "Gamma"],
    ]
    .iter()
    .map(|lines| join(lines))
    .collect();
    only_moves_forward(&stages);
}

// a table, a list and headings settle a line at a time

#[test]
fn settles_a_table_from_its_rule_on_row_by_finished_row() {
    let text = join(&["Intro.", "", "| File | Why |", "|---|---|", "| a.ts | one |", "| b.ts | tw"]);
    let settled = join(&["Intro.", "", "| File | Why |", "|---|---|", "| a.ts | one |"]);
    let now = live(&text);
    assert_eq!(kinds(&now), ["text", "table"], "{now:?}");
    // The finished rows are the settled table's; the row still arriving is drawn as a row of it, never as pipes.
    let done = finished(&settled);
    assert_eq!(now[0], done[0]);
    assert!(now[1].texts.starts_with(&done[1].texts), "{:?} then {:?}", done[1].texts, now[1].texts);
    assert_eq!(done[1].texts, ["File", "Why", "a.ts", "one"]);
    assert_eq!(now[1].texts[4..], ["b.ts", "tw"]);
}

#[test]
fn holds_a_header_row_back_until_the_rule_under_it_has_arrived() {
    // Electron settles "Intro." alone in both. A header row with nothing under it yet: the window holds it back too,
    // whole or still arriving, so a line that turns out to be prose never draws as a table first.
    for text in [join(&["Intro.", "", "| File | Why |", ""]), join(&["Intro.", "", "| File | Why |"]), join(&["Intro.", "", "| File | Wh"])] {
        assert_eq!(live(&text), finished("Intro."), "{text:?}");
    }
    only_moves_forward(&by_character(&join(&["Intro.", "", "| File | Why |", "is prose after all"])));
    // Once its rule has begun to arrive, the window draws the header as the table it is becoming, never as pipes in a
    // paragraph that then turns into a table: "Intro." is untouched and the header is a table's head, with no rows.
    let ruling = live(&join(&["Intro.", "", "| File | Why |", "|---|"]));
    assert_eq!(ruling[..1], finished("Intro.")[..]);
    assert_eq!(ruling[1..], [Drawn { kind: "table", texts: vec!["File".into(), "Why".into()] }]);
    only_moves_forward(&by_character(&join(&["Intro.", "", "| File | Why |", "|---|---|"])));
}

#[test]
fn settles_a_reply_that_opens_with_a_table() {
    let text = join(&["| A | B |", "| - | - |", "| 1 | 2 |", ""]);
    settled_part_holds(&text, &join(&["| A | B |", "| - | - |", "| 1 | 2 |"]));
    assert_eq!(live(&text), [Drawn { kind: "table", texts: ["A", "B", "1", "2"].map(str::to_owned).to_vec() }]);
}

#[test]
fn settles_each_finished_list_item_and_heading_never_the_line_still_being_written() {
    let text = join(&["## Findings", "- first, done", "- second, done", "1. third, done", "- fourth is still"]);
    settled_part_holds(&text, &join(&["## Findings", "- first, done", "- second, done", "1. third, done"]));
    let now = live(&text);
    assert_eq!(now[0], Drawn { kind: "heading", texts: vec!["Findings".into()] });
    // The line still being written is drawn as the item it is becoming, under its own marker.
    let last = now.last().expect("a block");
    assert_eq!(last.kind, "list");
    assert_eq!(last.texts.last().map(String::as_str), Some("fourth is still"));
}

#[test]
fn stops_at_prose_a_paragraph_line_still_waits_for_its_blank_line() {
    // Electron settles "- an item" and holds the prose after it. The item is drawn as the finished reply draws it; the
    // prose still arriving is its own paragraph under it (the window's parser takes no lazy line), drawn as it ends.
    let text = join(&["- an item", "A sentence after it", "that runs on"]);
    settled_part_holds(&text, "- an item");
    let now = live(&text);
    assert_eq!(now, finished(&text));
    assert_eq!(now[0], Drawn { kind: "list", texts: vec!["an item".into()] });
    assert_eq!(kinds(&now), ["list", "text"]);
    only_moves_forward(&by_character(&text));
}

#[test]
fn settles_the_rows_after_a_closed_fence_too() {
    let text = join(&[FENCE, "x", FENCE, "- after the code", "- still writ"]);
    settled_part_holds(&text, &join(&[FENCE, "x", FENCE, "- after the code"]));
    assert_eq!(kinds(&live(&text)), ["code", "list"]);
}

#[test]
fn only_moves_forward_as_a_table_arrives_a_character_at_a_time() {
    let whole = join(&["Intro.", "", "| A | B |", "|---|---|", "| 1 | 2 |", "| 3 | 4 |", "", "After."]);
    assert_eq!(kinds(&finished(&whole)), ["text", "table", "text"]);
    only_moves_forward(&by_character(&whole));
    for stage in by_character(&whole) {
        loses_nothing_but_held(&stage);
    }
}

/// Every finished line's words are on screen (a `|` line still arriving may be held until it is one).
fn loses_nothing_but_held(stage: &str) {
    let (done, _arriving) = stage.rsplit_once('\n').unwrap_or(("", stage));
    let held_header = |line: &str| line.trim_start().starts_with('|');
    let shown = live(stage).iter().flat_map(|b| b.texts.clone()).collect::<Vec<_>>().join("\n");
    let lines: Vec<&str> = done.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        // A header is held until the rule under it has arrived whole.
        if held_header(line) && lines.get(i + 1).is_none_or(|next| !next.contains("---|---|")) {
            continue;
        }
        for word in line.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()) {
            assert!(shown.contains(word), "{word:?} of {stage:?} is not drawn: {shown:?}");
        }
    }
}


#[test]
fn a_header_whose_rule_is_written_with_spaces_is_a_table_as_soon_as_the_rule_begins() {
    // settledText.ts TABLE_RULE: `|?[ \t]*:?-+:?[ \t]*(\|[ \t]*:?-+:?[ \t]*)*|?[ \t]*`: spaces around the dashes are the rule's.
    let drawn = live("| name | qty |\n| --- | --- |");
    assert_eq!(drawn.iter().map(|d| d.kind).collect::<Vec<_>>(), ["table"], "{drawn:?}");
    assert_eq!(drawn, draw("| name | qty |\n| --- | --- |", false));
}

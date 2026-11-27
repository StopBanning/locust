//! A folded tool row reads as Electron's (ActivityCard.tsx `.lc-steps__line`): its words, then the arrow after them,
//! the line only as wide as what it says, and the words brighter under the pointer
//! (`.lc-steps__line:hover { color: var(--lc-text-secondary) }`). Found by name in the rendered window.

use super::*;

fn thread() -> rune::Runtime<State> {
    let mut s = State::default();
    crate::scenes::ledger_transcript_scene(&mut s);
    let mut rt = rune::Runtime::new(s, |s, cx| crate::view(s, cx), (1215.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.016);
    rt
}

/// The tool row's words, its arrow and the line holding both.
fn fold(rt: &rune::Runtime<State>) -> (rune::ElementInfo, rune::ElementInfo, rune::ElementInfo) {
    let elements = rt.elements();
    let words = elements
        .iter()
        .find(|e| e.key.as_deref().is_some_and(|k| k.starts_with("fold-words/")) && e.text.as_deref().is_some_and(|t| t.contains("inspect")))
        .unwrap_or_else(|| panic!("no folded tool row: {:?}", elements.iter().filter_map(|e| e.key.clone()).filter(|k| k.contains("fold")).collect::<Vec<_>>()))
        .clone();
    let stem = words.key.as_deref().unwrap().trim_start_matches("fold-words/").to_owned();
    let find = |key: String| elements.iter().find(|e| e.key.as_deref() == Some(key.as_str())).cloned().unwrap_or_else(|| panic!("no {key}"));
    (words, find(format!("fold-arrow/{stem}")), find(format!("fold/{stem}")))
}

#[test]
fn a_folded_tool_row_says_its_words_then_the_arrow() {
    let rt = thread();
    let (words, arrow, line) = fold(&rt);
    let words_end = words.rect[0] + words.rect[2];
    assert!(words.rect[0] < arrow.rect[0], "the words lead: words at {}, arrow at {}", words.rect[0], arrow.rect[0]);
    // `gap: var(--lc-space-3)`, 6: the arrow right after the words, not pushed to the far edge.
    assert!((arrow.rect[0] - words_end - 6.0).abs() < 1.0, "the arrow sits 6 after the words: {} vs {}", arrow.rect[0], words_end);
    assert_eq!(arrow.rect[2], 12.0, "the arrow is 12 (`<Icon name=chevron-right size={{12}} />`)");
    // `display: inline-flex`: the line is as wide as what it says, not the column.
    assert!((line.rect[2] - (words.rect[2] + 6.0 + arrow.rect[2])).abs() < 1.0, "the line hugs its words: {:?}", line.rect);
}

#[test]
fn a_folded_tool_row_answers_the_pointer() {
    let mut rt = thread();
    let (_, _, line) = fold(&rt);
    assert_eq!(line.hover, 0.0, "nothing lit at rest");
    rt.pointer_move(line.rect[0] + line.rect[2] / 2.0, line.rect[1] + line.rect[3] / 2.0);
    for t in 1..20 {
        rt.frame(0.016 + t as f64 * 0.016);
    }
    let (_, _, line) = fold(&rt);
    // The line is a hover group, so its words and arrow take the secondary ink under the pointer (`group_color`;
    // the elements list reports the resting ink, so the lit colour is proven by the pointer picture).
    assert!(line.hover > 0.5, "the line is hovered under the pointer: {}", line.hover);
}

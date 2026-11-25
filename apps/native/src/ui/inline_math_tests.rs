//! Math 1: inline math stays in its sentence on the same line as the surrounding words, wraps
//! around the formula like a word, and leaves display math as its own block and money/code as prose.
use super::*;
use rune::{Env, Runtime};
use std::sync::Arc;

fn draw_reply(body: &str, width: f32) -> Vec<rune::ElementInfo> {
    let mut s = State::default();
    s.image_cache = Some(Arc::new(rune::ImageCache::new(Arc::new(|| {}))));
    let mut rt = Runtime::new(
        (s, body.to_owned()),
        |(s, text), _| reply_markdown_for_tests(s, text, &Palette::new(true)),
        (width, 1200.0),
        Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.frame(0.016);
    rt.elements()
}

fn mid_y(r: [f32; 4]) -> f32 {
    r[1] + r[3] * 0.5
}

#[test]
fn inline_math_stays_on_one_line_with_its_sentence_in_math_scene() {
    // case: draws $...$ inside a sentence as inline math
    let body = "The first coefficient is $c_1 = y$, and the whole system is\n\n$$\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}$$\n\nMoney like $5 and $10 stays prose.";
    let els = draw_reply(body, 700.0);

    let before = els
        .iter()
        .find(|e| e.text.as_deref().is_some_and(|t| t.contains("The first coefficient is")))
        .expect("opening prose");
    let formula = els
        .iter()
        .find(|e| e.role == "image" && e.label.as_deref() == Some("c_1 = y"))
        .expect("inline formula image");
    let comma = els
        .iter()
        .find(|e| e.text.as_deref() == Some(","))
        .expect("comma attached to formula");
    let after = els
        .iter()
        .find(|e| e.text.as_deref().is_some_and(|t| t.contains("and the whole system is")))
        .expect("closing prose");
    let display = els
        .iter()
        .find(|e| e.role == "image" && e.label.as_deref().is_some_and(|l| l.contains("pmatrix")))
        .expect("display math image");

    // All three parts of the sentence + comma share ONE horizontal line (vertical midpoints align).
    assert!(
        (mid_y(before.rect) - mid_y(formula.rect)).abs() < 6.0,
        "formula must sit on the same line as opening prose: before={:?}, formula={:?}",
        before.rect,
        formula.rect
    );
    assert!(
        (mid_y(formula.rect) - mid_y(comma.rect)).abs() < 6.0,
        "comma must stay on the same line as formula: formula={:?}, comma={:?}",
        formula.rect,
        comma.rect
    );
    assert!(
        (mid_y(before.rect) - mid_y(after.rect)).abs() < 6.0,
        "closing prose must stay on the same line: before={:?}, after={:?}",
        before.rect,
        after.rect
    );
    // Horizontal order: before -> formula -> comma -> after.
    assert!(
        before.rect[0] + before.rect[2] <= formula.rect[0] + 1.0,
        "formula must follow opening prose horizontally: before={:?}, formula={:?}",
        before.rect,
        formula.rect
    );
    assert!(
        formula.rect[0] + formula.rect[2] <= after.rect[0] + 1.0,
        "closing prose must follow formula horizontally: formula={:?}, after={:?}",
        formula.rect,
        after.rect
    );
    // Display math stays on its own block below the sentence.
    assert!(
        display.rect[1] > before.rect[1] + before.rect[3],
        "display math must stay a block below the sentence: sentence={:?}, display={:?}",
        before.rect,
        display.rect
    );
}

#[test]
fn wrapped_sentence_keeps_inline_formula_with_its_neighbouring_word() {
    // case: draws $...$ inside a sentence as inline math
    let body = "When solving a linear system of three equations in three unknowns, the first coefficient is $c_1 = y$, and the remaining coefficients follow by back substitution.";
    let els = draw_reply(body, 360.0);

    let formula = els
        .iter()
        .find(|e| e.role == "image" && e.label.as_deref() == Some("c_1 = y"))
        .expect("inline formula image");
    let comma = els
        .iter()
        .find(|e| e.text.as_deref() == Some(","))
        .expect("comma attached to formula");
    let words: Vec<_> = els
        .iter()
        .filter(|e| e.text.as_deref().is_some_and(|t| !t.trim().is_empty() && t != "Copy" && t != ","))
        .collect();

    // Sentence actually wrapped across multiple lines.
    let min_y = words.iter().map(|w| w.rect[1]).fold(f32::INFINITY, f32::min);
    let max_y = words.iter().map(|w| w.rect[1]).fold(f32::NEG_INFINITY, f32::max);
    assert!(
        max_y - min_y > 16.0,
        "sentence should wrap across multiple lines at 360px: min_y={min_y}, max_y={max_y}"
    );

    // Formula stays glued to its trailing comma and shares a line with at least one neighbouring word.
    assert!(
        (mid_y(formula.rect) - mid_y(comma.rect)).abs() < 6.0,
        "comma must stay on the formula's line: formula={:?}, comma={:?}",
        formula.rect,
        comma.rect
    );
    let same_line_word = words
        .iter()
        .find(|w| (mid_y(w.rect) - mid_y(formula.rect)).abs() < 6.0);
    assert!(
        same_line_word.is_some(),
        "wrapped inline formula must share its line with a neighbouring word: formula={:?}, words={:?}",
        formula.rect,
        words.iter().map(|w| (&w.text, w.rect)).collect::<Vec<_>>()
    );
}

#[test]
fn paren_inline_math_stays_inline_while_money_and_code_stay_prose() {
    // case: leaves money as money
    let body = "Costs $5 and $10, code `$x^2$` stays text, while \\(a + b = c\\) sits inline.";
    let els = draw_reply(body, 700.0);

    let formula = els
        .iter()
        .find(|e| e.role == "image" && e.label.as_deref() == Some("a + b = c"))
        .expect("paren inline formula image");
    let imgs: Vec<_> = els.iter().filter(|e| e.role == "image").collect();
    assert_eq!(imgs.len(), 1, "only \\(a + b = c\\) should be an image, got {imgs:?}");

    let all_text: String = els
        .iter()
        .filter_map(|e| e.text.as_deref())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(all_text.contains("$5") && all_text.contains("$10"), "{all_text}");
    assert!(all_text.contains("$x^2$"), "{all_text}");

    let sits = els
        .iter()
        .find(|e| e.text.as_deref().is_some_and(|t| t.contains("sits inline")))
        .expect("trailing prose");
    assert!(
        (mid_y(formula.rect) - mid_y(sits.rect)).abs() < 6.0,
        "\\(...\\) formula must sit on the same line as surrounding words: formula={:?}, sits={:?}",
        formula.rect,
        sits.rect
    );
}

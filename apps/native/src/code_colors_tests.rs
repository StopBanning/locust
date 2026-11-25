//! Electron's `code-in-a-reply-is-coloured.test.tsx` (cases 1, 3, 4, 5 on `reply_actions::highlight_reply_code`
//! and `rune_kit::syntax::highlight`), with Electron's exact values.
//! Note: `rune_kit::syntax` does not carry `ps1` or `c++` fences, unlike Electron's Shiki.
use crate::reply_actions::{highlight_reply_code, language_of_path};

// case: knows an opened file’s language by its name
#[test]
fn reply_actions_knows_an_opened_files_language_by_its_name() {
    assert_eq!(language_of_path(r"C:\work\shop\cart.py").as_deref(), Some("py"));
    assert_eq!(language_of_path("src/App.TSX").as_deref(), Some("tsx"));
    assert_eq!(language_of_path("deploy/Dockerfile").as_deref(), Some("dockerfile"));
    assert_eq!(language_of_path("notes.txt"), None);
    assert_eq!(language_of_path(".env"), None);
    assert_eq!(language_of_path(".ts"), None);
    assert_eq!(language_of_path("README"), None);
}

#[test]
fn knows_a_fence_by_the_words_people_write_on_it() {
    assert!(!highlight_reply_code(false, Some("ts"), "const x = 1;").is_empty());
    assert!(!highlight_reply_code(false, Some("Python"), "def f(): pass").is_empty());
    assert!(!highlight_reply_code(false, Some("bash"), "echo hi").is_empty());
    assert!(highlight_reply_code(false, Some("brainfudge"), "++++").is_empty());
    assert!(highlight_reply_code(false, None, "const x = 1;").is_empty());
}

#[test]
fn is_coloured_and_keeps_every_character() {
    let code = "const total = items.reduce((sum, item) => sum + item.price, 0) // the cart\nexport default \"done\"";
    let spans = highlight_reply_code(false, Some("ts"), code);
    assert!(!spans.is_empty());
    for (range, _) in spans {
        assert!(code.get(range).is_some());
    }
}

#[test]
fn stays_plain_in_a_language_it_does_not_carry_or_when_too_large() {
    assert!(highlight_reply_code(false, Some("brainfudge"), "++++[>++<-]").is_empty());
    let too_large = "x".repeat(60_001);
    assert!(highlight_reply_code(false, Some("ts"), &too_large).is_empty());
}

// case: draws plain first, and coloured at once when it was coloured before; never while it is being written
#[test]
fn reply_actions_never_colours_while_it_is_being_written() {
    let code = "def total(items):\n    return sum(items)";
    assert!(highlight_reply_code(true, Some("py"), code).is_empty());
    let spans = highlight_reply_code(false, Some("py"), code);
    assert!(!spans.is_empty());
    assert_eq!(highlight_reply_code(false, Some("py"), code), spans);
}

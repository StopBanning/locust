//! The file rows under a reply (`reply_markdown`): one per inline code span Electron treats as a path
//! (`looksLikeFilePath` in shared/file-menu.ts, which ThreadItems.tsx asks before it gives a code span its file menu).
//! Seen in the live window, 28 Nov 2026: a reply that quoted `1.0`, `s.frame_clock`, `s.frame_clock = now` and a git
//! status line drew each as a green file row, and pressing one opened the viewer on a file named `s.frame_clock`.
use super::*;

/// The texts the reply draws as elements of their own.
fn drawn(body: &str) -> Vec<String> {
    let mut rt = rune::Runtime::new(
        body.to_owned(),
        |body, _| reply(&State::default(), body, false, &Palette::new(true)),
        (900.0, 900.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect()
}

#[test]
fn only_a_code_span_that_names_a_path_gets_a_file_row_under_the_reply() {
    let body = "Not done: `1.0`, `s.frame_clock`, `s.frame_clock = now`, `M src/picture_tests.rs` and \
                `working_shimmer.rs`; the clock is set at `src/main.rs:2300` and in `apps/native/src/ui/messages.rs`.";
    let texts = drawn(body);
    for path in ["src/main.rs:2300", "apps/native/src/ui/messages.rs"] {
        assert_eq!(texts.iter().filter(|t| t.as_str() == path).count(), 1, "one file row for {path}: {texts:?}");
    }
    for not_a_path in ["1.0", "s.frame_clock", "s.frame_clock = now", "M src/picture_tests.rs", "working_shimmer.rs"] {
        assert!(!texts.iter().any(|t| t.as_str() == not_a_path), "no file row for {not_a_path:?}: {texts:?}");
    }
}

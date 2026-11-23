//! `a-refusal-offers-the-mode-that-works.test.ts` (2 cases): a run its mode refused is read as a refusal from the
//! sentence the OpenCode reader writes (`runtime/opencode_events.rs`), and nothing else is. The window's side -- no
//! Run it again that would hit the same boundary for ever -- is in src/ui/mode_refusal_tests.rs.
use crate::thread_view::mode_refused_a_tool;

// case: reads a mode refusal as a refusal
#[test]
fn reads_a_mode_refusal_as_a_refusal() {
    assert!(mode_refused_a_tool(Some(
        "The mode this run is in does not allow bash, so OpenCode stopped when it tried to use it. Nothing was changed."
    )));
    for tool in ["edit", "write", "patch"] {
        assert!(mode_refused_a_tool(Some(&format!("The mode this run is in does not allow {tool}, so OpenCode stopped."))), "{tool}");
    }
    // Said with the space a card may carry around it.
    assert!(mode_refused_a_tool(Some("  The mode this run is in does not allow edit, so OpenCode stopped.\n")));
}

// case: reads everything else as something that actually went wrong
#[test]
fn reads_everything_else_as_something_that_actually_went_wrong() {
    assert!(!mode_refused_a_tool(None));
    assert!(!mode_refused_a_tool(Some("OpenCode exited with code 1.")));
    assert!(!mode_refused_a_tool(Some("OpenCode ended without a step that reported it had stopped.")));
    // The confined-workspace refusal is a different case with its own sentence: the folder boundary holds in every mode.
    assert!(!mode_refused_a_tool(Some("OpenCode asked for C:/other, which is outside the folder this run may use, and stopped.")));
    // A tool the modes do not gate, and the phrase anywhere but at the start, are not this.
    assert!(!mode_refused_a_tool(Some("The mode this run is in does not allow webfetch, so OpenCode stopped.")));
    assert!(!mode_refused_a_tool(Some("It said: The mode this run is in does not allow bash.")));
    assert!(!mode_refused_a_tool(Some("The mode this run is in does not allow bashful things.")));
}

/// The adapter's own sentence and the predicate name the same phrase: reword one and this fails.
#[test]
fn the_opencode_reader_writes_the_sentence_the_predicate_reads() {
    let source = include_str!("runtime/opencode_events.rs");
    assert!(source.contains("\"The mode this run is in does not allow {tool}, so OpenCode stopped when it tried to use it.{}\""));
}

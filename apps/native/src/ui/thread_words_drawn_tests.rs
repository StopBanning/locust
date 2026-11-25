//! The thread's words as the window draws them: a helper's summary as words
//! (`renderer/src/a-helpers-summary-reads-as-words.test.ts`, 0.625) and the steps line's hover title
//! (`renderer/src/a-turns-steps-read-like-claude-code.test.ts`, 0.604). The rules are `locust_core::thread_words` and
//! `locust_core::steps_line`; these draw `activity_group` headless and read what it put on screen.
use super::*;
use locust_core::activity_entries::Patch;

const LONG: &str = "a-full-rule-store-keeps-every-rule.test.ts";

fn drawn(items: Vec<ledger::Item>, open: &[&str]) -> Vec<rune::ElementInfo> {
    let mut s = State::default();
    for key in open {
        s.activity_open.insert((*key).into(), true);
    }
    let mut rt = rune::Runtime::new(
        (s, items),
        |(s, items), _| activity_group(items, &Palette::new(true), s, "conv"),
        (900.0, 400.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.elements()
}

fn words(all: &[rune::ElementInfo]) -> String {
    all.iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n")
}

fn edited(id: &str, name: &str) -> ledger::Item {
    let text = format!("diff --git a/{name} b/{name}\n--- a/{name}\n+++ b/{name}\n@@ -1 +1 @@\n-a\n+b\n");
    ledger::Item::Tool {
        name: "write".into(),
        status: ledger::Status::Done,
        summary: name.into(),
        detail: Box::new(ledger::ToolDetail {
            id: id.into(),
            arguments: name.into(),
            finished: true,
            patch: Some(Patch { text, added: 1, removed: 1, truncated: false }),
            ..Default::default()
        }),
    }
}

// case: is what the helper row says when it reported back
#[test]
fn the_helper_row_the_window_draws_says_its_summary_as_words() {
    let helper = ledger::Item::Tool {
        name: "Agent".into(),
        status: ledger::Status::Done,
        summary: "Count the files".into(),
        detail: Box::new(ledger::ToolDetail {
            id: "h1".into(),
            helper: true,
            finished: true,
            output: "**33** files total in the `current`\nworking directory".into(),
            ..Default::default()
        }),
    };
    let said = words(&drawn(vec![helper], &["conv/group/h1"]));
    assert!(said.contains("reported back \u{b7} 33 files total in the current working directory"), "{said}");
    assert!(!said.contains("**"), "no Markdown marks on the row: {said}");
}

// case: offers shorter forms: names give way to counts, then the last phrases to "and N more"
#[test]
fn the_steps_line_the_window_draws_says_every_name_whole_on_hover() {
    let all = drawn(vec![edited("e1", LONG)], &[]);
    // The row cuts the long name in the middle; the hover says it whole.
    assert!(words(&all).contains("Edited a-full-rule-stor\u{2026}very-rule.test.ts"), "{}", words(&all));
    let hover: Vec<&str> = all.iter().filter_map(|e| e.help.as_deref()).collect();
    assert_eq!(hover, [format!("Edited {LONG}").as_str()], "{}", words(&all));
}

#[test]
fn a_steps_line_that_already_says_every_name_has_no_hover() {
    let all = drawn(vec![edited("e1", "short.ts")], &[]);
    assert!(words(&all).contains("Edited short.ts"), "{}", words(&all));
    assert!(all.iter().all(|e| e.help.is_none()), "nothing more to say on hover");
}

//! a-codex-sandbox-that-did-not-start-says-why.test.tsx, the row: a command Codex's Windows sandbox never started says
//! so on its row, the whole sentence on hover and above its output when opened, where it said only "failed".
use super::*;
use locust_core::codex_failure::SANDBOX_DID_NOT_START;

const SAID: &str = "Failed to create unified exec process: helper_unknown_error: setup refresh had errors";

fn command(output: &str, exit_code: Option<i64>, status: ledger::Status) -> ledger::Item {
    ledger::Item::Tool {
        name: "shell".into(),
        summary: String::new(),
        detail: Box::new(ledger::ToolDetail { id: "c1".into(), arguments: "node --version".into(), output: output.into(), exit_code, finished: true, ..Default::default() }),
        status,
    }
}

/// Every word and every label on the row, closed or opened.
fn row(item: ledger::Item, open: bool) -> Vec<String> {
    row_hovered(item, open, false)
}
fn row_hovered(item: ledger::Item, open: bool, hovered: bool) -> Vec<String> {
    let mut s = State::default();
    if open {
        s.activity_open.insert(activity_drawer_key("conv", &item), true);
    }
    if hovered {
        s.hover.set(&format!("{}/sandbox", activity_drawer_key("conv", &item)), true);
    }
    let mut rt = rune::Runtime::new((s, item), |(s, item), _| activity_item(item, &Palette::new(true), s, "conv"), (700.0, 400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().flat_map(|e| [e.text, e.label]).flatten().collect()
}

// case: says why on its row, and what to do
#[test]
fn a_command_the_sandbox_never_started_says_why_on_its_row_and_what_to_do() {
    let closed = row(command(SAID, None, ledger::Status::Failed), false);
    assert!(closed.iter().any(|w| w == "sandbox did not start"), "{closed:?}");
    // Closed, the badge carries the sentence; the sentence is not drawn as words until the row is opened.
    assert!(closed.iter().any(|w| w == SANDBOX_DID_NOT_START), "{closed:?}");
    assert_eq!(closed.iter().filter(|w| w.as_str() == SANDBOX_DID_NOT_START).count(), 1, "{closed:?}");
    // Resting on the badge shows the sentence (the hover tip).
    let hovered = row_hovered(command(SAID, None, ledger::Status::Failed), false, true);
    assert!(hovered.iter().filter(|w| w.as_str() == SANDBOX_DID_NOT_START).count() > 1, "{hovered:?}");
    let opened = row(command(SAID, None, ledger::Status::Failed), true);
    let why = opened.iter().position(|w| w == SANDBOX_DID_NOT_START && !w.is_empty()).expect("the sentence");
    let printed = opened.iter().rposition(|w| w.contains("helper_unknown_error")).expect("Codex's own words");
    assert!(why < printed, "the sentence comes above the output: {opened:?}");
    assert_eq!(opened.iter().filter(|w| w.as_str() == SANDBOX_DID_NOT_START).count(), 2, "badge and sentence: {opened:?}");
    assert!(SANDBOX_DID_NOT_START.contains("Task Manager") && SANDBOX_DID_NOT_START.contains("switch this teammate to Auto"));
}

// case: says nothing of the sort for a command that ran and failed (control)
#[test]
fn a_command_that_ran_and_failed_says_nothing_of_the_sort() {
    for open in [false, true] {
        let words = row(command("node : The term node is not recognized", Some(1), ledger::Status::Failed), open);
        assert!(!words.iter().any(|w| w.contains("sandbox did not start") || w.contains("Task Manager")), "{words:?}");
        assert!(words.iter().any(|w| w.contains("failed")), "{words:?}");
    }
    // Codex's words on a command that did not fail are only its output.
    for open in [false, true] {
        let words = row(command(SAID, Some(0), ledger::Status::Done), open);
        assert!(!words.iter().any(|w| w.contains("sandbox did not start") || w.contains("Task Manager")), "{words:?}");
    }
}

//! a-finished-run-has-nothing-still-running.test.tsx: once the turn is over, a command or tool that never reported
//! says "did not report" in amber, not "running" and not red; while the turn is going it still says it runs.

use super::*;

/// One row drawn alone: a command (`pnpm test`) or a tool (`grep_search`), settled or not, in a turn that is over or
/// still going. A finished turn marks its open calls finished, as the ledger reader does when the run ends.
fn drawn(shell: bool, settled: bool, finished: bool) -> (Vec<String>, Vec<rune::ElementInfo>) {
    let (name, args) = if shell { ("Bash", "pnpm test") } else { ("grep_search", "") };
    let item = ledger::Item::Tool {
        name: name.into(),
        summary: String::new(),
        detail: Box::new(ledger::ToolDetail { id: "c1".into(), arguments: args.into(), exit_code: settled.then_some(0), finished, ..Default::default() }),
        status: if settled { ledger::Status::Done } else { ledger::Status::Running },
    };
    let mut rt = rune::Runtime::new((State::default(), item), |(s, item), _| activity_item(item, &Palette::new(true), s, "conv"), (700.0, 200.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let els = rt.elements();
    (els.iter().flat_map(|e| [e.text.clone(), e.label.clone()]).flatten().collect(), els)
}

fn says(words: &[String], part: &str) -> bool {
    words.iter().any(|w| w.contains(part))
}

// case: says a command did not report once the turn is over
#[test]
fn a_command_did_not_report_once_the_turn_is_over() {
    let (words, _) = drawn(true, false, true);
    assert!(says(&words, "did not report"), "{words:?}");
    assert!(!words.iter().any(|w| w == "running"), "{words:?}");
}

// case: says a tool did not report once the turn is over
#[test]
fn a_tool_did_not_report_once_the_turn_is_over() {
    let (words, _) = drawn(false, false, true);
    assert!(says(&words, "did not report"), "{words:?}");
    assert!(!says(&words, "still running"), "{words:?}");
}

// case: still says running while the turn is genuinely going
#[test]
fn still_says_running_while_the_turn_is_going() {
    let (command, _) = drawn(true, false, false);
    assert!(command.iter().any(|w| w == "running"), "{command:?}");
    // A command's word is its badge's; the row's title stays the command alone.
    assert!(command.iter().any(|w| w == "pnpm test") && !says(&command, "still running"), "{command:?}");
    let (tool, _) = drawn(false, false, false);
    assert!(says(&tool, "still running"), "{tool:?}");
}

// case: marks it stalled rather than failed
#[test]
fn an_unreported_call_is_stalled_not_failed() {
    let d = ledger::ToolDetail { finished: true, ..Default::default() };
    assert_eq!(shell_badge(ledger::Status::Running, &d, true), ShellBadge::Stalled);
    let (_, els) = drawn(true, false, true);
    let l = crate::look::controls::Look::new(true);
    let fill = els.iter().find(|e| e.key.as_deref() == Some("conv/c1/badge")).map(|e| e.fill);
    assert_eq!(fill, Some(l.c("amber-tint")), "amber, not red");
    let (tool, _) = drawn(false, false, true);
    assert!(!says(&tool, "failed"), "{tool:?}");
}

// case: leaves a settled row alone
#[test]
fn a_settled_row_is_left_alone() {
    let (words, _) = drawn(true, true, true);
    assert!(!says(&words, "did not report"), "{words:?}");
    assert!(words.iter().any(|w| w == "exit 0"), "{words:?}");
}

//! Electron's `a-growing-group-shows-its-rows.test.tsx`: a steps group drawn open shows its rows under its line and
//! says it is open; a group nobody opened starts folded, its line alone, and says it is folded (`aria-expanded`, here
//! the line's `expanded`).
use super::*;

fn shell(id: &str, command: &str) -> ledger::Item {
    ledger::Item::Tool {
        name: "exec_command".into(),
        summary: command.into(),
        detail: Box::new(ledger::ToolDetail {
            id: id.into(),
            observed_edit: false,
            patch: None,
            reported_by: String::new(),
            parent: String::new(),
            arguments: String::new(),
            output: String::new(),
            output_reported: false,
            runtime_status: String::new(),
            duration_ms: None,
            exit_code: Some(0),
            helper: false,
            helper_type: String::new(),
            background: false,
            finished: true,
            progress: String::new(),
            background_ended: String::new(),
            children: Vec::new(),
            after_end: false,
        }),
        status: ledger::Status::Done,
    }
}

/// The group's words, and whether its line says it is open, drawn the way the thread draws a steps group.
fn drawn(open: Option<bool>) -> (String, Option<bool>) {
    let items = vec![shell("c1", "pnpm install"), shell("c2", "pnpm test")];
    let mut s = State::default();
    if let Some(open) = open {
        s.activity_open.insert("conv/group/c1".into(), open);
    }
    let mut rt = rune::Runtime::new((s, items), |(s, items), _| activity_group(items, &Palette::new(true), s, "conv"), (600.0, 400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    let words = all.iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n");
    let expanded = all.iter().find(|e| e.role == "button" && e.expanded.is_some()).and_then(|e| e.expanded);
    (words, expanded)
}

#[test]
fn shows_its_rows_under_its_line() {
    // case: shows its rows under its line
    let (words, expanded) = drawn(Some(true));
    assert_eq!(expanded, Some(true), "{words}");
    assert!(words.to_lowercase().contains("ran 2 commands"), "{words}");
    assert!(words.contains("pnpm install"), "{words}");
    assert!(words.contains("pnpm test"), "{words}");
}

#[test]
fn a_group_that_is_not_growing_starts_folded_as_before_control() {
    // case: a group that is not growing starts folded, as before (control)
    let (words, expanded) = drawn(None);
    assert_eq!(expanded, Some(false), "{words}");
    assert!(!words.contains("pnpm install"), "{words}");
    assert!(!words.contains("pnpm test"), "{words}");
    assert!(words.to_lowercase().contains("ran 2 commands"), "{words}");
}

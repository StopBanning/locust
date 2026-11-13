//! Electron's renderer/src/fold-tool-runs.test.ts.
use super::*;
use crate::ledger::ToolDetail;

fn tool(name: &str, target: &str) -> Item {
    Item::Tool { name: name.into(), status: Status::Done, summary: target.into(), detail: Box::new(ToolDetail::default()) }
}
fn read(target: &str) -> Item {
    tool("read_file", target)
}
fn with_status(item: Item, status: Status) -> Item {
    match item {
        Item::Tool { name, summary, detail, .. } => Item::Tool { name, status, summary, detail },
        other => other,
    }
}

// ---- folding a run of plain tool calls ----

#[test]
fn folds_two_or_more_into_one_row_that_names_and_counts_them() {
    let rows = fold_plain_tool_runs(&[read("a.ts"), read("b.ts"), read("c.ts")]);
    assert_eq!(rows, vec![Row::Folded { items: vec![0, 1, 2], names: vec!["a.ts".into(), "b.ts".into(), "c.ts".into()], verb: Some("read".into()) }]);
}

#[test]
fn leaves_a_single_tool_call_exactly_as_it_was() {
    assert_eq!(fold_plain_tool_runs(&[read("a.ts")]), vec![Row::One(0)]);
}

#[test]
fn never_folds_a_command_or_a_file_change_even_between_reads() {
    let rows = fold_plain_tool_runs(&[read("a.ts"), tool("bash", "pnpm test"), read("b.ts"), read("c.ts")]);
    assert!(matches!(&rows[..], [Row::One(0), Row::One(1), Row::Folded { .. }]), "{rows:?}");
    let rows = fold_plain_tool_runs(&[read("a.ts"), tool("exec_command", "cargo test"), read("b.ts")]);
    assert_eq!(rows, vec![Row::One(0), Row::One(1), Row::One(2)], "Codex's command is a command");
    let rows = fold_plain_tool_runs(&[read("a.ts"), tool("edit_file", "b.ts"), read("c.ts")]);
    assert_eq!(rows, vec![Row::One(0), Row::One(1), Row::One(2)]);
}

#[test]
fn never_folds_a_failed_call_or_one_still_running() {
    let failed = [read("a.ts"), with_status(read("b.ts"), Status::Failed), read("c.ts")];
    assert_eq!(fold_plain_tool_runs(&failed), vec![Row::One(0), Row::One(1), Row::One(2)]);
    let running = [read("a.ts"), with_status(read("b.ts"), Status::Running), read("c.ts")];
    assert_eq!(fold_plain_tool_runs(&running), vec![Row::One(0), Row::One(1), Row::One(2)]);
}

#[test]
fn drops_the_shared_verb_when_the_run_does_not_share_one() {
    let rows = fold_plain_tool_runs(&[read("a.ts"), tool("grep", "b.ts")]);
    assert!(matches!(&rows[..], [Row::Folded { verb: None, .. }]), "{rows:?}");
}

#[test]
fn never_folds_a_helper_or_an_image() {
    let mut helper = read("b.ts");
    if let Item::Tool { detail, .. } = &mut helper {
        detail.helper = true;
    }
    assert_eq!(fold_plain_tool_runs(&[read("a.ts"), helper, read("c.ts")]), vec![Row::One(0), Row::One(1), Row::One(2)]);
    assert_eq!(fold_plain_tool_runs(&[read("a.ts"), read("shot.png"), read("c.ts")]), vec![Row::One(0), Row::One(1), Row::One(2)]);
}

// ---- what a folded row says ----

#[test]
fn leads_with_the_shared_verb_and_the_count() {
    assert_eq!(folded_tools_text(&["a.ts".into(), "b.ts".into()], Some("read")), "Read 2 files \u{2014} a.ts, b.ts");
    assert_eq!(folded_tools_lead(2, Some("search")), "Searched 2 times");
}

#[test]
fn counts_tool_calls_when_there_is_no_shared_verb() {
    assert_eq!(folded_tools_text(&["a.ts".into(), "b.ts".into()], None), "2 tool calls \u{2014} a.ts, b.ts");
}

#[test]
fn says_how_many_it_is_not_showing_rather_than_stopping_silently() {
    let names: Vec<String> = (0..11).map(|i| format!("f{i}.ts")).collect();
    let said = folded_tools_text(&names, Some("read"));
    assert!(said.contains("Read 11 files"));
    assert!(said.contains(&format!("\u{2026} {} more", 11 - FOLDED_TOOL_NAMES_SHOWN)));
    assert!(said.contains("f0.ts, f1.ts, f2.ts"));
    assert_eq!(said, "Read 11 files \u{2014} f0.ts, f1.ts, f2.ts \u{2026} 8 more", "Electron shows three names");
}

#[test]
fn adds_no_tail_when_the_whole_list_fits() {
    assert!(!folded_tools_text(&["a.ts".into()], Some("read")).contains("more"));
}

//! `a-read-row-names-the-file.test.ts` (Yurt's beta report, 2026-09-23, #16): a read row names the file, not the
//! disk. OpenCode reports a read's path whole, and the fold drew the folder itself, then the start of the next path.
use crate::ledger::{Item, Status, ToolDetail};
use crate::path_words::relative_path;
use crate::tool_fold::{Row, fold_plain_tool_runs, folded_tools_text, shown_name};

const WORKSPACE: &str = r"C:\Users\<home>\Documents\locust-scratch\locust-walk-ws-EfOL3P";

fn tool(name: &str, target: &str) -> Item {
    Item::Tool {
        name: name.into(),
        status: Status::Done,
        summary: target.into(),
        detail: Box::new(ToolDetail::default()),
    }
}

fn read(target: &str) -> Item {
    tool("read", target)
}

/// What the row shows as the call's name, as Electron's `activityEntries` relativizes it to the folder.
fn entry_name(item: &Item, workspace: Option<&str>) -> String {
    let Item::Tool { name, summary, detail, .. } = item else { unreachable!() };
    relative_path(&shown_name(name, summary, &detail.arguments), workspace)
}

#[test]
fn fold_under_names_relative_to_the_folder() {
    let items = vec![
        read(WORKSPACE),
        read(&format!("{WORKSPACE}\\README.md")),
        read(&format!("{WORKSPACE}\\src\\index.ts")),
    ];
    let rows = fold_plain_tool_runs(&items);
    let Row::Folded { names, verb, .. } = &rows[0] else { panic!("three reads fold into one row: {rows:?}") };
    let names: Vec<String> = names.iter().map(|n| relative_path(n, Some(WORKSPACE))).collect();
    assert_eq!(
        folded_tools_text(&names, verb.as_deref()),
        "Read 3 files \u{2014} locust-walk-ws-EfOL3P, README.md, src/index.ts"
    );
}

#[test]
fn leave_a_name_that_is_not_a_path_as_it_was() {
    assert_eq!(entry_name(&tool("web search", ""), Some(WORKSPACE)), "web search");
}

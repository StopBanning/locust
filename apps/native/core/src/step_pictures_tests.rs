//! `a-thread-shows-local-pictures.test.tsx`: which rows bring a picture into their step.

use super::step_picture_paths;
use crate::activity_entries::Patch;
use crate::ledger::{Item, Status, ToolDetail};

fn tool(id: &str, name: &str, summary: &str) -> Item {
    Item::Tool {
        name: name.into(),
        status: Status::Done,
        summary: summary.into(),
        detail: Box::new(ToolDetail { id: id.into(), finished: true, ..ToolDetail::default() }),
    }
}

fn pictures(items: &[Item]) -> Vec<String> {
    step_picture_paths(items, Some("C:/conversation"))
}

#[test]
fn every_runtimes_read_tool_brings_its_picture() {
    for name in ["Read", "read_file", "view_file", "readFile", "ReadToolCall"] {
        assert_eq!(pictures(&[tool("t", name, "chart.png")]), ["chart.png"], "{name}");
    }
}

#[test]
fn a_read_of_a_file_that_is_not_a_picture_or_a_search_naming_one_brings_none() {
    assert!(pictures(&[tool("t", "Read", "notes.md")]).is_empty());
    assert!(pictures(&[tool("t", "Grep", "chart.png")]).is_empty());
    assert!(pictures(&[tool("t", "Read", "x.svg")]).is_empty(), "SVG is never drawn");
}

#[test]
fn a_failed_read_brings_no_picture() {
    let mut failed = tool("t", "Read", "chart.png");
    if let Item::Tool { status, .. } = &mut failed {
        *status = Status::Failed;
    }
    assert!(pictures(&[failed]).is_empty());
}

#[test]
fn a_written_or_seen_changed_picture_brings_its_thumbnail() {
    assert_eq!(pictures(&[tool("t", "Write", "red.png")]), ["red.png"]);
    let mut seen = tool("t", "observed_edit", "red.png");
    if let Item::Tool { detail, .. } = &mut seen {
        detail.observed_edit = true;
    }
    assert_eq!(pictures(&[seen]), ["red.png"]);
}

fn patched(deleted: bool) -> Item {
    let mut item = tool("t", "Write", "red.png");
    let text = format!(
        "diff --git a/red.png b/red.png\n--- {}\n+++ {}\n@@ -{} +{} @@\n{}image\n",
        if deleted { "a/red.png" } else { "/dev/null" },
        if deleted { "/dev/null" } else { "b/red.png" },
        if deleted { "1" } else { "0,0" },
        if deleted { "0,0" } else { "1" },
        if deleted { "-" } else { "+" },
    );
    if let Item::Tool { detail, .. } = &mut item {
        detail.patch = Some(Patch { text, added: usize::from(!deleted), removed: usize::from(deleted), truncated: false });
    }
    item
}

#[test]
fn a_parsed_changed_picture_brings_its_thumbnail_but_a_deleted_one_never() {
    assert_eq!(pictures(&[patched(false)]), ["red.png"]);
    assert!(pictures(&[patched(true)]).is_empty());
}

#[test]
fn a_step_shows_each_picture_once_in_the_order_it_was_touched() {
    let items = [
        tool("a", "Read", "before.png"),
        tool("b", "Bash", "convert before.png after.png"),
        tool("c", "Read", "after.png"),
        tool("d", "Read", "./before.png"),
    ];
    assert_eq!(pictures(&items), ["before.png", "after.png"]);
}

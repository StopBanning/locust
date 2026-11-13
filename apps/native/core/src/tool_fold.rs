//! Folding a run of plain tool calls into one row (Electron `missionView.ts` `foldPlainToolRuns`, `foldedToolsText`,
//! from grok-build's `group_tool_verbs`). A turn that read eleven files drew eleven rows of equal weight, and the one
//! edit among them looked exactly like the ten reads.
//!
//! The rule is narrow on purpose: a row folds only if it is a plain tool call, settled and not failed. A command, a
//! file change, a helper, anything still running and anything that failed keeps its own row, so the fold never hides
//! work that changed something or went wrong. A single row is left alone. A folded row names what it covers and counts
//! what it does not show: a list never stops without saying so.

use crate::ledger::{Item, Status};

/// How many names a folded row shows before it counts the rest.
pub const FOLDED_TOOL_NAMES_SHOWN: usize = 3;

/// A row of activity: one item as it was, or a run folded into one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Row {
    One(usize),
    Folded { items: Vec<usize>, names: Vec<String>, verb: Option<String> },
}

const IMAGE_EXTENSIONS: [&str; 8] = [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".avif", ".ico"];

fn is_image_path(path: &str) -> bool {
    let lower = path.trim().to_lowercase();
    IMAGE_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

/// What the row shows as the call's name: its target when the runtime gave one, else the tool's own name.
pub fn shown_name(name: &str, summary: &str, arguments: &str) -> String {
    [summary, arguments, name].into_iter().map(str::trim).find(|s| !s.is_empty()).unwrap_or_default().to_owned()
}

/// The kind of a tool by its name, as `railToolName` reads it: read, search, web search, open a page, or the name.
pub fn tool_kind(name: &str) -> String {
    let spaced = crate::invariants::regex(r"([a-z0-9])([A-Z])").replace_all(name, "$1 $2").to_lowercase();
    let words: Vec<&str> = spaced.split(|c: char| !c.is_ascii_lowercase()).filter(|w| !w.is_empty()).collect();
    let has = |w: &str| words.contains(&w);
    if has("websearch") || (has("web") && has("search")) {
        "websearch".into()
    } else if words.iter().any(|w| matches!(*w, "search" | "grep" | "find" | "glob")) {
        "search".into()
    } else if words.iter().any(|w| matches!(*w, "read" | "view" | "cat" | "open")) {
        "read".into()
    } else if words.iter().any(|w| matches!(*w, "fetch" | "browse")) {
        "fetch".into()
    } else {
        name.trim().to_lowercase()
    }
}

fn foldable(item: &Item) -> Option<String> {
    let Item::Tool { name, status, summary, detail } = item else { return None };
    let plain = *status == Status::Done
        && !detail.helper
        && !crate::mission_export::is_shell_tool(name, None)
        // Codex's command, which Electron knows by its kind (`command_execution`) and native's record by these names.
        && !matches!(name.to_lowercase().as_str(), "exec_command" | "commandexecution" | "command_execution")
        && name != "file_change"
        && !crate::mission_export::edit_tool_name(name);
    let shown = shown_name(name, summary, &detail.arguments);
    (plain && !is_image_path(&shown)).then_some(shown)
}

/// Fold runs of two or more consecutive plain tool calls; everything else keeps its own row, in order.
pub fn fold_plain_tool_runs(items: &[Item]) -> Vec<Row> {
    let mut out = Vec::new();
    let mut index = 0;
    while index < items.len() {
        if foldable(&items[index]).is_none() {
            out.push(Row::One(index));
            index += 1;
            continue;
        }
        let mut end = index + 1;
        while end < items.len() && foldable(&items[end]).is_some() {
            end += 1;
        }
        if end - index < 2 {
            out.push(Row::One(index));
        } else {
            let names: Vec<String> = items[index..end].iter().filter_map(foldable).filter(|n| !n.is_empty()).collect();
            let kinds: std::collections::BTreeSet<String> = items[index..end]
                .iter()
                .filter_map(|i| match i {
                    Item::Tool { name, .. } => Some(tool_kind(name)),
                    _ => None,
                })
                .collect();
            let verb = (kinds.len() == 1).then(|| kinds.into_iter().next().unwrap_or_default());
            out.push(Row::Folded { items: (index..end).collect(), names, verb });
        }
        index = end;
    }
    out
}

/// What a folded row did, in Activity's own words: "Read 3 files", "Searched 2 times"; a tool with no word of its own
/// is counted (`foldedToolsLead`).
pub fn folded_tools_lead(count: usize, verb: Option<&str>) -> String {
    match verb {
        Some("read") => format!("Read {count} files"),
        Some("search") => format!("Searched {count} times"),
        Some("websearch") => format!("Searched the web {count} times"),
        Some("fetch") => format!("Opened a page {count} times"),
        _ => format!("{count} tool calls"),
    }
}

/// The names a folded row shows, and how many it does not (`foldedToolsNames`).
pub fn folded_tools_names(names: &[String]) -> String {
    let shown = names.iter().take(FOLDED_TOOL_NAMES_SHOWN).cloned().collect::<Vec<_>>().join(", ");
    let rest = names.len().saturating_sub(FOLDED_TOOL_NAMES_SHOWN);
    if rest > 0 { format!("{shown} \u{2026} {rest} more") } else { shown }
}

/// The whole row's words (`foldedToolsText`).
pub fn folded_tools_text(names: &[String], verb: Option<&str>) -> String {
    format!("{} \u{2014} {}", folded_tools_lead(names.len(), verb), folded_tools_names(names))
}

#[cfg(test)]
#[path = "tool_fold_tests.rs"]
mod tests;

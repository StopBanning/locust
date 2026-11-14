//! The change card counts the patch it can draw, not the runtime's header.
use crate::{changes, ledger::{Item, Status}};
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Patch { pub text: String, pub added: usize, pub removed: usize, pub truncated: bool }
#[derive(Clone, Debug)]
pub struct FileEntry { pub key: String, pub file: changes::File, pub counts: (usize, usize), pub reported: Option<(usize, usize)>, pub large: bool }
pub fn files(item: &Item) -> Vec<FileEntry> {
    let Item::Tool { detail, .. } = item else { return vec![] };
    let Some(patch) = &detail.patch else { return vec![] };
    let parsed = changes::parse(&patch.text);
    let single = parsed.len() == 1;
    parsed.into_iter().enumerate().map(|(i, file)| {
        let counts = file.counts();
        let reported = (patch.truncated && single).then_some((patch.added, patch.removed));
        let total = reported.unwrap_or(counts);
        FileEntry { key: format!("{}/file/{i}", detail.id), file, counts, reported, large: total.0 + total.1 > 200 || patch.truncated }
    }).collect()
}
pub fn counts(items: &[Item]) -> (usize, usize) {
    items.iter().flat_map(files).fold((0,0), |(a,d), f| (a+f.counts.0,d+f.counts.1))
}
pub fn default_open(entries: &[FileEntry]) -> Option<&str> {
    let first = entries.first()?;
    let path = first.file.path.to_lowercase();
    let page = first.file.status == "ADDED" && (path.ends_with(".html") || path.ends_with(".htm"));
    (!first.large || page).then_some(first.key.as_str())
}
pub fn unreported(item: &Item) -> Option<(&str, bool, bool)> {
    let Item::Tool { name, summary, status, detail } = item else { return None };
    (crate::activity_summary::is_edit(item) && files(item).is_empty()).then_some((if summary.is_empty() { name } else { summary }, *status != Status::Running, detail.runtime_status == "result too large to keep"))
}
pub fn tool_name<'a>(name: &'a str, target: &str) -> Option<&'a str> { (name != target).then_some(name) }
#[cfg(test)]
#[path = "activity_entries_tests.rs"] mod tests;

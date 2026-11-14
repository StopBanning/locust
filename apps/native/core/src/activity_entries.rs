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
/// File identity uses the displayed path, status and every hunk/row. Restatements fold;
/// a later real edit to the same path has different hunks and stays visible.
pub fn path_key(path: &str, workspace: Option<&str>) -> String {
    crate::invariants::regex(r"[\\/]+").replace_all(&crate::path_words::relative_path(path, workspace), "/").trim_start_matches("./").to_lowercase()
}
pub fn entries(items: &[Item], workspace: Option<&str>) -> Vec<(usize, FileEntry)> {
    let mut seen = std::collections::HashSet::new();
    items.iter().enumerate().flat_map(|(i,item)| files(item).into_iter().map(move |f|(i,f))).filter(|(_,f)| {
        let signature = format!("{}|{}|{:?}", path_key(&f.file.path, workspace), f.file.status, f.file.hunks);
        seen.insert(signature)
    }).collect()
}
pub fn counts(items: &[Item]) -> (usize, usize) { counts_in(items, None) }
pub fn counts_in(items: &[Item], workspace: Option<&str>) -> (usize, usize) {
    entries(items, workspace).iter().fold((0,0), |(a,d), (_,f)| (a+f.counts.0,d+f.counts.1))
}
pub fn changed_paths(items: &[Item]) -> Vec<String> {
    items.iter().flat_map(|item| {
        let files = files(item);
        if !files.is_empty() { return files.into_iter().map(|f| f.file.path).collect(); }
        match item {
            Item::Tool { summary, name, status, .. } if crate::activity_summary::is_edit(item) && *status != Status::Failed => {
                (if summary.is_empty() {name} else {summary}).lines().map(str::trim).filter(|p|!p.is_empty()).map(str::to_owned).collect()
            },
            _ => vec![],
        }
    }).collect()
}
pub fn file_count(paths: &[String], workspace: Option<&str>) -> usize {
    paths.iter().map(|p|path_key(p,workspace)).collect::<std::collections::HashSet<_>>().len()
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

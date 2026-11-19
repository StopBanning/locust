//! The foot of a finished turn (Thread.tsx `TurnFoot`, `filesHeading`; missionView.ts `pathInWorkspace`, 0.491 /
//! 0.494 / 0.672 / 2026-10-05): the files the turn changed, one row each, headed by what the runtime edited apart
//! from what only changed in the folder while it ran. Paths outside the folder (a scratch tree a resumed run wrote
//! into) are not the turn's files, unless every edit is outside, when they are all there is to show.
use crate::activity_entries::{FileEntry, net_entries, path_key, unreported};
use crate::ledger::{Item, Status};

/// One row of the foot's files card: a file with its patch, or a file the turn named with no diff to show.
#[derive(Clone, Debug)]
pub enum Row {
    File { at: usize, entry: FileEntry, observed: bool },
    Unreported { at: usize, path: String, observed: bool },
}

impl Row {
    fn path(&self) -> &str {
        match self {
            Row::File { entry, .. } => &entry.file.path,
            Row::Unreported { path, .. } => path,
        }
    }
    fn observed(&self) -> bool {
        matches!(self, Row::File { observed: true, .. } | Row::Unreported { observed: true, .. })
    }
}

#[derive(Clone, Debug)]
pub struct Foot {
    /// `filesHeading`: "Edited 1 file · 2 more changed in the folder", or "2 files changed in the folder while it ran".
    pub heading: String,
    /// Indices of the turn's items the card is drawn from.
    pub edits: Vec<usize>,
    pub rows: Vec<Row>,
    pub edited: usize,
    pub seen: usize,
}

/// missionView.ts `pathInWorkspace`: inside the folder, already relative to it, or a copy of it that maps inside.
pub fn path_in_workspace(path: &str, workspace: Option<&str>) -> bool {
    let Some(folder) = workspace.filter(|w| !w.is_empty()) else { return true };
    let normalise = |v: &str| crate::invariants::regex(r"[\\/]+").replace_all(v, "/").trim_end_matches('/').to_lowercase();
    let (root, full) = (normalise(folder), normalise(path));
    if full == root || full.starts_with(&format!("{root}/")) {
        return true;
    }
    let absolute = crate::invariants::regex(r"^(?:[a-z]:)?/");
    if !absolute.is_match(&full) {
        return true;
    }
    let relative = crate::path_words::relative_path(path, Some(folder));
    relative != path && !crate::invariants::regex(r"^(?:[a-zA-Z]:)?[\\/]").is_match(&relative)
}

/// Thread.tsx `filesHeading`.
pub fn files_heading(edited: usize, seen: usize, undone: bool) -> String {
    let files = |n: usize| if n == 1 { "1 file".to_owned() } else { format!("{n} files") };
    let said = if edited > 0 {
        format!("Edited {}{}", files(edited), if seen > 0 { format!(" \u{b7} {seen} more changed in the folder") } else { String::new() })
    } else {
        format!("{} changed in the folder while it ran", files(seen))
    };
    if undone { format!("{said} \u{b7} undone") } else { said }
}

fn observed_status(item: &Item) -> &str {
    match item {
        Item::Tool { detail, .. } => &detail.runtime_status,
        _ => "",
    }
}

/// The foot of one finished turn, from that turn's items; None when nothing changed.
pub fn turn_foot(items: &[Item], workspace: Option<&str>) -> Option<Foot> {
    let named: Vec<usize> = items
        .iter()
        .enumerate()
        .filter(|(_, item)| crate::activity_summary::is_edit(item) && !matches!(item, Item::Tool { status: Status::Failed, .. }))
        .map(|(i, _)| i)
        .collect();
    let path_of = |i: usize| match &items[i] {
        Item::Tool { summary, .. } => summary.clone(),
        _ => String::new(),
    };
    let in_folder: Vec<usize> = named.iter().copied().filter(|&i| path_in_workspace(&path_of(i), workspace)).collect();
    let edits = if in_folder.is_empty() { named } else { in_folder };
    let picked: Vec<Item> = edits.iter().map(|&i| items[i].clone()).collect();
    let mut rows: Vec<Row> = net_entries(&picked, workspace)
        .into_iter()
        .map(|(at, entry)| Row::File { at: edits[at], observed: observed_status(&picked[at]) == "observed on disk", entry })
        .collect();
    let has_file = |rows: &[Row], path: &str| rows.iter().any(|r| matches!(r, Row::File { .. }) && path_key(r.path(), workspace) == path_key(path, workspace));
    for (at, item) in picked.iter().enumerate() {
        let Some((name, _, _)) = unreported(item) else { continue };
        let paths: Vec<String> = {
            let many = crate::activity_summary::unreported_paths(item);
            if many.is_empty() { vec![name.to_owned()] } else { many }
        };
        let observed = crate::invariants::regex(r"on disk|from disk").is_match(observed_status(item));
        for path in paths {
            // A file the host could not read sits beside the step that changed it: one row, the step's (0.672).
            if has_file(&rows, &path) || rows.iter().any(|r| path_key(r.path(), workspace) == path_key(&path, workspace)) {
                continue;
            }
            rows.push(Row::Unreported { at: edits[at], path, observed });
        }
    }
    let count = |seen: bool| rows.iter().filter(|r| r.observed() == seen).map(|r| path_key(r.path(), workspace)).collect::<std::collections::HashSet<_>>().len();
    let (edited, seen) = (count(false), count(true));
    (edited + seen > 0).then(|| Foot { heading: files_heading(edited, seen, false), edits, rows, edited, seen })
}

#[cfg(test)]
#[path = "turn_foot_tests.rs"]
mod tests;

//! A file as it was after an earlier turn, exactly or not at all (Electron `shared/reverse-diff.ts` +
//! `renderer/src/fileVersions.ts` + `renderer/src/missionView.ts::fileTurns`, 0.502 / 0.517).
//!
//! Rebuilt from the file as it is now by undoing each later turn's recorded change, newest first, with no fuzzy
//! matching and no offset guessing. A later change that arrived cut short, a file changed since, or a rename is
//! refused with a reason so a wrong document that looks right is never shown.

use crate::activity_entries;
use crate::changes::{self, File};
use crate::ledger::Item;

const CHANGED_SINCE: &str = "it was changed after those replies";

/// One turn that changed one file, oldest first (`missionView.ts::FileTurn`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileTurn {
    pub mission_id: String,
    pub prompt: String,
    pub file: File,
    pub counts: (usize, usize),
    pub truncated: bool,
    pub reported: Option<(usize, usize)>,
}

/// What the file became when reversed: its text, or `None` when the file was not there after that turn
/// (`shared/reverse-diff.ts::Reversed`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reversed {
    Ok(Option<String>),
    Err(String),
}

fn bare(line: &str) -> &str {
    line.strip_suffix('\r').unwrap_or(line)
}

/// One change undone on `content` (`None`: the file is not there) (`shared/reverse-diff.ts::reverseChange`).
pub fn reverse_change(content: Option<&str>, change: &File) -> Reversed {
    if change.status == "RENAMED" {
        return Reversed::Err("it was renamed, and a rename is not undone here".into());
    }
    if change.hunks.is_empty() {
        return Reversed::Err("its change was recorded without the lines".into());
    }
    let eol = if content.is_some_and(|c| c.contains("\r\n")) {
        "\r\n"
    } else {
        "\n"
    };

    if change.status == "ADDED" {
        let Some(text) = content else {
            return Reversed::Ok(None);
        };
        let made: Vec<&str> = change
            .hunks
            .iter()
            .flat_map(|h| h.rows.iter().filter(|r| r.kind == '+').map(|r| bare(&r.text)))
            .collect();
        let mut now: Vec<&str> = text.split('\n').map(bare).collect();
        if now.last() == Some(&"") {
            now.pop();
        }
        return if now == made {
            Reversed::Ok(None)
        } else {
            Reversed::Err(CHANGED_SINCE.into())
        };
    }

    if change.status == "DELETED" {
        if content.is_some() {
            return Reversed::Err(CHANGED_SINCE.into());
        }
        let hunk = &change.hunks[0];
        let new_count = hunk.rows.iter().filter(|r| r.kind != '-').count();
        if change.hunks.len() != 1 || hunk.old != 1 || new_count != 0 {
            return Reversed::Err("its recorded change does not hold the whole file".into());
        }
        let restored = hunk
            .rows
            .iter()
            .map(|r| bare(&r.text))
            .collect::<Vec<_>>()
            .join("\n");
        return Reversed::Ok(Some(format!("{restored}\n")));
    }

    let Some(text) = content else {
        return Reversed::Err("it is not there any more".into());
    };
    let mut lines: Vec<String> = text.split('\n').map(|l| bare(l).to_owned()).collect();
    let ends_with_newline = lines.last().is_some_and(|l| l.is_empty());
    if ends_with_newline {
        lines.pop();
    }
    // Bottom hunk first, so line numbers above it still hold.
    let mut hunks: Vec<&changes::Hunk> = change.hunks.iter().collect();
    hunks.sort_by(|left, right| right.new.cmp(&left.new));
    for hunk in hunks {
        let after: Vec<&str> = hunk
            .rows
            .iter()
            .filter(|r| r.kind != '-')
            .map(|r| bare(&r.text))
            .collect();
        let before: Vec<String> = hunk
            .rows
            .iter()
            .filter(|r| r.kind != '+')
            .map(|r| bare(&r.text).to_owned())
            .collect();
        // "@@ -a,b +0,0 @@" empties the file; otherwise the new side starts at `hunk.new`.
        let at = if after.is_empty() {
            hunk.new
        } else {
            match hunk.new.checked_sub(1) {
                Some(v) => v,
                None => return Reversed::Err(CHANGED_SINCE.into()),
            }
        };
        if at + after.len() > lines.len() {
            return Reversed::Err(CHANGED_SINCE.into());
        }
        for (idx, expected) in after.iter().enumerate() {
            if lines[at + idx] != *expected {
                return Reversed::Err(CHANGED_SINCE.into());
            }
        }
        lines.splice(at..at + after.len(), before);
    }
    Reversed::Ok(Some(format!(
        "{}{}",
        lines.join(eol),
        if ends_with_newline { eol } else { "" }
    )))
}

/// Several turns' changes to one file, oldest first as recorded; undone newest first
/// (`shared/reverse-diff.ts::reverseChanges`).
pub fn reverse_changes(content: Option<&str>, changes_list: &[&File]) -> Reversed {
    let mut now: Option<String> = content.map(str::to_owned);
    for change in changes_list.iter().rev() {
        match reverse_change(now.as_deref(), change) {
            Reversed::Ok(next) => now = next,
            err @ Reversed::Err(_) => return err,
        }
    }
    Reversed::Ok(now)
}

/// Rebuild the file as it was after `turns[index]` from `now` by undoing every later turn's change
/// (`renderer/src/fileVersions.ts::fileAsItWas`).
pub fn file_as_it_was(now: &str, turns: &[FileTurn], index: usize) -> Reversed {
    let later = turns.get(index + 1..).unwrap_or(&[]);
    if later.iter().any(|turn| turn.truncated) {
        return Reversed::Err("a later change to it was recorded cut short".into());
    }
    let later_files: Vec<&File> = later.iter().map(|turn| &turn.file).collect();
    reverse_changes(Some(now), &later_files)
}

/// Every turn in a conversation's ledger items that changed `path`, oldest first (`missionView.ts::fileTurns`).
pub fn file_turns_from_items(items: &[Item], path: &str, workspace: Option<&str>) -> Vec<FileTurn> {
    let wanted = activity_entries::path_key(path, workspace);
    let mut slices: Vec<(String, usize, usize)> = Vec::new();
    let mut start = 0usize;
    let mut prompt = String::new();
    let mut saw_user = false;
    for (idx, item) in items.iter().enumerate() {
        if let Item::User(words) = item {
            if saw_user || idx > start {
                slices.push((prompt, start, idx));
            }
            prompt = words.clone();
            start = idx + 1;
            saw_user = true;
        }
    }
    if saw_user || start < items.len() {
        slices.push((prompt, start, items.len()));
    }

    let mut out = Vec::new();
    for (turn_idx, (turn_prompt, from, to)) in slices.into_iter().enumerate() {
        for (_, entry) in activity_entries::net_entries(&items[from..to], workspace) {
            if activity_entries::path_key(&entry.file.path, workspace) != wanted {
                continue;
            }
            out.push(FileTurn {
                mission_id: format!("turn-{}", turn_idx + 1),
                prompt: turn_prompt.clone(),
                file: entry.file,
                counts: entry.counts,
                truncated: entry.truncated,
                reported: entry.reported,
            });
        }
    }
    out
}

#[cfg(test)]
#[path = "file_versions_tests.rs"]
mod tests;

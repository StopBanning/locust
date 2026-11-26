//! Cutover 1, row 4: a read-only check of a folder of Electron's conversation files (`mission-ledger`), run over a
//! COPY of it. Each `*.jsonl` file is read by the Rust ledger reader and sorted by what it found: read whole, read
//! with something wrong, refused, or read with no turns. It only opens files to read them and writes nothing.
use crate::ledger::{self, Issue};
use std::collections::BTreeMap;
use std::path::Path;

/// What the check found in one folder: files read whole, and every other file listed under the reason.
#[derive(Debug, Default, PartialEq)]
pub struct HistoryCheck {
    /// Files read whole with at least one turn: nothing to report.
    pub read_whole: usize,
    /// The file names, by reason, of every file that was refused, damaged or read with zero turns.
    pub findings: BTreeMap<String, Vec<String>>,
}

impl HistoryCheck {
    /// Files read, whole or not.
    pub fn files(&self) -> usize {
        self.read_whole + self.findings.values().map(Vec::len).sum::<usize>()
    }
}

/// Checks every `*.jsonl` file directly in `dir`; the same folder's `.trash/` (conversations Electron put aside) is
/// checked too and its findings say so. A folder that cannot be listed is an error.
pub fn check_history(dir: &Path) -> Result<HistoryCheck, String> {
    let mut done = HistoryCheck::default();
    check_folder(dir, "", &mut done)?;
    // The trash is optional: no `.trash` folder is no finding.
    if dir.join(".trash").is_dir() {
        check_folder(&dir.join(".trash"), "in .trash: ", &mut done)?;
    }
    for names in done.findings.values_mut() {
        names.sort();
    }
    Ok(done)
}

fn check_folder(dir: &Path, within: &str, done: &mut HistoryCheck) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{} could not be listed ({e})", dir.display()))?;
    for entry in entries {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "jsonl") {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        match judge(&path) {
            None => done.read_whole += 1,
            Some(why) => done.findings.entry(format!("{within}{why}")).or_default().push(name),
        }
    }
    Ok(())
}

/// None when the file is read whole with at least one turn; otherwise why not.
fn judge(path: &Path) -> Option<String> {
    let id = path.file_stem().and_then(|v| v.to_str()).unwrap_or_default();
    if !ledger::safe_id(id) {
        return Some("refused: the file name is not a conversation id".into());
    }
    let read = match ledger::read_ledger(path) {
        Ok(read) => read,
        Err(e) => return Some(format!("refused: could not be read ({:?})", e.kind())),
    };
    let codes = |issues: &[Issue]| issues.iter().map(|i| i.code()).collect::<Vec<_>>().join(" + ");
    if read.unreadable {
        return Some(format!("refused: no conversation could be read ({})", codes(&read.issues)));
    }
    if !read.issues.is_empty() {
        return Some(format!("damaged: only the part before the fault was read ({})", codes(&read.issues)));
    }
    let transcript = ledger::read_transcript(path);
    // The header alone always draws one item, the opening prompt: a conversation with nothing after it has no turns.
    if transcript.items.len() <= 1 {
        return Some("read with zero turns: only the opening prompt, no reply, tool or plan".into());
    }
    None
}

#[cfg(test)]
#[path = "history_check_tests.rs"]
mod tests;

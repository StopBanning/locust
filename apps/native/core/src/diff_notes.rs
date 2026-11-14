//! NOTES ON A DIFF, SENT AS ONE MESSAGE (Electron `renderer/src/diffNotes.ts`, 0.376 and 0.395).
//!
//! Reviewing a teammate's change meant retyping what you were looking at, one message per thought, and each message
//! set the teammate off before the next arrived. So a "+" on a line of the diff pins a note there; the notes wait as
//! one tile in the message box; and the next message carries them all, each with its file, its line and the line
//! itself. Sent, the block is read back out of the message and each note is held against what that turn went on to
//! change: the line changed, only the file, nothing, or the file went.

use crate::changes::{File, Row};

/// The most a note's own text may be: a thought, not a document.
pub const MAX_DIFF_NOTE: usize = 1_000;
/// The most a quoted line of code carries into the message.
const QUOTED_CODE: usize = 160;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffNote {
    /// Which line of which file, stable across draws (`diff_note_key`).
    pub key: String,
    pub path: String,
    /// The line number a person reads: the new file's, or the old one's for a removed line.
    pub line: Option<usize>,
    /// '+', '-' or ' ', as the diff row is.
    pub kind: char,
    /// The line's code, as the diff showed it.
    pub code: String,
    /// What the person wrote.
    pub text: String,
}

fn kind_word(kind: char) -> &'static str {
    match kind {
        '+' => "add",
        '-' => "del",
        _ => "context",
    }
}

fn number(n: Option<usize>) -> String {
    n.map(|n| n.to_string()).unwrap_or_default()
}

pub fn diff_note_key(path: &str, row: &Row) -> String {
    format!("{path}:{}:{}:{}", kind_word(row.kind), number(row.old), number(row.new))
}

pub fn diff_note_for(path: &str, row: &Row, text: &str) -> DiffNote {
    DiffNote {
        key: diff_note_key(path, row),
        path: path.into(),
        line: if row.kind == '-' { row.old } else { row.new.or(row.old) },
        kind: row.kind,
        code: row.text.clone(),
        text: text.trim().chars().take(MAX_DIFF_NOTE).collect(),
    }
}

/// Where a note is, as a person says it: "src/app.ts, line 12", or "..., removed line 12".
pub fn diff_note_place(path: &str, line: Option<usize>, kind: char) -> String {
    match line {
        None => path.into(),
        Some(n) => format!("{path}, {} {n}", if kind == '-' { "removed line" } else { "line" }),
    }
}

fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The notes as the message carries them: in file order, then line order, each with the line it is about. One
/// block, after the person's own words.
pub fn diff_notes_block(notes: &[DiffNote]) -> String {
    if notes.is_empty() {
        return String::new();
    }
    let mut ordered: Vec<&DiffNote> = notes.iter().collect();
    // localeCompare, near enough: by the words ignoring case first.
    ordered.sort_by(|a, b| {
        if a.path == b.path {
            a.line.unwrap_or(0).cmp(&b.line.unwrap_or(0))
        } else {
            a.path.to_lowercase().cmp(&b.path.to_lowercase()).then_with(|| a.path.cmp(&b.path))
        }
    });
    let mut lines = vec![format!("Notes on your changes ({}):", notes.len())];
    for note in ordered {
        let code = flat(&note.code);
        let quoted = if code.is_empty() {
            String::new()
        } else if code.chars().count() > QUOTED_CODE {
            format!(" `{}…`", code.chars().take(QUOTED_CODE - 1).collect::<String>())
        } else {
            format!(" `{code}`")
        };
        lines.push(format!("- {}{quoted}: {}", diff_note_place(&note.path, note.line, note.kind), flat(&note.text)));
    }
    lines.join("\n")
}

/// The message with its notes: the person's words first, then the block.
pub fn with_diff_notes(text: &str, notes: &[DiffNote]) -> String {
    let block = diff_notes_block(notes);
    if block.is_empty() {
        return text.into();
    }
    if text.trim().is_empty() { block } else { format!("{}\n\n{block}", text.trim_end()) }
}

/// Add a note, or replace the one already on that line.
pub fn with_note(notes: &[DiffNote], note: DiffNote) -> Vec<DiffNote> {
    let mut kept: Vec<DiffNote> = notes.iter().filter(|n| n.key != note.key).cloned().collect();
    kept.push(note);
    kept
}

/// The message box's tile for them.
pub fn diff_notes_tile(count: usize) -> String {
    if count == 1 { "1 note on the changes".into() } else { format!("{count} notes on the changes") }
}

/// A note as a sent message carried it, read back out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SentDiffNote {
    pub path: String,
    pub line: Option<usize>,
    /// It was on a line the earlier change had removed.
    pub removed: bool,
    pub code: Option<String>,
    pub text: String,
}

/// One "- place `code`: text" line of the block.
fn sent_note(row: &str) -> Option<SentDiffNote> {
    let rest = row.strip_prefix("- ")?;
    // The text follows the last ": " that comes after the place and any quoted code.
    let (head, text) = match rest.find('`') {
        Some(open) => {
            let close = open + 1 + rest[open + 1..].find('`')?;
            let after = rest[close + 1..].strip_prefix(": ")?;
            (format!("{}`{}`", &rest[..open], &rest[open + 1..close]), after.to_owned())
        }
        None => {
            let at = rest.find(": ")?;
            (rest[..at].to_owned(), rest[at + 2..].to_owned())
        }
    };
    let (place, code) = match head.find(" `") {
        Some(at) if head.ends_with('`') => (head[..at].to_owned(), Some(head[at + 2..head.len() - 1].to_owned())),
        _ => (head.clone(), None),
    };
    let (path, line, removed) = ["removed line", "line"]
        .iter()
        .find_map(|word| {
            let marker = format!(", {word} ");
            let at = place.rfind(&marker)?;
            let n = place[at + marker.len()..].parse::<usize>().ok()?;
            Some((place[..at].to_owned(), Some(n), *word == "removed line"))
        })
        .unwrap_or((place.clone(), None, false));
    if path.is_empty() {
        return None;
    }
    Some(SentDiffNote { path, line, removed, code, text: text.trim().into() })
}

/// A sent message's own words, and the notes it carried, read back out of it. The block is the message's last part,
/// written by `diff_notes_block`, so its shape is ours.
pub fn split_diff_notes(text: &str) -> (String, Vec<SentDiffNote>) {
    let head = "Notes on your changes (";
    let starts: Vec<usize> = text.match_indices(head).map(|(i, _)| i).filter(|&i| i == 0 || text[..i].ends_with("\n\n")).collect();
    for &at in starts.iter().rev() {
        let block = &text[at..];
        let Some(first_end) = block.find("):\n") else { continue };
        if block[head.len()..first_end].parse::<usize>().is_err() {
            continue;
        }
        let rows: Vec<&str> = block[first_end + 3..].trim_end().split('\n').collect();
        if rows.is_empty() || !rows.iter().all(|r| r.starts_with("- ")) {
            continue;
        }
        let notes: Vec<SentDiffNote> = rows.iter().filter_map(|r| sent_note(r.trim())).collect();
        if notes.is_empty() {
            continue;
        }
        return (text[..at].trim_end().to_owned(), notes);
    }
    (text.to_owned(), Vec::new())
}

/// What the turn that received a note went on to do where it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteOutcome {
    Line,
    File,
    Untouched,
    Deleted,
}

/// A note's line is the file as it was BEFORE the turn that read it: the old side of that turn's own diff. A removed
/// or rewritten row with that old number is the line the note was on.
pub fn note_outcome(note: &SentDiffNote, edited: &[File]) -> NoteOutcome {
    let key = |path: &str| {
        let p = path.replace('\\', "/");
        let p = p.trim_start_matches("./").to_lowercase();
        let mut out = String::new();
        for c in p.chars() {
            if !(c == '/' && out.ends_with('/')) {
                out.push(c);
            }
        }
        out
    };
    let wanted = key(&note.path);
    let Some(file) = edited.iter().find(|f| {
        let have = key(&f.path);
        have == wanted || have.ends_with(&format!("/{wanted}")) || wanted.ends_with(&format!("/{have}"))
    }) else {
        return NoteOutcome::Untouched;
    };
    if file.status == "DELETED" {
        return NoteOutcome::Deleted;
    }
    let (Some(line), false) = (note.line, note.removed) else { return NoteOutcome::File };
    if file.hunks.iter().any(|h| h.rows.iter().any(|r| r.kind == '-' && r.old == Some(line))) { NoteOutcome::Line } else { NoteOutcome::File }
}

/// The chip a note wears once the turn is over.
pub fn note_outcome_words(outcome: NoteOutcome) -> &'static str {
    match outcome {
        NoteOutcome::Line => "Line changed",
        NoteOutcome::File => "File changed, not this line",
        NoteOutcome::Deleted => "File deleted",
        NoteOutcome::Untouched => "Not changed",
    }
}

/// A note's file, as the window shows it (Electron `displayPath`): relative to the conversation's folder when it is
/// inside it; an absolute path elsewhere as its last two parts behind an ellipsis; anything else as given. The
/// message, the hover and the outcome keep the path the note was made with.
pub fn shown_path(path: &str, folder: Option<&str>) -> String {
    crate::path_words::display_path(path, folder)
}

fn edit_key(path: &str, workspace: Option<&str>) -> String {
    let rel = crate::path_words::relative_path(path, workspace);
    let p = rel.replace('\\', "/");
    let p = p.trim_start_matches("./").to_lowercase();
    let mut out = String::with_capacity(p.len());
    for c in p.chars() {
        if !(c == '/' && out.ends_with('/')) {
            out.push(c);
        }
    }
    out
}

/// Files a finished turn changed (`missionView.ts`'s `editedFiles` + net preference), combining event patches
/// (preferring `observed_edit` over step diffs) with optional host checkpoint diffs.
pub fn edited_files<'a, I>(events: I, workspace: Option<&str>, checkpoint_files: &[File]) -> Vec<File>
where
    I: IntoIterator<Item = &'a serde_json::Value>,
{
    use serde_json::Value;
    use std::collections::{HashMap, HashSet};

    struct CallPatch {
        net: bool,
        patch_text: Option<String>,
    }

    let mut order: Vec<String> = Vec::new();
    let mut calls: HashMap<String, CallPatch> = HashMap::new();

    for (idx, event) in events.into_iter().enumerate() {
        let Some(kind) = event.get("type").and_then(Value::as_str) else {
            continue;
        };
        if !matches!(kind, "tool.started" | "tool.completed" | "tool.failed") {
            continue;
        }
        let Some(p) = event.get("payload") else {
            continue;
        };
        let item_id = p
            .get("itemId")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| format!("seq:{idx}"));
        let net = p.get("toolKind").and_then(Value::as_str) == Some("observed_edit");
        let patch_text = p
            .get("patch")
            .and_then(|patch| patch.get("text"))
            .and_then(Value::as_str)
            .filter(|t| !t.trim().is_empty())
            .map(str::to_owned);
        if let Some(prev) = calls.get_mut(&item_id) {
            prev.net |= net;
            if let Some(t) = patch_text {
                prev.patch_text = Some(t);
            }
        } else {
            order.push(item_id.clone());
            calls.insert(item_id, CallPatch { net, patch_text });
        }
    }

    let mut parsed: Vec<(bool, File)> = Vec::new();
    let mut observed_paths: HashSet<String> = HashSet::new();
    for id in order {
        let Some(call) = calls.remove(&id) else {
            continue;
        };
        let Some(text) = call.patch_text else {
            continue;
        };
        for file in crate::changes::parse(&text) {
            let k = edit_key(&file.path, workspace);
            if call.net {
                observed_paths.insert(k);
            }
            parsed.push((call.net, file));
        }
    }

    let mut checkpoint_paths: HashSet<String> = HashSet::new();
    for file in checkpoint_files {
        checkpoint_paths.insert(edit_key(&file.path, workspace));
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<File> = Vec::new();
    for (net, file) in parsed {
        let k = edit_key(&file.path, workspace);
        if !net && (observed_paths.contains(&k) || checkpoint_paths.contains(&k)) {
            continue;
        }
        if seen.insert(k) {
            out.push(file);
        }
    }
    for file in checkpoint_files {
        let k = edit_key(&file.path, workspace);
        if seen.insert(k) {
            out.push(file.clone());
        }
    }
    out
}

#[cfg(test)]
#[path = "diff_notes_tests.rs"]
mod tests;

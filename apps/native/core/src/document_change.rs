//! A document's change, in its own words (Electron `renderer/src/documentChange.ts`, 0.530).
//!
//! For a Markdown or prose file, a change reads as a list of passages — a changed one with its old words struck and
//! its new words marked by whole word, an added one, a removed one — each with Markdown's marks taken off.

use crate::changes::{self, File};
use crate::invariants::regex;

/// One passage in a document's change (`documentChange.ts::ChangedPassage`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChangedPassage {
    Changed {
        before: String,
        removed: String,
        added: String,
        after: String,
    },
    Added {
        text: String,
    },
    Removed {
        text: String,
    },
}

/// Split a Markdown table row on unescaped `|` characters (matching JS `split(/(?<!\\)\|/)`).
fn split_unescaped_pipes(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut prev = '\0';
    for (idx, ch) in s.char_indices() {
        if ch == '|' && prev != '\\' {
            parts.push(&s[start..idx]);
            start = idx + 1;
        }
        prev = ch;
    }
    parts.push(&s[start..]);
    parts
}

/// A Markdown line as a reader sees it: no heading hashes, bullets as bullets, a table row as its cells
/// (`documentChange.ts::asRead`).
pub fn as_read(line: &str) -> String {
    let trimmed = line.trim();
    if regex(r"^\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)*\|?$").is_match(trimmed) {
        return String::new();
    }
    if trimmed.starts_with('|') {
        let inner = trimmed
            .strip_prefix('|')
            .unwrap_or(trimmed)
            .strip_suffix('|')
            .unwrap_or(trimmed.strip_prefix('|').unwrap_or(trimmed));
        return split_unescaped_pipes(inner)
            .into_iter()
            .map(|cell| cell.trim().replace("\\|", "|"))
            .collect::<Vec<_>>()
            .join(" · ");
    }
    let s = regex(r"^#{1,6}\s+").replace(trimmed, "");
    let s = regex(r"^[-*+]\s+").replace(&s, "• ");
    let s = regex(r"^>\s?").replace(&s, "");
    let s = regex(r"\*\*([^*]+)\*\*").replace_all(&s, "$1");
    regex(r"__([^_]+)__").replace_all(&s, "$1").into_owned()
}

fn is_word_char(ch: Option<&char>) -> bool {
    ch.is_some_and(|c| c.is_alphanumeric())
}

/// Every changed, added, or removed passage in a diffed document, with whole-word cuts
/// (`documentChange.ts::documentChanges`).
pub fn document_changes(file: &File) -> Vec<ChangedPassage> {
    let mut passages = Vec::new();
    let push = |passages: &mut Vec<ChangedPassage>, passage: ChangedPassage| {
        let empty = match &passage {
            ChangedPassage::Changed { before, removed, added, after } => {
                before.is_empty() && removed.is_empty() && added.is_empty() && after.is_empty()
            }
            ChangedPassage::Added { text } | ChangedPassage::Removed { text } => text.is_empty(),
        };
        if !empty {
            passages.push(passage);
        }
    };

    for hunk in &file.hunks {
        let rows = &hunk.rows;
        let mut index = 0;
        while index < rows.len() {
            if rows[index].kind == ' ' {
                index += 1;
                continue;
            }
            let mut dels = Vec::new();
            while index < rows.len() && rows[index].kind == '-' {
                dels.push(as_read(&rows[index].text));
                index += 1;
            }
            let mut adds = Vec::new();
            while index < rows.len() && rows[index].kind == '+' {
                adds.push(as_read(&rows[index].text));
                index += 1;
            }
            let pairs = dels.len().min(adds.len());
            for pair in 0..pairs {
                let old = &dels[pair];
                let next = &adds[pair];
                if old == next {
                    continue;
                }
                let (_, next_spans) = changes::word_diff(old, next);
                let old_chars: Vec<char> = old.chars().collect();
                let next_chars: Vec<char> = next.chars().collect();
                let mut head = if next_spans.first().is_some_and(|s| !s.changed) {
                    next_spans[0].text.chars().count()
                } else {
                    0
                };
                let mut tail = if next_spans.len() > 1 && next_spans.last().is_some_and(|s| !s.changed) {
                    next_spans.last().unwrap().text.chars().count()
                } else {
                    0
                };
                // Whole words: a cut inside a word moves out to the word's edge.
                while head > 0
                    && is_word_char(old_chars.get(head - 1))
                    && (is_word_char(old_chars.get(head)) || is_word_char(next_chars.get(head)))
                {
                    head -= 1;
                }
                while tail > 0
                    && is_word_char(old_chars.get(old_chars.len() - tail))
                    && (old_chars
                        .len()
                        .checked_sub(tail + 1)
                        .is_some_and(|i| is_word_char(old_chars.get(i)))
                        || next_chars
                            .len()
                            .checked_sub(tail + 1)
                            .is_some_and(|i| is_word_char(next_chars.get(i))))
                {
                    tail -= 1;
                }
                let before: String = old_chars[..head].iter().collect();
                let removed: String = old_chars[head..old_chars.len() - tail].iter().collect();
                let added: String = next_chars[head..next_chars.len() - tail].iter().collect();
                let after: String = if tail == 0 {
                    String::new()
                } else {
                    old_chars[old_chars.len() - tail..].iter().collect()
                };
                push(
                    &mut passages,
                    ChangedPassage::Changed { before, removed, added, after },
                );
            }
            for text in dels.into_iter().skip(pairs) {
                push(&mut passages, ChangedPassage::Removed { text });
            }
            for text in adds.into_iter().skip(pairs) {
                push(&mut passages, ChangedPassage::Added { text });
            }
        }
    }
    passages
}

#[cfg(test)]
#[path = "document_change_tests.rs"]
mod tests;

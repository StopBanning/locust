//! A tidy pass (Electron `shared/memory-tidy.ts`, and `stripMemoryBlocks` from `shared/memory.ts`): a teammate reads
//! the folder's memories and suggests merging, retiring or rewriting some. Nothing here changes a memory: each
//! suggestion becomes a proposal that waits for the person's Keep (`people::propose_tidy`).
//!
//! The teammate names memories by the ids it reads in `.locust/memory.md`, not by quoting them: a quote is matched
//! loosely, an id exactly, and a merge has to name two or more things without doubt.

use crate::invariants::regex;
use crate::protocol_tags::strip_blocks_outside_code;

pub const TIDY_TAG: &str = "locust-tidy";
/// At most this many suggestions are taken from one reply: a person answers each one.
pub const MAX_TIDY_SUGGESTIONS: usize = 12;
/// A merge joins two to this many memories.
pub const MAX_MERGED: usize = 6;
/// Past this many memories in a folder, the Memory screen says a tidy pass would help.
pub const TIDY_NUDGE_AT: usize = 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TidySuggestion {
    Merge { ids: Vec<String>, text: String },
    Retire { id: String, reason: String },
    Rewrite { id: String, text: String },
}

/// The brief's own example, line by line. A reply that repeats the brief proposes nothing: these lines are skipped
/// wherever they appear.
pub const TIDY_EXAMPLE_LINES: [&str; 3] = [
    "merge mem_a mem_b :: the one sentence that replaces them",
    "retire mem_c :: why it no longer holds",
    "rewrite mem_d :: the corrected sentence",
];

/// The suggestions in a reply, in order. A line that does not read, names the wrong number of ids, names one twice,
/// or repeats an earlier suggestion is dropped. Read inside a code fence too (0.372): the model copied the brief's
/// fence, and a pass that proposed nothing left the person an empty box; the brief's own example is skipped by its
/// lines instead.
pub fn parse_tidy_blocks(text: &str) -> Vec<TidySuggestion> {
    read_tidy_blocks(text).0
}

/// The suggestions, and how many lines in the block could not be read as one (blank lines, the brief's own example,
/// a repeat and lines past the limit are not counted: none of them is a suggestion that went missing).
pub fn read_tidy_blocks(text: &str) -> (Vec<TidySuggestion>, usize) {
    let block = regex(r"<locust-tidy\s*>([\s\S]*?)</locust-tidy>");
    // A bullet or a number in front is how a model writes a list; it is not part of the line.
    let line = regex(r"(?i)^\s*(?:[-*•]\s*|\d{1,2}[.)]\s*)?(merge|retire|rewrite)\s+([^:]*?)\s*::\s*(.+?)\s*$");
    let id = regex(r"mem_[A-Za-z0-9_-]{1,60}");
    let mut found = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut unread = 0;
    for caps in block.captures_iter(text) {
        for raw in caps[1].split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)) {
            if raw.trim().is_empty() || TIDY_EXAMPLE_LINES.contains(&raw.trim()) {
                continue;
            }
            if found.len() >= MAX_TIDY_SUGGESTIONS {
                return (found, unread);
            }
            let Some(l) = line.captures(raw) else {
                unread += 1;
                continue;
            };
            let verb = l[1].to_lowercase();
            let ids: Vec<String> = id.find_iter(&l[2]).map(|m| m.as_str().to_owned()).collect();
            let rest = crate::people::memory_text(&l[3]);
            let distinct: std::collections::HashSet<&String> = ids.iter().collect();
            if rest.is_empty() || distinct.len() != ids.len() {
                unread += 1;
                continue;
            }
            // The same suggestion twice -- a block shown, then written -- is asked once.
            let key = format!("{verb} {}", ids.join(" "));
            if seen.contains(&key) {
                continue;
            }
            if verb == "merge" && (2..=MAX_MERGED).contains(&ids.len()) {
                found.push(TidySuggestion::Merge { ids, text: rest });
            } else if verb != "merge" && ids.len() == 1 {
                let id = ids.into_iter().next().unwrap_or_default();
                found.push(if verb == "retire" { TidySuggestion::Retire { id, reason: rest } } else { TidySuggestion::Rewrite { id, text: rest } });
            } else {
                unread += 1;
                continue;
            }
            seen.insert(key);
        }
    }
    (found, unread)
}

/// What the teammate is asked, as the person's message on its turn. The example is shown exactly as the block should
/// be written -- not in a code fence, which the model copied (0.372).
pub fn tidy_prompt() -> String {
    [
        "Tidy this folder's team memory. Read .locust/memory.md -- every memory is listed there, with its id in square brackets at the end of its line.",
        "Suggest at most 10 changes, and only ones you are sure of:",
        "- merge two or more memories that say the same thing, as one sentence;",
        "- retire a memory that is stale, superseded by another, or no longer true, and say why in plain words -- one marked \"may be out of date\" names a file that changed after it was written, so check that file, and one marked \"not given to a teammate in N days\" may no longer matter;",
        "- rewrite a memory that is unclear or contradicts another, as the corrected sentence.",
        "Change nothing yourself: every suggestion waits for the person, who keeps it or not. The ids are for the block only; the person reads your reasons, so write them without ids.",
        "Say in a sentence or two what you found, then end your reply with a block in this form, one suggestion per line, each id exactly as the file shows it:",
        "",
        "<locust-tidy>",
        TIDY_EXAMPLE_LINES[0],
        TIDY_EXAMPLE_LINES[1],
        TIDY_EXAMPLE_LINES[2],
        "</locust-tidy>",
        "",
        "Write the block in your reply itself -- not in a code block, a command or a file. If nothing needs tidying, say so and leave the block out.",
    ]
    .join("\n")
}

/// Whether a run was asked for a tidy pass, by the brief's first line, which every version of the brief has kept.
pub fn is_tidy_prompt(prompt: Option<&str>) -> bool {
    let full = tidy_prompt();
    let opening = full.split('\n').next().unwrap_or(&full);
    prompt.is_some_and(|p| p.starts_with(opening))
}

/// A retirement's reason, as the person reads it: a cited id in brackets is dropped (the sentence already says it in
/// words); a bare one becomes the memory it names, quoted, or "another memory" when it is not one kept here.
pub fn readable_reason(reason: &str, memories: &[(&str, &str)]) -> String {
    let cited = regex(r"\s*\(\s*mem_[A-Za-z0-9_-]+(?:\s*[,;]\s*mem_[A-Za-z0-9_-]+)*\s*\)").replace_all(reason, "");
    let named = regex(r"\bmem_[A-Za-z0-9_-]+").replace_all(&cited, |c: &regex::Captures| match memories.iter().find(|(id, _)| *id == &c[0]) {
        Some((_, text)) => format!("\"{text}\""),
        None => "another memory".to_owned(),
    });
    regex(r"\s+([.,;:!?])").replace_all(&named, "$1").trim().to_owned()
}

/// The reply a person reads: memory blocks outside code taken out (one in a fence was not acted on, so it stays), and
/// every tidy block taken out, fenced or not, with the empty fence it leaves behind (`stripMemoryBlocks`).
pub fn strip_memory_blocks(text: &str) -> String {
    let kept = strip_blocks_outside_code(text, &regex(r"<locust-memory\s*>([\s\S]*?)</locust-memory>"));
    let kept = regex(r"<locust-tidy\s*>[\s\S]*?</locust-tidy>").replace_all(&kept, "");
    // Electron's EMPTY_FENCE ends in a lookahead for a newline or the end; the newline is captured and put back.
    let kept = regex(r"(^|\n)[ \t]*```[^\n`]*\n\s*```[ \t]*(\n|$)").replace_all(&kept, "$1$2");
    regex(r"\n{3,}").replace_all(&kept, "\n\n").trim_end().to_owned()
}

#[cfg(test)]
#[path = "memory_tidy_tests.rs"]
mod tests;

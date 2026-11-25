//! Files a person attached to a message, said in a way every runtime can act on (Electron's `shared/attachments.ts`).
//!
//! The message names the paths and asks the runtime to read them: it never carries a file's contents (a path costs a
//! line), and it never claims more than happened ("read these files", not "attached"). Lengths are counted as
//! JavaScript counts them, in UTF-16 units, so a prompt that fits in Electron fits here.

use crate::invariants::regex;

/// The most files one message may carry.
pub const MAX_ATTACHMENTS: usize = 8;

/// The most one pasted file may weigh: a full 4K PNG is well under it.
pub const MAX_PASTED_BYTES: usize = 24 * 1024 * 1024;

/// Where a file copied in from outside the workspace lives, workspace-relative.
pub const ATTACHMENT_DIR: &str = ".locust/attachments";

/// Node's `path.basename` for the platform: the last segment once trailing separators are gone (`/` everywhere, a backslash
/// too on Windows). Unlike `Path::file_name`, a last segment of `.` or `..` is returned as written.
pub(crate) fn basename(source: &str) -> &str {
    let separator = |c: char| c == '/' || (cfg!(windows) && c == '\\');
    let trimmed = source.trim_end_matches(separator);
    if trimmed.is_empty() {
        return "";
    }
    trimmed.rsplit(separator).next().unwrap_or(trimmed)
}

/// Node's `path.extname`: from the last dot of the name, unless that dot is its first character.
pub(crate) fn extname(name: &str) -> &str {
    match name.rfind('.') {
        Some(dot) if dot > 0 => &name[dot..],
        _ => "",
    }
}

/// `attachmentDestination` (attach-outside.ts): a workspace-relative destination for a file copied in from outside the
/// workspace, or pasted, that collides with nothing in `taken`. The name has to be a NAME, not a way out of the folder:
/// empty, `.`, `..`, or anything holding a slash, a backslash or a colon (a Windows alternate data stream) is refused,
/// whoever the caller is. A taken name gets `-2`, `-3` before its extension.
pub fn attachment_destination(source: &str, taken: &std::collections::HashSet<String>) -> Result<String, String> {
    let name = basename(source);
    if name.is_empty() || name == "." || name == ".." || name.contains(['\\', '/', ':']) {
        return Err(format!("That file cannot be attached: \"{name}\" is not a file name."));
    }
    if !taken.contains(name) {
        return Ok(format!("{ATTACHMENT_DIR}/{name}"));
    }
    let extension = extname(name);
    let stem = &name[..name.len() - extension.len()];
    // Starts at 2, because the file it is avoiding is the first one.
    (2..1_000)
        .map(|suffix| format!("{stem}-{suffix}{extension}"))
        .find(|candidate| !taken.contains(candidate))
        .map(|candidate| format!("{ATTACHMENT_DIR}/{candidate}"))
        .ok_or_else(|| format!("Too many files named {name} are already attached."))
}

/// What `.git/info/exclude` needs so the folder never reaches a commit (`GIT_EXCLUDE_LINE`).
pub const GIT_EXCLUDE_LINE: &str = ".locust/";

/// `excludeWith` (attach-outside.ts): the exclude file with `.locust/` added, or None when it already keeps it out.
pub fn exclude_with(current: &str) -> Option<String> {
    if current.split('\n').map(str::trim).any(|line| matches!(line, ".locust/" | ".locust" | "/.locust/")) {
        return None;
    }
    let body = if current.is_empty() || current.ends_with('\n') { current.to_owned() } else { format!("{current}\n") };
    Some(format!("{body}{GIT_EXCLUDE_LINE}\n"))
}

/// The line that goes above the person's own words, or None for no attachments, so a caller cannot prepend an empty
/// instruction. A name holding a control character (legal on macOS and Linux) is quoted with its breaks escaped, so it
/// stays one item and says exactly what the file is called.
pub fn attachment_preamble(paths: &[String]) -> Option<String> {
    if paths.is_empty() {
        return None;
    }
    let listed = paths
        .iter()
        .map(|path| {
            if path.chars().any(|c| c <= '\u{001f}' || c == '\u{007f}') {
                format!("- {}", serde_json::to_string(path).unwrap_or_else(|_| path.clone()))
            } else {
                format!("- {path}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some(if paths.len() == 1 {
        format!("Read this file in the workspace before you answer:\n{listed}")
    } else {
        format!("Read these {} files in the workspace before you answer:\n{listed}", paths.len())
    })
}

/// The prompt actually sent: the preamble, a blank line, then what was typed. `note`, when given and not empty, goes
/// between the two: what Locust did to make the files readable (`readable_pdf_note`). It is said only to the runtime;
/// the recorded prompt never carries one, so `split_attachments` never meets it.
pub fn with_attachments_note(prompt: &str, paths: &[String], note: Option<&str>) -> String {
    let Some(preamble) = attachment_preamble(paths) else {
        return prompt.to_owned();
    };
    match note {
        None | Some("") => format!("{preamble}\n\n{prompt}"),
        Some(note) => format!("{preamble}\n\n{note}\n\n{prompt}"),
    }
}

/// `with_attachments_note` with no note.
pub fn with_attachments(prompt: &str, paths: &[String]) -> String {
    with_attachments_note(prompt, paths, None)
}

/// What the composer chip says: "1 file", never "attached", because on most runtimes nothing is.
pub fn attachment_label(count: usize) -> String {
    if count == 1 {
        "1 file".into()
    } else {
        format!("{count} files")
    }
}

/// The inverse of `with_attachments`, for everything that SHOWS a prompt: the person's own words, and the paths in the
/// order they were attached. It matches only what `attachment_preamble` writes (the exact opening, a list of `- `
/// lines, a blank line, then the rest); anything else comes back whole, because a prompt this cannot account for is a
/// prompt to leave alone.
///
/// Electron's regular expression, character for character. The regex crate takes the same leftmost-first match as
/// JavaScript's backtracking engine, which matters: a preamble over an empty message ends `\n\n`, and the match gives
/// the last `\n` back to the list so the rest is that one newline.
pub fn split_attachments(prompt: &str) -> (String, Vec<String>) {
    let re = regex(r"^Read th(?:is file|ese [0-9]+ files) in the workspace before you answer:\n((?:- [^\n]+\n?)+)\n([\s\S]+)$");
    let Some(found) = re.captures(prompt) else {
        return (prompt.to_owned(), Vec::new());
    };
    let listed: Vec<String> = found[1]
        .split('\n')
        .filter(|line| line.starts_with("- "))
        .map(|line| line[2..].to_owned())
        .collect();
    // Electron also returns the prompt whole "when a preamble has nothing under it" (`listed.length === 0 || rest.length
    // === 0`). The expression cannot get there: it needs one `- ` line and `[\s\S]+` needs one character, so a preamble
    // with nothing under it never matches at all (the last test of `refuses_to_strip_something_it_did_not_write`).
    (found[2].to_owned(), listed)
}

#[cfg(test)]
#[path = "attachments_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "pasted_attachment_tests.rs"]
mod pasted_attachment_tests;

#[cfg(test)]
#[path = "attach_outside_tests.rs"]
mod attach_outside_tests;

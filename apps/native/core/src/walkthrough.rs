//! Walk me through this change (Electron's `shared/walkthrough.ts`, 0.771).
//!
//! "Walk me through it" asks the teammate whose conversation it is -- its own agent and model, in Ask mode -- to go
//! through the change the Changes panel shows as its author would in a review: what it does, then each file in the
//! order to read it, why it changed, and what to check. The panel then shows the change that way. It changes nothing.

/// How much of the change the model reads: past this, its files and their first 40,000 characters.
pub const MAX_WALK_DIFF_CHARS: usize = 40_000;
const MAX_SUMMARY_CHARS: usize = 700;
const MAX_NOTE_CHARS: usize = 400;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalkthroughFile {
    /// The file's path, as the panel lists it.
    pub path: String,
    /// What changed in it, and why.
    pub why: String,
    /// One thing worth checking or testing there.
    pub check: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Walkthrough {
    pub summary: String,
    /// The files in the order to read them; a file it did not mention is not here.
    pub files: Vec<WalkthroughFile>,
}

/// `clip`: kept whole up to `max` UTF-16 units, else cut to one less, trimmed, and an ellipsis added.
fn clip(text: &str, max: usize) -> String {
    if text.encode_utf16().count() <= max {
        return text.to_owned();
    }
    let mut units = 0;
    let mut end = 0;
    for (i, c) in text.char_indices() {
        if units + c.len_utf16() > max - 1 {
            break;
        }
        units += c.len_utf16();
        end = i + c.len_utf8();
    }
    format!("{}…", text[..end].trim_end())
}

/// JavaScript's `\s+` folded to one space.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The paths a unified diff changes, as `diff --git a/x b/y` names them: the `b` side, each once, in order.
pub fn changed_paths(diff: &str) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for line in diff.split('\n') {
        let Some(rest) = line.strip_prefix("diff --git a/") else { continue };
        // `a\/(.+?) b\/(.+)$`: the first " b/" after at least one character.
        let Some(at) = rest.get(1..).and_then(|r| r.find(" b/")).map(|i| i + 1) else { continue };
        let path = rest[at + 3..].trim();
        if !path.is_empty() && !seen.iter().any(|p| p == path) {
            seen.push(path.to_owned());
        }
    }
    seen
}

/// The prompt for one walkthrough (Electron's `walkthroughPrompt`).
pub fn walkthrough_prompt(diff: &str, cut: bool, paths: &[String], asks: &[String], said: Option<&str>) -> String {
    let asked: Vec<String> = asks.iter().map(|a| one_line(a)).filter(|a| !a.is_empty()).collect();
    let asked = &asked[asked.len().saturating_sub(5)..];
    let too_long = diff.encode_utf16().count() > MAX_WALK_DIFF_CHARS;
    let shown = if too_long { utf16_prefix(diff, MAX_WALK_DIFF_CHARS) } else { diff };
    let cut = cut || too_long;
    let mut parts: Vec<String> = vec![
        "Walk the person through this change as its author would in a review. Use only what is here: do not read files or run commands.".into(),
        [
            "- First, in two or three sentences, what the change does and why.",
            "- Then each file, in the order a reviewer should read them: what changed in it and why, in one or two sentences, and one thing worth checking or testing there, if there is one.",
            "- Name each file by its path exactly as it is listed below. Keep to what is in the change.",
        ]
        .join("\n"),
    ];
    if !asked.is_empty() {
        let list: Vec<String> = asked.iter().map(|a| format!("- {}", clip(a, 300))).collect();
        parts.push(format!("What the person asked for:\n{}", list.join("\n")));
    }
    if let Some(said) = said.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("What was said when the work was done:\n{}", clip(said, 2_000)));
    }
    let files: Vec<String> = paths.iter().map(|p| format!("- {p}")).collect();
    parts.push(format!("The files changed:\n{}", files.join("\n")));
    parts.push(format!(
        "The change{}:\n{shown}",
        if cut { " (cut short; the list of files above is whole)" } else { "" }
    ));
    parts.push("Answer in exactly this shape, and nothing else:\nSUMMARY: <what the change does and why>\nFILE: <a path from the list>\nWHY: <what changed in it and why>\nCHECK: <one thing to check there, or leave this line out>\n(FILE, WHY and CHECK again for each file, in reading order)\nEND".into());
    parts.join("\n\n")
}

fn utf16_prefix(text: &str, max: usize) -> &str {
    let mut units = 0;
    for (i, c) in text.char_indices() {
        if units + c.len_utf16() > max {
            return &text[..i];
        }
        units += c.len_utf16();
    }
    text
}

/// `\s*[*_]*\s*`: spaces, one run of emphasis marks, spaces.
fn skip_emphasis(text: &str) -> &str {
    text.trim_start().trim_start_matches(['*', '_']).trim_start()
}

/// `LABEL`: SUMMARY, FILE, WHY or CHECK, any case, wrapped in emphasis or not. A label needs its colon: a line of a
/// note that begins "Check the..." or "End users..." is still the note.
fn label_of(line: &str) -> Option<(&'static str, String)> {
    let start = skip_emphasis(line);
    for label in ["SUMMARY", "FILE", "WHY", "CHECK"] {
        let Some(head) = start.get(..label.len()) else { continue };
        if !head.eq_ignore_ascii_case(label) {
            continue;
        }
        let Some(rest) = skip_emphasis(&start[label.len()..]).strip_prefix(':') else { continue };
        return Some((label, skip_emphasis(rest).to_owned()));
    }
    None
}

/// `END`: the word alone on its line, wrapped in emphasis or not.
fn is_end(line: &str) -> bool {
    let start = skip_emphasis(line);
    start.get(..3).is_some_and(|w| w.eq_ignore_ascii_case("END")) && skip_emphasis(&start[3..]).is_empty()
}

/// A path the model named, as one the panel lists: exactly, by its end, or by a name only one file has.
fn known_path(named: &str, paths: &[String]) -> Option<String> {
    let mut clean = named.trim_end();
    // `\s+\([^)]*\)\s*$`: a trailing "(modified)".
    if clean.ends_with(')')
        && let Some(open) = clean.rfind('(')
        && !clean[open + 1..clean.len() - 1].contains(')')
        && clean[..open].ends_with(char::is_whitespace)
    {
        clean = clean[..open].trim_end();
    }
    let clean = clean.trim_matches(|c| c == '`' || c == '\'' || c == '"');
    let clean = clean.strip_prefix('.').filter(|r| r.starts_with('/')).unwrap_or(clean);
    let clean = clean.trim_start_matches('/');
    let clean = clean.strip_prefix("a/").or_else(|| clean.strip_prefix("b/")).unwrap_or(clean).trim();
    if clean.is_empty() {
        return None;
    }
    if paths.iter().any(|p| p == clean) {
        return Some(clean.to_owned());
    }
    let ending: Vec<&String> = paths.iter().filter(|p| p.ends_with(&format!("/{clean}"))).collect();
    if ending.len() == 1 {
        return Some(ending[0].clone());
    }
    let base = clean.rsplit('/').next().unwrap_or(clean);
    let same: Vec<&String> = paths.iter().filter(|p| p.rsplit('/').next().unwrap_or(p) == base).collect();
    if same.len() == 1 { Some(same[0].clone()) } else { None }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    Summary,
    Why,
    Check,
}

struct Open {
    path: Option<String>,
    why: String,
    check: String,
}

/// The walkthrough in an answer, or none when it is not in the shape asked for (Electron's `readWalkthrough`). Each
/// file is one the change has, once, in the order given; what a model says around the shape, and a file it invented,
/// are left out.
pub fn read_walkthrough(answer: &str, paths: &[String]) -> Option<Walkthrough> {
    let mut summary = String::new();
    let mut files: Vec<WalkthroughFile> = Vec::new();
    let mut current: Option<Open> = None;
    let mut field: Option<Field> = None;
    let close = |current: &mut Option<Open>, files: &mut Vec<WalkthroughFile>| {
        if let Some(open) = current.take()
            && let Some(path) = open.path
            && !open.why.trim().is_empty()
            && !files.iter().any(|f| f.path == path)
        {
            let check = open.check.trim();
            files.push(WalkthroughFile {
                path,
                why: clip(&one_line(&open.why), MAX_NOTE_CHARS),
                check: (!check.is_empty()).then(|| clip(&one_line(check), MAX_NOTE_CHARS)),
            });
        }
    };
    let text = answer.replace("\r\n", "\n").replace('\r', "\n");
    for line in text.split('\n') {
        if is_end(line) {
            break;
        }
        match label_of(line) {
            Some(("SUMMARY", rest)) => {
                field = Some(Field::Summary);
                summary = rest;
            }
            Some(("FILE", rest)) => {
                close(&mut current, &mut files);
                current = Some(Open { path: known_path(&rest, paths), why: String::new(), check: String::new() });
                field = None;
            }
            Some(("WHY", rest)) if current.is_some() => {
                field = Some(Field::Why);
                if let Some(open) = current.as_mut() {
                    open.why = rest;
                }
            }
            Some(("CHECK", rest)) if current.is_some() => {
                field = Some(Field::Check);
                if let Some(open) = current.as_mut() {
                    open.check = rest;
                }
            }
            _ => match (field, current.as_mut()) {
                (Some(Field::Summary), _) => {
                    summary.push('\n');
                    summary.push_str(line);
                }
                (Some(Field::Why), Some(open)) => {
                    open.why.push('\n');
                    open.why.push_str(line);
                }
                (Some(Field::Check), Some(open)) => {
                    open.check.push('\n');
                    open.check.push_str(line);
                }
                _ => {}
            },
        }
    }
    close(&mut current, &mut files);
    let said = one_line(&summary);
    if said.is_empty() {
        return None;
    }
    Some(Walkthrough { summary: clip(&said, MAX_SUMMARY_CHARS), files })
}

/// The order the panel shows the files in, as indexes into `paths`: walked through, each file the walk names in its
/// place in the reading, then the ones it did not mention in git's order (ChangesPanel.tsx `files`).
pub fn reading_order(paths: &[&str], walk: Option<&Walkthrough>) -> Vec<usize> {
    let mut order: Vec<usize> = (0..paths.len()).collect();
    if let Some(walk) = walk {
        let rank = |i: usize| walk.files.iter().position(|f| f.path == paths[i]).unwrap_or(10_000 + i);
        order.sort_by_key(|&i| rank(i));
    }
    order
}

/// The note on a file, and its place in the reading (1-based), when the walk names it.
pub fn note_on<'a>(walk: &'a Walkthrough, path: &str) -> Option<(usize, &'a WalkthroughFile)> {
    walk.files.iter().position(|f| f.path == path).map(|i| (i + 1, &walk.files[i]))
}

/// Walk me through this change (Electron's `folder:walkthrough` handler): one short read-only run of the conversation's
/// teammate, given the change the Changes panel shows (all of it, or one commit) with what was asked and what the
/// teammate said when it finished. Never a conversation (`native::run_once` keeps it in panel-runs). Blocks: call it
/// off the window's thread. It changes nothing.
pub fn walk(
    folder: &std::path::Path,
    sha: Option<&str>,
    asks: &[String],
    said: Option<&str>,
    teammate: &crate::engine::Teammate,
) -> Result<Walkthrough, String> {
    let no = |why: &str| format!("{} The changes are as they were.", why.trim_end());
    let one = sha.filter(|s| (40..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    let change = crate::changes::read(folder, one).ok();
    let paths = change.as_ref().map(|c| changed_paths(&c.raw)).unwrap_or_default();
    let Some(change) = change.filter(|_| !paths.is_empty()) else {
        return Err(no("There is no change here to walk through."));
    };
    let asks = &asks[asks.len().saturating_sub(5)..];
    let prompt = walkthrough_prompt(&change.raw, change.truncated, &paths, asks, said);
    let answer = crate::native::run_once(teammate, folder, &prompt).map_err(|why| no(&why))?;
    read_walkthrough(&answer, &paths).ok_or_else(|| no("It did not answer with a walkthrough."))
}

#[cfg(test)]
#[path = "walkthrough_tests.rs"]
mod tests;

//! Which `.cursorignore` rule hides a path from Cursor, if one does (Electron's `shared/cursorIgnore.ts` and
//! `main/cursor-visibility.ts`).
//!
//! Cursor's file tools refuse an ignored path and say only "permission denied", so the model invents a reason and the
//! person chases the invention. Locust can see the rule, so a sentence naming it replaces an afternoon.
//!
//! WHAT IT CHECKS, and what it does not. Directory rules only, in the two shapes these files carry: `name/` and
//! `name/*`. The second matters because it is the shape you MUST use to re-include anything (gitignore cannot negate
//! inside an excluded directory), so every fix produced a rule an older matcher could no longer see. A `!` line for the
//! same path cancels a hit, because that is exactly what the fixes look like. A quiet return means "no directory rule
//! matched", NEVER "Cursor can certainly see this": file globs are not read here.

use crate::invariants::regex;
use std::path::{Path, PathBuf};

/// The rule that hides a path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CursorIgnoreHit {
    /// The rule line, as written, so a person can find it in the file.
    pub rule: String,
    /// The directory it names, for the sentence.
    pub directory: String,
    /// Whether the rule is already the `dir/*` shape. It decides the advice: 0.87.0 told Colin to change `.claude/*`
    /// to `.claude/*`.
    pub already_narrowed: bool,
}

struct Rule {
    line: String,
    name: String,
    negated: bool,
    narrowed: bool,
}

/// JavaScript's `String.prototype.trim`: its white space and line terminators, which is not Rust's `trim` (that one
/// drops U+0085 and keeps U+FEFF, the byte-order mark an editor puts at the head of a file).
fn js_trim(text: &str) -> &str {
    let space = |c: char| {
        matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
    };
    text.trim_matches(space)
}

/// Either separator. (Electron writes the backslash as an escape so a shell cannot halve it: a Windows path must
/// split.)
fn names(path: &str) -> Vec<&str> {
    path.split(['/', '\\']).filter(|part| !part.is_empty()).collect()
}

fn rule_of(line: &str) -> Option<Rule> {
    let trimmed = js_trim(line);
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let negated = trimmed.starts_with('!');
    let body = if negated { &trimmed[1..] } else { trimmed };
    let body = body.strip_prefix("**/").unwrap_or(body);
    // `name/` and `name/*` are the two directory shapes. Anything else (a bare name, a glob over files) is outside what
    // this claims to read. `.` in JavaScript stops at every line terminator, not only `\n`.
    let found = regex(r"^([^\n\r\u{2028}\u{2029}]+?)/(\*)?$").captures(body)?;
    let name = found[1].to_owned();
    if name.is_empty() {
        return None;
    }
    Some(Rule { line: trimmed.to_owned(), name, negated, narrowed: found.get(2).is_some() })
}

/// Whether `wanted` appears as consecutive segments of `segments`.
fn contains_in_order(segments: &[&str], wanted: &[&str]) -> bool {
    if wanted.is_empty() || wanted.len() > segments.len() {
        return false;
    }
    segments.windows(wanted.len()).any(|window| window == wanted)
}

/// The rule that hides `path`, when one does. Later rules win (gitignore's own rule, and how a fix is written: the
/// exclusion first, the re-inclusion after it).
pub fn cursor_ignore_hit(path: &str, ignore_text: &str) -> Option<CursorIgnoreHit> {
    let segments = names(path);
    let mut hit = None;
    for raw in ignore_text.split('\n') {
        let Some(rule) = rule_of(raw) else { continue };
        // The rule may name a path (`.cursor/projects/`) or a single directory (`node_modules/`); both are a hit when
        // every part of it is on this path in order.
        let wanted = names(&rule.name);
        if !contains_in_order(&segments, &wanted) {
            continue;
        }
        hit = if rule.negated {
            None
        } else {
            Some(CursorIgnoreHit { rule: rule.line, directory: rule.name, already_narrowed: rule.narrowed })
        };
    }
    hit
}

/// The sentence a person can act on. It names the rule and the file, because "Cursor cannot read this folder" on its
/// own is the same dead end the runtime already produces.
pub fn cursor_ignore_sentence(hit: &CursorIgnoreHit, ignore_path: &str) -> String {
    let said = format!(
        "Cursor cannot read files here: {ignore_path} hides this folder with the rule \"{}\". Its tools will answer \"permission denied\" and give no reason.",
        hit.rule
    );
    // Two different situations, and one piece of advice cannot serve both. A bare `dir/` cannot be negated at all, so
    // the rule itself has to change; a `dir/*` already can be, so what is missing is the line that lets this folder
    // back in.
    if hit.already_narrowed {
        format!("{said} Add \"!{}/<the folder you need>\" beneath that rule, or remove it.", hit.directory)
    } else {
        format!(
            "{said} Change that rule to \"{0}/*\" and add \"!{0}/<the folder you need>\" beneath it, or remove it.",
            hit.directory
        )
    }
}

/// Where the rules can live, nearest first.
pub fn cursor_ignore_paths(cwd: &Path, home: Option<&Path>) -> Vec<PathBuf> {
    let mut paths = vec![cwd.join(".cursorignore")];
    if let Some(home) = home {
        paths.push(home.join(".cursorignore"));
    }
    paths
}

/// Say when Cursor has been told not to look at the folder it was given: the sentence for the first rules file (the
/// folder's own, then the home folder's) that hides it, or None. A file that cannot be read says nothing.
pub fn cursor_cannot_see(cwd: &Path, home: Option<&Path>, read: &dyn Fn(&Path) -> Option<String>) -> Option<String> {
    for path in cursor_ignore_paths(cwd, home) {
        let Some(text) = read(&path) else { continue };
        if let Some(hit) = cursor_ignore_hit(&cwd.to_string_lossy(), &text) {
            return Some(cursor_ignore_sentence(&hit, &path.to_string_lossy()));
        }
    }
    None
}

/// Whether a failed tool's record says Cursor was refused (the raw record of a failed tool carries the error message,
/// unredacted): `/permission denied/i` over its JSON.
pub fn says_permission_denied(raw: &serde_json::Value) -> bool {
    raw.to_string().to_lowercase().contains("permission denied")
}

/// The code the notice is raised under. `runtime_error` is deliberate and load-bearing: the thread drops any diagnostic
/// raised before the first tool call unless its code ends in `runtime_error` or `notification`.
pub const NOTICE_CODE: &str = "host.cursorignore.runtime_error";

/// The notice as a record: a warning, not an error (the run may well do useful work; it simply cannot read this
/// folder's files, and every failure it reports about them will name the wrong cause), not terminal, its evidence
/// redacted.
pub fn notice_payload(sentence: &str) -> serde_json::Value {
    serde_json::json!({"level": "warning", "code": NOTICE_CODE, "message": sentence, "terminal": false, "evidence": {"redacted": true}})
}

/// Say why a read was refused, AT THE MOMENT IT IS REFUSED (Electron's `explainADeniedRead`). Cursor obeys
/// `.cursorignore` and its tools answer "permission denied" with no reason, so the model invents one and the person
/// chases the invention. Not preemptively: a rule that hides a folder only matters when something is actually refused,
/// and a run that never opens a file never hears about it. Once per turn: the second denial has the same cause.
///
/// It wraps the Cursor runtime and changes nothing about it: every call goes through, and after a failed tool whose
/// record says "permission denied" the sentence follows that event, beside the failure it explains.
pub struct ExplainsDeniedReads<'a> {
    inner: &'a mut dyn crate::runtime::Runtime,
    folder: PathBuf,
    home: Option<PathBuf>,
    said: bool,
    queued: Option<crate::runtime::Event>,
}

impl<'a> ExplainsDeniedReads<'a> {
    pub fn new(inner: &'a mut dyn crate::runtime::Runtime, folder: &Path, home: Option<PathBuf>) -> Self {
        Self { inner, folder: folder.to_path_buf(), home, said: false, queued: None }
    }

    fn explain(&mut self, event: &crate::runtime::Event) {
        if self.said || event.kind != "tool.failed" {
            return;
        }
        let raw = &event.payload["evidence"]["raw"];
        if !says_permission_denied(raw) {
            return;
        }
        self.said = true;
        let read = |path: &Path| std::fs::read_to_string(path).ok();
        if let Some(sentence) = cursor_cannot_see(&self.folder, self.home.as_deref(), &read) {
            self.queued = Some(crate::runtime::Event { kind: "adapter.diagnostic".into(), payload: notice_payload(&sentence), thread: event.thread.clone() });
        }
    }
}

impl crate::runtime::Runtime for ExplainsDeniedReads<'_> {
    fn start(&mut self, turn: &crate::runtime::Turn) -> std::io::Result<()> {
        self.inner.start(turn)
    }
    fn next_event(&mut self) -> std::io::Result<Option<crate::runtime::Event>> {
        if let Some(notice) = self.queued.take() {
            return Ok(Some(notice));
        }
        let event = self.inner.next_event()?;
        if let Some(event) = &event {
            self.explain(event);
        }
        Ok(event)
    }
    fn stop(&mut self) -> std::io::Result<()> {
        self.inner.stop()
    }
    fn can_steer(&self) -> bool {
        self.inner.can_steer()
    }
    fn learned_commands(&mut self) -> Option<Vec<crate::runtime_commands::CommandInfo>> {
        self.inner.learned_commands()
    }
    fn steer(&mut self, text: &str) -> bool {
        self.inner.steer(text)
    }
}

#[cfg(test)]
#[path = "cursor_ignore_tests.rs"]
mod tests;

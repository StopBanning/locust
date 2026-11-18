//! A reply's words, made ready for Markdown: the steps `reply_markdown` runs before drawing, as one function, and the
//! rules Electron's `agentText.ts` reads a reply by where CommonMark reads it another way.
//!
//! rune-kit's Markdown is CommonMark (pulldown-cmark). It already keeps a wrapped bullet one bullet, nests by indent,
//! keeps numbered steps with blank lines between them one list, and resumes a list's numbering after a code block
//! (`a-wrapped-bullet-stays-one-bullet.test.ts`, `nested-lists.test.ts`, `a-blank-line-does-not-end-a-list.test.tsx`).
//! Where the two differ, the reply is rewritten here before it is drawn.
use std::collections::HashMap;
use std::sync::Arc;

/// What a reply draws: its Markdown, the diagram markers taken out of it, and the pictures of its equations.
pub struct Prepared {
    pub markdown: String,
    pub undrawn: HashMap<usize, String>,
    pub math: HashMap<String, Arc<[u8]>>,
}

/// The reply's words as the thread draws them (`reply_actions::reply_markdown`).
pub fn prepare_reply(body: &str, dark: bool) -> Prepared {
    let body = keep_html_as_words(&electron_line_shapes(&end_lists_at_margin_prose(body)));
    let prepared = crate::diagram::prepare_reply_markdown(&body);
    let (prepared, undrawn) = crate::diagram::take_undrawn_markers(&prepared);
    let (markdown, math) = crate::math_tex::prepare_math_markdown(&prepared, dark);
    Prepared { markdown, undrawn, math }
}

/// A line that starts a list item: `-`, `*` or `+`, or a number and `.` or `)`, then a space or the line's end.
fn is_item(line: &str) -> bool {
    let line = line.trim_start();
    let rest = if let Some(rest) = line.strip_prefix(['-', '*', '+']) {
        rest
    } else {
        let digits = line.len() - line.trim_start_matches(|c: char| c.is_ascii_digit()).len();
        if digits == 0 || digits > 9 {
            return false;
        }
        match line[digits..].strip_prefix(['.', ')']) {
            Some(rest) => rest,
            None => return false,
        }
    };
    rest.is_empty() || rest.starts_with([' ', '\t'])
}

fn fence_of(line: &str) -> Option<(char, usize)> {
    let line = line.trim_start();
    let mark = line.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = line.len() - line.trim_start_matches(mark).len();
    (n >= 3).then_some((mark, n))
}

/// Prose at the left margin straight after a list item ends the list (`a-wrapped-bullet-stays-one-bullet.test.ts`:
/// "still ends the list at UNINDENTED prose, which a model usually means as prose"). CommonMark would take the line
/// into the item as a lazy continuation, so a blank line goes in before it. An INDENTED line still wraps into its
/// item, and nothing inside a code fence is touched.
pub fn end_lists_at_margin_prose(body: &str) -> String {
    let mut out = String::with_capacity(body.len() + 8);
    let mut fence: Option<(char, usize)> = None;
    let mut in_list = false;
    let mut after_blank = true;
    for line in body.split_inclusive('\n') {
        let words = line.trim_end_matches(['\r', '\n']);
        if let Some((mark, n)) = fence {
            if fence_of(words).is_some_and(|(m, k)| m == mark && k >= n && words.trim_start().trim_start_matches(mark).trim().is_empty()) {
                fence = None;
            }
            out.push_str(line);
            continue;
        }
        if let Some(opened) = fence_of(words) {
            fence = Some(opened);
            in_list = false;
        } else if words.trim().is_empty() {
            after_blank = true;
            out.push_str(line);
            continue;
        } else if is_item(words) {
            in_list = true;
        } else if !words.starts_with([' ', '\t']) {
            if in_list && !after_blank && !words.starts_with(['>', '#', '|']) {
                out.push('\n');
            }
            in_list = false;
        }
        after_blank = false;
        out.push_str(line);
    }
    out
}

/// Three line shapes `agentText.ts` reads another way than CommonMark (`agentText.test.ts`):
/// - `2)` continues a list numbered `1.`: one list, whatever the numbers close with (CommonMark starts another);
/// - a heading deeper than three is drawn as the third ("the thread has three sizes");
/// - a lone `#` is a word, not an empty heading.
///
/// Nothing inside a code fence is touched.
pub fn electron_line_shapes(body: &str) -> String {
    let numbered = crate::invariants::regex(r"^([ \t>]*\d{1,9})\)([ \t])");
    let deep = crate::invariants::regex(r"^( {0,3})#{4,6}([ \t])");
    let lone = crate::invariants::regex(r"^( {0,3})(#{1,6}[ \t]*\r?)$");
    let mut out = String::with_capacity(body.len() + 8);
    let mut fence: Option<(char, usize)> = None;
    for line in body.split_inclusive('\n') {
        let words = line.trim_end_matches(['\r', '\n']);
        let end = &line[words.len()..];
        if let Some((mark, n)) = fence {
            if fence_of(words).is_some_and(|(m, k)| m == mark && k >= n && words.trim_start().trim_start_matches(mark).trim().is_empty()) {
                fence = None;
            }
            out.push_str(line);
            continue;
        }
        if let Some(opened) = fence_of(words) {
            fence = Some(opened);
            out.push_str(line);
            continue;
        }
        let shaped = numbered.replace(words, "$1.$2");
        let shaped = deep.replace(&shaped, "$1###$2");
        let shaped = lone.replace(&shaped, "$1\\$2");
        out.push_str(&shaped);
        out.push_str(end);
    }
    out
}

/// A reply is drawn as words, never as markup (`links-go-somewhere.test.ts`: "A reply is rendered as TEXT and never as
/// markup"). CommonMark reads `<model>`, `</div>` or `<!--` as HTML, and rune-kit draws nothing for HTML, so those words
/// were lost (`a-blank-line-does-not-end-a-list.test.tsx`, "drops no words from any entry in the changelog": 0.845.0
/// opens `<!-- big -->`, 0.479.0 says "Switch to <model>"). Each such `<` is escaped so the line reads as written. An
/// autolink (`<https://…>`) stays one, and code (fenced, indented, or between backticks) is not touched.
pub fn keep_html_as_words(body: &str) -> String {
    let mut out = String::with_capacity(body.len() + 8);
    let mut fence: Option<(char, usize)> = None;
    for line in body.split_inclusive('\n') {
        let words = line.trim_end_matches(['\r', '\n']);
        if let Some((mark, n)) = fence {
            if fence_of(words).is_some_and(|(m, k)| m == mark && k >= n && words.trim_start().trim_start_matches(mark).trim().is_empty()) {
                fence = None;
            }
            out.push_str(line);
            continue;
        }
        if let Some(opened) = fence_of(words) {
            fence = Some(opened);
            out.push_str(line);
            continue;
        }
        let rest = line.trim_start_matches([' ', '>']);
        // Four spaces in is code, drawn as written already.
        if line[..line.len() - rest.len()].trim_start_matches('>').starts_with("    ") {
            out.push_str(line);
            continue;
        }
        escape_tags(line, &mut out);
    }
    out
}

/// `line` with a backslash before each `<` that opens a tag, outside code spans.
fn escape_tags(line: &str, out: &mut String) {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut code: Option<usize> = None;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            let run = bytes[i..].iter().take_while(|b| **b == b'`').count();
            code = match code {
                None if line[i + run..].contains(&"`".repeat(run)) => Some(run),
                Some(open) if open == run => None,
                other => other,
            };
            out.push_str(&line[i..i + run]);
            i += run;
            continue;
        }
        if code.is_none() && bytes[i] == b'<' && (i == 0 || bytes[i - 1] != b'\\') {
            let after = &line[i + 1..];
            let tag = after.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || "/!?".contains(c));
            let lower = after.get(..8).unwrap_or(after).to_ascii_lowercase();
            let autolink = ["http://", "https://", "mailto:"].iter().any(|a| lower.starts_with(a));
            if tag && !autolink {
                out.push('\\');
            }
        }
        let c = line[i..].chars().next().unwrap_or_default();
        out.push(c);
        i += c.len_utf8().max(1);
    }
}

/// Where a link in a reply really goes, shown beside its label so a label cannot lie about it (Electron's
/// `outbound-links.ts` `linkHost`): the host, without `www.`, for a web address; none for anything else.
pub fn link_host(url: &str) -> Option<String> {
    if !locust_core::published_links::is_web_link(url) {
        return None;
    }
    let rest = url.split_once("://")?.1;
    let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let host = authority.rsplit('@').next()?.split(':').next()?.to_ascii_lowercase();
    Some(host.strip_prefix("www.").map(str::to_owned).unwrap_or(host))
}

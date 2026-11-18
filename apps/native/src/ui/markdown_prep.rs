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
    let body = fence_tool_calls(body);
    let body = electron_line_shapes(&end_lists_at_margin_prose(&body));
    let body = mark_colours(&mark_table_statuses(&body));
    let prepared = crate::diagram::prepare_reply_markdown(&body);
    let (prepared, undrawn) = crate::diagram::take_undrawn_markers(&prepared);
    let (markdown, math) = crate::math_tex::prepare_math_markdown(&prepared, dark);
    // Last, once math has been read: `$0<x<1$` is an equation, not a tag.
    let markdown = keep_html_as_words(&markdown);
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

/// A tool call a model wrote as TEXT (`<tool_call><function=edit>…`), fenced so it is drawn as code labelled "tool call"
/// (agentText.ts `fenceToolCalls`, `a-tool-call-written-as-text.test.ts`). An unclosed one runs to the end.
pub fn fence_tool_calls(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    if !lower.contains("<tool_call>") {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len() + 32);
    let mut at = 0;
    while let Some(open) = lower[at..].find("<tool_call>").map(|i| at + i) {
        out.push_str(&text[at..open]);
        let body_start = open + "<tool_call>".len();
        let (body_end, next) = match lower[body_start..].find("</tool_call>") {
            Some(i) => (body_start + i, body_start + i + "</tool_call>".len()),
            None => (text.len(), text.len()),
        };
        out.push_str("\n```tool call\n");
        out.push_str(text[body_start..body_end].trim());
        out.push_str("\n```\n");
        at = next;
    }
    out.push_str(&text[at..]);
    out
}

/// A status's tone (`statusChips.ts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Green,
    Blue,
    Amber,
    Red,
    Quiet,
}

impl Tone {
    fn word(self) -> &'static str {
        match self {
            Tone::Green => "green",
            Tone::Blue => "blue",
            Tone::Amber => "amber",
            Tone::Red => "red",
            Tone::Quiet => "quiet",
        }
    }
}

const TONES: &[(Tone, &[&str])] = &[
    (Tone::Red, &["blocked", "failed", "failing", "fail", "error", "broken", "cancelled", "canceled", "at risk", "overdue", "stuck"]),
    (Tone::Green, &["done", "complete", "completed", "finished", "shipped", "merged", "fixed", "resolved", "passed", "passing", "pass", "approved", "live", "ready"]),
    (Tone::Blue, &["in progress", "in-progress", "doing", "working", "running", "started", "ongoing", "underway", "under way", "wip", "active"]),
    (Tone::Amber, &["in review", "review", "needs review", "waiting", "pending", "on hold", "paused", "partial", "partly done"]),
    (Tone::Quiet, &["to do", "todo", "not started", "planned", "queued", "backlog", "next", "open", "not begun"]),
];

/// A cell's words without marks or a leading emoji, lower case (`statusChips.ts` `plainOf`).
fn plain_of(cell: &str) -> String {
    let marks: String = cell.chars().filter(|c| !"*_`~".contains(*c)).collect();
    let words = marks.trim_start_matches(|c: char| c.is_whitespace() || c == '\u{fe0f}' || c == '\u{200d}' || is_pictograph(c));
    words.trim().to_lowercase()
}

/// Unicode's Extended_Pictographic, as far as a status cell needs it: the emoji blocks and the dingbats and symbols
/// (✅ ❌ ⏳ 🚧 …).
fn is_pictograph(c: char) -> bool {
    matches!(c as u32, 0x2190..=0x21FF | 0x2300..=0x23FF | 0x2460..=0x27BF | 0x2900..=0x297F | 0x2B00..=0x2BFF | 0x1F000..=0x1FAFF)
}

/// The Status column of a table by its header, marks and all (`statusColumnOf`).
pub fn status_column_of(header: &[&str]) -> Option<usize> {
    header.iter().position(|cell| ["status", "state", "stage", "progress"].contains(&plain_of(cell).as_str()))
}

/// A status value's tone by its first words, and nothing it does not know (`statusToneOf`).
pub fn status_tone_of(cell: &str) -> Option<Tone> {
    let words = plain_of(cell);
    if words.is_empty() || words.chars().count() > 40 {
        return None;
    }
    TONES.iter().find_map(|(tone, known)| {
        known
            .iter()
            .any(|word| words == *word || words.strip_prefix(word).and_then(|rest| rest.chars().next()).is_some_and(|c| c.is_whitespace() || ":;,.()-–—/".contains(c)))
            .then_some(*tone)
    })
}

/// An exact `#rrggbb` colour, as GitHub draws one: a short form may be an issue number (`hexColourOf`).
pub fn hex_colour_of(text: &str) -> Option<&str> {
    let t = text.trim();
    (t.len() == 7 && t.starts_with('#') && t[1..].bytes().all(|b| b.is_ascii_hexdigit())).then_some(t)
}

/// A link's destination for a status chip or a colour swatch: the window's own marks, drawn by [`reply_link_look`] and
/// never opened.
pub const STATUS_MARK: &str = "locust-status:";
pub const SWATCH_MARK: &str = "locust-swatch:";

fn swatch(hex: &str) -> String {
    format!("[\u{25a0}]({SWATCH_MARK}{})", &hex[1..])
}

fn table_cells(row: &str) -> Option<Vec<String>> {
    let t = row.trim();
    if !t.contains('|') {
        return None;
    }
    let inner = t.strip_prefix('|').unwrap_or(t);
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    let mut cells = vec![String::new()];
    let mut prev = ' ';
    for c in inner.chars() {
        if c == '|' && prev != '\\' {
            cells.push(String::new());
        } else {
            cells.last_mut()?.push(c);
        }
        prev = c;
    }
    Some(cells)
}

fn is_delimiter_row(row: &str) -> bool {
    table_cells(row).is_some_and(|cells| {
        cells.iter().all(|c| {
            let c = c.trim();
            let c = c.strip_prefix(':').unwrap_or(c);
            let c = c.strip_suffix(':').unwrap_or(c);
            !c.is_empty() && c.bytes().all(|b| b == b'-')
        })
    })
}

/// A table's known statuses drawn as chips in their tone, and a cell that is nothing but a colour with its swatch
/// (`a-tables-status-is-a-chip.test.tsx`, `a-colour-code-shows-its-colour.test.tsx`). Other cells stay as written.
pub fn mark_table_statuses(body: &str) -> String {
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    let mut out = String::with_capacity(body.len() + 64);
    let mut fence: Option<(char, usize)> = None;
    let mut i = 0;
    while i < lines.len() {
        let words = lines[i].trim_end_matches(['\r', '\n']);
        if let Some((mark, n)) = fence {
            if fence_of(words).is_some_and(|(m, k)| m == mark && k >= n) {
                fence = None;
            }
            out.push_str(lines[i]);
            i += 1;
            continue;
        }
        if let Some(opened) = fence_of(words) {
            fence = Some(opened);
            out.push_str(lines[i]);
            i += 1;
            continue;
        }
        let header = table_cells(words);
        let rule = lines.get(i + 1).map(|l| l.trim_end_matches(['\r', '\n']));
        let (Some(header), Some(rule)) = (header, rule) else {
            out.push_str(lines[i]);
            i += 1;
            continue;
        };
        if !is_delimiter_row(rule) {
            out.push_str(lines[i]);
            i += 1;
            continue;
        }
        let status = status_column_of(&header.iter().map(String::as_str).collect::<Vec<_>>());
        out.push_str(lines[i]);
        out.push_str(lines[i + 1]);
        i += 2;
        while i < lines.len() {
            let row = lines[i].trim_end_matches(['\r', '\n']);
            let Some(cells) = table_cells(row).filter(|_| !row.trim().is_empty()) else { break };
            let cells: Vec<String> = cells
                .iter()
                .enumerate()
                .map(|(at, cell)| {
                    let words = cell.trim();
                    if let Some(hex) = hex_colour_of(words) {
                        format!(" {} {hex} ", swatch(hex))
                    } else if Some(at) == status && !words.contains("](") {
                        match status_tone_of(words) {
                            Some(tone) => format!(" [{words}]({STATUS_MARK}{}) ", tone.word()),
                            None => cell.clone(),
                        }
                    } else {
                        cell.clone()
                    }
                })
                .collect();
            out.push('|');
            out.push_str(&cells.join("|"));
            out.push('|');
            out.push_str(&lines[i][row.len()..]);
            i += 1;
        }
    }
    out
}

/// A colour code with its colour beside it: one written as code (`` `#7a9e8e` ``), anywhere outside a fence, and a list
/// item that is nothing but a colour. A colour inside a sentence is text, as GitHub draws it ("fixed in #123456").
pub fn mark_colours(body: &str) -> String {
    let code = crate::invariants::regex(r"(^|[^`])`(#[0-9a-fA-F]{6})`([^`]|$)");
    let item = crate::invariants::regex(r"^(\s*(?:[-*+]|\d{1,9}[.)])\s+)(#[0-9a-fA-F]{6})\s*$");
    let mut out = String::with_capacity(body.len() + 64);
    let mut fence: Option<(char, usize)> = None;
    for line in body.split_inclusive('\n') {
        let words = line.trim_end_matches(['\r', '\n']);
        if let Some((mark, n)) = fence {
            if fence_of(words).is_some_and(|(m, k)| m == mark && k >= n) {
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
        let shaped = item.replace(words, |c: &regex::Captures| format!("{}{} {}", &c[1], swatch(&c[2]), &c[2]));
        let shaped = code.replace_all(&shaped, |c: &regex::Captures| format!("{}{} `{}`{}", &c[1], swatch(&c[2]), &c[2], &c[3]));
        out.push_str(&shaped);
        out.push_str(&line[words.len()..]);
    }
    out
}

/// How a status chip or a colour swatch is drawn: a chip in its tone's text, tint and edge (`.lc-status--*`), and a
/// swatch as a square in its own colour (`.lc-swatch`). Other links keep Markdown's own look.
pub fn reply_link_look(url: &str, dark: bool) -> Option<(rune::Span, rune::LinkPaint)> {
    let l = crate::look::controls::Look::new(dark);
    if let Some(tone) = url.strip_prefix(STATUS_MARK) {
        let (ink, tint) = match tone {
            "green" | "blue" | "amber" | "red" => (l.c(&format!("{tone}-text")), l.c(&format!("{tone}-tint"))),
            _ => (l.c("text-muted"), l.c("chip-bg")),
        };
        let span = rune::Span::new(0, 0).color(ink).bg(tint, 3.0);
        return Some((span, rune::LinkPaint::new().color(ink).bg(tint)));
    }
    if let Some(hex) = url.strip_prefix(SWATCH_MARK) {
        let colour = rune::color::hex(&format!("#{hex}"))?;
        return Some((rune::Span::new(0, 0).color(colour), rune::LinkPaint::new().color(colour)));
    }
    None
}

/// The window's own marks are drawn, not opened.
pub fn is_own_mark(url: &str) -> bool {
    url.starts_with(STATUS_MARK) || url.starts_with(SWATCH_MARK)
}

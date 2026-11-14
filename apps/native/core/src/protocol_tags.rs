//! Protocol tags parsed out of teammate replies (`protocolTags.ts`, `decision.ts`, `next-steps.ts`, `handover.ts`).
//!
//! One shared parser for both the door (`door_tools::unwrap_protocol_tags`) and the native thread UI.

use crate::invariants::regex;
use regex::Regex;
use std::collections::HashSet;

pub const PROTOCOL_TAGS: &[&str] = &[
    "locust-share",
    "locust-memory",
    "locust-ask",
    "locust-task",
    "locust-file",
    "locust-tidy",
    "locust-instructions",
    "locust-key",
    "locust-next",
];

pub const DEFANGED_BRACKET: char = '‹';

/// Every protocol tag's opener or closer, made from [`PROTOCOL_TAGS`] once (protocolTags.ts `ANY_TAG`): a tag added to
/// the list is defanged and unwrapped everywhere at once, which is why there is one list and not a regex per file.
static ANY_TAG: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(&format!(r"(?i)<(/?)({})\b", PROTOCOL_TAGS.join("|"))).expect("tag names are plain words"));
/// A whole protocol tag, attributes and all, as `unwrap_protocol_tags` takes it off.
static LOOSE_TAG: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(&format!(r"(?i)</?(?:{})\b[^<>]{{0,400}}>", PROTOCOL_TAGS.join("|"))).expect("tag names are plain words"));

/// Text from a model, made safe to quote into another model's prompt (`defangProtocolBlocks`).
pub fn defang_protocol_blocks(text: &str) -> String {
    ANY_TAG.replace_all(text, |c: &regex::Captures| {
        format!("{DEFANGED_BRACKET}{}{}", &c[1], &c[2])
    })
    .into_owned()
}

/// Fenced blocks and inline code spans, which are quoted source and stay intact.
fn code_spans(text: &str) -> Vec<(usize, usize)> {
    let code = regex(r"```[\s\S]*?```|~~~[\s\S]*?~~~|`[^`\n]*`");
    code.find_iter(text).map(|m| (m.start(), m.end())).collect()
}

/// Every match of `block_re` whose opening tag is not inside code (`blocksOutsideCode`).
/// Returns `(start_byte, end_byte, body)` for each match.
pub fn blocks_outside_code(text: &str, block_re: &Regex) -> Vec<(usize, usize, String)> {
    let spans = code_spans(text);
    let in_code = |at: usize| spans.iter().any(|&(from, to)| at >= from && at < to);
    let mut found = Vec::new();
    let mut pos = 0;
    while pos <= text.len() {
        let Some(caps) = block_re.captures_at(text, pos) else {
            break;
        };
        let whole = caps.get(0).unwrap();
        if in_code(whole.start()) {
            let step = text[whole.start()..].chars().next().map_or(1, char::len_utf8);
            pos = whole.start() + step;
            continue;
        }
        let body = caps.get(1).map_or("", |m| m.as_str()).to_owned();
        found.push((whole.start(), whole.end(), body));
        if whole.is_empty() {
            let step = text[whole.start()..].chars().next().map_or(1, char::len_utf8);
            pos = whole.start() + step;
        } else {
            pos = whole.end();
        }
    }
    found
}

/// The text without the blocks `blocks_outside_code` finds (`stripBlocksOutsideCode`).
pub fn strip_blocks_outside_code(text: &str, block_re: &Regex) -> String {
    let mut kept = String::with_capacity(text.len());
    let mut from = 0;
    for (start, end, _) in blocks_outside_code(text, block_re) {
        kept.push_str(&text[from..start]);
        from = end;
    }
    kept.push_str(&text[from..]);
    kept
}

/// Take Locust's tags off anything still wearing them outside code, and strip a torn tag at the tail (`unwrapProtocolTags`).
pub fn unwrap_protocol_tags(text: &str) -> String {
    let code = regex(r"```[\s\S]*?```|~~~[\s\S]*?~~~|`[^`\n]*`");
    let key = regex(r#"(?i)<locust-key\s+name\s*=\s*"([^"]{1,80})"\s*>[\s\S]*?</locust-key>"#);
    let loose = &*LOOSE_TAG;
    let plain = |part: &str| -> String {
        let keyed = key.replace_all(part, |c: &regex::Captures| {
            format!("Asks for the key {} (below).", c[1].trim())
        });
        loose.replace_all(&keyed, "").into_owned()
    };
    let mut out = String::new();
    let mut at = 0;
    for span in code.find_iter(text) {
        out.push_str(&plain(&text[at..span.start()]));
        out.push_str(span.as_str());
        at = span.end();
    }
    out.push_str(&plain(&text[at..]));
    let torn = regex(r"(?i)\n?\s*</?(?:locu)[a-z-]*$");
    let out = torn.replace(&out, "").into_owned();
    let out = regex(r"\n{3,}").replace_all(&out, "\n\n").into_owned();
    out.trim_start_matches('\n').trim_end().to_owned()
}

/// Collapse control characters (except whitespace collapsed to single spaces) and trim.
fn clean_inline(value: &str) -> String {
    let filtered: String = value
        .chars()
        .filter(|&c| {
            let code = c as u32;
            !((code <= 0x08) || code == 0x0b || code == 0x0c || (0x0e..=0x1f).contains(&code) || code == 0x7f)
        })
        .collect();
    filtered.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn bounded_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_owned();
    }
    let prefix: String = value.chars().take(limit.saturating_sub(1)).collect();
    format!("{}…", prefix.trim_end())
}

// ---------------------------------------------------------------------------
// Decisions (`shared/decision.ts`)
// ---------------------------------------------------------------------------

pub const ASK_TAG: &str = "locust-ask";
pub const MAX_QUESTION_LENGTH: usize = 600;
pub const MAX_OPTION_LABEL_LENGTH: usize = 90;
pub const MAX_OPTION_NOTE_LENGTH: usize = 160;
pub const MIN_OPTIONS: usize = 2;
pub const MAX_OPTIONS: usize = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionOption {
    pub label: String,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionRequest {
    pub question: String,
    pub options: Vec<DecisionOption>,
}

const BARE_ASSENT: &[&str] = &[
    "yes",
    "no",
    "ok",
    "okay",
    "sure",
    "proceed",
    "continue",
    "go",
    "go ahead",
    "do it",
    "stop",
    "cancel",
    "abort",
    "wait",
    "approve",
    "reject",
    "deny",
    "confirm",
    "yes please",
    "no thanks",
    "don't",
    "do not",
];

fn is_bare_assent(label: &str) -> bool {
    let lowered = label.to_lowercase();
    let stripped = lowered.trim_end_matches(['.', '!', '?', ',', ';', ':']).trim();
    BARE_ASSENT.contains(&stripped)
}

/// Strip template placeholder brackets kept by small models (`withoutTemplateMarks`).
pub fn without_template_marks(value: &str) -> String {
    let prefix_re = regex(r"(?i)^<(?:option name|option|name|what it costs or implies)>\s*");
    let mut text = if let Some(m) = prefix_re.find(value) {
        let rest = &value[m.end()..];
        if rest.is_empty() {
            value.to_owned()
        } else {
            rest.to_owned()
        }
    } else {
        value.to_owned()
    };
    let wrapped_re = regex(r"^<([^<>]+)>$");
    if let Some(caps) = wrapped_re.captures(&text) {
        text = caps[1].trim().to_owned();
    }
    if !text.contains('<') && regex(r"[^-=\s]>$").is_match(&text) {
        text.pop();
        text = text.trim_end().to_owned();
    }
    if !text.contains('>') && text.starts_with('<') {
        text.remove(0);
        text = text.trim_start().to_owned();
    }
    text
}

pub fn is_placeholder_label(label: &str) -> bool {
    regex(r"(?i)^(?:<[^>]*>|(?:the )?(?:first|second|third|fourth|1st|2nd|3rd|4th) option|option (?:[1-4]|one|two|three|four|[a-d]))$")
        .is_match(label.trim())
}

fn ask_block_re() -> Regex {
    regex(r"<locust-ask\s*>([\s\S]*?)</locust-ask>")
}

fn option_line_re() -> Regex {
    regex(r"^(?:[-*]|\d{1,2}[.)])\s+(.+)$")
}

fn read_decision_block(body: &str) -> Option<DecisionRequest> {
    let mut question_lines = Vec::new();
    let mut options = Vec::new();
    let opt_re = option_line_re();

    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(caps) = opt_re.captures(trimmed) else {
            if options.is_empty() {
                question_lines.push(trimmed);
            }
            continue;
        };
        let rest_of_line = &caps[1];
        let (raw_label, raw_note) = rest_of_line.split_once("::").unwrap_or((rest_of_line, ""));
        let mut text = without_template_marks(&clean_inline(raw_label));
        if text.is_empty() {
            continue;
        }
        let mut note = without_template_marks(&clean_inline(raw_note));
        if is_placeholder_label(&text) && !note.is_empty() {
            text = std::mem::take(&mut note);
        }
        options.push(DecisionOption {
            label: bounded_chars(&text, MAX_OPTION_LABEL_LENGTH),
            note: if note.is_empty() {
                None
            } else {
                Some(bounded_chars(&note, MAX_OPTION_NOTE_LENGTH))
            },
        });
    }

    let question = bounded_chars(&clean_inline(&question_lines.join(" ")), MAX_QUESTION_LENGTH);
    if question.is_empty() {
        return None;
    }
    if options.len() < MIN_OPTIONS || options.len() > MAX_OPTIONS {
        return None;
    }
    let mut seen = HashSet::new();
    for opt in &options {
        if !seen.insert(opt.label.to_lowercase()) {
            return None;
        }
        if is_bare_assent(&opt.label) {
            return None;
        }
    }
    Some(DecisionRequest { question, options })
}

/// The first well-formed decision in a transcript (`parseDecision`).
pub fn parse_decision(text: &str) -> Option<DecisionRequest> {
    for (_, _, body) in blocks_outside_code(text, &ask_block_re()) {
        if let Some(parsed) = read_decision_block(&body) {
            return Some(parsed);
        }
    }
    None
}

fn plain_question(body: &str) -> String {
    let opt_re = option_line_re();
    let sep_re = regex(r"\s*::\s*");
    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| {
            if opt_re.is_match(line) {
                sep_re.replace(line, " — ").into_owned()
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Remove only the ask block that became the card; turn refused/subsequent ask blocks into plain prose (`stripDecisionBlocks`).
pub fn strip_decision_blocks(text: &str) -> String {
    let outside = blocks_outside_code(text, &ask_block_re());
    let card_start = outside
        .iter()
        .find(|(_, _, body)| read_decision_block(body).is_some())
        .map(|(start, _, _)| *start);
    let mut kept = String::with_capacity(text.len());
    let mut from = 0;
    for (start, end, body) in outside {
        kept.push_str(&text[from..start]);
        if Some(start) != card_start {
            kept.push_str(&plain_question(&body));
        }
        from = end;
    }
    kept.push_str(&text[from..]);
    let collapsed = regex(r"\n{3,}").replace_all(&kept, "\n\n");
    collapsed.trim_end().to_owned()
}

/// What the next turn is sent when a person picks (`decisionReply`).
pub fn decision_reply(option: &DecisionOption) -> String {
    match &option.note {
        None => format!("{}. Continue with that.", option.label),
        Some(note) => format!("{} ({note}). Continue with that.", option.label),
    }
}

// ---------------------------------------------------------------------------
// Suggested tasks (`shared/next-steps.ts`)
// ---------------------------------------------------------------------------

pub const NEXT_TAG: &str = "locust-next";
pub const MAX_NEXT_STEPS: usize = 3;
pub const MAX_NEXT_TITLE: usize = 80;
pub const MAX_NEXT_BRIEF: usize = 600;
pub const NEXT_PLACEHOLDERS: &[&str] = &["<short title>", "<what to do, and what done looks like>"];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NextStep {
    pub title: String,
    pub brief: String,
}

fn next_block_re() -> Regex {
    regex(r"<locust-next\s*>([\s\S]*?)</locust-next>")
}

/// The tasks a reply suggests, in the order it wrote them (`parseNextSteps`).
pub fn parse_next_steps(text: &str) -> Vec<NextStep> {
    let mut steps = Vec::new();
    let mut seen = HashSet::new();
    let bullet_re = regex(r"^(?:[-*]\s+|\d+[.)]\s+)");

    for (_, _, body) in blocks_outside_code(text, &next_block_re()) {
        for line in body.split('\n') {
            if steps.len() >= MAX_NEXT_STEPS {
                return steps;
            }
            let trimmed = bullet_re.replace(line.trim(), "");
            if trimmed.is_empty() {
                continue;
            }
            let (raw_title, raw_brief) = trimmed.split_once("::").unwrap_or((&trimmed, &trimmed));
            let title = bounded_chars(&clean_inline(raw_title), MAX_NEXT_TITLE);
            let brief = bounded_chars(&clean_inline(raw_brief), MAX_NEXT_BRIEF);
            if title.is_empty() || brief.is_empty() {
                continue;
            }
            if NEXT_PLACEHOLDERS
                .iter()
                .any(|p| title.contains(p) || brief.contains(p))
            {
                continue;
            }
            if !seen.insert(title.to_lowercase()) {
                continue;
            }
            steps.push(NextStep { title, brief });
        }
    }
    steps
}

/// The reply without its suggested-task blocks (`stripNextBlocks`).
pub fn strip_next_blocks(text: &str) -> String {
    let stripped = strip_blocks_outside_code(text, &next_block_re());
    regex(r"\n{3,}")
        .replace_all(&stripped, "\n\n")
        .trim_end()
        .to_owned()
}

/// What a started card is told (`nextStepPrompt`).
pub fn next_step_prompt(step: &NextStep) -> String {
    if step
        .brief
        .to_lowercase()
        .starts_with(&step.title.to_lowercase())
    {
        step.brief.clone()
    } else {
        format!("{}: {}", step.title, step.brief)
    }
}

// ---------------------------------------------------------------------------
// Handed-over files (`shared/handover.ts`)
// ---------------------------------------------------------------------------

pub const FILE_TAG: &str = "locust-file";
pub const MAX_FILES_PER_REPLY: usize = 4;
pub const MAX_FILE_NOTE_LENGTH: usize = 120;
pub const FILE_BLOCK_EXAMPLE_PATH: &str = "path/relative/to/the/folder.md";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandedFile {
    pub path: String,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefusedFileLine {
    pub path: String,
    pub why: String,
}

fn file_block_re() -> Regex {
    regex(r"<locust-file\s*>([\s\S]*?)</locust-file>")
}

fn has_control_char(s: &str) -> bool {
    s.chars().any(|c| {
        let code = c as u32;
        code <= 0x1f || code == 0x7f
    })
}

fn is_drive_relative(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && bytes.get(2) != Some(&b'/')
}

fn normalize_file_path(path: &str) -> String {
    let forward = path.replace('\\', "/");
    let stripped = forward.strip_prefix("./").unwrap_or(&forward);
    stripped.trim().to_owned()
}

fn acceptable_file_path(path: &str) -> bool {
    if path.is_empty() || path.chars().count() > 400 {
        return false;
    }
    if path.replace('\\', "/").to_lowercase() == FILE_BLOCK_EXAMPLE_PATH {
        return false;
    }
    if has_control_char(path) {
        return false;
    }
    if path.starts_with("//") {
        return false;
    }
    if is_drive_relative(path) {
        return false;
    }
    true
}

/// Whether a handed path is whole (`D:/scratch/a.md`, `/home/a.md`) rather than relative (`isWholePath`).
pub fn is_whole_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    path.starts_with('/')
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && bytes[2] == b'/')
}

/// Where a handed file is on disk (`handedFileOnDisk`).
pub fn handed_file_on_disk(folder: &str, path: &str) -> String {
    if is_whole_path(path) {
        path.to_owned()
    } else {
        format!("{folder}/{path}")
    }
}

/// The files a reply hands over, in the order they were written (`parseFileBlocks`).
pub fn parse_file_blocks(text: &str) -> Vec<HandedFile> {
    let mut files = Vec::new();
    let mut seen = HashSet::new();
    let bullet_re = regex(r"^[-*]\s+");

    for (_, _, body) in blocks_outside_code(text, &file_block_re()) {
        for line in body.split('\n') {
            if files.len() >= MAX_FILES_PER_REPLY {
                return files;
            }
            let trimmed = bullet_re.replace(line.trim(), "");
            if trimmed.is_empty() {
                continue;
            }
            let (raw_path, raw_note) = trimmed.split_once("::").unwrap_or((&trimmed, ""));
            let path = normalize_file_path(raw_path);
            if !acceptable_file_path(&path) {
                continue;
            }
            if !seen.insert(path.clone()) {
                continue;
            }
            let note_trimmed = raw_note.trim();
            let note = if note_trimmed.is_empty() {
                None
            } else if note_trimmed.chars().count() <= MAX_FILE_NOTE_LENGTH {
                Some(note_trimmed.to_owned())
            } else {
                let prefix: String = note_trimmed
                    .chars()
                    .take(MAX_FILE_NOTE_LENGTH - 1)
                    .collect();
                Some(format!("{prefix}…"))
            };
            files.push(HandedFile { path, note });
        }
    }
    files
}

/// The lines of a file block this app would not draw a button for, and why (`refusedFileLines`).
pub fn refused_file_lines(text: &str) -> Vec<RefusedFileLine> {
    let mut refused = Vec::new();
    let bullet_re = regex(r"^[-*]\s+");

    for (_, _, body) in blocks_outside_code(text, &file_block_re()) {
        for line in body.split('\n') {
            let trimmed = bullet_re.replace(line.trim(), "");
            if trimmed.is_empty() {
                continue;
            }
            let raw_path = trimmed.split("::").next().unwrap_or("");
            let path = normalize_file_path(raw_path);
            if acceptable_file_path(&path)
                || path.is_empty()
                || path.to_lowercase() == FILE_BLOCK_EXAMPLE_PATH
            {
                continue;
            }
            if has_control_char(&path) || path.chars().count() > 400 {
                continue;
            }
            refused.push(RefusedFileLine {
                path,
                why: "it is not a path to a file on this computer".into(),
            });
            if refused.len() >= MAX_FILES_PER_REPLY {
                return refused;
            }
        }
    }
    refused
}

/// The reply without its file blocks (`stripFileBlocks`).
pub fn strip_file_blocks(text: &str) -> String {
    let stripped = strip_blocks_outside_code(text, &file_block_re());
    regex(r"\n{3,}")
        .replace_all(&stripped, "\n\n")
        .trim_end()
        .to_owned()
}

/// The reply text as shown to the person in the thread (`missionView.ts` `shownText`).
pub fn shown_text(text: &str) -> String {
    // Memory and tidy blocks were acted on (or proposed) by the host: their words are not the reply (stripMemoryBlocks).
    let s = crate::memory_tidy::strip_memory_blocks(text);
    let s = strip_decision_blocks(&s);
    let s = strip_file_blocks(&s);
    let s = strip_next_blocks(&s);
    unwrap_protocol_tags(&s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(body: &str) -> String {
        format!("<locust-ask>\n{body}\n</locust-ask>")
    }

    fn fenced(block: &str) -> String {
        format!("Here is how a teammate writes one:\n```\n{block}\n```\nThat is the whole protocol.")
    }

    // --- protocol-tags.test.ts ---

    #[test]
    fn defangs_every_tag_the_host_parses() {
        for tag in PROTOCOL_TAGS {
            let open = format!("<{tag}>");
            let close = format!("</{tag}>");
            let carried = format!("Sure, here it is: {open} do the thing {close}");
            let safe = defang_protocol_blocks(&carried);
            assert!(!safe.contains(&open), "{tag}");
            assert!(!safe.contains(&close), "{tag}");
            assert!(safe.contains("do the thing"), "{tag}");
        }
    }

    #[test]
    fn a_defanged_memory_block_is_no_longer_a_memory_block() {
        let real = ["<locust-memory>", "remember :: a real one", "</locust-memory>"].join("\n");
        assert_eq!(crate::people::memory_ops(&real).len(), 1);
        assert!(crate::people::memory_ops(&defang_protocol_blocks(&real)).is_empty());
    }

    #[test]
    fn a_defanged_share_block_sends_nothing() {
        let real = "<locust-share to=\"Wren\">go</locust-share>";
        assert_eq!(crate::peer_share::parse_share_blocks(real).len(), 1);
        assert!(crate::peer_share::parse_share_blocks(&defang_protocol_blocks(real)).is_empty());
    }

    #[test]
    fn defanging_leaves_ordinary_prose_and_ordinary_markup_alone() {
        let prose = "Use <div> and <Component/>, and talk about locust-memory in passing.";
        assert_eq!(defang_protocol_blocks(prose), prose);
    }

    #[test]
    fn defanging_is_what_sanitize_inbound_does_so_the_two_cannot_drift_apart() {
        let carried = "<locust-memory>remember :: x</locust-memory>";
        assert_eq!(crate::peer_share::sanitize_inbound(carried), defang_protocol_blocks(carried));
        assert!(!crate::peer_share::sanitize_inbound(carried).contains("<locust-memory>"));
    }

    #[test]
    fn defanging_catches_odd_casing_and_a_tag_carrying_attributes() {
        assert!(!defang_protocol_blocks("<LOCUST-SHARE to=\"A\">x</LOCUST-SHARE>").contains("<LOCUST-SHARE"));
        assert!(!defang_protocol_blocks("<locust-share  to=\"A\" when=\"now\">x</locust-share>").contains("<locust-share"));
    }

    #[test]
    fn every_place_that_defangs_and_unwraps_reads_the_one_tag_list() {
        // Beyond Electron's file: people.rs once kept a copy of the list. A tag that is only in PROTOCOL_TAGS must be
        // defanged in a brief and unwrapped on screen too.
        for tag in PROTOCOL_TAGS {
            assert!(!crate::people::defang_protocol_for_tests(&format!("<{tag}>x</{tag}>")).contains(&format!("<{tag}")), "{tag}");
            assert!(!unwrap_protocol_tags(&format!("say <{tag}>x</{tag}>")).contains(&format!("<{tag}")), "{tag}");
        }
    }

    // --- decision.test.ts ---

    #[test]
    fn reads_dashed_numbered_and_asterisk_options() {
        let dashed = parse_decision(&ask("Which way?\n- Rewrite it :: slower, cleaner\n- Patch it :: quicker, riskier")).unwrap();
        assert_eq!(
            dashed.options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>(),
            vec!["Rewrite it", "Patch it"]
        );

        for body in [
            "Which way?\n1. Rewrite it :: slower, cleaner\n2. Patch it :: quicker, riskier",
            "Which way?\n1) Rewrite it :: slower, cleaner\n2) Patch it :: quicker, riskier",
        ] {
            let parsed = parse_decision(&ask(body)).unwrap();
            assert_eq!(
                parsed.options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>(),
                vec!["Rewrite it", "Patch it"]
            );
        }

        let stars = parse_decision(&ask("Which way?\n* Rewrite it\n* Patch it")).unwrap();
        assert_eq!(stars.options.len(), 2);

        assert!(parse_decision(&ask("I found 2 files.\nThere were 3 problems.")).is_none());
    }

    #[test]
    fn option_named_after_template_uses_note_and_keeps_real_names() {
        let asked = parse_decision(
            "<locust-ask>\nHow do you want the file creation handled, given write tools are disabled in this mode?\n- The first option :: Re-run with write/shell enabled, and I create forbidden-ask.txt\n- The second option :: Leave it undone; I only report the README values\n</locust-ask>",
        )
        .unwrap();
        assert_eq!(
            asked.options,
            vec![
                DecisionOption {
                    label: "Re-run with write/shell enabled, and I create forbidden-ask.txt".into(),
                    note: None,
                },
                DecisionOption {
                    label: "Leave it undone; I only report the README values".into(),
                    note: None,
                },
            ]
        );

        let real = parse_decision("<locust-ask>\nWhich?\n- Keep the old option :: safer\n- Drop it :: simpler\n</locust-ask>").unwrap();
        assert_eq!(
            real.options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>(),
            vec!["Keep the old option", "Drop it"]
        );
    }

    // --- a-question-that-is-not-a-card-stays-in-the-reply.test.ts ---

    #[test]
    fn ask_block_card_is_removed_and_refused_or_second_ask_stays_as_plain_words() {
        let reply = format!(
            "I need a decision.\n\n{}",
            ask("Which database?\n- Postgres :: what the team knows\n- SQLite :: nothing to run")
        );
        assert_eq!(parse_decision(&reply).unwrap().options.len(), 2);
        assert_eq!(strip_decision_blocks(&reply), "I need a decision.");

        for body in [
            "Ship it now?\n- Ship :: today",
            "Which colour?\n- Red\n- Blue\n- Green\n- Gold\n- Grey",
            "Delete the old branch?\n- Yes :: it is merged\n- No :: keep it",
            "Which one?\n- Postgres\n- postgres",
        ] {
            let reply = format!("I need a decision.\n\n{}", ask(body));
            assert!(parse_decision(&reply).is_none());
            let shown = strip_decision_blocks(&reply);
            assert!(shown.contains(body.lines().next().unwrap()));
            assert!(!shown.contains("locust-ask"));
            assert!(!shown.contains("::"));
            assert_eq!(
                shown.lines().filter(|l| l.starts_with("- ")).count(),
                body.lines().count() - 1
            );
        }

        let two = [
            "Two things.".to_owned(),
            ask("Which database?\n- Postgres\n- SQLite"),
            ask("Which host?\n- Fly :: cheaper\n- Render :: simpler"),
        ]
        .join("\n\n");
        assert_eq!(parse_decision(&two).unwrap().question, "Which database?");
        assert_eq!(
            strip_decision_blocks(&two),
            "Two things.\n\nWhich host?\n- Fly — cheaper\n- Render — simpler"
        );

        let in_code = format!("Write it like this:\n\n```\n{}\n```", ask("Which?\n- A\n- B"));
        assert_eq!(strip_decision_blocks(&in_code), in_code);
    }

    // --- a-block-in-code-is-an-example.test.ts ---

    #[test]
    fn blocks_inside_code_are_not_parsed() {
        assert!(parse_decision(&fenced("<locust-ask>\nWhich way?\n- Rewrite it :: slower\n- Patch it :: quicker\n</locust-ask>")).is_none());
        assert!(parse_file_blocks(&fenced("<locust-file>\ndocs/report.md :: the rollup\n</locust-file>")).is_empty());
        assert!(parse_next_steps(&fenced("<locust-next>\nIn code :: not a task\n</locust-next>")).is_empty());
    }

    // --- a-tag-quoted-in-code-swallows-nothing.test.ts ---

    const GHOST: &str = "Yes. I ended with a `<locust-file>` block for the completed Markdown report:\n\n`../Documents/Codex/locust-ship-wt/docs/BETA-REVIEW-2026-09-30-0509-sol-long.md`\n\nIf no file button appeared, report that Locust did not render the handoff block. The path goes outside `.claude` using `../`, which may be relevant.\n\n<locust-file>\n../Documents/Codex/locust-ship-wt/docs/BETA-REVIEW-2026-09-30-0509-sol-long.md :: Completed 0.509 workflow review with evidence\n</locust-file>";

    #[test]
    fn a_tag_quoted_in_code_keeps_every_word_of_the_reply_and_takes_off_only_the_real_block() {
        let shown = strip_file_blocks(GHOST);
        assert!(shown.contains("If no file button appeared, report that Locust did not render the handoff block."));
        assert!(shown.contains("which may be relevant."));
        assert!(!shown.contains("</locust-file>"));
        assert!(shown.starts_with("Yes. I ended with a `<locust-file>` block"));
    }

    #[test]
    fn a_tag_quoted_in_code_finds_the_real_block_after_a_quoted_one() {
        assert_eq!(blocks_outside_code(GHOST, &file_block_re()).len(), 1);
        let inside = "See `<locust-file>`.\n\n<locust-file>\ndocs/report.md :: the report\n</locust-file>";
        assert_eq!(parse_file_blocks(inside), vec![HandedFile { path: "docs/report.md".into(), note: Some("the report".into()) }]);
    }

    #[test]
    fn a_tag_quoted_in_code_draws_ghosts_file_outside_its_folder_and_leaves_the_folder_rule_to_the_host() {
        assert_eq!(
            parse_file_blocks(GHOST),
            vec![HandedFile {
                path: "../Documents/Codex/locust-ship-wt/docs/BETA-REVIEW-2026-09-30-0509-sol-long.md".into(),
                note: Some("Completed 0.509 workflow review with evidence".into()),
            }]
        );
        assert!(refused_file_lines(GHOST).is_empty());
    }

    #[test]
    fn a_handed_file_it_will_not_draw_is_said_rather_than_nothing() {
        assert_eq!(
            refused_file_lines("<locust-file>\n\\\\server\\share\\report.md\n</locust-file>"),
            vec![RefusedFileLine { path: "//server/share/report.md".into(), why: "it is not a path to a file on this computer".into() }]
        );
        assert!(refused_file_lines("<locust-file>\nC:\\Users\\x\\report.md\n</locust-file>").is_empty());
        assert!(refused_file_lines("<locust-file>\ndocs/report.md\n</locust-file>").is_empty());
        assert!(refused_file_lines("<locust-file>\npath/relative/to/the/folder.md\n</locust-file>").is_empty());
    }

    #[test]
    fn a_tag_quoted_in_code_swallows_nothing_for_a_share_and_an_ask_either() {
        let share = "I used `<locust-share to=\"Reviewer\">` once.\n\n<locust-share to=\"Reviewer\">\nThe tests pass.\n</locust-share>\n\nDone.";
        assert_eq!(crate::peer_share::parse_share_blocks(share).iter().map(|b| b.text.as_str()).collect::<Vec<_>>(), vec!["The tests pass."]);
        assert!(crate::peer_share::strip_share_blocks(share).contains("Done."));
        assert!(crate::peer_share::strip_share_blocks(share).contains("I used `<locust-share to=\"Reviewer\">` once."));
        let ask = "The form is `<locust-ask>`.\n\nMore words here.\n\n<locust-ask>\nWhich storage?\n- JSON :: simple\n- CSV :: spreadsheet\n</locust-ask>";
        assert!(strip_decision_blocks(ask).contains("More words here."));
    }

    // --- a-torn-tag-keeps-the-letters-before-it.test.ts ---

    #[test]
    fn torn_tag_at_end_is_removed_and_letters_before_it_kept() {
        assert_eq!(unwrap_protocol_tags("Yes, the tests pass<locust-sh"), "Yes, the tests pass");
        assert_eq!(unwrap_protocol_tags("Done: 3 files<locu"), "Done: 3 files");
        assert_eq!(unwrap_protocol_tags("Yes, the tests pass <locust-sh"), "Yes, the tests pass");
        assert_eq!(unwrap_protocol_tags("It works.\n</locu"), "It works.");
        assert_eq!(unwrap_protocol_tags("Tests pass"), "Tests pass");
    }

    // --- a-half-written-tag-is-never-shown-to-a-person.test.ts ---

    const SEEN: &str = "<locust-share>\nBooty :: Reply with exactly the word PEBBLE-6725 and nothing else.\n</locust-share>";

    #[test]
    fn a_half_written_tag_is_the_pair_of_failures_it_looks_like() {
        assert!(crate::peer_share::parse_share_blocks(SEEN).is_empty());
        assert!(crate::peer_share::strip_share_blocks(SEEN).contains("locust-share"));
    }

    #[test]
    fn a_half_written_tag_shows_the_words_and_not_the_tags() {
        assert_eq!(unwrap_protocol_tags(&crate::peer_share::strip_share_blocks(SEEN)), "Booty :: Reply with exactly the word PEBBLE-6725 and nothing else.");
    }

    #[test]
    fn a_half_written_tag_keeps_the_body_because_nothing_else_has_a_copy_of_it() {
        assert_eq!(unwrap_protocol_tags("<locust-share>the only copy</locust-share>"), "the only copy");
    }

    #[test]
    fn a_half_written_tag_covers_the_other_four_which_fail_from_the_opposite_side() {
        for tag in ["locust-memory", "locust-ask", "locust-task", "locust-file"] {
            assert_eq!(unwrap_protocol_tags(&format!("<{tag} scope=\"team\">what it said</{tag}>")), "what it said", "{tag}");
        }
    }

    #[test]
    fn a_half_written_tag_handles_a_turn_that_was_cut_off_mid_block() {
        assert_eq!(unwrap_protocol_tags("here it is\n<locust-share to=\"Wren\">"), "here it is");
    }

    #[test]
    fn a_half_written_tag_leaves_a_teammate_explaining_the_protocol_alone() {
        let answer = "You write:\n\n```\n<locust-share to=\"Wren\">the note</locust-share>\n```\n\nand the host posts it.";
        assert_eq!(unwrap_protocol_tags(answer), answer);
        assert_eq!(unwrap_protocol_tags("call it `<locust-task>` in prose"), "call it `<locust-task>` in prose");
    }

    #[test]
    fn a_half_written_tag_rule_does_not_touch_ordinary_prose_or_a_tag_that_is_not_ours() {
        assert_eq!(unwrap_protocol_tags("a < b and c > d"), "a < b and c > d");
        assert_eq!(unwrap_protocol_tags("<locust-unknown>x</locust-unknown>"), "<locust-unknown>x</locust-unknown>");
    }

    #[test]
    fn a_run_cut_off_takes_off_a_tag_the_stream_was_cut_off_inside() {
        assert_eq!(unwrap_protocol_tags("briefs/colin-founder-bio.md :: a draft\n</locust-"), "briefs/colin-founder-bio.md :: a draft");
        assert_eq!(unwrap_protocol_tags("the answer\n<locust-sh"), "the answer");
    }

    #[test]
    fn a_run_cut_off_leaves_a_lone_bracket_and_anyone_elses_torn_tag_alone() {
        assert_eq!(unwrap_protocol_tags("x < y"), "x < y");
        assert_eq!(unwrap_protocol_tags("a list item </li"), "a list item </li");
        assert_eq!(unwrap_protocol_tags("compare a < b and c > d"), "compare a < b and c > d");
    }

    #[test]
    fn a_run_cut_off_is_trimmed_only_at_the_end_because_that_is_the_only_place_a_cut_can_leave_one() {
        let middle = "I was going to write </locust- but then kept talking about it.";
        assert_eq!(unwrap_protocol_tags(middle), middle);
    }

    #[test]
    fn a_block_still_streaming_never_flashes_its_raw_tags_in_the_shown_text() {
        // Native's own: the window draws the streaming reply through shown_text.
        assert_eq!(shown_text("Thinking...\n<locust-ask>\nWhich?\n- A\n<locu"), "Thinking...\n\nWhich?\n- A");
    }

    #[test]
    fn a_run_cut_off_still_unwraps_the_complete_tags_on_the_same_message() {
        assert_eq!(unwrap_protocol_tags("<locust-file path=\"a.md\">a draft</locust-file>\nand then\n</locust-"), "a draft\nand then");
    }

    // --- next-steps & handover ---

    #[test]
    fn next_steps_parses_caps_dedups_and_prompts() {
        let reply = [
            "Done: the update images are cleaned up after an install.",
            "",
            "<locust-next>",
            "Clean the Linux cache too :: Do the same for the AppImage cache, keeping the one waiting to install.",
            "- Tell people in What's new :: Add a line to the next changelog saying old updates no longer pile up.",
            "<short title> :: <what to do, and what done looks like>",
            "Clean the Linux cache too :: the same title again, so it is one card",
            "A fourth task :: one too many",
            "A fifth task :: two too many",
            "</locust-next>",
        ]
        .join("\n");
        let steps = parse_next_steps(&reply);
        assert_eq!(steps.len(), MAX_NEXT_STEPS);
        assert_eq!(
            steps.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(),
            vec!["Clean the Linux cache too", "Tell people in What's new", "A fourth task"]
        );
        assert_eq!(
            steps[0].brief,
            "Do the same for the AppImage cache, keeping the one waiting to install."
        );
        assert_eq!(
            strip_next_blocks(&reply),
            "Done: the update images are cleaned up after an install."
        );
        assert_eq!(
            next_step_prompt(&NextStep {
                title: "Clean the Linux cache too".into(),
                brief: "Do the same for the AppImage cache.".into(),
            }),
            "Clean the Linux cache too: Do the same for the AppImage cache."
        );
    }

    #[test]
    fn handover_parses_normalizes_dedups_and_caps() {
        let files = parse_file_blocks(
            "Done.\n<locust-file>\n- docs\\a.md :: one\n./docs/a.md :: duplicate\npath/relative/to/the/folder.md\nD:\\work\\review.md :: win\n</locust-file>",
        );
        assert_eq!(
            files,
            vec![
                HandedFile { path: "docs/a.md".into(), note: Some("one".into()) },
                HandedFile { path: "D:/work/review.md".into(), note: Some("win".into()) },
            ]
        );
        assert!(is_whole_path("D:/work/review.md"));
        assert!(is_whole_path("/tmp/a.md"));
        assert!(!is_whole_path("docs/a.md"));
        assert_eq!(handed_file_on_disk("/repo", "docs/a.md"), "/repo/docs/a.md");
        assert_eq!(handed_file_on_disk("/repo", "/tmp/a.md"), "/tmp/a.md");
    }
}

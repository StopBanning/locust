//! Telegram Markdown-to-HTML formatter and command parser (Electron `shared/telegram.ts`, 0.831).

use crate::chat_link::ParsedCommand;
use regex::Regex;
use std::sync::OnceLock;

/// What Telegram takes in one message is 4,096 characters; markup is counted here too, so this leaves room.
pub const TELEGRAM_MESSAGE_MAX: usize = 4_000;
/// An answer longer than this many messages ends with where the rest is.
pub const TELEGRAM_MAX_PARTS: usize = 4;
pub const TELEGRAM_REST_IN_LOCUST: &str = "<i>The rest is in Locust on your computer.</i>";

pub fn is_telegram_token(s: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d{5,15}:[A-Za-z0-9_-]{30,50}$").unwrap())
        .is_match(s)
}

/// A message's command, the way Telegram sends one: `/ada hello`, or `/ada@your_bot hello` in a menu's tap.
pub fn parsed_command(text: &str, bot_username: &str) -> Option<ParsedCommand> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"^/([A-Za-z0-9_]{1,32})(?:@([A-Za-z0-9_]{3,64}))?(?:\s+([\s\S]*))?$").unwrap()
    });
    let caps = re.captures(text.trim())?;
    if let Some(target) = caps.get(2) {
        if !target.as_str().eq_ignore_ascii_case(bot_username) {
            return None;
        }
    }
    let command = caps[1].to_lowercase();
    let rest = caps.get(3).map(|m| m.as_str().trim()).unwrap_or("").to_owned();
    Some(ParsedCommand { command, rest })
}

/// Text for Telegram's HTML: only these three are special outside a tag.
pub fn telegram_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
    out
}

/// Replace `*...*` when bounded by `(^|[\s(])` and `($|[\s).,;:!?])` without consuming the trailing boundary.
pub fn replace_bounded_italic(text: &str, open_tag: &str, close_tag: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'*' {
            let prev_ok = i == 0 || matches!(bytes[i - 1], b' ' | b'\t' | b'\n' | b'\r' | b'(');
            let next_ok = i + 1 < bytes.len() && !bytes[i + 1].is_ascii_whitespace() && bytes[i + 1] != b'*';
            if prev_ok && next_ok {
                if let Some(end_rel) = text[i + 1..].find(|c: char| c == '*' || c == '\n') {
                    let end = i + 1 + end_rel;
                    if bytes[end] == b'*' && end > i + 1 {
                        let last_ch = bytes[end - 1];
                        let after_ok = end + 1 == bytes.len()
                            || matches!(
                                bytes[end + 1],
                                b' ' | b'\t' | b'\n' | b'\r' | b')' | b'.' | b',' | b';' | b':' | b'!' | b'?'
                            );
                        if !last_ch.is_ascii_whitespace() && last_ch != b'*' && after_ok {
                            out.push_str(open_tag);
                            out.push_str(&text[i + 1..end]);
                            out.push_str(close_tag);
                            i = end + 1;
                            continue;
                        }
                    }
                }
            }
        }
        let ch = text[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn inline(text: &str) -> String {
    static CODE_RE: OnceLock<Regex> = OnceLock::new();
    static LINK_RE: OnceLock<Regex> = OnceLock::new();
    static BOLD_RE: OnceLock<Regex> = OnceLock::new();
    static STRIKE_RE: OnceLock<Regex> = OnceLock::new();

    let code_re = CODE_RE.get_or_init(|| Regex::new(r"`[^`\n]+`").unwrap());
    let link_re = LINK_RE.get_or_init(|| Regex::new(r"\[([^\]\n]+)\]\((https?://[^\s)<>]+)\)").unwrap());
    let bold_re = BOLD_RE.get_or_init(|| Regex::new(r"\*\*([^\s*\n](?:[^*\n]*?[^\s*\n])?)\*\*").unwrap());
    let strike_re = STRIKE_RE.get_or_init(|| Regex::new(r"~~([^\s~\n](?:[^~\n]*?[^\s~\n])?)~~").unwrap());

    let mut out = String::new();
    let mut last = 0;
    for m in code_re.find_iter(text) {
        if m.start() > last {
            let plain = &text[last..m.start()];
            let mut html = telegram_escape(plain);
            html = link_re
                .replace_all(&html, |c: &regex::Captures| {
                    let url = c[2].replace('"', "&quot;");
                    format!("<a href=\"{url}\">{}</a>", &c[1])
                })
                .into_owned();
            html = bold_re.replace_all(&html, "<b>$1</b>").into_owned();
            html = replace_bounded_italic(&html, "<i>", "</i>");
            html = strike_re.replace_all(&html, "<s>$1</s>").into_owned();
            out.push_str(&html);
        }
        let inner = &m.as_str()[1..m.as_str().len() - 1];
        out.push_str("<code>");
        out.push_str(&telegram_escape(inner));
        out.push_str("</code>");
        last = m.end();
    }
    if last < text.len() {
        let plain = &text[last..];
        let mut html = telegram_escape(plain);
        html = link_re
            .replace_all(&html, |c: &regex::Captures| {
                let url = c[2].replace('"', "&quot;");
                format!("<a href=\"{url}\">{}</a>", &c[1])
            })
            .into_owned();
        html = bold_re.replace_all(&html, "<b>$1</b>").into_owned();
        html = replace_bounded_italic(&html, "<i>", "</i>");
        html = strike_re.replace_all(&html, "<s>$1</s>").into_owned();
        out.push_str(&html);
    }
    out
}

fn pre_of(text: &str, language: Option<&str>) -> String {
    static LANG_RE: OnceLock<Regex> = OnceLock::new();
    let lang_re = LANG_RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9+#-]{1,20}$").unwrap());
    match language {
        Some(lang) if lang_re.is_match(lang) => {
            format!("<pre><code class=\"language-{lang}\">{}</code></pre>", telegram_escape(text))
        }
        _ => format!("<pre>{}</pre>", telegram_escape(text)),
    }
}

fn pre_pieces(text: &str, language: Option<&str>, max: usize) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let flush = |pieces: &mut Vec<String>, lines: &mut Vec<String>| {
        if !lines.is_empty() {
            pieces.push(pre_of(&lines.join("\n"), language));
            lines.clear();
        }
    };
    for line in text.split('\n') {
        let cut = if line.chars().count() > max.saturating_sub(60) {
            let prefix: String = line.chars().take(max.saturating_sub(61)).collect();
            format!("{prefix}…")
        } else {
            line.to_owned()
        };
        let mut candidate = lines.join("\n");
        if !candidate.is_empty() {
            candidate.push('\n');
        }
        candidate.push_str(&cut);
        if telegram_escape(&candidate).chars().count() + 60 > max {
            flush(&mut pieces, &mut lines);
        }
        lines.push(cut);
    }
    flush(&mut pieces, &mut lines);
    pieces
}

pub fn plain_of(html: &str) -> String {
    static TAG_RE: OnceLock<Regex> = OnceLock::new();
    let tag_re = TAG_RE.get_or_init(|| Regex::new(r"<[^>]+>").unwrap());
    tag_re
        .replace_all(html, "")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
}

pub fn telegram_blocks(markdown: &str, max: usize) -> Vec<String> {
    static FENCE_RE: OnceLock<Regex> = OnceLock::new();
    static TABLE_RE: OnceLock<Regex> = OnceLock::new();
    static TABLE_SEP_RE: OnceLock<Regex> = OnceLock::new();
    static HEADING_RE: OnceLock<Regex> = OnceLock::new();
    static RULE_RE: OnceLock<Regex> = OnceLock::new();
    static BULLET_RE: OnceLock<Regex> = OnceLock::new();

    let fence_re = FENCE_RE.get_or_init(|| Regex::new(r"^\s*(```+|~~~+)\s*([^\s`]*)\s*$").unwrap());
    let table_re = TABLE_RE.get_or_init(|| Regex::new(r"^\s*\|.*\|\s*$").unwrap());
    let table_sep_re = TABLE_SEP_RE.get_or_init(|| Regex::new(r"^\s*\|[\s:|-]+\|\s*$").unwrap());
    let heading_re = HEADING_RE.get_or_init(|| Regex::new(r"^\s*#{1,6}\s+(.*?)\s*#*\s*$").unwrap());
    let rule_re = RULE_RE.get_or_init(|| Regex::new(r"^\s*(?:-{3,}|\*{3,}|_{3,})\s*$").unwrap());
    let bullet_re = BULLET_RE.get_or_init(|| Regex::new(r"^(\s*)[-*+]\s+(.*)$").unwrap());

    let normalized = markdown.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut blocks = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();

    let end_paragraph = |blocks: &mut Vec<String>, paragraph: &mut Vec<String>| {
        if paragraph.is_empty() {
            return;
        }
        let html = paragraph.join("\n");
        paragraph.clear();
        if html.chars().count() <= max {
            blocks.push(html);
            return;
        }
        let mut rest = plain_of(&html);
        while !rest.is_empty() {
            let room = max.saturating_sub(20);
            let char_len = rest.chars().count();
            let split_chars = if char_len <= room {
                char_len
            } else {
                let prefix: String = rest.chars().take(room).collect();
                match prefix.rfind(' ') {
                    Some(byte_pos) => {
                        let c = prefix[..byte_pos].chars().count();
                        c.max(room / 2)
                    }
                    None => room / 2,
                }
            };
            let byte_end = rest.char_indices().nth(split_chars).map(|(i, _)| i).unwrap_or(rest.len());
            blocks.push(telegram_escape(&rest[..byte_end]));
            rest = rest[byte_end..].trim_start().to_owned();
        }
    };

    let mut at = 0usize;
    while at < lines.len() {
        let line = lines[at];
        if let Some(caps) = fence_re.captures(line) {
            end_paragraph(&mut blocks, &mut paragraph);
            let marker = &caps[1];
            let lang = if caps[2].is_empty() { None } else { Some(&caps[2]) };
            let mut body = Vec::new();
            let mut end = at + 1;
            while end < lines.len() && !lines[end].trim().starts_with(marker) {
                body.push(lines[end]);
                end += 1;
            }
            at = end + 1;
            blocks.extend(pre_pieces(&body.join("\n"), lang, max));
            continue;
        }
        if table_re.is_match(line) {
            end_paragraph(&mut blocks, &mut paragraph);
            let mut table = Vec::new();
            while at < lines.len() && table_re.is_match(lines[at]) {
                if !table_sep_re.is_match(lines[at]) {
                    table.push(lines[at].trim());
                }
                at += 1;
            }
            blocks.extend(pre_pieces(&table.join("\n"), None, max));
            continue;
        }
        if line.trim_start().starts_with('>') {
            end_paragraph(&mut blocks, &mut paragraph);
            let mut quoted = Vec::new();
            while at < lines.len() && lines[at].trim_start().starts_with('>') {
                let stripped = lines[at].trim_start().strip_prefix('>').unwrap_or("");
                let stripped = stripped.strip_prefix(' ').unwrap_or(stripped);
                quoted.push(inline(stripped));
                at += 1;
            }
            blocks.push(format!("<blockquote>{}</blockquote>", quoted.join("\n")));
            continue;
        }
        if line.trim().is_empty() {
            end_paragraph(&mut blocks, &mut paragraph);
            at += 1;
            continue;
        }
        if let Some(caps) = heading_re.captures(line) {
            end_paragraph(&mut blocks, &mut paragraph);
            blocks.push(format!("<b>{}</b>", inline(&caps[1])));
            at += 1;
            continue;
        }
        if rule_re.is_match(line) || is_spaced_rule(line) {
            end_paragraph(&mut blocks, &mut paragraph);
            at += 1;
            continue;
        }
        if let Some(caps) = bullet_re.captures(line) {
            let indent = (caps[1].chars().count() / 2).min(3);
            paragraph.push(format!("{}• {}", "  ".repeat(indent), inline(&caps[2])));
        } else {
            paragraph.push(inline(line));
        }
        at += 1;
    }
    end_paragraph(&mut blocks, &mut paragraph);
    blocks
}

fn is_spaced_rule(line: &str) -> bool {
    let trimmed = line.trim();
    let Some(first) = trimmed.chars().next() else { return false };
    if !matches!(first, '-' | '*' | '_') {
        return false;
    }
    let mut count = 0;
    for ch in trimmed.chars() {
        if ch == first {
            count += 1;
        } else if !ch.is_whitespace() {
            return false;
        }
    }
    count >= 3
}

pub fn packed_messages(blocks: &[String], max: usize) -> Vec<String> {
    let mut messages = Vec::new();
    let mut current = String::new();
    for block in blocks {
        if !current.is_empty() && current.chars().count() + 2 + block.chars().count() > max {
            messages.push(current);
            current = String::new();
        }
        if current.is_empty() {
            current = block.clone();
        } else {
            current.push_str("\n\n");
            current.push_str(block);
        }
    }
    if !current.is_empty() {
        messages.push(current);
    }
    messages
}

pub fn answer_messages(name: &str, answer: &str, max: usize) -> Vec<String> {
    let mut all = vec![format!("<b>{}</b>", telegram_escape(name))];
    all.extend(telegram_blocks(answer, max.saturating_sub(40)));
    let messages = packed_messages(&all, max);
    if messages.len() <= TELEGRAM_MAX_PARTS {
        return messages;
    }
    let mut kept = messages[..TELEGRAM_MAX_PARTS].to_vec();
    let last = &kept[TELEGRAM_MAX_PARTS - 1];
    kept[TELEGRAM_MAX_PARTS - 1] = if last.chars().count() + 2 + TELEGRAM_REST_IN_LOCUST.chars().count() <= max {
        format!("{last}\n\n{TELEGRAM_REST_IN_LOCUST}")
    } else {
        TELEGRAM_REST_IN_LOCUST.to_owned()
    };
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_apps::{ChatTeammateRef, teammate_commands};

    const ANSWERS: &[&str] = &[
        "# Plan\n\nFirst **read** the `config.ts` file, then *run* the tests.\n\n- one\n- two\n  - nested\n\n1. first\n2. second",
        "```ts\nconst a = 1 < 2 && b > 3\nconsole.log(\"<b>not a tag</b>\")\n```\n\nDone.",
        "| File | Lines |\n|---|---|\n| a.ts | 12 |\n| b & c.ts | 40 |\n\nThat is the table.",
        "> A quote with **bold**\n> and a [link](https://example.com/a?b=1&c=2)\n\nAfter it.",
        "Snake_case_names and 2 * 3 * 4 and a lone * star, an <angle> & an ampersand.",
        "**unclosed bold and `unclosed code and [broken](link",
        "~~struck~~ and __not bold here__ and ***both*** and ** spaced **.",
        "A link [with **bold** inside](https://example.com) and **bold [with a link](https://example.com) inside**.",
        "```\nno language\n```\n~~~python\nprint(\"tilde fence\")\n~~~",
        "---\n\nAfter a rule.\n\n***\n",
        "",
    ];

    fn html_problem(html: &str) -> Option<String> {
        let mut stack: Vec<String> = Vec::new();
        let mut rest = html;
        while let Some(lt) = rest.find('<') {
            let after = &rest[lt + 1..];
            let Some(gt) = after.find('>') else {
                return Some("unclosed <".into());
            };
            let tag = &after[..gt];
            if let Some(close_name) = tag.strip_prefix('/') {
                match stack.pop() {
                    Some(open) if open == close_name => {}
                    other => return Some(format!("mismatched </{close_name}> vs {other:?}")),
                }
            } else {
                let name = tag.split_whitespace().next().unwrap_or("");
                if !matches!(name, "b" | "i" | "s" | "code" | "pre" | "blockquote" | "a") {
                    return Some(format!("unknown tag <{name}>"));
                }
                stack.push(name.to_owned());
            }
            rest = &after[gt + 1..];
        }
        if stack.is_empty() { None } else { Some(format!("unclosed tags: {stack:?}")) }
    }

    #[test]
    fn turns_every_answer_into_html_telegram_takes_in_messages_under_its_limit() {
        for answer in ANSWERS {
            let messages = answer_messages("Ada", answer, TELEGRAM_MESSAGE_MAX);
            assert!(messages[0].starts_with("<b>Ada</b>"), "for {answer}");
            for message in messages {
                assert_eq!(html_problem(&message), None, "{answer}\n=>\n{message}");
                assert!(message.chars().count() <= TELEGRAM_MESSAGE_MAX);
            }
        }
    }

    #[test]
    fn keeps_what_markdown_meant_bold_code_blocks_of_code_links_lists_quotes_and_tables() {
        let plan = &answer_messages("Ada", ANSWERS[0], TELEGRAM_MESSAGE_MAX)[0];
        assert!(plan.contains("<b>Plan</b>"));
        assert!(plan.contains("<b>read</b>"));
        assert!(plan.contains("<code>config.ts</code>"));
        assert!(plan.contains("<i>run</i>"));
        assert!(plan.contains("• one"));
        assert!(plan.contains("  • nested"));

        let code = &answer_messages("Ada", ANSWERS[1], TELEGRAM_MESSAGE_MAX)[0];
        assert!(code.contains("<pre><code class=\"language-ts\">const a = 1 &lt; 2 &amp;&amp; b &gt; 3"));
        assert!(code.contains("&lt;b&gt;not a tag&lt;/b&gt;"));

        let table = &answer_messages("Ada", ANSWERS[2], TELEGRAM_MESSAGE_MAX)[0];
        assert!(table.contains("<pre>| File | Lines |\n| a.ts | 12 |\n| b &amp; c.ts | 40 |</pre>"));

        let quote = &answer_messages("Ada", ANSWERS[3], TELEGRAM_MESSAGE_MAX)[0];
        assert!(quote.contains("<blockquote>A quote with <b>bold</b>\nand a <a href=\"https://example.com/a?b=1&amp;c=2\">link</a></blockquote>"));

        let plain = &answer_messages("Ada", ANSWERS[4], TELEGRAM_MESSAGE_MAX)[0];
        assert!(plain.contains("Snake_case_names and 2 * 3 * 4 and a lone * star, an &lt;angle&gt; &amp; an ampersand."));
    }

    #[test]
    fn splits_a_long_answer_between_blocks_and_says_where_the_rest_is_past_four_messages() {
        let paragraphs: String = (0..40)
            .map(|i| format!("Paragraph {i}: {}", "word ".repeat(80)))
            .collect::<Vec<_>>()
            .join("\n\n");
        let messages = answer_messages("Ada", &paragraphs, TELEGRAM_MESSAGE_MAX);
        assert_eq!(messages.len(), TELEGRAM_MAX_PARTS);
        assert!(messages.last().unwrap().ends_with(TELEGRAM_REST_IN_LOCUST));
        for message in messages {
            assert_eq!(html_problem(&message), None);
            assert!(message.chars().count() <= TELEGRAM_MESSAGE_MAX);
        }
        assert_eq!(answer_messages("Ada", "Done.", TELEGRAM_MESSAGE_MAX), vec!["<b>Ada</b>\n\nDone."]);
    }

    #[test]
    fn cuts_a_block_of_code_too_long_for_one_message_between_its_lines_each_piece_whole() {
        let code = format!(
            "```js\n{}\n```",
            (0..400).map(|i| format!("const line{i} = \"{}\" // <{i}>", "x".repeat(20))).collect::<Vec<_>>().join("\n")
        );
        let blocks = telegram_blocks(&code, TELEGRAM_MESSAGE_MAX);
        assert!(blocks.len() > 1);
        for block in &blocks {
            assert!(block.starts_with("<pre><code class=\"language-js\">"));
            assert_eq!(html_problem(block), None);
            assert!(block.chars().count() <= TELEGRAM_MESSAGE_MAX);
        }
        let line_count = blocks.iter().map(|b| plain_of(b)).collect::<Vec<_>>().join("\n").split('\n').count();
        assert_eq!(line_count, 400);

        let long = telegram_blocks(&"word ".repeat(2_000), TELEGRAM_MESSAGE_MAX);
        assert!(long.len() > 1);
        for block in long {
            assert!(block.chars().count() <= TELEGRAM_MESSAGE_MAX);
        }
        assert_eq!(packed_messages(&["a".into(), "b".into()], TELEGRAM_MESSAGE_MAX), vec!["a\n\nb"]);
    }

    #[test]
    fn names_each_teammate_by_a_command_telegram_allows() {
        let commands = teammate_commands(&[
            ChatTeammateRef { teammate_id: "t1".into(), name: "Ada".into() },
            ChatTeammateRef { teammate_id: "t2".into(), name: "ada".into() },
            ChatTeammateRef { teammate_id: "t3".into(), name: "Help".into() },
            ChatTeammateRef { teammate_id: "t4".into(), name: "Zoë Ng-Smith".into() },
            ChatTeammateRef { teammate_id: "t5".into(), name: "Борис".into() },
            ChatTeammateRef { teammate_id: "t6".into(), name: "A really long teammate name that goes past thirty-two letters".into() },
            ChatTeammateRef { teammate_id: "t7".into(), name: "Who".into() },
        ]);
        let vals: Vec<&str> = commands.iter().map(|(_, v)| v.as_str()).collect();
        assert_eq!(vals, ["ada", "ada2", "help2", "zoe_ng_smith", "teammate", "a_really_long_teammate_name", "who2"]);
    }

    #[test]
    fn reads_a_command_as_telegram_sends_one_and_leaves_another_bots_alone() {
        assert_eq!(parsed_command("/ada check the build", "locust_test_bot"), Some(ParsedCommand { command: "ada".into(), rest: "check the build".into() }));
        assert_eq!(parsed_command("/Ada@Locust_Test_Bot  hello\nthere ", "locust_test_bot"), Some(ParsedCommand { command: "ada".into(), rest: "hello\nthere".into() }));
        assert_eq!(parsed_command("/ada@other_bot hello", "locust_test_bot"), None);
        assert_eq!(parsed_command("just words", "locust_test_bot"), None);
        assert_eq!(parsed_command("/start Zx9_-abc", "locust_test_bot"), Some(ParsedCommand { command: "start".into(), rest: "Zx9_-abc".into() }));
    }

    #[test]
    fn knows_a_bots_token_by_its_shape() {
        assert!(is_telegram_token("123456789:AAHfakefakefakefakefakefakefakefake1"));
        assert!(!is_telegram_token("123456789:short"));
        assert!(!is_telegram_token(" 123456789:AAHfakefakefakefakefakefakefakefake1"));
        assert!(!is_telegram_token("sk-ant-123456789012345678901234567890")); // public-guard: allow (a made-up key in a test)
    }
}

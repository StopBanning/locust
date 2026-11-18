//! Discord Markdown formatter and token/invite helpers (Electron `shared/discord.ts`, 0.832).

use crate::chat_apps::{CHAT_MAX_PARTS, CHAT_REST_IN_LOCUST};
use regex::Regex;
use std::sync::OnceLock;

/// Discord takes 2,000 characters in one message; this leaves room for the header and a closing fence.
pub const DISCORD_MESSAGE_MAX: usize = 1_900;
pub const DISCORD_REST_IN_LOCUST: &str = "*The rest is in Locust on your computer.*";
pub const DISCORD_DEVELOPER_PORTAL: &str = "https://discord.com/developers/applications";

pub fn is_discord_token(s: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9_-]{20,40}\.[A-Za-z0-9_-]{4,10}\.[A-Za-z0-9_-]{20,80}$").unwrap())
        .is_match(s)
}

/// The link that adds the bot to a server of the person's: no permissions asked, since it only answers them directly.
pub fn discord_invite(application_id: &str) -> String {
    let encoded: String = url::form_urlencoded::byte_serialize(application_id.as_bytes()).collect();
    format!("https://discord.com/oauth2/authorize?client_id={encoded}&scope=bot%20applications.commands&permissions=0")
}

/// Plain text as Discord shows it: every mark Discord's Markdown reads is escaped, and a line's opening that would make
/// a heading, a quote or a list is too.
pub fn discord_escape(text: &str) -> String {
    static MARK_RE: OnceLock<Regex> = OnceLock::new();
    static LINE_OPEN_RE: OnceLock<Regex> = OnceLock::new();
    static NUM_LIST_RE: OnceLock<Regex> = OnceLock::new();

    let mark_re = MARK_RE.get_or_init(|| Regex::new(r"[\\*_~`|\[\]<@]").unwrap());
    let line_open_re = LINE_OPEN_RE.get_or_init(|| Regex::new(r"(?m)^([ \t]*)([#>+-])").unwrap());
    let num_list_re = NUM_LIST_RE.get_or_init(|| Regex::new(r"(?m)^([ \t]*\d+)\.").unwrap());

    let s = mark_re.replace_all(text, r"\$0");
    let s = line_open_re.replace_all(&s, r"${1}\$2");
    num_list_re.replace_all(&s, r"${1}\.").into_owned()
}

fn fence_of(language: Option<&str>) -> String {
    static LANG_RE: OnceLock<Regex> = OnceLock::new();
    let lang_re = LANG_RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9+#-]{1,20}$").unwrap());
    match language {
        Some(lang) if lang_re.is_match(lang) => format!("```{lang}"),
        _ => "```".to_owned(),
    }
}

fn code_pieces(text: &str, language: Option<&str>, max: usize) -> Vec<String> {
    let open = fence_of(language);
    let mut pieces = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let flush = |pieces: &mut Vec<String>, lines: &mut Vec<String>| {
        if !lines.is_empty() {
            pieces.push(format!("{open}\n{}\n```", lines.join("\n")));
            lines.clear();
        }
    };
    let safe = text.replace("```", "`\u{200b}``");
    for line in safe.split('\n') {
        let cut = if line.chars().count() > max.saturating_sub(40) {
            let prefix: String = line.chars().take(max.saturating_sub(41)).collect();
            format!("{prefix}…")
        } else {
            line.to_owned()
        };
        let mut candidate = lines.join("\n");
        if !candidate.is_empty() {
            candidate.push('\n');
        }
        candidate.push_str(&cut);
        if candidate.chars().count() + open.chars().count() + 8 > max {
            flush(&mut pieces, &mut lines);
        }
        lines.push(cut);
    }
    flush(&mut pieces, &mut lines);
    pieces
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

pub fn discord_blocks(markdown: &str, max: usize) -> Vec<String> {
    static FENCE_RE: OnceLock<Regex> = OnceLock::new();
    static TABLE_RE: OnceLock<Regex> = OnceLock::new();
    static TABLE_SEP_RE: OnceLock<Regex> = OnceLock::new();
    static HEADING_RE: OnceLock<Regex> = OnceLock::new();
    static IMG_RE: OnceLock<Regex> = OnceLock::new();

    let fence_re = FENCE_RE.get_or_init(|| Regex::new(r"^\s*(```+|~~~+)\s*([^\s`]*)\s*$").unwrap());
    let table_re = TABLE_RE.get_or_init(|| Regex::new(r"^\s*\|.*\|\s*$").unwrap());
    let table_sep_re = TABLE_SEP_RE.get_or_init(|| Regex::new(r"^\s*\|[\s:|-]+\|\s*$").unwrap());
    let heading_re = HEADING_RE.get_or_init(|| Regex::new(r"^\s*(#{1,6})\s+(.*?)\s*#*\s*$").unwrap());
    let img_re = IMG_RE.get_or_init(|| Regex::new(r"!\[([^\]\n]*)\]\((https?://[^\s)]+)\)").unwrap());

    let normalized = markdown.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut blocks = Vec::new();
    let mut paragraph: Vec<String> = Vec::new();

    let end_paragraph = |blocks: &mut Vec<String>, paragraph: &mut Vec<String>| {
        if paragraph.is_empty() {
            return;
        }
        let mut rest = paragraph.join("\n");
        paragraph.clear();
        while rest.chars().count() > max {
            let room = max.saturating_sub(20);
            let prefix: String = rest.chars().take(room).collect();
            let line_end = prefix.rfind('\n').map(|b| prefix[..b].chars().count());
            let split_chars = match line_end {
                Some(c) if c > room / 2 => c,
                _ => prefix.rfind(' ').map(|b| prefix[..b].chars().count()).unwrap_or(0).max(room / 2),
            };
            let byte_end = rest.char_indices().nth(split_chars).map(|(i, _)| i).unwrap_or(rest.len());
            blocks.push(rest[..byte_end].trim_end().to_owned());
            rest = rest[byte_end..].trim_start().to_owned();
        }
        if !rest.is_empty() {
            blocks.push(rest);
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
            blocks.extend(code_pieces(&body.join("\n"), lang, max));
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
            blocks.extend(code_pieces(&table.join("\n"), None, max));
            continue;
        }
        if line.trim().is_empty() || is_spaced_rule(line) {
            end_paragraph(&mut blocks, &mut paragraph);
            at += 1;
            continue;
        }
        if let Some(caps) = heading_re.captures(line) {
            end_paragraph(&mut blocks, &mut paragraph);
            let hashes = &caps[1];
            let text = &caps[2];
            if hashes.len() <= 3 {
                blocks.push(format!("{hashes} {text}"));
            } else {
                blocks.push(format!("**{text}**"));
            }
            at += 1;
            continue;
        }
        let replaced = img_re
            .replace_all(line, |c: &regex::Captures| {
                let label = if c[1].is_empty() { "picture" } else { &c[1] };
                format!("[{label}]({})", &c[2])
            })
            .into_owned();
        paragraph.push(replaced);
        at += 1;
    }
    end_paragraph(&mut blocks, &mut paragraph);
    blocks
}

pub fn discord_packed(blocks: &[String], max: usize) -> Vec<String> {
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

pub fn discord_messages(name: &str, answer: &str, max: usize) -> Vec<String> {
    let _ = CHAT_REST_IN_LOCUST;
    let mut all = vec![format!("**{}**", discord_escape(name))];
    all.extend(discord_blocks(answer, max.saturating_sub(40)));
    let messages = discord_packed(&all, max);
    if messages.len() <= CHAT_MAX_PARTS {
        return messages;
    }
    let mut kept = messages[..CHAT_MAX_PARTS].to_vec();
    let last = &kept[CHAT_MAX_PARTS - 1];
    kept[CHAT_MAX_PARTS - 1] = if last.chars().count() + 2 + DISCORD_REST_IN_LOCUST.chars().count() <= max {
        format!("{last}\n\n{DISCORD_REST_IN_LOCUST}")
    } else {
        DISCORD_REST_IN_LOCUST.to_owned()
    };
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    const ANSWERS: &[&str] = &[
        "# Plan\n\nFirst **read** the `config.ts` file, then *run* the tests.\n\n- one\n- two\n  - nested\n\n1. first\n2. second",
        "```ts\nconst a = 1 < 2 && b > 3\nconsole.log(\"<b>not a tag</b>\")\n```\n\nDone.",
        "| File | Lines |\n|---|---|\n| a.ts | 12 |\n| b & c.ts | 40 |\n\nThat is the table.",
        "> A quote with **bold**\n> and a [link](https://example.com/a?b=1&c=2)\n\nAfter it.",
        "#### A small heading\n\n![the chart](https://example.com/chart.png)\n\n---\n\nAfter a rule.",
        "**unclosed bold and `unclosed code and [broken](link",
        "```\ncode with ``` inside it\n```",
        "",
    ];

    fn fences(message: &str) -> usize {
        message.split('\n').filter(|line| line.starts_with("```")).count()
    }

    #[test]
    fn turns_every_answer_into_messages_under_discords_limit_each_block_closed() {
        for answer in ANSWERS {
            let messages = discord_messages("Ada", answer, DISCORD_MESSAGE_MAX);
            assert!(messages[0].starts_with("**Ada**"), "for {answer}");
            for message in messages {
                assert!(message.chars().count() <= 2_000, "{message}");
                assert_eq!(fences(&message) % 2, 0, "{message}");
            }
        }
    }

    #[test]
    fn keeps_what_discord_reads_and_makes_the_rest_into_what_it_does() {
        let plan = &discord_messages("Ada", ANSWERS[0], DISCORD_MESSAGE_MAX)[0];
        assert!(plan.contains("# Plan"));
        assert!(plan.contains("First **read** the `config.ts` file, then *run* the tests."));
        assert!(plan.contains("- one\n- two\n  - nested"));

        let code = &discord_messages("Ada", ANSWERS[1], DISCORD_MESSAGE_MAX)[0];
        assert!(code.contains("```ts\nconst a = 1 < 2 && b > 3\nconsole.log(\"<b>not a tag</b>\")\n```"));

        let table = &discord_messages("Ada", ANSWERS[2], DISCORD_MESSAGE_MAX)[0];
        assert!(table.contains("```\n| File | Lines |\n| a.ts | 12 |\n| b & c.ts | 40 |\n```"));

        let quote = &discord_messages("Ada", ANSWERS[3], DISCORD_MESSAGE_MAX)[0];
        assert!(quote.contains("> A quote with **bold**\n> and a [link](https://example.com/a?b=1&c=2)"));

        let small = &discord_messages("Ada", ANSWERS[4], DISCORD_MESSAGE_MAX)[0];
        assert_eq!(small, "**Ada**\n\n**A small heading**\n\n[the chart](https://example.com/chart.png)\n\nAfter a rule.");

        let inside = &discord_messages("Ada", ANSWERS[6], DISCORD_MESSAGE_MAX)[0];
        assert_eq!(fences(inside), 2);
    }

    #[test]
    fn splits_a_long_answer_between_blocks_and_says_where_the_rest_is_past_four_messages() {
        let paragraphs: String = (0..40)
            .map(|i| format!("Paragraph {i}: {}", "word ".repeat(80)))
            .collect::<Vec<_>>()
            .join("\n\n");
        let messages = discord_messages("Ada", &paragraphs, DISCORD_MESSAGE_MAX);
        assert_eq!(messages.len(), CHAT_MAX_PARTS);
        assert!(messages.last().unwrap().ends_with(DISCORD_REST_IN_LOCUST));
        for message in messages {
            assert!(message.chars().count() <= DISCORD_MESSAGE_MAX);
        }
        assert_eq!(discord_messages("Ada", "Done.", DISCORD_MESSAGE_MAX), vec!["**Ada**\n\nDone."]);
    }

    #[test]
    fn cuts_a_block_of_code_too_long_for_one_message_between_its_lines_each_piece_fenced_again() {
        let code = format!(
            "```js\n{}\n```",
            (0..300).map(|i| format!("const line{i} = \"{}\"", "x".repeat(20))).collect::<Vec<_>>().join("\n")
        );
        let blocks = discord_blocks(&code, DISCORD_MESSAGE_MAX);
        assert!(blocks.len() > 1);
        for block in &blocks {
            assert!(block.starts_with("```js\n"));
            assert!(block.ends_with("\n```"));
            assert!(block.chars().count() <= DISCORD_MESSAGE_MAX);
        }
        let line_count: usize = blocks.iter().map(|b| b.split('\n').count().saturating_sub(2)).sum();
        assert_eq!(line_count, 300);

        let long = discord_blocks(&"word ".repeat(2_000), DISCORD_MESSAGE_MAX);
        assert!(long.len() > 1);
        for block in long {
            assert!(block.chars().count() <= DISCORD_MESSAGE_MAX);
        }
    }

    #[test]
    fn escapes_plain_words_so_none_of_them_is_read_as_a_mark() {
        assert_eq!(
            discord_escape("snake_case *star* `tick` ~strike~ |pipe| [a](b) <@123> @everyone \\ back"),
            "snake\\_case \\*star\\* \\`tick\\` \\~strike\\~ \\|pipe\\| \\[a\\](b) \\<\\@123> \\@everyone \\\\ back"
        );
        assert_eq!(
            discord_escape("# not a heading\n> not a quote\n- not a list\n1. not a number"),
            "\\# not a heading\n\\> not a quote\n\\- not a list\n1\\. not a number"
        );
        assert_eq!(
            discord_escape("Settings > General - Chat apps #2"),
            "Settings > General - Chat apps #2"
        );
    }

    #[test]
    fn knows_a_bots_token_by_its_shape_and_adds_the_bot_asking_no_permissions() {
        assert!(is_discord_token("MTEwMDAwMDAwMDAwMDAwMDAwMQ.GfakeA.fake_fake_fake_fake_fake_fake_fake_fak"));
        assert!(!is_discord_token("MTEwMDAwMDAwMDAwMDAwMDAwMQ.GfakeA"));
        assert!(!is_discord_token("123456789:AAHfakefakefakefakefakefakefakefake1"));
        assert_eq!(
            discord_invite("1100000000000000001"),
            "https://discord.com/oauth2/authorize?client_id=1100000000000000001&scope=bot%20applications.commands&permissions=0"
        );
    }
}

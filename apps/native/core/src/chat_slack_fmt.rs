//! Slack mrkdwn formatter, plain-text decoder, token validators, and Socket Mode manifest (Electron `shared/slack.ts`, 0.832).

use crate::chat_apps::{CHAT_MAX_PARTS, CHAT_REST_IN_LOCUST};
use crate::chat_telegram_fmt::replace_bounded_italic;
use regex::Regex;
use serde_json::{Value, json};
use std::sync::OnceLock;

/// Slack takes this much in one message's words; this leaves room for the header and a closing fence.
pub const SLACK_MESSAGE_MAX: usize = 3_800;
pub const SLACK_REST_IN_LOCUST: &str = "_The rest is in Locust on your computer._";
pub const SLACK_APPS_PAGE: &str = "https://api.slack.com/apps";

pub fn is_slack_bot_token(s: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^xoxb-[A-Za-z0-9-]{20,}$").unwrap()).is_match(s)
}

pub fn is_slack_app_token(s: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^xapp-\d-[A-Za-z0-9-]{20,}$").unwrap()).is_match(s)
}

pub fn slack_manifest() -> Value {
    json!({
        "_metadata": { "major_version": 1, "minor_version": 1 },
        "display_information": {
            "name": "Locust",
            "description": "Your Locust teammates, in Slack. It answers while Locust is open on your computer.",
            "background_color": "#1b1d22"
        },
        "features": {
            "app_home": { "home_tab_enabled": false, "messages_tab_enabled": true, "messages_tab_read_only_enabled": false },
            "bot_user": { "display_name": "Locust", "always_online": true }
        },
        "oauth_config": { "scopes": { "bot": ["chat:write", "im:history", "reactions:write", "users:read"] } },
        "settings": {
            "event_subscriptions": { "bot_events": ["message.im"] },
            "interactivity": { "is_enabled": true },
            "org_deploy_enabled": false,
            "socket_mode_enabled": true,
            "token_rotation_enabled": false
        }
    })
}

pub fn slack_manifest_link() -> String {
    let encoded: String = url::form_urlencoded::byte_serialize(slack_manifest().to_string().as_bytes()).collect();
    format!("https://api.slack.com/apps?new_app=1&manifest_json={encoded}")
}

/// Text made safe in Slack's markup: only these three are its own.
pub fn slack_escape(text: &str) -> String {
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

/// A person's words as Slack sent them, back to plain: its links, mentions and the three escaped marks.
pub fn slack_plain(text: &str) -> String {
    static LABELED_LINK_RE: OnceLock<Regex> = OnceLock::new();
    static BARE_LINK_RE: OnceLock<Regex> = OnceLock::new();
    static CHANNEL_RE: OnceLock<Regex> = OnceLock::new();
    static USER_RE: OnceLock<Regex> = OnceLock::new();
    static SPECIAL_RE: OnceLock<Regex> = OnceLock::new();

    let labeled = LABELED_LINK_RE.get_or_init(|| Regex::new(r"<((?:https?|mailto):[^|>]+)\|([^>]+)>").unwrap());
    let bare = BARE_LINK_RE.get_or_init(|| Regex::new(r"<((?:https?|mailto):[^>]+)>").unwrap());
    let channel = CHANNEL_RE.get_or_init(|| Regex::new(r"<#[A-Z0-9]+\|([^>]+)>").unwrap());
    let user = USER_RE.get_or_init(|| Regex::new(r"<@[A-Z0-9]+>").unwrap());
    let special = SPECIAL_RE.get_or_init(|| Regex::new(r"<!(here|channel|everyone)>").unwrap());

    let s = labeled.replace_all(text, |c: &regex::Captures| {
        let url = &c[1];
        let label = &c[2];
        let stripped = url.strip_prefix("mailto:").unwrap_or(url);
        if stripped == label || url == label {
            label.to_owned()
        } else {
            format!("{label} ({url})")
        }
    });
    let s = bare.replace_all(&s, |c: &regex::Captures| {
        let url = &c[1];
        url.strip_prefix("mailto:").unwrap_or(url).to_owned()
    });
    let s = channel.replace_all(&s, "#$1");
    let s = user.replace_all(&s, "@someone");
    let s = special.replace_all(&s, "@$1");
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

fn inline(text: &str) -> String {
    static CODE_RE: OnceLock<Regex> = OnceLock::new();
    static LINK_RE: OnceLock<Regex> = OnceLock::new();
    static BOLD_STAR_RE: OnceLock<Regex> = OnceLock::new();
    static BOLD_UNDER_RE: OnceLock<Regex> = OnceLock::new();
    static STRIKE_RE: OnceLock<Regex> = OnceLock::new();

    let code_re = CODE_RE.get_or_init(|| Regex::new(r"`[^`\n]+`").unwrap());
    let link_re = LINK_RE.get_or_init(|| Regex::new(r"!?\[([^\]\n]+)\]\((https?://[^\s)<>]+)\)").unwrap());
    let bold_star = BOLD_STAR_RE.get_or_init(|| Regex::new(r"\*\*([^\s*\n](?:[^*\n]*?[^\s*\n])?)\*\*").unwrap());
    let bold_under = BOLD_UNDER_RE.get_or_init(|| Regex::new(r"__([^\s_\n](?:[^_\n]*?[^\s_\n])?)__").unwrap());
    let strike_re = STRIKE_RE.get_or_init(|| Regex::new(r"~~([^\s~\n](?:[^~\n]*?[^\s~\n])?)~~").unwrap());

    let format_plain = |plain: &str| -> String {
        let mut out = slack_escape(plain);
        out = link_re
            .replace_all(&out, |c: &regex::Captures| {
                let label = c[1].replace(['|', '<', '>'], " ");
                format!("<{}|{label}>", &c[2])
            })
            .into_owned();
        out = bold_star.replace_all(&out, "\u{0001}$1\u{0001}").into_owned();
        out = bold_under.replace_all(&out, "\u{0001}$1\u{0001}").into_owned();
        out = replace_bounded_italic(&out, "_", "_");
        out = strike_re.replace_all(&out, "~$1~").into_owned();
        out.replace('\u{0001}', "*")
    };

    let mut out = String::new();
    let mut last = 0;
    for m in code_re.find_iter(text) {
        if m.start() > last {
            out.push_str(&format_plain(&text[last..m.start()]));
        }
        let inner = &m.as_str()[1..m.as_str().len() - 1];
        out.push('`');
        out.push_str(&slack_escape(inner));
        out.push('`');
        last = m.end();
    }
    if last < text.len() {
        out.push_str(&format_plain(&text[last..]));
    }
    out
}

fn code_pieces(text: &str, max: usize) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    let flush = |pieces: &mut Vec<String>, lines: &mut Vec<String>| {
        if !lines.is_empty() {
            pieces.push(format!("```\n{}\n```", lines.join("\n")));
            lines.clear();
        }
    };
    let safe = slack_escape(text).replace("```", "`\u{200b}``");
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
        if candidate.chars().count() + 10 > max {
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

pub fn slack_blocks(markdown: &str, max: usize) -> Vec<String> {
    static FENCE_RE: OnceLock<Regex> = OnceLock::new();
    static TABLE_RE: OnceLock<Regex> = OnceLock::new();
    static TABLE_SEP_RE: OnceLock<Regex> = OnceLock::new();
    static HEADING_RE: OnceLock<Regex> = OnceLock::new();
    static BULLET_RE: OnceLock<Regex> = OnceLock::new();

    let fence_re = FENCE_RE.get_or_init(|| Regex::new(r"^\s*(```+|~~~+)\s*([^\s`]*)\s*$").unwrap());
    let table_re = TABLE_RE.get_or_init(|| Regex::new(r"^\s*\|.*\|\s*$").unwrap());
    let table_sep_re = TABLE_SEP_RE.get_or_init(|| Regex::new(r"^\s*\|[\s:|-]+\|\s*$").unwrap());
    let heading_re = HEADING_RE.get_or_init(|| Regex::new(r"^\s*#{1,6}\s+(.*?)\s*#*\s*$").unwrap());
    let bullet_re = BULLET_RE.get_or_init(|| Regex::new(r"^(\s*)[-*+]\s+(.*)$").unwrap());

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
            let mut body = Vec::new();
            let mut end = at + 1;
            while end < lines.len() && !lines[end].trim().starts_with(marker) {
                body.push(lines[end]);
                end += 1;
            }
            at = end + 1;
            blocks.extend(code_pieces(&body.join("\n"), max));
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
            blocks.extend(code_pieces(&table.join("\n"), max));
            continue;
        }
        if line.trim_start().starts_with('>') {
            end_paragraph(&mut blocks, &mut paragraph);
            let mut quoted = Vec::new();
            while at < lines.len() && lines[at].trim_start().starts_with('>') {
                let stripped = lines[at].trim_start().strip_prefix('>').unwrap_or("");
                let stripped = stripped.strip_prefix(' ').unwrap_or(stripped);
                quoted.push(format!("> {}", inline(stripped)));
                at += 1;
            }
            blocks.push(quoted.join("\n"));
            continue;
        }
        if line.trim().is_empty() || is_spaced_rule(line) {
            end_paragraph(&mut blocks, &mut paragraph);
            at += 1;
            continue;
        }
        if let Some(caps) = heading_re.captures(line) {
            end_paragraph(&mut blocks, &mut paragraph);
            blocks.push(format!("*{}*", inline(&caps[1]).replace('*', "")));
            at += 1;
            continue;
        }
        if let Some(caps) = bullet_re.captures(line) {
            let indent = (caps[1].chars().count() / 2).min(3);
            paragraph.push(format!("{}• {}", "    ".repeat(indent), inline(&caps[2])));
        } else {
            paragraph.push(inline(line));
        }
        at += 1;
    }
    end_paragraph(&mut blocks, &mut paragraph);
    blocks
}

pub fn slack_packed(blocks: &[String], max: usize) -> Vec<String> {
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

pub fn slack_messages(name: &str, answer: &str, max: usize) -> Vec<String> {
    let _ = CHAT_REST_IN_LOCUST;
    let mut all = vec![format!("*{}*", slack_escape(name))];
    all.extend(slack_blocks(answer, max.saturating_sub(40)));
    let messages = slack_packed(&all, max);
    if messages.len() <= CHAT_MAX_PARTS {
        return messages;
    }
    let mut kept = messages[..CHAT_MAX_PARTS].to_vec();
    let last = &kept[CHAT_MAX_PARTS - 1];
    kept[CHAT_MAX_PARTS - 1] = if last.chars().count() + 2 + SLACK_REST_IN_LOCUST.chars().count() <= max {
        format!("{last}\n\n{SLACK_REST_IN_LOCUST}")
    } else {
        SLACK_REST_IN_LOCUST.to_owned()
    };
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fences(message: &str) -> usize {
        message.split('\n').filter(|line| line.starts_with("```")).count()
    }

    #[test]
    fn turns_markdown_into_slacks_own_markup() {
        let plan = &slack_messages("Ada", "# Plan\n\nFirst **read** the `config.ts` file, then *run* the tests.\n\n- one\n- two\n  - nested\n\n1. first", SLACK_MESSAGE_MAX)[0];
        assert_eq!(plan, "*Ada*\n\n*Plan*\n\nFirst *read* the `config.ts` file, then _run_ the tests.\n\n• one\n• two\n    • nested\n\n1. first");

        let link = &slack_messages("Ada", "See [the docs](https://example.com/a?b=1&c=2) and ~~old~~ news.", SLACK_MESSAGE_MAX)[0];
        assert!(link.contains("See <https://example.com/a?b=1&amp;c=2|the docs> and ~old~ news."));

        let quote = &slack_messages("Ada", "> A quote with **bold** & <angles>\n\nAfter it.", SLACK_MESSAGE_MAX)[0];
        assert!(quote.contains("> A quote with *bold* &amp; &lt;angles&gt;\n\nAfter it."));

        let table = &slack_messages("Ada", "| File | Lines |\n|---|---|\n| a & b.ts | 12 |", SLACK_MESSAGE_MAX)[0];
        assert!(table.contains("```\n| File | Lines |\n| a &amp; b.ts | 12 |\n```"));

        let code = &slack_messages("Ada", "```ts\nif (a < b && c) {}\n```", SLACK_MESSAGE_MAX)[0];
        assert!(code.contains("```\nif (a &lt; b &amp;&amp; c) {}\n```"));

        let plain = &slack_messages("Ada", "Snake_case_names and 2 * 3 * 4.", SLACK_MESSAGE_MAX)[0];
        assert!(plain.contains("Snake_case_names and 2 * 3 * 4."));
    }

    #[test]
    fn splits_a_long_answer_into_messages_slack_takes_and_says_where_the_rest_is() {
        let paragraphs: String = (0..60)
            .map(|i| format!("Paragraph {i}: {}", "word ".repeat(100)))
            .collect::<Vec<_>>()
            .join("\n\n");
        let messages = slack_messages("Ada", &paragraphs, SLACK_MESSAGE_MAX);
        assert_eq!(messages.len(), CHAT_MAX_PARTS);
        assert!(messages.last().unwrap().ends_with(SLACK_REST_IN_LOCUST));
        for message in messages {
            assert!(message.chars().count() <= SLACK_MESSAGE_MAX);
        }

        let code_body = (0..400)
            .map(|i| format!("line {i} {}", "x".repeat(30)))
            .collect::<Vec<_>>()
            .join("\n");
        let code = slack_blocks(&format!("```\n{code_body}\n```"), SLACK_MESSAGE_MAX);
        assert!(code.len() > 1);
        for block in code {
            assert_eq!(fences(&block), 2);
            assert!(block.chars().count() <= SLACK_MESSAGE_MAX);
        }
    }

    #[test]
    fn reads_a_persons_words_back_from_slacks_markup() {
        assert_eq!(
            slack_plain("see <https://example.com/a?b=1&amp;c=2|the docs> &amp; <https://example.com>"),
            "see the docs (https://example.com/a?b=1&c=2) & https://example.com"
        );
        assert_eq!(
            slack_plain("ask <@U0SAM00001> in <#C0GENERAL1|general>, <!here> 1 &lt; 2"),
            "ask @someone in #general, @here 1 < 2"
        );
        assert_eq!(slack_escape("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }

    #[test]
    fn knows_each_token_by_its_shape() {
        let bot = ["xoxb", "1000000000001", "2000000000002", &"fake".repeat(6)].join("-");
        let app = ["xapp", "1", "A0LOCUST01", "3000000000003", &"fake".repeat(5)].join("-");
        let user = ["xoxp", "1000000000001", "2000000000002", &"fake".repeat(6)].join("-");
        assert!(is_slack_bot_token(&bot));
        assert!(is_slack_app_token(&app));
        assert!(!is_slack_bot_token(&app));
        assert!(!is_slack_app_token(&user));
    }

    #[test]
    fn asks_slack_for_an_app_that_reads_only_its_direct_messages_over_socket_mode() {
        let m = slack_manifest();
        assert_eq!(m["settings"]["socket_mode_enabled"], true);
        assert_eq!(m["settings"]["event_subscriptions"]["bot_events"], json!(["message.im"]));
        let mut scopes: Vec<&str> = m["oauth_config"]["scopes"]["bot"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        scopes.sort();
        assert_eq!(scopes, ["chat:write", "im:history", "reactions:write", "users:read"]);
        assert_eq!(m["features"]["app_home"], json!({"home_tab_enabled": false, "messages_tab_enabled": true, "messages_tab_read_only_enabled": false}));
        let m_str = m.to_string();
        assert!(!m_str.contains("request_url"));
        assert!(!m_str.contains("http://") && !m_str.contains("https://"));
        assert!(m["display_information"]["name"].as_str().unwrap().len() <= 35);
        assert!(m["display_information"]["description"].as_str().unwrap().len() <= 140);
        let link = url::Url::parse(&slack_manifest_link()).unwrap();
        assert_eq!(format!("{}://{}{}", link.scheme(), link.host_str().unwrap(), link.path()), "https://api.slack.com/apps");
        let params: std::collections::HashMap<_, _> = link.query_pairs().into_owned().collect();
        assert_eq!(params.get("new_app").map(String::as_str), Some("1"));
        let parsed_manifest: Value = serde_json::from_str(params.get("manifest_json").unwrap()).unwrap();
        assert_eq!(parsed_manifest, m);
    }
}

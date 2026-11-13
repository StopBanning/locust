//! Feedback and private report builders (`main/report-problem.ts`, `shared/report-problem.ts`,
//! `shared/support.ts`, `renderer/src/feedback.ts`, `main/diagnostics.ts`).
//!
//! Never sends anything without the person's press. Scrubs secrets before any report leaves.

use url::Url;

pub const SUPPORT_ADDRESS: &str = "support@locust.lol"; // public-guard: allow
pub const REPORT_DESTINATION: &str = "https://github.com/StopBanning/locust/issues/new"; // public-guard: allow

pub const MAX_REPORT_URL: usize = 7_500;
pub const MAX_MAILTO_URL: usize = 1_900;
pub const MAX_DESCRIPTION: usize = 2_000;
pub const MAX_LOG_BYTES: u64 = 512 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportFacts {
    pub version: String,
    pub release: String,
    pub arch: String,
    pub platform: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FeedbackReport {
    pub description: String,
    pub conversation: Option<String>,
}

pub fn system_name(facts: &ReportFacts) -> String {
    match facts.platform.as_deref() {
        Some("darwin") => format!("macOS (Darwin {}, {})", facts.release.trim(), facts.arch),
        Some(p) if p != "win32" => format!("{p} {} ({})", facts.release.trim(), facts.arch),
        _ => windows_name(&facts.release, &facts.arch),
    }
}

pub fn windows_name(release: &str, arch: &str) -> String {
    let trimmed = release.trim();
    if let Some(rest) = trimmed.strip_prefix("10.0.") {
        let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(build) = digits.parse::<u64>() {
            let label = if build >= 22_000 {
                "Windows 11"
            } else {
                "Windows 10"
            };
            return format!("{label}, build {build} ({arch})");
        }
    }
    format!("Windows {trimmed} ({arch})")
}

pub fn platform_display_name(platform: Option<&str>) -> &'static str {
    match platform {
        Some("darwin") => "macOS",
        Some("linux") => "Linux",
        _ => "Windows",
    }
}

/// Exact claim text shown in FeedbackDialog before the person presses Send.
pub fn report_claim(include_conversation: bool, has_conversation: bool, platform: Option<&str>) -> String {
    let conv = if include_conversation && has_conversation {
        ", this conversation,"
    } else {
        ""
    };
    let os = platform_display_name(platform);
    format!(
        "This report will include your description{conv} and your Locust and {os} versions. It opens on GitHub as a public issue that anyone can read, and you send it from there."
    )
}

fn scrub_trimmed(text: &str, max_chars: Option<usize>) -> String {
    let (scrubbed, _) = crate::secrets::scrub_secrets(text);
    let trimmed = scrubbed.trim();
    match max_chars {
        Some(limit) => trimmed.chars().take(limit).collect(),
        None => trimmed.to_owned(),
    }
}

fn body(facts: &ReportFacts, description: &str, conversation: Option<&str>, trimmed: bool) -> String {
    let mut lines = vec![
        description.to_owned(),
        String::new(),
        "---".into(),
        format!("Locust {} on {}", facts.version, system_name(facts)),
    ];
    if let Some(conv) = conversation.filter(|c| !c.is_empty()) {
        let suffix = if trimmed { " (its latest part)" } else { "" };
        lines.push(String::new());
        lines.push(format!(
            "<details><summary>The conversation it was sent from{suffix}</summary>"
        ));
        lines.push(String::new());
        lines.push("````text".into());
        lines.push(conv.to_owned());
        lines.push("````".into());
        lines.push(String::new());
        lines.push("</details>".into());
    }
    lines.push(String::new());
    lines.push(
        "Nothing else was attached. To add the log: Settings > Report a problem > Show the log, then drag the file into this box."
            .into(),
    );
    lines.join("\n")
}

fn url_with(text: &str) -> String {
    let mut url = Url::parse(REPORT_DESTINATION).expect("valid REPORT_DESTINATION");
    url.query_pairs_mut().append_pair("body", text);
    url.to_string()
}

pub fn report_file_text(facts: &ReportFacts, report: &FeedbackReport) -> String {
    let description = scrub_trimmed(&report.description, Some(MAX_DESCRIPTION));
    let conversation = report
        .conversation
        .as_deref()
        .map(|c| scrub_trimmed(c, None))
        .filter(|c| !c.is_empty());
    [
        format!(
            "A report for Locust's makers. Send it to {SUPPORT_ADDRESS}, or open it as an issue at {REPORT_DESTINATION} (public)."
        ),
        String::new(),
        body(facts, &description, conversation.as_deref(), false),
    ]
    .join("\n")
}

pub fn report_subject(facts: &ReportFacts, description: &str) -> String {
    let scrubbed = scrub_trimmed(description, None);
    let words: String = scrubbed.split_whitespace().collect::<Vec<_>>().join(" ");
    let char_count = words.chars().count();
    let first = if char_count > 60 {
        let prefix: String = words.chars().take(57).collect();
        format!("{}...", prefix.trim_end())
    } else if words.is_empty() {
        "a problem".into()
    } else {
        words
    };
    format!("Locust {}: {first}", facts.version)
}

fn encode_uri_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for b in input.as_bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(*b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn mailto_with(subject: &str, text: &str) -> String {
    format!(
        "mailto:{SUPPORT_ADDRESS}?subject={}&body={}",
        encode_uri_component(subject),
        encode_uri_component(text)
    )
}

fn tail_chars(text: &str, keep: usize) -> String {
    let total = text.chars().count();
    if keep >= total {
        return text.to_owned();
    }
    text.chars().skip(total - keep).collect()
}

pub fn support_mailto_url(facts: &ReportFacts, report: &FeedbackReport) -> String {
    let description = scrub_trimmed(&report.description, Some(MAX_DESCRIPTION));
    let subject = report_subject(facts, &description);
    let conversation = report
        .conversation
        .as_deref()
        .map(|c| scrub_trimmed(c, None))
        .filter(|c| !c.is_empty());
    let whole = mailto_with(&subject, &body(facts, &description, conversation.as_deref(), false));
    let Some(conv) = conversation else {
        return whole;
    };
    if whole.len() <= MAX_MAILTO_URL {
        return whole;
    }
    let mut keep = conv.chars().count();
    while keep > 40 {
        keep = (keep * 4) / 5;
        let snippet = format!("...{}", tail_chars(&conv, keep));
        let url = mailto_with(&subject, &body(facts, &description, Some(&snippet), true));
        if url.len() <= MAX_MAILTO_URL {
            return url;
        }
    }
    let fallback_body = [
        body(facts, &description, None, false),
        String::new(),
        "The conversation did not fit in an email; \"Save this report as a file\" in Locust carries it whole."
            .into(),
    ]
    .join("\n");
    mailto_with(&subject, &fallback_body)
}

pub fn feedback_url(facts: &ReportFacts, report: &FeedbackReport) -> String {
    let description = scrub_trimmed(&report.description, Some(MAX_DESCRIPTION));
    let conversation = report
        .conversation
        .as_deref()
        .map(|c| scrub_trimmed(c, None))
        .filter(|c| !c.is_empty());
    let whole = url_with(&body(facts, &description, conversation.as_deref(), false));
    let Some(conv) = conversation else {
        return whole;
    };
    if whole.len() <= MAX_REPORT_URL {
        return whole;
    }
    let mut keep = conv.chars().count();
    while keep > 0 {
        keep = (keep * 4) / 5;
        let snippet = format!("...{}", tail_chars(&conv, keep));
        let url = url_with(&body(facts, &description, Some(&snippet), true));
        if url.len() <= MAX_REPORT_URL {
            return url;
        }
    }
    url_with(&body(facts, &description, None, false))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportTurn {
    pub prompt: String,
    pub reply: String,
    pub failed_message: Option<String>,
}

/// Format turns as plain text for a report (`renderer/src/feedback.ts` `conversationText`).
pub fn conversation_text(turns: &[ReportTurn], agent_label: &str) -> String {
    let mut blocks = Vec::new();
    for turn in turns {
        let mut lines = vec![format!("You: {}", turn.prompt.trim())];
        let reply = turn.reply.trim();
        let said = if reply.is_empty() {
            "(nothing said)"
        } else {
            reply
        };
        lines.push(format!("{agent_label}: {said}"));
        if let Some(err) = &turn.failed_message {
            let msg = err.trim();
            if msg.is_empty() {
                lines.push("(the turn failed)".into());
            } else {
                lines.push(format!("(the turn failed: {msg})"));
            }
        }
        blocks.push(lines.join("\n\n"));
    }
    blocks.join("\n\n")
}

/// Diagnostic log helpers (`main/diagnostics.ts`).
pub fn should_roll(bytes: Option<u64>) -> bool {
    bytes.is_some_and(|b| b >= MAX_LOG_BYTES)
}

pub fn diagnostic_line(iso_timestamp: &str, label: &str, detail: &str) -> String {
    let trimmed = detail.trim();
    let d = if trimmed.is_empty() {
        "(no detail)"
    } else {
        trimmed
    };
    format!("{iso_timestamp} {label}: {d}\n")
}

pub fn startup_detail(version: &str, platform: &str, release: &str, packaged: bool) -> String {
    let mode = if packaged { "packaged" } else { "development" };
    format!("Locust {version} ({platform} {release}, {mode})")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win_facts() -> ReportFacts {
        ReportFacts {
            version: "0.306.0".into(),
            release: "10.0.19045".into(),
            arch: "x64".into(),
            platform: None,
        }
    }

    #[test]
    fn opens_single_destination_and_carries_words_facts_and_conversation() {
        let built = feedback_url(
            &win_facts(),
            &FeedbackReport {
                description: "The plan card stayed empty.".into(),
                conversation: Some(
                    "You: list the files\n\nWren (Codex): data.csv has the most lines.".into(),
                ),
            },
        );
        let parsed = Url::parse(&built).unwrap();
        assert_eq!(
            format!("{}{}", parsed.origin().ascii_serialization(), parsed.path()),
            REPORT_DESTINATION
        );
        assert_eq!(REPORT_DESTINATION, "https://github.com/StopBanning/locust/issues/new"); // public-guard: allow
        let keys: Vec<_> = parsed.query_pairs().map(|(k, _)| k.into_owned()).collect();
        assert_eq!(keys, vec!["body"]);
        let body = parsed
            .query_pairs()
            .find(|(k, _)| k == "body")
            .unwrap()
            .1
            .into_owned();
        assert!(body.starts_with("The plan card stayed empty."));
        assert!(body.contains("Locust 0.306.0 on Windows 10, build 19045 (x64)"));
        assert!(body.contains("Wren (Codex): data.csv has the most lines."));
        assert!(body.contains("Nothing else was attached."));

        let plain_url = feedback_url(
            &win_facts(),
            &FeedbackReport {
                description: "Settings would not open.".into(),
                conversation: None,
            },
        );
        let plain = Url::parse(&plain_url)
            .unwrap()
            .query_pairs()
            .next()
            .unwrap()
            .1
            .into_owned();
        assert!(!plain.contains("conversation it was sent from"));
    }

    #[test]
    fn keeps_end_of_long_conversation_and_bounds_url_length() {
        let long: String = (0..400)
            .map(|i| format!("line {i}: {}", "é".repeat(20)))
            .collect::<Vec<_>>()
            .join("\n");
        let cut = feedback_url(
            &win_facts(),
            &FeedbackReport {
                description: "It froze.".into(),
                conversation: Some(long),
            },
        );
        assert!(cut.len() <= MAX_REPORT_URL);
        let text = Url::parse(&cut)
            .unwrap()
            .query_pairs()
            .next()
            .unwrap()
            .1
            .into_owned();
        assert!(text.contains("line 399:"));
        assert!(!text.contains("line 0:"));
        assert!(text.contains("(its latest part)"));
        assert!(text.starts_with("It froze."));
    }

    #[test]
    fn names_windows_11_and_macos_correctly() {
        assert_eq!(
            windows_name("10.0.22631", "x64"),
            "Windows 11, build 22631 (x64)"
        );
        assert_eq!(
            windows_name("10.0.19045", "arm64"),
            "Windows 10, build 19045 (arm64)"
        );
        assert_eq!(
            windows_name("6.1.7601", "x64"),
            "Windows 6.1.7601 (x64)"
        );
        assert_eq!(
            system_name(&ReportFacts {
                version: "0.593.0".into(),
                release: "24.1.0".into(),
                arch: "arm64".into(),
                platform: Some("darwin".into()),
            }),
            "macOS (Darwin 24.1.0, arm64)"
        );
    }

    #[test]
    fn support_email_and_file_work_and_keep_uncut_conversation_in_file() {
        let facts = ReportFacts {
            version: "0.593.0".into(),
            release: "10.0.26100".into(),
            arch: "x64".into(),
            platform: Some("win32".into()),
        };
        assert_eq!(SUPPORT_ADDRESS, "support@locust.lol"); // public-guard: allow
        let file_small = report_file_text(
            &facts,
            &FeedbackReport {
                description: "It froze.".into(),
                conversation: None,
            },
        );
        assert!(file_small.contains(&format!("Send it to {SUPPORT_ADDRESS}")));
        assert!(file_small.contains(REPORT_DESTINATION));

        let mail = support_mailto_url(
            &facts,
            &FeedbackReport {
                description: "The thread stopped drawing after a long run.".into(),
                conversation: Some("Wren: starting\nWren: done".into()),
            },
        );
        assert!(mail.starts_with(&format!("mailto:{SUPPORT_ADDRESS}?subject=")));
        assert!(mail.len() <= MAX_MAILTO_URL);

        let long: String = (0..400)
            .map(|i| format!("line {i}: the teammate said something here"))
            .collect::<Vec<_>>()
            .join("\n");
        let cut_mail = support_mailto_url(
            &facts,
            &FeedbackReport {
                description: "Slow.".into(),
                conversation: Some(long.clone()),
            },
        );
        assert!(cut_mail.len() <= MAX_MAILTO_URL);

        let full_desc = "The composer stopped taking keys after the third run. ".repeat(34);
        let no_conv_mail = support_mailto_url(
            &facts,
            &FeedbackReport {
                description: full_desc,
                conversation: Some(long.clone()),
            },
        );
        assert!(no_conv_mail.contains("did%20not%20fit%20in%20an%20email"));

        assert_eq!(
            report_subject(&facts, &"A".repeat(100)),
            format!("Locust 0.593.0: {}...", "A".repeat(57))
        );
        assert_eq!(report_subject(&facts, "   "), "Locust 0.593.0: a problem");

        let file_long = report_file_text(
            &facts,
            &FeedbackReport {
                description: "Slow.".into(),
                conversation: Some(long),
            },
        );
        assert!(file_long.contains("line 0"));
        assert!(file_long.contains("line 399"));
        assert!(!file_long.contains("(its latest part)"));
        assert!(file_long.contains("To add the log:"));
    }

    #[test]
    fn report_scrubs_secrets_before_leaving() {
        let fake_key = ["sk", "ant", "api03-abcdefghijklmnopqrstuvwxyz0123456789"].join("-");
        let report = FeedbackReport {
            description: format!("Failed with key {fake_key}"),
            conversation: Some(format!("You: use {fake_key}")),
        };
        let file = report_file_text(&win_facts(), &report);
        assert!(!file.contains(&fake_key));
        assert!(file.contains(crate::secrets::SECRET_SHAPED_TEXT_REMOVED));

        let url = feedback_url(&win_facts(), &report);
        assert!(!url.contains(&fake_key));

        let mail = support_mailto_url(&win_facts(), &report);
        assert!(!mail.contains(&fake_key));
    }

    #[test]
    fn diagnostic_log_and_conversation_text_helpers_match_electron() {
        assert!(!should_roll(None));
        assert!(!should_roll(Some(10)));
        assert!(should_roll(Some(MAX_LOG_BYTES)));
        assert!(should_roll(Some(MAX_LOG_BYTES + 1)));

        assert_eq!(
            diagnostic_line("2026-09-15T10:00:00.000Z", "start", "Locust 0.138.0"),
            "2026-09-15T10:00:00.000Z start: Locust 0.138.0\n"
        );
        assert!(diagnostic_line("2026-09-15T10:00:00.000Z", "x", "   ").contains("(no detail)"));
        assert!(startup_detail("0.138.0", "win32", "10.0.19045", true).contains("packaged"));
        assert!(startup_detail("0", "x", "y", false).contains("development"));

        let text = conversation_text(
            &[
                ReportTurn {
                    prompt: "List the files".into(),
                    reply: "Three files.".into(),
                    failed_message: None,
                },
                ReportTurn {
                    prompt: "Count the lines".into(),
                    reply: String::new(),
                    failed_message: Some("The runtime did not answer in time.".into()),
                },
            ],
            "Wren (Codex CLI, GPT-5.6 Luna)",
        );
        assert!(text.contains("You: List the files"));
        assert!(text.contains("Wren (Codex CLI, GPT-5.6 Luna): Three files."));
        assert!(text.contains("You: Count the lines"));
        assert!(text.contains("Wren (Codex CLI, GPT-5.6 Luna): (nothing said)"));
        assert!(text.contains("(the turn failed: The runtime did not answer in time.)"));
    }
}

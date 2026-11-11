//! Secret detection and display redaction, matching Electron `shared/secrets.ts`.
//! One module owns the shapes used by memories, About you, rules and display.

use crate::invariants;
use regex::Regex;
use std::sync::OnceLock;

struct Shape {
    kind: &'static str,
    pattern: &'static str,
}

const SHAPES: &[Shape] = &[
    Shape {
        kind: "an Anthropic API key",
        pattern: r"(?-u:\b)sk-ant-[A-Za-z0-9_-]{20,}",
    },
    Shape {
        kind: "an OpenAI API key",
        pattern: r"(?-u:\b)sk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{20,}",
    },
    Shape {
        kind: "an AWS access key",
        pattern: r"(?-u:\b)(?:AKIA|ASIA)[0-9A-Z]{16}(?-u:\b)",
    },
    Shape {
        kind: "a Google API key",
        pattern: r"(?-u:\b)AIza[0-9A-Za-z_-]{35}(?-u:\b)",
    },
    Shape {
        kind: "a GitHub token",
        pattern: r"(?-u:\b)(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{22,})",
    },
    Shape {
        kind: "a GitLab token",
        pattern: r"(?-u:\b)glpat-[A-Za-z0-9_-]{20,}",
    },
    Shape {
        kind: "a Slack token",
        pattern: r"(?-u:\b)xox[abprs]-[A-Za-z0-9-]{10,}",
    },
    Shape {
        kind: "a Slack app token",
        pattern: r"(?-u:\b)xapp-\d-[A-Za-z0-9-]{10,}",
    },
    Shape {
        kind: "a Discord bot token",
        pattern: r"(?-u:\b)[MNO][A-Za-z0-9_-]{23,27}\.[A-Za-z0-9_-]{6}\.[A-Za-z0-9_-]{27,40}(?-u:\b)",
    },
    Shape {
        kind: "a Telegram bot token",
        pattern: r"(?-u:\b)\d{5,15}:AA[A-Za-z0-9_-]{30,40}(?-u:\b)",
    },
    Shape {
        kind: "an npm token",
        pattern: r"(?-u:\b)npm_[A-Za-z0-9]{36}(?-u:\b)",
    },
    Shape {
        kind: "a signed token (JWT)",
        pattern: r"(?-u:\b)eyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}",
    },
    Shape {
        kind: "a private key",
        pattern: r"-----BEGIN [A-Z ]*PRIVATE KEY-----",
    },
    Shape {
        kind: "a bearer token",
        pattern: r"(?-u:\b)Bearer\s+[A-Za-z0-9._~+/-]{20,}",
    },
    Shape {
        kind: "a password or key",
        pattern: r#"(?i)(?-u:\b)(?:password|passwd|pwd|secret|api[_-]?key|access[_-]?token|auth[_-]?token)\s*[:=]\s*['"]?[^\s"'<>]{8,}"#,
    },
    Shape {
        kind: "a login inside an address",
        pattern: r"(?i)(?-u:\b)[a-z][a-z0-9+.-]*://[^\s:/@]+:[^\s@/]+@",
    },
    Shape {
        kind: "a passphrase",
        pattern: r"(?i)(?-u:\b)passphrase(?-u:\b)[^.\n]{0,30}?(?-u:\b)(?:is|was)\s*[:=]?\s*\S{3,}|(?-u:\b)passphrase\s*[:=]\s*\S{3,}",
    },
];

fn shape_res() -> &'static [Regex] {
    static RES: OnceLock<Vec<Regex>> = OnceLock::new();
    RES.get_or_init(|| {
        SHAPES
            .iter()
            .map(|s| invariants::regex(s.pattern))
            .collect()
    })
}

/// AWS secret access key: 40 base64 chars near the word that names it (no lookaround).
fn aws_secret_key_in(text: &str) -> bool {
    static NEAR: OnceLock<Regex> = OnceLock::new();
    let near = NEAR.get_or_init(|| {
        invariants::regex(r"(?i)(?-u:\b)(?:secret|aws)(?-u:\b)[^\n]{0,60}?([A-Za-z0-9/+]{40})")
    });
    for caps in near.captures_iter(text) {
        let Some(m) = caps.get(1) else { continue };
        let s = m.as_str();
        let has_digit = s
            .chars()
            .any(|c| c.is_ascii_digit() || c == '/' || c == '+');
        let has_lower = s.chars().any(|c| c.is_ascii_lowercase());
        let has_upper = s.chars().any(|c| c.is_ascii_uppercase());
        if has_digit && has_lower && has_upper {
            return true;
        }
    }
    false
}

fn looks_like_a_password(value: &str) -> bool {
    value.len() >= 6
        && value
            .chars()
            .any(|c| "0123456789!@#$%^&*_+=?~-".contains(c))
        && !Regex::new(r"^\.[A-Za-z]+$")
            .expect("literal")
            .is_match(value)
}

fn password_said(text: &str) -> bool {
    static SPOKEN: OnceLock<Vec<Regex>> = OnceLock::new();
    let spoken = SPOKEN.get_or_init(|| {
        vec![
            invariants::regex(
                r#"(?i)(?-u:\b)(?:password|passwd|pwd|pin)(?-u:\b)[^.\n]{0,30}?(?-u:\b)(?:is|was)\s+['"`]?([^\s'"`]+)"#,
            ),
            invariants::regex(
                r#"(?i)(?-u:\b)(?:login|logins|credentials?|username)(?-u:\b)[^\n]{0,40}?\S+\s*[/:]\s*([^\s'"`]+)"#,
            ),
        ]
    });
    for re in spoken {
        for caps in re.captures_iter(text) {
            let bare = caps
                .get(1)
                .map(|m| m.as_str().trim_end_matches(|c| ".,;)".contains(c)))
                .unwrap_or("");
            if looks_like_a_password(bare) {
                return true;
            }
        }
    }
    false
}

fn looks_like_a_card(digits: &str) -> bool {
    let mut sum = 0i32;
    for (at, ch) in digits.chars().rev().enumerate() {
        let Some(mut digit) = ch.to_digit(10).map(|d| d as i32) else {
            return false;
        };
        if at % 2 == 1 {
            digit = if digit * 2 > 9 {
                digit * 2 - 9
            } else {
                digit * 2
            };
        }
        sum += digit;
    }
    let network = Regex::new(r"^(?:4|5[1-5]|2[2-7]|3[47]|6(?:011|5))")
        .expect("literal")
        .is_match(digits);
    let all_same = digits
        .chars()
        .all(|c| c == digits.chars().next().unwrap_or('\0'));
    sum % 10 == 0 && network && !all_same
}

fn card_number_in(text: &str) -> bool {
    static CARD_RUN: OnceLock<Regex> = OnceLock::new();
    let re = CARD_RUN
        .get_or_init(|| invariants::regex(r"(?:^|[^0-9])([0-9](?:[ -]?[0-9]){12,18})(?:[^0-9]|$)"));
    for caps in re.captures_iter(text) {
        let Some(m) = caps.get(1) else { continue };
        let digits: String = m.as_str().chars().filter(|c| c.is_ascii_digit()).collect();
        if looks_like_a_card(&digits) {
            return true;
        }
    }
    false
}

/// What kind of secret the text holds, if it holds one. Electron `secretIn`.
pub fn secret_in(text: &str) -> Option<&'static str> {
    for (shape, re) in SHAPES.iter().zip(shape_res().iter()) {
        if re.is_match(text) {
            return Some(shape.kind);
        }
    }
    if aws_secret_key_in(text) {
        return Some("an AWS secret key");
    }
    if password_said(text) {
        return Some("a password");
    }
    if card_number_in(text) {
        return Some("a card number");
    }
    None
}

pub fn about_you_secret_refusal(kind: &str, from: Option<&str>) -> String {
    match from {
        None => format!(
            "That holds {kind}, and About you is read by every teammate on every provider, so it was not saved. Keep keys in the runtime's own sign-in or an environment variable."
        ),
        Some(who) => format!(
            "A line {who} suggested for About you held {kind}, so it was not put to you: About you is read by every teammate on every provider."
        ),
    }
}

pub fn secret_refusal(kind: &str) -> String {
    format!(
        "It held {kind}, and a memory is given to every teammate on every provider, so it was not kept. Keep keys in the runtime's own sign-in or an environment variable."
    )
}

pub fn instructions_secret_refusal(name: &str, kind: &str) -> String {
    format!(
        "They hold {kind}, and {name}\u{2019}s model is given them on every turn, so they were not saved. Take it out first."
    )
}

pub fn rule_secret_refusal(kind: &str) -> String {
    format!(
        "That command holds {kind}, so it isn't saved as a rule: a rule keeps the command as written. Allow it once instead."
    )
}

/// Sensitive JSON key names (codex-events `SENSITIVE_KEY`), kept beside the shared shapes.
pub fn sensitive_key(key: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        invariants::regex(
            r"(?i)^(?:api[_-]?key|access[_-]?token|refresh[_-]?token|auth(?:orization)?|password|passwd|secret|cookie|set-cookie|credential|session[_-]?token)$",
        )
    })
    .is_match(key)
}

/// Evidence/display scrub used by ledger and runtime events (`[redacted]`),
/// using adapter-style shapes and key=value redaction.
pub fn redact_secrets(text: &str) -> String {
    static RULES: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
    let rules = RULES.get_or_init(|| {
        [
            (
                r"(?i)(?-u:\b)Bearer\s+[A-Za-z0-9._~+/=-]{8,}",
                "Bearer [redacted]",
            ),
            (r"(?-u:\b)(?:sk|pk)-[A-Za-z0-9_-]{12,}(?-u:\b)", "[redacted]"),
            (
                r"(?-u:\b)(?:gh[opusr]_[A-Za-z0-9]{20,}|xox[baprs]-[A-Za-z0-9-]{10,}|AKIA[A-Z0-9]{16})(?-u:\b)",
                "[redacted]",
            ),
            (
                r"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?-----END [A-Z0-9 ]*PRIVATE KEY-----",
                "[redacted]",
            ),
            (
                r"(?-u:\b)eyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}",
                "[redacted]",
            ),
            (r"(?-u:\b)AIza[0-9A-Za-z_-]{35}(?-u:\b)", "[redacted]"),
            (r"(?-u:\b)hf_[A-Za-z0-9]{30,}(?-u:\b)", "[redacted]"),
            (
                r#"(?i)((?:api[_-]?key|access[_-]?token|refresh[_-]?token|id[_-]?token|client[_-]?secret|authorization|password|secret|cookie|credential)["']?\s*[=:]\s*["']?)([^\s,"';}]+)"#,
                "${1}[redacted]",
            ),
            (r"(?-u:\b)sk-ant-[A-Za-z0-9_-]{12,}", "[redacted]"),
        ]
        .into_iter()
        .map(|(p, r)| (invariants::regex(p), r))
        .collect()
    });
    let mut safe = text.replace('\0', "");
    for (re, replacement) in rules {
        safe = re.replace_all(&safe, *replacement).into_owned();
    }
    safe
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_shapes_match_electron_hits_and_misses() {
        let hits = [
            format!("The deploy uses {}-ant-api03-{} for staging.", "sk", "a1B2c3D4".repeat(4)),
            format!("{}-proj-{}", "sk", "Zx9Yw8Vu".repeat(4)),
            format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE"),
            format!("AIza{}", "SyA1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6q".chars().take(35).collect::<String>()),
            format!("ghp_{}", "R2d2C3po".repeat(5)),
            "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U".into(),
            format!("Bearer {}", "q7W8e9R0t1Y2u3I4o5P6".repeat(2)),
            "password=hunter2hunter2".into(),
        ];
        for note in hits {
            assert!(secret_in(&note).is_some(), "{note}");
        }
        for note in [
            "Keep the API key in .env, never in the repo.",
            "The password is set in the admin panel, not here.",
            "Tokens expire after an hour; refresh with pnpm auth.",
            "api_key: <your key here>",
            "Commit 8ca60791 fixed the Bearer header parsing.",
        ] {
            assert_eq!(secret_in(note), None, "{note}");
        }
        // A bare key prefix with nothing after it is a word, not a key (built at runtime for the public-repo guard).
        let bare_prefix = format!("Use {}-ant keys only through the Claude sign-in.", "sk");
        assert_eq!(secret_in(&bare_prefix), None, "{bare_prefix}");
        assert_eq!(
            secret_in(&format!(
                "postgres://admin:hunter2{}db.internal:5432/app is the dev database",
                "@"
            )),
            Some("a login inside an address")
        );
        assert_eq!(
            secret_in("The admin login is admin / P@ssw0rd-2026!"),
            Some("a password")
        );
        assert_eq!(
            secret_in("The person's card number is 4111 1111 1111 1111 exp 12/29"),
            Some("a card number")
        );
        assert_eq!(secret_in("The password is stored in the vault."), None);
        assert_eq!(
            secret_in("Order 1234 5678 9012 3456 shipped on Tuesday."),
            None
        );
    }

    #[test]
    fn refusal_words_match_electron() {
        assert_eq!(
            about_you_secret_refusal("an Anthropic API key", None),
            "That holds an Anthropic API key, and About you is read by every teammate on every provider, so it was not saved. Keep keys in the runtime's own sign-in or an environment variable."
        );
        assert_eq!(
            secret_refusal("an OpenAI API key"),
            "It held an OpenAI API key, and a memory is given to every teammate on every provider, so it was not kept. Keep keys in the runtime's own sign-in or an environment variable."
        );
        assert_eq!(
            rule_secret_refusal("a GitHub token"),
            "That command holds a GitHub token, so it isn't saved as a rule: a rule keeps the command as written. Allow it once instead."
        );
    }
}

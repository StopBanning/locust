//! Electron browser action gates. Probe metadata is untrusted; unknown operations fail closed.
use serde_json::Value;
use url::Url;
fn matches(pattern: &str, s: &str) -> bool {
    regex::Regex::new(pattern)
        .map(|r| r.is_match(s))
        .unwrap_or(true)
}
pub fn gate(
    action: &str,
    element: &Value,
    key: &str,
    current: &str,
    trusted: &std::collections::HashSet<String>,
) -> Option<(&'static str, String, String)> {
    let name = element["name"]
        .as_str()
        .filter(|s| !s.is_empty())
        .or_else(|| element["placeholder"].as_str())
        .unwrap_or("element");
    let desc = ["name", "placeholder", "type", "role", "href", "formAction"]
        .iter()
        .filter_map(|k| element[k].as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let credential = matches(
        r"(?i)\b(password|passcode|passphrase|secret|api[-_\s]?key|token|otp|2fa|two[-_\s]?factor|verification\s+code|auth\s+code|pin\b|cvv|cvc|csc|card\s*number|cc[-_\s]?number|cc[-_\s]?exp|cc[-_\s]?csc|credit\s*card|debit\s*card|routing\s*number|bank\s*account|iban|ssn|social\s*security|tax\s*id)\b",
        &desc,
    ) || element["sensitive"] == true
        || element["type"] == "password";
    let purchase = matches(
        r"(?i)\b(buy\s+now|place\s+order|complete\s+(purchase|order)|checkout|check\s+out|pay\s+now|pay\s+\$?\d|confirm\s+(purchase|payment|order)|authorize\s+payment|subscribe\s+now|purchase\s+now|one[-_\s]?click\s+buy)\b",
        &desc,
    );
    let send = matches(
        r"(?i)\b(send(\s+(message|email|mail|reply|invite|invitation|money|payment|transfer|post|dm|note))?|post(\s+(comment|reply|update|message|tweet|status|review))?|publish(\s+(post|article|page|release))?|tweet|submit\s+(comment|reply|review|post|message))\b",
        &desc,
    );
    let host = Url::parse(current)
        .ok()
        .and_then(|u| u.host_str().map(str::to_owned))
        .unwrap_or_else(|| "this page".into());
    if action == "press" && ["enter", "return"].contains(&key.trim().to_lowercase().as_str()) {
        let decision = if credential {
            Some((
                "credential",
                format!("Submit credential field \"{name}\" on {host}?"),
            ))
        } else if purchase {
            Some(("purchase", format!("Confirm \"{name}\" on {host}?")))
        } else if send {
            Some(("send", format!("Send \"{name}\" on {host}?")))
        } else if element["role"] != "searchbox"
            && element["type"] != "search"
            && !matches(r"(?i)^\s*(search|find|filter|query|lookup|go)\s*$", name)
            && element["submit"] == true
        {
            Some(("submit", format!("Submit form via \"{name}\" on {host}?")))
        } else {
            None
        };
        return decision.map(|(gate, title)| {
            (
                gate,
                title,
                format!("Teammate wants to press Enter inside \"{name}\" on {current}."),
            )
        });
    }
    let decision = if action == "download" || action == "click" && element["download"] == true {
        Some(("download", format!("Download from \"{name}\" on {host}?")))
    } else if matches!(action, "type" | "select") && credential {
        Some((
            "credential",
            format!("Enter text into \"{name}\" on {host}?"),
        ))
    } else if action == "click" && purchase {
        Some(("purchase", format!("Click \"{name}\" on {host}?")))
    } else if action == "click" && send {
        Some(("send", format!("Click \"{name}\" on {host}?")))
    } else if action == "click"
        && !(matches(r"(?i)^\s*(search|find|filter|query|lookup|go)\s*$", name)
            && !credential
            && !purchase
            && !send)
        && (element["submit"] == true
            || element["type"] == "submit"
            || matches(
                r"(?i)\b(submit|sign\s*in|log\s*in|sign\s*up|register|create\s+account|continue\s+with|authorize|allow\s+access|grant\s+access|confirm|save\s+changes|apply\s+changes|delete\s+account|transfer)\b",
                name,
            ))
    {
        Some(("submit", format!("Submit \"{name}\" on {host}?")))
    } else {
        None
    };
    if let Some((gate, title)) = decision {
        return Some((
            gate,
            title,
            format!(
                "Teammate wants to {action} \"{name}\" on {current}. Input value stays hidden."
            ),
        ));
    }
    if action == "click"
        && let Some(href) = element["href"].as_str()
        && let Ok(target) = Url::parse(current).and_then(|u| u.join(href))
    {
        let origin = super::origin(&target);
        let same = Url::parse(current).is_ok_and(|u| super::origin(&u) == origin);
        if !super::loopback(&target) && !same && !trusted.contains(&origin) {
            return Some((
                "leave-origin",
                format!("Leave {host} for {origin}?"),
                format!(
                    "Teammate wants to follow link \"{name}\" to {target}, which has not been opened by you in this tab."
                ),
            ));
        }
    }
    None
}

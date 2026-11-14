//! A provider's refusal, said as a sentence (Electron `packages/runtime-adapters/src/provider-error.ts`).
//!
//! A runtime hands over its provider's error as JSON, often JSON inside a string inside JSON. The facts that matter --
//! the message, an HTTP status, a code -- are dug out and said as one sentence naming who refused and what to do.
//! A message that is not JSON is left exactly as it is, malformed JSON included; callers keep the original as evidence.

use serde_json::Value;

struct Facts {
    message: String,
    status: Option<i64>,
    kind: Option<String>,
}

fn facts_of(value: &Value, status: Option<i64>, kind: Option<String>, depth: u32) -> Option<Facts> {
    if depth > 5 {
        return None;
    }
    if let Value::String(text) = value {
        let body = crate::invariants::regex(r"^(?:OpenCode stopped:|Antigravity could not run it:|Error:)\s*").replace(text, "");
        let parsed: Value = serde_json::from_str(&body).ok()?;
        return facts_of(&parsed, status, kind, depth + 1);
    }
    let object = value.as_object()?;
    let code = object.get("status").filter(|v| !v.is_null()).or_else(|| object.get("statusCode"));
    // A status that is not a number is not one: the one around it stands.
    let status = code.and_then(Value::as_i64).filter(|_| code.is_some_and(Value::is_number)).or(status);
    let kind = object.get("code").and_then(Value::as_str).or_else(|| object.get("type").and_then(Value::as_str)).map(str::to_owned).or(kind);
    if let Some(error) = object.get("error").filter(|e| e.is_object() || e.is_string())
        && let Some(inner) = facts_of(error, status, kind.clone(), depth + 1)
    {
        return Some(inner);
    }
    if let Some(data) = object.get("data").filter(|d| d.is_object())
        && let Some(inner) = facts_of(data, status, kind.clone(), depth + 1)
    {
        return Some(inner);
    }
    let message = object.get("message").and_then(Value::as_str).filter(|m| !m.trim().is_empty())?;
    facts_of(&Value::String(message.to_owned()), status, kind.clone(), depth + 1).or_else(|| Some(Facts { message: message.trim().to_owned(), status, kind }))
}

fn provider_of(runtime: Option<&str>) -> &'static str {
    match runtime {
        Some("codex") => "OpenAI",
        Some("claude") => "Anthropic",
        Some("cursor") => "Cursor's provider",
        Some("antigravity") => "Antigravity's provider",
        Some("copilot") => "Copilot's provider",
        Some("opencode") => "OpenCode's provider",
        _ => "The provider",
    }
}

/// A recorded provider error as a sentence; a message that is not a provider's JSON is returned unchanged.
pub fn provider_error_sentence(message: &str, runtime: Option<&str>) -> String {
    let Some(facts) = facts_of(&Value::String(message.to_owned()), None, None, 0) else {
        return message.to_owned();
    };
    let provider = provider_of(runtime);
    let said = facts.message.as_str();
    let kind = facts.kind.as_deref().unwrap_or("");
    let with_kind = format!("{kind} {said}");
    let re = crate::invariants::regex;
    let model = re(r#"(?i)\bmodel\s+['"]?([^'"\s:]+)['"]?"#)
        .captures(said)
        .map(|c| c[1].to_owned())
        .filter(|m| !re(r"(?i)^(?:not|is|was|does|unavailable)$").is_match(m));
    if (re(r"(?i)\bmodel\b").is_match(said) || kind == "model_not_found")
        && re(r"(?i)not enabled|not found|does not exist|unknown model|model_not_found").is_match(&format!("{said} {kind}"))
    {
        let reason = if re(r"(?i)not enabled").is_match(said) { "it is not enabled here right now" } else { "it could not be found or is not available here" };
        let which = model.map_or_else(|| "the model".to_owned(), |m| format!("the model {m}"));
        return format!("{provider} refused {which}: {reason}. Pick another model and send again.");
    }
    if re(r"(?i)insufficient_quota|quota.*exhausted|usage limit|quota.*exceeded").is_match(&with_kind) {
        return format!("{provider} reported that its usage limit has been reached. Pick another model, or try again when usage is available.");
    }
    if facts.status == Some(429) || re(r"(?i)rate[_ -]?limit|too many requests").is_match(&with_kind) {
        return format!("{provider} rate limited this request. Wait a while, or pick another model and send again.");
    }
    if facts.status == Some(503) || re(r"(?i)overloaded|service unavailable|temporarily unavailable").is_match(said) {
        return format!("{provider} is overloaded or unavailable right now. Try again later, or pick another model.");
    }
    if matches!(facts.status, Some(401 | 403)) || re(r"(?i)authentication|unauthorized|invalid api key").is_match(&with_kind) {
        return format!("{provider} refused this request because of sign-in or access permissions. Check the agent's sign-in and model access, then send again.");
    }
    format!("{provider} refused this request: {}. Check the model and account settings before sending again.", said.trim_end_matches(['.', '!', '?']))
}

#[cfg(test)]
#[path = "provider_error_tests.rs"]
mod tests;

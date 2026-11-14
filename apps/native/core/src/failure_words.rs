//! What a failure card says (Electron `missionView.ts` `failureMessage`).
//!
//! The host's sentence names only the shape of a failure ("Codex invocation did not complete successfully"); two
//! runs that failed for different reasons read on screen as the same shrug. So the card adds the runtime's own last
//! word: the last line of its stderr that says something, colour codes taken off, a structured log line said as its
//! error. A provider's JSON is said as a sentence ([`crate::provider_error`]); capacity exhaustion is said in English
//! with the runtime's words kept for someone reporting it upstream; a limit the message already names in plain words
//! is not said twice; and a runtime that could not save its own settings file is told apart from the person's project.

use crate::invariants::regex;
use crate::provider_error::provider_error_sentence;

/// Capacity exhaustion, in whatever words a runtime uses for it.
const EXHAUSTION: &[&str] = &[
    r"(?i)\bresource_exhausted\b",
    r"(?i)\binsufficient_quota\b",
    r"(?i)\bquota (?:exceeded|exhausted)\b",
    r"(?i)\brate[_ -]?limit(?:ed| exceeded)?\b",
    r"(?i)\btoo many requests\b",
    r"(?i)\bhttp\s*429\b",
];

/// A failure that already names its limit in plain words, needing no runtime line after it (0.823).
const LIMIT_ALREADY_SAID: &str = r"(?i)\b(?:plan|quota) is used up\b|\bquota reached\b|\busage limit (?:has been |was )?reached\b";

/// The runtime's own settings file, as each CLI names it on disk.
const RUNTIME_CONFIG_FILES: &[&str] = &["cli-config.json", "config.json", "settings.json", "auth.json"];

fn is_exhaustion(text: &str) -> bool {
    EXHAUSTION.iter().any(|p| regex(p).is_match(text))
}

/// A runtime that could not rename over its OWN settings file (on Windows, while another run of it had it open).
fn is_locked_runtime_config(said: &str) -> bool {
    said.to_lowercase().contains("rename")
        && (regex(r"(?i)\bEPERM\b").is_match(said) || regex(r"(?i)\boperation not permitted\b").is_match(said))
        && RUNTIME_CONFIG_FILES.iter().any(|name| said.contains(name))
}

/// The last line of stderr that says something, terminal colour codes taken off.
pub fn last_stderr_line(stderr: Option<&str>) -> Option<String> {
    let plain = regex("\x1b\\[[0-9;?]*[ -/]*[@-~]").replace_all(stderr?, "");
    plain.split('\n').map(str::trim).filter(|line| !line.is_empty()).last().map(str::to_owned)
}

/// A structured log line (`level=ERROR ... message="..." error.error="..."`), said as its error, else its message;
/// anything that is not such a line is kept as it is.
pub fn message_of_log_line(line: &str) -> String {
    if !regex(r"\blevel=[A-Z]+\b").is_match(line) || !regex(r#"\b[a-zA-Z.]+=""#).is_match(line) {
        return line.to_owned();
    }
    let quoted = |key: &str| -> Option<String> {
        let at = line.find(&format!("{key}=\""))?;
        let mut text = String::new();
        let mut chars = line[at + key.len() + 2..].chars();
        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some(next) => text.push(next),
                    None => text.push('\\'),
                },
                '"' => return Some(text.trim().to_owned()),
                _ => text.push(c),
            }
        }
        Some(text.trim().to_owned())
    };
    quoted("error.error").or_else(|| quoted("error")).or_else(|| quoted("message")).unwrap_or_else(|| line.to_owned())
}

/// Two messages that say the same thing, allowing for the trim and the trailing period one channel adds.
pub fn same_sentence(a: &str, b: &str) -> bool {
    let norm = |text: &str| regex(r"\.\s*$").replace(text.trim(), "").to_lowercase();
    let (left, right) = (norm(a), norm(b));
    !left.is_empty() && (left == right || left.contains(&right) || right.contains(&left))
}

/// The failure card's words: the host's message, and the runtime's own last word when it adds something.
pub fn failure_message(message: &str, stderr: Option<&str>, runtime: Option<&str>) -> String {
    let readable = provider_error_sentence(message, runtime);
    if readable != message {
        return readable;
    }
    let Some(line) = last_stderr_line(stderr) else {
        return message.to_owned();
    };
    let said = message_of_log_line(&line);
    let readable_stderr = provider_error_sentence(&said, runtime);
    if readable_stderr != said {
        return readable_stderr;
    }
    if same_sentence(message, &said) {
        return message.to_owned();
    }
    if is_exhaustion(&said) {
        // Already said in plain words (Muse Code's plan used up, Antigravity's quota reached): the runtime's line
        // would only repeat it behind "out of capacity", request id and raw JSON included. The record keeps them.
        if regex(LIMIT_ALREADY_SAID).is_match(message) {
            return message.to_owned();
        }
        return format!("{message} The runtime reported that it is out of capacity right now \u{2014} its own limit, not this machine's: {said}");
    }
    if is_locked_runtime_config(&said) {
        return format!(
            "{message} It could not save its own settings file, which another run of the same runtime had open \u{2014} that file is the runtime's own, in your home folder, and nothing in this workspace was denied to it. Starting the run again usually works."
        );
    }
    format!("{message} The runtime's own last word was: {said}")
}

#[cfg(test)]
#[path = "failure_words_tests.rs"]
mod tests;

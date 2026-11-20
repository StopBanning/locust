//! TAG A TEAMMATE FROM ANY CONVERSATION (Electron `shared/tagging.ts`, 0.438).
//!
//! In a conversation with one teammate, a tag loops someone in: the message still goes to the teammate on screen, and
//! each teammate tagged is sent it too, in a conversation of their own, with where it came from. The person's words
//! come FIRST, so the tagged conversation is titled by them; the note of where it came from, and that conversation's
//! latest answer, follow -- fitted to the mission's limit. `conversation_turns::tagged_words` reads the words back.

use crate::engine::Teammate;
use crate::native::{Origin, Send};
use serde_json::json;

/// A tag reaches at most this many teammates at once.
pub const MAX_TAGGED: usize = 5;
/// The ledger's prompt limit (`ledger_write`: 8000 UTF-16 units, as Electron's `.length` counts).
const LIMIT: usize = 8_000;
const OVERHEAD: usize = 200;
pub(crate) const CONTEXT_MARK: &str = "\n\n(You were tagged in ";

fn units(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The longest start of `text` that is at most `most` UTF-16 units, cut on a character.
fn first_units(text: &str, most: usize) -> &str {
    let mut used = 0;
    for (at, ch) in text.char_indices() {
        used += ch.len_utf16();
        if used > most {
            return &text[..at];
        }
    }
    text
}

/// What a tagged teammate is sent: the message, then whose conversation it came from and that conversation's latest
/// answer. The answer is what gets cut when it does not fit, never the message.
pub fn tagged_prompt(message: &str, from_name: Option<&str>, answer: Option<&str>) -> String {
    let Some(from) = from_name else { return message.to_owned() };
    let answer = answer.unwrap_or("").trim();
    let room = LIMIT.saturating_sub(units(message) + OVERHEAD);
    let quoted = if units(answer) <= room {
        answer.to_owned()
    } else {
        format!("{}\n... (cut short here)", first_units(answer, room.saturating_sub(60)).trim_end())
    };
    let place = format!("{CONTEXT_MARK}{from}'s conversation.");
    if answer.is_empty() || room == 0 {
        format!("{message}{place})")
    } else {
        format!("{message}{place} {from}'s latest answer there, for context:)\n\n{quoted}")
    }
}

/// What came of a tag: who it started, and who it could not reach and why.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tagged {
    pub started: Vec<String>,
    pub refused: Vec<(String, String)>,
}

impl Tagged {
    /// The line the composer shows afterwards, or None when there is nothing to say.
    pub fn said(&self) -> Option<String> {
        let mut parts = Vec::new();
        if !self.started.is_empty() {
            parts.push(format!("Also sent to {}.", self.started.join(", ")));
        }
        for (name, why) in &self.refused {
            parts.push(format!("Not sent to {name}: {why}"));
        }
        (!parts.is_empty()).then(|| parts.join(" "))
    }
}

/// Start each tagged teammate's own conversation with the tagged prompt, recorded as started by a tag. At most
/// `MAX_TAGGED`, each once; one that is busy or refused is named, and the rest still go.
pub fn send_tagged(
    teammates: Vec<Teammate>,
    message: &str,
    from_name: Option<&str>,
    answer: Option<&str>,
    send: impl Fn(Send, Origin) -> Result<crate::engine::Started, String>,
) -> Tagged {
    let mut out = Tagged::default();
    let mut seen = std::collections::HashSet::new();
    let prompt = tagged_prompt(message, from_name, answer);
    for teammate in teammates.into_iter().filter(|t| seen.insert(t.id.clone())).take(MAX_TAGGED) {
        let name = teammate.name.clone();
        let origin = Origin { started_by: Some(json!({ "kind": "tag" })), ..Default::default() };
        let job = Send { teammate, conversation: None, prompt: prompt.clone(), model: None, effort: None };
        match send(job, origin) {
            Ok(_) => out.started.push(name),
            Err(why) if crate::native::is_room_teammate_busy_error(&why) => {
                out.refused.push((name.clone(), format!("{name} is busy. Tag them again when they are free.")));
            }
            Err(why) => out.refused.push((name, why)),
        }
    }
    out
}

#[cfg(test)]
#[path = "tagging_tests.rs"]
mod tests;

//! A relay is named by the received message, never by instructions to the runtime.
use crate::{ledger::{Metadata, Summary}, peer_thread::Message};
use std::collections::HashMap;

pub fn relayed_title(kind: Option<&str>, messages: &[Message]) -> Option<String> {
    if kind != Some("relay") { return None; }
    let asked = messages.iter().find(|m| m.direction == "received")?;
    let said = asked.text.as_ref()?.split_whitespace().collect::<Vec<_>>().join(" ");
    if said.is_empty() { return None; }
    Some(format!("{} asked: {said}", asked.from_name))
}

/// Use the same guarded ancestry walk as ordinary prompt titles, checking the
/// received message at each hop before falling back to the runtime briefing.
pub fn typed_prompt(m: &Metadata, by: &HashMap<String, Metadata>, peers: &HashMap<String, Vec<Message>>) -> String {
    let title = |current: &Metadata| relayed_title(current.started_by_kind.as_deref(), peers.get(&current.mission_id).map(Vec::as_slice).unwrap_or(&[]));
    let (source, words) = crate::conversation_turns::prompt_source(m, by, |current| title(current).is_some());
    title(source).unwrap_or_else(|| words.to_owned())
}

/// History already holds the workroom and links; no file access is needed to title a row.
pub fn history_title(summary: &Summary, messages: &HashMap<String, crate::workroom::Message>) -> Option<String> {
    let peers = summary.peer_links.iter().map(|link| {
        let message = messages.get(&link.message_id);
        Message { direction: link.direction.clone(), from_name: message.map(|m| m.from.name.clone()).unwrap_or_default(), text: message.map(|m| m.text.clone()), ..Default::default() }
    }).collect::<Vec<_>>();
    relayed_title(summary.started_by_kind.as_deref(), &peers)
}

#[cfg(test)]
#[path = "relay_title_tests.rs"]
mod tests;

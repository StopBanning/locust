//! COMPACT NOW, FROM THE CONTEXT RING (0.797, renderer/src/compactNow.ts).
//!
//! The OpenCode 2 docs review on 2026-10-19 (gap J): OpenCode offers "Compact session", and Claude Code says "Run
//! /compact to compact & continue" as its context fills. Pressing the ring opens what it says, with Compact now beneath,
//! and that sends `/compact` as the conversation's next message, exactly what typing it would send, while whatever is in
//! the box stays there. While a turn runs it waits in line instead, as a message sent then does.
//!
//! Offered only where the runtime itself lists `compact`, and only for a message that would reach this conversation's
//! runtime: a comparison, a cloud or background turn, or an earlier message being edited sends somewhere that cannot
//! compact it, so the ring then stays a ring. Another model in the route chip, or a message already waiting, keeps the
//! item and says why.
use crate::runtime_commands::CommandInfo;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompactOffer {
    Ready,
    AfterTurn,
    Refused(String),
}

pub struct OfferInput<'a> {
    /// The runtime this conversation's last turn ran on.
    pub runtime: Option<&'a str>,
    /// The runtime the route chip would send the next message to.
    pub next_runtime: &'a str,
    pub runtime_name: &'a str,
    pub commands: &'a BTreeMap<String, Vec<CommandInfo>>,
    pub running: bool,
    /// A message already waiting for the running turn to end.
    pub queued: bool,
    /// A comparison, a cloud or background turn, or an earlier message being edited.
    pub elsewhere: bool,
}

pub fn compact_offer(input: &OfferInput<'_>) -> Option<CompactOffer> {
    let runtime = input.runtime?;
    if input.elsewhere || !matches!(runtime, "claude" | "codex" | "opencode") {
        return None;
    }
    if !input.commands.get(runtime).is_some_and(|rows| rows.iter().any(|c| c.name == "compact")) {
        return None;
    }
    if input.next_runtime != runtime {
        return Some(CompactOffer::Refused(format!(
            "Your next message goes to another model. Pick {} again to compact this conversation.",
            input.runtime_name
        )));
    }
    if input.running && input.queued {
        return Some(CompactOffer::Refused("A message is already waiting for this turn to end. Compact after it goes.".into()));
    }
    Some(if input.running { CompactOffer::AfterTurn } else { CompactOffer::Ready })
}

/// What Compact now says it does, under its name.
pub fn consequence(runtime_name: &str) -> String {
    format!("Summarize the conversation so far, so the turns ahead have room. {runtime_name} also does this on its own when the context is full.")
}

/// And while a turn runs, when it waits for it.
pub fn after_turn(runtime_name: &str) -> String {
    format!("Waits until {runtime_name} finishes this turn, then summarizes the conversation so far.")
}

/// The item's name: it waits for a running turn.
pub fn item_name(offer: &CompactOffer) -> &'static str {
    if *offer == CompactOffer::AfterTurn { "Compact when this turn ends" } else { "Compact now" }
}

/// The line under the item's name: the reason when it cannot, else what it does or when it waits.
pub fn item_detail(offer: &CompactOffer, runtime_name: &str) -> String {
    match offer {
        CompactOffer::Refused(reason) => reason.clone(),
        CompactOffer::AfterTurn => after_turn(runtime_name),
        CompactOffer::Ready => consequence(runtime_name),
    }
}

#[cfg(test)]
#[path = "compact_now_tests.rs"]
mod tests;

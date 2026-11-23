//! NEEDS YOU (Electron `needsYou.ts`, 0.373): everything waiting on the person, in one place.
//!
//! Ordered most urgent first: paused runs (holding a process open), then questions a teammate stopped on,
//! then Scout cards with urgency `now`, then routines whose copy waits for Keep or Discard, then memory suggestions.

use std::collections::HashMap;

use serde_json::Value;

/// A line in the list: long enough to recognise, short enough to be a menu row (`NEEDS_YOU_TEXT`).
pub const NEEDS_YOU_TEXT: usize = 90;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NeedsYouItem {
    Approval {
        key: String,
        mission_id: String,
        teammate_id: Option<String>,
        name: String,
        what: String,
        asking: bool,
        command: bool,
    },
    Decision {
        key: String,
        mission_id: String,
        teammate_id: Option<String>,
        name: String,
        what: String,
    },
    Scout {
        key: String,
        card_id: String,
        name: String,
        what: String,
    },
    Routine {
        key: String,
        routine_id: String,
        name: String,
        what: String,
    },
    Memory {
        key: String,
        count: usize,
    },
}

impl NeedsYouItem {
    pub fn key(&self) -> &str {
        match self {
            Self::Approval { key, .. }
            | Self::Decision { key, .. }
            | Self::Scout { key, .. }
            | Self::Routine { key, .. }
            | Self::Memory { key, .. } => key,
        }
    }

    pub fn is_conversation_wait(&self) -> bool {
        matches!(self, Self::Approval { .. } | Self::Decision { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NeedsYouPlace {
    Memory,
    Home { card_id: String },
    Routines { routine_id: String },
    Conversation { mission_id: String },
}

#[derive(Clone, Copy, Debug)]
pub struct ApprovalInput<'a> {
    pub approval_id: &'a str,
    pub run_id: &'a str,
    pub mission_id: &'a str,
    pub kind: &'a str,
    pub summary: &'a str,
    pub detail: &'a str,
}

#[derive(Clone, Copy, Debug)]
pub struct ConversationInput<'a> {
    pub mission_id: &'a str,
    pub phase: &'a str,
    pub waiting_question: Option<&'a str>,
}

#[derive(Clone, Copy, Debug)]
pub struct ScoutCardInput<'a> {
    pub card_id: &'a str,
    pub urgency: &'a str,
    pub said: &'a str,
}

#[derive(Clone, Copy, Debug)]
pub struct RoutineInput<'a> {
    pub routine_id: &'a str,
    pub name: &'a str,
    pub files: Option<usize>,
}

fn one_line(text: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= NEEDS_YOU_TEXT {
        return flat;
    }
    let truncated: String = flat.chars().take(NEEDS_YOU_TEXT - 1).collect();
    format!("{}…", truncated.trim_end())
}

fn routine_wait_line(files: usize) -> String {
    if files == 0 {
        return "Waiting for you".to_owned();
    }
    format!("Waiting for you · {files} file{} in its copy", if files == 1 { "" } else { "s" })
}

/// The question a finished turn stopped on, or `None` (`needsYou.ts::openQuestion`).
///
/// The same reading the thread's decision card makes: the LAST message the runtime marked final, once `run.completed`
/// has been recorded.
pub fn open_question(events: &[Value]) -> Option<String> {
    if !events.iter().any(|e| e["type"] == "run.completed") {
        return None;
    }
    let mut order: Vec<String> = Vec::new();
    let mut by_id: HashMap<String, (String, bool)> = HashMap::new();
    for event in events {
        if event["type"] != "message.delta" {
            continue;
        }
        let payload = &event["payload"];
        let id = payload["itemId"].as_str().unwrap_or("").to_owned();
        let text = payload["text"].as_str().unwrap_or("");
        let op = payload["operation"].as_str().unwrap_or("append");
        let is_final = payload["final"].as_bool().unwrap_or(false);
        if !by_id.contains_key(&id) {
            order.push(id.clone());
        }
        let entry = by_id.entry(id).or_insert_with(|| (String::new(), false));
        if op == "replace" {
            entry.0 = text.to_owned();
        } else {
            entry.0.push_str(text);
        }
        if is_final {
            entry.1 = true;
        }
    }
    let last_final = order
        .iter()
        .rev()
        .find_map(|id| by_id.get(id).filter(|(_, is_final)| *is_final).map(|(text, _)| text.as_str()))?;
    crate::protocol_tags::parse_decision(last_final).map(|d| d.question)
}

/// Gather everything waiting on the person in one ordered list (`needsYou.ts::needsYou`).
#[allow(clippy::too_many_arguments)]
pub fn needs_you<'a, 'e>(
    approvals: &[ApprovalInput<'_>],
    owner_of_run: impl Fn(&str) -> Option<&'a str>,
    conversations: &[ConversationInput<'_>],
    owner_of_mission: impl Fn(&str) -> Option<&'a str>,
    events_of: impl Fn(&str) -> Option<&'e [Value]>,
    name_of: impl Fn(&str) -> Option<&'a str>,
    memory_waiting: usize,
    scout_cards: &[ScoutCardInput<'_>],
    scout_name: Option<&str>,
    routines: &[RoutineInput<'_>],
) -> Vec<NeedsYouItem> {
    let who = |teammate_id: Option<&'a str>| -> String {
        teammate_id.and_then(&name_of).unwrap_or("A teammate").to_owned()
    };
    let mut items = Vec::new();
    for req in approvals {
        let teammate_id = owner_of_run(req.run_id);
        let command = req.kind == "command" && !req.detail.trim().is_empty();
        items.push(NeedsYouItem::Approval {
            key: format!("approval:{}", req.approval_id),
            mission_id: req.mission_id.to_owned(),
            teammate_id: teammate_id.map(str::to_owned),
            name: who(teammate_id),
            what: one_line(if command { req.detail } else { req.summary }),
            asking: req.kind == "question",
            command,
        });
    }
    for conv in conversations {
        if conv.phase != "completed" {
            continue;
        }
        let question = conv
            .waiting_question
            .map(str::to_owned)
            .or_else(|| events_of(conv.mission_id).and_then(open_question));
        let Some(question) = question else { continue };
        let teammate_id = owner_of_mission(conv.mission_id);
        items.push(NeedsYouItem::Decision {
            key: format!("decision:{}", conv.mission_id),
            mission_id: conv.mission_id.to_owned(),
            teammate_id: teammate_id.map(str::to_owned),
            name: who(teammate_id),
            what: one_line(&question),
        });
    }
    let scout = scout_name.map(str::trim).filter(|s| !s.is_empty()).unwrap_or("Scout");
    for card in scout_cards {
        if card.urgency != "now" || card.card_id.trim().is_empty() {
            continue;
        }
        items.push(NeedsYouItem::Scout {
            key: format!("scout:{}", card.card_id),
            card_id: card.card_id.to_owned(),
            name: scout.to_owned(),
            what: one_line(card.said),
        });
    }
    for routine in routines {
        let Some(files) = routine.files else { continue };
        if routine.routine_id.trim().is_empty() {
            continue;
        }
        items.push(NeedsYouItem::Routine {
            key: format!("routine:{}", routine.routine_id),
            routine_id: routine.routine_id.to_owned(),
            name: routine.name.to_owned(),
            what: routine_wait_line(files),
        });
    }
    if memory_waiting > 0 {
        items.push(NeedsYouItem::Memory { key: "memory".to_owned(), count: memory_waiting });
    }
    items
}

/// Where the title bar opens this item (`needsYou.ts::needsYouPlace`).
pub fn needs_you_place(item: &NeedsYouItem) -> NeedsYouPlace {
    match item {
        NeedsYouItem::Memory { .. } => NeedsYouPlace::Memory,
        NeedsYouItem::Scout { card_id, .. } => NeedsYouPlace::Home { card_id: card_id.clone() },
        NeedsYouItem::Routine { routine_id, .. } => NeedsYouPlace::Routines { routine_id: routine_id.clone() },
        NeedsYouItem::Approval { mission_id, .. } | NeedsYouItem::Decision { mission_id, .. } => {
            NeedsYouPlace::Conversation { mission_id: mission_id.clone() }
        }
    }
}

/// The item as a row of the menu it opens (`needsYou.ts::needsYouLabel`).
pub fn needs_you_label(item: &NeedsYouItem) -> String {
    match item {
        NeedsYouItem::Memory { count, .. } => {
            format!("{count} memory suggestion{} waiting", if *count == 1 { "" } else { "s" })
        }
        NeedsYouItem::Scout { name, what, .. } => format!("{name} noticed: {what}"),
        NeedsYouItem::Routine { name, what, .. } => format!("{name}: {what}"),
        NeedsYouItem::Decision { name, what, .. } => format!("{name} asked: {what}"),
        NeedsYouItem::Approval { name, what, asking, command, .. } => {
            if *asking {
                format!("{name} is asking: {what}")
            } else if *command {
                format!("{name} wants to run: {what}")
            } else {
                format!("{name} needs your approval: {what}")
            }
        }
    }
}

/// The title bar chip: how many things wait, as a sentence fragment (`needsYou.ts::needsYouChip`).
pub fn needs_you_chip(count: usize) -> String {
    if count == 1 {
        "1 needs you".to_owned()
    } else {
        format!("{count} need you")
    }
}

#[cfg(test)]
#[path = "needs_you_tests.rs"]
mod needs_you_tests;

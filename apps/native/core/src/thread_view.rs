//! Conversation projection, following Electron's missionView (not the ledger's evidence).
use crate::ledger::{Item, PeerLink};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveLine {
    pub label: String,
    pub register: String,
    pub waiting: bool,
    pub started_at: String,
}

pub fn streaming(final_: bool, running: bool) -> bool {
    running && !final_
}

pub fn live_visible(running: bool, awaiting_decision: bool) -> bool {
    running && !awaiting_decision
}

/// An approval owns the waiting state, even when the approved tool remains open.
pub fn live_line(
    events: &[Value],
    running: bool,
    started_at: &str,
    awaiting_decision: bool,
) -> Option<LiveLine> {
    if !live_visible(running, awaiting_decision) {
        return None;
    }
    let at = if started_at.is_empty() {
        events.first()?.get("occurredAt")?.as_str()?
    } else {
        started_at
    };
    let mut step: Option<(String, String)> = None;
    let mut before_writing = None;
    let mut open: Vec<(String, String)> = Vec::new();
    let mut messages = std::collections::HashMap::new();
    for e in events {
        let p = &e["payload"];
        let s = |key: &str| p[key].as_str().unwrap_or("");
        match e["type"].as_str().unwrap_or("") {
            "step.started" if s("itemType") != "background" => {
                let kind = s("stepKind");
                let item = s("itemType").to_lowercase();
                let register = if kind == "reasoning" || item.contains("reasoning") {
                    "thinking"
                } else if kind == "turn" {
                    "working"
                } else if item.ends_with("message") {
                    "writing"
                } else {
                    "tool"
                };
                let label = p["message"].as_str().unwrap_or(if kind == "turn" {
                    "Working"
                } else {
                    "Thinking"
                });
                step = Some((label.to_owned(), register.into()));
                before_writing = None;
            }
            "step.completed" | "step.failed" if s("itemType") != "background" => {
                step = None;
                before_writing = None;
            }
            "tool.started" if s("parentItemId").is_empty() => {
                let label = p["title"]
                    .as_str()
                    .or(p["command"].as_str())
                    .or(p["name"].as_str())
                    .unwrap_or("");
                open.retain(|(id, _)| id != s("itemId"));
                open.push((s("itemId").into(), label.into()));
            }
            "tool.completed" | "tool.failed" => open.retain(|(id, _)| id != s("itemId")),
            "message.delta" => {
                let final_ = p["final"] == true;
                messages.insert(s("itemId").to_owned(), final_);
                if let Some((label, register)) = &mut step {
                    if matches!(register.as_str(), "working" | "writing" | "thinking") {
                        if final_ {
                            *label = before_writing.take().unwrap_or_else(|| {
                                if register == "thinking" {
                                    "Working".into()
                                } else {
                                    label.clone()
                                }
                            });
                            *register = "working".into();
                        } else {
                            before_writing.get_or_insert_with(|| {
                                if register == "thinking" {
                                    "Working".into()
                                } else {
                                    label.clone()
                                }
                            });
                            *label = "Writing".into();
                            *register = "writing".into();
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let (label, register, waiting) = if let Some((_, label)) = open.last() {
        (label.clone(), "tool".into(), false)
    } else if let Some((label, register)) = step {
        let waiting = register == "thinking";
        (label, register, waiting)
    } else if messages.values().any(|f| !f) {
        ("Writing".into(), "writing".into(), false)
    } else {
        let spoken = events.iter().any(|e| {
            !matches!(
                e["type"].as_str(),
                Some("run.started" | "adapter.diagnostic")
            )
        });
        (
            if spoken { "Working" } else { "Starting" }.into(),
            if spoken { "working" } else { "starting" }.into(),
            true,
        )
    };
    Some(LiveLine {
        label: crate::ledger::scrub(&label),
        register,
        waiting,
        started_at: at.into(),
    })
}

/// Host briefings never belong in the person's bubble. Keep the turn boundary in the ledger.
pub fn person_prompt(kind: Option<&str>) -> bool {
    kind.is_none_or(|k| {
        matches!(
            k,
            "mcp" | "routine" | "room" | "terminal" | "tag" | "compare"
        )
    })
}

pub fn received_message<'a>(message_id: &str, links: &'a [PeerLink]) -> Option<&'a PeerLink> {
    if message_id.is_empty() {
        return None;
    }
    links
        .iter()
        .find(|link| link.message_id == message_id && link.direction == "received")
}

pub fn error_already_shown(items: &[Item], error: &str) -> bool {
    items.iter().any(|item| match item {
        Item::Limit { kind, text } if kind == "quota-exhausted" => {
            crate::failure_words::same_sentence(error, text)
        }
        Item::Note { level, text } if level == "error" => {
            crate::failure_words::same_sentence(error, text)
        }
        _ => false,
    })
}

#[cfg(test)]
#[path = "thread_view_tests.rs"]
mod tests;

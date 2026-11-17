//! Conversation projection, following Electron's missionView (not the ledger's evidence).
use crate::ledger::{Item, PeerLink};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveLine {
    pub label: String,
    pub detail: Option<String>,
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
    let mut open: Vec<(String, String, Option<String>, bool)> = Vec::new();
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
                // The command without the shell that launched it (missionView.ts withoutShellWrapper).
                let command = p["command"].as_str().map(crate::shell_words::without_shell_wrapper);
                let label = p["title"]
                    .as_str()
                    .or(command.as_deref())
                    .or(p["name"].as_str())
                    .unwrap_or("");
                open.retain(|(id, ..)| id != s("itemId"));
                let mcp = crate::mcp_names::mcp_tool_parts(s("name"));
                let connector = mcp.is_some() || crate::mcp_names::is_bare_connector_tool(s("name"));
                let shown = p["title"].as_str().filter(|t| !t.is_empty()).map(str::to_owned)
                    .or_else(|| mcp.as_ref().map(|m| m.tool.clone())).unwrap_or_else(|| label.into());
                open.push((s("itemId").into(), shown, mcp.map(|m| m.server), connector));
            }
            "tool.completed" | "tool.failed" => open.retain(|(id, ..)| id != s("itemId")),
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
    let detail = open.last().and_then(|(_, _, detail, _)| detail.clone());
    let (label, register, waiting) = if let Some((_, label, _, connector)) = open.last() {
        (label.clone(), if *connector { "connector" } else { "tool" }.into(), false)
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
            if spoken { "Working" } else if events.iter().any(|e| {
                e["type"] == "adapter.diagnostic" && e["payload"]["code"].as_str().is_some_and(|c| c == "opencode.runtime_error" || c.ends_with(".provider_busy.runtime_error"))
            }) { "Trying the model again" } else { "Starting" }.into(),
            if spoken { "working" } else { "starting" }.into(),
            true,
        )
    };
    Some(LiveLine {
        label: crate::ledger::scrub(&label),
        detail: detail.map(|d| crate::ledger::scrub(&d)),
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

/// Only the newest ended turn offers the last provider-final question to answer.
pub fn ended_question(items: &[Item], running: bool, latest_turn: bool) -> Option<crate::protocol_tags::DecisionRequest> {
    if running || !latest_turn { return None; }
    let last_final = items.iter().rev().find_map(|item| match item {
        Item::Assistant { text, r#final: true, .. } => Some(text.as_str()),
        _ => None,
    })?;
    crate::protocol_tags::parse_decision(last_final)
}
#[cfg(test)] #[path="ended_question_tests.rs"] mod ended_question_tests;


/// Earlier turns are drawn whole; always retain the newest earlier turn,
/// even if it alone exceeds the budget. The current turn is never budgeted.
pub const EARLIER_ROWS_SHOWN: usize = 400;
pub fn first_earlier_turn_shown(sizes: &[usize], budget: usize) -> usize {
    if sizes.is_empty() { return 0; }
    let mut first = sizes.len() - 1;
    let mut used = sizes[first];
    while first > 0 && used.saturating_add(sizes[first - 1]) <= budget {
        first -= 1;
        used = used.saturating_add(sizes[first]);
    }
    first
}

/// Item boundary and count of earlier turns held back. A turn starts at its
/// prompt; retain any preamble with the first turn instead of losing evidence.
pub fn newest_turn_window(items: &[Item], budget: usize) -> (usize, usize) {
    let mut starts: Vec<usize> = items.iter().enumerate()
        .filter_map(|(i, item)| matches!(item, Item::User(_)).then_some(i)).collect();
    if starts.len() < 2 { return (0, 0); }
    starts[0] = 0;
    let sizes: Vec<_> = starts.windows(2).map(|w| w[1] - w[0]).collect();
    let first = first_earlier_turn_shown(&sizes, budget);
    (starts[first], first)
}

#[cfg(test)]
mod renderer_newest_tests {
    use super::*;
    #[test]
    fn the_newest_turns_whose_rows_fit_oldest_first_index() {
        assert_eq!(first_earlier_turn_shown(&[], EARLIER_ROWS_SHOWN), 0);
        assert_eq!(first_earlier_turn_shown(&[10, 10, 10], EARLIER_ROWS_SHOWN), 0);
        assert_eq!(first_earlier_turn_shown(&[100, 100, 100, 100, 100], 400), 1);
        assert_eq!(first_earlier_turn_shown(&[300, 300], 400), 1);
    }
    #[test]
    fn the_newest_earlier_turn_always_however_long() {
        assert_eq!(first_earlier_turn_shown(&[5, 900], 400), 1);
        assert_eq!(first_earlier_turn_shown(&[900], 400), 0);
    }
    #[test]
    fn each_show_earlier_adds_a_budget() {
        let sizes = vec![60; 20];
        assert_eq!(first_earlier_turn_shown(&sizes, EARLIER_ROWS_SHOWN), 14);
        assert_eq!(first_earlier_turn_shown(&sizes, EARLIER_ROWS_SHOWN * 2), 7);
        assert_eq!(first_earlier_turn_shown(&sizes, EARLIER_ROWS_SHOWN * 4), 0);
    }
}

/// Folder hover copy. Lexical paths only: no directory or credential reads.
pub fn folder_help(
    folder: Option<&str>,
    home: Option<&str>,
    windows: bool,
    system: Option<&str>,
) -> String {
    let Some(folder) = folder else {
        return "No folder chosen. Every teammate works inside one project folder.".into();
    };
    let caution = folder_caution(folder, home, windows, system);
    let mut help = format!("New conversations work in {folder}");
    if let Some(why) = caution {
        help.push_str(&format!(". This is {why}: a teammate here reads it as its project, sign-in files included. A project folder of its own is the safer place."));
    }
    help
}

/// The same lexical caution powers both the chip and the first-send boundary.
pub fn folder_caution(folder: &str, home: Option<&str>, windows: bool, system: Option<&str>) -> Option<String> {
    let normalize = |path: &str| -> Option<String> {
        let path = if windows {
            path.replace('\\', "/").to_lowercase()
        } else {
            path.to_owned()
        };
        if !path.starts_with('/')
            && !(windows
                && path.as_bytes().get(1) == Some(&b':')
                && path.as_bytes().get(2) == Some(&b'/'))
        {
            return None;
        }
        let mut parts = Vec::new();
        for part in path.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    if parts.last().is_some_and(|p: &&str| !p.ends_with(':')) {
                        parts.pop();
                    }
                }
                _ => parts.push(part),
            }
        }
        Some(parts.join("/"))
    };
    let inside = |candidate: &str, directory: &str| {
        candidate == directory
            || candidate.starts_with(&format!("{directory}/"))
            || directory.is_empty()
    };
    normalize(folder).and_then(|path| {
        if windows
            && system
                .and_then(normalize)
                .is_some_and(|sys| inside(&path, &sys))
        {
            return Some("Windows' own folder".to_owned());
        }
        let home = normalize(home?)?;
        if inside(&home, &path) {
            return Some("your home folder".to_owned());
        }
        [
            ".claude", ".codex", ".cursor", ".gemini", ".copilot", ".config",
        ]
        .into_iter()
        .find(|name| inside(&path, &format!("{home}/{name}")))
        .map(|name| format!("{name}, where the agents keep their own settings"))
    })
}

#[cfg(test)]
mod folder_help_tests {
    use super::*;
    #[test]
    fn cautions_are_lexical_bounded_and_follow_the_paths_platform() {
        let why = |path: &str, windows| {
            folder_help(
                Some(path),
                Some("/home/person"),
                windows,
                Some("C:/Windows"),
            )
            .contains("sign-in files")
        };
        assert!(why("/home/person/.claude/skills", false));
        assert!(!why("/home/person/.claude-other", false));
        assert!(!why("/home/person/.Claude", false));
        assert!(why("/HOME/PERSON/.Claude", true));
        assert!(!why("/home/person/.claude/../shop", false));
        assert!(why("/home", false));
        assert!(why("/", false));
        assert!(why("C:/Windows/System32", true));
        assert!(!why("C:/Windows-old", true));
        assert!(!why("relative/.claude", false));
    }
}

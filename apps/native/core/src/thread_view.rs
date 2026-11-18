//! Conversation projection, following Electron's missionView (not the ledger's evidence).
use crate::ledger::{Item, PeerLink};
use serde_json::Value;

/// One step of the plan with its state: "done", "running" or "pending" (missionView.ts `readPlan`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanStepView {
    pub text: String,
    pub state: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiveLine {
    pub label: String,
    /// The plan, when the line's detail is its "step N of M": what the step control opens (missionView.ts
    /// `live-step.plan`).
    pub plan: Option<Vec<PlanStepView>>,
    /// What the step under way is doing, in words (missionView.ts `live-step` `action`): the open call's
    /// `liveActionLine`, or the plan's step under way.
    pub action: Option<String>,
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

/// The live word, capitalised and ending in one ellipsis (ThreadItems.tsx `sweepText`).
pub fn sweep_text(word: &str) -> String {
    let mut chars = word.chars();
    let first = chars.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
    format!("{first}{}\u{2026}", chars.as_str())
}

/// The plan's step under way (missionView.ts `planAction` and `planAside`): its words without its own full stop (an
/// ellipsis the model wrote stays), and "step 2 of 3".
fn plan_step(event: &Value) -> Option<(String, String)> {
    let steps = event["payload"]["plan"]["plan"].as_array()?;
    let at = steps.iter().position(|s| s["status"] == "in_progress")?;
    let words = steps[at]["step"].as_str().unwrap_or("").trim();
    let words = if words.ends_with('.') && !words.ends_with("..") { &words[..words.len() - 1] } else { words };
    Some((words.to_owned(), format!("step {} of {}", at + 1, steps.len())))
}

fn plan_steps(event: &Value) -> Vec<PlanStepView> {
    let list = event["payload"]["plan"]["plan"].as_array().map(Vec::as_slice).unwrap_or(&[]);
    list.iter()
        .filter_map(|entry| {
            let (text, status) = match entry {
                Value::String(t) => (t.as_str(), String::new()),
                Value::Object(_) => (
                    ["step", "text", "title", "name", "content"].iter().find_map(|k| entry[*k].as_str())?,
                    entry["status"].as_str().unwrap_or("").to_lowercase(),
                ),
                _ => return None,
            };
            if text.is_empty() {
                return None;
            }
            let state = if status.contains("complete") || status == "done" {
                "done"
            } else if status.contains("progress") || status == "running" || status == "active" {
                "running"
            } else {
                "pending"
            };
            Some(PlanStepView { text: text.to_owned(), state })
        })
        .collect()
}

fn clip(text: &str, max: usize) -> String {
    if text.chars().count() > max { format!("{}\u{2026}", text.chars().take(max - 1).collect::<String>()) } else { text.to_owned() }
}

fn capital(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|c| c.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// A command's program, without its folder or `.exe`.
fn command_head(command: &str) -> String {
    let first = command.split_whitespace().next().unwrap_or("").trim_matches(['"', '\'']);
    let name = first.rsplit(['/', '\\']).next().unwrap_or(first).to_lowercase();
    name.strip_suffix(".exe").map(str::to_owned).unwrap_or(name)
}

/// What a command looks at (missionView.ts `commandLooksAt`).
fn command_looks_at(command: &str) -> Option<&'static str> {
    let head = command_head(command);
    if ["cat", "head", "tail", "less", "more", "type", "get-content", "gc", "bat", "nl", "sed"].contains(&head.as_str()) {
        return Some("read");
    }
    if head == "rg" && command.split_whitespace().any(|w| w == "--files") {
        return Some("list");
    }
    if ["rg", "grep", "egrep", "fgrep", "findstr", "select-string", "sls", "ag", "ack"].contains(&head.as_str()) {
        return Some("search");
    }
    ["ls", "dir", "tree", "find", "fd", "get-childitem", "gci", "ll", "la"].contains(&head.as_str()).then_some("list")
}

fn first_command(command: &str) -> &str {
    let line = command.lines().next().unwrap_or("");
    line.split("||").next().unwrap_or("").split("&&").next().unwrap_or("").split(';').next().unwrap_or("").split('|').next().unwrap_or("")
}

fn is_line_range(word: &str) -> bool {
    let w = word.strip_suffix('p').unwrap_or(word);
    w.split_once(',').is_some_and(|(a, b)| !a.is_empty() && !b.is_empty() && a.bytes().chain(b.bytes()).all(|c| c.is_ascii_digit()))
}

/// The last path-looking word of the first command, as its file name (`commandTarget`).
fn command_target(command: &str) -> Option<String> {
    let words = first_command(command).split_whitespace().skip(1).map(|w| w.trim_start_matches(['"', '\'']).trim_end_matches(['"', '\'', ',', ';']));
    let last = words.filter(|w| w.len() > 1 && !w.starts_with('-') && w.contains(['.', '/', '\\']) && !is_line_range(w)).last()?;
    last.split(['/', '\\']).rfind(|p| !p.is_empty()).map(str::to_owned)
}

/// What a search looked for: its first word that is not a flag (`commandPattern`).
fn command_pattern(command: &str) -> Option<String> {
    let word = first_command(command).split_whitespace().skip(1).find(|w| !w.starts_with('-') && w.chars().any(|c| c.is_ascii_alphanumeric()))?;
    let bare = word.trim_matches(['"', '\'']);
    (!bare.is_empty()).then(|| bare.to_owned())
}

/// The open call in words, as Claude Code's status line says it (missionView.ts `liveActionLine`, 0.569): the model's
/// own title, else what a command looks at or runs, else the file a writing tool writes. None where Electron's
/// helper, skill and reading-tool cases would answer (not carried yet): the line then shows the call as before.
pub fn live_action(title: Option<&str>, kind: &str, name: &str, command: Option<&str>) -> Option<String> {
    let title = title.map(|t| t.split_whitespace().collect::<Vec<_>>().join(" ")).filter(|t| !t.is_empty());
    if let Some(title) = title {
        return Some(clip(&capital(&title), 56));
    }
    let shell = kind == "command_execution" || (kind == "tool_use" && name.eq_ignore_ascii_case("bash")) || name == "shell";
    if shell && let Some(command) = command {
        let command = crate::shell_words::without_shell_wrapper(command);
        return Some(match command_looks_at(&command) {
            Some("read") => command_target(&command).map_or_else(|| "Reading a file".into(), |t| format!("Reading {}", clip(&t, 40))),
            Some("search") => command_pattern(&command).map_or_else(|| "Searching the files".into(), |t| format!("Searching for {}", clip(&t, 32))),
            Some(_) => "Listing files".into(),
            None => {
                let first = command.lines().next().unwrap_or("").trim();
                if first.chars().filter(char::is_ascii_alphabetic).count() < 3 { "Running a script".into() } else { format!("Running {}", clip(first, 40)) }
            }
        });
    }
    let tool = name.to_lowercase();
    let writes = ["write", "write_to_file", "writetofile", "create_file", "createfile", "edit", "edit_file", "editfile", "apply_patch", "applypatch"].contains(&tool.as_str())
        || tool.starts_with("str_replace") || tool.starts_with("replace");
    if writes || kind == "write_to_file" {
        let target = command.unwrap_or("");
        let file = target.trim().trim_matches(['"', '\'']).split(['/', '\\']).rfind(|p| !p.is_empty()).filter(|f| f.contains('.'));
        return Some(file.map_or_else(|| "Writing a file".into(), |f| format!("Editing {}", clip(f, 40))));
    }
    None
}

/// An approval owns the waiting state, even when the approved tool remains open.
pub fn live_line(
    events: &[Value],
    running: bool,
    started_at: &str,
    awaiting_decision: bool,
) -> Option<LiveLine> {
    live_line_starting(events, running, started_at, awaiting_decision, None)
}

/// `live_line` with the start's phase the host sent ("Reading the folder"), which takes the place of the word
/// "Starting" while nothing has arrived (missionView.ts `options.startingLabel`).
pub fn live_line_starting(
    events: &[Value],
    running: bool,
    started_at: &str,
    awaiting_decision: bool,
    starting_label: Option<&str>,
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
    let mut open: Vec<(String, String, Option<String>, bool, Option<String>)> = Vec::new();
    let mut messages = std::collections::HashMap::new();
    let mut plan: Option<(String, String)> = None;
    let mut plan_list: Vec<PlanStepView> = Vec::new();
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
                let action = if connector { None } else { live_action(p["title"].as_str(), s("toolKind"), s("name"), p["command"].as_str()) };
                open.push((s("itemId").into(), shown, mcp.map(|m| m.server), connector, action));
            }
            "tool.completed" | "tool.failed" => open.retain(|(id, ..)| id != s("itemId")),
            "plan.updated" => {
                plan = plan_step(e);
                plan_list = plan_steps(e);
            }
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
    let (plan_action, plan_aside) = plan.map_or((None, None), |(a, d)| (Some(a), Some(d)));
    // The open call's connector, else the plan's step: "step 2 of 3" (missionView.ts `detail`).
    let detail = open.last().and_then(|(_, _, detail, ..)| detail.clone()).or(plan_aside.clone());
    let action = match (open.last(), &step) {
        (Some((.., true, _)), _) => None,
        (Some((.., action)), _) => action.clone().or(plan_action),
        (None, Some((_, register))) if register == "thinking" => None,
        _ => plan_action,
    };
    let (label, register, waiting) = if let Some((_, label, _, connector, _)) = open.last() {
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
            }) { "Trying the model again" } else { starting_label.filter(|l| !l.is_empty()).unwrap_or("Starting") }.into(),
            if spoken { "working" } else { "starting" }.into(),
            true,
        )
    };
    let with_plan = detail.is_some() && detail == plan_aside && !plan_list.is_empty();
    Some(LiveLine {
        plan: with_plan.then_some(plan_list),
        label: crate::ledger::scrub(&label),
        action: action.map(|a| crate::ledger::scrub(&a)),
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

/// Electron's `runtimeNeverStarted` (missionView.ts, H4): any sign of the runtime at work means it started. Only a
/// record of nothing but the failure and the host's own diagnostics, naming no runtime thread, never started.
pub fn runtime_never_started<'a>(events: impl IntoIterator<Item = &'a Value>) -> bool {
    events.into_iter().all(|event| {
        matches!(event["type"].as_str(), Some("run.failed" | "adapter.diagnostic"))
            && event.get("runtimeThreadId").is_none()
    })
}

pub const RUN_AGAIN_RELAY_NOTE: &str = "The message is still waiting. It will be answered on this teammate's next run.";
pub const RUN_AGAIN_HOST_NOTE: &str =
    "Locust started this run. Start it again from its original control to keep its context.";
pub const RUN_AGAIN_NOTHING_STARTED: &str = "Nothing had started, so running this again cannot repeat anything.";
/// Thread.tsx's note for a damaged record (QA-2026-09-29 round 2, N7).
pub const DAMAGED_RECORD_NOTE: &str =
    "Part of this turn's record could not be read, so what came after it is missing here. The file is kept as it is.";

/// What a failed turn's retry is asked about: whose words it carries, and whether the host is carrying it on.
#[derive(Clone, Copy, Debug, Default)]
pub struct RunAgainTurn<'a> {
    pub prompt: &'a str,
    pub started_by_kind: Option<&'a str>,
    /// A carry-on to another agent is under way in this turn (Electron's live `handoff`).
    pub handoff: bool,
    pub continues_reason: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunAgainKind {
    Person,
    Relay,
    Host,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunAgainOffer {
    RunAgain,
    Note(&'static str),
}

static REVIEW_BRIEF: std::sync::LazyLock<regex::Regex> = std::sync::LazyLock::new(|| {
    regex::Regex::new(r"^(.+?) finished a piece of work and you are reviewing it\. You did not do this work").unwrap()
});

/// Electron's `briefAskedFor`, as a yes or no: the host's own brief (a tidy pass, a review), never the person's words.
pub fn brief_asked_for(prompt: &str) -> bool {
    crate::memory_tidy::is_tidy_prompt(Some(prompt)) || REVIEW_BRIEF.is_match(prompt)
}

/// Electron's `runAgainKind`: a relay waits for the teammate's next run; a host briefing keeps its own control.
pub fn run_again_kind(turn: &RunAgainTurn<'_>) -> RunAgainKind {
    if turn.started_by_kind == Some("relay") {
        return RunAgainKind::Relay;
    }
    if turn.handoff || turn.continues_reason == Some("route-switch") || brief_asked_for(turn.prompt) {
        return RunAgainKind::Host;
    }
    if matches!(turn.started_by_kind, None | Some("tag" | "terminal")) { RunAgainKind::Person } else { RunAgainKind::Host }
}

/// Electron's `runAgainOffer`: Run it again only for a failed, never-started run of the person's own words; a host
/// briefing cannot be retried as a newly typed person message, so it says why instead.
pub fn run_again_offer(turn: &RunAgainTurn<'_>, phase: &str, never_started: bool, running: bool) -> Option<RunAgainOffer> {
    if running || phase != "failed" || !never_started || turn.prompt.trim().is_empty() {
        return None;
    }
    Some(match run_again_kind(turn) {
        RunAgainKind::Person => RunAgainOffer::RunAgain,
        RunAgainKind::Relay => RunAgainOffer::Note(RUN_AGAIN_RELAY_NOTE),
        RunAgainKind::Host => RunAgainOffer::Note(RUN_AGAIN_HOST_NOTE),
    })
}

#[cfg(test)]
#[path = "run_again_tests.rs"]
mod run_again_tests;

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

//! CARRYING A CONVERSATION ON TO ANOTHER AGENT (shared/carry-on.ts, main/carry-on.ts, main/handoff.ts and
//! mission-store's checkpoint.ts).
//!
//! A turn whose plan ran out, whose provider failed before it did anything, or whose process died can be carried on
//! by another agent the person (or an app) picks. The new turn continues the conversation on the new agent, cold, with
//! a brief of the work so far: the task, the turns before it, the actions that never reported back (to be checked,
//! never redone blind), the actions that finished, and the previous agent's last words. The stopped turn is
//! reconciled first into a checkpoint derived from its ledger alone.
//!
//! A reply on another agent takes the same path with the person's words as the latest instruction.

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// runtimes.ts DISPLAY_NAMES: the agent's name in Locust's sentences.
pub fn display_name(runtime: &str) -> &str {
    match runtime {
        "codex" => "Codex CLI",
        "claude" => "Claude Code",
        "cursor" => "Cursor Agent",
        "gemini" => "Gemini CLI",
        "opencode" => "OpenCode",
        "copilot" => "Copilot CLI",
        "antigravity" => "Antigravity",
        "muse" => "Muse Code",
        "kilo" => "Kilo",
        "acp" => "Your own agent",
        other => other,
    }
}

/// Why a stopped turn can be carried on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Its account's usage ran out.
    Plan,
    /// Its provider, sign-in or network failed before it ran any tool.
    Provider,
    /// Its process or protocol failed.
    Process,
}

fn kind(event: &Value) -> Option<&str> {
    event["payload"]["kind"].as_str()
}

/// planRanOut: the agent's limit (`quota-exhausted`), with no completion after it.
pub fn plan_ran_out(events: &[Value]) -> bool {
    let mut limited = false;
    for e in events {
        if (e["type"] == "route.limit_detected" || e["type"] == "run.failed") && kind(e) == Some("quota-exhausted") {
            limited = true;
        } else if e["type"] == "run.completed" {
            limited = false;
        }
    }
    limited
}

const PROVIDER_TROUBLE: [&str; 16] = [
    r"(?i)\bat capacity\b",
    r"(?i)\boverloaded\b",
    r"(?i)\btemporarily unavailable\b",
    r"(?i)\bservice unavailable\b",
    r"(?i)\bhttp\s*(?:500|502|503|504|529)\b",
    r"(?i)\binternal server error\b",
    r"(?i)\bbad gateway\b",
    r"(?i)\bgateway time-?out\b",
    r"(?i)\bstream disconnected before completion\b",
    r"\b(?:ECONNREFUSED|ECONNRESET|ENOTFOUND|ETIMEDOUT|EAI_AGAIN|ENETUNREACH)\b",
    r"(?i)\bfetch failed\b",
    r"(?i)\bsocket hang up\b",
    r"(?i)\bnetwork (?:error|is unreachable)\b",
    r"(?i)\bgetaddrinfo\b",
    r"(?i)\b(?:unable|failed) to connect\b",
    r"(?i)\bconnection (?:refused|reset|error|timed out)\b",
];

/// providerFailedFirst: failed on its provider, sign-in or network before it ran any tool at all.
pub fn provider_failed_first(events: &[Value]) -> bool {
    let mut failed: Option<&Value> = None;
    for e in events {
        if matches!(e["type"].as_str(), Some("tool.started" | "tool.completed" | "tool.failed")) {
            return false;
        }
        if e["type"] == "run.completed" {
            failed = None;
        } else if e["type"] == "run.failed" {
            failed = Some(&e["payload"]);
        }
    }
    let Some(p) = failed else { return false };
    match p["kind"].as_str() {
        Some("authentication-failed" | "temporary-rate-limit") => return true,
        Some("unknown" | "process-failed") => {}
        _ => return false,
    }
    let stderr_last = p["process"]["stderr"]
        .as_str()
        .and_then(|s| s.lines().map(str::trim).rfind(|l| !l.is_empty()))
        .unwrap_or("");
    let said = [p["message"].as_str().unwrap_or(""), stderr_last];
    said.iter().any(|t| PROVIDER_TROUBLE.iter().any(|p| crate::invariants::regex(p).is_match(t)))
}

/// carryOnReason: plan, provider before tools, or a crashed process; never a finished, stopped or refused turn.
pub fn carry_on_reason(events: &[Value]) -> Option<Reason> {
    let terminal = events
        .iter()
        .rev()
        .find(|e| matches!(e["type"].as_str(), Some("run.failed" | "run.completed" | "run.cancelled")));
    let message = terminal.and_then(|e| e["payload"]["message"].as_str()).unwrap_or("");
    // Some adapters describe a host permission refusal as process-failed. That is not a crashed process.
    let refused = terminal.and_then(kind) == Some("safety-blocked")
        || crate::invariants::regex(r"(?i)(?:refusal|refused to|request (?:was )?refused|safety|not allow|does not allow|denied|outside the folder)")
            .is_match(message);
    if terminal.is_some_and(|e| e["type"] == "run.completed" || e["type"] == "run.cancelled") || refused {
        return None;
    }
    if plan_ran_out(events) {
        return Some(Reason::Plan);
    }
    if provider_failed_first(events) {
        return Some(Reason::Provider);
    }
    (terminal.is_some_and(|e| e["type"] == "run.failed") && matches!(terminal.and_then(kind), Some("process-failed" | "protocol-mismatch")))
        .then_some(Reason::Process)
}

/// carryOnRefusal: Approve each needs an agent that stops and asks; one that cannot is not carried to at all.
pub fn carry_on_refusal(mode: &str, to: &str) -> Option<String> {
    if mode != "approve-each" || matches!(to, "codex" | "opencode" | "copilot") {
        return None;
    }
    Some(format!("{} cannot ask before each action, so this turn was not carried on there.", display_name(to)))
}

/// carryOnPrompt: the carried-on turn's instruction, under the summary of what was done. Locust's words.
pub fn carry_on_prompt(from: &str, reason: Reason) -> String {
    let name = display_name(from);
    match reason {
        Reason::Process => format!(
            "{name} stopped because its process failed. Carry on from the recorded work. Reconcile any action without a confirmed result using receipts and idempotency keys before proceeding; never retry an irreversible action with an uncertain result."
        ),
        Reason::Provider => format!(
            "{name} stopped on an error from its provider before it did anything. Do the task from the start, the way the task asks."
        ),
        Reason::Plan => format!(
            "{name} ran out of its plan's usage before it finished. Carry on with the task from where it stopped: what it already did is above, so finish what is left, the way the task asks."
        ),
    }
}

/// howTurnEnded: how a turn that did not finish ended, in the brief's words.
pub fn how_turn_ended(events: &[Value]) -> Option<&'static str> {
    let end = events
        .iter()
        .rev()
        .find(|e| matches!(e["type"].as_str(), Some("run.completed" | "run.failed" | "run.cancelled")))?;
    match end["type"].as_str() {
        Some("run.completed") => None,
        Some("run.cancelled") => Some("the person stopped it"),
        _ => Some(match kind(end) {
            Some("quota-exhausted") => "its usage limit was reached",
            Some("temporary-rate-limit") => "its provider was turning requests away",
            _ => "it failed",
        }),
    }
}

/// The turn's last complete answer, its appends joined (checkpoint.ts assistantSummaryFor, unbounded).
pub fn latest_final(events: &[Value]) -> Option<String> {
    let mut buffers: Vec<(String, String)> = Vec::new();
    let mut latest = None;
    for e in events.iter().filter(|e| e["type"] == "message.delta") {
        let p = &e["payload"];
        let item = p["itemId"].as_str().unwrap_or_default();
        let piece = p["text"].as_str().unwrap_or_default();
        let at = match buffers.iter().position(|(i, _)| i == item) {
            Some(at) => at,
            None => {
                buffers.push((item.to_owned(), String::new()));
                buffers.len() - 1
            }
        };
        if p["operation"] == "replace" {
            buffers[at].1 = piece.to_owned();
        } else {
            buffers[at].1.push_str(piece);
        }
        if p["final"] == true {
            latest = Some(buffers[at].1.clone());
        }
    }
    latest
}

/// The agents the native engine can run on this computer: each whose program is found.
pub fn installed_runtimes() -> Vec<String> {
    [("claude", "claude"), ("codex", "codex"), ("cursor", "cursor-agent"), ("copilot", "copilot"), ("antigravity", "agy"), ("muse", "muse"), ("opencode", "opencode"), ("kilo", "kilo")]
        .iter()
        .filter(|(_, program)| crate::runtime::discovery::find_named(program).is_ok())
        .map(|(runtime, _)| (*runtime).to_owned())
        .collect()
}

// ---- the checkpoint ------------------------------------------------------------------------------------------------

pub const MAX_CHECKPOINT_SUMMARY_LENGTH: usize = 4000;
pub const MAX_SETTLED_NAMES: usize = 40;
pub const MAX_UNSETTLED_ACTIONS: usize = 64;

/// actionNameOf: a finished action named by what it ran.
fn action_name(p: &Value) -> String {
    let (tool_kind, name) = (p["toolKind"].as_str().unwrap_or(""), p["name"].as_str().unwrap_or(""));
    let head = if name == tool_kind { name.to_owned() } else { format!("{tool_kind}: {name}") };
    let what = p["command"].as_str().or_else(|| p["title"].as_str()).map(|w| w.split_whitespace().collect::<Vec<_>>().join(" "));
    let full = match what {
        Some(w) if !w.is_empty() && w != name => format!("{head}: {w}"),
        _ => head,
    };
    full.chars().take(300).collect()
}

/// reconcileMission: the checkpoint a stopped turn gets, from its ledger alone. `incomplete` is a ledger that stopped
/// short of its end when read, which is never safe to resume from.
pub fn reconcile(metadata: &Value, events: &[Value], incomplete: bool, reason: &str, epoch: u64, created_at: &str) -> Value {
    let mut open: Vec<(String, Value)> = Vec::new();
    let mut settled: Vec<String> = Vec::new();
    let mut started: Vec<(String, String)> = Vec::new();
    for e in events {
        let p = &e["payload"];
        let item = p["itemId"].as_str().unwrap_or_default().to_owned();
        if e["type"] == "tool.started" {
            started.retain(|(i, _)| *i != item);
            started.push((item.clone(), action_name(p)));
            open.retain(|(i, _)| *i != item);
            open.push((
                item.clone(),
                json!({"itemId": item, "toolKind": p["toolKind"], "name": p["name"], "startedAt": e["occurredAt"], "startedAtSequence": e["sequence"]}),
            ));
            continue;
        }
        if e["type"] != "tool.completed" && e["type"] != "tool.failed" {
            continue;
        }
        open.retain(|(i, _)| *i != item);
        if !settled.contains(&item) {
            settled.push(item);
        }
    }
    let mut unsettled: Vec<Value> = open.into_iter().map(|(_, a)| a).collect();
    unsettled.sort_by_key(|a| a["startedAtSequence"].as_u64().unwrap_or(0));
    unsettled.truncate(MAX_UNSETTLED_ACTIONS);
    let (safety, why) = if incomplete {
        ("unsafe", "The mission ledger stopped short of its end during recovery, so the recorded history is incomplete.".to_owned())
    } else if unsettled.len() == 1 {
        ("approval-required", "One action started and never reported an outcome, so whether it took effect is unknown.".to_owned())
    } else if !unsettled.is_empty() {
        ("approval-required", format!("{} actions started and never reported an outcome, so whether they took effect is unknown.", unsettled.len()))
    } else {
        ("safe", "Every recorded action reported an outcome and the ledger was read to its end.".to_owned())
    };
    let names: Vec<String> = settled.iter().filter_map(|id| started.iter().find(|(i, _)| i == id).map(|(_, n)| n.clone())).collect();
    let names = names[names.len().saturating_sub(MAX_SETTLED_NAMES)..].to_vec();
    let mut digest = Sha256::new();
    for e in events {
        digest.update(format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}",
            e["sequence"].as_u64().unwrap_or(0),
            e["id"].as_str().unwrap_or(""),
            e["type"].as_str().unwrap_or("")
        ));
    }
    let summary: String = latest_final(events).unwrap_or_default().replace('\0', "").chars().take(MAX_CHECKPOINT_SUMMARY_LENGTH).collect();
    let mut checkpoint = json!({
        "schemaVersion": 1,
        "missionId": metadata["missionId"],
        "runId": metadata["runId"],
        "epoch": epoch,
        "reason": reason,
        "reconciledThroughSequence": events.last().and_then(|e| e["sequence"].as_u64()).unwrap_or(0),
        "settledActions": settled,
        "settledNames": names,
        "unsettledActions": unsettled,
        "resumeSafety": safety,
        "safetyReason": why,
        "transcriptDigest": format!("{:x}", digest.finalize()),
        "assistantSummary": summary,
        "createdAt": created_at
    });
    if let Some(thread) = events.iter().find_map(|e| e["runtimeThreadId"].as_str().or_else(|| e["payload"]["runtimeThreadId"].as_str())) {
        checkpoint["runtimeThreadId"] = json!(thread);
    }
    checkpoint
}

// ---- the brief -----------------------------------------------------------------------------------------------------

/// MAX_HANDOFF_PROMPT_LENGTH: the composer's bound.
pub const MAX_HANDOFF_PROMPT_LENGTH: usize = 8000;
const NOTICE_BUDGET: usize = 120;
const ASKED_CHARS: usize = 240;
const ANSWERED_CHARS: usize = 360;
/// The optional sections in the order they are kept: the first given up is the last here.
const KEEP_ORDER: [&str; 4] = ["unsettled", "settled", "summary", "earlier"];

/// One earlier turn of the conversation, oldest first.
#[derive(Debug, Clone, PartialEq)]
pub struct EarlierTurn {
    pub asked: String,
    pub answered: Option<String>,
}

/// The brief a carried-on (or switched) turn starts with.
#[derive(Debug, Clone, PartialEq)]
pub struct Briefing {
    pub prompt: String,
    /// Sections left out to fit the bound, in the order they were dropped.
    pub omitted: Vec<String>,
    pub task_clipped: bool,
}

fn len(s: &str) -> usize {
    s.encode_utf16().count()
}

fn clipped(text: &str, limit: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= limit {
        return flat;
    }
    format!("{}\u{2026}", flat.chars().take(limit - 1).collect::<String>().trim_end())
}

fn bullets(lines: &[String]) -> String {
    lines.iter().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n")
}

struct Section {
    name: &'static str,
    text: String,
    shrink: Option<Box<dyn Fn(usize) -> Option<String>>>,
}

/// newestThatFit: the most recent items whose section fits `room`, at least one.
fn newest_that_fit(lines: &[String], room: usize, text: &dyn Fn(&[String], usize) -> String) -> Option<String> {
    (1..lines.len()).rev().map(|keep| text(&lines[lines.len() - keep..], lines.len() - keep)).find(|c| len(c) <= room)
}

fn earlier_text(kept: &[String], left: usize) -> String {
    let mut parts = Vec::new();
    if left > 0 {
        parts.push(format!("- ({left} earlier {} not listed here)", if left == 1 { "turn is" } else { "turns are" }));
    }
    parts.push(bullets(kept));
    format!("Earlier in this conversation, oldest first:\n{}", parts.join("\n"))
}

fn settled_text(shown: &[String], dropped: usize, unnamed: usize, capped: bool) -> String {
    let mut parts = Vec::new();
    if !shown.is_empty() {
        parts.push(bullets(shown));
    }
    let plural = |n: usize| if n == 1 { "action" } else { "actions" };
    if dropped > 0 {
        let n = dropped + if capped { unnamed } else { 0 };
        parts.push(format!("- and {n} earlier {}, not listed here", plural(n)));
    }
    if unnamed > 0 && !(dropped > 0 && capped) {
        parts.push(if capped {
            format!("- and {unnamed} earlier {}, not listed here", plural(unnamed))
        } else {
            format!("- {unnamed} {}{} whose details were not recorded", if shown.is_empty() { "" } else { "other " }, plural(unnamed))
        });
    }
    format!("These actions reported finishing before the stop:\n{}", parts.join("\n"))
}

fn sections_for(original: &str, checkpoint: &Value, from: &str, next: Option<&str>, earlier: &[EarlierTurn], ended: Option<&str>) -> Vec<Section> {
    let unsettled = checkpoint["unsettledActions"].as_array().cloned().unwrap_or_default();
    let next = next.map(str::trim).filter(|n| !n.is_empty());
    let clean = next.is_some() && unsettled.is_empty();
    let task = if clean {
        match ended {
            None => format!("You are taking over this conversation from another agent ({from}), which finished its last turn. The last thing it was asked was:\n\n{original}"),
            Some(how) => format!("You are taking over this conversation from another agent ({from}), whose last turn ended before it finished: {how}. The last thing it was asked was:\n\n{original}"),
        }
    } else {
        format!("You are continuing work that another agent ({from}) started and stopped partway through. The original task was:\n\n{original}")
    };
    let mut sections = vec![Section { name: "task", text: task, shrink: None }];
    if !earlier.is_empty() {
        let lines: Vec<String> = earlier
            .iter()
            .map(|t| {
                let asked = clipped(&t.asked, ASKED_CHARS);
                match t.answered.as_deref().filter(|a| !a.trim().is_empty()) {
                    None => format!("Asked: \"{asked}\" -- no reply was recorded."),
                    Some(a) => format!("Asked: \"{asked}\" -- answered: \"{}\"", clipped(a, ANSWERED_CHARS)),
                }
            })
            .collect();
        let shrink_lines = lines.clone();
        sections.push(Section {
            name: "earlier",
            text: earlier_text(&lines, 0),
            shrink: Some(Box::new(move |room| newest_that_fit(&shrink_lines, room, &earlier_text))),
        });
    }
    if !unsettled.is_empty() {
        let lines: Vec<String> = unsettled
            .iter()
            .map(|a| format!("{}: {}", a["toolKind"].as_str().unwrap_or(""), a["name"].as_str().unwrap_or("")))
            .collect();
        sections.push(Section {
            name: "unsettled",
            text: format!(
                "These actions STARTED and never reported back. Whether each took effect is unknown. Check the current state before you redo or build on any of them:\n{}",
                bullets(&lines)
            ),
            shrink: None,
        });
    }
    let settled = checkpoint["settledActions"].as_array().map(|a| a.len()).unwrap_or(0);
    if settled > 0 {
        let names: Vec<String> =
            checkpoint["settledNames"].as_array().into_iter().flatten().filter_map(|n| n.as_str().map(str::to_owned)).collect();
        let unnamed = settled - names.len().min(settled);
        let capped = names.len() >= MAX_SETTLED_NAMES;
        let shrink = (names.len() >= 2).then(|| {
            let names = names.clone();
            Box::new(move |room: usize| newest_that_fit(&names, room, &|shown: &[String], dropped| settled_text(shown, dropped, unnamed, capped)))
                as Box<dyn Fn(usize) -> Option<String>>
        });
        sections.push(Section { name: "settled", text: settled_text(&names, 0, unnamed, capped), shrink });
    }
    let summary = checkpoint["assistantSummary"].as_str().unwrap_or("").trim();
    if !summary.is_empty() {
        sections.push(Section { name: "summary", text: format!("What the previous agent said it had done:\n\n{summary}"), shrink: None });
    }
    if let Some(next) = next {
        sections.push(Section { name: "next", text: format!("The person now asks:\n\n{next}"), shrink: None });
    }
    sections
}

/// composeHandoffPrompt, without a long task's file or the person's own leave-outs (the window has neither yet).
/// None when even the task and the person's words cannot fit.
pub fn compose_handoff_prompt(original: &str, checkpoint: &Value, from: &str, next: Option<&str>, earlier: &[EarlierTurn], ended: Option<&str>) -> Option<Briefing> {
    let mut sections = sections_for(original, checkpoint, from, next, earlier, ended);
    let budget = MAX_HANDOFF_PROMPT_LENGTH - if sections.len() > 1 { NOTICE_BUDGET } else { 0 };
    let mandatory_cost = |sections: &[Section]| -> usize {
        sections
            .iter()
            .filter(|s| s.name == "task" || s.name == "next")
            .enumerate()
            .map(|(i, s)| len(&s.text) + if i == 0 { 0 } else { 2 })
            .sum()
    };
    let mut used = mandatory_cost(&sections);
    let mut task_clipped = false;
    if used > budget {
        // A long task gives up its end beside the person's new words; alone, half a task is a run on the wrong one.
        if !sections.iter().any(|s| s.name == "next") {
            return None;
        }
        let at = sections.iter().position(|s| s.name == "task")?;
        let over = used - budget;
        let task_len = len(&sections[at].text);
        let notice = |cut: usize| format!("\n\n[The original task continues for {cut} more characters that did not fit here. Ask the person if the part above is not enough.]");
        let room = task_len.checked_sub(over + len(&notice(task_len)))?;
        if room < 200 {
            return None;
        }
        let units: Vec<u16> = sections[at].text.encode_utf16().take(room).collect();
        let head = String::from_utf16_lossy(&units);
        sections[at].text = format!("{}{}", head.trim_end(), notice(task_len - room));
        task_clipped = true;
        used = mandatory_cost(&sections);
        if used > budget {
            return None;
        }
    }
    let mut kept: Vec<&'static str> = vec!["task", "next"];
    let mut omitted = Vec::new();
    for name in KEEP_ORDER {
        let Some(at) = sections.iter().position(|s| s.name == name) else { continue };
        let cost = len(&sections[at].text) + 2;
        if used + cost > budget {
            let smaller = sections[at].shrink.as_ref().and_then(|f| f(budget.saturating_sub(used + 2)));
            if let Some(smaller) = smaller {
                used += len(&smaller) + 2;
                sections[at].text = smaller;
                kept.push(name);
                continue;
            }
            omitted.push(name.to_owned());
            continue;
        }
        kept.push(name);
        used += cost;
    }
    let texts: Vec<&str> = sections.iter().filter(|s| kept.contains(&s.name)).map(|s| s.text.as_str()).collect();
    if texts.is_empty() {
        return None;
    }
    let mut prompt = texts.join("\n\n");
    if !omitted.is_empty() {
        prompt.push_str(&format!("\n\n(Some handoff detail did not fit and was left out: {}.)", omitted.join(", ")));
    }
    Some(Briefing { prompt, omitted, task_clipped })
}

#[cfg(test)]
#[path = "carry_on_tests.rs"]
mod tests;

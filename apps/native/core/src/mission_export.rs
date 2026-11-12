//! SAVE THE RECORD: one conversation, as one Markdown file or raw JSON.
//!
//! Ported from `apps/desktop/src/main/mission-export.ts` (and the pure tool-kind
//! helpers from `apps/desktop/src/shared/tool-kinds.ts`).

use crate::carry_on::display_name as runtime_display_name;
use crate::invariants;
use crate::secrets::scrub_secrets;
use chrono::Datelike;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

/// The host's own sentence, written by `answerByDecision` in index.ts when a saved rule says no.
pub const SAVED_RULE_DENIAL: &str = "A rule the person saved says no:";

/// The section the host writes a reply's words into (handoff.ts), last, under this sentence.
const HANDOFF_INSTRUCTION_SECTION: &str = "\n\nThe person now asks:\n\n";

const NOT_RECORDED: &str = "not recorded";

pub struct RecordSource<'a> {
    /// The conversation's missions, oldest first: the whole `continuesFrom` chain.
    /// Each entry has the `RecoveredMission` JSON shape from `mission-store`.
    pub missions: &'a [Value],
    /// The teammate's name, or `None` when the roster no longer says.
    pub teammate: Option<&'a str>,
    /// Where the teammate works now, when that is known. The ledger holds only a workspace id.
    pub folder: Option<&'a str>,
    /// The build writing this file.
    pub locust_version: &'a str,
    /// When the file is written (ISO), so the same ledger always gives the same file.
    pub saved_at: &'a str,
    /// A parent the chain points at that the ledger no longer holds.
    pub missing_parent: Option<&'a str>,
}

// ---- tool-kinds.ts helpers (not yet in another core module) ----

fn is_shell_tool(name: &str, tool_kind: Option<&str>) -> bool {
    static SHELL_RE: OnceLock<Regex> = OnceLock::new();
    let re = SHELL_RE.get_or_init(|| {
        invariants::regex(
            r"(?i)^(?:bash|shell|powershell|pwsh|cmd|run_command|run_terminal_cmd|execute_command)$",
        )
    });
    re.is_match(name)
        || matches!(
            tool_kind,
            Some("command_execution" | "run_command")
        )
}

fn is_edit_command(command: &str) -> bool {
    static EDIT_VERBS: OnceLock<Regex> = OnceLock::new();
    static SED_HEAD: OnceLock<Regex> = OnceLock::new();
    static SED_IN_PLACE: OnceLock<Regex> = OnceLock::new();
    static TEE_HEAD: OnceLock<Regex> = OnceLock::new();
    static TEE_FILE: OnceLock<Regex> = OnceLock::new();

    let text = command.trim();
    if EDIT_VERBS
        .get_or_init(|| invariants::regex(r"^(?:apply_patch|patch|edit|write)(?-u:\b)"))
        .is_match(text)
    {
        return true;
    }
    if SED_HEAD
        .get_or_init(|| invariants::regex(r"^sed(?-u:\b)"))
        .is_match(text)
    {
        return SED_IN_PLACE
            .get_or_init(|| invariants::regex(r"\s(?:-[a-zA-Z]*i[a-zA-Z.]*|--in-place(?-u:\b))"))
            .is_match(text);
    }
    if TEE_HEAD
        .get_or_init(|| invariants::regex(r"^tee(?-u:\b)"))
        .is_match(text)
    {
        return TEE_FILE
            .get_or_init(|| invariants::regex(r"^tee(?:\s+-[a-zA-Z-]+)*\s+[^-\s|]"))
            .is_match(text);
    }
    false
}

fn edit_tool_name(name: &str) -> bool {
    static NOT_EDIT: OnceLock<Regex> = OnceLock::new();
    static CAMEL_SPLIT: OnceLock<Regex> = OnceLock::new();
    static NON_ALPHA: OnceLock<Regex> = OnceLock::new();
    static READ_WORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();
    static EDIT_WORDS: OnceLock<HashSet<&'static str>> = OnceLock::new();

    let trimmed = name.trim();
    if trimmed.is_empty() {
        return false;
    }
    if NOT_EDIT
        .get_or_init(|| {
            invariants::regex(r"(?i)^(?:todowrite|todoread|todo_write|todo_read|write_agent|writeagent)$")
        })
        .is_match(trimmed)
    {
        return false;
    }
    if trimmed.contains("__") {
        return false;
    }
    let read_words = READ_WORDS.get_or_init(|| {
        [
            "view",
            "read",
            "open",
            "show",
            "list",
            "cat",
            "search",
            "find",
            "grep",
            "websearch",
            "fetch",
            "browse",
            "lookup",
        ]
        .into_iter()
        .collect()
    });
    let edit_words = EDIT_WORDS.get_or_init(|| {
        [
            "file", "files", "patch", "write", "edit", "delete", "remove", "create", "move",
            "rename",
        ]
        .into_iter()
        .collect()
    });
    let spaced = CAMEL_SPLIT
        .get_or_init(|| invariants::regex(r"([a-z0-9])([A-Z])"))
        .replace_all(trimmed, "$1 $2");
    let words: Vec<String> = NON_ALPHA
        .get_or_init(|| invariants::regex(r"[^A-Za-z]+"))
        .split(&spaced)
        .map(|w| w.to_ascii_lowercase())
        .collect();
    if words.iter().any(|w| read_words.contains(w.as_str())) {
        return false;
    }
    words.iter().any(|w| edit_words.contains(w.as_str()))
}

fn reports_a_change(name: &str, tool_kind: Option<&str>, command: Option<&str>) -> bool {
    if tool_kind == Some("observed_edit") {
        return true;
    }
    if is_shell_tool(name, tool_kind) {
        return command.is_some_and(is_edit_command);
    }
    edit_tool_name(name)
}

fn by_helper(payload: &Value) -> bool {
    payload
        .get("parentItemId")
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty())
}

// ---- Formatting & text helpers ----

fn handoff_instruction(prompt: &str) -> Option<&str> {
    let at = prompt.rfind(HANDOFF_INSTRUCTION_SECTION)?;
    let asked = prompt[at + HANDOFF_INSTRUCTION_SECTION.len()..].trim();
    if asked.is_empty() { None } else { Some(asked) }
}

/// A fence long enough that nothing inside it can close it.
fn fenced(text: &str, language: &str) -> String {
    let mut ticks = 3usize;
    let mut run = 0usize;
    for ch in text.chars() {
        if ch == '`' {
            run += 1;
            ticks = ticks.max(run + 1);
        } else {
            run = 0;
        }
    }
    let fence = "`".repeat(ticks);
    let body = text.trim_end_matches('\n');
    format!("{fence}{language}\n{body}\n{fence}")
}

/// Words quoted, so a heading or a list inside them cannot become this file's own.
fn quoted(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = normalized.trim_end_matches('\n');
    trimmed
        .split('\n')
        .map(|line| {
            if line.is_empty() {
                ">".to_owned()
            } else {
                format!("> {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn output_text(output: Option<&Value>) -> Option<String> {
    let v = output?;
    match v {
        Value::Null => None,
        Value::String(s) => Some(s.clone()),
        other => serde_json::to_string_pretty(other).ok(),
    }
}

fn scrubbed(text: &str, pieces_replaced: &mut usize) -> String {
    let (clean, count) = scrub_secrets(text);
    *pieces_replaced += count;
    clean
}

fn output_words(output: Option<&Value>, pieces_replaced: &mut usize) -> Option<String> {
    let words = output_text(output)?;
    Some(scrubbed(&words, pieces_replaced))
}

/// What the assistant said in one turn, rebuilt the way the thread rebuilds it: deltas append or replace per item.
fn said_in(events: &[Value]) -> String {
    let mut order: Vec<String> = Vec::new();
    let mut buffers: HashMap<String, String> = HashMap::new();
    for event in events {
        if event.get("type").and_then(Value::as_str) != Some("message.delta") {
            continue;
        }
        let p = &event["payload"];
        let item_id = p.get("itemId").and_then(Value::as_str).unwrap_or("");
        let operation = p.get("operation").and_then(Value::as_str).unwrap_or("");
        let text = p.get("text").and_then(Value::as_str).unwrap_or("");
        if !buffers.contains_key(item_id) {
            order.push(item_id.to_owned());
        }
        if operation == "replace" {
            buffers.insert(item_id.to_owned(), text.to_owned());
        } else {
            buffers
                .entry(item_id.to_owned())
                .or_default()
                .push_str(text);
        }
    }
    order
        .into_iter()
        .filter_map(|id| buffers.remove(&id))
        .filter(|text| !text.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

struct Patch {
    text: String,
    added: i64,
    removed: i64,
    truncated: bool,
}

struct Call {
    name: String,
    tool_kind: String,
    command: Option<String>,
    title: Option<String>,
    status: Option<String>,
    exit_code: Option<i64>,
    output: Option<Value>,
    patch: Option<Patch>,
    duration_ms: Option<f64>,
    started_at: String,
    background: bool,
    by_helper: bool,
    settled: bool,
    failed: bool,
}

fn parse_patch(v: &Value) -> Option<Patch> {
    if v.is_null() {
        return None;
    }
    let text = v.get("text")?.as_str()?.to_owned();
    let added = v.get("added").and_then(Value::as_i64).unwrap_or(0);
    let removed = v.get("removed").and_then(Value::as_i64).unwrap_or(0);
    let truncated = v.get("truncated").and_then(Value::as_bool).unwrap_or(false);
    Some(Patch {
        text,
        added,
        removed,
        truncated,
    })
}

/// Tool events folded into one row per call, in the order the calls began.
fn calls_in(events: &[Value]) -> Vec<Call> {
    let mut order: Vec<String> = Vec::new();
    let mut calls: HashMap<String, Call> = HashMap::new();

    for event in events {
        let Some(etype) = event.get("type").and_then(Value::as_str) else {
            continue;
        };
        if !matches!(etype, "tool.started" | "tool.completed" | "tool.failed") {
            continue;
        }
        let p = &event["payload"];
        let item_id = p
            .get("itemId")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let settled = etype != "tool.started";
        let failed_now = etype == "tool.failed";
        let name = p
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let tool_kind = p
            .get("toolKind")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let command = p.get("command").and_then(Value::as_str).map(str::to_owned);
        let title = p.get("title").and_then(Value::as_str).map(str::to_owned);
        let status = p.get("status").and_then(Value::as_str).map(str::to_owned);
        let exit_code = p.get("exitCode").and_then(Value::as_i64);
        let output = p.get("output").filter(|v| !v.is_null()).cloned();
        let patch = p.get("patch").and_then(parse_patch);
        let duration_ms = p.get("durationMs").and_then(Value::as_f64);
        let background = p.get("background").and_then(Value::as_bool) == Some(true);
        let helper = by_helper(p);
        let occurred_at = event
            .get("occurredAt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();

        if let Some(before) = calls.remove(&item_id) {
            calls.insert(
                item_id,
                Call {
                    name,
                    tool_kind,
                    command: command.or(before.command),
                    title: title.or(before.title),
                    status: status.or(before.status),
                    exit_code: exit_code.or(before.exit_code),
                    output: output.or(before.output),
                    patch: patch.or(before.patch),
                    duration_ms: duration_ms.or(before.duration_ms),
                    started_at: before.started_at,
                    background: background || before.background,
                    by_helper: helper || before.by_helper,
                    settled: settled || before.settled,
                    failed: failed_now || before.failed,
                },
            );
        } else {
            order.push(item_id.clone());
            calls.insert(
                item_id,
                Call {
                    name,
                    tool_kind,
                    command,
                    title,
                    status,
                    exit_code,
                    output,
                    patch,
                    duration_ms,
                    started_at: occurred_at,
                    background,
                    by_helper: helper,
                    settled,
                    failed: failed_now,
                },
            );
        }
    }

    order
        .into_iter()
        .filter_map(|id| calls.remove(&id))
        .collect()
}

#[derive(PartialEq, Eq)]
enum CallKind {
    Command,
    Change,
    Other,
}

fn kind_of(call: &Call) -> CallKind {
    if call.patch.is_some()
        || reports_a_change(
            &call.name,
            Some(call.tool_kind.as_str()),
            call.command.as_deref(),
        )
    {
        return CallKind::Change;
    }
    if is_shell_tool(&call.name, Some(call.tool_kind.as_str()))
        && !call.command.as_deref().is_some_and(is_edit_command)
    {
        return CallKind::Command;
    }
    CallKind::Other
}

fn is_declined(call: &Call) -> bool {
    call.status.as_deref() == Some("declined")
}

fn is_refused(call: &Call) -> bool {
    call.status.as_deref() == Some("refused")
}

fn names_a_saved_rule(call: &Call) -> bool {
    output_text(call.output.as_ref())
        .is_some_and(|text| text.contains(SAVED_RULE_DENIAL))
}

fn what(call: &Call) -> String {
    let target = match call.command.as_deref() {
        Some(cmd) if !cmd.is_empty() => cmd,
        _ => &call.name,
    };
    if is_shell_tool(&call.name, Some(call.tool_kind.as_str())) {
        return target.to_owned();
    }
    if call.name != target && call.command.is_some() {
        format!("{}: {target}", call.name)
    } else {
        target.to_owned()
    }
}

fn duration(ms: Option<f64>) -> Option<String> {
    let ms = ms?;
    if ms < 1000.0 {
        return Some(format!("{} ms", ms.round() as i64));
    }
    let seconds = ms / 1000.0;
    if seconds < 60.0 {
        Some(format!("{seconds:.1} s"))
    } else {
        Some(format!(
            "{} min {} s",
            (seconds / 60.0).floor() as i64,
            (seconds % 60.0).round() as i64
        ))
    }
}

fn own_call_state(call: &Call) -> String {
    if is_declined(call) {
        return "declined; it did not run".to_owned();
    }
    if is_refused(call) {
        return "refused; it did not run".to_owned();
    }
    if !call.settled {
        return "no result was recorded".to_owned();
    }
    let mut parts = vec![if call.failed { "failed" } else { "completed" }.to_owned()];
    if let Some(status) = call.status.as_deref()
        && !matches!(
            status.to_ascii_lowercase().as_str(),
            "completed" | "failed" | "started"
        )
    {
        parts.push(format!("runtime status \"{status}\""));
    }
    if let Some(exit_code) = call.exit_code {
        parts.push(format!("exit code {exit_code}"));
    }
    if let Some(took) = duration(call.duration_ms) {
        parts.push(took);
    }
    parts.join(" · ")
}

fn call_state(call: &Call) -> String {
    let by = if call.by_helper {
        " · made by a helper the teammate sent out"
    } else {
        ""
    };
    format!("{}{by}", own_call_state(call))
}

fn answer_label(answer: &str) -> &str {
    match answer {
        "allowed" => "Allowed",
        "allowed-always" => "Allowed for the rest of the session",
        "denied" => "Denied",
        "answered" => "Answered",
        other => other,
    }
}

fn answered_by_label(by: &str) -> &str {
    match by {
        "card" => "by the person, on the card",
        "card-saving-a-rule" => "by the person, on the card, saving it as a rule",
        "saved-rule" => "by a rule the person saved, before the card reached them",
        "earlier-always" => "by the person\u{2019}s Always on an earlier card of this run, with no card of its own",
        other => other,
    }
}

fn cards_answered(approvals: &[Value], pieces_replaced: &mut usize) -> Vec<String> {
    let mut lines = Vec::new();
    for approval in approvals {
        let answer = approval.get("answer").and_then(Value::as_str).unwrap_or("");
        let by = approval.get("by").and_then(Value::as_str).unwrap_or("");
        let kind = approval.get("kind").and_then(Value::as_str).unwrap_or("");
        let asked_at = approval.get("askedAt").and_then(Value::as_str).unwrap_or("");
        let occurred_at = approval
            .get("occurredAt")
            .and_then(Value::as_str)
            .unwrap_or("");
        let asked = approval.get("asked").and_then(Value::as_str).unwrap_or("");
        lines.push(format!(
            "- **{}** {} · {kind} · asked {asked_at} · answered {occurred_at}",
            answer_label(answer),
            answered_by_label(by)
        ));
        lines.push(String::new());
        lines.push(quoted(&format!("The card asked: {asked}")));
        lines.push(String::new());
        if let Some(words) = approval.get("words").and_then(Value::as_str) {
            let clean = scrubbed(words, pieces_replaced);
            lines.push(quoted(&format!("Said with the answer: {clean}")));
            lines.push(String::new());
        }
    }
    lines
}

fn approvals_section(
    calls: &[Call],
    events: &[Value],
    mission: &Value,
    pieces_replaced: &mut usize,
) -> String {
    static DENIED_OR_PERM: OnceLock<Regex> = OnceLock::new();
    static MULTI_BLANK: OnceLock<Regex> = OnceLock::new();

    let records_cards = mission
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        >= 20;
    let unwritten = events
        .iter()
        .filter(|e| {
            e.get("type").and_then(Value::as_str) == Some("adapter.diagnostic")
                && e["payload"].get("code").and_then(Value::as_str)
                    == Some("host.approval-not-recorded")
        })
        .count();

    let mut lines = if records_cards {
        let approvals = mission
            .get("approvals")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        cards_answered(approvals, pieces_replaced)
    } else {
        Vec::new()
    };

    for call in calls {
        if !is_declined(call) && !is_refused(call) && !names_a_saved_rule(call) {
            continue;
        }
        let words = output_words(call.output.as_ref(), pieces_replaced);
        let asked = format!("`{}`", one_line(&what(call)));
        if names_a_saved_rule(call) {
            lines.push(format!(
                "- **Denied, and the words recorded with it name a saved rule.** Asked: {asked}. The call did not run."
            ));
        } else if is_declined(call) && records_cards {
            lines.push(format!(
                "- **Declined.** Asked: {asked}. The call did not run; the answer that declined it is listed above."
            ));
        } else if is_declined(call) {
            lines.push(format!(
                "- **Declined.** Asked: {asked}. The ledger records the call as declined: the answer was no, given through Locust's approval card or a rule the person saved. It does not record which, and the words recorded with the call do not say."
            ));
        } else {
            lines.push(format!(
                "- **Refused before it ran.** Asked: {asked}. The mode or the runtime refused it; no answer from a person is recorded."
            ));
        }
        if let Some(w) = words
            && !w.is_empty()
        {
            lines.push(String::new());
            lines.push(quoted(&format!("Recorded with the call: {w}")));
            lines.push(String::new());
        }
    }

    let denied_re =
        DENIED_OR_PERM.get_or_init(|| invariants::regex(r"(?i)denied|permission"));
    for event in events {
        if event.get("type").and_then(Value::as_str) != Some("adapter.diagnostic") {
            continue;
        }
        let code = event["payload"]
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("");
        if !denied_re.is_match(code) {
            continue;
        }
        let occurred_at = event
            .get("occurredAt")
            .and_then(Value::as_str)
            .unwrap_or("");
        let message = event["payload"]
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("");
        lines.push(format!(
            "- **The runtime reported a denial** (`{code}`, {occurred_at}): {}",
            one_line(message)
        ));
    }

    let body = if lines.is_empty() {
        if records_cards {
            "No card was answered, and no call was declined or refused, in this turn.".to_owned()
        } else {
            "No declined or refused call is recorded in this turn.".to_owned()
        }
    } else {
        let joined = lines.join("\n");
        MULTI_BLANK
            .get_or_init(|| invariants::regex(r"\n\n\n+"))
            .replace_all(&joined, "\n\n")
            .into_owned()
    };

    let footer = if records_cards {
        let recorded = if unwritten > 0 {
            format!(
                "_Recorded:_ every card answered on this turn should be here; {unwritten} could not be written down, and the notes under \"How it ended\" say which. "
            )
        } else {
            "_Recorded:_ every card answered on this turn, with who answered it. ".to_owned()
        };
        format!(
            "{recorded}Not recorded: a card no one answered because the run ended first, a call that ran in a mode that asks nothing, and a call covered by an earlier \"Always\" on this run, which raises no card."
        )
    } else {
        "_Not in the ledger:_ an approval that was allowed, what the card asked in full, who answered it and how. A call listed under Commands or File changes may or may not have been approved first; the record cannot tell.".to_owned()
    };

    format!("{body}\n\n{footer}")
}

fn commands_section(calls: &[Call], pieces_replaced: &mut usize) -> String {
    let commands: Vec<&Call> = calls
        .iter()
        .filter(|c| kind_of(c) == CallKind::Command)
        .collect();
    if commands.is_empty() {
        return "No command is recorded in this turn.".to_owned();
    }
    commands
        .into_iter()
        .enumerate()
        .map(|(at, call)| {
            let bg = if call.background {
                " · run in the background"
            } else {
                ""
            };
            let mut out = vec![format!(
                "**Command {}** · {}{bg} · started {}",
                at + 1,
                call_state(call),
                call.started_at
            )];
            if let Some(title) = call.title.as_deref() {
                out.push(format!("   The runtime described it as: {}", one_line(title)));
            }
            out.push(String::new());
            out.push(fenced(call.command.as_deref().unwrap_or(&call.name), "sh"));
            match output_words(call.output.as_ref(), pieces_replaced) {
                None => {
                    out.push(String::new());
                    out.push("_No output is recorded for this command._".to_owned());
                }
                Some(words) if words.is_empty() => {
                    out.push(String::new());
                    out.push("_The command printed nothing._".to_owned());
                }
                Some(words) => {
                    out.push(String::new());
                    out.push("Output as recorded:".to_owned());
                    out.push(String::new());
                    out.push(fenced(&words, ""));
                }
            }
            out.join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn changes_section(calls: &[Call], pieces_replaced: &mut usize) -> String {
    static TOO_LARGE: OnceLock<Regex> = OnceLock::new();
    let changes: Vec<&Call> = calls
        .iter()
        .filter(|c| kind_of(c) == CallKind::Change)
        .collect();
    if changes.is_empty() {
        return "No file change is recorded in this turn.".to_owned();
    }
    changes
        .into_iter()
        .enumerate()
        .map(|(at, call)| {
            let paths = call
                .command
                .as_deref()
                .unwrap_or(&call.name)
                .split('\n')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(|p| format!("`{p}`"))
                .collect::<Vec<_>>()
                .join(", ");
            let on_disk = if call
                .status
                .as_deref()
                .is_some_and(|s| s.contains("on disk") || s.contains("from disk"))
            {
                " · seen on disk by Locust after the run, not reported by the runtime"
            } else {
                ""
            };
            let mut out = vec![format!(
                "**Change {}** · {paths} · {}{on_disk}",
                at + 1,
                call_state(call)
            )];
            let recorded = if call.patch.is_none() {
                output_text(call.output.as_ref())
            } else {
                None
            };
            if let Some(patch) = &call.patch {
                out.push(String::new());
                out.push(format!(
                    "Diff as recorded: +{} \u{2212}{}",
                    patch.added, patch.removed
                ));
                if patch.truncated {
                    out.push(String::new());
                    out.push(format!(
                        "_Too large to keep whole. The ledger holds only the first {} characters of this diff; the runtime reported +{} \u{2212}{} for the whole change._",
                        patch.text.chars().count(),
                        patch.added,
                        patch.removed
                    ));
                }
                out.push(String::new());
                out.push(fenced(&scrubbed(&patch.text, pieces_replaced), "diff"));
            } else if let Some(rec) = recorded.as_deref()
                && TOO_LARGE
                    .get_or_init(|| invariants::regex(r"(?i)too large"))
                    .is_match(&format!(
                        "{} {rec}",
                        call.status.as_deref().unwrap_or("")
                    ))
            {
                out.push(String::new());
                out.push(format!(
                    "No diff is recorded. The ledger's own words: {}",
                    one_line(&scrubbed(rec, pieces_replaced))
                ));
            } else {
                out.push(String::new());
                out.push("_No diff is recorded for this change._".to_owned());
                if let Some(rec) = recorded.as_deref()
                    && !rec.is_empty()
                {
                    out.push(String::new());
                    out.push(quoted(&format!(
                        "Recorded with the call: {}",
                        scrubbed(rec, pieces_replaced)
                    )));
                }
            }
            out.join("\n")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn other_calls_section(calls: &[Call]) -> Option<String> {
    let others: Vec<&Call> = calls
        .iter()
        .filter(|c| kind_of(c) == CallKind::Other)
        .collect();
    if others.is_empty() {
        return None;
    }
    Some(
        others
            .into_iter()
            .map(|call| format!("- `{}` · {}", one_line(&what(call)), call_state(call)))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn ending_section(mission: &Value, events: &[Value], pieces_replaced: &mut usize) -> String {
    let mut lines: Vec<String> = Vec::new();
    let terminal = events.iter().rev().find(|e| {
        matches!(
            e.get("type").and_then(Value::as_str),
            Some("run.completed" | "run.failed" | "run.cancelled")
        )
    });

    match terminal {
        None => {
            if mission.get("phase").and_then(Value::as_str) == Some("failed") {
                lines.push(
                    "**Failed before the runtime reported an ending.** Locust recorded a failure to start or keep the runtime, and the runtime itself recorded no completed, failed or stopped event.".to_owned(),
                );
            } else {
                lines.push(
                    "**No ending is recorded.** The record stops without a completed, failed or stopped event, so how this turn ended is not known. A turn still running when the file was saved reads the same way.".to_owned(),
                );
            }
        }
        Some(term) => {
            let etype = term.get("type").and_then(Value::as_str).unwrap_or("");
            let occurred_at = term.get("occurredAt").and_then(Value::as_str).unwrap_or("");
            if etype == "run.completed" {
                let model = term["payload"].get("resolvedModel").and_then(Value::as_str);
                let suffix = match model {
                    Some(m) => format!(" It named the model it ran as `{m}`."),
                    None => String::new(),
                };
                lines.push(format!(
                    "**Finished.** The runtime reported the run complete at {occurred_at}.{suffix}"
                ));
            } else if etype == "run.failed" {
                let kind = term["payload"]
                    .get("kind")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                let message = term["payload"]
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                lines.push(format!("**Failed** at {occurred_at} ({kind})."));
                if !message.trim().is_empty() {
                    lines.push(String::new());
                    lines.push(quoted(message));
                }
            } else {
                lines.push(format!("**Stopped before it finished**, at {occurred_at}."));
            }
        }
    }

    for event in events {
        if event.get("type").and_then(Value::as_str) == Some("route.limit_detected") {
            let kind = event["payload"]
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("");
            let occurred_at = event
                .get("occurredAt")
                .and_then(Value::as_str)
                .unwrap_or("");
            let message = event["payload"]
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("");
            lines.push(String::new());
            lines.push(format!(
                "The route reported a limit ({kind}) at {occurred_at}: {}",
                one_line(message)
            ));
        }
    }

    if let Some(failures) = mission.get("hostFailures").and_then(Value::as_array) {
        for failure in failures {
            let code = failure.get("code").and_then(Value::as_str).unwrap_or("");
            let occurred_at = failure
                .get("occurredAt")
                .and_then(Value::as_str)
                .unwrap_or("");
            let message = failure
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("");
            lines.push(String::new());
            lines.push(format!(
                "Locust reported `{code}` at {occurred_at}: {}",
                one_line(message)
            ));
        }
    }

    for event in events {
        if event.get("type").and_then(Value::as_str) == Some("adapter.diagnostic")
            && let Some(code) = event["payload"].get("code").and_then(Value::as_str)
            && code.starts_with("host.")
        {
            let occurred_at = event
                .get("occurredAt")
                .and_then(Value::as_str)
                .unwrap_or("");
            let message = event["payload"]
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("");
            lines.push(String::new());
            lines.push(format!(
                "Note Locust wrote into the record (`{code}`, {occurred_at}): {}",
                one_line(message)
            ));
        }
    }

    if let Some(checks) = mission.get("editChecks").and_then(Value::as_array) {
        for check in checks {
            let command = check.get("command").and_then(Value::as_str).unwrap_or("");
            let outcome = check.get("outcome").and_then(Value::as_str).unwrap_or("");
            let occurred_at = check
                .get("occurredAt")
                .and_then(Value::as_str)
                .unwrap_or("");
            lines.push(String::new());
            lines.push(format!(
                "The person's check, `{}`, after this turn: **{outcome}** ({occurred_at}).",
                one_line(command)
            ));
            if let Some(new_lines) = check.get("newLines").and_then(Value::as_array)
                && !new_lines.is_empty()
            {
                let joined = new_lines
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join("\n");
                lines.push(String::new());
                lines.push(fenced(&scrubbed(&joined, pieces_replaced), ""));
            }
        }
    }

    lines.join("\n")
}

fn model_of(mission: &Value) -> String {
    let model = mission["metadata"]
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or("");
    let ran = mission
        .get("events")
        .and_then(Value::as_array)
        .and_then(|events| {
            events
                .iter()
                .rev()
                .find(|e| e.get("type").and_then(Value::as_str) == Some("run.completed"))
        })
        .and_then(|e| e["payload"].get("resolvedModel"))
        .and_then(Value::as_str);
    match ran {
        Some(r) if r != model => format!("`{model}`, which the runtime named `{r}`"),
        _ => format!("`{model}`"),
    }
}

fn run_line(mission: &Value) -> String {
    let meta = &mission["metadata"];
    let runtime = meta.get("runtime").and_then(Value::as_str).unwrap_or("");
    let mode = meta
        .get("mode")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| format!("{NOT_RECORDED} (this turn predates it)"));
    let sandbox = meta.get("sandbox").and_then(Value::as_str).unwrap_or("");
    let mut parts = vec![
        runtime_display_name(runtime).to_owned(),
        format!("model {}", model_of(mission)),
        format!("mode {mode}"),
        format!("sandbox {sandbox}"),
    ];
    if let Some(cli) = meta.get("cliVersion").and_then(Value::as_str) {
        parts.push(format!("CLI {cli}"));
    }
    parts.join(" · ")
}

fn integrity(missions: &[Value], missing_parent: Option<&str>) -> String {
    let mut problems: Vec<String> = Vec::new();
    for mission in missions {
        let fallback_id = mission["metadata"]
            .get("missionId")
            .and_then(Value::as_str)
            .unwrap_or("");
        if let Some(issues) = mission.get("issues").and_then(Value::as_array) {
            for issue in issues {
                let code = issue.get("code").and_then(Value::as_str).unwrap_or("");
                let message = issue.get("message").and_then(Value::as_str).unwrap_or("");
                let id = issue
                    .get("missionId")
                    .and_then(Value::as_str)
                    .unwrap_or(fallback_id);
                problems.push(format!("`{code}`: {} ({id})", one_line(message)));
            }
        }
    }
    if let Some(parent) = missing_parent {
        problems.push(format!(
            "An earlier turn, `{parent}`, is not in the ledger (deleted, in the trash, or unreadable). This record begins after it."
        ));
    }
    if problems.is_empty() {
        "**record readable.** Every ledger file for this conversation was read and its event sequence has no gap. This is not tamper evidence: a record edited at rest, or a ledger file deleted, reads back the same.".to_owned()
    } else {
        let noun = if problems.len() == 1 {
            "problem was"
        } else {
            "problems were"
        };
        let mut out = vec![format!(
            "**record incomplete.** {} {noun} found reading this conversation's ledger, so what follows may stop short of what happened:",
            problems.len()
        )];
        for p in problems {
            out.push(format!("  - {p}"));
        }
        out.join("\n")
    }
}

fn handoffs_section(mission: &Value, missions: &[Value]) -> String {
    let mut lines: Vec<String> = Vec::new();
    let meta = &mission["metadata"];
    if let Some(link) = meta.get("continuesFrom")
        && link.get("reason").and_then(Value::as_str) == Some("route-switch")
    {
        let link_id = link.get("missionId").and_then(Value::as_str).unwrap_or("");
        let prior = missions
            .iter()
            .find(|entry| entry["metadata"].get("missionId").and_then(Value::as_str) == Some(link_id));
        let from = match prior {
            None => format!("a turn that is not in this record (`{link_id}`)"),
            Some(p) => format!(
                "{}, model {}",
                runtime_display_name(
                    p["metadata"].get("runtime").and_then(Value::as_str).unwrap_or("")
                ),
                model_of(p)
            ),
        };
        let runtime = meta.get("runtime").and_then(Value::as_str).unwrap_or("");
        let created_at = meta.get("createdAt").and_then(Value::as_str).unwrap_or("");
        lines.push(format!(
            "- **Handed over** from {from}, to {}, model {}, at {created_at}.",
            runtime_display_name(runtime),
            model_of(mission)
        ));

        let epoch = link.get("checkpointEpoch").and_then(Value::as_u64);
        let checkpoint = prior.and_then(|p| {
            p.get("checkpoints")
                .and_then(Value::as_array)
                .and_then(|cps| cps.iter().find(|cp| cp.get("epoch").and_then(Value::as_u64) == epoch))
        });
        match checkpoint {
            None => {
                lines.push(format!(
                    "  - It resumed from checkpoint {}, which is not in this record.",
                    epoch.unwrap_or(0)
                ));
            }
            Some(cp) => {
                let cp_epoch = cp.get("epoch").and_then(Value::as_u64).unwrap_or(0);
                let reason = cp.get("reason").and_then(Value::as_str).unwrap_or("");
                let safety = cp
                    .get("resumeSafety")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                lines.push(format!(
                    "  - Resumed from checkpoint {cp_epoch} ({reason}), recorded as {safety}."
                ));
                let unsettled = cp
                    .get("unsettledActions")
                    .and_then(Value::as_array)
                    .map(Vec::as_slice)
                    .unwrap_or(&[]);
                if unsettled.is_empty() {
                    lines.push("  - No action was left unsettled at that checkpoint.".to_owned());
                } else {
                    let names = unsettled
                        .iter()
                        .map(|a| {
                            format!(
                                "`{}`",
                                one_line(a.get("name").and_then(Value::as_str).unwrap_or(""))
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(", ");
                    lines.push(format!("  - Left unsettled at that checkpoint: {names}."));
                }
            }
        }

        if let Some(left_out) = link.get("leftOut").and_then(Value::as_array) {
            let joined = left_out
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("  - The hand-over brief left out, to fit: {joined}."));
        } else {
            lines.push("  - What the hand-over brief left out to fit: none recorded.".to_owned());
        }
        if let Some(by_you) = link.get("leftOutByYou").and_then(Value::as_array)
            && !by_you.is_empty()
        {
            let joined = by_you
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!(
                "  - The person chose to leave out of the brief: {joined}."
            ));
        }
        lines.push(
            "  - The brief the host wrote for the new runtime is held in the raw record, not repeated here.".to_owned(),
        );
    }

    if let Some(peers) = mission.get("peerLinks").and_then(Value::as_array) {
        for peer in peers {
            let dir = peer.get("direction").and_then(Value::as_str).unwrap_or("");
            let verb = if dir == "received" {
                "Received a message from"
            } else {
                "Posted a message to"
            };
            let peer_id = peer
                .get("peerTeammateId")
                .and_then(Value::as_str)
                .unwrap_or("");
            let occurred_at = peer.get("occurredAt").and_then(Value::as_str).unwrap_or("");
            let msg_id = peer.get("messageId").and_then(Value::as_str).unwrap_or("");
            lines.push(format!(
                "- **{verb} another teammate** (`{peer_id}`) at {occurred_at}. The message is held in the workroom, not in this ledger (message `{msg_id}`)."
            ));
        }
    }

    if lines.is_empty() {
        "No hand-off is recorded in this turn.".to_owned()
    } else {
        lines.join("\n")
    }
}

fn asked_section(mission: &Value, teammate: &str) -> (&'static str, String) {
    let meta = &mission["metadata"];
    let prompt = meta.get("prompt").and_then(Value::as_str).unwrap_or("");
    let by = meta.get("startedBy");
    let by_kind = by.and_then(|v| v.get("kind")).and_then(Value::as_str);
    let host = |origin: String| {
        (
            "Asked",
            format!("_{origin} The prompt the ledger holds:_\n\n{}", quoted(prompt)),
        )
    };

    if let Some(kind) = by_kind {
        let by = by.expect("by_kind implies by");
        match kind {
            "relay" => {
                let hop = by.get("hop").and_then(Value::as_u64).unwrap_or(0);
                return host(format!(
                    "Nobody typed this turn: Locust started it so {teammate} could answer another teammate (automatic turn {hop})."
                ));
            }
            "routine" => {
                let step = by.get("step").and_then(Value::as_u64).unwrap_or(0);
                return host(format!(
                    "Nobody typed this turn: Locust replayed step {step} of a saved routine."
                ));
            }
            "resume" => {
                let epoch = by.get("epoch").and_then(Value::as_u64).unwrap_or(0);
                return host(format!(
                    "Nobody typed this turn: Locust picked the work back up from checkpoint {epoch} after an interruption."
                ));
            }
            "side" => {
                let q = by.get("question").and_then(Value::as_u64).unwrap_or(0);
                return host(format!(
                    "A side question (number {q}) asked on a fork of the conversation; the conversation itself never saw it."
                ));
            }
            "carry-on" => {
                let from = by.get("from").and_then(Value::as_str).unwrap_or("");
                return host(format!(
                    "Nobody typed this turn: {} could not go on, its plan used up or its provider failing, so Locust carried the work on.",
                    runtime_display_name(from)
                ));
            }
            "check-fix" => {
                let cmd = by.get("command").and_then(Value::as_str).unwrap_or("");
                let fix = by.get("fix").and_then(Value::as_u64).unwrap_or(0);
                let of = by.get("of").and_then(Value::as_u64).unwrap_or(0);
                return host(format!(
                    "Nobody typed this turn: the check `{cmd}` failed after the turn before, so Locust sent it back to fix (fix {fix} of {of})."
                ));
            }
            "terminal" => {
                let exchange = by.get("exchange").and_then(Value::as_u64).unwrap_or(0);
                return (
                    "The person said, in the runtime's own terminal",
                    format!(
                        "_Locust did not run this exchange (number {exchange}); it was brought into the record afterwards._\n\n{}",
                        quoted(prompt)
                    ),
                );
            }
            _ => {}
        }
    }

    if meta
        .get("continuesFrom")
        .and_then(|l| l.get("reason"))
        .and_then(Value::as_str)
        == Some("route-switch")
    {
        if let Some(asked) = handoff_instruction(prompt) {
            return ("The person said", quoted(asked));
        }
        return (
            "Asked",
            "_Nobody typed this turn. Locust carried the work to another runtime on its own; the brief it wrote is held in the raw record._".to_owned(),
        );
    }

    ("The person said", quoted(prompt))
}

fn turn_section(
    mission: &Value,
    at: usize,
    missions: &[Value],
    teammate: &str,
    pieces_replaced: &mut usize,
) -> String {
    let events = mission
        .get("events")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let calls = calls_in(events);
    let meta = &mission["metadata"];
    let (asked_heading, asked_body) = asked_section(mission, teammate);
    let said = said_in(events);
    let other = other_calls_section(&calls);
    let created_at = meta.get("createdAt").and_then(Value::as_str).unwrap_or("");
    let last_updated_at = mission
        .get("lastUpdatedAt")
        .and_then(Value::as_str)
        .unwrap_or("");
    let mission_id = meta.get("missionId").and_then(Value::as_str).unwrap_or("");

    let mut out = vec![
        format!("## Turn {}", at + 1),
        String::new(),
        run_line(mission),
        format!("Started {created_at} · last recorded {last_updated_at} · `{mission_id}`"),
    ];
    if let Some(link) = meta.get("continuesFrom")
        && link.get("edited").and_then(Value::as_bool) == Some(true)
    {
        let link_id = link.get("missionId").and_then(Value::as_str).unwrap_or("");
        out.push(String::new());
        out.push(format!(
            "_This turn is an edit of an earlier message: it follows `{link_id}` in place of the turn that first followed it. That earlier version is still in the ledger and is not part of this file._"
        ));
    }
    out.extend([
        String::new(),
        format!("### {asked_heading}"),
        String::new(),
        asked_body,
        String::new(),
        format!("### {teammate} said"),
        String::new(),
        if said.is_empty() {
            "_Nothing was said in this turn._".to_owned()
        } else {
            quoted(&said)
        },
        String::new(),
        "### Approvals and refusals".to_owned(),
        String::new(),
        approvals_section(&calls, events, mission, pieces_replaced),
        String::new(),
        "### Commands".to_owned(),
        String::new(),
        commands_section(&calls, pieces_replaced),
        String::new(),
        "### File changes".to_owned(),
        String::new(),
        changes_section(&calls, pieces_replaced),
    ]);
    if let Some(other_text) = other {
        out.extend([
            String::new(),
            "### Other tool calls".to_owned(),
            String::new(),
            other_text,
        ]);
    }
    out.extend([
        String::new(),
        "### Hand-offs".to_owned(),
        String::new(),
        handoffs_section(mission, missions),
        String::new(),
        "### How it ended".to_owned(),
        String::new(),
        ending_section(mission, events, pieces_replaced),
    ]);
    out.join("\n")
}

/// The whole conversation, as Markdown.
pub fn mission_record_markdown(source: &RecordSource<'_>) -> String {
    let missions = source.missions;
    let teammate = source.teammate.unwrap_or("The teammate");
    let mut out = vec![
        format!(
            "# Record of a conversation with {}",
            source.teammate.unwrap_or("a teammate")
        ),
        String::new(),
    ];
    let (Some(first), Some(last)) = (missions.first(), missions.last()) else {
        out.push(
            "The ledger holds no turn of this conversation, so there is nothing to record."
                .to_owned(),
        );
        return format!("{}\n", out.join("\n"));
    };

    let last_updated = last
        .get("lastUpdatedAt")
        .and_then(Value::as_str)
        .unwrap_or("");
    let last_phase = last.get("phase").and_then(Value::as_str).unwrap_or("");
    let ended = if last_phase == "interrupted" {
        format!("no ending is recorded; the last thing recorded is at {last_updated}")
    } else {
        format!("{last_updated} ({last_phase})")
    };

    let mut pieces_replaced = 0usize;
    let turns: Vec<String> = missions
        .iter()
        .enumerate()
        .map(|(at, mission)| {
            turn_section(mission, at, missions, teammate, &mut pieces_replaced)
        })
        .collect();

    let workspace_id = first["metadata"]
        .get("workspaceId")
        .and_then(Value::as_str)
        .unwrap_or("");
    let created_at = first["metadata"]
        .get("createdAt")
        .and_then(Value::as_str)
        .unwrap_or("");

    let teammate_line = match source.teammate {
        Some(t) => t.to_owned(),
        None => format!("{NOT_RECORDED}: the roster no longer names them"),
    };
    let folder_line = match source.folder {
        None => format!(
            "{NOT_RECORDED}. The ledger holds only the workspace id `{workspace_id}`."
        ),
        Some(f) => format!(
            "{f} (where the teammate works now; the ledger holds only the workspace id `{workspace_id}`)"
        ),
    };
    let secrets_line = if pieces_replaced == 0 {
        "- **Secret-shaped text:** none found.".to_owned()
    } else {
        let noun = if pieces_replaced == 1 {
            "piece"
        } else {
            "pieces"
        };
        format!(
            "- **Secret-shaped text:** {pieces_replaced} {noun} replaced in this file. The raw record keeps the original."
        )
    };

    out.extend([
        format!("- **Teammate:** {teammate_line}"),
        format!("- **Folder:** {folder_line}"),
        format!("- **Started:** {created_at}"),
        format!("- **Ended:** {ended}"),
        format!("- **Turns:** {}", missions.len()),
        format!(
            "- **Locust version:** {}, the build that wrote this file. The ledger does not record which build ran each turn.",
            source.locust_version
        ),
        format!("- **Saved:** {}", source.saved_at),
        format!(
            "- **The record's own check:** {}",
            integrity(missions, source.missing_parent)
        ),
        secrets_line,
        String::new(),
        "## Who answered".to_owned(),
        String::new(),
    ]);

    for (at, mission) in missions.iter().enumerate() {
        if at > 0 {
            let prev_rt = missions[at - 1]["metadata"]
                .get("runtime")
                .and_then(Value::as_str)
                .unwrap_or("");
            let curr_rt = mission["metadata"]
                .get("runtime")
                .and_then(Value::as_str)
                .unwrap_or("");
            if prev_rt != curr_rt {
                out.push(format!(
                    "- \u{21aa} **Handed over** from {} to {}.",
                    runtime_display_name(prev_rt),
                    runtime_display_name(curr_rt)
                ));
            }
        }
        out.push(format!("- **Turn {}** · {}", at + 1, run_line(mission)));
    }
    out.push(String::new());
    for turn in turns {
        out.push(turn);
        out.push(String::new());
    }
    let collapsed = collapse_outside_fences(&out.join("\n"));
    format!("{}\n", collapsed.trim_end())
}

/// Runs of blank lines become one -- outside fenced blocks only: a command's output is kept as recorded.
pub fn collapse_outside_fences(text: &str) -> String {
    static FENCE_START: OnceLock<Regex> = OnceLock::new();
    let fence_re = FENCE_START.get_or_init(|| invariants::regex(r"^(`{3,}|~{3,})"));
    let mut kept: Vec<&str> = Vec::new();
    let mut fence: Option<String> = None;
    let mut blanks = 0usize;
    for line in text.split('\n') {
        let marker = fence_re.find(line).map(|m| m.as_str());
        if fence.is_none()
            && let Some(m) = marker
        {
            fence = Some(m.to_owned());
        } else if let Some(ref open) = fence
            && let Some(m) = marker
            && line.starts_with(open.as_str())
            && line.trim().len() == m.len()
        {
            fence = None;
        }
        if fence.is_none() && marker.is_none() && line.trim().is_empty() {
            blanks += 1;
            if blanks > 1 {
                continue;
            }
        } else {
            blanks = 0;
        }
        kept.push(line);
    }
    kept.join("\n")
}

/// The events exactly as the ledger holds them, for the person who wants to check the Markdown against them.
pub fn raw_record_json(missions: &[Value], locust_version: &str, saved_at: &str) -> String {
    let body = json!({
        "warning": "This file is the ledger as recorded, including any secret-shaped text the Markdown replaced.",
        "writtenBy": format!("Locust {locust_version}"),
        "savedAt": saved_at,
        "missions": missions,
    });
    let serialized = serde_json::to_string_pretty(&body).unwrap_or_else(|_| "{}".to_owned());
    format!("{serialized}\n")
}

/// `Locust record - <teammate> - <date>.md`, with nothing a file name cannot hold.
pub fn record_file_name(teammate: Option<&str>, now: impl Datelike) -> String {
    let cleaned: String = teammate
        .unwrap_or("conversation")
        .chars()
        .filter(|&c| !matches!(c, '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && (c as u32) > 0x1f)
        .collect();
    let collapsed = one_line(&cleaned);
    let truncated: String = collapsed.chars().take(60).collect();
    let who = if truncated.is_empty() {
        "conversation"
    } else {
        &truncated
    };
    format!(
        "Locust record - {who} - {:04}-{:02}-{:02}.md",
        now.year(),
        now.month(),
        now.day()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::SECRET_SHAPED_TEXT_REMOVED;
    use chrono::NaiveDate;

    const GOLDEN: &str = include_str!("../../../desktop/src/main/a-saved-record-says-only-what-the-ledger-holds.golden.md");
    const SAVED_AT: &str = "2026-10-03T08:00:00.000Z";

    fn iso(minute: u32, second: u32) -> String {
        format!("2026-10-02T09:{minute:02}:{second:02}.000Z")
    }

    fn process_evidence() -> Value {
        json!({
            "exitCode": 0,
            "signal": null,
            "stderr": "",
            "stderrTruncated": false,
            "recordCount": 1,
            "inputDeliveryFailed": false,
            "outputLimitExceeded": false,
            "oversizedRecordsDropped": 0,
            "forcedTerminationAttempted": false,
            "terminationUnconfirmed": false,
            "startedAt": iso(0, 0),
            "finishedAt": iso(1, 0),
        })
    }

    fn make_metadata(mission_id: &str, prompt: &str, runtime: &str, created_at: String) -> Value {
        let model = if runtime == "codex" { "gpt-test" } else { "sonnet" };
        let cli_version = if runtime == "codex" { "0.160.0" } else { "2.1.0" };
        json!({
            "missionId": mission_id,
            "runId": format!("run_{mission_id}"),
            "prompt": prompt,
            "runtime": runtime,
            "model": model,
            "requestedRouteId": runtime,
            "resolvedRouteId": format!("{runtime}:default"),
            "cliVersion": cli_version,
            "workspaceId": "ws_test",
            "sandbox": "workspace-write",
            "mode": "approve-each",
            "executionPolicyVersion": 1,
            "createdAt": created_at,
        })
    }

    struct Events {
        run_id: String,
        mission_id: String,
        runtime: String,
        n: u64,
    }

    impl Events {
        fn new(meta: &Value) -> Self {
            Self {
                run_id: meta["runId"].as_str().unwrap().to_owned(),
                mission_id: meta["missionId"].as_str().unwrap().to_owned(),
                runtime: meta["runtime"].as_str().unwrap().to_owned(),
                n: 0,
            }
        }

        fn make(&mut self, etype: &str, payload: Value, minute: u32, second: u32) -> Value {
            self.n += 1;
            json!({
                "id": format!("{}:{}", self.run_id, self.n),
                "runId": self.run_id,
                "missionId": self.mission_id,
                "sequence": self.n,
                "occurredAt": iso(minute, second),
                "sourceAdapter": self.runtime,
                "type": etype,
                "payload": payload,
            })
        }

        fn started(&mut self, minute: u32) -> Value {
            self.make(
                "run.started",
                json!({"runtimeThreadId": "thread_1", "evidence": {"redacted": true}}),
                minute,
                0,
            )
        }

        fn said(&mut self, text: &str, minute: u32, item_id: &str) -> Value {
            self.make(
                "message.delta",
                json!({"itemId": item_id, "operation": "append", "text": text, "final": true, "evidence": {"redacted": true}}),
                minute,
                5,
            )
        }

        fn tool(
            &mut self,
            etype: &str,
            item_id: &str,
            fields: Value,
            minute: u32,
            second: u32,
        ) -> Value {
            let mut payload = json!({
                "itemId": item_id,
                "toolKind": "command_execution",
                "name": "shell",
                "phase": if etype == "tool.started" { "started" } else { "completed" },
                "evidence": {"redacted": true},
            });
            if let (Some(dst), Some(src)) = (payload.as_object_mut(), fields.as_object()) {
                for (k, v) in src {
                    dst.insert(k.clone(), v.clone());
                }
            }
            self.make(etype, payload, minute, second)
        }

        fn completed(&mut self, minute: u32, resolved_model: Option<&str>) -> Value {
            let mut payload = json!({
                "runtimeThreadId": "thread_1",
                "process": process_evidence(),
            });
            if let Some(m) = resolved_model {
                payload["resolvedModel"] = json!(m);
            }
            self.make("run.completed", payload, minute, 50)
        }

        fn failed(&mut self, message: &str, minute: u32) -> Value {
            self.make(
                "run.failed",
                json!({
                    "kind": "process-failed",
                    "message": message,
                    "runtimeThreadId": "thread_1",
                    "runtimeTerminal": "failed",
                    "process": process_evidence(),
                }),
                minute,
                50,
            )
        }

        fn diagnostic(&mut self, code: &str, message: &str, minute: u32, second: u32) -> Value {
            self.make(
                "adapter.diagnostic",
                json!({
                    "level": "info",
                    "code": code,
                    "message": message,
                    "terminal": false,
                    "evidence": {"redacted": true},
                }),
                minute,
                second,
            )
        }
    }

    const DIFF: &str = "diff --git a/README.md b/README.md\n--- a/README.md\n+++ b/README.md\n@@ -1,2 +1,2 @@\n # Locust\n-Teh best tool\n+The best tool";

    fn seed_turns(tear_turn_three: bool) -> Vec<Value> {
        let one = make_metadata(
            "mission_one",
            "Tidy the build folder and fix the typo in the README.",
            "codex",
            iso(0, 0),
        );
        let mut a = Events::new(&one);
        let events_one = vec![
            a.started(0),
            a.said("I will run the tests first, then clean up.", 0, "msg_1"),
            a.tool("tool.started", "c1", json!({"command": "npm test"}), 1, 0),
            a.tool(
                "tool.completed",
                "c1",
                json!({"command": "npm test", "output": "Tests 12 passed", "exitCode": 0, "status": "completed", "durationMs": 4200}),
                1,
                5,
            ),
            a.tool("tool.started", "c2", json!({"command": "rm -rf build"}), 1, 10),
            a.tool(
                "tool.failed",
                "c2",
                json!({"command": "rm -rf build", "status": "declined", "output": "The person declined this, and said: use the build script instead"}),
                1,
                12,
            ),
            a.tool("tool.started", "c3", json!({"command": "git push origin main"}), 1, 20),
            a.tool(
                "tool.failed",
                "c3",
                json!({
                    "command": "git push origin main",
                    "status": "declined",
                    "output": format!("Denied in Locust. The person declined this, and said: {SAVED_RULE_DENIAL} never run git push for Wren in this folder."),
                }),
                1,
                22,
            ),
            a.tool("tool.started", "c4", json!({"command": "sed -i s/Teh/The/ README.md"}), 2, 0),
            a.tool(
                "tool.completed",
                "c4",
                json!({
                    "toolKind": "file_change",
                    "name": "Edit",
                    "command": "README.md",
                    "status": "completed",
                    "patch": {"text": DIFF, "added": 1, "removed": 1, "truncated": false},
                }),
                2,
                3,
            ),
            a.tool(
                "tool.completed",
                "c5",
                json!({
                    "toolKind": "file_change",
                    "name": "Write",
                    "command": "data/big.txt",
                    "status": "completed",
                    "patch": {
                        "text": "--- a/data/big.txt\n+++ b/data/big.txt\n@@ -1,3 +1,3 @@\n-aaa\n+bbb\n ccc",
                        "added": 61000,
                        "removed": 60000,
                        "truncated": true,
                    },
                }),
                2,
                10,
            ),
            a.tool(
                "tool.completed",
                "c6",
                json!({
                    "toolKind": "tool_use",
                    "name": "Write",
                    "command": "media/clip.bin",
                    "status": "result too large to keep",
                    "output": "The result was too large to keep (900 KB). The teammate read it; Locust keeps results up to 256 KB.",
                }),
                2,
                20,
            ),
            a.said("Ran the tests, fixed the typo, and left build/ alone as asked.", 3, "msg_2"),
            a.completed(3, Some("gpt-test-2026-10")),
        ];
        let m1 = json!({
            "metadata": one,
            "events": events_one,
            "hostFailures": [],
            "checkpoints": [{
                "epoch": 1,
                "reason": "route-switch",
                "resumeSafety": "safe",
                "unsettledActions": [],
                "createdAt": iso(30, 0),
            }],
            "peerLinks": [],
            "editChecks": [],
            "approvals": [],
            "schemaVersion": 22,
            "phase": "completed",
            "lastUpdatedAt": iso(30, 0),
            "ledgerSequence": 16,
            "issues": [],
        });

        let mut two = make_metadata(
            "mission_two",
            "Brief the host wrote: the earlier turns, summarised.\n\nThe person now asks:\n\nCan you review that change from a fresh pair of eyes?",
            "claude",
            iso(10, 0),
        );
        two["mode"] = json!("plan");
        two["continuesFrom"] = json!({
            "missionId": "mission_one",
            "checkpointEpoch": 1,
            "reason": "route-switch",
            "leftOut": ["earlier"],
            "leftOutByYou": ["summary"],
        });
        let mut b = Events::new(&two);
        let events_two = vec![
            b.started(10),
            b.tool(
                "tool.failed",
                "r1",
                json!({
                    "toolKind": "tool_use",
                    "name": "Bash",
                    "command": "npm install left-pad",
                    "status": "refused",
                    "output": "Plan mode does not run commands.",
                }),
                11,
                0,
            ),
            b.said("I could not install anything in Plan mode, but the change reads well.", 11, "msg_1"),
            b.failed("The connection dropped while the answer was being written.", 12),
        ];
        let m2 = json!({
            "metadata": two,
            "events": events_two,
            "hostFailures": [],
            "checkpoints": [],
            "peerLinks": [{
                "direction": "posted",
                "messageId": "msg_peer_1",
                "peerTeammateId": "tm_booty",
                "occurredAt": iso(11, 30),
            }],
            "editChecks": [{
                "command": "npm test",
                "outcome": "passed",
                "newLines": ["Tests 12 passed"],
                "unchanged": false,
                "first": true,
                "occurredAt": iso(13, 0),
            }],
            "approvals": [],
            "schemaVersion": 22,
            "phase": "failed",
            "lastUpdatedAt": iso(13, 0),
            "ledgerSequence": 8,
            "issues": [],
        });

        let mut three = make_metadata(
            "mission_three",
            "Thanks. Now say it in one sentence.",
            "claude",
            iso(20, 0),
        );
        three["mode"] = json!("ask");
        three["continuesFrom"] = json!({
            "missionId": "mission_two",
            "checkpointEpoch": 1,
            "reason": "follow-up",
        });
        let mut c = Events::new(&three);
        let events_three = vec![
            c.started(20),
            c.said("The typo is fixed and the tests pass.", 20, "msg_1"),
            c.completed(21, None),
        ];
        let issues_three = if tear_turn_three {
            json!([{
                "code": "truncated-tail",
                "missionId": "mission_three",
                "message": "An incomplete final ledger record was ignored after recovery.",
            }])
        } else {
            json!([])
        };
        let m3 = json!({
            "metadata": three,
            "events": events_three,
            "hostFailures": [],
            "checkpoints": [],
            "peerLinks": [],
            "editChecks": [],
            "approvals": [],
            "schemaVersion": 22,
            "phase": "completed",
            "lastUpdatedAt": iso(21, 50),
            "ledgerSequence": 4,
            "issues": issues_three,
        });

        vec![m1, m2, m3]
    }

    fn record(turns: &[Value]) -> String {
        mission_record_markdown(&RecordSource {
            missions: turns,
            teammate: Some("Wren"),
            folder: Some("C:\\work\\locust"),
            locust_version: "0.575.0",
            saved_at: SAVED_AT,
            missing_parent: None,
        })
    }

    fn before20(turns: &[Value]) -> Vec<Value> {
        turns
            .iter()
            .cloned()
            .map(|mut t| {
                t["schemaVersion"] = json!(19);
                t["approvals"] = json!([]);
                t
            })
            .collect()
    }

    #[test]
    fn golden_file_matches_byte_for_byte() {
        let turns = seed_turns(true);
        let markdown = record(&before20(&turns));
        assert_eq!(markdown, GOLDEN);
    }

    #[test]
    fn writing_twice_from_same_ledger_is_identical() {
        let turns = seed_turns(false);
        assert_eq!(record(&turns), record(&turns));
    }

    #[test]
    fn names_a_missing_parent_instead_of_skipping_it() {
        let turns = seed_turns(false);
        let markdown = mission_record_markdown(&RecordSource {
            missions: &turns[1..],
            teammate: Some("Wren"),
            folder: Some("C:\\work\\locust"),
            locust_version: "0.575.0",
            saved_at: SAVED_AT,
            missing_parent: Some("mission_one"),
        });
        assert!(markdown.contains("`mission_one`, is not in the ledger"));
        assert!(markdown.contains("record incomplete"));
    }

    #[test]
    fn says_nothing_to_record_when_no_turn_is_found() {
        let markdown = record(&[]);
        assert!(markdown.contains("holds no turn of this conversation"));
    }

    #[test]
    fn approvals_before_v20_lists_declined_without_claiming_click_or_rule() {
        let turns = seed_turns(false);
        let markdown = record(&before20(&turns));
        assert!(markdown.contains("- **Declined.** Asked: `rm -rf build`."));
        assert!(markdown.contains(
            "> Recorded with the call: The person declined this, and said: use the build script instead"
        ));
        assert!(markdown.contains(
            "It does not record which, and the words recorded with the call do not say."
        ));
    }

    #[test]
    fn approvals_lists_saved_rule_denial_and_quotes_sentence() {
        let turns = seed_turns(false);
        let markdown = record(&turns);
        assert!(markdown.contains(
            "- **Denied, and the words recorded with it name a saved rule.** Asked: `git push origin main`. The call did not run."
        ));
        assert!(markdown.contains(&format!(
            "{SAVED_RULE_DENIAL} never run git push for Wren in this folder."
        )));
    }

    #[test]
    fn approvals_lists_mode_refusal_with_no_person_answer_claimed() {
        let turns = seed_turns(false);
        let markdown = record(&turns);
        assert!(markdown.contains("- **Refused before it ran.** Asked: `npm install left-pad`."));
        assert!(markdown.contains("no answer from a person is recorded"));
    }

    #[test]
    fn approvals_before_v20_notes_allowed_approvals_missing_in_every_turn() {
        let turns = seed_turns(false);
        let markdown = record(&before20(&turns));
        let count = markdown
            .matches("_Not in the ledger:_ an approval that was allowed, what the card asked in full, who answered it and how.")
            .count();
        assert_eq!(count, 3);
        let lower = markdown.to_ascii_lowercase();
        assert!(!lower.contains("approved by"));
        assert!(!lower.contains("was approved"));
        assert!(!lower.contains("allowed by"));
    }

    #[test]
    fn approvals_before_v20_says_none_recorded_for_clean_turn() {
        let turns = seed_turns(false);
        assert!(record(&before20(&turns)).contains("No declined or refused call is recorded in this turn."));
    }

    fn turns_with_answered_cards() -> String {
        let mut turns = seed_turns(false);
        turns[0]["approvals"] = json!([
            {
                "approvalId": "ap_1",
                "kind": "command",
                "asked": "Run a command\nnpm test",
                "answer": "allowed",
                "by": "card",
                "askedAt": iso(0, 59),
                "occurredAt": iso(1, 0),
            },
            {
                "approvalId": "ap_2",
                "kind": "command",
                "asked": "Run a command\nrm -rf build",
                "answer": "denied",
                "by": "card",
                "words": "use the build script instead",
                "askedAt": iso(1, 10),
                "occurredAt": iso(1, 12),
            },
            {
                "approvalId": "ap_3",
                "kind": "command",
                "asked": "Run a command\ngit push origin main",
                "answer": "denied",
                "by": "saved-rule",
                "words": "never run git push for Wren in this folder.",
                "askedAt": iso(1, 20),
                "occurredAt": iso(1, 21),
            },
            {
                "approvalId": "ap_4",
                "kind": "command",
                "asked": "Run a command\nnpm run lint",
                "answer": "allowed",
                "by": "earlier-always",
                "words": "The Always given earlier in this run allows every command it runs.",
                "askedAt": iso(1, 30),
                "occurredAt": iso(1, 30),
            },
        ]);
        record(&turns)
    }

    #[test]
    fn cards_lists_each_card_answered() {
        let markdown = turns_with_answered_cards();
        assert!(markdown.contains(&format!(
            "- **Allowed** by the person, on the card · command · asked {} · answered {}",
            iso(0, 59),
            iso(1, 0)
        )));
        assert!(markdown.contains("> The card asked: Run a command\n> npm test"));
        assert!(markdown.contains("- **Denied** by the person, on the card · command"));
        assert!(markdown.contains("> Said with the answer: use the build script instead"));
        assert!(markdown.contains("- **Denied** by a rule the person saved, before the card reached them · command"));
        assert!(markdown.contains("- **Allowed** by the person\u{2019}s Always on an earlier card of this run, with no card of its own · command"));
        assert!(markdown.contains(
            "> Said with the answer: The Always given earlier in this run allows every command it runs."
        ));
    }

    #[test]
    fn cards_points_declined_call_at_answer_above() {
        let markdown = turns_with_answered_cards();
        assert!(markdown.contains(
            "- **Declined.** Asked: `rm -rf build`. The call did not run; the answer that declined it is listed above."
        ));
        assert!(!markdown.contains("It does not record which"));
    }

    #[test]
    fn cards_says_what_it_records_and_never_that_allowed_is_missing() {
        let markdown = turns_with_answered_cards();
        assert!(markdown.contains("_Recorded:_ every card answered on this turn, with who answered it."));
        assert!(!markdown.contains("_Not in the ledger:_ an approval that was allowed"));
    }

    #[test]
    fn cards_says_none_answered_rather_than_omitting_section() {
        let markdown = turns_with_answered_cards();
        assert!(markdown.contains(
            "No card was answered, and no call was declined or refused, in this turn."
        ));
    }

    #[test]
    fn cards_counts_unwritten_approval_note() {
        let mut turns = seed_turns(false);
        let mut ev = Events::new(&turns[2]["metadata"]);
        ev.n = 10;
        let note = ev.diagnostic(
            "host.approval-not-recorded",
            "The answer to a card (command, allowed) could not be written to this record: disk full",
            21,
            30,
        );
        turns[2]["events"].as_array_mut().unwrap().push(note);
        let markdown = record(&turns);
        let turn_three = &markdown[markdown.find("## Turn 3").unwrap()..];
        assert!(turn_three.contains(
            "_Recorded:_ every card answered on this turn should be here; 1 could not be written down, and the notes under \"How it ended\" say which."
        ));
        assert!(!turn_three.contains("with who answered it."));
        let before_three = &markdown[..markdown.find("## Turn 3").unwrap()];
        assert!(before_three.contains("_Recorded:_ every card answered on this turn, with who answered it."));
        assert!(markdown.contains(&format!(
            "Note Locust wrote into the record (`host.approval-not-recorded`, {}): The answer to a card (command, allowed) could not be written to this record: disk full",
            iso(21, 30)
        )));
    }

    #[test]
    fn commands_and_changes_formatting() {
        let turns = seed_turns(false);
        let markdown = record(&turns);
        assert!(markdown.contains("completed · exit code 0 · 4.2 s"));
        assert!(markdown.contains("```sh\nnpm test\n```"));
        assert!(markdown.contains("Output as recorded:\n\n```\nTests 12 passed\n```"));
        assert!(markdown.contains("Diff as recorded: +1 \u{2212}1"));
        assert!(markdown.contains("```diff\ndiff --git a/README.md b/README.md"));
        assert!(markdown.contains("_Too large to keep whole."));
        assert!(markdown.contains("the runtime reported +61000 \u{2212}60000 for the whole change"));
        assert!(markdown.contains("No diff is recorded. The ledger's own words: The result was too large to keep (900 KB)."));
    }

    #[test]
    fn missing_diff_never_becomes_empty_diff() {
        let mut turns = seed_turns(false);
        for ev in turns[0]["events"].as_array_mut().unwrap() {
            if ev["type"] == "tool.completed" && ev["payload"]["itemId"] == "c4" {
                ev["payload"].as_object_mut().unwrap().remove("patch");
            }
        }
        assert!(record(&turns[..1]).contains("_No diff is recorded for this change._"));
    }

    #[test]
    fn code_fence_in_command_cannot_close_outer_fence() {
        let mut turns = seed_turns(false);
        for ev in turns[0]["events"].as_array_mut().unwrap() {
            if (ev["type"] == "tool.started" || ev["type"] == "tool.completed")
                && ev["payload"]["itemId"] == "c1"
            {
                ev["payload"]["command"] = json!("echo \"```\"");
            }
        }
        assert!(record(&turns[..1]).contains("````sh\necho \"```\"\n````"));
    }

    #[test]
    fn who_answered_handoffs_and_endings() {
        let turns = seed_turns(false);
        let markdown = record(&turns);
        assert!(markdown.contains("- **Turn 1** · Codex CLI · model `gpt-test`, which the runtime named `gpt-test-2026-10` · mode approve-each · sandbox workspace-write · CLI 0.160.0"));
        assert!(markdown.contains("- \u{21aa} **Handed over** from Codex CLI to Claude Code."));
        assert!(markdown.contains("- **Turn 2** · Claude Code · model `sonnet` · mode plan"));
        assert!(markdown.contains("- **Turn 3** · Claude Code · model `sonnet` · mode ask"));
        assert_eq!(markdown.matches('\u{21aa}').count(), 1);
        assert!(markdown.contains("- **Handed over** from Codex CLI, model `gpt-test`, which the runtime named `gpt-test-2026-10`, to Claude Code, model `sonnet`, at"));
        assert!(markdown.contains("Resumed from checkpoint 1 (route-switch)"));
        assert!(markdown.contains("The hand-over brief left out, to fit: earlier."));
        assert!(markdown.contains("The person chose to leave out of the brief: summary."));
        assert!(!markdown.contains("Brief the host wrote"));
        assert!(markdown.contains("> Can you review that change from a fresh pair of eyes?"));
        assert!(markdown.contains("**Posted a message to another teammate** (`tm_booty`)"));
        assert!(markdown.contains("The message is held in the workroom, not in this ledger (message `msg_peer_1`)."));
        assert!(markdown.contains("**Finished.** The runtime reported the run complete"));
        assert!(markdown.contains("**Failed** at 2026-10-02T09:12:50.000Z (process-failed)."));
        assert!(markdown.contains("> The connection dropped while the answer was being written."));
        assert!(markdown.contains("The person's check, `npm test`, after this turn: **passed**"));
    }

    #[test]
    fn missing_mode_is_not_recorded() {
        let mut turns = seed_turns(false);
        turns[0]["metadata"].as_object_mut().unwrap().remove("mode");
        assert!(record(&turns[..1]).contains("mode not recorded (this turn predates it)"));
    }

    #[test]
    fn interrupted_turn_has_no_ending_recorded() {
        let mut turns = seed_turns(false);
        turns[0]["phase"] = json!("interrupted");
        turns[0]["events"] = json!(
            turns[0]["events"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| e["type"] != "run.completed")
                .cloned()
                .collect::<Vec<_>>()
        );
        let markdown = record(&turns[..1]);
        assert!(markdown.contains("**No ending is recorded.**"));
        assert!(!markdown.contains("**Finished.**"));
    }

    #[test]
    fn host_written_relay_prompt_is_not_presented_as_persons_words() {
        let mut turns = seed_turns(false);
        turns[0]["metadata"]["startedBy"] = json!({"kind": "relay", "hop": 2});
        let markdown = record(&turns[..1]);
        assert!(markdown.contains(
            "Nobody typed this turn: Locust started it so Wren could answer another teammate (automatic turn 2)."
        ));
        assert!(!markdown.contains("### The person said"));
    }

    #[test]
    fn handoff_with_no_instruction_quotes_nothing_as_persons_words() {
        let mut turns = seed_turns(false);
        turns[1]["metadata"]["prompt"] = json!("Brief the host wrote with no instruction in it.");
        let markdown = record(&turns[..2]);
        assert!(markdown.contains(
            "Nobody typed this turn. Locust carried the work to another runtime on its own"
        ));
        assert!(!markdown.contains("Brief the host wrote with no instruction"));
    }

    #[test]
    fn edited_turn_is_marked_as_edit() {
        let mut turns = seed_turns(false);
        turns[2]["metadata"]["continuesFrom"]["edited"] = json!(true);
        assert!(record(&turns).contains("This turn is an edit of an earlier message"));
    }

    #[test]
    fn self_check_and_header_fields() {
        let clean = record(&seed_turns(false));
        assert!(clean.contains("- **The record's own check:** **record readable.**"));
        assert!(clean.contains("This is not tamper evidence"));
        assert!(!clean.contains("record incomplete"));
        assert!(clean.contains("# Record of a conversation with Wren"));
        assert!(clean.contains("- **Folder:** C:\\work\\locust (where the teammate works now; the ledger holds only the workspace id `ws_test`)"));
        assert!(clean.contains("- **Started:** 2026-10-02T09:00:00.000Z"));
        assert!(clean.contains("- **Locust version:** 0.575.0, the build that wrote this file. The ledger does not record which build ran each turn."));
        assert!(!clean.contains("Read it before you send it"));

        let torn = record(&seed_turns(true));
        assert!(torn.contains("**record incomplete.** 1 problem was found reading this conversation's ledger"));
        assert!(torn.contains("`truncated-tail`"));
        assert!(torn.contains("(mission_three)"));
        assert!(!torn.contains("**record readable.**"));

        let unnamed = mission_record_markdown(&RecordSource {
            missions: &seed_turns(false),
            teammate: None,
            folder: None,
            locust_version: "0.575.0",
            saved_at: SAVED_AT,
            missing_parent: None,
        });
        assert!(unnamed.contains("- **Teammate:** not recorded: the roster no longer names them"));
        assert!(unnamed.contains("- **Folder:** not recorded. The ledger holds only the workspace id `ws_test`."));
    }

    #[test]
    fn quotes_headings_in_reply_so_outline_stays_clean() {
        let mut turns = seed_turns(false);
        for ev in turns[2]["events"].as_array_mut().unwrap() {
            if ev["type"] == "message.delta" {
                ev["payload"]["text"] = json!("# A heading\n\n- a list");
            }
        }
        let markdown = record(&turns[2..]);
        assert!(markdown.contains("> # A heading\n>\n> - a list"));
        assert!(!markdown.lines().any(|l| l == "# A heading"));
    }

    #[test]
    fn raw_record_holds_events_and_metadata_as_recorded() {
        let turns = seed_turns(false);
        let raw = raw_record_json(&turns, "0.575.0", SAVED_AT);
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed["writtenBy"], "Locust 0.575.0");
        let parsed_missions = parsed["missions"].as_array().unwrap();
        assert_eq!(parsed_missions.len(), turns.len());
        for (got, expected) in parsed_missions.iter().zip(&turns) {
            assert_eq!(got["events"], expected["events"]);
            assert_eq!(got["metadata"], expected["metadata"]);
        }
    }

    #[test]
    fn file_name_formats_and_sanitizes() {
        assert_eq!(
            record_file_name(Some("Wren"), NaiveDate::from_ymd_opt(2026, 10, 3).unwrap()),
            "Locust record - Wren - 2026-10-03.md"
        );
        assert_eq!(
            record_file_name(Some("Wr:en/<Code>?"), NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()),
            "Locust record - WrenCode - 2026-01-05.md"
        );
        assert_eq!(
            record_file_name(None, NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()),
            "Locust record - conversation - 2026-01-05.md"
        );
        assert_eq!(
            record_file_name(Some("***"), NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()),
            "Locust record - conversation - 2026-01-05.md"
        );
    }

    // ---- a-saved-record-keeps-keys-out.test.ts ----

    fn turns_holding(command_output: &str, diff: &str) -> Vec<Value> {
        let meta = make_metadata(
            "mission_keys",
            "Rotate the deploy key and tell me what you ran.",
            "codex",
            iso(0, 0),
        );
        let mut ev = Events::new(&meta);
        let events = vec![
            ev.started(0),
            ev.tool("tool.started", "c1", json!({"command": "cat .env"}), 1, 0),
            ev.tool(
                "tool.completed",
                "c1",
                json!({"command": "cat .env", "output": command_output, "exitCode": 0, "status": "completed", "durationMs": 10}),
                1,
                2,
            ),
            ev.tool(
                "tool.completed",
                "c2",
                json!({
                    "toolKind": "file_change",
                    "name": "Edit",
                    "command": "deploy.yml",
                    "status": "completed",
                    "patch": {"text": diff, "added": 1, "removed": 1, "truncated": false},
                }),
                1,
                3,
            ),
            ev.completed(1, None),
        ];
        vec![json!({
            "metadata": meta,
            "events": events,
            "hostFailures": [],
            "checkpoints": [],
            "peerLinks": [],
            "editChecks": [],
            "approvals": [],
            "schemaVersion": 22,
            "phase": "completed",
            "lastUpdatedAt": iso(1, 50),
            "ledgerSequence": 6,
            "issues": [],
        })]
    }

    const PLAIN_DIFF: &str = "--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b";

    #[test]
    fn keeps_keys_out_replaces_command_output_and_diff_and_counts_them() {
        let anthropic_key = format!("{}-ant-abcdefghijklmnopqrst", "sk");
        let github_token = format!("{}_abcdefghijklmnopqrst", "ghp");
        let diff_with_token = format!("--- a/deploy.yml\n+++ b/deploy.yml\n@@ -1 +1 @@\n-token: old\n+token: {github_token}");
        let turns = turns_holding(&format!("ANTHROPIC_API_KEY={anthropic_key}\nDONE"), &diff_with_token);
        let markdown = mission_record_markdown(&RecordSource {
            missions: &turns,
            teammate: Some("Wren"),
            folder: Some("C:\\work\\locust"),
            locust_version: "0.590.0",
            saved_at: SAVED_AT,
            missing_parent: None,
        });
        assert!(!markdown.contains(&anthropic_key));
        assert!(!markdown.contains(&github_token));
        assert_eq!(markdown.matches(SECRET_SHAPED_TEXT_REMOVED).count(), 2);
        assert!(markdown.contains("- **Secret-shaped text:** 2 pieces replaced in this file. The raw record keeps the original."));
    }

    #[test]
    fn keeps_keys_out_uses_singular_piece_when_one_replaced() {
        let anthropic_key = format!("{}-ant-abcdefghijklmnopqrst", "sk");
        let turns = turns_holding(&format!("token {anthropic_key}"), PLAIN_DIFF);
        let markdown = mission_record_markdown(&RecordSource {
            missions: &turns,
            teammate: Some("Wren"),
            folder: Some("C:\\work\\locust"),
            locust_version: "0.590.0",
            saved_at: SAVED_AT,
            missing_parent: None,
        });
        assert!(markdown.contains("- **Secret-shaped text:** 1 piece replaced in this file. The raw record keeps the original."));
    }

    #[test]
    fn keeps_keys_out_raw_record_keeps_originals_and_warns_first() {
        let anthropic_key = format!("{}-ant-abcdefghijklmnopqrst", "sk");
        let github_token = format!("{}_abcdefghijklmnopqrst", "ghp");
        let diff_with_token = format!("--- a/deploy.yml\n+++ b/deploy.yml\n@@ -1 +1 @@\n-token: old\n+token: {github_token}");
        let turns = turns_holding(&format!("ANTHROPIC_API_KEY={anthropic_key}"), &diff_with_token);
        let raw = raw_record_json(&turns, "0.590.0", SAVED_AT);
        assert!(raw.contains(&anthropic_key));
        assert!(raw.contains(&github_token));
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        let first_key = parsed.as_object().unwrap().keys().next().unwrap();
        assert_eq!(first_key, "warning");
        assert_eq!(
            parsed["warning"],
            "This file is the ledger as recorded, including any secret-shaped text the Markdown replaced."
        );
    }

    #[test]
    fn keeps_keys_out_says_none_found_when_clean() {
        let turns = turns_holding("Tests 12 passed", PLAIN_DIFF);
        let markdown = mission_record_markdown(&RecordSource {
            missions: &turns,
            teammate: Some("Wren"),
            folder: Some("C:\\work\\locust"),
            locust_version: "0.590.0",
            saved_at: SAVED_AT,
            missing_parent: None,
        });
        assert!(markdown.contains("- **Secret-shaped text:** none found."));
        assert!(!markdown.contains(SECRET_SHAPED_TEXT_REMOVED));
    }

    #[test]
    fn keeps_keys_out_replaces_keys_in_card_answer_and_edit_check() {
        let anthropic_key = format!("{}-ant-abcdefghijklmnopqrst", "sk");
        let github_token = format!("{}_abcdefghijklmnopqrst", "ghp");
        let mut turns = turns_holding("ok", PLAIN_DIFF);
        turns[0]["approvals"] = json!([{
            "kind": "command",
            "asked": "cat .env",
            "askedAt": iso(1, 0),
            "occurredAt": iso(2, 0),
            "answer": "denied",
            "by": "person",
            "words": format!("use {github_token} instead"),
        }]);
        turns[0]["editChecks"] = json!([{
            "command": "npm test",
            "outcome": "failed",
            "newLines": [format!("auth {anthropic_key}")],
            "unchanged": false,
            "first": true,
            "occurredAt": iso(3, 0),
        }]);
        let markdown = mission_record_markdown(&RecordSource {
            missions: &turns,
            teammate: Some("Wren"),
            folder: Some("C:\\work\\locust"),
            locust_version: "0.590.0",
            saved_at: SAVED_AT,
            missing_parent: None,
        });
        assert!(!markdown.contains(&github_token));
        assert!(!markdown.contains(&anthropic_key));
        assert!(markdown.contains("- **Secret-shaped text:** 2 pieces replaced in this file."));
    }

    // ---- a-saved-record-keeps-output-as-recorded.test.ts ----

    #[test]
    fn blank_lines_squeezed_between_sections_and_kept_inside_fence() {
        let text = ["# Turn 1", "", "", "", "Ran a command.", "```", "line one", "", "", "line four", "```", "", "", "Done."].join("\n");
        let expected = ["# Turn 1", "", "Ran a command.", "```", "line one", "", "", "line four", "```", "", "Done."].join("\n");
        assert_eq!(collapse_outside_fences(&text), expected);
    }

    #[test]
    fn longer_fence_reads_as_one_fence_whatever_it_holds() {
        let text = ["````", "```", "", "", "```", "````", "", "", "after"].join("\n");
        let expected = ["````", "```", "", "", "```", "````", "", "after"].join("\n");
        assert_eq!(collapse_outside_fences(&text), expected);
    }
}

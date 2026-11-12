//! Claude Code stream-json -> ledger events. Ports createClaudeEventNormalizer (claude-events.ts).
use super::Event;
use crate::runtime::events::text;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn identity(v: &Value, key: &str) -> String {
    s(v, key).replace('\0', "").chars().take(512).collect()
}
fn obj(v: &Value) -> bool {
    v.is_object()
}
fn from_subagent(v: &Value) -> bool {
    !s(v, "parent_tool_use_id").is_empty()
}

const INIT_KEYS: &[&str] = &[
    "session_id",
    "model",
    "permissionMode",
    "cwd",
    "claude_code_version",
    "output_style",
    "apiKeySource",
];
const CLAUDE_SIGNED_OUT: &str = "Claude Code is not signed in. Open a terminal, run claude, and sign in: with your Claude account, or with an Anthropic Console account to pay with an API key. Then send again. Locust never hands an API key from your environment to an agent; to use a Claude API key directly, add it in Settings, Your own models.";

pub fn summarize_init(value: &Value) -> Value {
    let mut summary = json!({});
    for key in INIT_KEYS {
        match &value[*key] {
            Value::String(s) => {
                summary[*key] = json!(s.replace('\0', "").chars().take(512).collect::<String>())
            }
            Value::Number(n) => summary[*key] = json!(n),
            Value::Bool(b) => summary[*key] = json!(b),
            _ => {}
        }
    }
    for (key, label) in [
        ("tools", "toolCount"),
        ("mcp_servers", "mcpServerCount"),
        ("slash_commands", "slashCommandCount"),
        ("agents", "agentCount"),
        ("skills", "skillCount"),
    ] {
        if let Some(arr) = value[key].as_array() {
            summary[label] = json!(arr.len());
        }
    }
    summary
}

pub fn limit_kind_for(status: &str) -> Option<&'static str> {
    let lower = status.to_ascii_lowercase();
    if lower == "allowed" || lower.is_empty() {
        None
    } else if lower.contains("allowed") {
        Some("temporary-rate-limit")
    } else {
        Some("quota-exhausted")
    }
}

pub fn resets_at_iso(value: &Value) -> Option<String> {
    let secs = value.as_f64().filter(|n| n.is_finite())?;
    let ms = secs * 1000.0;
    if !ms.is_finite() {
        return None;
    }
    chrono::DateTime::from_timestamp_millis(ms as i64)
        .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

fn window_name(key: &str) -> String {
    key.replace("five_hour", "5-hour")
        .replace("seven_day", "7-day")
        .replace('_', " ")
}

pub fn limit_sentence(info: &Value) -> String {
    let key = s(info, "rateLimitType");
    let windows = &info["unifiedWindows"];
    let own = if key.is_empty() {
        &Value::Null
    } else {
        &windows[key]
    };
    let utilization = info["utilization"]
        .as_f64()
        .or_else(|| own["utilization"].as_f64());
    let percent = utilization
        .filter(|n| n.is_finite())
        .map(|n| (n * 100.0).round() as i64);
    let when = resets_at_iso(&info["resetsAt"])
        .map(|r| format!(" · resets {r}"))
        .unwrap_or_default();
    let window = format!(
        "{} window",
        window_name(if key.is_empty() { "usage" } else { key })
    );
    if limit_kind_for(s(info, "status")) == Some("temporary-rate-limit") {
        match percent {
            None => format!("Your {window} is running low{when}"),
            Some(p) => format!("You've used {p}% of your {window}{when}"),
        }
    } else {
        match percent {
            None => format!("{window}{when}"),
            Some(p) => format!("{window} {p}% used{when}"),
        }
    }
}

pub fn usage_window_text(windows: &Value) -> Option<String> {
    let obj = windows.as_object()?;
    let mut parts: Vec<(i64, String)> = Vec::new();
    for (key, value) in obj {
        let used = value["utilization"].as_f64().filter(|n| n.is_finite())?;
        let used = (used * 100.0).round() as i64;
        let label = match key.as_str() {
            "five_hour" => "5-hour window".into(),
            "seven_day" => "7-day window".into(),
            other => format!("{other} window"),
        };
        let resets = resets_at_iso(&value["resetsAt"])
            .map(|r| format!(" · resets {r}"))
            .unwrap_or_default();
        parts.push((used, format!("{label} {used}% used{resets}")));
    }
    if parts.is_empty() {
        return None;
    }
    parts.sort_by_key(|a| std::cmp::Reverse(a.0));
    Some(
        parts
            .into_iter()
            .map(|(_, s)| s)
            .collect::<Vec<_>>()
            .join(" · "),
    )
}

pub fn own_window_text(info: &Value) -> Option<String> {
    let key = s(info, "rateLimitType");
    let utilization = info["utilization"].as_f64().filter(|n| n.is_finite())?;
    if key.is_empty() {
        return None;
    }
    let resets = resets_at_iso(&info["resetsAt"])
        .map(|r| format!(" · resets {r}"))
        .unwrap_or_default();
    Some(format!(
        "{} window {}% used{resets}",
        window_name(key),
        (utilization * 100.0).round() as i64
    ))
}

pub fn claude_tool_target(name: &str, input: &Value) -> Option<String> {
    if !obj(input) {
        return None;
    }
    let first = |keys: &[&str]| {
        keys.iter().find_map(|k| {
            input[*k]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        })
    };
    match name {
        "Bash" => first(&["command"]),
        "Glob" | "Grep" => first(&["pattern", "path"]),
        "Task" | "Agent" => first(&["description", "prompt"]),
        "WebFetch" | "WebSearch" => first(&["url", "query"]),
        "Skill" => first(&["skill"]),
        _ => first(&["file_path", "path", "notebook_path", "command", "pattern"]),
    }
}

pub fn claude_tool_title(name: &str, input: &Value) -> Option<String> {
    if name != "Bash" || !obj(input) {
        return None;
    }
    input["description"].as_str().map(str::to_owned)
}

/// claude-events.ts withoutThinkingEvidence: a thinking block or delta kept only as its type. None when nothing changed.
fn without_thinking_evidence(parsed: &Value) -> Option<Value> {
    let block = |v: &Value| -> Value {
        match v["type"].as_str() {
            Some(t @ ("thinking" | "thinking_delta" | "redacted_thinking" | "signature_delta")) if v.is_object() => json!({"type": t}),
            _ => v.clone(),
        }
    };
    if parsed["type"] == "stream_event" && parsed["event"].is_object() {
        let event = &parsed["event"];
        let has_block = event.get("content_block").is_some();
        let has_delta = event.get("delta").is_some();
        if has_block || has_delta {
            let mut out = parsed.clone();
            if has_block {
                out["event"]["content_block"] = block(&event["content_block"]);
            }
            if has_delta {
                out["event"]["delta"] = block(&event["delta"]);
            }
            return Some(out);
        }
    }
    if parsed["type"] == "assistant" && parsed["message"]["content"].is_array() {
        let mut out = parsed.clone();
        out["message"]["content"] = Value::Array(parsed["message"]["content"].as_array().into_iter().flatten().map(block).collect());
        return Some(out);
    }
    None
}

/// claude-events.ts CLAUDE_COMPACTED and CLAUDE_COMPACTED_ON_REQUEST.
pub const CLAUDE_COMPACTED: &str =
    "The conversation outgrew the model's context, so Claude Code summarized it and carried on from the summary.";
pub const CLAUDE_COMPACTED_ON_REQUEST: &str = "Claude Code summarized the conversation so far, as asked, and carries on from the summary.";

struct OpenTool {
    name: String,
    target: Option<String>,
    title: Option<String>,
    parent: Option<String>,
    /// A Bash call run in the background (`run_in_background`), which ends later than its result.
    background: bool,
}

/// claude-events.ts claudeToolBackgrounded.
fn claude_tool_backgrounded(name: &str, input: &Value) -> bool {
    name == "Bash" && input["run_in_background"] == true
}

/// claude-events.ts backgroundEnding: how background work ended, in the words its row uses. A stop after the run's own
/// result could only be the run ending.
pub fn background_ending(status: Option<&str>, after_result: bool) -> &'static str {
    match status {
        Some("completed") => "completed",
        Some("failed") => "failed",
        Some("stopped") | Some("killed") => {
            if after_result { "stopped-with-run" } else { "stopped" }
        }
        _ => "ended",
    }
}

/// claude-events.ts claudeShowsOutput: the tools whose printed output Claude Code shows under them.
fn claude_shows_output(name: Option<&str>) -> bool {
    match name {
        None => false,
        Some(n) if n.starts_with("mcp__") => true,
        Some(n) => matches!(n, "Bash" | "BashOutput" | "PowerShell" | "Grep" | "Glob" | "WebFetch" | "WebSearch" | "LS"),
    }
}

/// claude-events.ts claudeToolResultText: a tool_result's string content, or its text parts joined.
fn claude_tool_result_text(block: &Value) -> Option<String> {
    match &block["content"] {
        Value::String(t) => (!t.is_empty()).then(|| t.clone()),
        Value::Array(parts) => {
            let joined = parts
                .iter()
                .filter(|p| p["type"] == "text")
                .filter_map(|p| p["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n");
            (!joined.is_empty()).then_some(joined)
        }
        _ => None,
    }
}

/// claude-events.ts LOCUST_GUARD_SAID: how the command guard's refusals begin.
const LOCUST_GUARD_SAID: &str = "Locust stopped this command before it ran";

/// claude-events.ts claudeFilePatch: the change a file call made, from the record that answers it (`tool_use_result`):
/// its hunks written out, or, for a new or rewritten file, its text compared with what it was.
fn claude_file_patch(result: &Value) -> Option<Value> {
    let path = result["filePath"].as_str()?;
    let hunks: Vec<&Value> = result["structuredPatch"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|h| {
                    ["oldStart", "oldLines", "newStart", "newLines"].iter().all(|k| h[*k].as_f64().is_some_and(f64::is_finite))
                        && h["lines"].as_array().is_some_and(|l| l.iter().all(Value::is_string))
                })
                .collect()
        })
        .unwrap_or_default();
    let unified = if !hunks.is_empty() {
        let side = |prefix: &str| {
            if path.starts_with(['/', '\\']) || path.as_bytes().get(1) == Some(&b':') { path.to_owned() } else { format!("{prefix}{path}") }
        };
        let mut lines = vec![format!("--- {}", side("a/")), format!("+++ {}", side("b/"))];
        let n = |h: &Value, k: &str| super::events::num(h[k].as_f64().unwrap_or(0.0)).to_string();
        for h in hunks {
            lines.push(format!("@@ -{},{} +{},{} @@", n(h, "oldStart"), n(h, "oldLines"), n(h, "newStart"), n(h, "newLines")));
            lines.extend(h["lines"].as_array().into_iter().flatten().filter_map(|l| l.as_str().map(str::to_owned)));
        }
        lines.join("\n") + "\n"
    } else {
        let content = result["content"].as_str()?;
        let original = match &result["originalFile"] {
            Value::Null => None,
            Value::String(t) => Some(t.as_str()),
            _ => return None,
        };
        super::acp_events::unified_diff_text(path, original, content)?
    };
    super::cursor_events::tool_patch_from(&unified)
}

#[derive(Default)]
pub struct ClaudeNormalizer {
    pub thread: Option<String>,
    pub ended: bool,
    count: usize,
    open_tools: HashMap<String, OpenTool>,
    open_thinking: HashMap<i64, String>,
    text_block_id: Option<String>,
    streaming_message_id: Option<String>,
    message_ordinal: usize,
    saw_result: bool,
    terminal_failure: Option<String>,
    terminal_failure_kind: Option<String>,
    synthetic_error: Option<String>,
    completed_usage: Option<Value>,
    resolved_model: Option<String>,
    limits: HashSet<String>,
    /// Background work by task id, and the call that started it (claude-events.ts backgroundTasks).
    background_tasks: HashMap<String, Option<String>>,
    /// A helper's type and its summary, by the call that launched it, for that call's end.
    subagent_kinds: HashMap<String, String>,
    subagent_summaries: HashMap<String, String>,
    /// Calls Claude Code refused before they ran (`permission_denied`), and why, for their result that follows.
    refused_calls: HashMap<String, String>,
    /// The teammate's latest model call: its message id, prompt tokens and output tokens (claude-events.ts lastCall),
    /// so the run can say how full the conversation is.
    last_call: Option<(Option<String>, f64, f64)>,
    /// The run was on the subscription's usage windows, and whether it spilled into extra usage.
    on_usage_windows: bool,
    used_extra_usage: bool,
    /// Records the process gave, for `recordCount`.
    records: usize,
}

/// claude-events.ts promptTokensOf: input, cache-write and cache-read tokens together.
fn prompt_tokens_of(usage: &Value) -> Option<f64> {
    usage.as_object()?;
    let mut total = None;
    for key in ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"] {
        if let Some(n) = usage[key].as_f64().filter(|n| n.is_finite() && *n >= 0.0) {
            total = Some(total.unwrap_or(0.0) + n);
        }
    }
    total
}

/// claude-events.ts outputTokensOf.
fn output_tokens_of(usage: &Value) -> Option<f64> {
    usage["output_tokens"].as_f64().filter(|n| n.is_finite() && *n >= 0.0)
}

impl ClaudeNormalizer {
    fn event(&mut self, kind: &str, payload: Value) -> Event {
        self.count += 1;
        Event {
            kind: kind.into(),
            payload,
            thread: self.thread.clone(),
        }
    }

    fn process(&self, success: bool, cancelled: bool) -> Value {
        json!({
            "exitCode": if success { json!(0) } else { Value::Null },
            "signal": null,
            "stderr": "",
            "stderrTruncated": false,
            "recordCount": self.records,
            "inputDeliveryFailed": false,
            "outputLimitExceeded": false,
            "oversizedRecordsDropped": 0,
            "forcedTerminationAttempted": cancelled,
            "terminationUnconfirmed": false,
            "startedAt": crate::ledger_write::now(),
            "finishedAt": crate::ledger_write::now()
        })
    }

    pub fn diagnostic(&mut self, code: &str, message: &str) -> Event {
        self.event(
            "adapter.diagnostic",
            json!({
                "level": "warning",
                "code": code,
                "message": text(message, 16384),
                "terminal": false,
                "evidence": {"runtimeEventType":"claude","redacted":true}
            }),
        )
    }

    /// The turn's `result` record has arrived.
    pub fn saw_result(&self) -> bool {
        self.saw_result
    }
    pub fn finish(&mut self, cancelled: bool, message: &str) -> Event {
        self.ended = true;
        if cancelled {
            let mut p = json!({"process": self.process(false, true)});
            if let Some(t) = &self.thread {
                p["runtimeThreadId"] = json!(t);
            }
            return self.event("run.cancelled", p);
        }
        if let Some(failure) = self.terminal_failure.clone() {
            let mut p = json!({
                "kind": self.terminal_failure_kind.clone().unwrap_or_else(|| "unknown".into()),
                "message": failure,
                "runtimeTerminal": "failed",
                "process": self.process(false, false)
            });
            if let Some(t) = &self.thread {
                p["runtimeThreadId"] = json!(t);
            }
            return self.event("run.failed", p);
        }
        if !self.saw_result {
            let mut p = json!({
                "kind": "process-failed",
                "message": if message.is_empty() {
                    "Claude Code ended without a terminal result record.".into()
                } else {
                    text(message, 16384)
                },
                "runtimeTerminal": "missing",
                "process": self.process(false, false)
            });
            if let Some(t) = &self.thread {
                p["runtimeThreadId"] = json!(t);
            }
            return self.event("run.failed", p);
        }
        let mut p = json!({"process": self.process(true, false)});
        if let Some(t) = &self.thread {
            p["runtimeThreadId"] = json!(t);
        }
        if let Some(usage) = &self.completed_usage {
            let mut usage = usage.clone();
            if let Some((_, prompt, output)) = self.last_call {
                usage["contextTokens"] = super::events::num(prompt + output);
            }
            if self.on_usage_windows && !self.used_extra_usage {
                usage["billing"] = json!("subscription");
            }
            p["usage"] = usage;
        }
        if let Some(model) = &self.resolved_model {
            p["resolvedModel"] = json!(model);
        }
        self.event("run.completed", p)
    }

    pub fn accept(&mut self, parsed: &Value) -> Vec<Event> {
        if self.ended {
            return vec![];
        }
        self.records += 1;
        let type_name = s(parsed, "type");
        if type_name.is_empty() {
            return vec![];
        }
        // The record kept as evidence, with thinking cut to its type (claude-events.ts withoutThinkingEvidence).
        let safe = without_thinking_evidence(parsed);
        let (raw, changed) = super::events::evidence_raw(safe.as_ref().unwrap_or(parsed));
        let evidence = json!({"runtimeEventType": type_name, "raw": raw, "redacted": changed || safe.is_some()});

        if type_name == "tool_progress" {
            return vec![];
        }

        if type_name == "control_request" {
            // The approval channel answers on the wire; no ledger diagnostic here.
            return vec![];
        }

        if type_name == "rate_limit_event" {
            let info = if obj(&parsed["rate_limit_info"]) {
                &parsed["rate_limit_info"]
            } else {
                &Value::Null
            };
            let mut out = Vec::new();
            // Emit limit then reading in that order (0.413 sequence counting).
            if info["unifiedWindows"].as_object().is_some_and(|w| !w.is_empty()) {
                self.on_usage_windows = true;
            }
            if info["isUsingOverage"] == true {
                self.used_extra_usage = true;
            }
            if let Some(kind) = limit_kind_for(s(info, "status")) {
                let key = format!("{kind}:{}", limit_sentence(info));
                if self.limits.insert(key) {
                    out.push(self.event(
                        "route.limit_detected",
                        json!({
                            "kind": kind,
                            "message": text(&limit_sentence(info), 16384),
                            "evidence": evidence
                        }),
                    ));
                }
            }
            if let Some(window) =
                usage_window_text(&info["unifiedWindows"]).or_else(|| own_window_text(info))
            {
                out.push(self.event(
                    "adapter.diagnostic",
                    json!({
                        "code": "claude.usage_window",
                        "level": "info",
                        "terminal": false,
                        "message": text(&window, 16384),
                        "evidence": evidence
                    }),
                ));
            }
            return out;
        }

        if type_name == "system" {
            let subtype = s(parsed, "subtype");
            if subtype == "init" {
                self.thread = Some(identity(parsed, "session_id")).filter(|s| !s.is_empty());
                let summary = summarize_init(parsed);
                return vec![self.event(
                    "run.started",
                    json!({
                        "runtimeThreadId": self.thread.clone().unwrap_or_default(),
                        "evidence": {"runtimeEventType":"system","raw":summary,"redacted":true}
                    }),
                )];
            }
            if subtype == "task_started" || subtype == "task_progress" {
                let task_id = Some(identity(parsed, "task_id")).filter(|t| !t.is_empty()).unwrap_or_else(|| "task".into());
                let started_by = Some(identity(parsed, "tool_use_id")).filter(|t| !t.is_empty());
                let string_of = |k: &str| parsed[k].as_str().filter(|v| !v.is_empty()).map(str::to_owned);
                if parsed["is_backgrounded"] == true || self.background_tasks.contains_key(&task_id) {
                    let call_id = started_by.clone().or_else(|| self.background_tasks.get(&task_id).cloned().flatten());
                    self.background_tasks.insert(task_id.clone(), call_id.clone());
                    if let Some(id) = &call_id
                        && let Some(open) = self.open_tools.get_mut(id)
                    {
                        open.background = true;
                    }
                    if let (Some(id), Some(kind)) = (&call_id, string_of("subagent_type")) {
                        self.subagent_kinds.insert(id.clone(), kind);
                    }
                    let doing = [string_of("description"), string_of("last_tool_name").map(|t| format!("last tool {t}"))]
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>()
                        .join(" · ");
                    let mut p = json!({
                        "stepKind": "item",
                        "itemId": call_id.unwrap_or_else(|| format!("background:{task_id}")),
                        "itemType": "background",
                        "status": "running",
                        "evidence": evidence
                    });
                    if !doing.is_empty() {
                        p["message"] = json!(text(&doing, 16384));
                    }
                    return vec![self.event("step.started", p)];
                }
                let kind = string_of("subagent_type");
                let words: Vec<String> = [kind.clone(), string_of("description"), string_of("last_tool_name").map(|t| format!("last tool {t}"))]
                    .into_iter()
                    .flatten()
                    .collect();
                if let (Some(id), Some(k)) = (&started_by, &kind) {
                    self.subagent_kinds.insert(id.clone(), k.clone());
                }
                let mut p = json!({"stepKind":"item","itemId":format!("subagent:{task_id}"),"itemType":"subagent","evidence":evidence});
                if !words.is_empty() {
                    p["message"] = json!(text(&words.join(" · "), 16384));
                }
                return vec![self.event("step.started", p)];
            }
            if subtype == "task_notification" {
                let task_id = Some(identity(parsed, "task_id")).filter(|t| !t.is_empty()).unwrap_or_else(|| "task".into());
                let tool_use_id = Some(identity(parsed, "tool_use_id")).filter(|t| !t.is_empty());
                let summary = parsed["summary"].as_str().filter(|v| !v.is_empty()).map(str::to_owned);
                let status = parsed["status"].as_str().filter(|v| !v.is_empty());
                if self.background_tasks.contains_key(&task_id) {
                    let call_id = tool_use_id.or_else(|| self.background_tasks.get(&task_id).cloned().flatten());
                    self.background_tasks.remove(&task_id);
                    let ended = background_ending(status, self.saw_result);
                    let mut p = json!({
                        "stepKind": "item",
                        "itemId": call_id.unwrap_or_else(|| format!("background:{task_id}")),
                        "itemType": "background",
                        "status": ended,
                        "evidence": evidence
                    });
                    if let Some(said) = &summary {
                        p["message"] = json!(text(said, 16384));
                    }
                    return vec![self.event(if ended == "failed" { "step.failed" } else { "step.completed" }, p)];
                }
                if let (Some(id), Some(said)) = (&tool_use_id, &summary) {
                    self.subagent_summaries.insert(id.clone(), said.clone());
                }
                let mut p = json!({"stepKind":"item","itemId":format!("subagent:{task_id}"),"itemType":"subagent","evidence":evidence});
                if let Some(st) = status {
                    p["status"] = json!(st);
                }
                return vec![self.event(if status == Some("failed") { "step.failed" } else { "step.completed" }, p)];
            }
            if subtype == "task_updated" || subtype == "thinking_tokens" {
                return vec![];
            }
            if subtype == "permission_denied" {
                let call_id = identity(parsed, "tool_use_id");
                let why = parsed["decision_reason"].as_str().or_else(|| parsed["message"].as_str()).unwrap_or("");
                if !call_id.is_empty() {
                    self.refused_calls.insert(call_id, why.to_owned());
                }
                return vec![];
            }
            if subtype == "background_tasks_changed" || subtype == "commands_changed" {
                return vec![];
            }
            if subtype == "compact_boundary" {
                let manual = parsed["compact_metadata"]["trigger"] == "manual";
                return vec![self.event(
                    "adapter.diagnostic",
                    json!({
                        "code": "claude.context_compacted",
                        "level": "info",
                        "terminal": false,
                        "message": if manual { CLAUDE_COMPACTED_ON_REQUEST } else { CLAUDE_COMPACTED },
                        "evidence": evidence
                    }),
                )];
            }
            if subtype == "notification" {
                let text_v = s(parsed, "text");
                if text_v.is_empty() {
                    return vec![];
                }
                let immediate = s(parsed, "priority") == "immediate";
                let cleaned = regex_replace_ctrl(text_v);
                return vec![self.event(
                    "adapter.diagnostic",
                    json!({
                        "code": "claude.notification",
                        "level": if immediate { "warning" } else { "info" },
                        "terminal": false,
                        "message": text(&cleaned, 16384),
                        "evidence": evidence
                    }),
                )];
            }
            // Any other system record is the turn moving on: its status, as a step (claude-events.ts).
            let mut p = json!({"stepKind":"turn","evidence":evidence});
            if !subtype.is_empty() {
                p["status"] = json!(subtype);
            }
            if let Some(st) = parsed["status"].as_str().filter(|v| !v.is_empty()) {
                p["message"] = json!(st);
            }
            return vec![self.event("step.started", p)];
        }

        if type_name == "stream_event" {
            let inner = if obj(&parsed["event"]) {
                &parsed["event"]
            } else {
                &Value::Null
            };
            let inner_type = s(inner, "type");
            if inner_type == "message_start" {
                let message = &inner["message"];
                if let Some(prompt) = prompt_tokens_of(&message["usage"])
                    && !from_subagent(parsed)
                {
                    let id = Some(identity(message, "id")).filter(|s| !s.is_empty());
                    self.last_call = Some((id, prompt, output_tokens_of(&message["usage"]).unwrap_or(0.0)));
                }
                if !from_subagent(parsed) {
                    self.message_ordinal += 1;
                    self.text_block_id = None;
                    self.streaming_message_id =
                        Some(identity(&inner["message"], "id")).filter(|s| !s.is_empty());
                }
                return vec![self.event(
                    "step.started",
                    json!({"stepKind":"turn","evidence":evidence}),
                )];
            }
            if inner_type == "message_delta" {
                // The call's final output count arrives here, at its end.
                if let Some(output) = output_tokens_of(&inner["usage"])
                    && !from_subagent(parsed)
                    && let Some(call) = self.last_call.as_mut()
                {
                    call.2 = call.2.max(output);
                }
                return vec![];
            }
            if inner_type == "content_block_start" {
                let block = &inner["content_block"];
                let block_type = s(block, "type");
                if (block_type == "thinking" || block_type == "redacted_thinking")
                    && !from_subagent(parsed)
                {
                    let index = inner["index"].as_i64().unwrap_or(0);
                    let item_id = format!("m{}_thinking_{}", self.message_ordinal, index);
                    self.open_thinking.insert(index, item_id.clone());
                    return vec![self.event(
                        "step.started",
                        json!({"stepKind":"reasoning","itemId":item_id,"evidence":evidence}),
                    )];
                }
                if block_type == "tool_use" {
                    let item_id = {
                        let id = identity(block, "id");
                        if id.is_empty() {
                            format!("block_{}", inner["index"].as_i64().unwrap_or(0))
                        } else {
                            id
                        }
                    };
                    let name = {
                        let n = identity(block, "name");
                        if n.is_empty() { "tool".into() } else { n }
                    };
                    let parent = if from_subagent(parsed) {
                        Some(identity(parsed, "parent_tool_use_id"))
                    } else {
                        None
                    };
                    self.open_tools.insert(
                        item_id.clone(),
                        OpenTool {
                            name: name.clone(),
                            target: None,
                            title: None,
                            parent: parent.clone(),
                            background: false,
                        },
                    );
                    let mut payload = json!({
                        "itemId": item_id,
                        "toolKind": "tool_use",
                        "name": name,
                        "phase": "started",
                        "evidence": evidence
                    });
                    if let Some(p) = parent {
                        payload["parentItemId"] = json!(p);
                    }
                    return vec![self.event("tool.started", payload)];
                }
                return vec![];
            }
            if inner_type == "content_block_delta" {
                // Thinking deltas are deliberately dropped (AGENTS.md: never keep hidden reasoning).
                let delta = &inner["delta"];
                let delta_type = s(delta, "type");
                if delta_type == "thinking_delta" || delta_type == "redacted_thinking_delta" {
                    return vec![];
                }
                let text_v = s(delta, "text");
                if text_v.is_empty() || from_subagent(parsed) {
                    return vec![];
                }
                let item_id = format!(
                    "m{}_block_{}",
                    self.message_ordinal,
                    inner["index"].as_i64().unwrap_or(0)
                );
                self.text_block_id = Some(item_id.clone());
                return vec![self.event(
                    "message.delta",
                    json!({
                        "itemId": item_id,
                        "operation": "append",
                        "text": text(text_v, 131072),
                        "final": false,
                        "evidence": evidence
                    }),
                )];
            }
            if inner_type == "content_block_stop" {
                let index = inner["index"].as_i64().unwrap_or(0);
                if let Some(item_id) = self.open_thinking.remove(&index) {
                    return vec![self.event(
                        "step.completed",
                        json!({"stepKind":"reasoning","itemId":item_id,"evidence":evidence}),
                    )];
                }
                return vec![];
            }
            if inner_type == "message_stop" {
                self.open_thinking.clear();
                return vec![self.event(
                    "step.completed",
                    json!({"stepKind":"turn","evidence":evidence}),
                )];
            }
            return vec![];
        }

        if type_name == "assistant" {
            let message = &parsed["message"];
            let content = message["content"].as_array().cloned().unwrap_or_default();
            if message["model"].as_str() == Some("<synthetic>")
                && parsed["error"].as_str().is_some()
            {
                self.synthetic_error = parsed["error"].as_str().map(str::to_owned);
                return vec![];
            }
            // The complete message states its call's usage too: the same call as the last `message_start` when the run
            // streams, the only word on it when it does not.
            if !from_subagent(parsed) {
                let id = Some(identity(message, "id")).filter(|s| !s.is_empty());
                let output = output_tokens_of(&message["usage"]);
                let prompt = prompt_tokens_of(&message["usage"]);
                match self.last_call.as_mut() {
                    Some(call) if id.is_some() && call.0 == id => {
                        if let Some(o) = output {
                            call.2 = call.2.max(o);
                        }
                    }
                    _ => {
                        if let Some(p) = prompt {
                            self.last_call = Some((id, p, output.unwrap_or(0.0)));
                        }
                    }
                }
            }
            // A helper's own calls are its row's children (claude-events.ts, helper visibility): this complete message is
            // the first and only word on each, so each opens here, named, under the Agent call that sent the helper.
            // Its words stay out; only its calls are kept.
            if from_subagent(parsed) {
                let parent = identity(parsed, "parent_tool_use_id");
                let mut opened = Vec::new();
                for block in &content {
                    if s(block, "type") != "tool_use" {
                        continue;
                    }
                    let item_id = identity(block, "id");
                    if item_id.is_empty() || self.open_tools.contains_key(&item_id) {
                        continue;
                    }
                    let name = Some(identity(block, "name")).filter(|n| !n.is_empty()).unwrap_or_else(|| "tool".into());
                    let target = claude_tool_target(&name, &block["input"]);
                    let title = claude_tool_title(&name, &block["input"]);
                    self.open_tools.insert(
                        item_id.clone(),
                        OpenTool { name: name.clone(), target: target.clone(), title: title.clone(), parent: Some(parent.clone()), background: false },
                    );
                    let mut p = json!({
                        "itemId": item_id,
                        "toolKind": "tool_use",
                        "name": name,
                        "parentItemId": parent,
                        "phase": "started",
                        "evidence": evidence
                    });
                    if let Some(t) = &target {
                        p["command"] = json!(text(t, 16384));
                    }
                    if let Some(t) = &title {
                        p["title"] = json!(text(t, 16384));
                    }
                    opened.push(self.event("tool.started", p));
                }
                return opened;
            }
            let mut restated = Vec::new();
            for block in &content {
                if s(block, "type") != "tool_use" {
                    continue;
                }
                let item_id = identity(block, "id");
                if item_id.is_empty() {
                    continue;
                }
                let Some(open) = self.open_tools.get(&item_id) else {
                    continue;
                };
                let target = claude_tool_target(&open.name, &block["input"]);
                let title = claude_tool_title(&open.name, &block["input"]);
                let background = claude_tool_backgrounded(&open.name, &block["input"]);
                // A helper's type is on its own call, so its row carries it however the launch and its notice are ordered.
                let helper_type = matches!(open.name.as_str(), "Agent" | "Task")
                    .then(|| block["input"]["subagent_type"].as_str().filter(|t| !t.is_empty()).map(str::to_owned))
                    .flatten();
                if let Some(kind) = &helper_type
                    && !self.subagent_kinds.contains_key(&item_id)
                {
                    self.subagent_kinds.insert(item_id.clone(), kind.clone());
                }
                if target.is_none() && title.is_none() && !background {
                    continue;
                }
                let first_naming = open.target.is_none() && open.title.is_none();
                if let Some(entry) = self.open_tools.get_mut(&item_id) {
                    if target.is_some() {
                        entry.target = target.clone();
                    }
                    if title.is_some() {
                        entry.title = title.clone();
                    }
                    entry.background |= background;
                }
                // Announced again only the first time the call can be named (claude-events.ts).
                if !first_naming {
                    continue;
                }
                let open = self
                    .open_tools
                    .get(&item_id)
                    // Invariant: tool existence checked before restating its input.
                    .expect("tool existence checked before restating its input");
                let mut payload = json!({
                    "itemId": item_id,
                    "toolKind": "tool_use",
                    "name": open.name,
                    "phase": "started",
                    "evidence": evidence
                });
                if let Some(t) = &target {
                    payload["command"] = json!(text(t, 16384));
                }
                if let Some(t) = &title {
                    payload["title"] = json!(text(t, 16384));
                }
                if background {
                    payload["background"] = json!(true);
                }
                if let Some(kind) = &helper_type {
                    payload["status"] = json!(kind);
                }
                if let Some(p) = &open.parent {
                    payload["parentItemId"] = json!(p);
                }
                restated.push(self.event("tool.started", payload));
            }
            let text_joined = content
                .iter()
                .filter_map(|b| b["text"].as_str())
                .collect::<Vec<_>>()
                .join("");
            if text_joined.is_empty() {
                return restated;
            }
            let message_id = identity(message, "id");
            let streamed_here = self.text_block_id.is_some()
                && (message_id.is_empty()
                    || self
                        .streaming_message_id
                        .as_ref()
                        .is_none_or(|sid| sid == &message_id));
            let item_id = if streamed_here {
                self.text_block_id
                    .clone()
                    // Invariant: streamed_here requires text_block_id.is_some().
                    .expect("streamed_here requires text_block_id.is_some()")
            } else if !message_id.is_empty() {
                format!("{message_id}:text")
            } else {
                format!("m{}:text", self.message_ordinal)
            };
            restated.push(self.event(
                "message.delta",
                json!({
                    "itemId": item_id,
                    "operation": "replace",
                    "text": text(&text_joined, 131072),
                    "final": true,
                    "evidence": evidence
                }),
            ));
            return restated;
        }

        if type_name == "user" {
            let content = parsed["message"]["content"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let mut events = Vec::new();
            for block in &content {
                if s(block, "type") != "tool_result" {
                    continue;
                }
                let item_id = identity(block, "tool_use_id");
                if item_id.is_empty() {
                    continue;
                }
                let open = self.open_tools.remove(&item_id);
                if open.is_none() && from_subagent(parsed) {
                    continue;
                }
                let failed = block["is_error"] == true;
                let mut refused_for = self.refused_calls.remove(&item_id);
                // Locust's own guard said no: refused, never run, in its words.
                if failed
                    && let Some(said) = claude_tool_result_text(block)
                    && let Some(at) = said.find(LOCUST_GUARD_SAID)
                {
                    refused_for = Some(said[at..].to_owned());
                }
                let subagent_kind = self.subagent_kinds.remove(&item_id);
                let subagent_summary = self.subagent_summaries.remove(&item_id);
                let name = open.as_ref().map(|o| o.name.clone()).unwrap_or_else(|| "tool".into());
                let mut payload = json!({
                    "itemId": item_id,
                    "toolKind": "tool_use",
                    "name": name,
                    "phase": "completed",
                    "evidence": evidence
                });
                if let Some(o) = &open {
                    if let Some(t) = &o.target {
                        payload["command"] = json!(text(t, 16384));
                    }
                    if let Some(t) = &o.title {
                        payload["title"] = json!(text(t, 16384));
                    }
                    if o.background {
                        payload["background"] = json!(true);
                    }
                    if let Some(p) = &o.parent {
                        payload["parentItemId"] = json!(p);
                    }
                }
                // Refused is not failed: it never ran, and the reason rides along.
                if let Some(why) = &refused_for {
                    payload["status"] = json!("refused");
                    if !why.is_empty() {
                        payload["output"] = json!(text(why, 16384));
                    }
                } else if failed {
                    payload["status"] = json!("error");
                } else if let Some(kind) = &subagent_kind {
                    payload["status"] = json!(kind);
                }
                // What a helper came back with, in its own words.
                if let Some(summary) = &subagent_summary {
                    payload["output"] = json!(text(summary, 16384));
                }
                if !failed
                    && refused_for.is_none()
                    && let Some(patch) = claude_file_patch(&parsed["tool_use_result"])
                {
                    payload["patch"] = patch;
                }
                if refused_for.is_none()
                    && subagent_summary.is_none()
                    && claude_shows_output(open.as_ref().map(|o| o.name.as_str()))
                    && let Some(printed) = claude_tool_result_text(block)
                {
                    payload["output"] = json!(text(&printed, 16384));
                }
                events.push(self.event(if failed { "tool.failed" } else { "tool.completed" }, payload));
            }
            return events;
        }

        if type_name == "result" {
            self.saw_result = true;
            let mut usage = json!({});
            if let Some(n) = parsed["total_cost_usd"].as_f64().filter(|n| n.is_finite()) {
                usage["usd"] = super::events::num(n);
            }
            if obj(&parsed["usage"]) {
                for (from, to) in [
                    ("input_tokens", "inputTokens"),
                    ("output_tokens", "outputTokens"),
                    ("cache_read_input_tokens", "cacheReadTokens"),
                    ("cache_creation_input_tokens", "cacheWriteTokens"),
                ] {
                    if let Some(n) = parsed["usage"][from].as_f64().filter(|n| n.is_finite()) {
                        usage[to] = super::events::num(n);
                    }
                }
            }
            if let Some(model_usage) = parsed["modelUsage"].as_object() {
                if model_usage.len() == 1 {
                    let (key, only) = model_usage
                        .iter()
                        .next()
                        // Invariant: model_usage length is exactly one in this branch.
                        .expect("model_usage length is exactly one in this branch");
                    let named = only["canonicalModel"]
                        .as_str()
                        .filter(|s| !s.is_empty())
                        .unwrap_or(key);
                    if !named.is_empty() {
                        self.resolved_model = Some(named.into());
                    }
                }
                let mut widest: Option<f64> = None;
                for entry in model_usage.values() {
                    if let Some(w) = entry["contextWindow"]
                        .as_f64()
                        .filter(|n| n.is_finite() && *n > 0.0)
                    {
                        widest = Some(widest.map_or(w, |c| c.max(w)));
                    }
                }
                if let Some(w) = widest {
                    usage["contextWindow"] = super::events::num(w);
                }
            }
            if usage.as_object().is_some_and(|o| !o.is_empty()) {
                self.completed_usage = Some(usage);
            }
            let is_error = parsed["is_error"] == true;
            let subtype = s(parsed, "subtype");
            let reason = {
                let t = s(parsed, "terminal_reason");
                if t.is_empty() { subtype } else { t }
            };
            if is_error || (!subtype.is_empty() && subtype != "success") {
                let said = s(parsed, "result");
                if self.synthetic_error.as_deref() == Some("authentication_failed")
                    || said.starts_with("Not logged in")
                    || said.contains("Please run /login")
                    || said.contains("Invalid API key")
                {
                    self.terminal_failure_kind = Some("authentication-failed".into());
                    self.terminal_failure = Some(CLAUDE_SIGNED_OUT.into());
                } else {
                    let msg = if said.is_empty() {
                        format!(
                            "Claude Code ended with {}.",
                            if reason.is_empty() {
                                "an error"
                            } else {
                                reason
                            }
                        )
                    } else {
                        said.into()
                    };
                    self.terminal_failure = Some(text(&msg, 16384));
                }
            }
            let denials = parsed["permission_denials"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            if denials.is_empty() {
                return vec![];
            }
            let listed: Vec<String> = denials
                .iter()
                .filter(|d| obj(d))
                .map(|d| {
                    let tool = s(d, "tool_name");
                    let tool = if tool.is_empty() { "a tool" } else { tool };
                    let input = &d["tool_input"];
                    let detail = input["command"]
                        .as_str()
                        .or(input["file_path"].as_str())
                        .or(input["description"].as_str());
                    match detail {
                        Some(d) => format!("{tool} `{d}`"),
                        None => tool.into(),
                    }
                })
                .collect();
            if listed.is_empty() {
                return vec![];
            }
            let message = if listed.len() == 1 {
                format!(
                    "Claude Code was not permitted to use {}, so it did not.",
                    listed[0]
                )
            } else {
                format!(
                    "Claude Code was not permitted to use {} tools, so it did not: {}.",
                    listed.len(),
                    listed.join("; ")
                )
            };
            return vec![self.diagnostic("claude.permission_denied", &message)];
        }

        vec![self.diagnostic(
            "claude.unknown_event",
            &format!("Unhandled Claude record: {type_name}"),
        )]
    }
}

fn regex_replace_ctrl(text_v: &str) -> String {
    let re = crate::invariants::regex(r"(?i)\s*·\s*ctrl\+o to see\s*$");
    re.replace(text_v, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_is_allow_listed_and_starts_run() {
        let mut n = ClaudeNormalizer::default();
        let events = n.accept(&json!({
            "type":"system","subtype":"init","session_id":"sess-1",
            "tools":["Read"],"mcp_servers":[{"name":"Gmail"}],
            "model":"claude-opus-5","permissionMode":"default",
            "powershell_path":"C:/Windows/powershell.exe"
        }));
        assert_eq!(events[0].kind, "run.started");
        let raw = events[0].payload["evidence"]["raw"].to_string();
        assert!(!raw.contains("Gmail") && !raw.contains("powershell"));
        assert_eq!(n.thread.as_deref(), Some("sess-1"));
    }

    #[test]
    fn deltas_append_and_assistant_replaces() {
        let mut n = ClaudeNormalizer::default();
        n.accept(&json!({"type":"system","subtype":"init","session_id":"s"}));
        let d = n.accept(&json!({
            "type":"stream_event",
            "event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hi"}}
        }));
        assert_eq!(d[0].payload["operation"], "append");
        let r = n.accept(&json!({
            "type":"assistant",
            "message":{"content":[{"type":"text","text":"Hi"}]}
        }));
        assert_eq!(r[0].payload["operation"], "replace");
        assert_eq!(r[0].payload["final"], true);
    }

    #[test]
    fn thinking_is_a_step_without_text() {
        let mut n = ClaudeNormalizer::default();
        n.accept(
            &json!({"type":"stream_event","event":{"type":"message_start","message":{"id":"m1"}}}),
        );
        let start = n.accept(&json!({
            "type":"stream_event",
            "event":{"type":"content_block_start","index":0,"content_block":{"type":"thinking"}}
        }));
        assert_eq!(start[0].kind, "step.started");
        assert_eq!(start[0].payload["stepKind"], "reasoning");
        let delta = n.accept(&json!({
            "type":"stream_event",
            "event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"SECRET_THOUGHT"}}
        }));
        assert!(delta.is_empty());
        let stop = n.accept(&json!({
            "type":"stream_event",
            "event":{"type":"content_block_stop","index":0}
        }));
        assert_eq!(stop[0].kind, "step.completed");
        let serialized = format!("{}{}", start[0].payload, stop[0].payload);
        assert!(!serialized.contains("SECRET_THOUGHT"));
    }

    #[test]
    fn tools_pair_and_limits_map() {
        let mut n = ClaudeNormalizer::default();
        n.accept(&json!({
            "type":"stream_event",
            "event":{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"toolu_1","name":"Read"}}
        }));
        n.accept(&json!({
            "type":"assistant",
            "message":{"content":[{"type":"tool_use","id":"toolu_1","name":"Read","input":{"file_path":"a.txt"}}]}
        }));
        let done = n.accept(&json!({
            "type":"user",
            "message":{"content":[{"type":"tool_result","tool_use_id":"toolu_1","is_error":false}]}
        }));
        assert_eq!(done[0].kind, "tool.completed");
        assert_eq!(done[0].payload["command"], "a.txt");
        assert_eq!(
            limit_kind_for("allowed_warning"),
            Some("temporary-rate-limit")
        );
        assert_eq!(limit_kind_for("allowed"), None);
        assert_eq!(limit_kind_for("rejected"), Some("quota-exhausted"));
    }

    #[test]
    fn result_and_cancel_finish() {
        let mut n = ClaudeNormalizer::default();
        n.accept(&json!({"type":"result","subtype":"success","is_error":false}));
        assert_eq!(n.finish(false, "").kind, "run.completed");
        let mut n = ClaudeNormalizer::default();
        assert_eq!(n.finish(true, "").kind, "run.cancelled");
        let mut n = ClaudeNormalizer::default();
        n.accept(&json!({"type":"result","subtype":"error_max_turns","is_error":true,"result":"too many"}));
        let failed = n.finish(false, "");
        assert_eq!(failed.kind, "run.failed");
        assert!(
            failed.payload["message"]
                .as_str()
                .unwrap()
                .contains("too many")
        );
    }
}

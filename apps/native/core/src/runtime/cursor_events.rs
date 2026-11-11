//! Cursor Agent stream-json -> ledger events. Ports createCursorEventNormalizer (cursor-events.ts).
use super::Event;
use crate::runtime::events::text;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

const INIT_KEYS: &[&str] = &[
    "session_id",
    "model",
    "permissionMode",
    "cwd",
    "apiKeySource",
];

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn identity(v: &Value, key: &str) -> String {
    s(v, key).replace('\0', "").chars().take(512).collect()
}
fn obj(v: &Value) -> bool {
    v.is_object()
}
fn string_opt(v: &Value) -> Option<String> {
    v.as_str().map(|s| s.to_owned()).filter(|s| !s.is_empty())
}

pub fn summarize_init(value: &Value) -> Value {
    let mut summary = json!({});
    if !obj(value) {
        return summary;
    }
    for key in INIT_KEYS {
        if let Value::String(held) = &value[*key] {
            summary[*key] = json!(text(held, 16384));
        }
    }
    summary
}

pub fn is_message_fragment(record: &Value) -> bool {
    record["timestamp_ms"].as_f64().is_some() && record.get("model_call_id").is_none()
}

/// `readToolCall` -> `read`; anything that does not follow the pattern is kept whole.
pub fn tool_kind(key: &str) -> String {
    let bytes = key.as_bytes();
    if let Some(rest) = key.strip_suffix("ToolCall")
        && rest.starts_with(|c: char| c.is_ascii_lowercase())
        && rest.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        let _ = bytes;
        return rest.to_owned();
    }
    key.to_owned()
}

/// Token counts only, as numbers.
fn sanitized_usage(value: &Value) -> Value {
    let mut counts = serde_json::Map::new();
    if let Some(obj) = value.as_object() {
        for (k, v) in obj {
            if let Some(n) = v.as_f64().filter(|n| n.is_finite()) {
                counts.insert(k.clone(), json!(n));
            }
        }
    }
    Value::Object(counts)
}

fn remember_todo(into: &mut HashMap<String, Value>, todo: &Value) {
    if !obj(todo) {
        return;
    }
    let content = string_opt(&todo["content"]).or_else(|| string_opt(&todo["text"]));
    let id = {
        let raw = identity(todo, "id");
        if raw.is_empty() {
            content.clone()
        } else {
            Some(raw)
        }
    };
    let (Some(id), Some(_)) = (id, content) else {
        return;
    };
    into.insert(id, todo.clone());
}

fn glob_target(args: &Value) -> Option<String> {
    let pattern = string_opt(&args["globPattern"])?;
    match string_opt(&args["targetDirectory"]) {
        None => Some(pattern),
        Some(dir) => Some(format!("{pattern} in {dir}")),
    }
}

/// What a completed tool call actually did.
pub fn tool_outcome(result: &Value) -> (bool, Option<String>, Option<i64>) {
    let Some(obj) = result.as_object() else {
        return (true, Some("unknown".into()), None);
    };
    let Some(key) = obj.keys().next().cloned() else {
        return (true, Some("unknown".into()), None);
    };
    if key != "success" {
        return (true, Some(key), None);
    }
    let body = match obj.get("success") {
        Some(v) if v.is_object() => &result["success"],
        _ => &Value::Null,
    };
    let exit = body["exitCode"]
        .as_f64()
        .filter(|n| n.is_finite())
        .map(|n| n as i64);
    match exit {
        Some(code) if code != 0 => (true, Some("exit".into()), Some(code)),
        Some(code) => (false, None, Some(code)),
        None => (false, None, None),
    }
}

fn assistant_text(parsed: &Value) -> String {
    let message = if obj(&parsed["message"]) {
        &parsed["message"]
    } else {
        &Value::Null
    };
    let content = message["content"].as_array().cloned().unwrap_or_default();
    content
        .iter()
        .filter_map(|block| {
            if obj(block) {
                block["text"].as_str().map(str::to_owned)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Cursor's provider refusal wording (provider-error.ts, cursor branch).
pub fn provider_error_sentence(message: &str) -> String {
    let lower = message.to_ascii_lowercase();
    let provider = "Cursor's provider";
    if (lower.contains("model") || lower.contains("model_not_found"))
        && (lower.contains("not enabled")
            || lower.contains("not found")
            || lower.contains("does not exist")
            || lower.contains("unknown model")
            || lower.contains("model_not_found"))
    {
        let reason = if lower.contains("not enabled") {
            "it is not enabled here right now"
        } else {
            "it could not be found or is not available here"
        };
        return format!(
            "{provider} refused the model: {reason}. Pick another model and send again."
        );
    }
    if lower.contains("insufficient_quota")
        || lower.contains("usage limit")
        || (lower.contains("quota") && (lower.contains("exhausted") || lower.contains("exceeded")))
    {
        return format!(
            "{provider} reported that its usage limit has been reached. Pick another model, or try again when usage is available."
        );
    }
    if lower.contains("rate") && lower.contains("limit") || lower.contains("too many requests") {
        return format!(
            "{provider} rate limited this request. Wait a while, or pick another model and send again."
        );
    }
    if lower.contains("overloaded")
        || lower.contains("service unavailable")
        || lower.contains("temporarily unavailable")
    {
        return format!(
            "{provider} is overloaded or unavailable right now. Try again later, or pick another model."
        );
    }
    if lower.contains("authentication")
        || lower.contains("unauthorized")
        || lower.contains("invalid api key")
    {
        return format!(
            "{provider} refused this request because of sign-in or access permissions. Check the agent's sign-in and model access, then send again."
        );
    }
    // Non-JSON / unrecognized: leave message unchanged (provider-error.ts).
    if message.trim().starts_with('{') {
        let cleaned = message.trim_end_matches(['.', '!', '?']);
        return format!(
            "{provider} refused this request: {cleaned}. Check the model and account settings before sending again."
        );
    }
    message.to_owned()
}

/// toolPatchFrom (codex-events.ts): counts from a unified diff, text bounded.
pub fn tool_patch_from(unified: &str) -> Option<Value> {
    if unified.is_empty() {
        return None;
    }
    let lines: Vec<&str> = unified.split('\n').collect();
    let mut added = 0usize;
    let mut removed = 0usize;
    let has_hunk = lines.iter().any(|l| l.starts_with("@@"));
    if !has_hunk {
        for line in &lines {
            if line.starts_with("+++") || line.starts_with("---") {
                continue;
            }
            if line.starts_with('+') {
                added += 1;
            } else if line.starts_with('-') {
                removed += 1;
            }
        }
    }
    let hunk_re = crate::invariants::regex(r"^@@ -\d+(?:,(\d+))? \+\d+(?:,(\d+))? @@");
    let mut old_left = 0usize;
    let mut new_left = 0usize;
    let mut open = false;
    let mut at = 0;
    while at < lines.len() {
        let line = lines[at];
        if old_left > 0 || new_left > 0 {
            if line.starts_with('+') && new_left > 0 {
                added += 1;
                new_left -= 1;
                at += 1;
                continue;
            }
            if line.starts_with('-') && old_left > 0 {
                removed += 1;
                old_left -= 1;
                at += 1;
                continue;
            }
            if line.starts_with(' ') && old_left > 0 && new_left > 0 {
                old_left -= 1;
                new_left -= 1;
                at += 1;
                continue;
            }
        }
        if let Some(m) = hunk_re.captures(line) {
            old_left = m
                .get(1)
                .map(|g| g.as_str().parse().unwrap_or(1))
                .unwrap_or(1);
            new_left = m
                .get(2)
                .map(|g| g.as_str().parse().unwrap_or(1))
                .unwrap_or(1);
            open = false;
            at += 1;
            continue;
        }
        if line.starts_with("@@") {
            old_left = 0;
            new_left = 0;
            open = true;
            at += 1;
            continue;
        }
        if line.starts_with("--- ") && lines.get(at + 1).is_some_and(|n| n.starts_with("+++ ")) {
            open = false;
            at += 2;
            continue;
        }
        if open {
            if line.starts_with('+') {
                added += 1;
            } else if line.starts_with('-') {
                removed += 1;
            }
        }
        at += 1;
    }
    let clean = unified.replace('\0', "");
    let truncated = clean.encode_utf16().count() > 65536;
    Some(json!({
        "text": text(&clean, 65536),
        "added": added,
        "removed": removed,
        "truncated": truncated
    }))
}

struct OpenTool {
    kind: String,
    target: Option<String>,
}

#[derive(Default)]
pub struct CursorNormalizer {
    pub thread: Option<String>,
    pub ended: bool,
    count: usize,
    open_tools: HashMap<String, OpenTool>,
    todos: HashMap<String, Value>,
    pub saw_result: bool,
    terminal_failure: Option<String>,
    usage: Option<Value>,
    message_index: usize,
    thinking: bool,
    reasoning: String,
    delivered_length: HashMap<String, usize>,
    delivered_text: HashMap<String, String>,
    delivered_pieces: HashMap<String, usize>,
    unknown_types: HashSet<String>,
    oversized_dropped: usize,
    output_limit_exceeded: bool,
}

impl CursorNormalizer {
    fn event(&mut self, kind: &str, payload: Value) -> Event {
        self.count += 1;
        Event {
            kind: kind.into(),
            payload,
            thread: self.thread.clone(),
        }
    }

    fn process(&self, success: bool, cancelled: bool, exit_code: Option<i32>) -> Value {
        json!({
            "exitCode": exit_code.map(Value::from).unwrap_or(Value::Null),
            "signal": null,
            "stderr": "",
            "stderrTruncated": false,
            "recordCount": self.count,
            "inputDeliveryFailed": false,
            "outputLimitExceeded": self.output_limit_exceeded,
            "oversizedRecordsDropped": self.oversized_dropped,
            "forcedTerminationAttempted": cancelled,
            "terminationUnconfirmed": !success && !cancelled,
            "startedAt": crate::ledger_write::now(),
            "finishedAt": crate::ledger_write::now()
        })
    }

    fn evidence(&self, type_name: &str) -> Value {
        json!({"runtimeEventType": type_name, "redacted": true})
    }

    pub fn diagnostic(&mut self, level: &str, code: &str, message: &str) -> Event {
        self.event(
            "adapter.diagnostic",
            json!({
                "level": level,
                "code": code,
                "message": text(message, 16384),
                "terminal": false,
                "evidence": self.evidence("cursor")
            }),
        )
    }

    pub fn note_oversized_drop(&mut self) {
        self.oversized_dropped = self.oversized_dropped.saturating_add(1);
    }

    pub fn finish(&mut self, cancelled: bool, exit_code: Option<i32>) -> Event {
        self.ended = true;
        let thread = self.thread.clone();
        if cancelled {
            let mut p = json!({"process": self.process(false, true, exit_code)});
            if let Some(t) = thread {
                p["runtimeThreadId"] = json!(t);
            }
            return self.event("run.cancelled", p);
        }
        if let Some(failure) = self.terminal_failure.clone() {
            let mut p = json!({
                "kind": "unknown",
                "message": failure,
                "runtimeTerminal": "failed",
                "process": self.process(false, false, exit_code)
            });
            if let Some(t) = thread {
                p["runtimeThreadId"] = json!(t);
            }
            return self.event("run.failed", p);
        }
        if !self.saw_result || exit_code != Some(0) {
            let message = if self.output_limit_exceeded {
                "Cursor Agent sent more output than Locust could take in, so the run was stopped."
                    .into()
            } else if self.saw_result {
                format!(
                    "Cursor Agent exited with code {}.",
                    exit_code
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "null".into())
                )
            } else if self.oversized_dropped > 0 {
                let n = self.oversized_dropped;
                format!(
                    "Cursor Agent ended without a terminal result record, after Locust skipped {n} piece{} of output too large to take in. The answer above is what arrived before that.",
                    if n == 1 { "" } else { "s" }
                )
            } else {
                "Cursor Agent ended without a terminal result record.".into()
            };
            let mut p = json!({
                "kind": "process-failed",
                "message": text(&message, 16384),
                "runtimeTerminal": if self.saw_result { "completed" } else { "missing" },
                "process": self.process(false, false, exit_code)
            });
            if let Some(t) = thread {
                p["runtimeThreadId"] = json!(t);
            }
            return self.event("run.failed", p);
        }
        let mut p = json!({"process": self.process(true, false, exit_code)});
        if let Some(t) = thread {
            p["runtimeThreadId"] = json!(t);
        }
        if let Some(usage) = &self.usage {
            p["usage"] = usage.clone();
        }
        self.event("run.completed", p)
    }

    pub fn accept_line(&mut self, raw: &str) -> Vec<Event> {
        if self.ended {
            return vec![];
        }
        let parsed: Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(_) => {
                return vec![self.diagnostic(
                    "warning",
                    "cursor.malformed_record",
                    "A Cursor record could not be parsed.",
                )];
            }
        };
        if !obj(&parsed) {
            return vec![self.diagnostic(
                "warning",
                "cursor.malformed_record",
                "A Cursor record was not an object.",
            )];
        }
        self.accept(&parsed)
    }

    pub fn accept(&mut self, parsed: &Value) -> Vec<Event> {
        if self.ended {
            return vec![];
        }
        let type_name = s(parsed, "type");
        if type_name.is_empty() {
            return vec![];
        }

        if type_name == "thinking" {
            let evidence = self.evidence(type_name);
            // Cursor exposes this thinking to its user. Match TS: scrub each
            // fragment, accumulate one step, and bound its message only on close.
            let fragment = super::events::redact_secrets(s(parsed, "text"));
            if s(parsed, "subtype") == "completed" {
                if !self.thinking {
                    return vec![];
                }
                self.thinking = false;
                let said = text(self.reasoning.trim(), 16384);
                self.reasoning.clear();
                let mut p = json!({"stepKind":"reasoning","evidence":evidence});
                if !said.is_empty() {
                    p["message"] = json!(said);
                }
                return vec![self.event("step.completed", p)];
            }
            self.reasoning.push_str(&fragment);
            if self.thinking {
                return vec![];
            }
            self.thinking = true;
            return vec![self.event(
                "step.started",
                json!({"stepKind":"reasoning","evidence":evidence}),
            )];
        }

        let evidence = self.evidence(type_name);

        if type_name == "system" {
            if s(parsed, "subtype") != "init" {
                return vec![
                    self.event(
                        "step.started",
                        json!({"stepKind":"turn","evidence":evidence}),
                    ),
                    self.event(
                        "step.completed",
                        json!({"stepKind":"turn","evidence":evidence}),
                    ),
                ];
            }
            let sid = identity(parsed, "session_id");
            self.thread = (!sid.is_empty()).then_some(sid.clone());
            return vec![self.event(
                "run.started",
                json!({
                    "runtimeThreadId": self.thread.clone().unwrap_or_default(),
                    "evidence": {"runtimeEventType":"system","raw":summarize_init(parsed),"redacted":true}
                }),
            )];
        }

        if type_name == "user" {
            return vec![];
        }

        if type_name == "assistant" {
            let text_body = text(&assistant_text(parsed), 1_000_000);
            if text_body.is_empty() {
                return vec![];
            }
            let item_id = format!("msg_{}", self.message_index);
            let pieces = *self.delivered_pieces.get(&item_id).unwrap_or(&0);
            let prior = self
                .delivered_text
                .get(&item_id)
                .cloned()
                .unwrap_or_default();
            let restates = text_body.len() > 1 && pieces > 1 && text_body == prior;
            if is_message_fragment(parsed) && !restates {
                let utf16 = text_body.encode_utf16().count();
                let prior_len = self.delivered_length.get(&item_id).copied().unwrap_or(0);
                self.delivered_length
                    .insert(item_id.clone(), prior_len + utf16);
                self.delivered_text
                    .entry(item_id.clone())
                    .and_modify(|t| t.push_str(&text_body))
                    .or_insert(text_body.clone());
                *self.delivered_pieces.entry(item_id.clone()).or_insert(0) += 1;
                return vec![self.event(
                    "message.delta",
                    json!({
                        "itemId": item_id,
                        "operation": "append",
                        "text": text(&text_body, 131072),
                        "final": false,
                        "evidence": evidence
                    }),
                )];
            }
            self.message_index += 1;
            let bounded = text(&text_body, 16384);
            let already = self.delivered_length.get(&item_id).copied().unwrap_or(0);
            if bounded.encode_utf16().count() < already {
                return vec![self.event(
                    "message.delta",
                    json!({
                        "itemId": item_id,
                        "operation": "append",
                        "text": "",
                        "final": true,
                        "evidence": evidence
                    }),
                )];
            }
            return vec![self.event(
                "message.delta",
                json!({
                    "itemId": item_id,
                    "operation": "replace",
                    "text": bounded,
                    "final": true,
                    "evidence": evidence
                }),
            )];
        }

        if type_name == "tool_call" {
            return self.accept_tool(parsed, evidence);
        }

        if type_name == "result" {
            self.saw_result = true;
            if obj(&parsed["usage"]) {
                self.usage = Some(sanitized_usage(&parsed["usage"]));
            }
            let is_error = parsed["is_error"] == true;
            let subtype = s(parsed, "subtype");
            if is_error || (!subtype.is_empty() && subtype != "success") {
                let raw = s(parsed, "result");
                let message = if raw.is_empty() {
                    format!(
                        "Cursor Agent ended with {}.",
                        if subtype.is_empty() {
                            "an error"
                        } else {
                            subtype
                        }
                    )
                } else {
                    raw.to_owned()
                };
                self.terminal_failure = Some(text(&provider_error_sentence(&message), 16384));
            }
            return vec![];
        }

        if type_name == "interaction_query" {
            return vec![];
        }

        if type_name == "connection" || type_name == "retry" {
            let subtype = s(parsed, "subtype");
            let attempt = parsed["attempt"]
                .as_f64()
                .filter(|n| n.is_finite())
                .map(|n| format!(" (attempt {})", n as i64))
                .unwrap_or_default();
            let said = if type_name == "connection" {
                if subtype == "reconnecting" {
                    format!("Cursor lost its connection and is reconnecting{attempt}.")
                } else {
                    format!(
                        "Cursor's connection: {}{attempt}.",
                        if subtype.is_empty() {
                            "changed"
                        } else {
                            subtype
                        }
                    )
                }
            } else if subtype == "starting" {
                if parsed["is_resume"] == true {
                    format!("Cursor is picking the turn back up after the reconnect{attempt}.")
                } else {
                    format!("Cursor is retrying{attempt}.")
                }
            } else {
                format!(
                    "Cursor's retry: {}{attempt}.",
                    if subtype.is_empty() {
                        "changed"
                    } else {
                        subtype
                    }
                )
            };
            return vec![self.diagnostic("info", &format!("cursor.{type_name}"), &said)];
        }

        if type_name == "locust.oversized" {
            return self.accept_oversized(parsed, evidence);
        }

        if !self.unknown_types.insert(type_name.to_owned()) {
            return vec![];
        }
        vec![self.diagnostic(
            "info",
            "cursor.unknown_event",
            &format!("Unhandled Cursor record: {type_name}"),
        )]
    }

    fn accept_tool(&mut self, parsed: &Value, evidence: Value) -> Vec<Event> {
        let wrapper = if obj(&parsed["tool_call"]) {
            &parsed["tool_call"]
        } else {
            &Value::Null
        };
        let key = wrapper
            .as_object()
            .into_iter()
            .flatten()
            .map(|(k, _)| k.clone())
            .find(|name| name.ends_with("ToolCall"));
        let call = key
            .as_ref()
            .and_then(|k| wrapper.get(k))
            .filter(|v| obj(v))
            .cloned()
            .unwrap_or(Value::Null);
        let args = if obj(&call["args"]) {
            &call["args"]
        } else {
            &Value::Null
        };
        let outcome = if obj(&call["result"]) {
            &call["result"]
        } else {
            &Value::Null
        };
        let refused = if obj(&outcome["rejected"]) {
            Some(&outcome["rejected"])
        } else {
            None
        };
        let raw_id = {
            let a = identity(wrapper, "toolCallId");
            let b = identity(parsed, "call_id");
            if !a.is_empty() {
                Some(a)
            } else if !b.is_empty() {
                Some(b)
            } else {
                None
            }
        };
        let item_id = raw_id
            .map(|id| {
                id.split_whitespace()
                    .filter(|p| !p.is_empty())
                    .collect::<Vec<_>>()
                    .join("|")
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("tool_{}", self.count + 1));
        let kind = key
            .as_ref()
            .map(|k| tool_kind(k))
            .unwrap_or_else(|| "tool".into());
        let target = string_opt(&args["command"])
            .or_else(|| string_opt(&args["path"]))
            .or_else(|| refused.and_then(|r| string_opt(&r["command"])))
            .or_else(|| refused.and_then(|r| string_opt(&r["path"])))
            .or_else(|| string_opt(&args["description"]))
            .or_else(|| string_opt(&args["prompt"]))
            .or_else(|| glob_target(args));
        let subtype = s(parsed, "subtype");

        if key.as_deref() == Some("updateTodosToolCall") {
            if subtype != "completed" {
                return vec![];
            }
            let from_result = outcome
                .get("success")
                .filter(|v| obj(v))
                .and_then(|v| v.get("todos"));
            if let Some(Value::Array(list)) = from_result {
                self.todos.clear();
                for todo in list {
                    remember_todo(&mut self.todos, todo);
                }
            } else {
                if args["merge"] != true {
                    self.todos.clear();
                }
                if let Some(Value::Array(list)) = args.get("todos") {
                    for todo in list {
                        remember_todo(&mut self.todos, todo);
                    }
                }
            }
            if self.todos.is_empty() {
                return vec![];
            }
            let plan: Vec<Value> = {
                let mut keys: Vec<_> = self.todos.keys().cloned().collect();
                keys.sort_by(|a, b| match (a.parse::<u64>(), b.parse::<u64>()) {
                    (Ok(x), Ok(y)) => x.cmp(&y),
                    _ => a.cmp(b),
                });
                keys.into_iter().map(|k| self.todos[&k].clone()).collect()
            };
            return vec![self.event(
                "plan.updated",
                json!({
                    "itemId": item_id,
                    "plan": plan,
                    "final": false,
                    "evidence": evidence
                }),
            )];
        }

        if subtype == "started" {
            self.open_tools.insert(
                item_id.clone(),
                OpenTool {
                    kind: kind.clone(),
                    target: target.clone(),
                },
            );
            let mut p = json!({
                "itemId": item_id,
                "toolKind": kind,
                "name": kind,
                "phase": "started",
                "evidence": evidence
            });
            if let Some(t) = target {
                p["command"] = json!(text(&t, 16384));
            }
            return vec![self.event("tool.started", p)];
        }
        if subtype == "completed" {
            let open = self.open_tools.remove(&item_id);
            let (failed, status, exit_code) = tool_outcome(outcome);
            let success = if obj(&outcome["success"]) {
                &outcome["success"]
            } else {
                &Value::Null
            };
            let patch = success["diffString"].as_str().and_then(tool_patch_from);
            let tool_kind = open.as_ref().map(|o| o.kind.clone()).unwrap_or(kind);
            let mut p = json!({
                "itemId": item_id,
                "toolKind": tool_kind,
                "name": tool_kind,
                "phase": "completed",
                "evidence": evidence
            });
            if let Some(t) = target {
                p["command"] = json!(text(&t, 16384));
            }
            if let Some(st) = status {
                p["status"] = json!(st);
            }
            if let Some(code) = exit_code {
                p["exitCode"] = json!(code);
            }
            if let Some(patch) = patch {
                p["patch"] = patch;
            }
            let kind_name = if failed {
                "tool.failed"
            } else {
                "tool.completed"
            };
            return vec![self.event(kind_name, p)];
        }
        vec![]
    }

    fn accept_oversized(&mut self, parsed: &Value, evidence: Value) -> Vec<Event> {
        if parsed.get("recordType").and_then(|v| v.as_str()) != Some("tool_call") {
            return vec![];
        }
        let kb = parsed["bytes"]
            .as_f64()
            .filter(|n| n.is_finite())
            .map(|n| (n / 1024.0).round() as i64);
        let edit = if obj(&parsed["edit"]) {
            Some(&parsed["edit"])
        } else {
            None
        };
        let added = edit.and_then(|e| e["added"].as_i64()).filter(|&n| n >= 0);
        let removed = edit.and_then(|e| e["removed"].as_i64()).filter(|&n| n >= 0);
        let diff = edit.and_then(|e| e["diff"].as_str());
        let mut events = Vec::new();
        let call_ids = parsed["callIds"].as_array().cloned().unwrap_or_default();
        for id in call_ids {
            let Some(item_id) = id.as_str().map(str::to_owned) else {
                continue;
            };
            let Some(open) = self.open_tools.remove(&item_id) else {
                continue;
            };
            if open.kind == "edit" && added.is_some() && removed.is_some() {
                let patch = diff.and_then(tool_patch_from).unwrap_or_else(|| {
                    json!({
                        "text": "",
                        // Invariant: branch requires added.is_some().
                        "added": added.expect("branch requires added.is_some()"),
                        // Invariant: branch requires removed.is_some().
                        "removed": removed.expect("branch requires removed.is_some()"),
                        "truncated": true
                    })
                });
                let mut p = json!({
                    "itemId": item_id,
                    "toolKind": open.kind,
                    "name": open.kind,
                    "patch": patch,
                    "phase": "completed",
                    "evidence": evidence
                });
                if let Some(t) = &open.target {
                    p["command"] = json!(text(t, 16384));
                }
                events.push(self.event("tool.completed", p));
                continue;
            }
            let output = match kb {
                Some(k) => format!(
                    "The result was too large to keep ({k} KB). Locust keeps results up to 256 KB."
                ),
                None => {
                    "The result was too large to keep. Locust keeps results up to 256 KB.".into()
                }
            };
            let mut p = json!({
                "itemId": item_id,
                "toolKind": open.kind,
                "name": open.kind,
                "phase": "completed",
                "status": "result too large to keep",
                "output": output,
                "evidence": evidence
            });
            if let Some(t) = &open.target {
                p["command"] = json!(text(t, 16384));
            }
            events.push(self.event("tool.completed", p));
        }
        events
    }
}

//! GitHub Copilot CLI jsonl -> ledger events. Ports createCopilotEventNormalizer (copilot-events.ts).
use super::Event;
use crate::runtime::events::text;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

const IGNORED: &[&str] = &[
    "model.turn_started",
    "model.turn_ended",
    "model.call_start",
    "model.call_finished",
    "model.model_call_started",
    "model.model_call_success",
    "model.message",
    "model.response",
    "model.tool_execution",
    "model.captured_assignment_context",
    "session.mcp_server_status_changed",
    "session.mcp_servers_loaded",
    "session.tools_updated",
    "session.indexed_search",
    "assistant.message_start",
    "assistant.tool_call_delta",
    "assistant.idle",
    "model.messages_snapshot",
    "session.skills_loaded",
    "tool.shell_output",
    "tool.execution_partial_result",
    "user.message",
];
const AUTO_KEYS: &[&str] = &[
    "chosenModel",
    "routingMethod",
    "availableModels",
    "fallback",
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

pub fn summarize_auto_mode(value: &Value) -> Value {
    let mut summary = json!({});
    if !obj(value) {
        return summary;
    }
    for key in AUTO_KEYS {
        match &value[*key] {
            Value::String(s) => summary[*key] = json!(text(s, 16384)),
            Value::Bool(b) => summary[*key] = json!(b),
            Value::Array(a) => {
                summary[*key] = json!(a.iter().filter_map(|e| e.as_str()).collect::<Vec<_>>())
            }
            _ => {}
        }
    }
    summary
}

pub fn usage(value: &Value) -> Option<Value> {
    if !obj(value) {
        return None;
    }
    let mut u = serde_json::Map::new();
    if let Some(n) = value["totalPremiumRequests"]
        .as_f64()
        .filter(|n| n.is_finite())
    {
        u.insert("premiumRequests".into(), json!(n));
    }
    if let Some(n) = value["totalNanoAiu"].as_f64().filter(|n| n.is_finite()) {
        u.insert("nanoAiu".into(), json!(n));
    }
    if u.is_empty() {
        None
    } else {
        Some(Value::Object(u))
    }
}

pub fn tool_command(arguments: &Value) -> Option<String> {
    if let Some(s) = arguments.as_str() {
        return (!s.is_empty()).then(|| s.to_owned());
    }
    if !obj(arguments) {
        return None;
    }
    for key in ["command", "path", "file_path", "pattern", "description"] {
        if let Some(s) = arguments[key].as_str().filter(|s| !s.is_empty()) {
            return Some(s.to_owned());
        }
    }
    None
}

pub fn failure_from(stderr: &str) -> Option<(String, String)> {
    let lower = stderr.to_ascii_lowercase();
    if lower.contains("access denied by policy settings")
        || lower.contains("copilot cli policy setting")
    {
        return Some(("authentication-failed".into(),
            "Copilot plan required: this GitHub account's Copilot plan does not include the CLI, or an organization policy blocks it. Check https://github.com/settings/copilot".into()));
    }
    let re = crate::invariants::regex(
        r#"Error:\s*(Model "[^"]*" from --model flag is not available\.)"#,
    );
    if let Some(c) = re.captures(stderr) {
        return Some((
            "unknown".into(),
            c.get(1)
                // Invariant: successful regex match includes mandatory capture group one.
                .expect("successful regex match includes mandatory capture group one")
                .as_str()
                .to_owned(),
        ));
    }
    None
}

fn patch_from_detailed(detailed: &str) -> Option<Value> {
    if !(detailed.starts_with("diff --git ")
        || detailed.contains("\n--- ")
        || detailed.starts_with("--- "))
    {
        // also allow /^--- /m
        if !crate::invariants::regex(r"(?m)^diff --git |^--- ").is_match(detailed) {
            return None;
        }
    }
    let patch = crate::runtime::cursor_events::tool_patch_from(detailed)?;
    let added = patch["added"].as_u64().unwrap_or(0);
    let removed = patch["removed"].as_u64().unwrap_or(0);
    if added + removed == 0 {
        None
    } else {
        Some(patch)
    }
}

#[derive(Default)]
pub struct CopilotNormalizer {
    pub thread: Option<String>,
    pub ended: bool,
    pub saw_result: bool,
    count: usize,
    open_tools: HashMap<String, (String, Option<String>)>,
    open_reasoning: HashSet<String>,
    reasoning_ordinals: HashMap<String, String>,
    unknown: HashSet<String>,
    started_run: bool,
    terminal_failure: Option<String>,
    usage: Option<Value>,
    open_turns: HashSet<String>,
    delivered_length: HashMap<String, usize>,
    host_session: Option<String>,
    stderr: String,
}

impl CopilotNormalizer {
    pub fn with_session(mut self, session: Option<String>) -> Self {
        self.host_session = session.clone();
        self.thread = session;
        self
    }
    pub fn set_stderr(&mut self, stderr: &str) {
        self.stderr = stderr.to_owned();
    }
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
            "signal": null, "stderr": text(&self.stderr, 16384), "stderrTruncated": false,
            "recordCount": self.count, "inputDeliveryFailed": false, "outputLimitExceeded": false,
            "oversizedRecordsDropped": 0, "forcedTerminationAttempted": cancelled,
            "terminationUnconfirmed": !success && !cancelled,
            "startedAt": crate::ledger_write::now(), "finishedAt": crate::ledger_write::now()
        })
    }
    fn evidence(&self, t: &str) -> Value {
        json!({"runtimeEventType": t, "redacted": true})
    }
    pub fn diagnostic(&mut self, level: &str, code: &str, message: &str) -> Event {
        self.event(
            "adapter.diagnostic",
            json!({
                "level": level, "code": code, "message": text(message,16384), "terminal": false,
                "evidence": self.evidence("copilot")
            }),
        )
    }
    fn reasoning_key(&mut self, raw: &str) -> String {
        let key = if raw.is_empty() {
            "reasoning".into()
        } else {
            raw.to_owned()
        };
        if let Some(k) = self.reasoning_ordinals.get(&key) {
            return k.clone();
        }
        let next = (self.reasoning_ordinals.len() + 1).to_string();
        self.reasoning_ordinals.insert(key, next.clone());
        next
    }
    pub fn finish(&mut self, cancelled: bool, exit_code: Option<i32>) -> Event {
        self.ended = true;
        let thread = self.thread.clone();
        if cancelled {
            let mut p = json!({"process": self.process(false,true,exit_code)});
            if let Some(t) = &thread {
                p["runtimeThreadId"] = json!(t);
            }
            return self.event("run.cancelled", p);
        }
        if let Some((kind, message)) = failure_from(&self.stderr) {
            let mut p = json!({
                "kind": kind, "message": message,
                "runtimeTerminal": if self.saw_result {"completed"} else {"missing"},
                "process": self.process(false,false,exit_code)
            });
            if let Some(t) = &thread {
                p["runtimeThreadId"] = json!(t);
            }
            return self.event("run.failed", p);
        }
        if let Some(failure) = self.terminal_failure.clone() {
            let mut p = json!({"kind":"unknown","message":failure,"runtimeTerminal":"failed","process":self.process(false,false,exit_code)});
            if let Some(t) = &thread {
                p["runtimeThreadId"] = json!(t);
            }
            return self.event("run.failed", p);
        }
        if !self.saw_result || exit_code != Some(0) {
            let message = if self.saw_result {
                format!(
                    "Copilot CLI exited with code {}.",
                    exit_code
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "null".into())
                )
            } else {
                "Copilot CLI ended without a terminal result record.".into()
            };
            let mut p = json!({
                "kind":"process-failed","message":text(&message,16384),
                "runtimeTerminal": if self.saw_result {"completed"} else {"missing"},
                "process": self.process(false,false,exit_code)
            });
            if let Some(t) = &thread {
                p["runtimeThreadId"] = json!(t);
            }
            return self.event("run.failed", p);
        }
        let mut p = json!({"process": self.process(true,false,exit_code)});
        if let Some(t) = &thread {
            p["runtimeThreadId"] = json!(t);
        }
        if let Some(u) = &self.usage {
            p["usage"] = u.clone();
        }
        self.event("run.completed", p)
    }
    #[cfg(any(test, feature = "test-hooks"))]
    #[cfg(any(test, feature = "test-hooks"))]
    pub fn accept_line(&mut self, raw: &str) -> Vec<Event> {
        if self.ended {
            return vec![];
        }
        let parsed: Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(_) => {
                return vec![self.diagnostic(
                    "warning",
                    "copilot.malformed_record",
                    "A Copilot record could not be parsed.",
                )];
            }
        };
        if !obj(&parsed) {
            return vec![self.diagnostic(
                "warning",
                "copilot.malformed_record",
                "A Copilot record was not an object.",
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
        if IGNORED.contains(&type_name) {
            return vec![];
        }
        let data = if obj(&parsed["data"]) {
            &parsed["data"]
        } else {
            &Value::Null
        };
        let evidence = self.evidence(type_name);

        if type_name == "session.background_tasks_changed" {
            if obj(data) && data.as_object().map(|o| o.is_empty()).unwrap_or(true) {
                return vec![];
            }
            return vec![self.diagnostic(
                "info",
                "copilot.background_tasks_changed",
                "Copilot reported a change to its background tasks.",
            )];
        }
        if type_name == "session.auto_mode_resolved" {
            if self.started_run {
                return vec![];
            }
            self.started_run = true;
            return vec![self.event("run.started", json!({
                "runtimeThreadId": self.thread.clone().unwrap_or_default(),
                "evidence": {"runtimeEventType":"session.auto_mode_resolved","raw":summarize_auto_mode(data),"redacted":true}
            }))];
        }
        if type_name == "session.warning" {
            let msg = s(data, "message");
            return vec![self.diagnostic(
                "warning",
                "copilot.session_warning",
                if msg.is_empty() {
                    "Copilot CLI reported a session warning."
                } else {
                    msg
                },
            )];
        }
        if type_name == "assistant.turn_start" {
            let turn_id = {
                let t = identity(data, "turnId");
                if t.is_empty() {
                    format!("turn_{}", self.count + 1)
                } else {
                    t
                }
            };
            if !self.open_turns.insert(turn_id.clone()) {
                return vec![];
            }
            return vec![self.event(
                "step.started",
                json!({"stepKind":"turn","itemId":turn_id,"evidence":evidence}),
            )];
        }
        if type_name == "assistant.turn_end" {
            let turn_id = {
                let t = identity(data, "turnId");
                if t.is_empty() {
                    format!("turn_{}", self.count + 1)
                } else {
                    t
                }
            };
            if !self.open_turns.remove(&turn_id) {
                return vec![];
            }
            return vec![self.event(
                "step.completed",
                json!({"stepKind":"turn","itemId":turn_id,"evidence":evidence}),
            )];
        }
        if type_name == "assistant.reasoning_delta" {
            let rid = self.reasoning_key(s(data, "reasoningId"));
            if !self.open_reasoning.insert(rid.clone()) {
                return vec![];
            }
            return vec![self.event("step.started", json!({"stepKind":"reasoning","itemId":format!("reasoning_{rid}"),"evidence":evidence}))];
        }
        if type_name == "assistant.reasoning" {
            let rid = self.reasoning_key(s(data, "reasoningId"));
            let was = self.open_reasoning.remove(&rid);
            let mut out = Vec::new();
            if !was {
                out.push(self.event("step.started", json!({"stepKind":"reasoning","itemId":format!("reasoning_{rid}"),"evidence":evidence})));
            }
            out.push(self.event("step.completed", json!({"stepKind":"reasoning","itemId":format!("reasoning_{rid}"),"evidence":evidence})));
            return out;
        }
        if matches!(
            type_name,
            "assistant.tool_call_delta"
                | "model.call_start"
                | "model.call_finished"
                | "assistant.message_start"
                | "assistant.idle"
        ) {
            return vec![];
        }
        if type_name == "model.call_final_result" && s(data, "result") == "success" {
            return vec![];
        }

        if type_name == "assistant.message_delta" {
            let text_body = s(data, "deltaContent");
            let mid = identity(data, "messageId");
            if text_body.is_empty() || mid.is_empty() {
                return vec![];
            }
            let item_id = format!("msg_{mid}");
            let delivered = text(text_body, 16384);
            let n = delivered.encode_utf16().count();
            *self.delivered_length.entry(item_id.clone()).or_insert(0) += n;
            return vec![self.event("message.delta", json!({"itemId":item_id,"operation":"append","text":delivered,"final":false,"evidence":evidence}))];
        }
        if type_name == "assistant.message" {
            let text_body = s(data, "content");
            let mid = identity(data, "messageId");
            if text_body.is_empty() || mid.is_empty() {
                return vec![];
            }
            let item_id = format!("msg_{mid}");
            let bounded = text(text_body, 131072);
            // TS uses boundedAnswerText for complete; compare UTF-16 lengths
            let already = self.delivered_length.get(&item_id).copied().unwrap_or(0);
            if bounded.encode_utf16().count() < already {
                return vec![self.event("message.delta", json!({"itemId":item_id,"operation":"append","text":"","final":true,"evidence":evidence}))];
            }
            return vec![self.event("message.delta", json!({"itemId":item_id,"operation":"replace","text":bounded,"final":true,"evidence":evidence}))];
        }
        if type_name == "tool.execution_start" {
            let item_id = {
                let t = identity(data, "toolCallId");
                if t.is_empty() {
                    format!("tool_{}", self.count + 1)
                } else {
                    t
                }
            };
            let kind = {
                let t = identity(data, "toolName");
                if t.is_empty() { "tool".into() } else { t }
            };
            let command = tool_command(&data["arguments"]);
            self.open_tools
                .insert(item_id.clone(), (kind.clone(), command.clone()));
            let mut p = json!({"itemId":item_id,"toolKind":kind,"name":kind,"phase":"started","evidence":evidence});
            if let Some(c) = command {
                p["command"] = json!(text(&c, 16384));
            }
            return vec![self.event("tool.started", p)];
        }
        if type_name == "tool.execution_complete" {
            let item_id = {
                let t = identity(data, "toolCallId");
                if t.is_empty() {
                    format!("tool_{}", self.count + 1)
                } else {
                    t
                }
            };
            let open = self.open_tools.remove(&item_id);
            let failed = data["success"] != true;
            let error = if obj(&data["error"]) {
                &data["error"]
            } else {
                &Value::Null
            };
            let status = if failed {
                let code = s(error, "code");
                let msg = s(error, "message");
                Some(text(
                    if !code.is_empty() {
                        code
                    } else if !msg.is_empty() {
                        msg
                    } else {
                        "unknown"
                    },
                    512,
                ))
            } else {
                None
            };
            let result = if obj(&data["result"]) {
                &data["result"]
            } else {
                &Value::Null
            };
            let patch = result["detailedContent"]
                .as_str()
                .and_then(patch_from_detailed);
            let kind = open.as_ref().map(|o| o.0.clone()).unwrap_or_else(|| {
                let t = identity(data, "toolName");
                if t.is_empty() { "tool".into() } else { t }
            });
            let mut p = json!({"itemId":item_id,"toolKind":kind,"name":kind,"phase":"completed","evidence":evidence});
            if let Some((_, Some(c))) = &open {
                p["command"] = json!(text(c, 16384));
            }
            if let Some(st) = status {
                p["status"] = json!(st);
            }
            if let Some(patch) = patch {
                p["patch"] = patch;
            }
            return vec![self.event(
                if failed {
                    "tool.failed"
                } else {
                    "tool.completed"
                },
                p,
            )];
        }
        if type_name == "session.usage_checkpoint" {
            self.usage = usage(data);
            return vec![];
        }
        if type_name == "session.info" && s(data, "infoType") == "configuration" {
            return vec![];
        }
        if type_name == "result" {
            self.saw_result = true;
            let sid = identity(parsed, "sessionId");
            if !sid.is_empty() {
                self.thread = Some(sid);
            }
            if let Some(code) = parsed["exitCode"].as_i64()
                && code != 0
            {
                self.terminal_failure = Some(format!("Copilot CLI reported exit code {code}."));
            }
            return vec![];
        }
        if !self.unknown.insert(type_name.to_owned()) {
            return vec![];
        }
        vec![self.diagnostic(
            "info",
            "copilot.unknown_event",
            &format!("Unhandled Copilot record: {type_name}"),
        )]
    }
}

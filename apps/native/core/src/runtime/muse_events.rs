//! Muse Code exec --json → ledger events. Ports createMuseEventNormalizer (muse-events.ts).
use super::Event;
use crate::runtime::events::text;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

pub const MUSE_BILLING_SENTENCE: &str = "Meta will not run Muse Code until the account's payment method is verified (API error 402, billing_error), so nothing in this run happened. That is settled in your Meta account, not in Locust.";
pub const MUSE_SANDBOX_SENTENCE: &str = "Muse Code can't run commands on this computer yet: its Windows sandbox isn't set up, so in Edit every command fails before it starts. Switch this teammate to Auto to run commands without that sandbox, or run `muse sandbox windows setup` once in a terminal; it may ask for an administrator's OK.";
pub const MUSE_QUOTA_SENTENCE: &str = "Muse Code's plan is used up for now, so it stopped before finishing. Pick another model to carry on, or send again once it resets.";

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn obj(v: &Value) -> bool {
    v.is_object()
}
fn identity(v: &Value) -> Option<String> {
    v.as_str()
        .filter(|t| !t.is_empty())
        .map(|t| t.replace('\0', "").chars().take(512).collect())
}

pub fn muse_session_id_of(parsed: &Value) -> Option<String> {
    let stream = if obj(&parsed["stream"]) {
        &parsed["stream"]
    } else {
        return None;
    };
    if s(stream, "kind") != "session" {
        return None;
    }
    identity(&stream["id"])
}

pub fn muse_task_is_internal(task_kind: Option<&str>) -> bool {
    task_kind.is_some_and(|k| crate::invariants::regex(r"^(?:model|reminder)\.").is_match(k))
}

pub fn muse_quota_limit(reason: &str) -> String {
    let re =
        crate::invariants::regex(r"resets at (\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z)");
    match re.captures(reason) {
        Some(c) => format!("Muse Code's plan is used up · resets {}", &c[1]),
        None => "Muse Code's plan is used up".into(),
    }
}

#[derive(Default)]
pub struct MuseNormalizer {
    pub thread: Option<String>,
    pub ended: bool,
    pub saw_result: bool,
    count: usize,
    run_open: bool,
    terminal: Option<String>,
    terminal_reason: Option<String>,
    streamed_text: String,
    message_closed: bool,
    task_kinds: HashMap<String, String>,
    task_operations: HashMap<String, String>,
    started_tasks: HashSet<String>,
    retries_said: HashSet<i64>,
    sandbox_said: bool,
    quota_said: bool,
    output_limit_exceeded: bool,
}

impl MuseNormalizer {
    #[cfg(any(test, feature = "test-hooks"))]
    #[cfg(any(test, feature = "test-hooks"))]
    pub fn with_output_limit_exceeded(mut self, v: bool) -> Self {
        self.output_limit_exceeded = v;
        self
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
            "signal": null, "stderr": "", "stderrTruncated": false,
            "recordCount": self.count, "inputDeliveryFailed": false,
            "outputLimitExceeded": self.output_limit_exceeded,
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
            json!({"level":level,"code":code,"message":text(message,16384),"terminal":false,"evidence":self.evidence("muse")}),
        )
    }
    fn tool_row(&self, task_id: &str) -> Value {
        let kind = self
            .task_kinds
            .get(task_id)
            .map(String::as_str)
            .unwrap_or("task");
        let name = kind.strip_prefix("tool.").unwrap_or(kind);
        let mut p = json!({"itemId": task_id, "toolKind": kind, "name": name});
        if let Some(op) = self.task_operations.get(task_id) {
            p["command"] = json!(text(op, 16384));
        }
        p
    }
    pub fn finish(&mut self, cancelled: bool, exit_code: Option<i32>) -> Vec<Event> {
        if self.ended {
            return vec![];
        }
        self.ended = true;
        let thread = self.thread.clone();
        if cancelled {
            let mut p = json!({"process": self.process(false, true, exit_code)});
            if let Some(t) = &thread {
                p["runtimeThreadId"] = json!(t);
            }
            return vec![self.event("run.cancelled", p)];
        }
        let finished = self.terminal.as_deref() == Some("completed");
        if finished && exit_code == Some(0) {
            let mut p = json!({"process": self.process(true, false, exit_code)});
            if let Some(t) = &thread {
                p["runtimeThreadId"] = json!(t);
            }
            return vec![self.event("run.completed", p)];
        }
        let reason = self.terminal_reason.clone().unwrap_or_default();
        let quota = !finished
            && !reason.is_empty()
            && crate::invariants::regex(r"(?i)subscription quota exhausted|usage window resets")
                .is_match(&reason);
        let mut out = Vec::new();
        if quota && !self.quota_said {
            self.quota_said = true;
            out.push(self.event(
                "route.limit_detected",
                json!({"kind":"quota-exhausted","message":muse_quota_limit(&reason),"evidence":self.evidence("run.terminal")}),
            ));
        }
        let message = if self.output_limit_exceeded {
            "Muse Code sent output faster than Locust could record it, so the run was stopped rather than leave a gap in its record. Sending it again usually works.".into()
        } else if self.terminal.is_none() {
            "Muse Code ended without a record saying the run had finished.".into()
        } else if !finished {
            if reason.is_empty() {
                format!(
                    "Muse Code ended {}.",
                    self.terminal.as_deref().unwrap_or("unknown")
                )
            } else if crate::invariants::regex(
                r"(?i)\bbilling_error\b|\bAPI error 402\b|billing verification failed",
            )
            .is_match(&reason)
            {
                MUSE_BILLING_SENTENCE.into()
            } else if quota {
                MUSE_QUOTA_SENTENCE.into()
            } else {
                format!(
                    "Muse Code ended {}: {reason}",
                    self.terminal.as_deref().unwrap_or("unknown")
                )
            }
        } else {
            format!(
                "Muse Code exited with code {}.",
                exit_code
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "null".into())
            )
        };
        let mut p = json!({
            "kind":"process-failed","message":text(&message,16384),
            "runtimeTerminal": if self.terminal.is_none() {"missing"} else if finished {"completed"} else {"failed"},
            "process": self.process(false,false,exit_code)
        });
        if let Some(t) = &thread {
            p["runtimeThreadId"] = json!(t);
        }
        out.push(self.event("run.failed", p));
        out
    }
    #[cfg(any(test, feature = "test-hooks"))]
    #[cfg(any(test, feature = "test-hooks"))]
    pub fn accept_line(&mut self, raw: &str) -> Vec<Event> {
        if self.ended {
            return vec![];
        }
        let cleaned = raw.trim_start_matches('\u{feff}');
        let parsed: Value = match serde_json::from_str(cleaned) {
            Ok(v) => v,
            Err(_) => {
                return vec![self.diagnostic(
                    "warning",
                    "muse.malformed_record",
                    "A Muse Code record could not be parsed.",
                )];
            }
        };
        if !obj(&parsed) {
            return vec![self.diagnostic(
                "warning",
                "muse.malformed_record",
                "A Muse Code record was not an object.",
            )];
        }
        self.accept(&parsed)
    }
    pub fn accept(&mut self, parsed: &Value) -> Vec<Event> {
        if self.ended {
            return vec![];
        }
        let payload_type = s(parsed, "payload_type");
        if payload_type.is_empty() {
            return vec![];
        }
        if self.thread.is_none() {
            self.thread = muse_session_id_of(parsed);
        }
        let payload = if obj(&parsed["payload"]) {
            &parsed["payload"]
        } else {
            &Value::Null
        };
        let evidence = self.evidence(payload_type);

        if payload_type == "runtime.command.accepted"
            || payload_type == "task.stream.linked"
            || payload_type.starts_with("session.")
            || payload_type == "turn.input.user"
            || payload_type == "run.model.configured"
            || payload_type == "tool.result"
            || payload_type == "run.usage"
        {
            return vec![];
        }
        if payload_type == "run.lifecycle.started" {
            if self.run_open {
                return vec![];
            }
            self.run_open = true;
            return vec![self.event(
                "step.started",
                json!({"stepKind":"turn","evidence":evidence}),
            )];
        }
        if payload_type == "run.output.delta" {
            let t = s(payload, "text");
            if t.is_empty() {
                return vec![];
            }
            self.streamed_text.push_str(t);
            return vec![self.event(
                "message.delta",
                json!({"itemId":"msg_0","operation":"append","text":text(t,131072),"final":false,"evidence":evidence}),
            )];
        }
        if payload_type.starts_with("task.lifecycle.") {
            return self.accept_task(payload, evidence);
        }
        if let Some(terminal) = payload_type.strip_prefix("run.terminal.") {
            self.saw_result = true;
            let term = s(payload, "terminal");
            self.terminal = Some(if term.is_empty() {
                terminal.to_owned()
            } else {
                term.to_owned()
            });
            self.terminal_reason = payload["reason"].as_str().map(str::to_owned);
            let mut events = Vec::new();
            let final_text = {
                let t = s(payload, "text");
                if !t.is_empty() {
                    Some(t.to_owned())
                } else if !self.streamed_text.is_empty() {
                    Some(self.streamed_text.clone())
                } else {
                    None
                }
            };
            if let Some(final_text) = final_text
                && !self.message_closed
            {
                self.message_closed = true;
                events.push(self.event(
                        "message.delta",
                        json!({"itemId":"msg_0","operation":"replace","text":text(&final_text,131072),"final":true,"evidence":evidence}),
                    ));
            }
            if self.run_open {
                self.run_open = false;
                events.push(self.event(
                    "step.completed",
                    json!({"stepKind":"turn","evidence":evidence}),
                ));
            }
            return events;
        }
        vec![self.diagnostic(
            "info",
            "muse.unknown_event",
            &format!("Unhandled Muse record: {payload_type}"),
        )]
    }
    fn accept_task(&mut self, payload: &Value, evidence: Value) -> Vec<Event> {
        let event = if obj(&payload["event"]) {
            &payload["event"]
        } else {
            &Value::Null
        };
        let task_id = identity(&event["task_id"])
            .or_else(|| identity(&payload["task_id"]))
            .unwrap_or_default();
        if task_id.is_empty() {
            return vec![];
        }
        let kind = s(event, "kind");
        if kind == "proposed" {
            let tk = s(event, "task_kind");
            if !tk.is_empty() {
                self.task_kinds.insert(task_id, tk.into());
            }
            return vec![];
        }
        if kind == "side_effect_intent" {
            let op = s(event, "operation");
            if !op.is_empty() {
                self.task_operations.insert(task_id, op.into());
            }
            return vec![];
        }
        if kind == "accepted" || kind == "scheduled" {
            return vec![];
        }
        let internal = muse_task_is_internal(self.task_kinds.get(&task_id).map(String::as_str));
        if kind == "started" {
            if internal {
                return vec![];
            }
            self.started_tasks.insert(task_id.clone());
            let mut p = self.tool_row(&task_id);
            p["phase"] = json!("started");
            p["evidence"] = evidence;
            return vec![self.event("tool.started", p)];
        }
        if kind == "completed" {
            if internal {
                return vec![];
            }
            let mut p = self.tool_row(&task_id);
            p["phase"] = json!("completed");
            p["evidence"] = evidence;
            return vec![self.event("tool.completed", p)];
        }
        if kind == "failed" {
            let reason = event["reason"].as_str();
            if internal {
                if reason.is_some_and(|r| {
                    crate::invariants::regex(
                        r"(?i)subscription quota exhausted|usage window resets",
                    )
                    .is_match(r)
                }) {
                    if self.quota_said {
                        return vec![];
                    }
                    self.quota_said = true;
                    return vec![self.event(
                        "route.limit_detected",
                        // Invariant: reason.is_some_and guards this branch.
                        json!({"kind":"quota-exhausted","message":muse_quota_limit(reason.expect("reason.is_some_and guards this branch")),"evidence":evidence}),
                    )];
                }
                let named = self
                    .task_kinds
                    .get(&task_id)
                    .map(String::as_str)
                    .unwrap_or("a task");
                let message = match reason {
                    Some(r) => format!("Muse Code's own {named} did not run: {r}"),
                    None => format!("Muse Code's own {named} did not run."),
                };
                return vec![self.diagnostic("info", "muse.internal_task_failed", &message)];
            }
            let mut p = self.tool_row(&task_id);
            p["phase"] = json!("completed");
            p["evidence"] = evidence.clone();
            if let Some(r) = reason {
                p["status"] = json!(r.chars().take(512).collect::<String>());
            }
            let failed = self.event("tool.failed", p);
            if self.sandbox_said
                || reason.is_none_or(|r| {
                    !crate::invariants::regex(
                        r"(?i)sandbox enforcement unavailable|sandbox users are not ready",
                    )
                    .is_match(r)
                })
            {
                return vec![failed];
            }
            self.sandbox_said = true;
            return vec![
                failed,
                self.diagnostic("warning", "muse.sandbox_not_set_up", MUSE_SANDBOX_SENTENCE),
            ];
        }
        if kind == "output" || kind == "tool_output_ref" {
            return vec![];
        }
        if kind == "cancelled" {
            if internal || !self.started_tasks.contains(&task_id) {
                return vec![];
            }
            let mut p = self.tool_row(&task_id);
            p["phase"] = json!("completed");
            p["evidence"] = evidence;
            let status = match event["reason"].as_str() {
                Some(r) => format!(
                    "Muse Code stopped it ({})",
                    r.chars().take(400).collect::<String>()
                ),
                None => "Muse Code stopped it".into(),
            };
            p["status"] = json!(status);
            return vec![self.event("tool.failed", p)];
        }
        if kind == "rejected" {
            if internal {
                return vec![];
            }
            let mut p = self.tool_row(&task_id);
            p["phase"] = json!("completed");
            p["evidence"] = evidence;
            let status = match event["reason"].as_str() {
                Some(r) => format!(
                    "Muse Code did not run it ({})",
                    r.chars().take(400).collect::<String>()
                ),
                None => "Muse Code did not run it".into(),
            };
            p["status"] = json!(status);
            return vec![self.event("tool.failed", p)];
        }
        if kind == "status" {
            let details = if obj(&event["details"]) {
                &event["details"]
            } else {
                &Value::Null
            };
            let facets = details["facets"].as_array().cloned().unwrap_or_default();
            let attempt = facets.iter().find(|f| s(f, "kind") == "external_attempt");
            let at = attempt.and_then(|a| a["attempt"].as_i64());
            let of = attempt.and_then(|a| a["max_attempts"].as_i64());
            let Some(at) = at.filter(|a| *a >= 2) else {
                return vec![];
            };
            if !self.retries_said.insert(at) {
                return vec![];
            }
            let message = match of {
                Some(of) => format!(
                    "Meta's model did not answer at once, and Muse Code is trying again on its own (attempt {at} of {of})."
                ),
                None => format!(
                    "Meta's model did not answer at once, and Muse Code is trying again on its own (attempt {at})."
                ),
            };
            return vec![self.diagnostic("warning", "muse.provider_busy", &message)];
        }
        vec![self.diagnostic(
            "info",
            "muse.unknown_task_state",
            &format!(
                "Unhandled Muse task state: {}",
                if kind.is_empty() { "none" } else { kind }
            ),
        )]
    }
}

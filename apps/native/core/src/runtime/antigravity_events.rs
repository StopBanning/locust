//! Antigravity's transcript, read as Electron reads it (antigravity-events.ts createAntigravityEventNormalizer).
//!
//! Antigravity writes each conversation step as one JSON line: the person's words (USER_INPUT), its private summary
//! (CHECKPOINT), the host's own messages (SYSTEM_MESSAGE), the model's turns (PLANNER_RESPONSE, with any `tool_calls`)
//! and each call's result (GENERIC). A step that has not settled is written again once it has, so only DONE lines are
//! read, each step once. A planner answer with no tool calls is the turn's end (`latest_final`).
//!
//! The live native driver uses `agy_events` and the agy CLI; this reader is what Electron's Antigravity runtime uses.
use super::Event;
use crate::runtime::events::{evidence_of, evidence_raw, malformed_raw, redact_text, text};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};

fn obj(v: &Value) -> bool {
    v.is_object()
}

/// codex-events.ts stringValue: a non-empty string, NULs dropped.
fn string_value(v: &Value) -> Option<String> {
    let s = v.as_str()?.replace('\0', "");
    (!s.is_empty()).then_some(s)
}

/// codex-events.ts identityValue: a string with something in it, capped at 512 characters.
fn identity_value(v: &Value) -> Option<String> {
    let clean = string_value(v)?;
    if clean.trim().is_empty() {
        return None;
    }
    let chars: Vec<char> = clean.chars().collect();
    Some(if chars.len() > 512 { chars[..511].iter().collect::<String>() + "\u{2026}" } else { clean })
}

/// Lines that are the host's own words coming back, or Antigravity's private summary.
const SILENT_TYPES: [&str; 3] = ["USER_INPUT", "CHECKPOINT", "SYSTEM_MESSAGE"];

/// antigravityToolArg: every value in `tool_calls[].args` is a JSON-encoded string, so it is decoded; a value that does
/// not parse comes back as it arrived.
pub fn tool_arg(args: &Value, key: &str) -> Option<Value> {
    if !obj(args) {
        return None;
    }
    let raw = args.get(key)?;
    match raw.as_str() {
        Some(s) => Some(serde_json::from_str(s).unwrap_or_else(|_| raw.clone())),
        None => Some(raw.clone()),
    }
}

fn decoded_string(args: &Value, key: &str) -> Option<String> {
    tool_arg(args, key).as_ref().and_then(string_value)
}

/// Collapse to a single line: the ledger renders `command` on one row.
fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// antigravityToolCommand: what a call acted on, the command line first, then the file, then the model's phrase.
pub fn tool_command(args: &Value) -> Option<String> {
    let target = ["CommandLine", "TargetFile", "AbsolutePath", "toolAction", "toolSummary", "Description"]
        .iter()
        .find_map(|k| decoded_string(args, k))?;
    let line = one_line(&target);
    (!line.is_empty()).then(|| redact_text(&line))
}

/// antigravityToolTitle: the model's own phrase for a call, when the row leads with something else.
pub fn tool_title(args: &Value, command: Option<&str>) -> Option<String> {
    let phrase = decoded_string(args, "toolAction").or_else(|| decoded_string(args, "toolSummary"))?;
    let line = one_line(&phrase);
    (!line.is_empty() && Some(line.as_str()) != command).then(|| redact_text(&line))
}

/// antigravityBackgroundTask: the task a result line put in the background ("running as a background task with task
/// id: <conv>/task-N"), and what it said it was doing.
pub fn background_task(content: Option<&str>) -> Option<(String, Option<String>)> {
    let content = content?;
    let found = crate::invariants::regex(r"running as a background task with task id:\s*(\S+/task-\d+)").captures(content)?;
    let doing = crate::invariants::regex(r"Task Description:\s*(.+)").captures(content).map(|c| one_line(&c[1]));
    Some((found[1].to_owned(), doing))
}

/// ownBackgroundTask: only a line's own task, the one numbered by its step index. A transcript read with `view_file`
/// quotes other lines' background text.
pub fn own_background_task(task: Option<(String, Option<String>)>, step: Option<i64>) -> Option<(String, Option<String>)> {
    match (task, step) {
        (Some(t), Some(step)) => t.0.ends_with(&format!("/task-{step}")).then_some(t),
        (task, None) => task,
        (None, _) => None,
    }
}

/// antigravityMessageSender: the task a SYSTEM_MESSAGE came from (`sender=<conv>/task-N`).
pub fn message_sender(content: Option<&str>) -> Option<String> {
    crate::invariants::regex(r"sender=(\S+/task-\d+)").captures(content?).map(|c| c[1].to_owned())
}

/// antigravityTaskEnding: how a background task ended, from the SYSTEM_MESSAGE Antigravity sends when it does.
pub fn task_ending(content: Option<&str>) -> Option<(String, &'static str, Option<i64>)> {
    let content = content?;
    let task = crate::invariants::regex(r#"Task id "([^"]+/task-\d+)" (finished|was canceled|failed)"#).captures(content)?;
    if &task[2] == "was canceled" {
        return Some((task[1].to_owned(), "stopped", None));
    }
    let exit_code = crate::invariants::regex(r"The command exited with code (-?\d+)")
        .captures(content)
        .and_then(|c| c[1].parse::<i64>().ok());
    let failed = &task[2] == "failed" || exit_code.is_some_and(|c| c != 0);
    Some((task[1].to_owned(), if failed { "failed" } else { "completed" }, exit_code))
}

/// antigravityWritePatch: a `write_to_file` without `Overwrite` is a new file, its content the added lines; an
/// overwrite reports no before-text, so it gets no patch.
pub fn write_patch(args: &Value) -> Option<Value> {
    if tool_arg(args, "Overwrite") == Some(Value::Bool(true)) {
        return None;
    }
    let target = decoded_string(args, "TargetFile")?;
    let content = decoded_string(args, "CodeContent")?;
    // opencode-events.ts addedFilePatch, with `existed` false.
    let mut lines: Vec<&str> = content.split('\n').collect();
    if lines.len() > 1 && lines.last() == Some(&"") {
        lines.pop();
    }
    let body = lines.iter().map(|l| format!("+{l}")).collect::<Vec<_>>().join("\n");
    super::cursor_events::tool_patch_from(&format!("--- /dev/null\n+++ b/{target}\n@@ -0,0 +1,{} @@\n{body}\n", lines.len()))
}

/// antigravityQuestion: the first question inside an `ask_question` call, its options and whether several may be picked.
pub fn question(args: &Value) -> Option<(String, Vec<String>, bool)> {
    let decoded = tool_arg(args, "questions")?;
    let first = decoded.as_array()?.first()?;
    if !obj(first) {
        return None;
    }
    let asked = string_value(&first["question"])?;
    let options = first["options"]
        .as_array()
        .map(|o| o.iter().filter_map(|v| v.as_str().filter(|s| !s.is_empty()).map(str::to_owned)).collect())
        .unwrap_or_default();
    Some((asked, options, first["is_multi_select"] == true))
}

fn evidence_for(parsed: &Value, kind: &str) -> Value {
    evidence_of(kind, Some(&evidence_raw(parsed)))
}

/// plannerEvidence: the model's reasoning deleted from the kept record, not blanked.
fn planner_evidence(parsed: &Value) -> Value {
    if parsed.get("thinking").is_none() {
        return evidence_for(parsed, "PLANNER_RESPONSE");
    }
    let mut rest = parsed.clone();
    if let Some(o) = rest.as_object_mut() {
        o.remove("thinking");
    }
    let mut evidence = evidence_for(&rest, "PLANNER_RESPONSE");
    evidence["redacted"] = json!(true);
    evidence
}

#[derive(Clone, PartialEq)]
struct OpenTool {
    item_id: String,
    name: String,
    command: Option<String>,
    title: Option<String>,
}

impl OpenTool {
    fn payload(&self) -> Value {
        let mut p = json!({"itemId": self.item_id, "toolKind": self.name, "name": self.name});
        if let Some(c) = &self.command {
            p["command"] = json!(c);
        }
        if let Some(t) = &self.title {
            p["title"] = json!(t);
        }
        p
    }
}

#[derive(Default)]
pub struct TranscriptNormalizer {
    /// The conversation id the host knows (the transcript never repeats it).
    pub thread: Option<String>,
    seen: HashSet<i64>,
    count: usize,
    records: usize,
    pub latest_final: bool,
    pub ended: bool,
    /// Calls opened by a planner line and not yet closed by a result, in order.
    open_tools: Vec<OpenTool>,
    /// Each open call by the step its result will carry: a planner line at step p with n calls is answered at p+1..p+n.
    awaiting: HashMap<i64, OpenTool>,
    /// Results read before the planner line that opened their call, with their evidence.
    early: BTreeMap<i64, (Value, Value)>,
    /// Work put in the background, by task id: the call it came from.
    background: HashMap<String, OpenTool>,
    /// Timers by task id, and the task each waits on; a schedule call's condition by item id until its task is known.
    timer_waits_on: Vec<(String, String)>,
    timer_conditions: HashMap<String, String>,
}

impl TranscriptNormalizer {
    fn event(&mut self, kind: &str, payload: Value) -> Event {
        self.count += 1;
        Event { kind: kind.into(), payload, thread: self.thread.clone() }
    }

    fn diagnostic(&mut self, level: &str, code: &str, message: &str, evidence: Value) -> Event {
        self.event(
            "adapter.diagnostic",
            json!({"level": level, "code": code, "message": message, "terminal": false, "evidence": evidence}),
        )
    }

    fn unattached(&mut self, evidence: Value) -> Event {
        self.diagnostic(
            "info",
            "antigravity.unattached_result",
            "An Antigravity tool result arrived with no tool call open to attach it to.",
            evidence,
        )
    }

    /// The name of a tool that started and never reported back (an `ask_question` waiting on its answer, say).
    pub fn pending_tool_name(&self) -> Option<&str> {
        self.open_tools.last().map(|t| t.name.as_str())
    }

    /// Background tasks started and not yet reported ended: while any is, an answer does not end the turn.
    pub fn pending_background(&self) -> usize {
        self.background.len()
    }

    fn settle(&mut self, task_id: &str, ended: &str, message: Option<&str>, evidence: &Value) -> Vec<Event> {
        let Some(from) = self.background.remove(task_id) else {
            return vec![];
        };
        let mut p = json!({"stepKind": "item", "itemId": from.item_id, "itemType": "background", "status": ended, "evidence": evidence});
        if let Some(m) = message {
            p["message"] = json!(m);
        }
        let mut events = vec![self.event(if ended == "failed" { "step.failed" } else { "step.completed" }, p)];
        // Timers that were only waiting on this task will never fire now.
        let moot: Vec<String> = self.timer_waits_on.iter().filter(|(_, w)| w == task_id).map(|(t, _)| t.clone()).collect();
        self.timer_waits_on.retain(|(_, w)| w != task_id);
        for timer in moot {
            events.extend(self.settle(&timer, "completed", Some("no longer needed: what it waited on finished"), evidence));
        }
        events
    }

    fn close_tool(&mut self, open: OpenTool, parsed: &Value, evidence: Value) -> Vec<Event> {
        if let Some(at) = self.open_tools.iter().position(|t| *t == open) {
            self.open_tools.remove(at);
        }
        let content = string_value(&parsed["content"]);
        if let Some((task_id, doing)) = own_background_task(background_task(content.as_deref()), parsed["step_index"].as_i64()) {
            // The call handed its work to the background; the work is a step of its own until its task says how it ended.
            self.background.insert(task_id.clone(), open.clone());
            let mut moot = false;
            if let Some(watched) = self.timer_conditions.remove(&open.item_id) {
                if self.background.contains_key(&watched) {
                    self.timer_waits_on.push((task_id.clone(), watched));
                } else {
                    moot = true;
                }
            }
            let mut done = open.payload();
            done["background"] = json!(true);
            done["phase"] = json!("completed");
            done["evidence"] = evidence.clone();
            let mut started = json!({"stepKind": "item", "itemId": open.item_id, "itemType": "background", "status": "running", "evidence": evidence});
            if let Some(d) = doing {
                started["message"] = json!(text(&d, 16384));
            }
            let mut events = vec![self.event("tool.completed", done), self.event("step.started", started)];
            if moot {
                events.extend(self.settle(&task_id, "completed", Some("no longer needed: what it waited on had already finished"), &evidence));
            }
            return events;
        }
        let mut done = open.payload();
        if let Some(c) = &content {
            done["output"] = json!(redact_text(c));
        }
        done["phase"] = json!("completed");
        done["evidence"] = evidence;
        vec![self.event("tool.completed", done)]
    }

    fn reasoning(&mut self, events: &mut Vec<Event>, evidence: &Value) {
        // Opened and closed here: the transcript has no line saying the model stopped thinking.
        events.push(self.event("step.started", json!({"stepKind": "reasoning", "evidence": evidence})));
        events.push(self.event("step.completed", json!({"stepKind": "reasoning", "evidence": evidence})));
    }

    fn planner_events(&mut self, parsed: &Value, step: i64) -> Vec<Event> {
        let evidence = planner_evidence(parsed);
        let thinking = string_value(&parsed["thinking"]);
        let calls = parsed["tool_calls"].as_array().cloned().unwrap_or_default();
        let content = string_value(&parsed["content"]);

        if calls.is_empty() && content.is_none() && thinking.is_none() {
            return vec![self.diagnostic(
                "info",
                "antigravity.empty_planner_response",
                "An Antigravity planner line carried neither content, reasoning nor tool calls.",
                evidence,
            )];
        }
        let mut events = Vec::new();
        if calls.is_empty()
            && let Some(answer) = &content
        {
            // The completion signal, even when the model reasoned in the same step.
            self.latest_final = true;
            if thinking.is_some() {
                self.reasoning(&mut events, &evidence);
            }
            events.push(self.event(
                "message.delta",
                json!({"itemId": format!("msg_{step}"), "operation": "replace", "text": text(answer, 131072), "final": true, "evidence": evidence}),
            ));
            return events;
        }

        self.latest_final = false;
        if thinking.is_some() {
            self.reasoning(&mut events, &evidence);
        }
        for (index, call) in calls.iter().enumerate() {
            if !obj(call) {
                continue;
            }
            let name = identity_value(&call["name"]).unwrap_or_else(|| "tool".into());
            let args = &call["args"];
            // No captured call carried an id, so its place in the file is the stable handle.
            let item_id = identity_value(&call["id"])
                .or_else(|| identity_value(&call["call_id"]))
                .or_else(|| identity_value(&call["toolCallId"]))
                .unwrap_or_else(|| format!("tool_{step}_{index}"));
            let asked = if name == "ask_question" { question(args) } else { None };
            let command = match &asked {
                None => tool_command(args),
                Some((q, options, _)) => {
                    Some(one_line(&if options.is_empty() { q.clone() } else { format!("{q} — {}", options.join(" / ")) }))
                }
            };
            let title = if asked.is_none() { tool_title(args, command.as_deref()) } else { None };
            if name == "schedule"
                && let Some(watched) = decoded_string(args, "TimerCondition")
            {
                self.timer_conditions.insert(item_id.clone(), watched);
            }
            let open = OpenTool { item_id, name: name.clone(), command, title };
            self.open_tools.push(open.clone());
            self.awaiting.insert(step + 1 + index as i64, open.clone());
            let mut p = open.payload();
            if name == "write_to_file"
                && let Some(patch) = write_patch(args)
            {
                p["patch"] = patch;
            }
            if let Some((q, options, multi)) = asked {
                p["question"] = json!({"question": q, "options": options, "multiSelect": multi, "askedAtStep": step});
            }
            p["phase"] = json!("started");
            p["evidence"] = evidence.clone();
            events.push(self.event("tool.started", p));
        }
        // A result the file wrote before this line is this line's, if its step says so; one from before this step that
        // nothing claimed never will be.
        let held: Vec<i64> = self.early.keys().copied().collect();
        for result_step in held {
            if let Some(open) = self.awaiting.remove(&result_step) {
                if let Some((parsed, evidence)) = self.early.remove(&result_step) {
                    events.extend(self.close_tool(open, &parsed, evidence));
                }
            } else if result_step < step
                && let Some((_, evidence)) = self.early.remove(&result_step)
            {
                events.push(self.unattached(evidence));
            }
        }
        if let Some(said) = &content {
            // Content beside tool calls is delivered, but not as a final answer.
            events.push(self.event(
                "message.delta",
                json!({"itemId": format!("msg_{step}"), "operation": "replace", "text": text(said, 131072), "final": false, "evidence": evidence}),
            ));
        }
        events
    }

    /// One transcript line, as Electron's `accept` reads it.
    pub fn accept_line(&mut self, raw: &str) -> Vec<Event> {
        if self.ended {
            return vec![];
        }
        self.records += 1;
        let malformed = |raw: &str| {
            let (r, redacted) = malformed_raw(raw);
            json!({"raw": r, "redacted": redacted})
        };
        let parsed: Value = match serde_json::from_str(raw) {
            Ok(v) => v,
            Err(_) => {
                return vec![self.diagnostic(
                    "warning",
                    "antigravity.malformed_record",
                    "An Antigravity transcript line could not be parsed.",
                    malformed(raw),
                )];
            }
        };
        if !obj(&parsed) {
            return vec![self.diagnostic(
                "warning",
                "antigravity.malformed_record",
                "An Antigravity transcript line was not an object.",
                malformed(raw),
            )];
        }
        let typ = string_value(&parsed["type"]);
        let step = parsed["step_index"].as_f64().filter(|n| n.is_finite());
        let (Some(typ), Some(step)) = (typ, step) else {
            return vec![self.diagnostic(
                "warning",
                "antigravity.malformed_record",
                "An Antigravity transcript line had no step_index or no type.",
                malformed(raw),
            )];
        };
        let step = step as i64;
        if self.seen.contains(&step) {
            return vec![];
        }
        // A step not yet finished is written again with its result, except a result that handed its work to the
        // background: that line is never rewritten, so it is read as it stands.
        let status = string_value(&parsed["status"]);
        let content = string_value(&parsed["content"]);
        let handed_off = typ == "GENERIC"
            && status.as_deref() == Some("RUNNING")
            && own_background_task(background_task(content.as_deref()), Some(step)).is_some();
        if status.as_deref() != Some("DONE") && !handed_off {
            return vec![];
        }
        self.seen.insert(step);

        if SILENT_TYPES.contains(&typ.as_str()) {
            // A new user or system turn means the previous answer no longer ends the conversation.
            self.latest_final = false;
            if typ != "SYSTEM_MESSAGE" {
                return vec![];
            }
            let evidence = evidence_for(&parsed, &typ);
            if let Some((task_id, ended, exit_code)) = task_ending(content.as_deref()) {
                let message = exit_code.map(|c| format!("exited with code {c}"));
                return self.settle(&task_id, ended, message.as_deref(), &evidence);
            }
            // Any other message from a background task is a timer firing.
            let Some(sender) = message_sender(content.as_deref()).filter(|s| self.background.contains_key(s)) else {
                return vec![];
            };
            self.timer_waits_on.retain(|(t, _)| *t != sender);
            return self.settle(&sender, "completed", Some("timer went off"), &evidence);
        }

        if typ == "PLANNER_RESPONSE" {
            return self.planner_events(&parsed, step);
        }

        if typ == "GENERIC" && string_value(&parsed["source"]).as_deref() == Some("MODEL") {
            self.latest_final = false;
            let evidence = evidence_for(&parsed, &typ);
            let Some(open) = self.awaiting.remove(&step) else {
                // Its planner line may not be read yet; that line settles it.
                self.early.insert(step, (parsed, evidence));
                return vec![];
            };
            return self.close_tool(open, &parsed, evidence);
        }

        self.latest_final = false;
        let evidence = evidence_for(&parsed, &typ);
        let message = format!("Unhandled Antigravity transcript line: {}", redact_text(&typ));
        vec![self.diagnostic("info", "antigravity.unknown_step", &message, evidence)]
    }

    fn process(&self, cancelled: bool, exit_code: Option<i32>) -> Value {
        json!({
            "exitCode": exit_code.map(Value::from).unwrap_or(Value::Null),
            "signal": null, "stderr": "", "stderrTruncated": false,
            "recordCount": self.records, "inputDeliveryFailed": false, "outputLimitExceeded": false,
            "oversizedRecordsDropped": 0, "forcedTerminationAttempted": cancelled,
            "terminationUnconfirmed": false,
            "startedAt": crate::ledger_write::now(), "finishedAt": crate::ledger_write::now()
        })
    }

    /// The run's end: held results nothing claimed are said, then cancelled, failed (a host driver that died, or a
    /// transcript that never ended on an answer) or completed.
    pub fn finish(&mut self, cancelled: bool, exit_code: Option<i32>) -> Vec<Event> {
        if self.ended {
            return vec![];
        }
        self.ended = true;
        let process = self.process(cancelled, exit_code);
        let held: Vec<Value> = std::mem::take(&mut self.early).into_values().map(|(_, e)| e).collect();
        let mut events: Vec<Event> = held.into_iter().map(|e| self.unattached(e)).collect();
        let mut p = json!({"process": process});
        if let Some(t) = &self.thread {
            p["runtimeThreadId"] = json!(t);
        }
        if cancelled {
            events.push(self.event("run.cancelled", p));
            return events;
        }
        if let Some(code) = exit_code.filter(|c| *c != 0) {
            p["kind"] = json!("process-failed");
            p["message"] = json!(format!("The Antigravity language server exited with code {code}."));
            p["runtimeTerminal"] = json!(if self.latest_final { "completed" } else { "missing" });
            events.push(self.event("run.failed", p));
            return events;
        }
        if !self.latest_final {
            p["kind"] = json!("process-failed");
            p["message"] = json!(
                "The Antigravity transcript never ended on a planner answer with no tool calls, so the host stopped waiting for one."
            );
            p["runtimeTerminal"] = json!("missing");
            events.push(self.event("run.failed", p));
            return events;
        }
        events.push(self.event("run.completed", p));
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lines(name: &str) -> Vec<String> {
        std::fs::read_to_string(format!("{}/../scripts/fixtures/antigravity/{name}", env!("CARGO_MANIFEST_DIR")))
            .unwrap()
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_owned)
            .collect()
    }
    #[test]
    fn plain_reply_fixture_yields_final_message() {
        let mut n = TranscriptNormalizer::default();
        let mut events = Vec::new();
        for line in lines("transcript-plain-reply.jsonl") {
            events.extend(n.accept_line(&line));
        }
        assert!(n.latest_final);
        assert!(events.iter().any(|e| e.kind == "message.delta" && e.payload["final"] == true));
        assert_eq!(n.finish(false, Some(0)).last().unwrap().kind, "run.completed");
    }

    /// antigravity-events.ts: a result written before its planner line is held for that line, and a background task's
    /// row stays open until Antigravity says how it ended.
    #[test]
    fn an_early_result_waits_for_its_call_and_background_work_settles_on_its_notice() {
        let mut n = TranscriptNormalizer::default();
        let call = json!({"step_index": 4, "type": "PLANNER_RESPONSE", "status": "DONE", "source": "MODEL",
            "tool_calls": [{"name": "run_command", "args": {"CommandLine": "\"npm test\""}}]});
        let result = json!({"step_index": 5, "type": "GENERIC", "status": "RUNNING", "source": "MODEL",
            "content": "Tool is running as a background task with task id: c1/task-5\nTask Description: tests"});
        assert!(n.accept_line(&result.to_string()).is_empty());
        let events = n.accept_line(&call.to_string());
        let kinds: Vec<_> = events.iter().map(|e| e.kind.as_str()).collect();
        assert_eq!(kinds, ["tool.started", "tool.completed", "step.started"]);
        assert_eq!(events[0].payload["command"], "npm test");
        assert_eq!(n.pending_background(), 1);
        let notice = json!({"step_index": 6, "type": "SYSTEM_MESSAGE", "status": "DONE", "source": "SYSTEM",
            "content": "Task id \"c1/task-5\" finished with result: ok. The command exited with code 1."});
        let settled = n.accept_line(&notice.to_string());
        assert_eq!(settled[0].kind, "step.failed");
        assert_eq!(settled[0].payload["message"], "exited with code 1");
        assert_eq!(n.pending_background(), 0);
    }

    #[test]
    fn an_ask_question_names_its_question_and_options() {
        let args = json!({"questions": "[{\"question\":\"Which?\",\"options\":[\"A\",\"B\"],\"is_multi_select\":false}]"});
        assert_eq!(question(&args), Some(("Which?".into(), vec!["A".into(), "B".into()], false)));
        let mut n = TranscriptNormalizer::default();
        let line = json!({"step_index": 2, "type": "PLANNER_RESPONSE", "status": "DONE", "tool_calls": [{"name": "ask_question", "args": args}]});
        let events = n.accept_line(&line.to_string());
        assert_eq!(events[0].payload["command"], "Which? — A / B");
        assert_eq!(events[0].payload["question"]["askedAtStep"], 2);
        assert_eq!(n.pending_tool_name(), Some("ask_question"));
    }
}

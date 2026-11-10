//! Antigravity CLI (agy) stream-json -> ledger events. Ports createAgyEventNormalizer (agy-events.ts).
use super::Event;
use crate::runtime::events::text;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

pub const QUOTA_SENTENCE: &str = "Antigravity's quota is used up for now, so it stopped before finishing. Pick another model to carry on, or send again once it resets.";
pub const STALLED_SENTENCE: &str = "Antigravity's model stopped answering for five minutes, so Antigravity ended the turn. What it did so far is kept. Antigravity says this can be tried again: send \"continue\".";
pub const REFUSED_SENTENCE: &str = "Google refused this turn's next request as invalid, after it had already worked; what it did so far is kept. Send \"carry on\" to go on from there. If Google refuses again, start a new conversation and say what is already done.";

fn s<'a>(v: &'a Value, key: &str) -> &'a str { v[key].as_str().unwrap_or("") }
fn identity(v: &Value, key: &str) -> String { s(v, key).replace('\0',"").chars().take(512).collect() }
fn obj(v: &Value) -> bool { v.is_object() }

pub fn tool_target(parameters: &Value) -> Option<String> {
    if !obj(parameters) { return None; }
    for key in ["CommandLine","AbsolutePath","TargetFile","Url","Query","query","SearchPath","DirectoryPath","Pattern"] {
        if let Some(v) = parameters[key].as_str().filter(|t| !t.trim().is_empty()) { return Some(v.to_owned()); }
    }
    None
}

pub fn denied_sentence(actions: &[String]) -> Option<String> {
    if actions.is_empty() { return None; }
    let reads = actions.iter().any(|a| a.to_ascii_lowercase().contains("read"));
    let writes = actions.iter().any(|a| {
        let l = a.to_ascii_lowercase();
        l.contains("write") || l.contains("edit") || (l.contains("file") && !l.contains("read"))
    });
    let commands = actions.iter().any(|a| a.to_ascii_lowercase().contains("command"));
    Some(if reads && !writes && !commands {
        "Antigravity was not allowed to read a file outside this conversation's folder in this mode. Move the file into the folder, or choose Auto to let it.".into()
    } else if commands && !writes && !reads {
        "Antigravity was not allowed to run a command in this mode. Edit lets it change files but not run commands; Auto lets it do both.".into()
    } else if writes && !commands && !reads {
        "Antigravity was not allowed to change files in this mode. Choose Edit or Auto to let it.".into()
    } else {
        format!("Antigravity was not allowed to do everything it tried in this mode ({}). Choose Auto to let it.", actions.join(", "))
    })
}

pub fn quota_limit(error: &str, now_ms: i64) -> Option<String> {
    if !regex::Regex::new(r"(?i)quota reached|RESOURCE_EXHAUSTED").unwrap().is_match(error) { return None; }
    let re = regex::Regex::new(r"(?i)resets in\s+(?:(\d+)h)?\s*(?:(\d+)m)?\s*(?:(\d+)s)?").unwrap();
    let Some(c) = re.captures(error) else { return Some("Antigravity's quota is used up".into()); };
    let seconds = c.get(1).and_then(|m| m.as_str().parse::<i64>().ok()).unwrap_or(0) * 3600
        + c.get(2).and_then(|m| m.as_str().parse::<i64>().ok()).unwrap_or(0) * 60
        + c.get(3).and_then(|m| m.as_str().parse::<i64>().ok()).unwrap_or(0);
    if seconds <= 0 { return Some("Antigravity's quota is used up".into()); }
    let when = chrono::DateTime::from_timestamp_millis(now_ms + seconds * 1000)
        .map(|d| d.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
        .unwrap_or_default();
    Some(format!("Antigravity's quota is used up · resets {when}"))
}

#[derive(Clone)]
struct Held {
    item_id: String,
    tool_kind: String,
    name: String,
    command: Option<String>,
    evidence: Value,
    duration_ms: Option<i64>,
}

pub struct AgyNormalizer {
    pub thread: Option<String>,
    pub ended: bool,
    pub(crate) saw_result: bool,
    count: usize,
    turn_open: bool,
    status: Option<String>,
    result_error: Option<String>,
    answered_then_unavailable: bool,
    answer_closed: bool,
    usage: Option<Value>,
    last_message_item: Option<String>,
    spoken_text: HashMap<String, String>,
    open_tools: HashSet<i64>,
    spoken: HashSet<i64>,
    held: Vec<Held>,
    sandbox: String,
}

impl Default for AgyNormalizer {
    fn default() -> Self {
        Self {
            thread: None, ended: false, saw_result: false, count: 0, turn_open: false,
            status: None, result_error: None, answered_then_unavailable: false, answer_closed: false,
            usage: None, last_message_item: None, spoken_text: HashMap::new(),
            open_tools: HashSet::new(), spoken: HashSet::new(), held: Vec::new(),
            sandbox: "read-only".into(),
        }
    }
}

impl AgyNormalizer {
    pub fn with_sandbox(mut self, sandbox: &str) -> Self { self.sandbox = sandbox.into(); self }
    fn event(&mut self, kind: &str, payload: Value) -> Event {
        self.count += 1;
        Event { kind: kind.into(), payload, thread: self.thread.clone() }
    }
    fn process(&self, success: bool, cancelled: bool, exit_code: Option<i32>) -> Value {
        json!({
            "exitCode": exit_code.map(Value::from).unwrap_or(Value::Null),
            "signal": null, "stderr": "", "stderrTruncated": false,
            "recordCount": self.count, "inputDeliveryFailed": false, "outputLimitExceeded": false,
            "oversizedRecordsDropped": 0, "forcedTerminationAttempted": cancelled,
            "terminationUnconfirmed": !success && !cancelled,
            "startedAt": crate::ledger_write::now(), "finishedAt": crate::ledger_write::now()
        })
    }
    fn evidence(&self, t: &str) -> Value { json!({"runtimeEventType": t, "redacted": true}) }
    pub fn diagnostic(&mut self, level: &str, code: &str, message: &str) -> Event {
        self.event("adapter.diagnostic", json!({"level":level,"code":code,"message":text(message,16384),"terminal":false,"evidence":self.evidence("antigravity")}))
    }
    fn is_command(kind: &str) -> bool { regex::Regex::new(r"(?i)command|shell|terminal").unwrap().is_match(kind) }
    fn is_write(kind: &str) -> bool { regex::Regex::new(r"(?i)write|edit|replace|create|delete|move").unwrap().is_match(kind) }
    fn own_file(cmd: &Option<String>) -> bool {
        cmd.as_ref().is_some_and(|c| regex::Regex::new(r"[\\/]\.gemini[\\/]antigravity[^\\/]*[\\/]brain[\\/]").unwrap().is_match(c))
    }
    fn may_be_refused(&self, kind: &str, cmd: &Option<String>) -> bool {
        if self.sandbox == "full-access" { return false; }
        if Self::is_command(kind) { return true; }
        self.sandbox != "workspace-write" && Self::is_write(kind) && !Self::own_file(cmd)
    }
    fn release_held(&mut self, denied: &[String]) -> Vec<Event> {
        let calls = std::mem::take(&mut self.held);
        let mut refused = HashSet::new();
        for action in denied {
            let lower = action.to_ascii_lowercase();
            let check_cmd = lower.contains("command");
            let check_write = regex::Regex::new(r"(?i)write|edit|file").unwrap().is_match(action);
            for (i, call) in calls.iter().enumerate().rev() {
                let match_kind = if check_cmd { Self::is_command(&call.tool_kind) } else if check_write { Self::is_write(&call.tool_kind) } else { false };
                if match_kind && !refused.contains(&i) && !Self::own_file(&call.command) {
                    refused.insert(i);
                    break;
                }
            }
        }
        let mut out = Vec::new();
        for (i, call) in calls.into_iter().enumerate() {
            let mut p = json!({"itemId":call.item_id,"toolKind":call.tool_kind,"name":call.name,"phase":"completed","evidence":call.evidence});
            if let Some(c)=&call.command { p["command"]=json!(text(c,16384)); }
            if let Some(d)=call.duration_ms { p["durationMs"]=json!(d); }
            if refused.contains(&i) {
                p["output"]=json!("Not allowed in this mode.");
                p["status"]=json!("refused");
                out.push(self.event("tool.failed", p));
            } else {
                out.push(self.event("tool.completed", p));
            }
        }
        out
    }
    pub fn finish(&mut self, cancelled: bool, exit_code: Option<i32>) -> Vec<Event> {
        let mut ending = self.release_held(&[]);
        self.ended = true;
        let thread = self.thread.clone();
        if cancelled {
            let mut p = json!({"process": self.process(false,true,exit_code)});
            if let Some(t)=&thread { p["runtimeThreadId"]=json!(t); }
            ending.push(self.event("run.cancelled", p));
            return ending;
        }
        let success = (self.status.as_deref() == Some("SUCCESS") && exit_code == Some(0)) || self.answered_then_unavailable;
        if success {
            let mut p = json!({"process": self.process(true,false,exit_code)});
            if let Some(t)=&thread { p["runtimeThreadId"]=json!(t); }
            if let Some(u)=&self.usage { p["usage"]=u.clone(); }
            ending.push(self.event("run.completed", p));
            return ending;
        }
        let err = self.result_error.clone().unwrap_or_default();
        let lost = regex::Regex::new(r"(?i)trajectory not found").unwrap().is_match(&err);
        let now_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
        let quota = if err.is_empty() { None } else { quota_limit(&err, now_ms) };
        if let Some(q) = &quota {
            ending.push(self.event("route.limit_detected", json!({"kind":"quota-exhausted","message":q,"evidence":self.evidence("result")})));
        }
        let message = if lost {
            "Antigravity CLI does not have this conversation: it began in the Antigravity app. Send it again and it starts fresh, with the conversation so far.".into()
        } else if self.status.is_none() {
            "Antigravity ended without a record saying the run had finished.".into()
        } else if quota.is_some() {
            QUOTA_SENTENCE.into()
        } else if regex::Regex::new(r"(?i)stream idle timeout|DEADLINE_EXCEEDED|context deadline exceeded").unwrap().is_match(&err) {
            STALLED_SENTENCE.into()
        } else if regex::Regex::new(r"(?i)INVALID_ARGUMENT|Request contains an invalid argument").unwrap().is_match(&err) {
            REFUSED_SENTENCE.into()
        } else if !err.is_empty() {
            format!("Antigravity could not run it: {}", err.lines().next().unwrap_or(&err))
        } else {
            format!("Antigravity ended {}{}.", self.status.as_deref().unwrap_or("unknown").to_ascii_lowercase(),
                exit_code.filter(|c| *c != 0).map(|c| format!(" (exit code {c})")).unwrap_or_default())
        };
        let mut p = json!({
            "kind":"process-failed","message":text(&message,16384),
            "runtimeTerminal": if self.status.is_none() {"missing"} else if self.status.as_deref()==Some("SUCCESS") {"completed"} else {"failed"},
            "process": self.process(false,false,exit_code)
        });
        if lost { p["sessionEnded"]=json!(true); }
        if let Some(t)=&thread { p["runtimeThreadId"]=json!(t); }
        ending.push(self.event("run.failed", p));
        ending
    }
    pub fn accept_line(&mut self, raw: &str) -> Vec<Event> {
        if self.ended { return vec![]; }
        let cleaned = raw.trim_start_matches('\u{feff}');
        let parsed: Value = match serde_json::from_str(cleaned) {
            Ok(v)=>v,
            Err(_)=> return vec![self.diagnostic("warning","antigravity.malformed_record","An Antigravity CLI record could not be parsed.")],
        };
        if !obj(&parsed) {
            return vec![self.diagnostic("warning","antigravity.malformed_record","An Antigravity CLI record was not an object.")];
        }
        self.accept(&parsed)
    }
    pub fn accept(&mut self, parsed: &Value) -> Vec<Event> {
        if self.ended { return vec![]; }
        let kind = s(parsed, "event");
        if kind.is_empty() { return vec![]; }
        let evidence = self.evidence(kind);
        if kind == "init" {
            let sid = identity(parsed, "conversation_id");
            if !sid.is_empty() { self.thread = Some(sid); }
            if self.turn_open { return vec![]; }
            self.turn_open = true;
            return vec![
                self.event("run.started", json!({"runtimeThreadId": self.thread.clone().unwrap_or_default(),"evidence":evidence})),
                self.event("step.started", json!({"stepKind":"turn","evidence":evidence})),
            ];
        }
        if kind == "step_update" {
            let step = if obj(&parsed["step_update"]) { &parsed["step_update"] } else { return vec![]; };
            let cid = identity(step, "conversation_id");
            if !cid.is_empty() { self.thread = Some(cid); }
            return self.accept_step(step, evidence);
        }
        if kind == "result" {
            return self.accept_result(parsed, evidence);
        }
        vec![self.diagnostic("info","antigravity.unknown_event",&format!("Unhandled Antigravity CLI record: {kind}"))]
    }
    fn accept_result(&mut self, parsed: &Value, evidence: Value) -> Vec<Event> {
        self.saw_result = true;
        let result = if obj(&parsed["result"]) { &parsed["result"] } else { &Value::Null };
        let cid = identity(result, "conversation_id");
        if !cid.is_empty() { self.thread = Some(cid); }
        let status = s(result, "status");
        self.status = if status.is_empty() {
            Some("UNKNOWN".into())
        } else {
            Some(status.into())
        };
        self.result_error = result["error"].as_str().map(str::to_owned);
        if obj(&result["usage"]) {
            let u = &result["usage"];
            if let (Some(i), Some(o)) = (u["input_tokens"].as_f64(), u["output_tokens"].as_f64()) {
                self.usage = Some(json!({"inputTokens":i,"outputTokens":o}));
            }
        }
        let mut events = Vec::new();
        let response = result["response"].as_str().map(str::to_owned);
        let mut whole = response.clone();
        if let Some(ref mut left) = whole {
            let last = self.last_message_item.clone();
            for (item_id, words) in self.spoken_text.clone() {
                if Some(&item_id) == last.as_ref() { continue; }
                let earlier = words.trim().to_owned();
                let rest = left.trim_start().to_owned();
                if !earlier.is_empty() && rest.starts_with(&earlier) {
                    *left = rest[earlier.len()..].to_owned();
                }
            }
        }
        let denied: Vec<String> = result["denied_actions"].as_array().into_iter().flatten()
            .filter_map(|e| e["action"].as_str().map(str::to_owned)).collect();
        events.extend(self.release_held(&denied));
        if let Some(whole) = whole {
            if !whole.trim().is_empty() {
                let item = self.last_message_item.clone().unwrap_or_else(|| "msg_final".into());
                events.push(self.event("message.delta", json!({
                    "itemId": item, "operation":"replace", "text": text(whole.trim_start(), 131072), "final": true, "evidence": evidence
                })));
            }
        }
        let stopped = response.as_ref().map(|r| r.trim().is_empty()).unwrap_or(true);
        if let Some(said) = denied_sentence(&denied) {
            let msg = if stopped { format!("It stopped there. {said}") } else { said };
            events.push(self.diagnostic("warning","antigravity.denied_actions",&msg));
        }
        let err = self.result_error.clone().unwrap_or_default();
        let status = self.status.clone().unwrap_or_default();
        if status != "SUCCESS" && !stopped && regex::Regex::new(r"\bUNAVAILABLE\b|\b503\b").unwrap().is_match(&err) {
            self.answered_then_unavailable = true;
            events.push(self.diagnostic("warning","antigravity.unavailable_after_answer",
                "Google's service was unavailable for Antigravity's last call, after this answer (code 503). The answer and its changes are kept. If it was not finished, send \"continue\"."));
        }
        if status != "SUCCESS" && !stopped && !self.answered_then_unavailable && self.answer_closed
            && regex::Regex::new(r"(?i)stream was interrupted").unwrap().is_match(&err)
        {
            self.answered_then_unavailable = true;
            events.push(self.diagnostic("warning","antigravity.interrupted_after_answer",
                "Antigravity said its stream was interrupted after this answer had finished. The answer and its changes are kept. If it was not finished, send \"continue\"."));
        }
        if self.turn_open {
            self.turn_open = false;
            events.push(self.event("step.completed", json!({"stepKind":"turn","evidence":evidence})));
        }
        events
    }
    fn accept_step(&mut self, step: &Value, evidence: Value) -> Vec<Event> {
        let index = step["step_index"].as_i64();
        let type_name = s(step, "step_type");
        let state = s(step, "state");
        let Some(index) = index else { return vec![]; };
        if type_name.is_empty() { return vec![]; }
        if type_name == "user_input" { return vec![]; }
        let text_delta = s(step, "text_delta");
        self.answer_closed = type_name == "agent_response" && state == "DONE" && (self.spoken.contains(&index) || !text_delta.is_empty());
        if type_name == "agent_response" {
            if text_delta.is_empty() && state == "DONE" && !self.spoken.contains(&index) {
                let used = step["usage"]["thinking_tokens"].as_f64().unwrap_or(0.0);
                let seconds = step["duration_seconds"].as_f64();
                if used <= 0.0 || seconds.is_none() { return vec![]; }
                let item_id = format!("thought_{index}");
                let ms = (seconds.unwrap() * 1000.0).round() as i64;
                return vec![
                    self.event("step.started", json!({"stepKind":"reasoning","itemId":item_id,"evidence":evidence})),
                    self.event("step.completed", json!({"stepKind":"reasoning","itemId":item_id,"durationMs":ms,"evidence":evidence})),
                ];
            }
            if text_delta.is_empty() { return vec![]; }
            self.spoken.insert(index);
            let item_id = format!("msg_{index}");
            self.last_message_item = Some(item_id.clone());
            self.spoken_text.entry(item_id.clone()).and_modify(|t| t.push_str(text_delta)).or_insert(text_delta.to_owned());
            return vec![self.event("message.delta", json!({"itemId":item_id,"operation":"append","text":text(text_delta,131072),"final":false,"evidence":evidence}))];
        }
        if type_name == "tool" {
            let name = { let n=s(step,"tool_name"); if n.is_empty(){"tool".into()} else {n.to_owned()} };
            let info = if obj(&step["tool_info"]) { &step["tool_info"] } else { &Value::Null };
            let target = tool_target(&info["parameters"]);
            let item_id = format!("tool_{index}");
            let mut events = Vec::new();
            if state == "ACTIVE" {
                if self.open_tools.insert(index) {
                    let mut p = json!({"itemId":item_id,"toolKind":name,"name":name,"phase":"started","evidence":evidence});
                    if let Some(t)=&target { p["command"]=json!(text(t,16384)); }
                    events.push(self.event("tool.started", p));
                }
                return events;
            }
            if !self.open_tools.contains(&index) {
                let mut p = json!({"itemId":item_id,"toolKind":name,"name":name,"phase":"started","evidence":evidence});
                if let Some(t)=&target { p["command"]=json!(text(t,16384)); }
                events.push(self.event("tool.started", p));
            }
            self.open_tools.remove(&index);
            let output = info["output"].as_str().map(str::to_owned);
            let seconds = step["duration_seconds"].as_f64();
            let duration_ms = seconds.map(|s| (s * 1000.0).round() as i64);
            let error_text = info["error"]["message"].as_str().map(str::to_owned);
            let refused = error_text.as_ref().is_some_and(|e| regex::Regex::new(r"(?i)permission check failed|permission denied|not allowed|denied").unwrap().is_match(e));
            if state == "DONE" && error_text.is_none() && output.is_none() && self.may_be_refused(&name, &target) {
                self.held.push(Held { item_id, tool_kind: name.clone(), name, command: target, evidence, duration_ms });
                return events;
            }
            let mut p = json!({"itemId":item_id,"toolKind":name,"name":name,"phase":"completed","evidence":evidence});
            if let Some(t)=&target { p["command"]=json!(text(t,16384)); }
            if let Some(o)=&output { p["output"]=json!(text(o,16384)); }
            if let Some(d)=duration_ms { p["durationMs"]=json!(d); }
            if state == "DONE" {
                events.push(self.event("tool.completed", p));
            } else {
                if let Some(e)=&error_text { if output.is_none() { p["output"]=json!(text(if refused {"Not allowed in this mode."} else {e},16384)); } }
                p["status"]=json!(if refused {"refused"} else {"failed"});
                events.push(self.event("tool.failed", p));
            }
            return events;
        }
        vec![]
    }
}

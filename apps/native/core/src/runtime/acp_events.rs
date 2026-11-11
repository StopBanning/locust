//! ACP work updates. Raw frames and hidden thoughts never enter the ledger (PLAN.md).
use super::{Event, events::text};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct AcpNormalizer {
    pub thread: Option<String>,
    pub ended: bool,
    pub started_at: String,
    pub records: u64,
    started: bool,
    ordinal: usize,
    message: Option<(String, Option<String>, String)>,
    segments: HashMap<String, usize>,
    tools: HashMap<String, Value>,
    declined: HashSet<String>,
    unknown: HashSet<String>,
    thinking: bool,
}
fn string(v: &Value) -> Option<&str> {
    v.as_str().filter(|s| !s.is_empty())
}
pub fn content_text(v: &Value) -> Option<String> {
    if let Some(items) = v.as_array() {
        let texts: Vec<_> = items
            .iter()
            .filter_map(|x| content_text(x.get("content").unwrap_or(x)))
            .collect();
        return (!texts.is_empty()).then(|| texts.join("\n"));
    }
    (v["type"] == "text")
        .then(|| v["text"].as_str().map(str::to_owned))
        .flatten()
}
fn naming(kind: &str, title: &str) -> (String, String) {
    let word = title.trim().to_lowercase();
    let plain = !word.is_empty()
        && word.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && word
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(&b));
    let k = if (kind.is_empty() || kind == "other") && plain {
        word.as_str()
    } else {
        kind
    };
    let (kind, name) = match k {
        "execute" => ("command_execution", "bash"),
        "edit" | "delete" | "move" => ("file_change", k),
        "fetch" => ("web_fetch", "webfetch"),
        "" => ("tool", "tool"),
        _ => (k, k),
    };
    (kind.into(), name.into())
}
impl AcpNormalizer {
    pub fn event(&self, kind: &str, mut payload: Value) -> Event {
        if payload.get("evidence").is_none()
            && !matches!(kind, "run.completed" | "run.failed" | "run.cancelled")
        {
            payload["evidence"] = json!({"runtimeEventType":kind,"redacted":true});
        }
        for field in ["itemId", "name", "toolKind"] {
            if let Some(v) = payload[field].as_str() {
                payload[field] = json!(text(v, 512));
            }
        }
        Event {
            kind: kind.into(),
            payload,
            thread: self.thread.clone(),
        }
    }
    pub fn diagnostic(&self, code: &str, message: &str) -> Event {
        self.event("adapter.diagnostic", json!({"level":"warning","code":code,"message":text(message,16384),"terminal":false,"evidence":{"runtimeEventType":code,"redacted":true}}))
    }
    fn start(&mut self, out: &mut Vec<Event>) {
        if !self.started {
            self.started = true;
            out.push(self.event(
                "run.started",
                json!({"runtimeThreadId":self.thread.clone().unwrap_or_default()}),
            ));
        }
    }
    fn close_message(&mut self, out: &mut Vec<Event>) {
        if let Some((id, _, said)) = self.message.take()
            && !said.is_empty()
        {
            out.push(self.event(
                "message.delta",
                json!({"itemId":id,"operation":"replace","text":text(&said,131072),"final":true}),
            ));
        }
    }
    fn close_thought(&mut self, out: &mut Vec<Event>) {
        if self.thinking {
            self.thinking = false;
            out.push(self.event(
                "step.completed",
                json!({"stepKind":"reasoning","itemId":format!("reasoning_{}",self.ordinal)}),
            ));
        }
    }
    pub fn decline(&mut self, id: &str) {
        self.records += 1;
        self.declined.insert(id.into());
    }
    pub fn update(&mut self, v: &Value) -> Vec<Event> {
        let kind = v["sessionUpdate"].as_str().unwrap_or("");
        if kind != "available_commands_update" {
            self.records += 1;
        }
        if self.ended
            || matches!(
                kind,
                "" | "available_commands_update"
                    | "user_message_chunk"
                    | "session_info_update"
                    | "config_option_update"
                    | "current_mode_update"
                    | "usage_update"
            )
        {
            return vec![];
        }
        let mut out = vec![];
        if kind == "agent_thought_chunk" {
            self.start(&mut out);
            self.close_message(&mut out);
            if !self.thinking {
                self.thinking = true;
                self.ordinal += 1;
                out.push(self.event(
                    "step.started",
                    json!({"stepKind":"reasoning","itemId":format!("reasoning_{}",self.ordinal)}),
                ));
            }
            return out;
        }
        if kind == "agent_message_chunk" {
            let Some(said) = content_text(&v["content"]).filter(|t| !t.is_empty()) else {
                return out;
            };
            self.start(&mut out);
            self.close_thought(&mut out);
            let agent = string(&v["messageId"]).map(str::to_owned);
            if self
                .message
                .as_ref()
                .is_some_and(|(_, held, _)| agent.is_some() && *held != agent)
            {
                self.close_message(&mut out);
            }
            if self.message.is_none() {
                let id = if let Some(a) = &agent {
                    let n = self.segments.entry(a.clone()).or_default();
                    *n += 1;
                    if *n == 1 {
                        a.clone()
                    } else {
                        format!("{a}:{n}")
                    }
                } else {
                    self.ordinal += 1;
                    format!("message_{}", self.ordinal)
                };
                self.message = Some((id, agent, String::new()));
            }
            let (id, _, held) = self
                .message
                .as_mut()
                // Invariant: message inserted immediately above.
                .expect("message inserted immediately above");
            held.push_str(&said);
            *held = text(held, 131072);
            let id = id.clone();
            out.push(self.event(
                "message.delta",
                json!({"itemId":id,"operation":"append","text":text(&said,131072),"final":false}),
            ));
            return out;
        }
        if kind == "plan" {
            self.start(&mut out);
            self.close_thought(&mut out);
            self.close_message(&mut out);
            let plan: Vec<_>=v["entries"].as_array().into_iter().flatten().take(100).map(|e|json!({"content":text(e["content"].as_str().unwrap_or(""),16384),"status":e["status"]})).collect();
            out.push(self.event(
                "plan.updated",
                json!({"itemId":"plan","plan":plan,"final":false}),
            ));
            return out;
        }
        if matches!(kind, "tool_call" | "tool_call_update") {
            let Some(id) = string(&v["toolCallId"]) else {
                return out;
            };
            self.start(&mut out);
            self.close_thought(&mut out);
            let title = v["title"].as_str().unwrap_or("");
            let (tk, name) = naming(v["kind"].as_str().unwrap_or(""), title);
            let is_new = !self.tools.contains_key(id);
            if is_new {
                self.close_message(&mut out);
                self.tools
                    .insert(id.into(), json!({"itemId":id,"toolKind":tk,"name":name}));
            }
            let open = self
                .tools
                .get_mut(id)
                // Invariant: tool inserted immediately above when absent.
                .expect("tool inserted immediately above when absent");
            if matches!(open["name"].as_str(), Some("other" | "tool")) {
                open["toolKind"] = json!(tk);
                open["name"] = json!(name);
            }
            let shell = v["kind"] == "execute"
                || open["toolKind"] == "command_execution"
                || matches!(
                    open["name"].as_str(),
                    Some("bash" | "shell" | "powershell" | "pwsh" | "cmd")
                );
            let command = string(&v["rawInput"]["command"])
                .map(str::to_owned)
                .or_else(|| {
                    if shell {
                        return None;
                    }
                    let paths: Vec<_> = v["locations"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|l| string(&l["path"]))
                        .collect();
                    (!paths.is_empty()).then(|| paths.join("\n"))
                });
            if let Some(c) = command {
                open["command"] = json!(text(&c, 16384));
            }
            if !title.is_empty()
                && title != open["name"].as_str().unwrap_or("")
                && title != open["command"].as_str().unwrap_or("")
            {
                open["title"] = json!(text(title, 16384));
            }
            if is_new {
                let mut p = open.clone();
                p["phase"] = json!("started");
                out.push(self.event("tool.started", p));
            }
            if matches!(v["status"].as_str(), Some("completed" | "failed")) {
                let mut p = self
                    .tools
                    .remove(id)
                    // Invariant: tool was retrieved above and no intervening removal occurs.
                    .expect("tool was retrieved above and no intervening removal occurs");
                let failed = v["status"] == "failed";
                p["phase"] = json!("completed");
                p["status"] = json!(if failed {
                    if self.declined.contains(id) {
                        "declined"
                    } else {
                        "failed"
                    }
                } else {
                    "completed"
                });
                if let Some(output) = content_text(&v["content"]) {
                    p["output"] = json!(text(&output, 16384));
                }
                if !failed && let Some(patch) = patch_of(&v["content"]) {
                    p["patch"] = patch;
                }
                out.push(self.event(
                    if failed {
                        "tool.failed"
                    } else {
                        "tool.completed"
                    },
                    p,
                ));
            }
            return out;
        }
        if self.unknown.insert(kind.into()) {
            out.push(self.diagnostic(
                "acp.unknown_update",
                &format!("Unhandled ACP update: {kind}"),
            ));
        }
        out
    }
    pub fn prompt_boundary(&mut self, stop: &str) -> Vec<Event> {
        self.records += 1;
        let mut out = vec![];
        self.close_thought(&mut out);
        self.close_message(&mut out);
        if matches!(stop, "max_tokens" | "max_turn_requests") {
            out.push(self.diagnostic("acp.stopped_at_limit",if stop=="max_tokens" {"The agent stopped this turn at its length limit, so the answer may be cut short."} else {"The agent stopped this turn at its limit of steps, so the work may be unfinished."}));
        }
        out
    }
    pub fn finish(
        &mut self,
        stop: &str,
        usage: Value,
        cancelled: bool,
        failure: Option<&str>,
        declined: bool,
    ) -> Vec<Event> {
        if self.ended {
            return vec![];
        }
        self.ended = true;
        let mut out = vec![];
        self.close_thought(&mut out);
        self.close_message(&mut out);
        let process = json!({"exitCode":if failure.is_none() && !cancelled {json!(0)} else {Value::Null},"signal":null,"stderr":failure.map(|f|text(f,16384)).unwrap_or_default(),"stderrTruncated":false,"recordCount":self.records,"cancelled":cancelled,"inputDeliveryFailed":false,"outputLimitExceeded":failure.is_some_and(|s|s.contains("overflow") || s.contains("exceeds 8 MiB")),"oversizedRecordsDropped":0,"forcedTerminationAttempted":false,"terminationUnconfirmed":false,"startedAt":self.started_at,"finishedAt":crate::ledger_write::now()});
        let mut p = json!({"process":process});
        if let Some(thread) = &self.thread {
            p["runtimeThreadId"] = json!(thread);
        }
        let terminal = if cancelled || (stop == "cancelled" && !declined) {
            "run.cancelled"
        } else if let Some(f) = failure {
            let lower = f.to_lowercase();
            let k = if lower.contains("quota") || lower.contains("usage limit") {
                "quota-exhausted"
            } else if lower.contains("rate limit") || lower.contains("429") {
                "temporary-rate-limit"
            } else if lower.contains("authenticat")
                || lower.contains("sign in")
                || lower.contains("log in")
            {
                "authentication-failed"
            } else {
                "process-failed"
            };
            if matches!(k, "quota-exhausted" | "temporary-rate-limit") {
                out.push(self.event("route.limit_detected",json!({"kind":k,"message":text(f,16384),"evidence":{"runtimeEventType":"acp.error","redacted":true}})));
            }
            p["kind"] = json!(k);
            p["message"] = json!(text(f, 16384));
            p["runtimeTerminal"] = json!("missing");
            "run.failed"
        } else if stop == "refusal" {
            p["kind"] = json!("safety-blocked");
            p["message"] = json!("The agent refused to continue this turn.");
            p["runtimeTerminal"] = json!("failed");
            "run.failed"
        } else {
            if usage.is_object() {
                let amounts: serde_json::Map<_, _> = usage
                    .as_object()
                    // Invariant: usage.is_object() guards this branch.
                    .expect("usage.is_object() guards this branch")
                    .iter()
                    .filter(|(_, v)| v.is_number())
                    .take(32)
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                if !amounts.is_empty() {
                    p["usage"] = Value::Object(amounts);
                }
            }
            "run.completed"
        };
        out.push(self.event(terminal, p));
        out
    }
}
/// line-diff.ts: common ends, bounded LCS, and three lines of context.
fn unified_diff(path: &str, old: Option<&str>, new: &str) -> Option<(String, usize, usize)> {
    fn lines(s: &str) -> Vec<&str> {
        let mut v: Vec<_> = s.split('\n').collect();
        if v.last() == Some(&"") {
            v.pop();
        }
        v
    }
    let before = lines(old.unwrap_or(""));
    let after = lines(new);
    let mut start = 0;
    while start < before.len() && start < after.len() && before[start] == after[start] {
        start += 1;
    }
    let (mut eb, mut ea) = (before.len(), after.len());
    while eb > start && ea > start && before[eb - 1] == after[ea - 1] {
        eb -= 1;
        ea -= 1;
    }
    if start == eb && start == ea {
        return None;
    }
    let mut ops: Vec<(char, &str)> = before[..start].iter().map(|s| (' ', *s)).collect();
    let (b, a) = (&before[start..eb], &after[start..ea]);
    if b.len() > 1500 || a.len() > 1500 {
        ops.extend(b.iter().map(|s| ('-', *s)));
        ops.extend(a.iter().map(|s| ('+', *s)));
    } else {
        let cols = a.len() + 1;
        let mut table = vec![0u32; (b.len() + 1) * cols];
        for i in (0..b.len()).rev() {
            for j in (0..a.len()).rev() {
                table[i * cols + j] = if b[i] == a[j] {
                    table[(i + 1) * cols + j + 1] + 1
                } else {
                    table[(i + 1) * cols + j].max(table[i * cols + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < b.len() && j < a.len() {
            if b[i] == a[j] {
                ops.push((' ', b[i]));
                i += 1;
                j += 1;
            } else if table[(i + 1) * cols + j] >= table[i * cols + j + 1] {
                ops.push(('-', b[i]));
                i += 1;
            } else {
                ops.push(('+', a[j]));
                j += 1;
            }
        }
        ops.extend(b[i..].iter().map(|s| ('-', *s)));
        ops.extend(a[j..].iter().map(|s| ('+', *s)));
    }
    ops.extend(before[eb..].iter().map(|s| (' ', *s)));
    let (mut ol, mut nl) = (1, 1);
    let positions: Vec<_> = ops
        .iter()
        .map(|(k, _)| {
            let p = (ol, nl);
            if *k != '+' {
                ol += 1;
            }
            if *k != '-' {
                nl += 1;
            }
            p
        })
        .collect();
    let side = |prefix: &str| {
        if path.starts_with(['/', '\\']) || path.as_bytes().get(1) == Some(&b':') {
            path.to_string()
        } else {
            format!("{prefix}{path}")
        }
    };
    let mut diff = format!(
        "--- {}\n+++ {}\n",
        if old.is_none() {
            "/dev/null".into()
        } else {
            side("a/")
        },
        side("b/")
    );
    let mut index = 0;
    while index < ops.len() {
        if ops[index].0 == ' ' {
            index += 1;
            continue;
        }
        let first = index.saturating_sub(3);
        let mut last = index;
        for (probe, (k, _)) in ops.iter().enumerate().skip(index) {
            if *k != ' ' {
                last = probe;
            } else if probe - last > 6 {
                break;
            }
        }
        let end = (last + 3).min(ops.len() - 1);
        let slice = &ops[first..=end];
        let oc = slice.iter().filter(|(k, _)| *k != '+').count();
        let nc = slice.iter().filter(|(k, _)| *k != '-').count();
        diff.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            positions[first].0 - usize::from(oc == 0),
            oc,
            positions[first].1 - usize::from(nc == 0),
            nc
        ));
        for (k, s) in slice {
            diff.push(*k);
            diff.push_str(s);
            diff.push('\n');
        }
        index = end + 1;
    }
    Some((
        diff,
        ops.iter().filter(|(k, _)| *k == '+').count(),
        ops.iter().filter(|(k, _)| *k == '-').count(),
    ))
}
pub fn patch_of(content: &Value) -> Option<Value> {
    let mut unified = String::new();
    let (mut added, mut removed) = (0, 0);
    for d in content.as_array().into_iter().flatten() {
        if d["type"] != "diff" {
            continue;
        }
        let (Some(path), Some(new)) = (d["path"].as_str(), d["newText"].as_str()) else {
            continue;
        };
        if let Some((diff, a, r)) = unified_diff(path, d["oldText"].as_str(), new) {
            unified.push_str(&diff);
            added += a;
            removed += r;
        }
    }
    if unified.is_empty() {
        None
    } else {
        Some(
            json!({"text":text(&unified,65536),"added":added,"removed":removed,"truncated":unified.encode_utf16().count()>65536}),
        )
    }
}

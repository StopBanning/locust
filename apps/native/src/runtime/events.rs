//! Whitelist normalization, ported from app-server-events.ts. Raw params never enter evidence.
use super::Event;
use crate::ledger_write::now;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
static SECRETS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    [
    r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]{8,}", r"\b(?:sk|pk)-[A-Za-z0-9_-]{12,}\b",
    r"\b(?:gh[opusr]_[A-Za-z0-9]{20,}|xox[baprs]-[A-Za-z0-9-]{10,}|AKIA[A-Z0-9]{16})\b",
    r"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?-----END [A-Z0-9 ]*PRIVATE KEY-----",
    r"\beyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}",
    r"\bAIza[0-9A-Za-z_-]{35}\b", r"\bhf_[A-Za-z0-9]{30,}\b",
    r#"(?i)((?:api[_-]?key|access[_-]?token|refresh[_-]?token|id[_-]?token|client[_-]?secret|authorization|password|secret|cookie|credential)["']?\s*[=:]\s*["']?)([^\s,"';}]+)"#,
].iter().map(|r| Regex::new(r).unwrap()).collect()
});
pub fn text(value: &str, limit: usize) -> String {
    let mut clean = value.replace('\0', "");
    for (i, regex) in SECRETS.iter().enumerate() {
        clean = regex
            .replace_all(
                &clean,
                if i == 7 {
                    "${1}[redacted]"
                } else {
                    "[redacted]"
                },
            )
            .into_owned();
    }
    let chars: Vec<char> = clean.chars().collect();
    if clean.encode_utf16().count() <= limit {
        clean
    } else {
        let take = |input: Vec<char>, max: usize| {
            let mut used = 0;
            input
                .into_iter()
                .take_while(|c| {
                    used += c.len_utf16();
                    used <= max
                })
                .collect::<Vec<_>>()
        };
        let head = take(chars.clone(), limit - 4111)
            .into_iter()
            .collect::<String>();
        let tail = take(chars.into_iter().rev().collect(), 4096)
            .into_iter()
            .rev()
            .collect::<String>();
        format!("{head}\n…[truncated]…\n{tail}")
    }
}
fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key].as_str().unwrap_or("")
}
fn identity(v: &Value, key: &str) -> String {
    s(v, key).replace('\0', "").chars().take(512).collect()
}
pub fn thread_id(p: &Value) -> Option<String> {
    p["threadId"]
        .as_str()
        .or(p["thread"]["id"].as_str())
        .or(p["turn"]["threadId"].as_str())
        .map(str::to_owned)
}
#[derive(Default)]
pub struct Normalizer {
    pub thread: Option<String>,
    pub turn: Option<String>,
    pub ended: bool,
    count: usize,
    limits: HashSet<String>,
    usage: HashMap<String, (Value, Value)>,
    own_started: bool,
    window: Option<String>,
    compaction_items: usize,
    compaction_notices: usize,
    compactions: usize,
}
impl Normalizer {
    fn event(&mut self, kind: &str, payload: Value) -> Event {
        self.count += 1;
        Event {
            kind: kind.into(),
            payload,
            thread: self.thread.clone(),
        }
    }
    fn process(&self, success: bool, cancelled: bool) -> Value {
        json!({"exitCode":if success {json!(0)} else {Value::Null},"signal":null,"stderr":"","stderrTruncated":false,
            "recordCount":self.count,"inputDeliveryFailed":false,"outputLimitExceeded":false,"oversizedRecordsDropped":0,
            "forcedTerminationAttempted":cancelled,"terminationUnconfirmed":!success && !cancelled,"startedAt":now(),"finishedAt":now()})
    }
    pub fn finish(&mut self, cancelled: bool, message: &str) -> Event {
        self.ended = true;
        let mut p = json!({"process":self.process(false,cancelled)});
        if let Some(t) = &self.thread {
            p["runtimeThreadId"] = json!(t);
        }
        if !cancelled {
            p["kind"] = json!("process-failed");
            p["message"] = json!(text(message, 16384));
            p["runtimeTerminal"] = json!("missing");
        }
        self.event(
            if cancelled {
                "run.cancelled"
            } else {
                "run.failed"
            },
            p,
        )
    }
    pub fn diagnostic(&mut self, method: &str, message: &str) -> Event {
        self.event("adapter.diagnostic", json!({"level":"warning","code":"app.warning","message":text(message,16384),"terminal":false,"evidence":{"runtimeEventType":method,"redacted":true}}))
    }
    pub fn accept(&mut self, method: &str, p: &Value) -> Vec<Event> {
        if self.ended {
            return vec![];
        }
        let from = thread_id(p);
        if method == "thread/tokenUsage/updated" {
            let key = from.unwrap_or_default();
            let total = p["tokenUsage"]["total"].clone();
            let row = self.usage.entry(key).or_insert((Value::Null, Value::Null));
            if !self.own_started {
                row.0 = total;
            } else {
                if row.0.is_null() {
                    let mut before = json!({});
                    for field in [
                        "inputTokens",
                        "outputTokens",
                        "cachedInputTokens",
                        "cacheWriteInputTokens",
                    ] {
                        if let Some(n) = total[field].as_u64() {
                            before[field] = json!(n.saturating_sub(
                                p["tokenUsage"]["last"][field].as_u64().unwrap_or(0)
                            ));
                        }
                    }
                    row.0 = before;
                }
                row.1 = total;
            }
            return vec![];
        }
        if from.is_some() && self.thread.is_some() && from != self.thread {
            return vec![];
        }
        let evidence = json!({"runtimeEventType":method,"redacted":true});
        let event = match method {
            "thread/started" => {
                if self.thread.is_none() {
                    self.thread = from;
                }
                return vec![];
            }
            "turn/started" => {
                self.thread = self.thread.clone().or(from);
                self.turn = p["turn"]["id"].as_str().map(str::to_owned);
                self.own_started = true;
                self.event("run.started",json!({"runtimeThreadId":self.thread.clone().unwrap_or_default(),"evidence":evidence}))
            }
            "item/agentMessage/delta" => {
                if p["delta"].as_str().is_none() || s(p, "itemId").is_empty() {
                    return vec![];
                }
                self.event("message.delta",json!({"itemId":identity(p,"itemId"),"operation":"append","text":text(s(p,"delta"),131072),"final":false,"evidence":evidence}))
            }
            "item/started" | "item/completed" => {
                let i = &p["item"];
                if !i.is_object() {
                    return vec![];
                }
                let kind = s(i, "type");
                let done = method == "item/completed";
                let id = if s(i, "id").is_empty() {
                    format!("item_{}", self.count + 1)
                } else {
                    identity(i, "id")
                };
                if [
                    "userMessage",
                    "plan",
                    "enteredReviewMode",
                    "exitedReviewMode",
                ]
                .contains(&kind)
                {
                    return vec![];
                }
                if kind == "contextCompaction" {
                    if !done {
                        return vec![];
                    }
                    self.compaction_items += 1;
                    if self.compaction_items <= self.compactions {
                        return vec![];
                    }
                    self.compactions = self.compaction_items;
                    return vec![self.event("adapter.diagnostic",json!({"level":"info","code":"codex.context_compacted","terminal":false,"message":"The conversation outgrew the model's context, so Codex summarized it and carried on from the summary.","evidence":evidence}))];
                }
                if kind == "agentMessage" {
                    if !done {
                        return vec![];
                    }
                    self.event("message.delta",json!({"itemId":id,"operation":"replace","text":text(s(i,"text"),131072),"final":true,"evidence":evidence}))
                } else if [
                    "commandExecution",
                    "fileChange",
                    "mcpToolCall",
                    "dynamicToolCall",
                    "webSearch",
                    "imageGeneration",
                    "collabAgentToolCall",
                ]
                .contains(&kind)
                {
                    let name = match kind {
                        "commandExecution" => "shell".into(),
                        "fileChange" => "apply_patch".into(),
                        "webSearch" => "web_search".into(),
                        "collabAgentToolCall" => format!("subagent:{}", s(i, "tool")),
                        _ => {
                            if !s(i, "server").is_empty() && !s(i, "tool").is_empty() {
                                format!("{}.{}", s(i, "server"), s(i, "tool"))
                            } else if !s(i, "tool").is_empty() {
                                identity(i, "tool")
                            } else {
                                kind.into()
                            }
                        }
                    };
                    let mut payload = json!({"itemId":id,"toolKind":kind,"name":name,"phase":if done {"completed"} else {"started"},"evidence":evidence});
                    if let Some(command) = i["command"].as_str().or(i["query"].as_str()) {
                        payload["command"] = json!(text(command, 16384));
                    }
                    if payload.get("command").is_none() {
                        if let Some(changes) = i["changes"].as_array() {
                            let paths: Vec<_> = changes
                                .iter()
                                .filter_map(|v| v["path"].as_str())
                                .filter(|s| !s.is_empty())
                                .collect();
                            let command = if !paths.is_empty() && paths.len() == changes.len() {
                                paths.join("\n")
                            } else if changes.len() == 1 {
                                "1 file".into()
                            } else {
                                format!("{} files", changes.len())
                            };
                            payload["command"] = json!(text(&command, 16384));
                        }
                    }
                    if done {
                        for field in ["status", "exitCode"] {
                            if !i[field].is_null() {
                                payload[field] = i[field].clone();
                            }
                        }
                        if let Some(output) = i["aggregatedOutput"].as_str() {
                            payload["output"] = json!(text(output, 16384));
                        }
                    }
                    let failed = done
                        && (i["status"].as_str().is_some_and(|v| v != "completed")
                            || i["exitCode"].as_i64().is_some_and(|v| v != 0));
                    self.event(
                        if !done {
                            "tool.started"
                        } else if failed {
                            "tool.failed"
                        } else {
                            "tool.completed"
                        },
                        payload,
                    )
                } else {
                    let mut payload =
                        json!({"stepKind":"item","itemId":id,"itemType":kind,"evidence":evidence});
                    if done && kind == "reasoning" {
                        if let Some(parts) = i["summary"].as_array() {
                            let summary = parts
                                .iter()
                                .filter_map(Value::as_str)
                                .filter(|s| !s.trim().is_empty())
                                .collect::<Vec<_>>()
                                .join("\n\n");
                            if !summary.is_empty() {
                                payload["message"] = json!(text(&summary, 16384));
                            }
                        }
                    } // Deliberately never reads reasoning content.
                    self.event(
                        if done {
                            "step.completed"
                        } else {
                            "step.started"
                        },
                        payload,
                    )
                }
            }
            "turn/plan/updated" => {
                // Whitelist plan entries; do not persist arbitrary nested provider data.
                let plan: Vec<Value> = p["plan"].as_array().into_iter().flatten().take(100).map(|v|json!({"step":text(s(v,"step"),16384),"status":text(s(v,"status"),16384)})).collect();
                self.event("plan.updated",json!({"itemId":if s(p,"turnId").is_empty(){"plan".into()}else{identity(p,"turnId")},"plan":plan,"final":false,"evidence":evidence}))
            }
            "turn/completed" => {
                let status = s(&p["turn"], "status");
                if status == "interrupted" {
                    return vec![self.finish(true, "")];
                }
                if status == "failed" {
                    return self.accept("error", &json!({"error":p["turn"]["error"]}));
                }
                self.ended = true;
                let mut payload =
                    json!({"runtimeThreadId":self.thread,"process":self.process(true,false)});
                let mut usage = json!({});
                for (field, to) in [
                    ("inputTokens", "inputTokens"),
                    ("outputTokens", "outputTokens"),
                    ("cachedInputTokens", "cacheReadTokens"),
                    ("cacheWriteInputTokens", "cacheWriteTokens"),
                ] {
                    let mut n = 0;
                    let mut seen = false;
                    for (before, total) in self.usage.values() {
                        if let Some(t) = total[field].as_u64() {
                            n += t.saturating_sub(before[field].as_u64().unwrap_or(0));
                            seen = true;
                        }
                    }
                    if seen {
                        usage[to] = json!(n);
                    }
                }
                if usage.as_object().is_some_and(|v| !v.is_empty()) {
                    payload["usage"] = usage;
                }
                self.event("run.completed", payload)
            }
            "error" | "turn/failed" => {
                let message = p["error"]["message"]
                    .as_str()
                    .unwrap_or("The runtime reported an error.");
                if p["willRetry"] == true {
                    return vec![self.diagnostic(method, message)];
                }
                let info = p["error"]["codexErrorInfo"]
                    .as_str()
                    .or_else(|| {
                        p["error"]["codexErrorInfo"]
                            .as_object()
                            .and_then(|v| v.keys().next().map(String::as_str))
                    })
                    .unwrap_or("");
                let lower = message.to_lowercase();
                let kind = if info == "usageLimitExceeded"
                    || lower.contains("quota")
                    || lower.contains("usage limit")
                {
                    "quota-exhausted"
                } else if info == "rateLimitExceeded" || lower.contains("rate limit") {
                    "temporary-rate-limit"
                } else if info == "unauthorized" {
                    "authentication-failed"
                } else if ["cyberPolicy", "misalignmentPolicyViolation"].contains(&info) {
                    "safety-blocked"
                } else {
                    "unknown"
                };
                let mut out = vec![];
                if ["quota-exhausted", "temporary-rate-limit"].contains(&kind)
                    && self.limits.insert(kind.into())
                {
                    out.push(self.event(
                        "route.limit_detected",
                        json!({"kind":kind,"message":text(message,16384),"evidence":evidence}),
                    ));
                }
                self.ended = true;
                out.push(self.event("run.failed",json!({"kind":kind,"message":text(message,16384),"runtimeThreadId":self.thread,"runtimeTerminal":"failed","process":self.process(false,false)})));
                return out;
            }
            "account/rateLimits/updated" => {
                let mut windows = vec![];
                if let Some(snapshot) = p["rateLimits"].as_object() {
                    for (label, v) in snapshot {
                        let used = v["usedPercent"]
                            .as_f64()
                            .or(v["used_percent"].as_f64())
                            .or(v["utilization"].as_f64().map(|n| n * 100.));
                        let Some(used) = used else { continue };
                        let minutes = v["windowDurationMins"]
                            .as_f64()
                            .or(v["windowMinutes"].as_f64())
                            .or(v["window_minutes"].as_f64());
                        let name = match minutes {
                            Some(n) if n >= 10080. => "weekly".into(),
                            Some(n) if n >= 1440. => format!("{}-day", (n / 1440.).round()),
                            Some(n) if n >= 60. && n % 60. == 0. => format!("{}-hour", n / 60.),
                            Some(n) => format!("{n}-minute"),
                            None => label.clone(),
                        };
                        let reset = v["resetsAt"]
                            .as_i64()
                            .or(v["resets_at"].as_i64())
                            .filter(|n| *n >= 1_000_000_000)
                            .and_then(|n| chrono::DateTime::from_timestamp(n, 0));
                        windows.push((name, used, reset));
                    }
                }
                windows.sort_by(|a, b| b.1.total_cmp(&a.1));
                let mut out = vec![];
                if !windows.is_empty() {
                    let reading = windows
                        .iter()
                        .map(|(name, used, reset)| {
                            format!(
                                "{name} window {}% used{}",
                                used.round().clamp(0., 100.),
                                reset
                                    .map(|r| format!(
                                        " · resets {}",
                                        r.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                                    ))
                                    .unwrap_or_default()
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(" · ");
                    if self.window.as_ref() != Some(&reading) {
                        self.window = Some(reading.clone());
                        out.push(self.event("adapter.diagnostic",json!({"level":"info","code":"codex.usage_window","terminal":false,"message":reading,"evidence":evidence})));
                    }
                    let (name, used, reset) = &windows[0];
                    if *used >= 90. {
                        let kind = if *used >= 100. {
                            "quota-exhausted"
                        } else {
                            "temporary-rate-limit"
                        };
                        if self.limits.insert(kind.into()) {
                            let until = reset
                                .map(|r| {
                                    format!(
                                        ", resets {}",
                                        r.with_timezone(&chrono::Local).format("%H:%M")
                                    )
                                })
                                .unwrap_or_default();
                            let message = if *used >= 100. {
                                format!("{name} limit reached{until}")
                            } else {
                                format!("{name} limit {}% used{until}", used.round())
                            };
                            out.push(self.event(
                                "route.limit_detected",
                                json!({"kind":kind,"message":message,"evidence":evidence}),
                            ));
                        }
                    }
                }
                return out;
            }
            "thread/compacted" => {
                self.compaction_notices += 1;
                if self.compaction_notices <= self.compactions {
                    return vec![];
                }
                self.compactions = self.compaction_notices;
                self.event("adapter.diagnostic",json!({"level":"info","code":"codex.context_compacted","terminal":false,"message":"The conversation outgrew the model's context, so Codex summarized it and carried on from the summary.","evidence":evidence}))
            }
            "warning" | "guardianWarning" | "configWarning" => {
                let message = p["summary"]
                    .as_str()
                    .or(p["message"].as_str())
                    .unwrap_or("");
                if message.is_empty()
                    || message.starts_with("`--dangerously-bypass-hook-trust` is enabled.")
                {
                    return vec![];
                }
                self.diagnostic(method, message)
            }
            _ => return vec![],
        };
        vec![event]
    }
}

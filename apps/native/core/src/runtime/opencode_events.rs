//! OpenCode 1 run records; shared error facts and sentences from opencode-error.ts.
use super::{Event, acp_events::AcpNormalizer, events::text};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
#[derive(Clone, Debug, Default)]
pub struct ErrorFacts {
    pub message: Option<String>,
    pub status: Option<i64>,
    pub kind: Option<String>,
    pub name: Option<String>,
}
impl ErrorFacts {
    pub fn of(v: &Value) -> Option<Self> {
        let e = v.get("error")?.as_object()?;
        let d = e.get("data").unwrap_or(&Value::Null);
        let name = e.get("name").and_then(Value::as_str).map(str::to_owned);
        let mut kind = d["type"].as_str().map(str::to_owned).or(name.clone());
        if let Some(body) = d["responseBody"].as_str()
            && let Ok(inner) = serde_json::from_str::<Value>(body)
            && let Some(k) = inner["error"]["type"].as_str()
        {
            kind = Some(k.into());
        }
        Some(Self {
            message: d["message"]
                .as_str()
                .or_else(|| e.get("message").and_then(Value::as_str))
                .map(str::to_owned),
            status: d["statusCode"].as_i64(),
            kind,
            name,
        })
    }
    pub fn session_ended(&self) -> bool {
        self.message
            .as_ref()
            .is_some_and(|s| s.to_lowercase().contains("encrypted_content"))
    }
    pub fn limit(&self) -> Option<&'static str> {
        if self.kind.as_deref() == Some("FreeUsageLimitError") {
            Some("quota-exhausted")
        } else if self.status == Some(429) {
            Some("temporary-rate-limit")
        } else {
            None
        }
    }
    pub fn sentence(&self, free: bool) -> String {
        let quoted = self.message.as_deref().map(str::trim);
        let suffix = quoted.map(|s| format!(": {s}")).unwrap_or_default();
        if self.kind.as_deref() == Some("FreeUsageLimitError") {
            return "The free model has no usage left right now, so OpenCode stopped. Pick another model, or try the free one again later.".into();
        }
        if self.session_ended() {
            return "OpenCode could not continue this session, so the run stopped. Nothing was lost from the conversation; the next message starts a fresh session.".into();
        }
        if self.status == Some(429) {
            return format!("OpenCode was rate limited by the provider and stopped{suffix}");
        }
        if matches!(self.status, Some(401 | 403)) {
            if free {
                return format!(
                    "OpenCode's service refused the free model, so OpenCode stopped{}. Free models need no sign-in, so this is on OpenCode's side: pick another model, or try this one again later.",
                    quoted
                        .map(|s| format!(": {}", s.trim_end_matches('.')))
                        .unwrap_or_default()
                );
            }
            return format!(
                "OpenCode was refused by the provider and stopped{suffix}. Its sign-in may have expired -- `opencode auth login` in a terminal."
            );
        }
        if let Some(s) = quoted.filter(|s| !s.is_empty()) {
            return format!("OpenCode stopped: {s}");
        }
        "OpenCode reported an error and stopped, without saying what it was.".into()
    }
}
pub struct Normalizer {
    pub base: AcpNormalizer,
    free: bool,
    step: bool,
    saw_stop: bool,
    index: usize,
    usage: [f64; 6],
    saw_tokens: bool,
    error: Option<ErrorFacts>,
    overflow: u8,
    parts: HashMap<String, (String, String, bool)>,
    step_messages: Vec<String>,
    step_tools: bool,
    finished: Option<(Vec<String>, bool)>,
    changed: bool,
    retries: HashMap<String, usize>,
    unknown: HashSet<String>,
}
impl Normalizer {
    pub fn new(model: Option<&str>) -> Self {
        Self {
            base: AcpNormalizer::default(),
            free: model.is_some_and(|s| {
                s.to_lowercase().ends_with("-free") || s.to_lowercase().ends_with("/free")
            }),
            step: false,
            saw_stop: false,
            index: 0,
            usage: [0.; 6],
            saw_tokens: false,
            error: None,
            overflow: 0,
            parts: HashMap::new(),
            step_messages: vec![],
            step_tools: false,
            finished: None,
            changed: false,
            retries: HashMap::new(),
            unknown: HashSet::new(),
        }
    }
    fn emit(&self, k: &str, p: Value) -> Event {
        self.base.event(k, p)
    }
    fn diagnostic(&self, level: &str, code: &str, message: &str) -> Event {
        self.emit(
            "adapter.diagnostic",
            json!({"level":level,"code":code,"message":text(message,16384),"terminal":false}),
        )
    }
    fn close_partials(&mut self) -> Vec<Event> {
        let mut items = vec![];
        for (id, said, closed) in self.parts.values_mut() {
            if !*closed && !said.is_empty() {
                *closed = true;
                items.push((id.clone(), said.clone()));
            }
        }
        items.into_iter().map(|(id,said)|self.emit("message.delta",json!({"itemId":id,"operation":"replace","text":text(&said,131072),"final":true}))).collect()
    }
    pub fn accept(&mut self, v: &Value) -> Vec<Event> {
        if self.base.ended {
            return vec![];
        }
        self.base.records += 1;
        if self.base.thread.is_none() {
            self.base.thread = v["sessionID"].as_str().map(|s| text(s, 2048));
        }
        let kind = v["type"].as_str().unwrap_or("");
        let p = &v["part"];
        let mut out = if matches!(kind, "tool_use" | "step_finish") {
            self.close_partials()
        } else {
            vec![]
        };
        match kind {
            "step_start" => {
                self.step_messages.clear();
                self.step_tools = false;
                self.finished = None;
                if !self.step {
                    self.step = true;
                    out.push(self.emit("step.started", json!({"stepKind":"turn"})));
                }
            }
            "step_finish" => {
                let t = &p["tokens"];
                let values = [
                    t["input"].as_f64(),
                    t["output"].as_f64(),
                    t["reasoning"].as_f64(),
                    t["cache"]["read"].as_f64(),
                    t["cache"]["write"].as_f64(),
                    p["cost"].as_f64(),
                ];
                for (i, n) in values.iter().enumerate() {
                    if let Some(n) = n.filter(|n| n.is_finite() && *n >= 0.) {
                        self.usage[i] += n;
                        if i < 2 {
                            self.saw_tokens = true;
                        }
                    }
                }
                if p["reason"] == "stop" {
                    self.saw_stop = true;
                    if self.overflow == 2 {
                        self.overflow = 3;
                    }
                }
                self.finished = Some((std::mem::take(&mut self.step_messages), self.step_tools));
                self.step_tools = false;
                if self.step {
                    self.step = false;
                    out.push(self.emit("step.completed", json!({"stepKind":"turn"})));
                }
            }
            "text_partial" | "text" => {
                if p["synthetic"] == true {
                    if kind == "text" && p["metadata"]["compaction_continue"] == true {
                        if let Some((messages, false)) = self.finished.take() {
                            for id in messages {
                                out.push(self.emit("message.delta",json!({"itemId":id,"operation":"replace","text":"","final":true})));
                            }
                        }
                        self.saw_stop = false;
                        if self.overflow == 1 {
                            self.overflow = 2;
                        }
                        out.push(self.diagnostic("info","opencode.context_compacted","The conversation outgrew the model's context, so OpenCode summarized it and carried on from the summary."));
                    }
                    return out;
                }
                let Some(said) = p["text"].as_str().filter(|s| !s.is_empty()) else {
                    return out;
                };
                let part_id = p["id"].as_str();
                if kind == "text_partial" {
                    let Some(pid) = part_id else {
                        return out;
                    };
                    if !self.parts.contains_key(pid) {
                        let id = format!("msg_{}", self.index);
                        self.index += 1;
                        self.step_messages.push(id.clone());
                        self.parts.insert(pid.into(), (id, String::new(), false));
                    }
                    let held = self
                        .parts
                        .get_mut(pid)
                        // Invariant: part inserted immediately above when absent.
                        .expect("part inserted immediately above when absent");
                    if held.2 {
                        return out;
                    }
                    let grew = said.starts_with(&held.1);
                    let delta = if grew { &said[held.1.len()..] } else { said };
                    let id = held.0.clone();
                    held.1 = said.into();
                    if !delta.is_empty() {
                        out.push(self.emit("message.delta",json!({"itemId":id,"operation":if grew{"append"}else{"replace"},"text":text(delta,131072),"final":false})));
                    }
                } else {
                    let held = part_id.and_then(|id| self.parts.remove(id));
                    let id = held.map(|p| p.0).unwrap_or_else(|| {
                        let id = format!("msg_{}", self.index);
                        self.index += 1;
                        self.step_messages.push(id.clone());
                        id
                    });
                    out.push(self.emit("message.delta",json!({"itemId":id,"operation":"replace","text":text(said,131072),"final":true})));
                }
            }
            "tool_use" => {
                self.step_tools = true;
                let state = &p["state"];
                let input = &state["input"];
                let meta = &state["metadata"];
                let tool = p["tool"].as_str().unwrap_or("tool");
                let id = p["callID"]
                    .as_str()
                    .map(str::to_owned)
                    .unwrap_or_else(|| format!("tool_{}", self.base.records));
                if matches!(tool.to_lowercase().as_str(), "todowrite" | "todo_write")
                    && input["todos"].as_array().is_some_and(|a| !a.is_empty())
                {
                    // Invariant: branch requires todos.is_array().
                    let plan:Vec<_>=input["todos"].as_array().expect("branch requires todos.is_array()").iter().take(100).map(|t|json!({"content":text(t["content"].as_str().unwrap_or(""),16384),"status":text(t["status"].as_str().unwrap_or(""),512),"priority":text(t["priority"].as_str().unwrap_or(""),512)})).collect();
                    out.push(self.emit(
                        "plan.updated",
                        json!({"itemId":id,"plan":plan,"final":false}),
                    ));
                    return out;
                }
                let target = [
                    &input["filePath"],
                    &input["command"],
                    &input["pattern"],
                    &input["description"],
                    &input["prompt"],
                    &meta["filepath"],
                ]
                .into_iter()
                .find_map(Value::as_str);
                let mut common = json!({"itemId":id,"toolKind":tool,"name":tool});
                if let Some(t) = target {
                    common["command"] = json!(text(t, 16384));
                }
                if matches!(tool.to_lowercase().as_str(), "bash" | "shell")
                    && let Some(title) = input["description"].as_str()
                {
                    common["title"] = json!(text(title, 16384));
                }
                let mut start = common.clone();
                start["phase"] = json!("started");
                out.push(self.emit("tool.started", start));
                let status = state["status"].as_str().unwrap_or("unknown");
                let failed = status != "completed";
                if failed {
                    let error = state["error"].as_str().unwrap_or("");
                    common["status"] = json!(if status == "error"
                        && error.to_lowercase().contains("rejected permission")
                    {
                        if error.contains("declined this in Locust")
                            || error.contains("The person declined this, and said:")
                        {
                            "declined"
                        } else {
                            "refused"
                        }
                    } else {
                        status
                    });
                }
                if !failed
                    && matches!(
                        tool.to_lowercase().as_str(),
                        "bash" | "edit" | "write" | "patch" | "multiedit"
                    )
                {
                    self.changed = true;
                }
                if let Some(diff) = meta["diff"].as_str() {
                    common["patch"] = patch(diff);
                } else if tool == "write"
                    && meta["exists"] == false
                    && let (Some(path), Some(content)) = (
                        input["filePath"].as_str().or(meta["filepath"].as_str()),
                        input["content"].as_str(),
                    )
                    && let Some(patch) = super::acp_events::patch_of(
                        &json!([{"type":"diff","path":path,"oldText":null,"newText":content}]),
                    )
                {
                    common["patch"] = patch;
                }
                if let (Some(start), Some(end)) = (
                    state["time"]["start"].as_f64(),
                    state["time"]["end"].as_f64(),
                ) && end >= start
                {
                    common["durationMs"] = json!(end - start);
                }
                common["phase"] = json!("completed");
                out.push(self.emit(
                    if failed {
                        "tool.failed"
                    } else {
                        "tool.completed"
                    },
                    common,
                ));
            }
            "locust.retry" => {
                let said = v["message"].as_str().unwrap_or("");
                if self.retries.insert(format!("served:{said}"), 1).is_none() {
                    out.push(self.diagnostic("warning","opencode.provider_busy.runtime_error",&format!("The model's provider answered \"{said}\", and OpenCode is trying again on its own. To go on now, press Stop and pick another model.")));
                }
            }
            "error" => {
                if let Some(f) = ErrorFacts::of(v) {
                    if f.name.as_deref() == Some("ContextOverflowError") {
                        self.overflow = 1;
                        out.push(self.diagnostic("info","opencode.context_overflow","The request was longer than the model accepts, so OpenCode is summarizing the conversation to try again."));
                    } else {
                        self.overflow = 0;
                        out.push(self.diagnostic(
                            "error",
                            "opencode.provider_error",
                            &f.sentence(self.free),
                        ));
                    }
                    self.error = Some(f);
                }
            }
            "locust.stderr" => {}
            _ => {
                if self.unknown.insert(kind.into()) && !kind.is_empty() {
                    out.push(self.diagnostic(
                        "info",
                        "opencode.unknown_event",
                        &format!("Unhandled OpenCode record: {kind}"),
                    ));
                }
            }
        }
        out
    }
    pub fn finish(
        &mut self,
        cancelled: bool,
        code: Option<i32>,
        stderr: &str,
        output_limit: bool,
    ) -> Vec<Event> {
        if self.base.ended {
            return vec![];
        }
        self.base.ended = true;
        let mut p = json!({"process":{"exitCode":code,"signal":null,"stderr":text(stderr,16384),"stderrTruncated":false,"recordCount":self.base.records,"inputDeliveryFailed":false,"outputLimitExceeded":output_limit,"oversizedRecordsDropped":0,"forcedTerminationAttempted":cancelled,"terminationUnconfirmed":false,"startedAt":self.base.started_at,"finishedAt":crate::ledger_write::now()}});
        if let Some(id) = &self.base.thread {
            p["runtimeThreadId"] = json!(id);
        }
        if cancelled {
            return vec![self.emit("run.cancelled", p)];
        }
        let refused = crate::invariants::regex(
            r"(?i)permission requested:\s*external_directory\s*\(([^)]*)\)",
        )
        .captures(stderr)
        .and_then(|m| m.get(1))
        .map(|m| {
            m.as_str()
                .trim()
                .trim_end_matches(['*', '/', '\\'])
                .to_owned()
        });
        let blocked =
            crate::invariants::regex(r"(?i)permission requested:\s*(bash|edit|write|patch)\b")
                .captures(stderr)
                .and_then(|m| m.get(1))
                .map(|m| m.as_str().to_lowercase());
        let recovered = self.overflow == 3
            && code == Some(1)
            && refused.is_none()
            && blocked.is_none()
            && !output_limit;
        if !self.saw_stop || (code != Some(0) && !recovered) {
            let message = if let Some(path) = refused {
                format!(
                    "OpenCode asked for {path}, which is outside the folder this run may use, and stopped."
                )
            } else if let Some(tool) = blocked {
                format!(
                    "The mode this run is in does not allow {tool}, so OpenCode stopped when it tried to use it.{}",
                    if self.changed {
                        " What it did before that still stands: look at the folder."
                    } else {
                        " Nothing was changed."
                    }
                )
            } else if output_limit {
                "OpenCode sent output faster than Locust could record it, so the run was stopped rather than leave a gap in its record. Sending it again usually works.".into()
            } else if let Some(f) = &self.error {
                f.sentence(self.free)
            } else if self.saw_stop {
                format!(
                    "OpenCode exited with code {}.",
                    code.map(|c| c.to_string()).unwrap_or("null".into())
                )
            } else {
                "OpenCode ended without a step that reported it had stopped.".into()
            };
            p["kind"] = json!("process-failed");
            p["message"] = json!(text(&message, 16384));
            p["runtimeTerminal"] = json!(if self.saw_stop {
                "completed"
            } else {
                "missing"
            });
            if self.error.as_ref().is_some_and(ErrorFacts::session_ended) {
                p["sessionEnded"] = json!(true);
            }
            let mut out = vec![];
            if let Some(k) = self.error.as_ref().and_then(ErrorFacts::limit) {
                out.push(self.emit(
                    "route.limit_detected",
                    json!({"kind":k,"message":text(&message,16384)}),
                ));
            }
            out.push(self.emit("run.failed", p));
            return out;
        }
        if self.saw_tokens {
            let mut u =
                json!({"inputTokens":self.usage[0],"outputTokens":self.usage[1]+self.usage[2]});
            for (k, i) in [("cacheReadTokens", 3), ("cacheWriteTokens", 4), ("usd", 5)] {
                if self.usage[i] > 0. {
                    u[k] = json!(self.usage[i]);
                }
            }
            p["usage"] = u;
        }
        vec![self.emit("run.completed", p)]
    }
}
pub fn patch(diff: &str) -> Value {
    let (mut added, mut removed) = (0, 0);
    let (mut old, mut new) = (0usize, 0usize);
    let hunk = crate::invariants::regex(r"^@@ -\d+(?:,(\d+))? \+\d+(?:,(\d+))? @@");
    for line in diff.lines() {
        if let Some(m) = hunk.captures(line) {
            old = m
                .get(1)
                .map(|m| m.as_str().parse().unwrap_or(1))
                .unwrap_or(1);
            new = m
                .get(2)
                .map(|m| m.as_str().parse().unwrap_or(1))
                .unwrap_or(1);
            continue;
        }
        if line.starts_with('+') && new > 0 {
            added += 1;
            new -= 1;
        } else if line.starts_with('-') && old > 0 {
            removed += 1;
            old -= 1;
        } else if line.starts_with(' ') && old > 0 && new > 0 {
            old -= 1;
            new -= 1;
        }
    }
    json!({"text":text(diff,65536),"added":added,"removed":removed,"truncated":diff.encode_utf16().count()>65536})
}

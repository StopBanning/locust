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
    tools: HashMap<String, OpenTool>,
    declined: HashSet<String>,
    unknown: HashSet<String>,
    thinking: bool,
    /// What the open thought has said so far: its step's message once it closes (acp-events.ts `closeReasoning`).
    thought: String,
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
/// A title that is one tool word (`read`, `grep`, `shell`), not a sentence about the call (Electron's `plainAcpToolWord`).
fn plain_tool_word(title: Option<&str>) -> Option<String> {
    let word = title?.trim().to_lowercase();
    let plain = word.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
        && word
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_-".contains(&b));
    plain.then_some(word)
}
/// How the row is named (Electron's `acpShownTool`): a kind of `other` or none is named from a tool word title.
fn shown_tool(kind: Option<&str>, title: Option<&str>) -> (String, String) {
    let word = plain_tool_word(title);
    let k = match (kind, &word) {
        (None | Some("other"), Some(w)) => Some(w.as_str()),
        _ => kind,
    };
    let (kind, name) = match k {
        Some("execute") => ("command_execution", "bash"),
        Some(k @ ("edit" | "delete" | "move")) => ("file_change", k),
        Some("fetch") => ("web_fetch", "webfetch"),
        None => ("tool", "tool"),
        Some(k) => (k, k),
    };
    (kind.into(), name.into())
}
/// A shell call (Electron's `acpCallIsShell`). Its `locations` entry is the folder it runs in, never the command.
fn is_shell(kind: Option<&str>, title: Option<&str>, tool_kind: &str, name: &str) -> bool {
    const SHELLS: [&str; 5] = ["bash", "shell", "powershell", "pwsh", "cmd"];
    if kind == Some("execute") || tool_kind == "command_execution" {
        return true;
    }
    if SHELLS.iter().any(|w| name.eq_ignore_ascii_case(w)) {
        return true;
    }
    matches!(kind, None | Some("other"))
        && plain_tool_word(title).is_some_and(|w| SHELLS.contains(&w.as_str()) || w == "execute")
}
/// The files a tool named, one per line. Never a shell's command: its `locations` path is its folder.
fn file_target(v: &Value) -> Option<String> {
    let paths: Vec<_> = v["locations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|l| string(&l["path"]))
        .collect();
    (!paths.is_empty()).then(|| paths.join("\n"))
}
/// An open call as Electron holds it: the raw command and title, bounded only when a row is written.
struct OpenTool {
    tool_kind: String,
    name: String,
    command: Option<String>,
    title: Option<String>,
}
impl OpenTool {
    /// The row's words. A title that only repeats the command or the tool's name says nothing.
    fn row(&self, id: &str) -> Value {
        let mut p = json!({"itemId":id,"toolKind":self.tool_kind,"name":self.name});
        if let Some(c) = &self.command {
            p["command"] = json!(text(c, 16384));
        }
        if let Some(t) = &self.title
            && Some(t) != self.command.as_ref()
            && *t != self.name
        {
            p["title"] = json!(text(t, 16384));
        }
        p
    }
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
            let said = text(std::mem::take(&mut self.thought).trim(), 16384);
            let mut p = json!({"stepKind":"reasoning","itemId":format!("reasoning_{}",self.ordinal)});
            if !said.is_empty() {
                p["message"] = json!(said);
            }
            out.push(self.event("step.completed", p));
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
            // The words are the step's message; held bounded as they grow, as a message is.
            self.thought.push_str(&content_text(&v["content"]).unwrap_or_default());
            self.thought = text(&self.thought, 131072);
            if self.thinking {
                return out;
            }
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
            let incoming = string(&v["kind"]);
            let title = string(&v["title"]);
            let said = string(&v["rawInput"]["command"]);
            if let Some(open) = self.tools.get_mut(id) {
                let shell = is_shell(incoming, title.or(open.title.as_deref()), &open.tool_kind, &open.name);
                if let Some(c) = said {
                    open.command = Some(c.into());
                } else if !shell && let Some(files) = file_target(v) {
                    open.command = Some(files);
                }
                // A later update can replace a bare tool word (`shell`) with what it ran (`echo hi`).
                if let Some(t) = title
                    && Some(t) != open.title.as_deref()
                    && plain_tool_word(open.title.as_deref()).is_some()
                {
                    open.title = Some(t.into());
                }
                // Kind `other` until the title arrives: take the tool word once it does, never `other` or `tool`.
                if (open.name == "other" || open.name == "tool" || open.tool_kind == "other")
                    && matches!(incoming, None | Some("other"))
                {
                    let (tk, name) = shown_tool(incoming, title.or(open.title.as_deref()));
                    if name != "other" && name != "tool" {
                        open.tool_kind = tk;
                        open.name = name;
                    }
                }
            } else {
                // What was said before the call is a message of its own.
                self.close_message(&mut out);
                let (tool_kind, name) = shown_tool(incoming, title);
                let shell = is_shell(incoming, title, &tool_kind, &name);
                let command = said
                    .map(str::to_owned)
                    .or_else(|| if shell { None } else { file_target(v) });
                let open = OpenTool { tool_kind, name, command, title: title.map(str::to_owned) };
                let mut p = open.row(id);
                p["phase"] = json!("started");
                out.push(self.event("tool.started", p));
                self.tools.insert(id.into(), open);
            }
            if matches!(v["status"].as_str(), Some("completed" | "failed")) {
                let open = self
                    .tools
                    .remove(id)
                    // Invariant: the call was found or inserted above and nothing removed it since.
                    .expect("the call was found or inserted above");
                let mut p = open.row(id);
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
        let process = json!({"exitCode":if failure.is_none() && !cancelled {json!(0)} else {Value::Null},"signal":null,"stderr":failure.map(|f|text(f,16384)).unwrap_or_default(),"stderrTruncated":false,"recordCount":self.records,"cancelled":cancelled,"inputDeliveryFailed":false,"outputLimitExceeded":failure.is_some_and(output_limit),"oversizedRecordsDropped":0,"forcedTerminationAttempted":false,"terminationUnconfirmed":false,"startedAt":self.started_at,"finishedAt":crate::ledger_write::now()});
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
            } else if signed_out(&lower) {
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
/// A failure the host made by stopping a run whose output outran it (acp-run.ts `outputLimitExceeded`): the queue's
/// overflow, said in Electron's words (`acp::OVERFLOW_MESSAGE`), or a record past 8 MiB.
fn output_limit(failure: &str) -> bool {
    failure == super::acp::OVERFLOW_MESSAGE || failure.contains("overflow") || failure.contains("exceeds 8 MiB")
}
/// A failure that says the CLI is not signed in (acp-events.ts `SIGNED_OUT`:
/// `/authenticat|not (?:logged|signed) in|\blog ?in\b|\bsign ?in\b/i`), read from lowercased words.
fn signed_out(lower: &str) -> bool {
    let word = |w: &str| {
        lower.match_indices(w).any(|(at, _)| {
            let before = lower[..at].chars().next_back();
            let after = lower[at + w.len()..].chars().next();
            !before.is_some_and(|c| c.is_alphanumeric() || c == '_') && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
        })
    };
    lower.contains("authenticat")
        || lower.contains("not logged in")
        || lower.contains("not signed in")
        || ["log in", "login", "sign in", "signin"].into_iter().any(word)
}
/// line-diff.ts unifiedDiffOf: the diff's text alone, for readers that do not count lines.
pub fn unified_diff_text(path: &str, old: Option<&str>, new: &str) -> Option<String> {
    unified_diff(path, old, new).map(|(diff, _, _)| diff)
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

#[cfg(test)]
#[path = "line_diff_case_tests.rs"]
mod line_diff_case_tests;

#[cfg(test)]
mod overflow_words_tests {
    use super::*;

    /// SOAK-NOTES row 2a: a run the host stopped for outrunning the queue ends in Electron's sentence and is marked as
    /// the output limit, as acp-run.ts records it.
    #[test]
    fn a_queue_overflow_ends_in_electrons_sentence_marked_as_the_output_limit() {
        let mut normal = AcpNormalizer::default();
        let out = normal.finish("end_turn", Value::Null, false, Some(super::super::acp::OVERFLOW_MESSAGE), false);
        let last = out.last().unwrap();
        assert_eq!(last.kind, "run.failed");
        assert_eq!(last.payload["message"], super::super::acp::OVERFLOW_MESSAGE);
        assert_eq!(last.payload["process"]["outputLimitExceeded"], true, "{:?}", last.payload);
        let other = AcpNormalizer::default().finish("end_turn", Value::Null, false, Some("The agent exited."), false);
        assert_eq!(other.last().unwrap().payload["process"]["outputLimitExceeded"], false);
    }
}

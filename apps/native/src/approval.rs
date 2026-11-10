//! Approvals: what a runtime asks mid-run, and what the person answers.
//!
//! Ports `apps/desktop/src/main/approval-channel.ts` (describe / decide / protocol
//! answers), `permission-host.ts:builtInOrConnector` for Claude `can_use_tool`, and
//! `commands.ts:codexAppServerPolicy` for mode → sandbox / approvalPolicy.
//! Saved approval rules are out of scope (later round).

use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
    },
    time::{Duration, Instant},
};

const MAX_PENDING: usize = 16;
const MAX_DETAIL: usize = 8_000;


/// Product modes Locust names on a teammate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Ask,
    AcceptEdits,
    ApproveEach,
    Plan,
    Auto,
}

impl Mode {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "ask" => Some(Self::Ask),
            "accept-edits" => Some(Self::AcceptEdits),
            "approve-each" => Some(Self::ApproveEach),
            "plan" => Some(Self::Plan),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::AcceptEdits => "accept-edits",
            Self::ApproveEach => "approve-each",
            Self::Plan => "plan",
            Self::Auto => "auto",
        }
    }
}

/// `commands.ts:codexAppServerPolicy` — sandbox + approvalPolicy for app-server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodexPolicy {
    pub sandbox: &'static str,
    pub approval_policy: &'static str,
}

pub fn codex_app_server_policy(mode: Mode, sandbox_hint: Option<&str>) -> CodexPolicy {
    // Approve-each stops (`untrusted`); never paired with full access.
    if mode == Mode::ApproveEach {
        let sandbox = match sandbox_hint {
            Some("full-access") | Some("danger-full-access") => "workspace-write",
            Some("workspace-write") => "workspace-write",
            _ => "workspace-write",
        };
        return CodexPolicy {
            sandbox,
            approval_policy: "untrusted",
        };
    }
    if matches!(sandbox_hint, Some("full-access") | Some("danger-full-access"))
        || mode == Mode::Auto
    {
        return CodexPolicy {
            sandbox: "danger-full-access",
            approval_policy: "never",
        };
    }
    // Ask / Plan / Accept-edits (non-full): read-only + never, as M1.
    CodexPolicy {
        sandbox: "read-only",
        approval_policy: "never",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Command,
    FileChange,
    Question,
    Connector,
}

impl Kind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::FileChange => "file-change",
            Self::Question => "question",
            Self::Connector => "connector",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    ApproveOnce,
    ApproveAlways,
    Deny,
}

impl Decision {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ApproveOnce => "approve-once",
            Self::ApproveAlways => "approve-always",
            Self::Deny => "deny",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Request {
    pub approval_id: String,
    pub run_id: String,
    pub mission_id: String,
    pub runtime: String,
    pub kind: Kind,
    pub summary: String,
    pub detail: String,
    pub cwd: Option<String>,
    pub requested_at: String,
    pub always_covers: Option<String>,
    pub always_key: Option<String>,
    pub data_sent_says: Option<String>,
    pub reversible_says: Option<String>,
    pub tool_use_id: Option<String>,
    /// Wire id the runtime needs to answer (Codex JSON-RPC id, or Claude request_id).
    pub wire_id: Value,
    /// Claude `can_use_tool` input, kept so allow can return updatedInput.
    pub claude_input: Option<Value>,
}

#[derive(Clone, Debug)]
pub enum Answer {
    Decision {
        approval_id: String,
        decision: Decision,
        reason: Option<String>,
    },
}

impl Answer {
    pub fn approval_id(&self) -> &str {
        match self {
            Self::Decision { approval_id, .. } => approval_id,
        }
    }
}

/// Ledger `mission.approval` payload (mission-store MissionApproval).
#[derive(Clone, Debug)]
pub struct Recorded {
    pub mission_id: String,
    pub approval_id: String,
    pub kind: Kind,
    pub asked: String,
    pub answer: String,
    pub by: String,
    pub words: Option<String>,
    pub asked_at: String,
    pub occurred_at: String,
}

fn bounded(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        if out.len() >= MAX_DETAIL {
            break;
        }
        out.push(ch);
    }
    out
}

fn whole_detail(value: &str) -> String {
    // approval-detail.ts:wholeDetail — keep newlines; cap length.
    if value.len() <= MAX_DETAIL {
        value.to_string()
    } else {
        bounded(value)
    }
}

/// `approval-channel.ts:approvalKindFor`
pub fn approval_kind_for(method: &str) -> Option<Kind> {
    match method {
        "item/commandExecution/requestApproval" => Some(Kind::Command),
        "item/fileChange/requestApproval" => Some(Kind::FileChange),
        "item/tool/requestUserInput" => Some(Kind::Question),
        _ => None,
    }
}

/// `approval-channel.ts:describeApproval` for Codex app-server methods.
pub fn describe_codex(method: &str, params: &Value) -> Option<(Kind, String, String, Option<String>)> {
    let kind = approval_kind_for(method)?;
    let cwd = params["cwd"].as_str().map(str::to_owned);
    match kind {
        Kind::Command => {
            let command = whole_detail(params["command"].as_str().unwrap_or(""));
            let summary = if command.is_empty() {
                "Run a command it did not describe".into()
            } else {
                "Run a command".into()
            };
            Some((kind, summary, command, cwd))
        }
        Kind::FileChange => {
            let changes = params["changes"].as_array().map(|a| a.len());
            let summary = match changes {
                None => "Change files".into(),
                Some(1) => "Change 1 file".into(),
                Some(n) => format!("Change {n} files"),
            };
            let detail = whole_detail(
                params["summary"]
                    .as_str()
                    .or_else(|| params["explanation"].as_str())
                    .unwrap_or(""),
            );
            Some((kind, summary, detail, cwd))
        }
        Kind::Question => {
            let detail = whole_detail(
                params["question"]
                    .as_str()
                    .or_else(|| params["prompt"].as_str())
                    .or_else(|| params["message"].as_str())
                    .unwrap_or(""),
            );
            Some((kind, "Answer a question".into(), detail, cwd))
        }
        Kind::Connector => None,
    }
}

/// `permission-host.ts:builtInOrConnector` for Claude `can_use_tool`.
pub fn describe_claude_tool(
    tool_name: &str,
    input: &Value,
    cwd: &str,
) -> (Kind, String, String, Option<String>, Option<String>) {
    let text = |v: &Value| -> String {
        v.as_str()
            .map(|s| s.replace('\n', " ").replace('\r', " ").trim().chars().take(600).collect())
            .unwrap_or_default()
    };
    if tool_name == "WebSearch" {
        return (
            Kind::Connector,
            "Search the web".into(),
            text(&input["query"]),
            Some("The search words above, to Claude Code's web search.".into()),
            Some("Nothing is changed by a search.".into()),
        );
    }
    if tool_name == "WebFetch" {
        let url = text(&input["url"]);
        let host = url.split_once("://").filter(|(scheme, _)| *scheme == "https" || *scheme == "http").map(|(_, rest)| rest.split(['/', '?', '#']).next().unwrap_or(""));
        return (
            Kind::Connector,
            host.filter(|h| !h.is_empty()).map(|host| format!("Open a page on {host}")).unwrap_or_else(|| "Open a web page".into()),
            [url, text(&input["prompt"])].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n"),
            Some("The whole address above, to that site. Read it: anything written into an address is sent with it.".into()),
            Some("Opening a page changes nothing here. A site can act on an address it is sent.".into()),
        );
    }
    if tool_name == "Bash" {
        let command = input["command"].as_str().map(whole_detail).unwrap_or_default();
        let summary = if command.is_empty() {
            "Run a command it did not describe".into()
        } else {
            "Run a command".into()
        };
        return (Kind::Command, summary, command, None, None);
    }
    if matches!(
        tool_name,
        "Edit" | "Write" | "MultiEdit" | "NotebookEdit"
    ) {
        let file = {
            let a = text(&input["file_path"]);
            if a.is_empty() {
                text(&input["notebook_path"])
            } else {
                a
            }
        };
        let outside = !file.is_empty() && !cwd.is_empty() && {
            let path = std::path::Path::new(&file);
            path.components().any(|c| c == std::path::Component::ParentDir)
                || (path.is_absolute() && !path.starts_with(cwd))
        };
        if outside {
            return (
                Kind::FileChange,
                "Change 1 file outside your project folder".into(),
                file,
                None,
                Some("Not from here — it is outside your project folder, so the project’s version control does not hold it.".into()),
            );
        }
        return (Kind::FileChange, "Change 1 file".into(), std::path::Path::new(&file).strip_prefix(cwd).ok().map(|p| p.to_string_lossy().into_owned()).unwrap_or(file), None, None);
    }
    if tool_name.starts_with("mcp__") {
        return (
            Kind::Connector,
            connector_name(tool_name),
            whole_detail(&input.to_string()),
            None,
            None,
        );
    }
    (
        Kind::Command,
        format!("Use {tool_name}"),
        whole_detail(&input.to_string()),
        None,
        None,
    )
}

fn always_covers(tool: &str) -> String {
    if tool == "Bash" { return "every command it runs".into(); }
    tool.strip_prefix("mcp__").and_then(|s| s.split_once("__")).map(|(server, name)| format!("{name} on {} again, and no other tool", server.strip_prefix("claude_ai_").unwrap_or(server).replace('_', " "))).unwrap_or_else(|| format!("every use of {tool}"))
}

fn connector_name(tool: &str) -> String {
    tool.strip_prefix("mcp__").and_then(|s| s.split_once("__")).map(|(server, name)| format!("Use {name} on {}", server.strip_prefix("claude_ai_").unwrap_or(server).replace('_', " "))).unwrap_or_else(|| format!("Use {tool}"))
}

pub fn plain_action(card: &Request) -> Option<String> {
    if card.kind != Kind::Command { return None; }
    let pattern = regex::Regex::new(r"^touch ([A-Za-z0-9_./-]+)$").unwrap();
    pattern.captures(&card.detail).map(|caps| format!("Makes {} if it is not there yet.", &caps[1]))
}

/// `approval-channel.ts:protocolDecisionFor`
pub fn protocol_decision_for(decision: &Decision) -> &'static str {
    match decision {
        Decision::ApproveOnce => "accept",
        Decision::ApproveAlways => "acceptForSession",
        Decision::Deny => "reject",
    }
}

/// Codex JSON-RPC result for one answer.
pub fn protocol_answer_for(answer: &Answer) -> Value {
    match answer {
        Answer::Decision { decision, reason, .. } => {
            let mut result = json!({"decision": protocol_decision_for(decision)});
            if let (Decision::Deny, Some(why)) = (decision, reason) {
                if !why.is_empty() {
                    result["reason"] = json!(why);
                }
            }
            result
        }
    }
}

/// Claude control_response body for one answer.
pub fn claude_behavior_for(answer: &Answer, input: Option<&Value>) -> Value {
    match answer {
        Answer::Decision {
            decision: Decision::Deny,
            reason,
            ..
        } => {
            let message = reason
                .as_deref()
                .filter(|s| !s.is_empty())
                .unwrap_or("The person declined this in Locust.");
            json!({"behavior":"deny","message": message})
        }
        Answer::Decision { .. } => {
            json!({"behavior":"allow","updatedInput": input.cloned().unwrap_or(json!({}))})
        }
    }
}

fn new_id() -> String {
    format!(
        "ap_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    )
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

struct Pending {
    request: Request,
    tx: SyncSender<Answer>,
}

/// In-process approval channel: raise a card, wait for decide(), release on turn end.
pub struct Channel {
    inner: Mutex<Inner>,
}

struct Inner {
    pending: HashMap<String, Pending>,
    /// Mission ids whose turn has ended: later answers are refused.
    ended: HashMap<String, Instant>,
    /// Answers the turn loop still owes a `mission.approval` record for.
    recorded: Vec<Recorded>,
    always: HashMap<(String, String), ()>,
}

impl Channel {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Inner {
                pending: HashMap::new(),
                ended: HashMap::new(),
                recorded: Vec::new(),
                always: HashMap::new(),
            }),
        })
    }

    /// Drain answered cards so the turn's ledger writer can append them.
    pub fn take_recorded(&self) -> Vec<Recorded> {
        std::mem::take(&mut self.inner.lock().unwrap().recorded)
    }

    pub fn take_recorded_for(&self, mission_id: &str) -> Vec<Recorded> {
        let mut guard = self.inner.lock().unwrap();
        let (mine, others) = std::mem::take(&mut guard.recorded).into_iter().partition(|r| r.mission_id == mission_id);
        guard.recorded = others;
        mine
    }

    pub fn pending_count(&self) -> usize {
        self.inner.lock().unwrap().pending.len()
    }

    /// Cards waiting on the person, oldest first.
    pub fn waiting(&self) -> Vec<Request> {
        let guard = self.inner.lock().unwrap();
        let mut rows: Vec<_> = guard.pending.values().map(|p| p.request.clone()).collect();
        rows.sort_by(|a, b| a.requested_at.cmp(&b.requested_at));
        rows
    }

    pub fn waiting_for_mission(&self, mission_id: &str) -> Vec<Request> {
        self.waiting()
            .into_iter()
            .filter(|r| r.mission_id == mission_id)
            .collect()
    }

    /// Raise a Codex approval and block until answered, cancelled, or the turn ends.
    pub fn ask_codex(
        &self,
        run_id: &str,
        mission_id: &str,
        method: &str,
        params: &Value,
        wire_id: Value,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Value, String> {
        let Some((kind, summary, detail, cwd)) = describe_codex(method, params) else {
            return Ok(json!({"decision":"reject"}));
        };
        let request = Request {
            approval_id: new_id(),
            run_id: run_id.into(),
            mission_id: mission_id.into(),
            runtime: "codex".into(),
            kind,
            summary,
            detail,
            cwd,
            requested_at: now_iso(),
            always_covers: None,
            always_key: None,
            data_sent_says: None,
            reversible_says: None,
            tool_use_id: params["itemId"].as_str().map(str::to_owned),
            wire_id,
            claude_input: None,
        };
        self.ask(request, cancel).map(|answer| protocol_answer_for(&answer))
    }

    /// Raise a Claude `can_use_tool` card and block until answered.
    pub fn ask_claude(
        &self,
        run_id: &str,
        mission_id: &str,
        request_id: &str,
        tool_name: &str,
        input: &Value,
        cwd: &str,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Value, String> {
        let (kind, summary, detail, data_sent, reversible) =
            describe_claude_tool(tool_name, input, cwd);
        let request = Request {
            approval_id: new_id(),
            run_id: run_id.into(),
            mission_id: mission_id.into(),
            runtime: "claude".into(),
            kind,
            summary,
            detail,
            cwd: if cwd.is_empty() {
                None
            } else {
                Some(cwd.to_owned())
            },
            requested_at: now_iso(),
            always_covers: Some(always_covers(tool_name)),
            always_key: Some(format!("claude:{tool_name}")),
            data_sent_says: data_sent,
            reversible_says: reversible,
            tool_use_id: input["tool_use_id"].as_str().map(str::to_owned),
            wire_id: json!(request_id),
            claude_input: Some(input.clone()),
        };
        let input_kept = request.claude_input.clone();
        self.ask(request, cancel)
            .map(|answer| claude_behavior_for(&answer, input_kept.as_ref()))
    }

    /// ACP options are selected by kind; Always stays in this run, never in the agent.
    pub fn ask_acp_runtime(&self, runtime: &str, run_id: &str, mission_id: &str, params: &Value, cwd: &str, cancel: &dyn Fn() -> bool) -> Result<bool, String> {
        let call = &params["toolCall"];
        let call_kind=call["kind"].as_str().unwrap_or("");
        let title=call["title"].as_str().unwrap_or("Use a tool");
        let shell=call_kind=="execute" || ((call_kind.is_empty() || call_kind=="other") && matches!(title.trim().to_lowercase().as_str(),"bash"|"shell"|"powershell"|"pwsh"|"cmd"|"execute"));
        let command=call["rawInput"]["command"].as_str();
        let mut paths:Vec<&str>=call["locations"].as_array().into_iter().flatten().filter_map(|l|l["path"].as_str()).collect();
        paths.extend(call["content"].as_array().into_iter().flatten().filter(|d|d["type"]=="diff").filter_map(|d|d["path"].as_str()));
        for field in ["path","file_path","filePath"] {if let Some(path)=call["rawInput"][field].as_str() {paths.push(path);}}
        let mut seen=std::collections::HashSet::new();paths.retain(|p|seen.insert(*p));
        let target=command.map(str::to_owned).or_else(||(!shell && !paths.is_empty()).then(||paths.join("\n")));
        let file=matches!(call_kind,"edit"|"delete"|"move");
        let kind=if file {Kind::FileChange} else {Kind::Command};
        let summary=if file {"Change files"} else {"Run a command"};
        let patch=crate::runtime::acp_events::patch_of(&call["content"]);
        let fallback=if shell {command.unwrap_or("").to_owned()} else if let Some(target)=&target {format!("{title}: {target}")} else {title.to_owned()};
        let detail=patch.as_ref().and_then(|p|p["text"].as_str()).unwrap_or(&fallback);
        let request = Request {
            approval_id: new_id(), run_id: run_id.into(), mission_id: mission_id.into(), runtime: runtime.into(),
            kind, summary: bounded(summary), detail: bounded(detail), cwd: Some(cwd.into()), requested_at: now_iso(),
            always_covers: target.as_ref().map(|_|if command.is_some() {"this same command again".into()} else {"the same change to these files again".into()}), always_key: target.map(|target|format!("acp:{call_kind}:{target}")),
            data_sent_says: None, reversible_says: None, tool_use_id: call["toolCallId"].as_str().map(str::to_owned),
            wire_id: Value::Null, claude_input: None,
        };
        self.ask(request, cancel).map(|a| matches!(a, Answer::Decision {decision: Decision::ApproveOnce | Decision::ApproveAlways, ..}))
    }

    fn ask(&self, request: Request, cancel: &dyn Fn() -> bool) -> Result<Answer, String> {
        let (tx, rx) = mpsc::sync_channel(1);
        let id = request.approval_id.clone();
        {
            let mut guard = self.inner.lock().unwrap();
            if cancel() || guard.ended.contains_key(&request.mission_id) {
                return Err("This turn already ended; the card is no longer waiting.".into());
            }
            if !asks_every_time(&request) && request.always_key.as_ref().is_some_and(|key| guard.always.contains_key(&(request.mission_id.clone(), key.clone()))) {
                let answer = Answer::Decision { approval_id: id, decision: Decision::ApproveOnce, reason: None };
                let mut record = record_of(&request, &answer);
                record.by = "earlier-always".into();
                guard.recorded.push(record);
                return Ok(answer);
            }
            if guard.pending.len() >= MAX_PENDING { return Err("Too many approvals waiting.".into()); }
            guard.pending.insert(id.clone(), Pending { request, tx });
        }
        let answer = wait_answer(&rx, cancel);
        // If wait ended without decide(), drop the pending entry.
        {
            let mut guard = self.inner.lock().unwrap();
            guard.pending.remove(&id);
        }
        answer
    }

    /// The person's answer. False when nothing was waiting, or the turn already ended.
    pub fn decide(&self, answer: Answer) -> Result<Recorded, String> {
        let mut guard = self.inner.lock().unwrap();
        let pending = match guard.pending.remove(answer.approval_id()) {
            Some(p) => p,
            None => {
                // Already answered, or turn ended: refuse visibly.
                return Err("That approval is no longer waiting. An answer after the turn ended is refused.".into());
            }
        };
        if guard.ended.contains_key(&pending.request.mission_id) {
            let _ = pending.tx.try_send(Answer::Decision {
                approval_id: pending.request.approval_id.clone(),
                decision: Decision::Deny,
                reason: Some("The turn ended before this was answered.".into()),
            });
            return Err("That approval is no longer waiting. An answer after the turn ended is refused.".into());
        }
        if matches!(&answer, Answer::Decision { decision: Decision::ApproveAlways, .. }) && asks_every_time(&pending.request) {
            guard.pending.insert(pending.request.approval_id.clone(), pending);
            return Err("This action must be asked about every time. Choose Approve once or Deny.".into());
        }
        if matches!(&answer, Answer::Decision { decision: Decision::ApproveAlways, .. }) {
            if let Some(key) = &pending.request.always_key {
                guard.always.insert((pending.request.mission_id.clone(), key.clone()), ());
            }
        }
        let recorded = record_of(&pending.request, &answer);
        guard.recorded.push(recorded.clone());
        if pending.tx.try_send(answer).is_err() {
            if let Some(key) = &pending.request.always_key { guard.always.remove(&(pending.request.mission_id.clone(), key.clone())); }
            guard.recorded.pop();
            return Err("That approval is no longer waiting.".into());
        }
        Ok(recorded)
    }

    /// The run is over: refuse whatever it was still asking.
    pub fn release(&self, mission_id: &str) {
        let mut guard = self.inner.lock().unwrap();
        guard.ended.insert(mission_id.to_owned(), Instant::now());
        guard.always.retain(|(mission, _), _| mission != mission_id);
        let ids: Vec<_> = guard
            .pending
            .iter()
            .filter(|(_, p)| p.request.mission_id == mission_id)
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            if let Some(pending) = guard.pending.remove(&id) {
                let _ = pending.tx.try_send(Answer::Decision {
                    approval_id: id,
                    decision: Decision::Deny,
                    reason: Some("The turn ended before this was answered.".into()),
                });
            }
        }
    }
}

fn wait_answer(rx: &Receiver<Answer>, cancel: &dyn Fn() -> bool) -> Result<Answer, String> {
    loop {
        if cancel() {
            return Err("Interrupted while waiting for approval".into());
        }
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(answer) => return Ok(answer),
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => {
                return Err("Approval channel closed".into());
            }
        }
    }
}

fn utf16_bounded(text: &str, limit: usize) -> String {
    let mut units = 0;
    text.chars().take_while(|ch| { units += ch.len_utf16(); units <= limit }).collect()
}

fn record_of(request: &Request, answer: &Answer) -> Recorded {
    let asked = [request.summary.as_str(), request.detail.as_str()]
        .into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .fold(Vec::new(), |mut all, s| { if !all.contains(&s) { all.push(s); } all })
        .join("\n");
    let (given, words) = match answer {
        Answer::Decision {
            decision: Decision::Deny,
            reason,
            ..
        } => ("denied", reason.clone()),
        Answer::Decision {
            decision: Decision::ApproveAlways,
            ..
        } => ("allowed-always", None),
        Answer::Decision { .. } => ("allowed", None),
    };
    Recorded {
        mission_id: request.mission_id.clone(),
        approval_id: request.approval_id.clone(),
        kind: request.kind.clone(),
        asked: if asked.is_empty() { "(the card said nothing more)".into() } else { utf16_bounded(&asked, 2_000) },
        answer: given.into(),
        by: "card".into(),
        words: words.map(|w| utf16_bounded(w.trim(), 1_000)),
        asked_at: request.requested_at.clone(),
        occurred_at: now_iso(),
    }
}

impl Recorded {
    pub fn to_json(&self) -> Value {
        let mut v = json!({
            "approvalId": self.approval_id,
            "kind": self.kind.as_str(),
            "asked": self.asked,
            "answer": self.answer,
            "by": self.by,
            "askedAt": self.asked_at,
            "occurredAt": self.occurred_at,
        });
        if let Some(words) = &self.words {
            if !words.is_empty() {
                v["words"] = json!(words);
            }
        }
        v
    }
}

/// Words shared with Electron's ApprovalCard.tsx / approval-data.ts.
// Conservative until the complete shell parser is ported: even quoted process
// control names cannot acquire or use a broad session grant.
pub fn asks_every_time(card: &Request) -> bool {
    card.kind == Kind::Command && regex::Regex::new(r"(?i)\b(taskkill|tskill|stop-process|spps|kill|pkill|killall|wmic|invoke-cimmethod|fuser|kill-port|shutdown|restart-computer|reboot|stop-computer|poweroff|halt|systemctl)\b").unwrap().is_match(&card.detail)
}

pub fn card_note(card: &Request) -> String {
    if asks_every_time(card) { return "Nothing has happened yet. A command that reaches beyond this run is asked about every time.".into(); }
    let mut note = card.always_covers.as_ref().map(|covers| format!("Nothing has happened yet. “Always” allows {covers}, until this run ends.")).unwrap_or_else(|| "Nothing has happened yet. “Always” lasts until this run ends.".into());
    if card.always_key.is_some() { note.push_str(" Your saved rules still come first."); }
    note
}
pub fn effect_lines(card: &Request) -> Vec<(&'static str, String)> {
    let network = regex::Regex::new(r"(?i)(^|[\s|&;(<>])(curl|wget|ssh|scp|sftp|rsync|nc|netcat|telnet|ftp|npm|pnpm|yarn|pip|pip3|cargo|gem|go|brew|apt|docker|kubectl|aws|gcloud|az|gh|git)(\s|$)|https?://").unwrap().is_match(&card.detail);
    if card.kind == Kind::Command && !network && card.data_sent_says.is_none() && card.reversible_says.is_none() {
        return vec![("Effects", "Unknown. Locust cannot tell what a command sends or undo what it does; it can do anything the sandbox allows.".into())];
    }
    let data = card.data_sent_says.clone().unwrap_or_else(|| if card.kind == Kind::FileChange { "Nothing. The change is written to this machine and sent nowhere.".into() } else if card.kind == Kind::Command { "This command can reach the network. Locust cannot see what it would send — read it above.".into() } else { "The input above, to the service the connector reaches. It acts there, not on this machine.".into() });
    let undo = card.reversible_says.clone().unwrap_or_else(|| if card.kind == Kind::FileChange { "Yes for tracked files, if the workspace is under version control.".into() } else if card.kind == Kind::Command { "Unknown — a command can do anything the workspace sandbox allows.".into() } else { "Unknown — a connector acts on the service it reaches, and Locust cannot undo what happens there.".into() });
    vec![("Data sent",data),("Reversible",undo)]
}

/// Status line words while a card waits (Electron: "needs you" / "waiting on you").
pub fn needs_you_status(count: usize) -> String {
    match count {
        0 => "Ready".into(),
        1 => "Needs you".into(),
        n => format!("{n} need you"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_are_scoped_and_card_words_match_electron() {
        let ch = Channel::new();
        let mut request = Request { approval_id: "a".into(), mission_id: "one".into(), run_id: "r1".into(), runtime: "codex".into(), kind: Kind::Command, summary: "same".into(), detail: "same".into(), cwd: None, requested_at: now_iso(), always_covers: None, always_key: None, data_sent_says: None, reversible_says: None, tool_use_id: None, wire_id: json!(1), claude_input: None };
        let answer = Answer::Decision { approval_id: "a".into(), decision: Decision::Deny, reason: Some("stop".into()) };
        let first = record_of(&request, &answer);
        assert_eq!(first.asked, "same");
        assert_eq!(first.to_json()["by"], "card");
        assert!(first.to_json().get("mission_id").is_none());
        request.mission_id = "two".into();
        ch.inner.lock().unwrap().recorded.extend([first, record_of(&request, &answer)]);
        assert_eq!(ch.take_recorded_for("one").len(), 1);
        assert!(ch.take_recorded_for("one").is_empty());
        assert_eq!(ch.take_recorded_for("two").len(), 1);
        assert_eq!(card_note(&request), "Nothing has happened yet. “Always” lasts until this run ends.");
        assert_eq!(effect_lines(&request)[0].0, "Effects");
        request.detail = "touch hello.txt".into();
        assert_eq!(plain_action(&request).as_deref(), Some("Makes hello.txt if it is not there yet."));
        request.detail = "curl https://example.com".into();
        assert_eq!(effect_lines(&request)[0].0, "Data sent");
        request.detail = "pkill python".into();
        assert!(asks_every_time(&request));
        assert_eq!(card_note(&request), "Nothing has happened yet. A command that reaches beyond this run is asked about every time.");
        request.kind = Kind::FileChange;
        assert_eq!(effect_lines(&request)[0].1, "Nothing. The change is written to this machine and sent nowhere.");
    }

    #[test]
    fn ended_or_cancelled_requests_never_raise_a_card() {
        let ch = Channel::new(); ch.release("ended");
        assert!(ch.ask_claude("r", "ended", "a", "Write", &json!({}), "/tmp", &|| false).is_err());
        assert!(ch.ask_claude("r", "cancelled", "b", "Write", &json!({}), "/tmp", &|| true).is_err());
        assert!(ch.waiting().is_empty());
    }

    #[test]
    fn claude_always_is_only_for_the_chosen_tool_in_this_turn() {
        let ch = Channel::new(); let worker = ch.clone();
        let handle = std::thread::spawn(move || worker.ask_claude("r", "m", "a", "Write", &json!({}), "/tmp", &|| false));
        let end = Instant::now() + Duration::from_secs(5);
        while ch.waiting().is_empty() { assert!(Instant::now() < end); std::thread::sleep(Duration::from_millis(5)); }
        ch.decide(Answer::Decision { approval_id: ch.waiting()[0].approval_id.clone(), decision: Decision::ApproveAlways, reason: None }).unwrap();
        assert_eq!(handle.join().unwrap().unwrap()["behavior"], "allow");
        assert_eq!(ch.ask_claude("r", "m", "b", "Write", &json!({}), "/tmp", &|| false).unwrap()["behavior"], "allow");
        assert_eq!(ch.take_recorded_for("m")[1].by, "earlier-always");
        let worker = ch.clone();
        let handle = std::thread::spawn(move || worker.ask_claude("r", "m", "c", "Bash", &json!({}), "/tmp", &|| false));
        while ch.waiting().is_empty() { assert!(Instant::now() < end); std::thread::sleep(Duration::from_millis(5)); }
        ch.release("m");
        assert_eq!(handle.join().unwrap().unwrap()["behavior"], "deny");
        assert!(ch.inner.lock().unwrap().always.is_empty());
    }

    #[test]
    fn ledger_text_bounds_match_typescript_utf16_lengths() {
        assert_eq!(utf16_bounded(&"😀".repeat(1500), 2000).encode_utf16().count(), 2000);
        assert_eq!(utf16_bounded("a😀", 2), "a");
    }

    #[test]
    fn sibling_and_parent_paths_are_outside_the_project() {
        for path in ["/project-other/a", "../a", "./../a", "/project/../a"] {
            let (_, summary, _, _, undo) = describe_claude_tool("Write", &json!({"file_path":path}), "/project");
            assert!(summary.contains("outside"), "{path}");
            assert!(undo.unwrap().starts_with("Not from here"));
        }
        assert_eq!(describe_claude_tool("Write", &json!({"file_path":"/project/a"}), "/project").2, "a");
    }

    #[test]
    fn codex_policy_approve_each_asks() {
        let p = codex_app_server_policy(Mode::ApproveEach, Some("read-only"));
        assert_eq!(p.approval_policy, "untrusted");
        assert_eq!(p.sandbox, "workspace-write");
    }

    #[test]
    fn codex_policy_ask_never() {
        let p = codex_app_server_policy(Mode::Ask, None);
        assert_eq!(p.approval_policy, "never");
        assert_eq!(p.sandbox, "read-only");
    }

    #[test]
    fn describe_command_and_claude_bash() {
        let (k, s, d, _) = describe_codex(
            "item/commandExecution/requestApproval",
            &json!({"command":"touch x","cwd":"/tmp"}),
        )
        .unwrap();
        assert_eq!(k, Kind::Command);
        assert_eq!(s, "Run a command");
        assert_eq!(d, "touch x");
        let (k, s, d, _, _) = describe_claude_tool("Bash", &json!({"command":"ls"}), "/tmp");
        assert_eq!(k, Kind::Command);
        assert_eq!(s, "Run a command");
        assert_eq!(d, "ls");
    }

    #[test]
    fn allow_deny_and_after_end_refused() {
        let ch = Channel::new();
        let ch2 = ch.clone();
        let handle = std::thread::spawn(move || {
            ch2.ask_codex(
                "run_1",
                "mission_1",
                "item/commandExecution/requestApproval",
                &json!({"command":"echo hi"}),
                json!(1),
                &|| false,
            )
        });
        // Wait until pending.
        for _ in 0..100 {
            if ch.pending_count() == 1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(ch.pending_count(), 1);
        assert_eq!(needs_you_status(1), "Needs you");
        let req = ch.waiting()[0].clone();
        let recorded = ch
            .decide(Answer::Decision {
                approval_id: req.approval_id.clone(),
                decision: Decision::ApproveOnce,
                reason: None,
            })
            .unwrap();
        assert_eq!(recorded.answer, "allowed");
        let result = handle.join().unwrap().unwrap();
        assert_eq!(result["decision"], "accept");

        // After release, decide is refused.
        let ch = Channel::new();
        let ch2 = ch.clone();
        let handle = std::thread::spawn(move || {
            ch2.ask_claude(
                "run_1",
                "mission_2",
                "req_1",
                "Bash",
                &json!({"command":"rm -rf /"}),
                "/tmp",
                &|| false,
            )
        });
        for _ in 0..100 {
            if ch.pending_count() == 1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let id = ch.waiting()[0].approval_id.clone();
        ch.release("mission_2");
        let err = ch
            .decide(Answer::Decision {
                approval_id: id,
                decision: Decision::ApproveOnce,
                reason: None,
            })
            .unwrap_err();
        assert!(err.contains("no longer waiting") || err.contains("turn ended"), "{err}");
        let denied = handle.join().unwrap().unwrap();
        assert_eq!(denied["behavior"], "deny");
    }
}

//! Approvals: what a runtime asks mid-run, and what the person answers.
//!
//! Ports `apps/desktop/src/main/approval-channel.ts` (describe / decide / protocol
//! answers), `permission-host.ts:builtInOrConnector` for Claude `can_use_tool`, and
//! `commands.ts:codexAppServerPolicy` for mode → sandbox / approvalPolicy.
//! Saved approval rules are consulted before session Always and before a card is raised.

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
const MAX_DETAIL: usize = 20_000;

/// Product modes Locust names on a teammate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
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

    /// The sandbox Electron's host gives each mode: Ask and Plan read only, Accept edits and Approve each write in the
    /// folder, Auto runs as the person.
    pub fn sandbox(self) -> &'static str {
        match self {
            Self::Ask | Self::Plan => "read-only",
            Self::AcceptEdits | Self::ApproveEach => "workspace-write",
            Self::Auto => "full-access",
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
    // The sandbox the mode earns, as Electron's host gives it, unless the caller names one.
    let chosen = match sandbox_hint {
        Some("full-access") | Some("danger-full-access") => "full-access",
        Some("workspace-write") => "workspace-write",
        Some("read-only") => "read-only",
        _ => mode.sandbox(),
    };
    // Approve-each stops (`untrusted`); never paired with full access.
    if mode == Mode::ApproveEach {
        return CodexPolicy {
            sandbox: if chosen == "full-access" { "workspace-write" } else { chosen },
            approval_policy: "untrusted",
        };
    }
    if chosen == "full-access" {
        return CodexPolicy {
            sandbox: "danger-full-access",
            approval_policy: "never",
        };
    }
    CodexPolicy {
        sandbox: chosen,
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

impl Decision {}

/// One option of a question the runtime asked; its label is its identity (the protocol gives it no id or index).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QuestionOption {
    pub label: String,
    pub description: Option<String>,
}

/// One question of a Codex `item/tool/requestUserInput` (approval-channel.ts MissionQuestion).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Question {
    pub id: String,
    pub header: Option<String>,
    pub question: String,
    pub options: Vec<QuestionOption>,
    /// The person may write their own answer.
    pub is_other: bool,
    pub is_secret: bool,
}

/// How many questions one request may carry, and how many options each may offer.
const MAX_QUESTIONS: usize = 12;
const MAX_OPTIONS: usize = 24;

/// The `questions` array, defensively: a malformed entry is dropped, not guessed at (questionsOf). A question with no
/// id has nowhere for its answer to go, and an option with no label cannot be chosen, so neither is offered.
pub fn questions_of(params: &Value) -> Vec<Question> {
    let bounded = |v: &Value| v.as_str().map(whole_detail).unwrap_or_default();
    params["questions"]
        .as_array()
        .into_iter()
        .flatten()
        .take(MAX_QUESTIONS)
        .filter_map(|entry| {
            let id = entry["id"].as_str().filter(|id| !id.is_empty())?;
            let question = bounded(&entry["question"]);
            if question.is_empty() {
                return None;
            }
            let options = entry["options"]
                .as_array()
                .into_iter()
                .flatten()
                .take(MAX_OPTIONS)
                .filter_map(|o| {
                    let label = bounded(&o["label"]);
                    (!label.is_empty()).then(|| QuestionOption { label, description: o["description"].as_str().map(whole_detail) })
                })
                .collect();
            Some(Question {
                id: id.into(),
                header: entry["header"].as_str().map(whole_detail),
                question,
                options,
                is_other: entry["isOther"] == true,
                is_secret: entry["isSecret"] == true,
            })
        })
        .collect()
}

/// Questions as the wire spells them, so a card crosses into the engine process whole (read back by `questions_of`).
pub fn questions_json(questions: &[Question]) -> Value {
    Value::Array(
        questions
            .iter()
            .map(|q| {
                json!({"id": q.id, "header": q.header, "question": q.question, "isOther": q.is_other, "isSecret": q.is_secret,
                    "options": q.options.iter().map(|o| json!({"label": o.label, "description": o.description})).collect::<Vec<_>>()})
            })
            .collect(),
    )
}

#[derive(Clone, Debug)]
pub struct Request {
    pub approval_id: String,

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

    /// Wire id the runtime needs to answer (Codex JSON-RPC id, or Claude request_id).

    /// Claude `can_use_tool` input, kept so allow can return updatedInput.
    pub claude_input: Option<Value>,
    /// What a question card asks, each answered by id (empty for an authorization).
    pub questions: Vec<Question>,
}

#[derive(Clone, Debug)]
pub enum Answer {
    Decision {
        approval_id: String,
        decision: Decision,
        reason: Option<String>,
    },
    /// A question's answers, by question id; a chosen option by its literal label. An empty list is a question the
    /// person left alone, kept present rather than absent.
    Answers {
        approval_id: String,
        answers: Vec<(String, Vec<String>)>,
    },
}

impl Answer {
    pub fn approval_id(&self) -> &str {
        match self {
            Self::Decision { approval_id, .. } | Self::Answers { approval_id, .. } => approval_id,
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

fn whole_detail(value: &str) -> String {
    let normalized = value.replace("\r\n", "\n").replace('\r', "\n");
    let text = normalized.trim();
    let length = text.encode_utf16().count();
    if length <= MAX_DETAIL {
        return text.to_owned();
    }
    let rest = length - MAX_DETAIL;
    let count = rest
        .to_string()
        .chars()
        .rev()
        .enumerate()
        .fold(String::new(), |mut out, (i, ch)| {
            if i > 0 && i % 3 == 0 {
                out.push(',');
            }
            out.push(ch);
            out
        })
        .chars()
        .rev()
        .collect::<String>();
    format!(
        "{}\n\n[{count} more character{} not shown here. Deny if you cannot see enough to decide.]",
        utf16_bounded(text, MAX_DETAIL),
        if rest == 1 { "" } else { "s" }
    )
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
pub fn describe_codex(
    method: &str,
    params: &Value,
) -> Option<(Kind, String, String, Option<String>)> {
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
            // The questions array first; the singular keys still read, last, so a question is never unshown again.
            let questions = questions_of(params);
            let detail = match questions.first() {
                Some(first) => first.question.clone(),
                None => whole_detail(params["question"].as_str().or_else(|| params["prompt"].as_str()).or_else(|| params["message"].as_str()).unwrap_or("")),
            };
            let summary = if questions.len() > 1 { format!("Answer {} questions", questions.len()) } else { "Answer a question".into() };
            Some((kind, summary, detail, cwd))
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
            .map(|s| {
                s.replace(['\n', '\r'], " ")
                    .trim()
                    .chars()
                    .take(600)
                    .collect()
            })
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
        let host = url
            .split_once("://")
            .filter(|(scheme, _)| *scheme == "https" || *scheme == "http")
            .map(|(_, rest)| rest.split(['/', '?', '#']).next().unwrap_or(""));
        return (
            Kind::Connector,
            host.filter(|h| !h.is_empty()).map(|host| format!("Open a page on {host}")).unwrap_or_else(|| "Open a web page".into()),
            [url, text(&input["prompt"])].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n"),
            Some("The whole address above, to that site. Read it: anything written into an address is sent with it.".into()),
            Some("Opening a page changes nothing here. A site can act on an address it is sent.".into()),
        );
    }
    if tool_name == "Bash" {
        let command = input["command"]
            .as_str()
            .map(whole_detail)
            .unwrap_or_default();
        let summary = if command.is_empty() {
            "Run a command it did not describe".into()
        } else {
            "Run a command".into()
        };
        return (Kind::Command, summary, command, None, None);
    }
    if matches!(tool_name, "Edit" | "Write" | "MultiEdit" | "NotebookEdit") {
        let file = {
            let a = input["file_path"].as_str().unwrap_or("").to_owned();
            if a.is_empty() {
                input["notebook_path"].as_str().unwrap_or("").to_owned()
            } else {
                a
            }
        };
        let outside = !file.is_empty() && !cwd.is_empty() && {
            let path = std::path::Path::new(&file);
            path.components()
                .any(|c| c == std::path::Component::ParentDir)
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
        return (
            Kind::FileChange,
            "Change 1 file".into(),
            std::path::Path::new(&file)
                .strip_prefix(cwd)
                .ok()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or(file),
            None,
            None,
        );
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
    if tool == "Bash" {
        return "every command it runs".into();
    }
    tool.strip_prefix("mcp__")
        .and_then(|s| s.split_once("__"))
        .map(|(server, name)| {
            format!(
                "{name} on {} again, and no other tool",
                server
                    .strip_prefix("claude_ai_")
                    .unwrap_or(server)
                    .replace('_', " ")
            )
        })
        .unwrap_or_else(|| format!("every use of {tool}"))
}

fn connector_name(tool: &str) -> String {
    tool.strip_prefix("mcp__")
        .and_then(|s| s.split_once("__"))
        .map(|(server, name)| {
            format!(
                "Use {name} on {}",
                server
                    .strip_prefix("claude_ai_")
                    .unwrap_or(server)
                    .replace('_', " ")
            )
        })
        .unwrap_or_else(|| format!("Use {tool}"))
}

pub fn plain_action(card: &Request) -> Option<String> {
    if card.kind != Kind::Command {
        return None;
    }
    crate::command_plain::plain_command(&card.detail).map(|plain| plain.said)
}

/// Reach classifier for the card (Electron commandReach on the command detail).
pub fn command_reach_of(card: &Request) -> Option<crate::command_reach::CommandReach> {
    if card.kind != Kind::Command {
        return None;
    }
    crate::command_reach::command_reach(Some(&card.detail))
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
        Answer::Answers { answers, .. } => {
            let mut map = serde_json::Map::new();
            for (id, given) in answers {
                map.insert(id.clone(), json!({"answers": given}));
            }
            json!({"answers": map})
        }
        Answer::Decision {
            decision, reason, ..
        } => {
            let mut result = json!({"decision": protocol_decision_for(decision)});
            if let (Decision::Deny, Some(why)) = (decision, reason)
                && !why.is_empty()
            {
                result["reason"] = json!(why);
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
        // A Claude card is an authorization: answers never reach one (the channel checks the pairing first).
        Answer::Answers { .. } => json!({"behavior":"deny","message":"The person declined this in Locust."}),
    }
}

fn new_id() -> String {
    format!(
        "ap_{}_{}",
        std::process::id(),
        crate::invariants::unique_nanos()
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
    teammates: HashMap<String, String>,
}

impl Channel {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Inner {
                pending: HashMap::new(),
                ended: HashMap::new(),
                recorded: Vec::new(),
                always: HashMap::new(),
                teammates: HashMap::new(),
            }),
        })
    }

    pub fn register_teammate(&self, mission_id: &str, teammate_id: &str) {
        crate::invariants::lock(&self.inner)
            .teammates
            .insert(mission_id.into(), teammate_id.into());
    }

    pub fn teammate_for(&self, mission_id: &str) -> Option<String> {
        crate::invariants::lock(&self.inner)
            .teammates
            .get(mission_id)
            .cloned()
    }

    /// Drain answered cards so the turn's ledger writer can append them.
    #[cfg(any(test, feature = "test-hooks"))]
    pub fn take_recorded(&self) -> Vec<Recorded> {
        std::mem::take(&mut crate::invariants::lock(&self.inner).recorded)
    }

    pub fn take_recorded_for(&self, mission_id: &str) -> Vec<Recorded> {
        let mut guard = crate::invariants::lock(&self.inner);
        let (mine, others) = std::mem::take(&mut guard.recorded)
            .into_iter()
            .partition(|r| r.mission_id == mission_id);
        guard.recorded = others;
        mine
    }

    #[cfg(any(test, feature = "test-hooks"))]
    pub fn pending_count(&self) -> usize {
        crate::invariants::lock(&self.inner).pending.len()
    }

    /// Cards waiting on the person, oldest first.
    pub fn waiting(&self) -> Vec<Request> {
        let guard = crate::invariants::lock(&self.inner);
        let mut rows: Vec<_> = guard.pending.values().map(|p| p.request.clone()).collect();
        rows.sort_by(|a, b| a.requested_at.cmp(&b.requested_at));
        rows
    }

    /// Cards one turn raised that wait on the person.
    pub fn waiting_for_mission(&self, mission_id: &str) -> Vec<Request> {
        self.waiting()
            .into_iter()
            .filter(|r| r.mission_id == mission_id)
            .collect()
    }

    /// Raise a Codex approval and block until answered, cancelled, or the turn ends.
    pub fn ask_codex(
        &self,
        _run_id: &str,

        mission_id: &str,
        method: &str,
        params: &Value,

        _wire_id: Value,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Value, String> {
        let Some((kind, summary, detail, cwd)) = describe_codex(method, params) else {
            return Ok(json!({"decision":"reject"}));
        };
        let questions = if kind == Kind::Question { questions_of(params) } else { Vec::new() };
        let request = Request {
            approval_id: new_id(),

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

            claude_input: None,
            questions,
        };
        self.ask(request, cancel)
            .map(|answer| protocol_answer_for(&answer))
    }

    /// Raise a Claude `can_use_tool` card and block until answered.
    pub fn ask_claude(
        &self,
        _run_id: &str,

        mission_id: &str,
        _request_id: &str,
        tool_name: &str,
        input: &Value,
        cwd: &str,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Value, String> {
        let (kind, summary, detail, data_sent, reversible) =
            describe_claude_tool(tool_name, input, cwd);
        let request = Request {
            approval_id: new_id(),

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

            claude_input: Some(input.clone()),

            questions: Vec::new(),
        };
        let input_kept = request.claude_input.clone();
        self.ask(request, cancel)
            .map(|answer| claude_behavior_for(&answer, input_kept.as_ref()))
    }

    /// ACP options are selected by kind; Always stays in this run, never in the agent.
    pub fn ask_acp_runtime(
        &self,
        runtime: &str,
        _run_id: &str,
        mission_id: &str,
        params: &Value,
        cwd: &str,
        cancel: &dyn Fn() -> bool,
    ) -> Result<bool, String> {
        let call = &params["toolCall"];
        let call_kind = call["kind"].as_str().unwrap_or("");
        let title = call["title"].as_str().unwrap_or("Use a tool");
        let shell = call_kind == "execute"
            || ((call_kind.is_empty() || call_kind == "other")
                && matches!(
                    title.trim().to_lowercase().as_str(),
                    "bash" | "shell" | "powershell" | "pwsh" | "cmd" | "execute"
                ));
        let command = call["rawInput"]["command"].as_str();
        let mut paths: Vec<&str> = call["locations"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|l| l["path"].as_str())
            .collect();
        paths.extend(
            call["content"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|d| d["type"] == "diff")
                .filter_map(|d| d["path"].as_str()),
        );
        for field in ["path", "file_path", "filePath"] {
            if let Some(path) = call["rawInput"][field].as_str() {
                paths.push(path);
            }
        }
        let mut seen = std::collections::HashSet::new();
        paths.retain(|p| seen.insert(*p));
        let target = command
            .map(str::to_owned)
            .or_else(|| (!shell && !paths.is_empty()).then(|| paths.join("\n")));
        let file = matches!(call_kind, "edit" | "delete" | "move");
        let kind = if file {
            Kind::FileChange
        } else {
            Kind::Command
        };
        let summary = if file {
            "Change files"
        } else {
            "Run a command"
        };
        let patch = crate::runtime::acp_events::patch_of(&call["content"]);
        let fallback = if shell {
            command.unwrap_or("").to_owned()
        } else if let Some(target) = &target {
            format!("{title}: {target}")
        } else {
            title.to_owned()
        };
        let detail = patch
            .as_ref()
            .and_then(|p| p["text"].as_str())
            .unwrap_or(&fallback);
        let request = Request {
            approval_id: new_id(),

            mission_id: mission_id.into(),
            runtime: runtime.into(),
            kind,
            summary: whole_detail(summary),
            detail: whole_detail(detail),
            cwd: Some(cwd.into()),
            requested_at: now_iso(),
            always_covers: target.as_ref().map(|_| {
                if command.is_some() {
                    "this same command again".into()
                } else {
                    "the same change to these files again".into()
                }
            }),
            always_key: target.map(|target| format!("acp:{call_kind}:{target}")),
            data_sent_says: None,
            reversible_says: None,

            claude_input: None,

            questions: Vec::new(),
        };
        self.ask(request, cancel).map(|a| {
            matches!(
                a,
                Answer::Decision {
                    decision: Decision::ApproveOnce | Decision::ApproveAlways,
                    ..
                }
            )
        })
    }

    /// Browser actions never inherit a command allow rule or Auto-mode bypass.
    pub fn ask_browser(&self, mission: &str, title: &str, detail: &str, cancel: &dyn Fn() -> bool) -> Result<bool, String> {
        let request = Request { approval_id: new_id(), mission_id: mission.into(), runtime: "browser".into(), kind: Kind::Connector, summary: whole_detail(title), detail: whole_detail(detail), cwd: None, requested_at: now_iso(), always_covers: None, always_key: None, data_sent_says: None, reversible_says: None, claude_input: None, questions: Vec::new() };
        self.ask(request, cancel).map(|a| matches!(a, Answer::Decision { decision: Decision::ApproveOnce | Decision::ApproveAlways, .. }))
    }

    fn ask(&self, request: Request, cancel: &dyn Fn() -> bool) -> Result<Answer, String> {
        let (tx, rx) = mpsc::sync_channel(1);
        let id = request.approval_id.clone();
        {
            let mut guard = crate::invariants::lock(&self.inner);
            if cancel() || guard.ended.contains_key(&request.mission_id) {
                return Err("This turn already ended; the card is no longer waiting.".into());
            }
            // Hard command guard (Electron locust-command-guard): refuse before a card.
            if request.kind == Kind::Command
                && let Some(reason) = crate::command_guard::stopped_because(
                    &request.detail,
                    &crate::command_guard::GuardContext {
                        fix_copy: crate::command_guard::in_fix_copy(&[request
                            .cwd
                            .as_deref()
                            .unwrap_or("")]),
                        started_pids: Vec::new(),
                    },
                )
            {
                let answer = Answer::Decision {
                    approval_id: id.clone(),
                    decision: Decision::Deny,
                    reason: Some(reason),
                };
                let mut record = record_of(&request, &answer);
                record.by = "command-guard".into();
                guard.recorded.push(record);
                return Ok(answer);
            }
            // Saved rules answer before a card (Electron decideByRules). Unreadable → every card asks.
            let saved =
                if request.runtime == "browser" { Ok(None) } else { decide_by_saved_rules(&request, guard.teammates.get(&request.mission_id).cloned()) };
            let readable = saved.is_ok();
            if let Ok(Some(auto)) = saved {
                let mut record = record_of(&request, &auto);
                record.by = "saved-rule".into();
                guard.recorded.push(record);
                return Ok(auto);
            }
            if readable
                && request.kind != Kind::Question
                && !asks_every_time(&request)
                && request.always_key.as_ref().is_some_and(|key| {
                    guard
                        .always
                        .contains_key(&(request.mission_id.clone(), key.clone()))
                })
            {
                let answer = Answer::Decision {
                    approval_id: id,
                    decision: Decision::ApproveOnce,
                    reason: None,
                };
                let mut record = record_of(&request, &answer);
                record.by = "earlier-always".into();
                guard.recorded.push(record);
                return Ok(answer);
            }
            if guard.pending.len() >= MAX_PENDING {
                return Err("Too many approvals waiting.".into());
            }
            guard.pending.insert(id.clone(), Pending { request, tx });
        }
        let answer = wait_answer(&rx, cancel);
        // If wait ended without decide(), drop the pending entry.
        {
            let mut guard = crate::invariants::lock(&self.inner);
            guard.pending.remove(&id);
        }
        answer
    }

    /// The person's answer. False when nothing was waiting, or the turn already ended.
    pub fn decide(&self, answer: Answer) -> Result<Recorded, String> {
        let mut guard = crate::invariants::lock(&self.inner);
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
            return Err(
                "That approval is no longer waiting. An answer after the turn ended is refused."
                    .into(),
            );
        }
        // A question is answered and an action authorized: neither reaches the other's request, since the runtime
        // would read the wrong shape as an empty answer. Refused, and the runtime told no.
        let answering = matches!(answer, Answer::Answers { .. });
        if answering != (pending.request.kind == Kind::Question) {
            let _ = pending.tx.try_send(Answer::Decision { approval_id: pending.request.approval_id.clone(), decision: Decision::Deny, reason: None });
            return Err(if answering {
                "Locust did not send that answer: it belongs to a question, and the runtime was waiting on an authorization."
            } else {
                "Locust did not send that decision: the runtime asked a question, which is answered rather than approved."
            }
            .into());
        }
        let answer = match &answer {
            Answer::Decision {
                approval_id,
                decision: Decision::ApproveAlways,
                reason,
            } => {
                let (decision, note) = crate::approval_rules::enforced_answer_decision(
                    request_view(&pending.request),
                    "approve-always",
                );
                if decision == "approve-once" {
                    if let Some(note) = note {
                        // Host forces once for reaching commands (Electron enforcedAnswer).
                        let _ = note;
                    }
                    Answer::Decision {
                        approval_id: approval_id.clone(),
                        decision: Decision::ApproveOnce,
                        reason: reason.clone(),
                    }
                } else {
                    answer.clone()
                }
            }
            _ => answer,
        };
        if matches!(
            &answer,
            Answer::Decision {
                decision: Decision::ApproveAlways,
                ..
            }
        ) && asks_every_time(&pending.request)
        {
            guard
                .pending
                .insert(pending.request.approval_id.clone(), pending);
            return Err(
                "This action must be asked about every time. Choose Approve once or Deny.".into(),
            );
        }
        if matches!(
            &answer,
            Answer::Decision {
                decision: Decision::ApproveAlways,
                ..
            }
        ) && let Some(key) = &pending.request.always_key
        {
            guard
                .always
                .insert((pending.request.mission_id.clone(), key.clone()), ());
        }
        let recorded = record_of(&pending.request, &answer);
        guard.recorded.push(recorded.clone());
        if pending.tx.try_send(answer).is_err() {
            if let Some(key) = &pending.request.always_key {
                guard
                    .always
                    .remove(&(pending.request.mission_id.clone(), key.clone()));
            }
            guard.recorded.pop();
            return Err("That approval is no longer waiting.".into());
        }
        Ok(recorded)
    }

    /// The run is over: refuse whatever it was still asking.
    pub fn release(&self, mission_id: &str) {
        let mut guard = crate::invariants::lock(&self.inner);
        guard.ended.insert(mission_id.to_owned(), Instant::now());
        guard.always.retain(|(mission, _), _| mission != mission_id);
        guard.teammates.remove(mission_id);
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
    text.chars()
        .take_while(|ch| {
            units += ch.len_utf16();
            units <= limit
        })
        .collect()
}

fn record_of(request: &Request, answer: &Answer) -> Recorded {
    let asked = [request.summary.as_str(), request.detail.as_str()]
        .into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .fold(Vec::new(), |mut all, s| {
            if !all.contains(&s) {
                all.push(s);
            }
            all
        })
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
        Answer::Answers { answers, .. } => {
            let said: Vec<String> = answers.iter().filter(|(_, given)| !given.is_empty()).map(|(_, given)| given.join(", ")).collect();
            ("answered", (!said.is_empty()).then(|| said.join("; ")))
        }
    };
    Recorded {
        mission_id: request.mission_id.clone(),
        approval_id: request.approval_id.clone(),
        kind: request.kind.clone(),
        asked: if asked.is_empty() {
            "(the card said nothing more)".into()
        } else {
            utf16_bounded(&asked, 2_000)
        },
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
        if let Some(words) = &self.words
            && !words.is_empty()
        {
            v["words"] = json!(words);
        }
        v
    }
}

/// Words shared with Electron's ApprovalCard.tsx / approval-data.ts.
/// A reaching command never gets Always / saved rules (Electron commandReach).
pub fn asks_every_time(card: &Request) -> bool {
    card.runtime == "browser" || command_reach_of(card).is_some()
}

fn request_view(card: &Request) -> crate::approval_rules::RequestView<'_> {
    crate::approval_rules::RequestView {
        kind: card.kind.as_str(),
        summary: &card.summary,
        detail: &card.detail,
        runtime: Some(card.runtime.as_str()),
        patch_text: None,
    }
}

/// Unreadable rules also block session Always; they are never treated as an empty store.
fn decide_by_saved_rules(
    request: &Request,
    teammate_id: Option<String>,
) -> Result<Option<Answer>, String> {
    let store = crate::native::store().ok_or(crate::approval_rule_store::UNREADABLE)?;
    let rules = crate::approval_rule_store::list(&store)?;
    let action = crate::approval_rules::ruled_action_of(request_view(request));
    let context = crate::approval_rules::RuleContext {
        teammate_id,
        folder: request.cwd.clone(),
    };
    Ok(
        match crate::approval_rules::decide_by_rules(&action, &rules, &context) {
            crate::approval_rules::RuleVerdict::Allow { rule } => {
                let _ = crate::approval_rule_store::used(&store, &rule.rule_id);
                Some(Answer::Decision {
                    approval_id: request.approval_id.clone(),
                    decision: Decision::ApproveOnce,
                    reason: None,
                })
            }
            crate::approval_rules::RuleVerdict::Deny { rule } => {
                let _ = crate::approval_rule_store::used(&store, &rule.rule_id);
                Some(Answer::Decision {
                    approval_id: request.approval_id.clone(),
                    decision: Decision::Deny,
                    reason: Some(crate::approval_rules::rule_sentence(&rule, None)),
                })
            }
            crate::approval_rules::RuleVerdict::Ask { .. } => None,
        },
    )
}

/// Whether “Yes, and don't ask again” may save a rule for this card.
pub fn can_save_allow_rule(card: &Request) -> bool {
    if asks_every_time(card) {
        return false;
    }
    let action = crate::approval_rules::ruled_action_of(request_view(card));
    let context = crate::approval_rules::RuleContext {
        teammate_id: crate::native::approvals_channel().teammate_for(&card.mission_id),
        folder: card.cwd.clone(),
    };
    crate::approval_rules::rule_candidate_of(&action, &context).is_some()
}

/// Save an allow or deny rule from the card, then answer once.
pub fn save_rule_and_decide(
    card: &Request,
    effect: crate::approval_rules::RuleEffect,
) -> Result<(), String> {
    let channel = crate::native::approvals_channel();
    save_pending_rule(&channel, card, effect)
}

fn save_pending_rule(
    channel: &Channel,
    card: &Request,
    effect: crate::approval_rules::RuleEffect,
) -> Result<(), String> {
    let guard = crate::invariants::lock(&channel.inner);
    let card = &guard
        .pending
        .get(&card.approval_id)
        .ok_or("That approval is no longer waiting.")?
        .request;
    let teammate_id = guard
        .teammates
        .get(&card.mission_id)
        .cloned()
        .ok_or("This approval has no teammate, so no rule was saved.")?;
    let store = crate::native::store().ok_or("Native data folder could not be determined.")?;
    let action = crate::approval_rules::ruled_action_of(request_view(card));
    let context = crate::approval_rules::RuleContext {
        teammate_id: Some(teammate_id),
        folder: card.cwd.clone(),
    };
    if let Some(reach_note) = crate::approval_rules::rule_refusal_for(request_view(card)) {
        return Err(reach_note);
    }
    let draft = match effect {
        crate::approval_rules::RuleEffect::Allow => {
            crate::approval_rules::rule_candidate_of(&action, &context).ok_or_else(|| {
                "This one cannot be saved as a rule exactly, so it was not saved. Answer the card as usual.".to_owned()
            })?
        }
        crate::approval_rules::RuleEffect::Deny => {
            let mut draft = crate::approval_rules::rule_candidate_of(&action, &context).ok_or_else(|| {
                "This one cannot be saved as a rule exactly, so it was not saved. Answer the card as usual.".to_owned()
            })?;
            draft.effect = crate::approval_rules::RuleEffect::Deny;
            draft
        }
    };
    crate::approval_rule_store::add(&store, draft)?;
    Ok(())
}

pub fn card_note(card: &Request) -> String {
    if asks_every_time(card) {
        return "Nothing has happened yet. A command that reaches beyond this run is asked about every time.".into();
    }
    let mut note = card
        .always_covers
        .as_ref()
        .map(|covers| {
            format!("Nothing has happened yet. “Always” allows {covers}, until this run ends.")
        })
        .unwrap_or_else(|| "Nothing has happened yet. “Always” lasts until this run ends.".into());
    if card.always_key.is_some() {
        note.push_str(" Your saved rules still come first.");
    }
    note
}
pub fn effect_lines(card: &Request) -> Vec<(&'static str, String)> {
    let network = crate::invariants::regex(r"(?i)(^|[\s|&;(<>])(curl|wget|ssh|scp|sftp|rsync|nc|netcat|telnet|ftp|npm|pnpm|yarn|pip|pip3|cargo|gem|go|brew|apt|docker|kubectl|aws|gcloud|az|gh|git)(\s|$)|https?://").is_match(&card.detail);
    if card.kind == Kind::Command
        && !network
        && card.data_sent_says.is_none()
        && card.reversible_says.is_none()
    {
        return vec![("Effects", "Unknown. Locust cannot tell what a command sends or undo what it does; it can do anything the sandbox allows.".into())];
    }
    let data = card.data_sent_says.clone().unwrap_or_else(|| if card.kind == Kind::FileChange { "Nothing. The change is written to this machine and sent nowhere.".into() } else if card.kind == Kind::Command { "This command can reach the network. Locust cannot see what it would send — read it above.".into() } else { "The input above, to the service the connector reaches. It acts there, not on this machine.".into() });
    let undo = card.reversible_says.clone().unwrap_or_else(|| if card.kind == Kind::FileChange { "Yes for tracked files, if the workspace is under version control.".into() } else if card.kind == Kind::Command { "Unknown — a command can do anything the workspace sandbox allows.".into() } else { "Unknown — a connector acts on the service it reaches, and Locust cannot undo what happens there.".into() });
    vec![("Data sent", data), ("Reversible", undo)]
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
mod question_tests {
    //! Electron's main/question-answers.test.ts.
    use super::*;

    #[test]
    fn reads_the_array_with_ids_options_and_flags() {
        let params = json!({"questions": [{"id": "database", "header": "Storage", "question": "Which database should the fixture use?", "isOther": true, "isSecret": false,
            "options": [{"label": "SQLite", "description": "No server to run"}, {"label": "Postgres", "description": null}]}]});
        let (kind, summary, detail, _) = describe_codex("item/tool/requestUserInput", &params).unwrap();
        assert_eq!((kind, summary.as_str(), detail.as_str()), (Kind::Question, "Answer a question", "Which database should the fixture use?"));
        let q = &questions_of(&params)[0];
        assert_eq!((q.id.as_str(), q.header.as_deref(), q.is_other), ("database", Some("Storage"), true));
        assert_eq!(q.options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>(), vec!["SQLite", "Postgres"]);
        assert_eq!(q.options[1].description, None);
    }

    #[test]
    fn says_how_many_when_a_request_carries_several_and_still_reads_a_singular_question() {
        let several = json!({"questions": [{"id": "a", "question": "First?", "options": []}, {"id": "b", "question": "Second?", "options": []}]});
        assert_eq!(describe_codex("item/tool/requestUserInput", &several).unwrap().1, "Answer 2 questions");
        assert_eq!(questions_of(&several).len(), 2);
        assert_eq!(describe_codex("item/tool/requestUserInput", &json!({"question": "Ready?"})).unwrap().2, "Ready?");
    }

    #[test]
    fn drops_a_question_with_no_id_and_an_option_with_no_label_and_survives_malformed_requests() {
        let ids = questions_of(&json!({"questions": [{"question": "no id here", "options": []}, {"id": "good", "question": "this one is answerable", "options": []}]}));
        assert_eq!(ids.iter().map(|q| q.id.as_str()).collect::<Vec<_>>(), vec!["good"]);
        let labels = questions_of(&json!({"questions": [{"id": "q", "question": "Pick", "options": [{"description": "nameless"}, {"label": "Real"}]}]}));
        assert_eq!(labels[0].options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>(), vec!["Real"]);
        assert!(questions_of(&json!({})).is_empty());
        assert!(questions_of(&json!({"questions": "not an array"})).is_empty());
        assert!(questions_of(&json!({"questions": [null, 7, "x"]})).is_empty());
    }

    #[test]
    fn sends_answers_keyed_by_question_id_an_option_by_its_label_and_an_unanswered_one_present_and_empty() {
        let answer = |answers: Vec<(&str, Vec<&str>)>| Answer::Answers {
            approval_id: "ap_1".into(),
            answers: answers.into_iter().map(|(id, given)| (id.to_owned(), given.into_iter().map(str::to_owned).collect())).collect(),
        };
        assert_eq!(
            protocol_answer_for(&answer(vec![("database", vec!["SQLite"]), ("notes", vec!["Keep the existing migration names."])])),
            json!({"answers": {"database": {"answers": ["SQLite"]}, "notes": {"answers": ["Keep the existing migration names."]}}})
        );
        let sent = protocol_answer_for(&answer(vec![("q", vec!["Postgres"])]));
        assert_eq!(sent["answers"]["q"]["answers"], json!(["Postgres"]));
        assert_eq!(protocol_answer_for(&answer(vec![("q", vec![])])), json!({"answers": {"q": {"answers": []}}}));
    }

    #[test]
    fn still_sends_a_decision_for_an_authorization() {
        let decision = |d| Answer::Decision { approval_id: "ap_1".into(), decision: d, reason: None };
        assert_eq!(protocol_answer_for(&decision(Decision::ApproveOnce)), json!({"decision": "accept"}));
        assert_eq!(protocol_answer_for(&decision(Decision::ApproveAlways)), json!({"decision": "acceptForSession"}));
        assert_eq!(protocol_answer_for(&decision(Decision::Deny)), json!({"decision": "reject"}));
    }

    #[test]
    fn questions_cross_into_the_engine_process_whole() {
        let params = json!({"questions": [{"id": "d", "header": "H", "question": "Q?", "isOther": true, "isSecret": true, "options": [{"label": "A", "description": "a"}]}]});
        let questions = questions_of(&params);
        assert_eq!(questions_of(&json!({"questions": questions_json(&questions)})), questions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_deny_and_guard_precede_session_always() {
        let root = std::env::temp_dir().join(format!(
            "channel-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
        let _hook = crate::native::hooks::install(store.clone(), None);
        let ch = Channel::new();
        ch.register_teammate("m", "tm_one");
        crate::invariants::lock(&ch.inner)
            .always
            .insert(("m".into(), "claude:Bash".into()), ());
        crate::approval_rule_store::add(
            &store,
            crate::approval_rules::ApprovalRuleDraft {
                effect: crate::approval_rules::RuleEffect::Deny,
                kind: crate::approval_rules::RuleKind::Command,
                pattern: "npm test".into(),
                teammate_id: Some("tm_one".into()),
                folder: None,
            },
        )
        .unwrap();
        let result = ch
            .ask_claude(
                "r",
                "m",
                "a",
                "Bash",
                &json!({"command":"npm test"}),
                "/project",
                &|| false,
            )
            .unwrap();
        assert_eq!(result["behavior"], "deny");
        assert_eq!(ch.take_recorded()[0].by, "saved-rule");
        let result = ch
            .ask_claude(
                "r",
                "m",
                "b",
                "Bash",
                &json!({"command":"taskkill /IM chrome.exe"}),
                "/project",
                &|| false,
            )
            .unwrap();
        assert_eq!(result["behavior"], "deny");
        assert_eq!(ch.take_recorded()[0].by, "command-guard");
        crate::invariants::lock(&ch.inner).always.clear();
        let worker = ch.clone();
        let handle = std::thread::spawn(move || {
            worker.ask_claude(
                "r",
                "m",
                "c",
                "Bash",
                &json!({"command":"printf hi"}),
                "/project",
                &|| false,
            )
        });
        let end = Instant::now() + Duration::from_secs(5);
        while ch.waiting().is_empty() {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(5));
        }
        let mut card = ch.waiting()[0].clone();
        card.detail = "a stale window's different command".into();
        save_pending_rule(&ch, &card, crate::approval_rules::RuleEffect::Allow).unwrap();
        let rules = crate::approval_rule_store::list(&store).unwrap();
        let saved = rules.iter().find(|r| r.pattern == "printf hi").unwrap();
        assert_eq!(saved.teammate_id.as_deref(), Some("tm_one"));
        assert_eq!(saved.folder.as_deref(), Some("/project"));
        ch.release("m");
        assert_eq!(handle.join().unwrap().unwrap()["behavior"], "deny");
        assert!(save_pending_rule(&ch, &card, crate::approval_rules::RuleEffect::Allow).is_err());
        assert_eq!(ch.teammate_for("m"), None);
        store
            .save_json(
                "approval-rules.json",
                &json!({"schemaVersion":99,"rules":[]}),
            )
            .unwrap();
        crate::invariants::lock(&ch.inner)
            .always
            .insert(("n".into(), "claude:Bash".into()), ());
        let worker = ch.clone();
        let handle = std::thread::spawn(move || {
            worker.ask_claude(
                "r",
                "n",
                "d",
                "Bash",
                &json!({"command":"printf hi"}),
                "/project",
                &|| false,
            )
        });
        let end = Instant::now() + Duration::from_secs(5);
        while ch.waiting().is_empty() {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(5));
        }
        ch.release("n");
        assert_eq!(handle.join().unwrap().unwrap()["behavior"], "deny");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn exact_detail_counts_omitted_text_and_keeps_long_paths() {
        assert_eq!(whole_detail("  a\r\nb\rc  "), "a\nb\nc");
        let text = format!("{}x", "a".repeat(20_000));
        assert!(whole_detail(&text).ends_with(
            "[1 more character not shown here. Deny if you cannot see enough to decide.]"
        ));
        let path = format!("/project/{}outside.txt", "x".repeat(650));
        assert_eq!(
            describe_claude_tool("Write", &json!({"file_path":path}), "/project").2,
            path.strip_prefix("/project/").unwrap()
        );
    }

    #[test]
    fn records_are_scoped_and_card_words_match_electron() {
        let ch = Channel::new();
        let mut request = Request {
            approval_id: "a".into(),
            mission_id: "one".into(),

            runtime: "codex".into(),
            kind: Kind::Command,
            summary: "same".into(),
            detail: "same".into(),
            cwd: None,
            requested_at: now_iso(),
            always_covers: None,
            always_key: None,
            data_sent_says: None,
            reversible_says: None,

            claude_input: None,

            questions: Vec::new(),
        };
        let answer = Answer::Decision {
            approval_id: "a".into(),
            decision: Decision::Deny,
            reason: Some("stop".into()),
        };
        let first = record_of(&request, &answer);
        assert_eq!(first.asked, "same");
        assert_eq!(first.to_json()["by"], "card");
        assert!(first.to_json().get("mission_id").is_none());
        request.mission_id = "two".into();
        ch.inner
            .lock()
            .unwrap()
            .recorded
            .extend([first, record_of(&request, &answer)]);
        assert_eq!(ch.take_recorded_for("one").len(), 1);
        assert!(ch.take_recorded_for("one").is_empty());
        assert_eq!(ch.take_recorded_for("two").len(), 1);
        assert_eq!(
            card_note(&request),
            "Nothing has happened yet. “Always” lasts until this run ends."
        );
        assert_eq!(effect_lines(&request)[0].0, "Effects");
        request.detail = "touch hello.txt".into();
        assert_eq!(
            plain_action(&request).as_deref(),
            Some("Makes hello.txt if it is not there yet.")
        );
        request.detail = "curl https://example.com".into();
        assert_eq!(effect_lines(&request)[0].0, "Data sent");
        request.detail = "pkill python".into();
        assert!(asks_every_time(&request));
        assert_eq!(
            card_note(&request),
            "Nothing has happened yet. A command that reaches beyond this run is asked about every time."
        );
        request.kind = Kind::FileChange;
        assert_eq!(
            effect_lines(&request)[0].1,
            "Nothing. The change is written to this machine and sent nowhere."
        );
    }

    #[test]
    fn ended_or_cancelled_requests_never_raise_a_card() {
        let ch = Channel::new();
        ch.release("ended");
        assert!(
            ch.ask_claude("r", "ended", "a", "Write", &json!({}), "/tmp", &|| false)
                .is_err()
        );
        assert!(
            ch.ask_claude("r", "cancelled", "b", "Write", &json!({}), "/tmp", &|| true)
                .is_err()
        );
        assert!(ch.waiting().is_empty());
    }

    #[test]
    fn claude_always_is_only_for_the_chosen_tool_in_this_turn() {
        let ch = Channel::new();
        let worker = ch.clone();
        let handle = std::thread::spawn(move || {
            worker.ask_claude("r", "m", "a", "Write", &json!({}), "/tmp", &|| false)
        });
        let end = Instant::now() + Duration::from_secs(5);
        while ch.waiting().is_empty() {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(5));
        }
        ch.decide(Answer::Decision {
            approval_id: ch.waiting()[0].approval_id.clone(),
            decision: Decision::ApproveAlways,
            reason: None,
        })
        .unwrap();
        assert_eq!(handle.join().unwrap().unwrap()["behavior"], "allow");
        assert_eq!(
            ch.ask_claude("r", "m", "b", "Write", &json!({}), "/tmp", &|| false)
                .unwrap()["behavior"],
            "allow"
        );
        assert_eq!(ch.take_recorded_for("m")[1].by, "earlier-always");
        let worker = ch.clone();
        let handle = std::thread::spawn(move || {
            worker.ask_claude("r", "m", "c", "Bash", &json!({}), "/tmp", &|| false)
        });
        while ch.waiting().is_empty() {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(5));
        }
        ch.release("m");
        assert_eq!(handle.join().unwrap().unwrap()["behavior"], "deny");
        assert!(crate::invariants::lock(&ch.inner).always.is_empty());
    }

    #[test]
    fn ledger_text_bounds_match_typescript_utf16_lengths() {
        assert_eq!(
            utf16_bounded(&"😀".repeat(1500), 2000)
                .encode_utf16()
                .count(),
            2000
        );
        assert_eq!(utf16_bounded("a😀", 2), "a");
    }

    #[test]
    fn sibling_and_parent_paths_are_outside_the_project() {
        for path in ["/project-other/a", "../a", "./../a", "/project/../a"] {
            let (_, summary, _, _, undo) =
                describe_claude_tool("Write", &json!({"file_path":path}), "/project");
            assert!(summary.contains("outside"), "{path}");
            assert!(undo.unwrap().starts_with("Not from here"));
        }
        assert_eq!(
            describe_claude_tool("Write", &json!({"file_path":"/project/a"}), "/project").2,
            "a"
        );
    }

    #[test]
    fn codex_policy_approve_each_asks() {
        let p = codex_app_server_policy(Mode::ApproveEach, None);
        assert_eq!(p.approval_policy, "untrusted");
        assert_eq!(p.sandbox, "workspace-write");
        // As commands.ts: a named sandbox is kept, except that full access is never paired with asking.
        assert_eq!(codex_app_server_policy(Mode::ApproveEach, Some("read-only")).sandbox, "read-only");
        assert_eq!(codex_app_server_policy(Mode::ApproveEach, Some("full-access")).sandbox, "workspace-write");
    }

    // codexAppServerPolicy over the sandbox each mode earns (codex-mission.ts).
    #[test]
    fn codex_policy_follows_each_modes_sandbox() {
        for (mode, sandbox) in [
            (Mode::Ask, "read-only"),
            (Mode::Plan, "read-only"),
            (Mode::AcceptEdits, "workspace-write"),
            (Mode::Auto, "danger-full-access"),
        ] {
            let p = codex_app_server_policy(mode, None);
            assert_eq!((p.sandbox, p.approval_policy), (sandbox, "never"), "{mode:?}");
        }
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
        assert!(
            err.contains("no longer waiting") || err.contains("turn ended"),
            "{err}"
        );
        let denied = handle.join().unwrap().unwrap();
        assert_eq!(denied["behavior"], "deny");
    }
}

//! Approvals: what a runtime asks mid-run, and what the person answers.
//!
//! Ports `apps/desktop/src/main/approval-channel.ts` (describe / decide / protocol
//! answers), `permission-host.ts:builtInOrConnector` for Claude `can_use_tool`, and
//! `commands.ts:codexAppServerPolicy` for mode → sandbox / approvalPolicy.
//! Saved approval rules are consulted before session Always and before a card is raised.

use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender},
    },
    time::{Duration, Instant},
};

/// raised-approvals.ts MAX_RAISED_APPROVALS: the cards still waiting that Locust holds (0.599). An answered card leaves at
/// once, so the cap counts only cards still waiting. Past it Rust refuses the newest, where Electron drops the oldest
/// card's record: here the record and the runtime's waiting answer are one, and dropping it would leave a run waiting
/// on an answer nobody can give.
pub const MAX_PENDING: usize = 64;
const MAX_DETAIL: usize = 20_000;
/// Locust's own reason when a run ends before a card is answered: never told to a runtime as the person's words.
const TURN_ENDED: &str = "The turn ended before this was answered.";

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
    pub patch: Option<crate::approval_patch::ToolPatch>,

    /// Wire id the runtime needs to answer (Codex JSON-RPC id, or Claude request_id).

    /// Claude `can_use_tool` input, kept so allow can return updatedInput.
    pub claude_input: Option<Value>,
    /// A question the runtime lets the person skip (Antigravity's own card has Skip): every answer left empty.
    pub skippable: bool,
    /// Where a question has to be answered when Locust cannot answer it (Antigravity, when its waiting step was not
    /// found): the card shows it and says where, with no buttons.
    pub answer_in: Option<String>,
    /// What a question card asks, each answered by id (empty for an authorization).
    pub questions: Vec<Question>,
}

static TOOL_USES: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());

fn remember_tool_use(approval_id: &str, tool_use_id: &str) {
    let mut kept = crate::invariants::lock(&TOOL_USES);
    kept.push((approval_id.to_owned(), tool_use_id.to_owned()));
    // Bounded: only cards still being answered need it.
    let extra = kept.len().saturating_sub(64);
    kept.drain(..extra);
}

static ASKED_AGAIN: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());

/// who-decides.ts decide, step 4: an Always given earlier in this run covers this card's key, but the command reaches
/// beyond its run, so the card asks anyway. The host says why, and the card shows it in place of the general line.
/// None for any other card.
pub fn asked_again_why(card: &Request, remembered: bool) -> Option<String> {
    if !remembered || card.kind == Kind::Question {
        return None;
    }
    let reach = command_reach_of(card)?;
    Some(format!("Asked again, though you chose Always earlier in this run: this command {}, and such a command is asked about every time.", reach.short))
}

fn remember_asked_again(approval_id: &str, why: String) {
    let mut kept = crate::invariants::lock(&ASKED_AGAIN);
    kept.push((approval_id.to_owned(), why));
    // Bounded: only cards still being answered need it.
    let extra = kept.len().saturating_sub(64);
    kept.drain(..extra);
}

/// Why a card is asked although Always was given earlier in its run (`askedAgain`), when it is.
pub fn asked_again_of(approval_id: &str) -> Option<String> {
    crate::invariants::lock(&ASKED_AGAIN).iter().rev().find(|(a, _)| a == approval_id).map(|(_, w)| w.clone())
}

/// The tool call a card was raised for, when the runtime said (Claude Code's `tool_use_id`).
pub fn tool_use_of(approval_id: &str) -> Option<String> {
    crate::invariants::lock(&TOOL_USES).iter().rev().find(|(a, _)| a == approval_id).map(|(_, t)| t.clone())
}

/// Who asked, when a helper raised the card (`helperAskedBy`): "the Explore helper of Wren", "a helper of this
/// teammate" when its type is not known; None when the teammate asked itself or the call is not known.
pub fn helper_asked_by(tool_use_id: Option<&str>, items: &[crate::ledger::Item], teammate: Option<&str>) -> Option<String> {
    let id = tool_use_id?;
    let helper = items.iter().find_map(|item| match item {
        crate::ledger::Item::Tool { detail, .. } if detail.helper && detail.children.iter().any(|c| matches!(c, crate::ledger::Item::Tool { detail, .. } if detail.id == id)) => Some(detail),
        _ => None,
    })?;
    let whose = teammate.map(str::trim).filter(|t| !t.is_empty()).unwrap_or("this teammate");
    Some(if helper.helper_type.is_empty() { format!("a helper of {whose}") } else { format!("the {} helper of {whose}", helper.helper_type) })
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

/// approval-detail.ts `wholeInput`: the input as `JSON.stringify(input, null, 2)` writes it, nothing for none.
fn whole_input(input: &Value) -> String {
    if input.is_null() {
        return String::new();
    }
    serde_json::to_string_pretty(input).map(|text| whole_detail(&text)).unwrap_or_default()
}

/// approval-detail.ts `programAsRun` (0.837): Codex on Windows sends a command with its program's path escaped, every
/// backslash doubled. When every backslash in the leading quoted path is half of a pair, each pair is read as one;
/// anything else, and the rest of the command, is shown exactly as sent.
pub fn program_as_run(command: &str) -> String {
    let Some(rest) = command.strip_prefix('"') else { return command.to_owned() };
    let Some(end) = rest.find('"') else { return command.to_owned() };
    let path = &rest[..end];
    if !path.contains("\\\\") || path.replace("\\\\", "").contains('\\') {
        return command.to_owned();
    }
    format!("\"{}\"{}", path.replace("\\\\", "\\"), &rest[end + 1..])
}

pub(crate) fn whole_detail(value: &str) -> String {
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
            let command = program_as_run(&whole_detail(params["command"].as_str().unwrap_or("")));
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
    // permission-host.ts `text`: every run of whitespace one space (a search typed with two spaces or a line break
    // reads as its words), trimmed, at most 600.
    let text = |v: &Value| -> String {
        v.as_str()
            .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(600).collect())
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
        let relative = if !file.is_empty() && !cwd.is_empty() {
            crate::approval_rules::inside_folder(&file, cwd)
        } else {
            None
        };
        let outside = !file.is_empty() && !cwd.is_empty() && relative.is_none();
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
            relative.unwrap_or(file),
            None,
            None,
        );
    }
    if tool_name.starts_with("mcp__") {
        return (
            Kind::Connector,
            connector_name(tool_name),
            whole_input(input),
            None,
            None,
        );
    }
    (
        Kind::Command,
        format!("Use {tool_name}"),
        whole_input(input),
        None,
        None,
    )
}

pub fn always_covers(tool: &str) -> String {
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

/// approval-channel.ts codexAlwaysFor: Codex asks again after an Always, so Locust remembers the exact action, not
/// the runtime. A command by its whole text; a file change by its paths (moves included), sorted, as JSON so a path
/// boundary is never ambiguous. Anything else, or a change naming no path, has no key: Always is then not offered a
/// memory, and the next request asks.
pub fn codex_always_for(kind: &Kind, params: &Value, changes: Option<&[crate::approval_patch::FileChangeRecord]>) -> Option<(String, String)> {
    if *kind == Kind::Command {
        let command = params["command"].as_str().filter(|c| !c.is_empty())?;
        return Some((format!("codex:command:{command}"), "this same command again".into()));
    }
    if *kind != Kind::FileChange {
        return None;
    }
    let mut paths: Vec<String> = Vec::new();
    if let Some(records) = changes {
        for change in records {
            if change.path.is_empty() {
                return None;
            }
            paths.push(change.path.clone());
            if let Some(moved) = change.move_path.as_ref().filter(|m| !m.is_empty()) {
                paths.push(moved.clone());
            }
        }
    } else {
        for entry in params["changes"].as_array().map(Vec::as_slice).unwrap_or_default() {
            let path = entry.get("path").and_then(Value::as_str).filter(|p| !p.is_empty())?;
            paths.push(path.to_owned());
            if let Some(moved) = entry.get("movePath").and_then(Value::as_str).filter(|m| !m.is_empty()) {
                paths.push(moved.to_owned());
            }
        }
    }
    if paths.is_empty() {
        return None;
    }
    paths.sort();
    Some((format!("codex:files:{}", serde_json::to_string(&paths).unwrap_or_default()), "the same change to these files again".into()))
}

/// `approval-channel.ts:protocolDecisionFor`: Codex's v2 decision (`CommandExecutionApprovalDecision`,
/// `FileChangeApprovalDecision`) for each of the person's answers. Every value is checked against Codex 0.172.0's own
/// schemas (`codex_decision_schema_tests.rs`): Deny was `"reject"`, which no Codex schema has.
pub fn protocol_decision_for(decision: &Decision) -> &'static str {
    match decision {
        Decision::ApproveOnce => "accept",
        Decision::ApproveAlways => "acceptForSession",
        // `decline`, not `cancel`: the turn carries on without the action ("don't do that, carry on"), so a reason the
        // person gave can still reach it as its next input (`take_denied_saying` → `turn/steer`). `cancel` would also
        // interrupt the turn, and the reason would have nothing to be steered into.
        Decision::Deny => "decline",
    }
}

/// Locust's own refusal of a Codex v2 approval it does not put on a card (another thread's, a mode that does not ask,
/// a card that could not be shown): the person's Deny, so it is the same value on the wire.
pub fn codex_refusal() -> Value {
    json!({"decision": protocol_decision_for(&Decision::Deny)})
}

/// What a legacy refusal tells the model (`codex-app-server-run.ts` `CODEX_LEGACY_REFUSAL_SAYS`).
pub const CODEX_LEGACY_REFUSAL_SAYS: &str = "Locust answers only Codex's current approval requests, so it did not allow this one.";

/// Locust's refusal of a legacy Codex approval (`execCommandApproval`, `applyPatchApproval`), never put on a card. Their
/// `ReviewDecision` has no bare `"denied"`: the denied variant is an object carrying its rejection, and the agent tries
/// something else. `"abort"` would stop the turn until the person's next message, so it matches `cancel`, not Deny.
pub fn codex_legacy_refusal() -> Value {
    json!({"decision": {"denied": {"rejection": CODEX_LEGACY_REFUSAL_SAYS}}})
}

/// A ToolRequestUserInputResponse with no answers supplied by the person.
pub(super) fn codex_unanswered_questions(questions: &[Question]) -> Value {
    let answers: serde_json::Map<_, _> = questions.iter()
        .map(|question| (question.id.clone(), json!({"answers": []})))
        .collect();
    json!({"answers": answers})
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
        // Exactly what Codex's app-server expects: no field it does not know. A denial's reason reaches the run another
        // way (`take_denied_saying`), or rides OpenCode's reject (`ask_codex_with_changes`).
        Answer::Decision { decision, .. } => json!({"decision": protocol_decision_for(decision)}),
    }
}

/// The most of a denial's reason that is passed on: a sentence or two, not a document (approval-channel.ts).
pub const MAX_DENY_REASON: usize = 500;

/// What a runtime is told alongside a denial the person explained (approval-channel.ts `deniedSaying`, 0.374).
pub fn denied_saying(reason: &str) -> String {
    format!("The person declined this, and said: {reason}")
}

/// A denial's reason as it is passed on: one line, bounded, and nothing when there is none.
fn deny_reason(raw: Option<&str>) -> Option<String> {
    let line = raw.unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ");
    let bounded = utf16_bounded(&line, MAX_DENY_REASON);
    (!bounded.is_empty()).then_some(bounded)
}

/// The window's answer, as the channel takes it (approval-channel.ts `approvalAnswerFrom`). Anything but a recognized
/// answer is refused or read as a denial: a malformed message must never be able to approve an action.
pub fn approval_answer_from(raw: &Value) -> Option<Answer> {
    let approval_id = raw["approvalId"].as_str()?.to_owned();
    if let Some(answers) = raw.get("answers") {
        let mut clean = Vec::new();
        for (question_id, given) in answers.as_object()? {
            // Bounded: an answer is words a person typed, not a file.
            let given = given.as_array()?.iter().map(|g| g.as_str().map(str::to_owned)).collect::<Option<Vec<_>>>()?;
            clean.push((question_id.clone(), given.into_iter().take(64).map(|g| utf16_bounded(&g, 4_000)).collect()));
        }
        return Some(Answer::Answers { approval_id, answers: clean });
    }
    let decision = match raw["decision"].as_str() {
        Some("approve-once") => return Some(Answer::Decision { approval_id, decision: Decision::ApproveOnce, reason: None }),
        Some("approve-always") => return Some(Answer::Decision { approval_id, decision: Decision::ApproveAlways, reason: None }),
        _ => Decision::Deny,
    };
    // A denial may say why (0.374): the person's words, bounded, and nothing when there are none.
    Some(Answer::Decision { approval_id, decision, reason: deny_reason(raw["reason"].as_str()) })
}

/// A denial the person explained, on a runtime whose reply cannot carry it (Codex's app-server): the run is shown the
/// reason as its next input instead (approval-channel.ts `onDeniedSaying`). OpenCode's reply carries it and never comes
/// here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeniedSaying {
    pub run_id: String,
    pub mission_id: String,
    pub reason: String,
}

/// Claude control_response body for one answer.
/// `persons` is whether the answer is the person's (a card, or a rule they saved): only then is a reason their words.
pub fn claude_behavior_for(answer: &Answer, input: Option<&Value>, persons: bool) -> Value {
    match answer {
        Answer::Decision {
            decision: Decision::Deny,
            reason,
            ..
        } => {
            // permission-host.ts decide: Claude reads the denial's message as the tool's result, so a reason the
            // person gave rides it (0.374), in approval-channel.ts `deniedSaying`'s words.
            let message = match reason.as_deref().filter(|s| !s.is_empty()) {
                Some(why) if persons => format!("Denied in Locust. The person declined this, and said: {why}"),
                // Locust's own reason (the command guard's, a turn that ended first) goes as it is.
                Some(why) => why.to_owned(),
                None => "Denied in Locust.".to_owned(),
            };
            json!({"behavior":"deny","message": message})
        }
        Answer::Decision { .. } => {
            json!({"behavior":"allow","updatedInput": input.cloned().unwrap_or(json!({}))})
        }
        // A Claude card is an authorization: answers never reach one (the channel checks the pairing first).
        Answer::Answers { .. } => json!({"behavior":"deny","message":"Locust did not send that answer: it belongs to a question, and the runtime was waiting on a permission."}),
    }
}

/// approval-channel.ts `acpPermissionRequest` over acp-run.ts `acpPermissionRequestOf`: what an ACP agent asked, as
/// the Codex app server's method and params, with what an Always on it would remember.
pub fn acp_permission_request(params: &Value, cwd: &str) -> (String, Value) {
    let text = |v: &Value| v.as_str().filter(|s| !s.is_empty()).map(str::to_owned);
    let call = &params["toolCall"];
    let input = &call["rawInput"];
    let (title, kind, command) = (text(&call["title"]), text(&call["kind"]), text(&input["command"]));
    let diffs: Vec<&Value> = call["content"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["type"] == "diff" && d["path"].is_string() && d["newText"].is_string())
        .collect();
    let diff: String = diffs
        .iter()
        .filter_map(|d| crate::runtime::acp_events::unified_diff_text(d["path"].as_str()?, d["oldText"].as_str(), d["newText"].as_str()?))
        .collect();
    let mut paths: Vec<String> = call["locations"].as_array().into_iter().flatten().filter_map(|l| text(&l["path"])).collect();
    paths.extend(diffs.iter().filter_map(|d| text(&d["path"])));
    paths.extend(["path", "file_path", "filePath"].iter().filter_map(|field| text(&input[*field])));
    let mut seen = std::collections::HashSet::new();
    paths.retain(|p| seen.insert(p.clone()));
    let (call_kind, call_title) = (kind.as_deref().unwrap_or(""), title.as_deref().unwrap_or(""));
    // A shell's locations path is the folder, not the command an Always would remember.
    let shell = call_kind == "execute"
        || ((call_kind.is_empty() || call_kind == "other")
            && matches!(call_title.trim().to_lowercase().as_str(), "bash" | "shell" | "powershell" | "pwsh" | "cmd" | "execute"));
    let mut base = json!({"cwd": cwd});
    if let Some(id) = text(&call["toolCallId"]) {
        base["itemId"] = json!(id);
    }
    let files = paths.iter().map(|p| crate::approval_patch::relative_to_folder(p, Some(cwd))).collect::<Vec<_>>().join(", ");
    let (method, mut request) = if shell {
        base["command"] = json!(command.clone().unwrap_or_default());
        ("item/commandExecution/requestApproval", base)
    } else if matches!(call_kind, "edit" | "delete" | "move") {
        base["summary"] = json!(if files.is_empty() { call_title.to_owned() } else { files.clone() });
        if !diff.is_empty() {
            base["unifiedDiff"] = json!(diff);
        }
        ("item/fileChange/requestApproval", base)
    } else {
        let what = title.clone().unwrap_or_else(|| format!("Use {}", kind.as_deref().unwrap_or("a tool")));
        let named = command.clone().or_else(|| (!files.is_empty()).then(|| files.clone()));
        base["command"] = json!(match named {
            Some(target) => format!("{what}: {target}"),
            None => what,
        });
        ("item/commandExecution/requestApproval", base)
    };
    let target = command.clone().or_else(|| (!shell && !paths.is_empty()).then(|| paths.join("\n")));
    if let Some(target) = target {
        request["locustAlwaysKey"] = json!(format!("acp:{call_kind}:{target}"));
        request["locustCard"] = json!({"alwaysCovers": if command.is_some() { "this same command again" } else { "the same change to these files again" }});
    }
    (method.to_owned(), request)
}

/// approval-channel.ts `acpAnswerFor`: the channel's answer as the kind of ACP option to choose; anything unclear is a
/// refusal.
pub fn acp_answer_for(result: &Value) -> &'static str {
    match result["decision"].as_str() {
        Some("accept") => "allow_once",
        Some("acceptForSession") => "allow_always",
        _ => "reject_once",
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
    /// The run that asked, and the item it asked about when it named one: a denial is remembered under them.
    run_id: String,
    item_id: Option<String>,
    /// A denial's reason goes to the run as its next input (Codex, ACP), not in the reply.
    steers_reason: bool,
}

/// Bounded per-run Always keys (`createRunAlways` in Electron `who-decides.ts`): least-recently-used runs are evicted
/// past `max_runs`, and keys past `max_keys` in one run are not kept (so that run asks again).
pub struct RunAlways {
    max_runs: usize,
    max_keys: usize,
    order: Vec<String>,
    runs: HashMap<String, HashSet<String>>,
}

impl RunAlways {
    pub fn new(max_runs: usize, max_keys: usize) -> Self {
        Self { max_runs, max_keys, order: Vec::new(), runs: HashMap::new() }
    }

    pub fn has(&self, run_id: &str, key: &str) -> bool {
        self.runs.get(run_id).is_some_and(|keys| keys.contains(key))
    }

    pub fn add(&mut self, run_id: &str, key: &str) {
        if self.runs.get(run_id).is_some_and(|keys| keys.len() >= self.max_keys && !keys.contains(key)) {
            return;
        }
        let keys = self.runs.entry(run_id.to_owned()).or_default();
        keys.insert(key.to_owned());
        self.order.retain(|r| r != run_id);
        self.order.push(run_id.to_owned());
        while self.order.len() > self.max_runs {
            let oldest = self.order.remove(0);
            self.runs.remove(&oldest);
        }
    }

    pub fn remove(&mut self, run_id: &str, key: &str) {
        if let Some(keys) = self.runs.get_mut(run_id) {
            keys.remove(key);
        }
    }

    pub fn release(&mut self, run_id: &str) {
        self.runs.remove(run_id);
        self.order.retain(|r| r != run_id);
    }

    #[cfg(test)]
    pub(crate) fn clear(&mut self) {
        self.runs.clear();
        self.order.clear();
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }
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
    always: RunAlways,
    teammates: HashMap<String, String>,
    /// The items the person declined, by run id, with the mission the run belongs to (approval-channel.ts
    /// `declinedByRun`): kept only while the run lasts.
    declined: HashMap<String, (String, HashSet<String>)>,
    /// Denials the person explained, waiting for their run to be shown them (`take_denied_saying`).
    denied_saying: Vec<DeniedSaying>,
}

impl Channel {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Inner {
                pending: HashMap::new(),
                ended: HashMap::new(),
                recorded: Vec::new(),
                always: RunAlways::new(64, 256),
                teammates: HashMap::new(),
                declined: HashMap::new(),
                denied_saying: Vec::new(),
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
        run_id: &str,
        mission_id: &str,
        method: &str,
        params: &Value,
        wire_id: Value,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Value, String> {
        self.ask_codex_with_changes(run_id, mission_id, "codex", method, params, wire_id, None, cancel)
    }

    /// Raise a Codex/OpenCode-shaped request card, waiting briefly for Codex `item/started` file changes.
    #[allow(clippy::too_many_arguments)]
    pub fn ask_codex_with_changes(
        &self,
        run_id: &str,
        mission_id: &str,
        runtime: &str,
        method: &str,
        params: &Value,
        _wire_id: Value,
        changes_by_item: Option<
            &std::sync::Mutex<
                std::collections::HashMap<String, Vec<crate::approval_patch::FileChangeRecord>>,
            >,
        >,
        cancel: &dyn Fn() -> bool,
    ) -> Result<Value, String> {
        // Refused, and said out loud by the caller: an unattributed denial must not be quiet in a mode whose whole point
        // is that a person decides (approval-channel.ts).
        let Some((kind, summary, detail, cwd)) = describe_codex(method, params) else {
            return Err(format!("Locust denied a request it does not recognise ({method}) because it could not say what would happen. This build is older than the runtime's protocol."));
        };
        let questions = if kind == Kind::Question {
            questions_of(params)
        } else {
            Vec::new()
        };
        let item_id = params["itemId"].as_str();
        let read_changes = || {
            if kind != Kind::FileChange {
                return None;
            }
            let (id, map) = (item_id?, changes_by_item?);
            crate::invariants::lock(map).get(id).cloned()
        };
        let mut changes = read_changes();
        let runtime = if runtime.is_empty() { "codex" } else { runtime };
        let codex = runtime == "codex";
        if codex
            && kind == Kind::FileChange
            && item_id.is_some()
            && changes.is_none()
            && !params["unifiedDiff"].is_string()
        {
            for _ in 0..10 {
                if cancel() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
                changes = read_changes();
                if changes.is_some() {
                    break;
                }
            }
        }
        let said = |key: &str| -> Option<String> {
            params["locustCard"]
                .get(key)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
        };
        let cwd_str = cwd.as_deref().unwrap_or("");
        let offered = params["unifiedDiff"]
            .as_str()
            .and_then(|d| diff_patch_from(d, cwd_str));
        let patch = changes
            .as_deref()
            .and_then(|c| {
                crate::approval_patch::approval_patch_from(
                    Some(c),
                    (!cwd_str.is_empty()).then_some(cwd_str),
                )
            })
            .or_else(|| offered.clone());
        let summary = if let Some(c) = changes.as_ref().filter(|c| !c.is_empty()) {
            format!("Change {} file{}", c.len(), if c.len() == 1 { "" } else { "s" })
        } else {
            said("summary").unwrap_or_else(|| {
                if offered.is_some() {
                    "Change 1 file".into()
                } else {
                    summary
                }
            })
        };
        // Codex's Always is Locust's to keep (codexAlwaysFor): answered to Codex as once, remembered here under the
        // exact action, so a saved rule still decides the next request.
        let codex_always = if codex { codex_always_for(&kind, params, changes.as_deref()) } else { None };
        let always_key = if codex {
            codex_always.as_ref().map(|(key, _)| key.clone())
        } else {
            params["locustAlwaysKey"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|s| s.chars().take(2_000).collect())
        };
        let request = Request {
            approval_id: new_id(),
            mission_id: mission_id.into(),
            runtime: runtime.into(),
            kind,
            summary,
            detail,
            cwd,
            requested_at: now_iso(),
            always_covers: codex_always.map(|(_, covers)| covers).or_else(|| said("alwaysCovers")),
            always_key,
            data_sent_says: said("dataSentSays"),
            reversible_says: said("reversibleSays"),
            patch,
            skippable: false, answer_in: None, claude_input: None,
            questions,
        };
        let remembered_here = codex && request.always_key.is_some();
        let item = item_id.map(str::to_owned);
        let opencode = runtime == "opencode";
        self.ask_by_run(request, run_id, item, !opencode, cancel).map(|(answer, _)| {
            // Never "acceptForSession" to Codex for an action Locust remembers itself: told that, Codex stops asking
            // and a saved rule never sees the next one (approval-channel.ts, codexAlways).
            let answer = match answer {
                Answer::Decision { approval_id, decision: Decision::ApproveAlways, reason } if remembered_here => {
                    Answer::Decision { approval_id, decision: Decision::ApproveOnce, reason }
                }
                other => other,
            };
            // Release and wrong-kind replies use a denial within the channel.
            // On Codex's wire the pending request, not that denial, chooses the result type.
            if codex && method == "item/tool/requestUserInput" && matches!(answer, Answer::Decision { .. }) {
                return codex_unanswered_questions(&questions_of(params));
            }
            let mut result = protocol_answer_for(&answer);
            // OpenCode's reject carries a message the model reads, so a reason rides the denial itself; read by
            // `open_code_reply_for`, it never reaches Codex.
            if opencode
                && let Answer::Decision { decision: Decision::Deny, reason: Some(why), .. } = &answer
                && !why.is_empty()
            {
                result["reason"] = json!(why);
            }
            result
        })
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
            patch: None,

            skippable: false, answer_in: None, claude_input: Some(input.clone()),

            questions: Vec::new(),
        };
        // The tool call this card is about, so the window can say when a helper raised it (helperAskedBy).
        // Only what looks like an id is carried (permission-host.ts: /^[A-Za-z0-9_-]{1,128}$/).
        if _request_id != "permission" && crate::trash::is_safe_id(_request_id) {
            remember_tool_use(&request.approval_id, _request_id);
        }
        let input_kept = request.claude_input.clone();
        self.ask_by(request, cancel)
            .map(|(answer, whose)| claude_behavior_for(&answer, input_kept.as_ref(), whose == "person"))
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
        // approval-channel.ts: an ACP request becomes the Codex-shaped request every other card is made from
        // (acpPermissionRequest), and the answer goes back as the kind of option to choose (acpAnswerFor).
        let (method, request) = acp_permission_request(params, cwd);
        self.ask_codex_with_changes(_run_id, mission_id, runtime, &method, &request, json!(0), None, cancel)
            .map(|result| acp_answer_for(&result) != "reject_once")
    }

    /// Browser actions never inherit a command allow rule or Auto-mode bypass.
    pub fn ask_browser(&self, mission: &str, title: &str, detail: &str, cancel: &dyn Fn() -> bool) -> Result<bool, String> {
        let request = Request { approval_id: new_id(), mission_id: mission.into(), runtime: "browser".into(), kind: Kind::Connector, summary: whole_detail(title), detail: whole_detail(detail), cwd: None, requested_at: now_iso(), always_covers: None, always_key: None, data_sent_says: None, reversible_says: None, patch: None, skippable: false, answer_in: None, claude_input: None, questions: Vec::new() };
        self.ask(request, cancel).map(|a| matches!(a, Answer::Decision { decision: Decision::ApproveOnce | Decision::ApproveAlways, .. }))
    }

    fn ask(&self, request: Request, cancel: &dyn Fn() -> bool) -> Result<Answer, String> {
        self.ask_by(request, cancel).map(|(answer, _)| answer)
    }

    /// The answer, and whose it is: the person's own (`person`, a card or a rule they saved) or Locust's (`locust`: the
    /// command guard, an earlier Always, a turn that ended first).
    fn ask_by(&self, request: Request, cancel: &dyn Fn() -> bool) -> Result<(Answer, &'static str), String> {
        self.ask_by_run(request, "", None, false, cancel)
    }

    /// `ask_by`, for a run that names the item it asks about, so the person's denial of it is remembered.
    fn ask_by_run(
        &self,
        request: Request,
        run_id: &str,
        item_id: Option<String>,
        steers_reason: bool,
        cancel: &dyn Fn() -> bool,
    ) -> Result<(Answer, &'static str), String> {
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
                return Ok((answer, "locust"));
            }
            // Saved rules answer before a card (Electron decideByRules). Unreadable → every card asks.
            let saved =
                if request.runtime == "browser" { Ok(Ruled::Ask(None)) } else { decide_by_saved_rules(&request, guard.teammates.get(&request.mission_id).cloned()) };
            let readable = saved.is_ok();
            // Why the rules still ask, when a person would wonder (who-decides.ts: ruled.why, e.g. COMPOUND_REFUSAL).
            let ruled_why = match &saved { Ok(Ruled::Ask(why)) => why.clone(), _ => None };
            if let Ok(Ruled::Answered(auto)) = saved {
                let mut record = record_of(&request, &auto);
                record.by = "saved-rule".into();
                guard.recorded.push(record);
                return Ok((auto, "person"));
            }
            let remembered = readable
                && request.always_key.as_ref().is_some_and(|key| guard.always.has(&request.mission_id, key));
            if let Some(why) = asked_again_why(&request, remembered).or(ruled_why) {
                remember_asked_again(&id, why);
            }
            if remembered && request.kind != Kind::Question && !asks_every_time(&request) {
                let answer = Answer::Decision {
                    approval_id: id,
                    decision: Decision::ApproveOnce,
                    reason: None,
                };
                let mut record = record_of(&request, &answer);
                record.by = "earlier-always".into();
                guard.recorded.push(record);
                return Ok((answer, "locust"));
            }
            if guard.pending.len() >= MAX_PENDING {
                return Err(format!("Locust denied an action because {MAX_PENDING} approvals are already waiting on you. Answer some of them and ask again."));
            }
            guard.pending.insert(id.clone(), Pending { request, tx, run_id: run_id.to_owned(), item_id, steers_reason });
        }
        let answer = wait_answer(&rx, cancel);
        // If wait ended without decide(), drop the pending entry.
        {
            let mut guard = crate::invariants::lock(&self.inner);
            guard.pending.remove(&id);
        }
        // A turn that ended first is Locust's word, not the person's.
        answer.map(|answer| {
            let ended = matches!(&answer, Answer::Decision { reason: Some(why), .. } if why == TURN_ENDED);
            (answer, if ended { "locust" } else { "person" })
        })
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
                reason: Some(TURN_ENDED.into()),
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
            // Claude's bridge turns the answer itself into permission-host.ts's refusal, which says it belonged to a
            // question; every other runtime is told no.
            let refusal = if answering && pending.request.runtime == "claude" {
                answer.clone()
            } else {
                Answer::Decision { approval_id: pending.request.approval_id.clone(), decision: Decision::Deny, reason: None }
            };
            let _ = pending.tx.try_send(refusal);
            return Err(if answering {
                "Locust did not send that answer: it belongs to a question, and the runtime was waiting on an authorization."
            } else {
                "Locust did not send that decision: the runtime asked a question, which is answered rather than approved."
            }
            .into());
        }
        // A reason is the person's words on a denial, one line and bounded; it never rides an approval.
        let answer = match answer {
            Answer::Decision { approval_id, decision: Decision::Deny, reason } => {
                Answer::Decision { approval_id, decision: Decision::Deny, reason: deny_reason(reason.as_deref()) }
            }
            Answer::Decision { approval_id, decision, .. } => Answer::Decision { approval_id, decision, reason: None },
            other => other,
        };
        // The host's own guard on Always: the record keeps why it was applied as once (index.ts answerApproval words).
        let mut enforced_note: Option<String> = None;
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
                    // Host forces once for reaching commands (Electron enforcedAnswer), and says so.
                    enforced_note = note;
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
                "This action must be asked about every time. Choose Allow once or Deny.".into(),
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
            guard.always.add(&pending.request.mission_id, key);
        }
        let mut recorded = record_of(&pending.request, &answer);
        if enforced_note.is_some() {
            recorded.words = enforced_note;
        }
        guard.recorded.push(recorded.clone());
        let denied = matches!(&answer, Answer::Decision { decision: Decision::Deny, .. });
        let steered = match &answer {
            Answer::Decision { decision: Decision::Deny, reason: Some(why), .. } if pending.steers_reason => Some(why.clone()),
            _ => None,
        };
        if pending.tx.try_send(answer).is_err() {
            if let Some(key) = &pending.request.always_key {
                guard.always.remove(&pending.request.mission_id, key);
            }
            guard.recorded.pop();
            return Err("That approval is no longer waiting.".into());
        }
        // The person said no to this item: when the runtime reports it failed, it is said as declined (0.374).
        if denied && let Some(item) = &pending.item_id {
            guard
                .declined
                .entry(pending.run_id.clone())
                .or_insert_with(|| (pending.request.mission_id.clone(), HashSet::new()))
                .1
                .insert(item.clone());
        }
        if let Some(reason) = steered {
            guard.denied_saying.push(DeniedSaying {
                run_id: pending.run_id.clone(),
                mission_id: pending.request.mission_id.clone(),
                reason,
            });
        }
        Ok(recorded)
    }

    /// The items in a run the person declined, by item id (`with_declines`).
    pub fn declined(&self, run_id: &str) -> HashSet<String> {
        crate::invariants::lock(&self.inner)
            .declined
            .get(run_id)
            .map(|(_, items)| items.clone())
            .unwrap_or_default()
    }

    /// The denials the person explained in a run, each once, for the run to be shown as its next input.
    pub fn take_denied_saying(&self, run_id: &str) -> Vec<DeniedSaying> {
        let mut guard = crate::invariants::lock(&self.inner);
        let (mine, others) = std::mem::take(&mut guard.denied_saying).into_iter().partition(|d| d.run_id == run_id);
        guard.denied_saying = others;
        mine
    }

    /// The denials the person explained in a mission, each once: for the host to steer to a run that takes its next
    /// input itself (index.ts `onDeniedSaying`), an ACP agent's.
    pub fn take_denied_saying_for_mission(&self, mission_id: &str) -> Vec<DeniedSaying> {
        let mut guard = crate::invariants::lock(&self.inner);
        let (mine, others) =
            std::mem::take(&mut guard.denied_saying).into_iter().partition(|d| d.mission_id == mission_id);
        guard.denied_saying = others;
        mine
    }

    /// The run is over: refuse whatever it was still asking.
    pub fn release(&self, mission_id: &str) {
        let mut guard = crate::invariants::lock(&self.inner);
        guard.ended.insert(mission_id.to_owned(), Instant::now());
        guard.always.release(mission_id);
        guard.teammates.remove(mission_id);
        guard.declined.retain(|_, (mission, _)| mission != mission_id);
        guard.denied_saying.retain(|d| d.mission_id != mission_id);
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
                    reason: Some(TURN_ENDED.into()),
                });
            }
        }
    }
}

/// A call the person declined, recorded as declined (approval-channel.ts `withDeclines`, 0.374).
///
/// Codex 0.157 runs a command inside a script, and a declined approval comes back as the script FAILING -- no
/// `declined` status -- so the row read "failed" in red and the fold counted a command that "exited non-zero". Nothing
/// failed: the person said no. Locust answered that approval itself, so a failure of an item the person declined is
/// said as declined, and reads "refused" like every other runtime's.
/// A fileChange item's tool rows carry the change itself (approval-channel.ts `withFileChanges`). The normaliser names
/// the item's files as Codex wrote them and gives no patch, so after an approved edit the activity read as though Codex
/// had not reported the change beside the card that had just shown its diff. The changes remembered by item id for the
/// approval give the row its paths, relative to the folder, and its patch; a change that failed or was declined wrote
/// nothing, so it names its files and carries no patch.
pub fn with_file_changes(
    events: Vec<crate::runtime::Event>,
    changes_by_item: &std::collections::HashMap<String, Vec<crate::approval_patch::FileChangeRecord>>,
    workspace_path: &str,
) -> Vec<crate::runtime::Event> {
    events
        .into_iter()
        .map(|mut event| {
            if !matches!(event.kind.as_str(), "tool.started" | "tool.completed" | "tool.failed")
                || event.payload["toolKind"] != "fileChange"
            {
                return event;
            }
            let Some(changes) = event.payload["itemId"].as_str().and_then(|id| changes_by_item.get(id)).filter(|c| !c.is_empty()) else {
                return event;
            };
            let workspace = Some(workspace_path).filter(|w| !w.is_empty());
            let paths: Vec<String> = changes.iter().map(|c| crate::approval_patch::relative_to_folder(&c.path, workspace)).collect();
            event.payload["command"] = json!(paths.join("\n"));
            if event.kind != "tool.failed"
                && let Some(patch) = crate::approval_patch::approval_patch_from(Some(changes), workspace)
            {
                event.payload["patch"] = patch.to_json();
            }
            event
        })
        .collect()
}

pub fn with_declines(events: Vec<crate::runtime::Event>, declined: &HashSet<String>) -> Vec<crate::runtime::Event> {
    if declined.is_empty() {
        return events;
    }
    events
        .into_iter()
        .map(|mut event| {
            let item = event.payload["itemId"].as_str();
            if event.kind == "tool.failed"
                && item.is_some_and(|id| declined.contains(id))
                && event.payload["status"] != "declined"
            {
                event.payload["status"] = json!("declined");
            }
            event
        })
        .collect()
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

/// `approval-channel.ts:diffPatchFrom`
pub fn diff_patch_from(diff: &str, cwd: &str) -> Option<crate::approval_patch::ToolPatch> {
    let cwd = (!cwd.is_empty()).then_some(cwd);
    let mut out = Vec::new();
    for line in diff.lines() {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.starts_with("Index: ")
            || (line.len() >= 10 && line.bytes().all(|b| b == b'='))
        {
            continue;
        }
        if let Some(rest) = line.strip_prefix("--- ").or_else(|| line.strip_prefix("+++ ")) {
            let prefix = &line[..3];
            let rel = crate::approval_patch::relative_to_folder(rest.trim(), cwd);
            out.push(format!("{prefix} {rel}"));
        } else {
            out.push(line.to_owned());
        }
    }
    let text = out.join("\n").trim().to_owned();
    text.contains("@@")
        .then(|| crate::approval_patch::tool_patch_from(&text))
        .flatten()
}

/// `approval-channel.ts:openCodePermissionRequest`
pub fn open_code_permission_request(permission: &Value, cwd: &str) -> Value {
    const OUTSIDE_NOT_UNDOABLE: &str = "Not from here — it is outside your project folder, so the project’s version control does not hold it.";
    let perm = permission["permission"].as_str().unwrap_or("unknown");
    let patterns: Vec<String> = permission["patterns"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect();
    let always: Vec<String> = permission["always"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    let said = |key: &str| -> Option<String> {
        permission["metadata"]
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let always_covers = if always.is_empty() {
        None
    } else if always.len() == 1 && always[0] == "*" {
        Some(match perm {
            "edit" => "every file change it asks for".into(),
            "bash" => "every command it asks to run".into(),
            "webfetch" => "every web page it asks to fetch".into(),
            "external_directory" => "every place outside the project it asks to reach".into(),
            other => format!("every “{other}” it asks for"),
        })
    } else {
        Some(format!(
            "anything matching {}",
            always
                .iter()
                .map(|p| format!("“{p}”"))
                .collect::<Vec<_>>()
                .join(" or ")
        ))
    };
    let by = if permission["bySubagent"].as_bool() == Some(true) {
        ", asked by a subagent it started"
    } else {
        ""
    };
    let card = |summary_override: Option<&str>,
                plain: &str,
                data_sent: Option<&str>,
                reversible: Option<&str>|
     -> Value {
        let mut obj = serde_json::Map::new();
        let summary = summary_override
            .or_else(|| (!by.is_empty()).then_some(plain))
            .map(|s| format!("{s}{by}"));
        if let Some(s) = summary {
            obj.insert("summary".into(), json!(s));
        }
        if let Some(d) = data_sent {
            obj.insert("dataSentSays".into(), json!(d));
        }
        if let Some(r) = reversible {
            obj.insert("reversibleSays".into(), json!(r));
        }
        if let Some(ac) = &always_covers {
            obj.insert("alwaysCovers".into(), json!(ac));
        }
        Value::Object(obj)
    };
    let (method, mut params) = match perm {
        "bash" => {
            let cmd = said("command").unwrap_or_else(|| patterns.join(" "));
            (
                "item/commandExecution/requestApproval",
                json!({
                    "command": cmd,
                    "cwd": cwd,
                    "locustCard": card(None, "Run a command", None, None),
                }),
            )
        }
        "edit" => {
            let file = said("filepath")
                .or_else(|| said("filePath"))
                .unwrap_or_else(|| patterns.join(", "));
            let diff = said("diff");
            let outside = !file.is_empty()
                && !cwd.is_empty()
                && crate::approval_rules::inside_folder(&file, cwd).is_none();
            let locust_card = if outside {
                card(
                    Some("Change 1 file outside your project folder"),
                    "Change 1 file",
                    None,
                    Some(OUTSIDE_NOT_UNDOABLE),
                )
            } else {
                card(None, "Change 1 file", None, None)
            };
            let rel = crate::approval_patch::relative_to_folder(
                &file,
                (!cwd.is_empty()).then_some(cwd),
            );
            let mut p = json!({
                "summary": rel,
                "cwd": cwd,
                "locustCard": locust_card,
            });
            if let Some(d) = diff {
                p["unifiedDiff"] = json!(d);
            }
            ("item/fileChange/requestApproval", p)
        }
        "external_directory" => (
            "item/commandExecution/requestApproval",
            json!({
                "command": patterns.join("\n"),
                "cwd": cwd,
                "locustCard": card(
                    Some("Reach files outside your project folder"),
                    "",
                    Some("Nothing by this alone: it lets the teammate’s tools read or change these paths on this machine."),
                    Some("Reading changes nothing. A change there is outside the project’s version control."),
                ),
            }),
        ),
        "webfetch" => {
            let address = said("url").unwrap_or_else(|| patterns.join("\n"));
            (
                "item/commandExecution/requestApproval",
                json!({
                    "command": address,
                    "cwd": cwd,
                    "locustCard": card(
                        Some("Fetch a web page"),
                        "",
                        Some("The address above is requested from this machine, and the page it returns goes to the model."),
                        Some("Nothing on this machine is changed by fetching."),
                    ),
                }),
            )
        }
        other => {
            let filtered: Vec<&str> = patterns
                .iter()
                .map(String::as_str)
                .filter(|p| *p != "*")
                .collect();
            (
                "item/commandExecution/requestApproval",
                json!({
                    "command": filtered.join("\n"),
                    "cwd": cwd,
                    "locustCard": card(
                        Some(&format!("Use {other}")),
                        "",
                        Some(&format!("Unknown. OpenCode names this “{other}”, and Locust cannot tell what it sends.")),
                        Some(&format!("Unknown — Locust cannot tell what “{other}” does.")),
                    ),
                }),
            )
        }
    };
    let covers: Vec<String> = if !always.is_empty() {
        always
    } else {
        patterns.into_iter().filter(|p| !p.is_empty()).collect()
    };
    if !covers.is_empty() {
        params["locustAlwaysKey"] = json!(format!("opencode:{perm}:{}", covers.join("\n")));
    }
    json!({
        "id": 0,
        "method": method,
        "params": params,
    })
}

/// `approval-channel.ts:openCodeReplyFor`
pub fn open_code_reply_for(result: &Value) -> Value {
    match result["decision"].as_str() {
        Some("accept" | "acceptForSession") => json!("once"),
        _ => match result["reason"].as_str().filter(|r| !r.is_empty()) {
            Some(reason) => json!({
                "reply": "reject",
                "message": denied_saying(reason),
            }),
            None => json!("reject"),
        },
    }
}

fn request_view(card: &Request) -> crate::approval_rules::RequestView<'_> {
    crate::approval_rules::RequestView {
        kind: card.kind.as_str(),
        summary: &card.summary,
        detail: &card.detail,
        runtime: Some(card.runtime.as_str()),
        patch_text: card.patch.as_ref().map(|p| p.text.as_str()),
    }
}

/// What the saved rules made of a request: an answer, or a card, with why when a person would wonder.
enum Ruled {
    Answered(Answer),
    Ask(Option<String>),
}

/// Unreadable rules also block session Always; they are never treated as an empty store.
fn decide_by_saved_rules(
    request: &Request,
    teammate_id: Option<String>,
) -> Result<Ruled, String> {
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
                Ruled::Answered(Answer::Decision {
                    approval_id: request.approval_id.clone(),
                    decision: Decision::ApproveOnce,
                    reason: None,
                })
            }
            crate::approval_rules::RuleVerdict::Deny { rule } => {
                let _ = crate::approval_rule_store::used(&store, &rule.rule_id);
                Ruled::Answered(Answer::Decision {
                    approval_id: request.approval_id.clone(),
                    decision: Decision::Deny,
                    reason: Some(crate::approval_rules::rule_sentence(&rule, None)),
                })
            }
            crate::approval_rules::RuleVerdict::Ask { why } => Ruled::Ask(why),
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
    if let Some(why) = asked_again_of(&card.approval_id) {
        return format!("Nothing has happened yet. {why}");
    }
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
/// approval-data.ts commandReachesNetwork: the command names a program that reaches the network as its ordinary
/// purpose (a whole word, so `curling` and `wget-notes.md` do not count), or holds a bare URL.
pub fn command_reaches_network(command: &str) -> bool {
    crate::invariants::regex(r"(?i)(^|[\s|&;(<>])(curl|wget|ssh|scp|sftp|rsync|nc|netcat|telnet|ftp|npm|pnpm|yarn|pip|pip3|cargo|gem|go|brew|apt|docker|kubectl|aws|gcloud|az|gh|git)(\s|$)")
        .is_match(command)
        || crate::invariants::regex(r"(?i)\bhttps?://").is_match(command)
}

/// approval-data.ts dataSentLine: the card's Data sent row, or none for a question. "Nothing" is said only for a file
/// change, where it is a fact; a command is said to be able to reach the network, or Unknown, never to send nothing.
pub fn data_sent_line(kind: &Kind, command: &str) -> Option<String> {
    Some(match kind {
        Kind::FileChange => "Nothing. The change is written to this machine and sent nowhere.",
        Kind::Connector => "The input above, to the service the connector reaches. It acts there, not on this machine.",
        Kind::Question => return None,
        Kind::Command if command_reaches_network(command) => {
            "This command can reach the network. Locust cannot see what it would send — read it above."
        }
        Kind::Command => "Unknown. Locust cannot tell what a command sends; a script can do anything the sandbox allows.",
    }
    .to_owned())
}

pub fn effect_lines(card: &Request) -> Vec<(&'static str, String)> {
    if card.kind == Kind::Command
        && !command_reaches_network(&card.detail)
        && card.data_sent_says.is_none()
        && card.reversible_says.is_none()
    {
        return vec![("Effects", "Unknown. Locust cannot tell what a command sends or undo what it does; it can do anything the sandbox allows.".into())];
    }
    let mut lines = Vec::new();
    // The asking route's own words first (ApprovalCard.tsx); a question draws no Data sent row.
    if let Some(data) = card.data_sent_says.clone().or_else(|| data_sent_line(&card.kind, &card.detail)) {
        lines.push(("Data sent", data));
    }
    let undo = card.reversible_says.clone().unwrap_or_else(|| {
        match card.kind {
            Kind::Command => "Unknown — a command can do anything the workspace sandbox allows.",
            Kind::FileChange => "Yes for tracked files, if the workspace is under version control.",
            Kind::Connector => "Unknown — a connector acts on the service it reaches, and Locust cannot undo what happens there.",
            Kind::Question => "Nothing is changed by answering.",
        }
        .into()
    });
    lines.push(("Reversible", undo));
    lines
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
#[path = "approval_whole_action_tests.rs"]
mod whole_action_tests;

#[cfg(test)]
#[path = "approval_codex_always_tests.rs"]
mod codex_always_tests;

#[cfg(test)]
#[path = "approval_rules_cases_tests.rs"]
mod approval_rules_cases_tests;

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
        assert_eq!(protocol_answer_for(&decision(Decision::Deny)), json!({"decision": "decline"}));
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
            .add("m", "claude:Bash");
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
        let rule_deadline = Instant::now() + Duration::from_secs(2);
        let result = ch
            .ask_claude(
                "r",
                "m",
                "a",
                "Bash",
                &json!({"command":"npm test"}),
                "/project",
                &|| Instant::now() >= rule_deadline,
            )
            .unwrap();
        assert_eq!(result["behavior"], "deny");
        assert_eq!(ch.take_recorded()[0].by, "saved-rule");
        let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let watcher_done = Arc::clone(&done);
        let watcher_ch = ch.clone();
        let watcher = std::thread::spawn(move || {
            while !watcher_done.load(std::sync::atomic::Ordering::Relaxed) {
                if let Some(card) = watcher_ch.waiting().first() {
                    let _ = watcher_ch.decide(Answer::Decision {
                        approval_id: card.approval_id.clone(),
                        decision: Decision::ApproveOnce,
                        reason: None,
                    });
                    return;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
        });
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
        done.store(true, std::sync::atomic::Ordering::Relaxed);
        watcher.join().unwrap();
        assert_eq!(result["behavior"], "deny");
        assert!(ch.take_recorded().is_empty());
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
            .add("n", "claude:Bash");
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
        let _rules = crate::native::hooks::no_saved_rules();
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
            patch: None,

            skippable: false, answer_in: None, claude_input: None,

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
        let _rules = crate::native::hooks::no_saved_rules();
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
        let _rules = crate::native::hooks::no_saved_rules();
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
        let always_deadline = Instant::now() + Duration::from_secs(2);
        assert_eq!(
            ch.ask_claude("r", "m", "b", "Write", &json!({}), "/tmp", &|| Instant::now() >= always_deadline)
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
        let _rules = crate::native::hooks::no_saved_rules();
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

    #[test]
    fn says_what_always_lets_through_on_claude_the_one_tool_or_every_command_for_bash() {
        assert_eq!(
            always_covers("mcp__github__list_issues"),
            "list_issues on github again, and no other tool"
        );
        assert_eq!(always_covers("Bash"), "every command it runs");
    }

    #[test]
    fn carries_opencodes_own_words_to_the_card_a_fetch_and_what_always_matches() {
        let _rules = crate::native::hooks::no_saved_rules();
        let ch = Channel::new();
        let ch2 = ch.clone();
        let asked = open_code_permission_request(
            &json!({
                "permission": "webfetch",
                "patterns": ["https://example.com/"],
                "always": ["*"],
                "metadata": {"url": "https://example.com/"},
            }),
            "C:/work",
        );
        let handle = std::thread::spawn(move || {
            ch2.ask_codex_with_changes(
                "run_1",
                "m_1",
                "opencode",
                asked["method"].as_str().unwrap(),
                &asked["params"],
                asked["id"].clone(),
                None,
                &|| false,
            )
        });
        for _ in 0..100 {
            if ch.pending_count() == 1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let raised = ch.waiting();
        assert_eq!(raised.len(), 1);
        assert_eq!(raised[0].summary, "Fetch a web page");
        assert_eq!(raised[0].detail, "https://example.com/");
        assert_eq!(
            raised[0].data_sent_says.as_deref(),
            Some("The address above is requested from this machine, and the page it returns goes to the model.")
        );
        assert_eq!(
            raised[0].always_covers.as_deref(),
            Some("every web page it asks to fetch")
        );
        ch.release("m_1");
        let _ = handle.join().unwrap();
    }

    #[test]
    fn says_when_a_subagent_the_teammate_started_is_the_one_asking() {
        let bash = open_code_permission_request(
            &json!({
                "permission": "bash",
                "patterns": ["echo FROM-SUBAGENT"],
                "bySubagent": true,
                "metadata": {"command": "echo FROM-SUBAGENT"},
            }),
            "C:/work",
        );
        assert_eq!(bash["params"]["command"], "echo FROM-SUBAGENT");
        assert_eq!(
            bash["params"]["locustCard"]["summary"],
            "Run a command, asked by a subagent it started"
        );
        let fetch = open_code_permission_request(
            &json!({
                "permission": "webfetch",
                "patterns": ["https://example.com/"],
                "bySubagent": true,
                "metadata": {},
            }),
            "C:/work",
        );
        assert_eq!(
            fetch["params"]["locustCard"]["summary"],
            "Fetch a web page, asked by a subagent it started"
        );
        let plain = open_code_permission_request(
            &json!({
                "permission": "bash",
                "patterns": ["ls"],
                "metadata": {"command": "ls"},
            }),
            "C:/work",
        );
        assert!(!plain.to_string().contains("subagent"));
    }

    #[test]
    fn says_a_claude_write_outside_the_folder_is_outside_it_and_not_undoable_from_here() {
        let card = describe_claude_tool(
            "Write",
            &json!({"file_path": "C:/Windows/System32/drivers/etc/hosts", "content": "x"}),
            "C:/work",
        );
        assert_eq!(card.0, Kind::FileChange);
        assert_eq!(card.1, "Change 1 file outside your project folder");
        assert_eq!(card.2, "C:/Windows/System32/drivers/etc/hosts");
        assert!(card.4.as_deref().unwrap_or("").starts_with("Not from here"));
        let inside = describe_claude_tool(
            "Write",
            &json!({"file_path": "C:/work/notes.txt", "content": "x"}),
            "C:/work",
        );
        assert_eq!(
            (inside.0, inside.1.as_str(), inside.2.as_str(), inside.3, inside.4),
            (Kind::FileChange, "Change 1 file", "notes.txt", None, None)
        );
    }

    #[test]
    fn a_claude_bash_request_is_a_command_card() {
        let card = describe_claude_tool("Bash", &json!({"command": "node check.js"}), "C:/work");
        assert_eq!((card.0, card.1.as_str(), card.2.as_str()), (Kind::Command, "Run a command", "node check.js"));
    }

    #[test]
    fn a_claude_edit_request_is_a_file_card_named_inside_the_folder() {
        let card = describe_claude_tool(
            "Edit",
            &json!({"file_path": "C:/work/notes.txt", "old_string": "a", "new_string": "b"}),
            "C:/work",
        );
        assert_eq!((card.0, card.1.as_str(), card.2.as_str()), (Kind::FileChange, "Change 1 file", "notes.txt"));
    }

    #[test]
    fn a_claude_mcp_request_is_still_a_connector() {
        let card = describe_claude_tool("mcp__claude_ai_Gmail__send_message", &json!({"to": "x"}), "C:/work");
        assert_eq!(card.0, Kind::Connector);
    }

    #[test]
    fn draws_a_codex_file_change_whose_approval_arrived_before_its_item_was_read() {
        let _rules = crate::native::hooks::no_saved_rules();
        let ch = Channel::new();
        let ch2 = ch.clone();
        let changes_by_item = std::sync::Arc::new(std::sync::Mutex::new(
            std::collections::HashMap::<String, Vec<crate::approval_patch::FileChangeRecord>>::new(),
        ));
        let changes_writer = changes_by_item.clone();
        let handle = std::thread::spawn(move || {
            ch2.ask_codex_with_changes(
                "run_2",
                "m_2",
                "codex",
                "item/fileChange/requestApproval",
                &json!({"itemId": "item_1", "cwd": "C:/work"}),
                json!(7),
                Some(&changes_by_item),
                &|| false,
            )
        });
        std::thread::sleep(Duration::from_millis(5));
        crate::invariants::lock(&changes_writer).insert(
            "item_1".into(),
            vec![crate::approval_patch::FileChangeRecord {
                path: "C:/work/approved.txt".into(),
                kind: crate::approval_patch::FileChangeKind::Add,
                move_path: None,
                diff: "hello\n".into(),
            }],
        );
        for _ in 0..100 {
            if ch.pending_count() == 1 {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let raised = ch.waiting();
        assert_eq!(raised.len(), 1);
        assert_eq!(raised[0].kind, Kind::FileChange);
        assert_eq!(raised[0].summary, "Change 1 file");
        assert!(raised[0]
            .patch
            .as_ref()
            .is_some_and(|p| p.text.contains("+hello")));
        ch.release("m_2");
        let _ = handle.join().unwrap();
    }

    #[test]
    fn diff_patch_from_and_opencode_edit_cover_outside_folder_and_replies() {
        let raw = "Index: C:/work/src/a.ts\n===================================================================\n--- C:/work/src/a.ts\n+++ C:/work/src/a.ts\n@@ -1 +1 @@\n-old\n+new\n";
        let patch = diff_patch_from(raw, "C:/work").expect("valid unified diff");
        assert!(!patch.text.contains("Index: "));
        assert!(!patch.text.contains("==="));
        assert!(patch.text.contains("--- src/a.ts"));
        assert!(patch.text.contains("+++ src/a.ts"));
        assert_eq!((patch.added, patch.removed), (1, 1));
        assert!(diff_patch_from("--- a.ts\n+++ a.ts\nno hunks", "C:/work").is_none());

        let outside = open_code_permission_request(
            &json!({
                "permission": "edit",
                "patterns": ["/etc/hosts"],
                "always": ["*"],
                "metadata": {"filepath": "/etc/hosts", "diff": raw},
            }),
            "/work",
        );
        assert_eq!(
            outside["params"]["locustCard"]["summary"],
            "Change 1 file outside your project folder"
        );
        assert!(outside["params"]["locustCard"]["reversibleSays"]
            .as_str()
            .unwrap_or("")
            .starts_with("Not from here"));
        assert_eq!(
            outside["params"]["locustCard"]["alwaysCovers"],
            "every file change it asks for"
        );
        assert_eq!(outside["params"]["unifiedDiff"], raw);

        assert_eq!(open_code_reply_for(&json!({"decision": "accept"})), json!("once"));
        assert_eq!(open_code_reply_for(&json!({"decision": "reject"})), json!("reject"));
        assert_eq!(
            open_code_reply_for(&json!({"decision": "reject", "reason": "not now"})),
            json!({"reply": "reject", "message": "The person declined this, and said: not now"})
        );

        let codex_card = Request {
            approval_id: "ap_p".into(),
            mission_id: "m_p".into(),
            runtime: "codex".into(),
            kind: Kind::FileChange,
            summary: "Change 1 file".into(),
            detail: String::new(),
            cwd: Some("C:/work".into()),
            requested_at: now_iso(),
            always_covers: None,
            always_key: None,
            data_sent_says: None,
            reversible_says: None,
            patch: Some(patch),
            skippable: false, answer_in: None, claude_input: None,
            questions: Vec::new(),
        };
        assert_eq!(
            crate::approval_rules::ruled_action_of(request_view(&codex_card)),
            crate::approval_rules::RuledAction::Edit {
                paths: vec!["src/a.ts".into()]
            }
        );
    }

    #[test]
    fn asks_every_time_refusal_says_allow_once_not_approve_once() {
        let ch = Channel::new();
        let req = Request {
            approval_id: "ap_browser_1".into(),
            mission_id: "m_browser".into(),
            runtime: "browser".into(),
            kind: Kind::Connector,
            summary: "Click submit".into(),
            detail: "Submit the form".into(),
            cwd: None,
            requested_at: now_iso(),
            always_covers: None,
            always_key: None,
            data_sent_says: None,
            reversible_says: None,
            patch: None,
            skippable: false,
            answer_in: None,
            claude_input: None,
            questions: Vec::new(),
        };
        let worker = ch.clone();
        let handle = std::thread::spawn(move || worker.ask(req, &|| false));
        let end = Instant::now() + Duration::from_secs(5);
        while ch.waiting().is_empty() {
            assert!(Instant::now() < end);
            std::thread::sleep(Duration::from_millis(5));
        }
        let err = ch
            .decide(Answer::Decision {
                approval_id: "ap_browser_1".into(),
                decision: Decision::ApproveAlways,
                reason: None,
            })
            .unwrap_err();
        assert_eq!(
            err,
            "This action must be asked about every time. Choose Allow once or Deny."
        );
        ch.release("m_browser");
        let _ = handle.join();
    }
}

#[cfg(test)]
#[path = "web_search_asks_tests.rs"]
mod web_search_asks_tests;

#[cfg(test)]
#[path = "opencode_card_tests.rs"]
mod opencode_card_tests;

#[cfg(test)]
#[path = "approval_declined_tests.rs"]
mod approval_declined_tests;

#[cfg(test)]
#[path = "approval_denial_reason_tests.rs"]
mod approval_denial_reason_tests;

#[cfg(test)]
#[path = "always_refused_for_reach_case_tests.rs"]
mod always_refused_for_reach_case_tests;

#[cfg(test)]
#[path = "approval_one_path_tests.rs"]
mod one_path_tests;
#[cfg(test)] #[path = "approval_asks_every_time_refusal_tests.rs"] mod asks_every_time_refusal_tests;

#[cfg(test)]
#[path = "approval_record_bounds_tests.rs"]
mod approval_record_bounds_tests;

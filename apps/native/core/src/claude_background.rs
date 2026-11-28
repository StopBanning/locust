//! Claude Code work that keeps going when Locust closes (`claude --bg`, `claude agents --json`,
//! `claude stop`).
//!
//! Ports `apps/desktop/src/main/claude-background.ts` (Row 1) and
//! `apps/desktop/src/main/claude-background-runs.ts` (Row 2).
//! Locust only watches these sessions; it never answers approval prompts for them.

use chrono::{DateTime, SecondsFormat, Utc};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// State of a Claude Code background session as reported by `claude agents --json`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BackgroundState {
    Working,
    Blocked,
    Done,
    Failed,
    Stopped,
    Unknown,
}

impl BackgroundState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Blocked => "blocked",
            Self::Done => "done",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
            Self::Unknown => "unknown",
        }
    }
}

/// One background session entry parsed from `claude agents --json`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundAgent {
    pub id: String,
    pub session_id: Option<String>,
    pub cwd: Option<String>,
    pub name: Option<String>,
    pub started_at: Option<String>,
    pub state: BackgroundState,
    /// What a blocked session waits on, in Claude Code's own words when it gives any.
    pub waiting_for: Option<String>,
}

/// Locust's sandbox mode mapped to Claude Code's `--permission-mode` for a `--bg` session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackgroundSandbox {
    ReadOnly,
    WorkspaceWrite,
    FullAccess,
}

/// Options for building `claude --bg ...` arguments.
#[derive(Clone, Debug)]
pub struct BackgroundArgsOptions<'a> {
    pub prompt: &'a str,
    pub sandbox: BackgroundSandbox,
    pub ask_each: bool,
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
    pub resume_session_id: Option<&'a str>,
}

fn trimmed_text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Claude Code's state word, or `Unknown` for one this build has not met (never a guess).
pub fn background_state_of(value: Option<&str>) -> BackgroundState {
    match value.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("working") => BackgroundState::Working,
        Some("blocked") => BackgroundState::Blocked,
        Some("done") => BackgroundState::Done,
        Some("failed") => BackgroundState::Failed,
        Some("stopped") => BackgroundState::Stopped,
        _ => BackgroundState::Unknown,
    }
}

/// What a blocked session says it waits for: a string, or an object's own words.
fn waiting_words(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(s) = value.as_str() {
        let trimmed = s.trim();
        return if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }
    let obj = value.as_object()?;
    for key in ["message", "description", "title", "tool", "kind", "type"] {
        if let Some(said) = trimmed_text(obj.get(key)) {
            return Some(said);
        }
    }
    None
}

fn format_started_at(value: Option<&Value>) -> Option<String> {
    let value = value?;
    if let Some(ms) = value.as_i64() {
        return DateTime::from_timestamp_millis(ms)
            .map(|dt| dt.to_rfc3339_opts(SecondsFormat::Millis, true));
    }
    if let Some(ms) = value.as_f64().filter(|n| n.is_finite()) {
        return DateTime::from_timestamp_millis(ms as i64)
            .map(|dt| dt.to_rfc3339_opts(SecondsFormat::Millis, true));
    }
    trimmed_text(Some(value))
}

/// The background sessions in `claude agents --json`; interactive ones and malformed entries are left out.
/// `None` when the answer is not a list at all: reading an empty/malformed answer as "no sessions"
/// would wrongly mark every live session as stopped.
pub fn parse_background_agents(json: &str) -> Option<Vec<BackgroundAgent>> {
    let parsed: Value = serde_json::from_str(json).ok()?;
    let list = parsed.as_array()?;
    let mut agents = Vec::new();
    for entry in list {
        let Some(obj) = entry.as_object() else {
            continue;
        };
        if obj.get("kind").and_then(Value::as_str) != Some("background") {
            continue;
        }
        let Some(id) = trimmed_text(obj.get("id")) else {
            continue;
        };
        agents.push(BackgroundAgent {
            id,
            session_id: trimmed_text(obj.get("sessionId")),
            cwd: trimmed_text(obj.get("cwd")),
            name: trimmed_text(obj.get("name")),
            started_at: format_started_at(obj.get("startedAt")),
            state: background_state_of(obj.get("state").and_then(Value::as_str)),
            waiting_for: waiting_words(obj.get("waitingFor")),
        });
    }
    Some(agents)
}

/// Still going: Locust keeps watching. Done, failed and stopped are over; so is one Claude Code no longer lists.
pub fn background_is_live(state: BackgroundState) -> bool {
    matches!(
        state,
        BackgroundState::Working | BackgroundState::Blocked | BackgroundState::Unknown
    )
}

/// The card's words for a state.
pub fn background_sentence(state: BackgroundState, waiting_for: Option<&str>) -> String {
    match state {
        BackgroundState::Working => "Working in the background.".to_string(),
        BackgroundState::Blocked => match waiting_for {
            Some(what) => format!(
                "Waiting for you: {what}. Open it to answer; Locust does not answer for you."
            ),
            None => "Waiting for you. Open it to answer; Locust does not answer for you.".to_string(),
        },
        BackgroundState::Done => "Finished in the background.".to_string(),
        BackgroundState::Failed => {
            "Stopped with an error in the background. Open it to see what happened.".to_string()
        }
        BackgroundState::Stopped => "Stopped.".to_string(),
        BackgroundState::Unknown => "Running in the background.".to_string(),
    }
}

/// Locust's mode, said as Claude Code's own permission mode for a session Locust does not run.
pub fn background_permission_mode(sandbox: BackgroundSandbox, ask_each: bool) -> &'static str {
    if ask_each && sandbox == BackgroundSandbox::WorkspaceWrite {
        return "default";
    }
    match sandbox {
        BackgroundSandbox::ReadOnly => "plan",
        BackgroundSandbox::WorkspaceWrite => "acceptEdits",
        BackgroundSandbox::FullAccess => "bypassPermissions",
    }
}

/// `claude --bg ...`: the conversation's own session when it has one, in the folder, on the chosen model and mode.
pub fn background_args(options: &BackgroundArgsOptions<'_>) -> Result<Vec<String>, String> {
    let prompt = options.prompt.trim();
    if prompt.is_empty() {
        return Err("A background turn needs a message.".to_string());
    }
    let mut args = vec!["--bg".to_string()];
    if let Some(resume) = options.resume_session_id {
        args.push("--resume".to_string());
        args.push(resume.to_string());
    }
    args.push("--permission-mode".to_string());
    args.push(background_permission_mode(options.sandbox, options.ask_each).to_string());
    if let Some(model) = options.model {
        args.push("--model".to_string());
        args.push(model.to_string());
    }
    if let Some(effort) = options.effort {
        args.push("--effort".to_string());
        args.push(effort.to_string());
    }
    // After `--`, so a message that begins with a dash is a message, never a flag.
    args.push("--".to_string());
    args.push(prompt.to_string());
    Ok(args)
}

/// `Workspace not trusted. ...`: the folder needs the person, once, in Claude Code itself.
pub fn needs_trust(said: &str) -> bool {
    let lower = said.to_ascii_lowercase();
    lower.contains("workspace not trusted") || lower.contains("accept the trust prompt")
}

/// The id `claude --bg` prints, from what it printed ("backgrounded · ad39859a").
pub fn background_id_from(said: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)backgrounded\s*\S\s*([0-9a-f]{6,})").expect("valid background id regex")
    });
    re.captures(said)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// Result of completing one poll tick of `BackgroundWatch`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchTickOutcome {
    /// Emitted only when the observed set of watched agents changed since the previous notice.
    pub changed: Option<Vec<BackgroundAgent>>,
    /// True when at least one watched id is still live and another poll should be scheduled.
    pub schedule_next: bool,
}

/// Poll state machine for `claude agents --json`:
/// polls only while at least one of Locust's own ids is live, marks missing ids as `Stopped`,
/// suppresses duplicate notices, and keeps a single poll loop when `watch()` arrives mid-tick.
#[derive(Clone, Debug, Default)]
pub struct BackgroundWatch {
    watched: Vec<String>,
    watched_set: HashSet<String>,
    timer_scheduled: bool,
    ticking: bool,
    last_snapshot: Option<Vec<BackgroundAgent>>,
}

impl BackgroundWatch {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start (or keep) watching these ids. Returns `true` if the caller should schedule a poll now.
    pub fn watch(&mut self, ids: &[&str]) -> bool {
        for &id in ids {
            if self.watched_set.insert(id.to_string()) {
                self.watched.push(id.to_string());
            }
        }
        if !self.watched.is_empty() && !self.timer_scheduled && !self.ticking {
            self.timer_scheduled = true;
            true
        } else {
            false
        }
    }

    /// Currently watched live ids.
    pub fn watching(&self) -> Vec<String> {
        self.watched.clone()
    }

    /// True if a timer is currently armed.
    pub fn timer_scheduled(&self) -> bool {
        self.timer_scheduled
    }

    /// Mark the start of an in-flight `claude agents --json` look.
    pub fn begin_tick(&mut self) {
        self.timer_scheduled = false;
        self.ticking = true;
    }

    /// Finish an in-flight look with the parsed agent list (or `None` when the list call failed).
    pub fn finish_tick(&mut self, all: Option<&[BackgroundAgent]>) -> WatchTickOutcome {
        self.ticking = false;
        let mut changed = None;
        if let Some(all) = all {
            let mut seen: Vec<BackgroundAgent> = all
                .iter()
                .filter(|agent| self.watched_set.contains(&agent.id))
                .cloned()
                .collect();
            let seen_ids: HashSet<&str> = seen.iter().map(|a| a.id.as_str()).collect();
            let missing: Vec<BackgroundAgent> = self
                .watched
                .iter()
                .filter(|id| !seen_ids.contains(id.as_str()))
                .map(|id| BackgroundAgent {
                    id: id.clone(),
                    session_id: None,
                    cwd: None,
                    name: None,
                    started_at: None,
                    state: BackgroundState::Stopped,
                    waiting_for: None,
                })
                .collect();
            seen.extend(missing);

            for agent in &seen {
                if !background_is_live(agent.state) {
                    self.watched_set.remove(&agent.id);
                    self.watched.retain(|id| id != &agent.id);
                }
            }

            if self.last_snapshot.as_ref() != Some(&seen) {
                self.last_snapshot = Some(seen.clone());
                changed = Some(seen);
            }
        }
        let schedule_next = !self.watched.is_empty();
        if schedule_next {
            self.timer_scheduled = true;
        }
        WatchTickOutcome {
            changed,
            schedule_next,
        }
    }

    /// Stop watching everything and disarm any pending timer.
    pub fn stop(&mut self) {
        self.timer_scheduled = false;
        self.watched.clear();
        self.watched_set.clear();
    }
}

/// Said where the choice is made: Locust watches a background run; it does not run it.
pub const BACKGROUND_TRADE: &str = "Claude Code runs it, so it keeps going if you close Locust. Locust's approval cards and spend limits don't apply; Claude Code's own permission mode does, and when it needs a yes, you answer in Claude Code.";

/// The id Claude Code gives a background session: 6..=64 hex characters. Nothing else is passed on to `claude stop`.
pub fn is_valid_session_id(id: &str) -> bool {
    let len = id.len();
    (6..=64).contains(&len) && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// One background turn, as it is kept across launches (`claude-background.json`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackgroundRun {
    pub account: Option<String>,
    pub id: String,
    pub session_id: Option<String>,
    pub folder: String,
    pub prompt: String,
    pub started_at: String,
    /// The conversation it continues (any turn of it), when it continues one.
    pub conversation: Option<String>,
    pub teammate_id: Option<String>,
    pub model: Option<String>,
    pub state: BackgroundState,
    pub waiting_for: Option<String>,
    pub name: Option<String>,
    /// The conversation turn it came back as, once it is over.
    pub brought_in: Option<String>,
    /// It is over but could not be brought back: why, in words.
    pub not_brought_in: Option<String>,
}

impl BackgroundRun {
    fn to_json(&self) -> Value {
        let mut obj = serde_json::Map::new();
        if let Some(ref v) = self.account {
            obj.insert("account".into(), json!(v));
        }
        obj.insert("id".into(), json!(self.id));
        if let Some(ref v) = self.session_id {
            obj.insert("sessionId".into(), json!(v));
        }
        obj.insert("folder".into(), json!(self.folder));
        obj.insert("prompt".into(), json!(self.prompt));
        obj.insert("startedAt".into(), json!(self.started_at));
        if let Some(ref v) = self.conversation {
            obj.insert("conversation".into(), json!(v));
        }
        if let Some(ref v) = self.teammate_id {
            obj.insert("teammateId".into(), json!(v));
        }
        if let Some(ref v) = self.model {
            obj.insert("model".into(), json!(v));
        }
        obj.insert("state".into(), json!(self.state.as_str()));
        if let Some(ref v) = self.waiting_for {
            obj.insert("waitingFor".into(), json!(v));
        }
        if let Some(ref v) = self.name {
            obj.insert("name".into(), json!(v));
        }
        if let Some(ref v) = self.brought_in {
            obj.insert("broughtIn".into(), json!(v));
        }
        if let Some(ref v) = self.not_brought_in {
            obj.insert("notBroughtIn".into(), json!(v));
        }
        Value::Object(obj)
    }

    fn from_json(value: &Value) -> Option<Self> {
        let obj = value.as_object()?;
        let id = obj.get("id")?.as_str()?;
        if !is_valid_session_id(id) {
            return None;
        }
        let folder = obj.get("folder")?.as_str()?.to_string();
        let prompt = obj.get("prompt")?.as_str()?.to_string();
        let started_at = obj
            .get("startedAt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        Some(Self {
            account: obj.get("account").and_then(Value::as_str).map(str::to_string),
            id: id.to_string(),
            session_id: obj.get("sessionId").and_then(Value::as_str).map(str::to_string),
            folder,
            prompt,
            started_at,
            conversation: obj
                .get("conversation")
                .and_then(Value::as_str)
                .map(str::to_string),
            teammate_id: obj
                .get("teammateId")
                .and_then(Value::as_str)
                .map(str::to_string),
            model: obj.get("model").and_then(Value::as_str).map(str::to_string),
            state: background_state_of(obj.get("state").and_then(Value::as_str)),
            waiting_for: obj
                .get("waitingFor")
                .and_then(Value::as_str)
                .map(str::to_string),
            name: obj.get("name").and_then(Value::as_str).map(str::to_string),
            brought_in: obj.get("broughtIn").and_then(Value::as_str).map(str::to_string),
            not_brought_in: obj
                .get("notBroughtIn")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
    }
}

/// The words a card says for a run's state (`apps/desktop/src/shared/background.ts`).
pub fn background_state_words(run: &BackgroundRun) -> String {
    match run.state {
        BackgroundState::Working => "Working in the background".to_string(),
        BackgroundState::Blocked => match run.waiting_for.as_deref() {
            Some(what) => format!("Waiting for you ({what}). Open it to answer."),
            None => "Waiting for you. Open it to answer.".to_string(),
        },
        BackgroundState::Done => {
            if run.brought_in.is_some() {
                "Done. Its answer is in the conversation.".to_string()
            } else if let Some(ref why) = run.not_brought_in {
                format!("Done, but {}", lower_first(why))
            } else {
                "Done".to_string()
            }
        }
        BackgroundState::Failed => {
            "Ended with an error. Open it to see what happened.".to_string()
        }
        BackgroundState::Stopped => {
            if run.brought_in.is_some() {
                "Stopped. What it did is in the conversation.".to_string()
            } else {
                "Stopped".to_string()
            }
        }
        BackgroundState::Unknown => "Running in the background".to_string(),
    }
}

fn lower_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
    }
}

/// Raw process output from invoking `claude <args>` in `cwd`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaudeCliOutput {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Request to start a background turn via `claude --bg`.
#[derive(Clone, Debug)]
pub struct StartBackgroundRequest {
    pub prompt: String,
    pub folder: String,
    pub sandbox: BackgroundSandbox,
    pub ask_each: bool,
    pub conversation: Option<String>,
    pub teammate_id: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub account: Option<String>,
}

/// Result of attempting to start a background turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartBackgroundOutcome {
    Started(BackgroundRun),
    Refused { message: String, needs_trust: bool },
}

/// Result of bringing a finished background run back into its conversation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BringInOutcome {
    BroughtIn(String),
    Refused(String),
}

/// Host hooks for `BackgroundRuns` (CLI runner, session lookup, bring-in, clock, change notice).
pub trait BackgroundHost {
    fn run_claude(&self, args: &[String], cwd: &Path, account: Option<&str>) -> ClaudeCliOutput;
    fn session_of(&self, conversation: &str, account: Option<&str>) -> Option<String>;
    fn bring_in(&self, run: &BackgroundRun) -> BringInOutcome;
    fn on_change(&self, _runs: &[BackgroundRun]) {}
    fn now_iso(&self) -> String {
        Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
    }
}

/// The start-time receipt check used by Electron's createSpendRefusal. Call after the
/// route check: reaching a teammate's limit and launching without receipts are different refusals.
pub(crate) fn launch_spend_refusal(
    store: &crate::store::Store,
    policy: &crate::org_policy::PolicyState,
    teammate_id: Option<&str>,
) -> Option<String> {
    use crate::{org_policy, spend, usage_ledger};

    let managed = policy.policy.contains_key("monthlyLimitUsd");
    if teammate_id.is_none() && !managed {
        return None;
    }
    let unreadable = || {
        if managed {
            org_policy::policy_refusal(
                policy,
                "monthlyLimitUsd",
                "The monthly spending limit could not be checked. Ask your administrator to restore access to the spending record.",
            )
        } else {
            "The teammate's monthly spending limit could not be checked. Nothing was started.".into()
        }
    };
    let roster = if teammate_id.is_some() {
        match crate::people::team(store) {
            Ok(roster) => roster,
            Err(_) => return Some(unreadable()),
        }
    } else {
        serde_json::Value::Null
    };
    let teammate = teammate_id.and_then(|id| {
        roster["teammates"]
            .as_array()?
            .iter()
            .find(|entry| entry["teammateId"] == id)
    });
    let personal = teammate
        .and_then(|entry| entry["monthlyLimitUsd"].as_f64())
        .filter(|limit| spend::is_monthly_limit(*limit));
    let limit = org_policy::monthly_limit(Some(policy), personal)?;
    let now = chrono::Local::now();
    let Some(month) = spend::month_of(&now.to_rfc3339()) else {
        return Some(unreadable());
    };
    let turns = if managed {
        usage_ledger::usage_turns_readable(store)
    } else {
        usage_ledger::usage_turns(store)
    };
    let Ok(turns) = turns else {
        return Some(unreadable());
    };
    let owner = teammate.and_then(|entry| entry["teammateId"].as_str());
    let spent = spend::spent_by(&turns, &month, |conversation| {
        owner.is_none_or(|id| roster["missionOwners"][conversation] == id)
    });
    if !spend::limit_reached(Some(&spent), limit) {
        return None;
    }
    let name = teammate
        .and_then(|entry| entry["name"].as_str())
        .unwrap_or("Locust");
    if managed {
        Some(org_policy::policy_refusal(
            policy,
            "monthlyLimitUsd",
            &format!("{name} has reached this month's effective limit of {limit:.2} USD. It starts again next month."),
        ))
    } else {
        Some(spend::limit_refusal(name, Some(&spent), limit, now))
    }
}

/// Manages `claude-background.json`, watches live runs, and brings finished runs back into their conversation.
pub struct BackgroundRuns<H: BackgroundHost> {
    file: PathBuf,
    host: H,
    runs: Option<Vec<BackgroundRun>>,
    watch: BackgroundWatch,
}

impl<H: BackgroundHost> BackgroundRuns<H> {
    pub fn new(file: PathBuf, host: H) -> Self {
        Self {
            file,
            host,
            runs: None,
            watch: BackgroundWatch::new(),
        }
    }

    pub fn watching(&self) -> Vec<String> {
        self.watch.watching()
    }

    fn load(&mut self) -> &mut Vec<BackgroundRun> {
        if self.runs.is_none() {
            let loaded = fs::read_to_string(&self.file)
                .ok()
                .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
                .and_then(|v| v.as_array().cloned())
                .map(|arr| arr.iter().filter_map(BackgroundRun::from_json).collect())
                .unwrap_or_default();
            self.runs = Some(loaded);
        }
        self.runs.as_mut().expect("runs loaded")
    }

    fn save(&self) {
        let Some(ref runs) = self.runs else {
            return;
        };
        let arr: Vec<Value> = runs.iter().map(BackgroundRun::to_json).collect();
        let Ok(body) = serde_json::to_string_pretty(&arr) else {
            return;
        };
        let body = format!("{body}\n");
        if let Some(parent) = self.file.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let temp = self
            .file
            .with_extension(format!("json.{}.tmp", std::process::id()));
        if fs::write(&temp, body).is_ok() {
            let _ = fs::rename(&temp, &self.file);
        }
    }

    /// Picks up where the last launch left off: watches what is open, brings back what ended meanwhile.
    pub fn resume(&mut self) -> Vec<String> {
        let open: Vec<String> = self
            .load()
            .iter()
            .filter(|run| run.brought_in.is_none() && run.not_brought_in.is_none())
            .map(|run| run.id.clone())
            .collect();
        if !open.is_empty() {
            let refs: Vec<&str> = open.iter().map(String::as_str).collect();
            self.watch.watch(&refs);
        }
        open
    }

    pub fn list(&mut self) -> Vec<BackgroundRun> {
        self.load().clone()
    }

    pub fn start(&mut self, mut request: StartBackgroundRequest) -> StartBackgroundOutcome {
        let refuse = |message| StartBackgroundOutcome::Refused { message, needs_trust: false };
        if crate::model_lists::free_routes_only_now() {
            return refuse("This window spends nothing, and a background turn runs Claude Code on your plan.".into());
        }
        // Check the authority this launch actually gets, not a renderer's label.
        let mode = match background_permission_mode(request.sandbox, request.ask_each) {
            "plan" => "plan",
            "default" => "approve-each",
            "acceptEdits" => "accept-edits",
            _ => "auto",
        };
        // Use the same plain-model validation as Claude Cloud. An invalid choice
        // selects the account default; it is not an allowed named model.
        let choice = crate::claude_cloud::CloudChoice { model: request.model.take(), ..Default::default() };
        let args = match crate::claude_cloud::choice_args(Some(&choice)) {
            Ok(args) => args,
            Err(message) => return refuse(message),
        };
        request.model = args.get(1).cloned();
        let policy = crate::org_policy::current();
        if let Some(message) = crate::org_policy::unmetered_refusal(
            Some(&policy), "claude", request.model.as_deref(), mode, false, request.account.is_some(),
        ) {
            return refuse(message);
        }
        let settings_store = || crate::native::store().filter(|store| self.file.parent() == Some(store.root()));
        if let Some(teammate) = request.teammate_id.as_deref() {
            let Some(store) = settings_store() else {
                return refuse("The teammate's spending limit could not be checked. Nothing was started.".into());
            };
            if let Some(message) = launch_spend_refusal(&store, &policy, Some(teammate)) {
                return refuse(message);
            }
        }
        // A policy that permits Auto does not turn on the person's own Settings switch.
        if mode == "auto"
            && !settings_store()
                .and_then(|store| crate::workspace_settings::load(&store).ok())
                .is_some_and(|settings| settings.auto_mode)
        {
            return refuse("Auto is off in Settings, so a background turn cannot use it.".into());
        }
        let resume_session_id = request
            .conversation
            .as_deref()
            .and_then(|conv| self.host.session_of(conv, request.account.as_deref()));
        let args = match background_args(&BackgroundArgsOptions {
            prompt: &request.prompt,
            sandbox: request.sandbox,
            ask_each: request.ask_each,
            model: request.model.as_deref(),
            effort: request.effort.as_deref(),
            resume_session_id: resume_session_id.as_deref(),
        }) {
            Ok(args) => args,
            Err(message) => {
                return StartBackgroundOutcome::Refused {
                    message,
                    needs_trust: false,
                };
            }
        };
        let ran = self
            .host
            .run_claude(&args, Path::new(&request.folder), request.account.as_deref());
        let said = format!("{}\n{}", ran.stdout, ran.stderr);
        if needs_trust(&said) {
            return StartBackgroundOutcome::Refused {
                needs_trust: true,
                message: "Claude Code has not been told to trust this folder, so it will not work here in the background yet. Open Claude Code in this folder once, answer its question, then send again.".to_string(),
            };
        }
        let id = background_id_from(&said);
        if ran.code != 0 || id.is_none() {
            let line = said
                .lines()
                .map(str::trim)
                .find(|part| !part.is_empty())
                .map(|part| &part[..part.len().min(300)]);
            let message = match line {
                Some(l) => format!("Claude Code did not start it in the background: {l}"),
                None => "Claude Code did not start it in the background.".to_string(),
            };
            return StartBackgroundOutcome::Refused {
                message,
                needs_trust: false,
            };
        }
        let id = id.unwrap();
        let run = BackgroundRun {
            account: request.account,
            id: id.clone(),
            session_id: resume_session_id,
            folder: request.folder,
            prompt: request.prompt.trim().to_string(),
            started_at: self.host.now_iso(),
            conversation: request.conversation,
            teammate_id: request.teammate_id,
            model: request.model,
            state: BackgroundState::Working,
            waiting_for: None,
            name: None,
            brought_in: None,
            not_brought_in: None,
        };
        let snapshot = {
            let all = self.load();
            all.push(run.clone());
            all.clone()
        };
        self.save();
        self.host.on_change(&snapshot);
        self.watch.watch(&[id.as_str()]);
        StartBackgroundOutcome::Started(run)
    }

    /// Queries `claude agents --json --all` across active accounts and applies any changes.
    pub fn poll_once(&mut self) -> Result<WatchTickOutcome, String> {
        let (where_folder, accounts) = {
            let all = self.load();
            let folder = all
                .first()
                .map(|r| PathBuf::from(&r.folder))
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
            let mut seen_accounts: Vec<Option<String>> = Vec::new();
            for run in all.iter() {
                if !seen_accounts.contains(&run.account) {
                    seen_accounts.push(run.account.clone());
                }
            }
            if seen_accounts.is_empty() {
                seen_accounts.push(None);
            }
            (folder, seen_accounts)
        };

        self.watch.begin_tick();
        let mut combined = Vec::new();
        let args = vec![
            "agents".to_string(),
            "--json".to_string(),
            "--all".to_string(),
        ];
        for account in &accounts {
            let answer = self
                .host
                .run_claude(&args, &where_folder, account.as_deref());
            let parsed = if answer.code == 0 {
                parse_background_agents(&answer.stdout)
            } else {
                None
            };
            let Some(agents) = parsed else {
                self.watch.finish_tick(None);
                return Err("claude agents gave no answer".to_string());
            };
            combined.extend(agents);
        }

        let outcome = self.watch.finish_tick(Some(&combined));
        if let Some(ref changed) = outcome.changed {
            self.apply_seen(changed);
        }
        Ok(outcome)
    }

    fn apply_seen(&mut self, agents: &[BackgroundAgent]) {
        for agent in agents {
            let updated = {
                let all = self.load();
                if let Some(run) = all.iter_mut().find(|r| r.id == agent.id) {
                    run.state = agent.state;
                    if let Some(ref sid) = agent.session_id {
                        run.session_id = Some(sid.clone());
                    }
                    if let Some(ref name) = agent.name {
                        run.name = Some(name.clone());
                    }
                    if let Some(ref waiting) = agent.waiting_for {
                        run.waiting_for = Some(waiting.clone());
                    } else if agent.state != BackgroundState::Blocked {
                        // A waiting-for that is over is gone, not kept as stale words.
                        run.waiting_for = None;
                    }
                    Some((run.clone(), all.clone()))
                } else {
                    None
                }
            };
            if let Some((run, snapshot)) = updated {
                self.save();
                self.host.on_change(&snapshot);
                if !background_is_live(run.state) {
                    self.finish_run(&run);
                }
            }
        }
    }

    fn finish_run(&mut self, run: &BackgroundRun) {
        if run.brought_in.is_some() || run.not_brought_in.is_some() {
            return;
        }
        let back = self.host.bring_in(run);
        let brought = matches!(back, BringInOutcome::BroughtIn(_));
        let snapshot = {
            let all = self.load();
            if let Some(entry) = all.iter_mut().find(|r| r.id == run.id) {
                match back {
                    BringInOutcome::BroughtIn(mission_id) => {
                        entry.brought_in = Some(mission_id);
                    }
                    BringInOutcome::Refused(reason) => {
                        entry.not_brought_in = Some(reason);
                    }
                }
            }
            all.clone()
        };
        self.save();
        self.host.on_change(&snapshot);
        // Done, and its turn is in the conversation: stop the idle background session so the next `--resume`
        // stays on one session instead of forking a copy.
        if brought && run.state == BackgroundState::Done && is_valid_session_id(&run.id) {
            let stop_args = vec!["stop".to_string(), run.id.clone()];
            let _ = self
                .host
                .run_claude(&stop_args, Path::new(&run.folder), run.account.as_deref());
        }
    }

    /// `claude stop <id>`: Claude Code keeps the conversation; it comes back like any finished run.
    pub fn stop(&mut self, id: &str) -> bool {
        if !is_valid_session_id(id) {
            return false;
        }
        let found = self
            .load()
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| (entry.folder.clone(), entry.account.clone()));
        let Some((folder, account)) = found else {
            return false;
        };
        let args = vec!["stop".to_string(), id.to_string()];
        let ran = self
            .host
            .run_claude(&args, Path::new(&folder), account.as_deref());
        ran.code == 0
    }

    /// Takes an ended run off the list; its turn stays in the conversation.
    pub fn dismiss(&mut self, id: &str) -> bool {
        let snapshot = {
            let all = self.load();
            let Some(pos) = all
                .iter()
                .position(|run| run.id == id && !background_is_live(run.state))
            else {
                return false;
            };
            all.remove(pos);
            all.clone()
        };
        self.save();
        self.host.on_change(&snapshot);
        true
    }

    /// The conversation turn ids brought back from background runs.
    pub fn brought_in_ids(&mut self) -> HashSet<String> {
        self.load()
            .iter()
            .filter_map(|run| run.brought_in.clone())
            .collect()
    }

    pub fn dispose(&mut self) {
        self.watch.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::claude;
    use std::path::PathBuf;
    use std::process::Command;

    fn fixture_script() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("scripts")
            .join("fixtures")
            .join("claude-bg.py")
    }

    #[test]
    // case: reads only background sessions from claude agents --json, and nothing from a malformed answer
    fn reads_only_background_sessions_from_claude_agents_json_and_nothing_from_a_malformed_answer() {
        let json = serde_json::json!([
            {
                "id": "bg-1a2b3c",
                "cwd": "C:/work/pebble",
                "kind": "background",
                "startedAt": "2026-10-06T10:00:00Z",
                "sessionId": "s-1",
                "name": "fix the cart",
                "state": "working"
            },
            {
                "pid": 4321,
                "cwd": "C:/work/pebble",
                "kind": "interactive",
                "startedAt": "2026-10-06T09:00:00Z",
                "sessionId": "s-2",
                "name": "mine",
                "status": "idle"
            },
            {
                "id": "interactive-1",
                "kind": "interactive",
                "state": "working"
            },
            {
                "id": "bg-blocked",
                "kind": "background",
                "state": "blocked",
                "waitingFor": { "tool": "Bash" }
            },
            {
                "id": "bg-new",
                "kind": "background",
                "state": "paused"
            },
            {
                "kind": "background",
                "state": "working"
            }
        ])
        .to_string();

        assert_eq!(
            parse_background_agents(&json),
            Some(vec![
                BackgroundAgent {
                    id: "bg-1a2b3c".into(),
                    cwd: Some("C:/work/pebble".into()),
                    started_at: Some("2026-10-06T10:00:00Z".into()),
                    session_id: Some("s-1".into()),
                    name: Some("fix the cart".into()),
                    state: BackgroundState::Working,
                    waiting_for: None,
                },
                BackgroundAgent {
                    id: "bg-blocked".into(),
                    cwd: None,
                    started_at: None,
                    session_id: None,
                    name: None,
                    state: BackgroundState::Blocked,
                    waiting_for: Some("Bash".into()),
                },
                // A state this build has not met is said as unknown, never guessed.
                BackgroundAgent {
                    id: "bg-new".into(),
                    cwd: None,
                    started_at: None,
                    session_id: None,
                    name: None,
                    state: BackgroundState::Unknown,
                    waiting_for: None,
                },
            ])
        );

        // Not a list: no answer, never "none are running".
        assert_eq!(parse_background_agents(""), None);
        assert_eq!(parse_background_agents(r#"{"id":"x"}"#), None);

        // As measured: epoch milliseconds, and what a blocked one waits for.
        let measured = serde_json::json!([{
            "pid": 21492,
            "id": "ad39859a",
            "kind": "background",
            "startedAt": 1791339613341_i64,
            "sessionId": "ad39859a-5d0f",
            "name": "create hello.txt file",
            "status": "waiting",
            "waitingFor": "permission prompt",
            "state": "blocked"
        }])
        .to_string();
        assert_eq!(
            parse_background_agents(&measured),
            Some(vec![BackgroundAgent {
                id: "ad39859a".into(),
                session_id: Some("ad39859a-5d0f".into()),
                cwd: None,
                name: Some("create hello.txt file".into()),
                started_at: Some("2026-10-07T02:20:13.341Z".into()),
                state: BackgroundState::Blocked,
                waiting_for: Some("permission prompt".into()),
            }])
        );

        assert_eq!(
            background_id_from(
                "Starting background service…\nbackgrounded · ad39859a\n  claude agents  list sessions"
            ),
            Some("ad39859a".into())
        );
    }

    #[test]
    // case: says a blocked session waits for the person, and that Locust does not answer for them
    fn says_a_blocked_session_waits_for_the_person_and_that_locust_does_not_answer_for_them() {
        assert_eq!(
            background_sentence(BackgroundState::Blocked, Some("Bash")),
            "Waiting for you: Bash. Open it to answer; Locust does not answer for you."
        );
        assert!(background_is_live(BackgroundState::Blocked));
        assert!(!background_is_live(BackgroundState::Done));
    }

    #[test]
    // case: starts claude --bg on the conversation, the mode said as Claude Code's, the message never taken for a flag
    fn starts_claude_bg_on_the_conversation_the_mode_said_as_claude_codes_the_message_never_taken_for_a_flag() {
        assert_eq!(
            background_args(&BackgroundArgsOptions {
                prompt: "--help me",
                sandbox: BackgroundSandbox::WorkspaceWrite,
                ask_each: false,
                model: Some("haiku"),
                effort: None,
                resume_session_id: Some("s-1"),
            }),
            Ok(vec![
                "--bg".into(),
                "--resume".into(),
                "s-1".into(),
                "--permission-mode".into(),
                "acceptEdits".into(),
                "--model".into(),
                "haiku".into(),
                "--".into(),
                "--help me".into(),
            ])
        );
        assert_eq!(
            background_args(&BackgroundArgsOptions {
                prompt: "Read it.",
                sandbox: BackgroundSandbox::ReadOnly,
                ask_each: false,
                model: None,
                effort: None,
                resume_session_id: None,
            }),
            Ok(vec![
                "--bg".into(),
                "--permission-mode".into(),
                "plan".into(),
                "--".into(),
                "Read it.".into(),
            ])
        );
        assert_eq!(
            background_permission_mode(BackgroundSandbox::WorkspaceWrite, true),
            "default"
        );
        assert_eq!(
            background_permission_mode(BackgroundSandbox::FullAccess, false),
            "bypassPermissions"
        );
        assert!(background_args(&BackgroundArgsOptions {
            prompt: "  ",
            sandbox: BackgroundSandbox::ReadOnly,
            ask_each: false,
            model: None,
            effort: None,
            resume_session_id: None,
        })
        .is_err());
    }

    #[test]
    // case: knows the folder must be trusted in Claude Code first
    fn knows_the_folder_must_be_trusted_in_claude_code_first() {
        assert!(needs_trust(
            "Workspace not trusted. Run `claude` in C:\\x once and accept the trust prompt, then retry."
        ));
        assert!(!needs_trust("bg-1a2b3c"));
    }

    #[test]
    // case: polls only while one of its own is live, and says one Claude Code no longer lists is over
    fn polls_only_while_one_of_its_own_is_live_and_says_one_claude_code_no_longer_lists_is_over() {
        let mut watch = BackgroundWatch::new();
        let mut changes: Vec<Vec<BackgroundAgent>> = Vec::new();
        assert!(!watch.timer_scheduled());

        assert!(watch.watch(&["a"]));
        assert!(watch.timer_scheduled());

        let listed = vec![
            BackgroundAgent {
                id: "a".into(),
                session_id: None,
                cwd: None,
                name: None,
                started_at: None,
                state: BackgroundState::Working,
                waiting_for: None,
            },
            BackgroundAgent {
                id: "someone-else".into(),
                session_id: None,
                cwd: None,
                name: None,
                started_at: None,
                state: BackgroundState::Working,
                waiting_for: None,
            },
        ];
        watch.begin_tick();
        let out = watch.finish_tick(Some(&listed));
        if let Some(c) = out.changed {
            changes.push(c);
        }
        assert_eq!(
            changes.last().unwrap(),
            &vec![BackgroundAgent {
                id: "a".into(),
                session_id: None,
                cwd: None,
                name: None,
                started_at: None,
                state: BackgroundState::Working,
                waiting_for: None,
            }]
        );
        assert!(out.schedule_next);
        assert!(watch.timer_scheduled());

        let listed = vec![BackgroundAgent {
            id: "a".into(),
            session_id: None,
            cwd: None,
            name: None,
            started_at: None,
            state: BackgroundState::Blocked,
            waiting_for: Some("Bash".into()),
        }];
        watch.begin_tick();
        let out = watch.finish_tick(Some(&listed));
        if let Some(c) = out.changed {
            changes.push(c);
        }
        assert_eq!(
            changes.last().unwrap(),
            &vec![BackgroundAgent {
                id: "a".into(),
                session_id: None,
                cwd: None,
                name: None,
                started_at: None,
                state: BackgroundState::Blocked,
                waiting_for: Some("Bash".into()),
            }]
        );

        // Unchanged: no second notice.
        watch.begin_tick();
        let out = watch.finish_tick(Some(&listed));
        if let Some(c) = out.changed {
            changes.push(c);
        }
        assert_eq!(changes.len(), 2);

        // Missing from claude agents --json: marked stopped, nothing left live, no timer left.
        watch.begin_tick();
        let out = watch.finish_tick(Some(&[]));
        if let Some(c) = out.changed {
            changes.push(c);
        }
        assert_eq!(
            changes.last().unwrap(),
            &vec![BackgroundAgent {
                id: "a".into(),
                session_id: None,
                cwd: None,
                name: None,
                started_at: None,
                state: BackgroundState::Stopped,
                waiting_for: None,
            }]
        );
        assert!(!out.schedule_next);
        assert!(!watch.timer_scheduled());
        assert!(watch.watching().is_empty());
    }

    #[test]
    // case: keeps one poll loop when a second watch comes while a look is in flight (2026-10-10 sweep)
    fn keeps_one_poll_loop_when_a_second_watch_comes_while_a_look_is_in_flight() {
        let mut watch = BackgroundWatch::new();
        assert!(watch.watch(&["a"]));
        watch.begin_tick();
        // The look is out; a second id arrives and must NOT schedule a second poll loop.
        assert!(!watch.watch(&["b"]));
        assert!(!watch.timer_scheduled());

        let out = watch.finish_tick(Some(&[
            BackgroundAgent {
                id: "a".into(),
                session_id: None,
                cwd: None,
                name: None,
                started_at: None,
                state: BackgroundState::Working,
                waiting_for: None,
            },
            BackgroundAgent {
                id: "b".into(),
                session_id: None,
                cwd: None,
                name: None,
                started_at: None,
                state: BackgroundState::Working,
                waiting_for: None,
            },
        ]));
        assert!(out.schedule_next);
        assert!(watch.timer_scheduled());
        assert_eq!(watch.watching(), vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn runtime_claude_background_caller_runs_fake_script_and_never_answers_blocked_session() {
        let script = fixture_script();
        assert!(script.exists(), "fixture script missing: {}", script.display());
        let tmp = std::env::temp_dir().join(format!(
            "locust-bg-row1-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&tmp);
        let calls_file = tmp.join("calls.jsonl");
        let _ = std::fs::remove_file(&calls_file);

        // 1. Start via runtime::claude::background_command_with_program (caller path).
        let opts = BackgroundArgsOptions {
            prompt: "--help me",
            sandbox: BackgroundSandbox::WorkspaceWrite,
            ask_each: false,
            model: Some("haiku"),
            effort: None,
            resume_session_id: Some("s-1"),
        };
        let mut cmd = Command::new("python3");
        cmd.arg(&script);
        cmd.env("CLAUDE_BG_CALLS_FILE", &calls_file);
        let started = claude::start_background_with(cmd, &tmp, &opts).expect("start ok");
        assert_eq!(started.id, "ad39859a");
        assert_eq!(
            started.sentence,
            "Working in the background."
        );

        // 2. Untrusted folder refusal through the same caller.
        let mut untrusted_cmd = Command::new("python3");
        untrusted_cmd.arg(&script);
        untrusted_cmd.env("CLAUDE_BG_UNTRUSTED", "1");
        let err = claude::start_background_with(untrusted_cmd, &tmp, &opts).unwrap_err();
        assert!(err.needs_trust, "expected needs_trust refusal, got: {err:?}");

        // 3. Watch & poll a blocked session via runtime::claude::poll_background_with.
        let mut watch = BackgroundWatch::new();
        assert!(watch.watch(&["ad39859a"]));
        let mut list_cmd = Command::new("python3");
        list_cmd.arg(&script);
        list_cmd.env(
            "CLAUDE_BG_AGENTS_JSON",
            r#"[{"id":"ad39859a","kind":"background","state":"blocked","waitingFor":"permission prompt"}]"#,
        );
        let outcome = claude::poll_background_with(&mut watch, list_cmd, &tmp).expect("poll ok");
        let seen = outcome.changed.expect("changed");
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].state, BackgroundState::Blocked);
        assert_eq!(
            background_sentence(seen[0].state, seen[0].waiting_for.as_deref()),
            "Waiting for you: permission prompt. Open it to answer; Locust does not answer for you."
        );
        assert!(outcome.schedule_next);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ---- Row 2: a-background-turn-comes-back.test.ts ----

    use std::cell::RefCell;

    struct FakeHost {
        agents: RefCell<String>,
        start_says: RefCell<Option<String>>,
        calls: RefCell<Vec<Vec<String>>>,
        session: Option<(String, String)>,
        brought: RefCell<Vec<BackgroundRun>>,
        bring_in_id: String,
        fixed_now: String,
    }

    impl FakeHost {
        fn new(bring_in_id: &str) -> Self {
            Self {
                agents: RefCell::new("[]".to_string()),
                start_says: RefCell::new(None),
                calls: RefCell::new(Vec::new()),
                session: None,
                brought: RefCell::new(Vec::new()),
                bring_in_id: bring_in_id.to_string(),
                fixed_now: "2026-10-06T22:00:00.000Z".to_string(),
            }
        }
    }

    impl BackgroundHost for &FakeHost {
        fn run_claude(&self, args: &[String], _cwd: &Path, _account: Option<&str>) -> ClaudeCliOutput {
            self.calls.borrow_mut().push(args.to_vec());
            match args.first().map(String::as_str) {
                Some("--bg") => ClaudeCliOutput {
                    code: 0,
                    stdout: self.start_says.borrow().clone().unwrap_or_else(|| {
                        "Starting background service…\nbackgrounded · ad39859a\n".to_string()
                    }),
                    stderr: String::new(),
                },
                Some("agents") => ClaudeCliOutput {
                    code: 0,
                    stdout: self.agents.borrow().clone(),
                    stderr: String::new(),
                },
                Some("stop") => ClaudeCliOutput {
                    code: 0,
                    stdout: format!("stopped {}", args.get(1).map(String::as_str).unwrap_or("")),
                    stderr: String::new(),
                },
                _ => ClaudeCliOutput {
                    code: 1,
                    stdout: String::new(),
                    stderr: "unexpected".to_string(),
                },
            }
        }

        fn session_of(&self, conversation: &str, _account: Option<&str>) -> Option<String> {
            self.session
                .as_ref()
                .filter(|(c, _)| c == conversation)
                .map(|(_, s)| s.clone())
        }

        fn bring_in(&self, run: &BackgroundRun) -> BringInOutcome {
            self.brought.borrow_mut().push(run.clone());
            BringInOutcome::BroughtIn(self.bring_in_id.clone())
        }

        fn now_iso(&self) -> String {
            self.fixed_now.clone()
        }
    }

    fn temp_bg_file(tag: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "locust-bg-row2-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        let file = dir.join("claude-background.json");
        (dir, file)
    }

    #[test]
    // case: starts on the conversation's own session, is remembered, and comes back into it when it is done
    fn starts_on_the_conversations_own_session_is_remembered_and_comes_back_into_it_when_it_is_done() {
        let (dir, path) = temp_bg_file("comes-back");
        let mut host = FakeHost::new("mission_back");
        host.session = Some((
            "mission_a".into(),
            "ad39859a-5d0f-4439-8dc5-2f147a5b4cf6".into(),
        ));
        let mut runs = BackgroundRuns::new(path.clone(), &host);

        let started = runs.start(StartBackgroundRequest {
            prompt: "Fix the cart total.".into(),
            folder: "C:/work/pebble".into(),
            sandbox: BackgroundSandbox::WorkspaceWrite,
            ask_each: false,
            conversation: Some("mission_a".into()),
            teammate_id: None,
            model: Some("haiku".into()),
            effort: None,
            account: None,
        });
        let StartBackgroundOutcome::Started(run) = started else {
            panic!("expected Started, got {started:?}");
        };
        assert_eq!(run.id, "ad39859a");
        assert_eq!(run.state, BackgroundState::Working);
        assert_eq!(run.conversation.as_deref(), Some("mission_a"));
        assert_eq!(
            run.session_id.as_deref(),
            Some("ad39859a-5d0f-4439-8dc5-2f147a5b4cf6")
        );
        assert_eq!(
            host.calls.borrow()[0],
            vec![
                "--bg",
                "--resume",
                "ad39859a-5d0f-4439-8dc5-2f147a5b4cf6",
                "--permission-mode",
                "acceptEdits",
                "--model",
                "haiku",
                "--",
                "Fix the cart total.",
            ]
        );
        assert_eq!(runs.watching(), vec!["ad39859a".to_string()]);

        // Waiting for the person: said, and nothing brought in.
        *host.agents.borrow_mut() = serde_json::json!([{
            "id": "ad39859a",
            "kind": "background",
            "state": "blocked",
            "waitingFor": "permission prompt",
            "sessionId": "ad39859a-5d0f-4439-8dc5-2f147a5b4cf6"
        }])
        .to_string();
        runs.poll_once().expect("poll blocked");
        let list = runs.list();
        assert_eq!(list[0].state, BackgroundState::Blocked);
        assert_eq!(list[0].waiting_for.as_deref(), Some("permission prompt"));
        assert!(host.brought.borrow().is_empty());

        // Finished: brought into the conversation, waitingFor cleared, idle session stopped.
        *host.agents.borrow_mut() = serde_json::json!([{
            "id": "ad39859a",
            "kind": "background",
            "state": "done",
            "sessionId": "ad39859a-5d0f-4439-8dc5-2f147a5b4cf6"
        }])
        .to_string();
        runs.poll_once().expect("poll done");
        assert_eq!(
            host.brought
                .borrow()
                .iter()
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>(),
            vec!["ad39859a"]
        );
        let raw = fs::read_to_string(&path).expect("saved json");
        let saved: Vec<Value> = serde_json::from_str(&raw).expect("valid json");
        assert_eq!(saved[0]["state"], "done");
        assert_eq!(saved[0]["broughtIn"], "mission_back");
        assert!(saved[0].get("waitingFor").is_none());
        assert_eq!(
            runs.brought_in_ids(),
            HashSet::from(["mission_back".to_string()])
        );
        assert_eq!(
            host.calls.borrow().last().unwrap(),
            &vec!["stop".to_string(), "ad39859a".to_string()]
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    // case: one that ended while Locust was closed is brought back the next time it opens
    fn one_that_ended_while_locust_was_closed_is_brought_back_the_next_time_it_opens() {
        let (dir, path) = temp_bg_file("relaunch");
        let first_host = FakeHost::new("x");
        let mut first = BackgroundRuns::new(path.clone(), &first_host);
        let started = first.start(StartBackgroundRequest {
            prompt: "Write the docs.".into(),
            folder: "C:/work/pebble".into(),
            sandbox: BackgroundSandbox::WorkspaceWrite,
            ask_each: false,
            conversation: None,
            teammate_id: None,
            model: None,
            effort: None,
            account: None,
        });
        assert!(matches!(started, StartBackgroundOutcome::Started(_)));

        // Locust quits; Claude Code finishes; Locust opens again.
        let second_host = FakeHost::new("mission_new");
        let mut again = BackgroundRuns::new(path.clone(), &second_host);
        let resumed = again.resume();
        assert_eq!(resumed, vec!["ad39859a".to_string()]);
        assert_eq!(again.watching(), vec!["ad39859a".to_string()]);

        *second_host.agents.borrow_mut() = serde_json::json!([{
            "id": "ad39859a",
            "kind": "background",
            "state": "done",
            "sessionId": "ad39859a-1111"
        }])
        .to_string();
        again.poll_once().expect("poll after resume");
        assert_eq!(
            second_host
                .brought
                .borrow()
                .iter()
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>(),
            vec!["ad39859a"]
        );
        let list = again.list();
        assert_eq!(list[0].brought_in.as_deref(), Some("mission_new"));
        assert_eq!(list[0].session_id.as_deref(), Some("ad39859a-1111"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    // case: an untrusted folder is said as such, and nothing is kept
    fn an_untrusted_folder_is_said_as_such_and_nothing_is_kept() {
        let (dir, path) = temp_bg_file("untrusted");
        let host = FakeHost::new("x");
        *host.start_says.borrow_mut() = Some(
            "Workspace not trusted. Run `claude` in C:\\work\\pebble once and accept the trust prompt, then retry."
                .to_string(),
        );
        let mut runs = BackgroundRuns::new(path.clone(), &host);
        let started = runs.start(StartBackgroundRequest {
            prompt: "Go.".into(),
            folder: "C:/work/pebble".into(),
            sandbox: BackgroundSandbox::ReadOnly,
            ask_each: false,
            conversation: None,
            teammate_id: None,
            model: None,
            effort: None,
            account: None,
        });
        assert!(matches!(
            started,
            StartBackgroundOutcome::Refused {
                needs_trust: true,
                ..
            }
        ));
        assert!(runs.list().is_empty());
        assert!(!path.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    // case: stops only an id of its own, and a finished run leaves the list while its turn stays
    fn stops_only_an_id_of_its_own_and_a_finished_run_leaves_the_list_while_its_turn_stays() {
        let (dir, path) = temp_bg_file("stop-dismiss");
        let host = FakeHost::new("mission_back");
        let mut runs = BackgroundRuns::new(path.clone(), &host);
        runs.start(StartBackgroundRequest {
            prompt: "Go.".into(),
            folder: "C:/work/pebble".into(),
            sandbox: BackgroundSandbox::ReadOnly,
            ask_each: false,
            conversation: None,
            teammate_id: None,
            model: None,
            effort: None,
            account: None,
        });
        assert!(!is_valid_session_id("--all"));
        assert!(is_valid_session_id("ad39859a"));
        // Invalid ids persisted in claude-background.json are ignored on load.
        let bad_path = dir.join("bad-id.json");
        fs::write(
            &bad_path,
            r#"[{"id":"--all","folder":"C:/work/pebble","prompt":"bad","startedAt":"2026-10-06T22:00:00.000Z","state":"working"}]"#,
        )
        .expect("write bad json");
        let mut bad_runs = BackgroundRuns::new(bad_path, &host);
        assert!(bad_runs.list().is_empty());
        assert!(!runs.stop("--all"));
        // Even if an invalid id were present in memory, stop() refuses it before calling claude.
        runs.load().push(BackgroundRun {
            account: None,
            id: "--all".into(),
            session_id: None,
            folder: "C:/work/pebble".into(),
            prompt: "bad".into(),
            started_at: "2026-10-06T22:00:00.000Z".into(),
            conversation: None,
            teammate_id: None,
            model: None,
            state: BackgroundState::Working,
            waiting_for: None,
            name: None,
            brought_in: None,
            not_brought_in: None,
        });
        assert!(!runs.stop("--all"));
        runs.load().retain(|r| r.id != "--all");
        assert!(runs.stop("ad39859a"));
        assert_eq!(
            host.calls.borrow().last().unwrap(),
            &vec!["stop".to_string(), "ad39859a".to_string()]
        );

        // Still live: not dismissed.
        assert!(!runs.dismiss("ad39859a"));
        assert_eq!(runs.list().len(), 1);

        // Stopped: now dismiss removes it from the list.
        *host.agents.borrow_mut() = serde_json::json!([{
            "id": "ad39859a",
            "kind": "background",
            "state": "stopped"
        }])
        .to_string();
        runs.poll_once().expect("poll stopped");
        assert_eq!(runs.list()[0].state, BackgroundState::Stopped);
        assert!(runs.dismiss("ad39859a"));
        assert!(runs.list().is_empty());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn native_background_runs_caller_tracks_and_persists_with_fake_claude_script() {
        let (dir, _unused) = temp_bg_file("native-caller");
        let store = crate::store::Store::new(dir.join("native"), dir.join("electron"))
            .expect("temp store");
        let script = fixture_script();
        let agents_file = dir.join("agents.json");
        let calls_file = dir.join("calls.jsonl");
        fs::write(&agents_file, "[]").expect("agents file");

        let mut runs = crate::native::background_runs_for_with_program(
            &store,
            "python3",
            vec![script.to_string_lossy().into_owned()],
            vec![
                (
                    "CLAUDE_BG_AGENTS_FILE".to_string(),
                    agents_file.to_string_lossy().into_owned(),
                ),
                (
                    "CLAUDE_BG_CALLS_FILE".to_string(),
                    calls_file.to_string_lossy().into_owned(),
                ),
            ],
        );
        let started = runs.start(StartBackgroundRequest {
            prompt: "Check the build.".into(),
            folder: dir.to_string_lossy().into_owned(),
            sandbox: BackgroundSandbox::ReadOnly,
            ask_each: false,
            conversation: None,
            teammate_id: None,
            model: None,
            effort: None,
            account: None,
        });
        let StartBackgroundOutcome::Started(run) = started else {
            panic!("expected Started via native caller, got {started:?}");
        };
        assert_eq!(run.id, "ad39859a");
        assert!(store.root().join("claude-background.json").exists());

        // Poll blocked state via fake script.
        fs::write(
            &agents_file,
            r#"[{"id":"ad39859a","kind":"background","state":"blocked","waitingFor":"permission prompt"}]"#,
        )
        .expect("write blocked");
        runs.poll_once().expect("poll blocked");
        let list = runs.list();
        assert_eq!(list[0].state, BackgroundState::Blocked);
        assert_eq!(
            background_state_words(&list[0]),
            "Waiting for you (permission prompt). Open it to answer."
        );

        // Stop via fake script.
        assert!(runs.stop("ad39859a"));
        assert!(!runs.stop("--all"));
        runs.dispose();

        // Verify the exact argv handed to the fake claude script for --bg, agents --json, and stop.
        let recorded: Vec<Vec<String>> = fs::read_to_string(&calls_file)
            .expect("calls log")
            .lines()
            .map(|line| serde_json::from_str(line).expect("json argv"))
            .collect();
        assert_eq!(
            recorded,
            vec![
                vec![
                    "--bg".to_string(),
                    "--permission-mode".to_string(),
                    "plan".to_string(),
                    "--".to_string(),
                    "Check the build.".to_string(),
                ],
                vec![
                    "agents".to_string(),
                    "--json".to_string(),
                    "--all".to_string(),
                ],
                vec!["stop".to_string(), "ad39859a".to_string()],
            ]
        );

        let _ = fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
#[path = "claude_background_policy_tests.rs"]
mod claude_background_policy_tests;

#[cfg(test)]
#[path = "claude_background_launch_guard_tests.rs"]
mod launch_guard_tests;

//! Claude Code cloud sessions (`apps/desktop/src/main/claude-cloud.ts` and
//! `apps/desktop/src/shared/claude-cloud.ts`): starting a cloud session with
//! optional model, effort and per-folder organization environment ID, and
//! confirming follow-up sends strictly from Claude Code's JSON receipt on stdout.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use chrono::Utc;
use regex::Regex;
use serde_json::{Value, json};

pub const TASK_VARIABLE: &str = "LOCUST_CLAUDE_CLOUD_TASK";
pub const ASKS_THE_PERSON: &str =
    "Quick safety check|trust this folder|Do you trust|Workspace trust not yet accepted";
const MAX_KEPT: usize = 30;
pub const UNCONFIRMED_SEND_MESSAGE: &str = "Locust could not confirm that Claude Code sent that. Look at the session on claude.ai before sending it again, so it does not arrive twice.";

fn env_id_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^ccpool_[A-Za-z0-9_-]{1,80}$").expect("env id regex"))
}

fn model_name_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._\-\[\]]{0,79}$").expect("model regex"))
}

fn session_id_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^session_[A-Za-z0-9]{8,64}$").expect("session id regex"))
}

fn worktree_name_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^locust-cloud-[a-f0-9]{1,12}$").expect("worktree regex"))
}

fn cloud_view_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"https://claude\.ai/code/(session_[A-Za-z0-9]{8,64})[^\s]*")
            .expect("cloud view regex")
    })
}

fn created_title_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"Created cloud session:[ \t]*(.+)").expect("title regex"))
}

fn asks_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(&format!("(?i){ASKS_THE_PERSON}")).expect("asks regex"))
}

fn error_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"Error:[ \t]*(.+)").expect("error regex"))
}

fn ansi_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\x1b(?:\[[0-9;?]*[ -/]*[@-~]|\][^\x07\x1b]*(?:\x07|\x1b\\)|[@-Z\\-_])")
            .expect("ansi regex")
    })
}

fn multi_ws_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\s{2,}").expect("ws regex"))
}

const EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];

/// W6: only the documented organization's environment id, never a shell word.
pub fn is_cloud_environment_id(value: &str) -> bool {
    env_id_re().is_match(value)
}

/// The task as one line Windows' console passes through untouched: no double quote, no line breaks.
pub fn cloud_task_text(prompt: &str) -> String {
    let no_quotes = prompt.replace('"', "'");
    let no_breaks: String = no_quotes
        .chars()
        .map(|c| if matches!(c, '\r' | '\n' | '\t') { ' ' } else { c })
        .collect();
    let collapsed = multi_ws_re().replace_all(&no_breaks, " ");
    collapsed.trim().chars().take(4_000).collect()
}

pub fn plain_terminal_text(drawn: &str) -> String {
    let stripped = ansi_re().replace_all(drawn, "");
    stripped.replace("\r\n", "\n").replace('\r', "")
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CloudChoice {
    pub model: Option<String>,
    pub effort: Option<String>,
    pub environment: Option<String>,
}

pub fn choice_args(choice: Option<&CloudChoice>) -> Result<Vec<String>, String> {
    let Some(c) = choice else {
        return Ok(Vec::new());
    };
    if let Some(env) = c.environment.as_deref()
        && !is_cloud_environment_id(env)
    {
        return Err(
            "Use an environment ID beginning ccpool_ with 1 to 80 letters, digits, underscores or hyphens, or leave it empty."
                .into(),
        );
    }
    let mut out = Vec::new();
    if let Some(model) = c.model.as_deref()
        && model != "account-default"
        && model_name_re().is_match(model)
    {
        out.push("--model".into());
        out.push(model.into());
    }
    if let Some(effort) = c.effort.as_deref()
        && EFFORTS.contains(&effort)
    {
        out.push("--effort".into());
        out.push(effort.into());
    }
    if let Some(env) = c.environment.as_deref() {
        out.push("--environment".into());
        out.push(env.into());
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudAction {
    Start,
    Home,
    Send,
    Read,
}

#[derive(Debug, Clone, Default)]
pub struct WindowsCommandOptions<'a> {
    pub session: Option<&'a str>,
    pub stay: Option<bool>,
    pub choice: Option<&'a CloudChoice>,
    pub worktree: Option<&'a str>,
}

pub fn windows_command_line(
    path: &str,
    prefix: &[&str],
    action: CloudAction,
    options: &WindowsCommandOptions<'_>,
) -> Result<String, String> {
    let mut head_parts = Vec::with_capacity(1 + prefix.len());
    head_parts.push(format!("\"{path}\""));
    for part in prefix {
        head_parts.push(format!("\"{part}\""));
    }
    let head = head_parts.join(" ");
    let session = options
        .session
        .filter(|s| session_id_re().is_match(s));
    let worktree = options
        .worktree
        .filter(|w| worktree_name_re().is_match(w));
    let tail = match action {
        CloudAction::Start => {
            let mut parts = choice_args(options.choice)?;
            parts.push("--cloud".into());
            parts.push(format!("\"%{TASK_VARIABLE}%\""));
            parts.join(" ")
        }
        CloudAction::Send => format!(
            "-p --cloud {} --output-format json \"%{TASK_VARIABLE}%\"",
            session.unwrap_or("")
        ),
        CloudAction::Read => format!(
            "--teleport {} --worktree {}",
            session.unwrap_or(""),
            worktree.unwrap_or("")
        ),
        CloudAction::Home => match session {
            Some(s) => format!("--teleport {s}"),
            None => "--teleport".into(),
        },
    };
    let flag = if options.stay == Some(false) { "/c" } else { "/k" };
    Ok(format!("/d {flag} \"{head} {tail}\""))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudStartReading {
    Created {
        session_id: String,
        url: String,
        title: Option<String>,
        note: Option<String>,
    },
    Asks,
    Failed {
        message: Option<String>,
    },
}

pub fn read_cloud_start(drawn: &str) -> CloudStartReading {
    let text = plain_terminal_text(drawn);
    if let Some(caps) = cloud_view_re().captures(&text) {
        let session_id = caps[1].to_string();
        let url = format!("https://claude.ai/code/{session_id}");
        let title = created_title_re()
            .captures(&text)
            .map(|c| c[1].trim().chars().take(120).collect::<String>())
            .filter(|s| !s.is_empty());
        let lines: Vec<&str> = text.lines().collect();
        let note = lines
            .iter()
            .position(|line| line.contains("Resume with:"))
            .map(|idx| {
                let joined = lines[idx + 1..].join(" ");
                let collapsed = multi_ws_re().replace_all(&joined, " ");
                collapsed.trim().chars().take(400).collect::<String>()
            })
            .filter(|s| !s.is_empty());
        return CloudStartReading::Created {
            session_id,
            url,
            title,
            note,
        };
    }
    if asks_re().is_match(&text) {
        return CloudStartReading::Asks;
    }
    let message = error_line_re()
        .captures(&text)
        .map(|c| c[1].trim().chars().take(300).collect::<String>())
        .filter(|s| !s.is_empty());
    CloudStartReading::Failed { message }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudPlatform {
    Win32,
    Darwin,
    Linux,
}

#[derive(Debug, Clone)]
pub struct ClaudeLaunch {
    pub discovered_path: String,
    pub executable_path: String,
    pub prefix_args: Vec<String>,
    pub env: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct ExecOptions {
    pub cwd: String,
    pub env: BTreeMap<String, String>,
    pub verbatim: bool,
}

#[derive(Debug, Clone)]
pub struct ExecReply {
    pub code: i32,
    pub output: String,
    pub stdout: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PseudoTerminalRun {
    pub line: String,
    pub cwd: String,
    pub env: BTreeMap<String, String>,
    pub seconds: u64,
    pub stop_when: String,
}

#[derive(Debug, Clone)]
pub enum PseudoTerminalResult {
    Ok { drawn: String },
    Err,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeCloudSession {
    pub id: String,
    pub started_at: String,
    pub prompt: String,
    pub folder: String,
    pub teammate_id: Option<String>,
    pub session_id: Option<String>,
    pub url: Option<String>,
    pub title: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudSendResult {
    Ok,
    Err { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeCloudAnswer {
    Ok { session: ClaudeCloudSession },
    Err { message: String },
}

pub struct ClaudeCloud<'a> {
    store_path: PathBuf,
    platform: CloudPlatform,
    discover: Box<dyn Fn() -> Option<ClaudeLaunch> + 'a>,
    exec: Option<Box<dyn Fn(&str, &[String], &ExecOptions) -> ExecReply + 'a>>,
    terminal: Option<Box<dyn Fn(&PseudoTerminalRun) -> PseudoTerminalResult + 'a>>,
    run_window: Option<Box<dyn Fn(&str, &[String], &Path) -> Result<(), String> + 'a>>,
    now: Box<dyn Fn() -> String + 'a>,
    env_lock: Mutex<()>,
}

impl<'a> ClaudeCloud<'a> {
    pub fn new(
        store_path: impl Into<PathBuf>,
        platform: CloudPlatform,
        discover: impl Fn() -> Option<ClaudeLaunch> + 'a,
    ) -> Self {
        Self {
            store_path: store_path.into(),
            platform,
            discover: Box::new(discover),
            exec: None,
            terminal: None,
            run_window: None,
            now: Box::new(|| Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
            env_lock: Mutex::new(()),
        }
    }

    pub fn with_exec(
        mut self,
        exec: impl Fn(&str, &[String], &ExecOptions) -> ExecReply + 'a,
    ) -> Self {
        self.exec = Some(Box::new(exec));
        self
    }

    pub fn with_terminal(
        mut self,
        terminal: impl Fn(&PseudoTerminalRun) -> PseudoTerminalResult + 'a,
    ) -> Self {
        self.terminal = Some(Box::new(terminal));
        self
    }

    pub fn with_run_window(
        mut self,
        run_window: impl Fn(&str, &[String], &Path) -> Result<(), String> + 'a,
    ) -> Self {
        self.run_window = Some(Box::new(run_window));
        self
    }

    pub fn with_now(mut self, now: impl Fn() -> String + 'a) -> Self {
        self.now = Box::new(now);
        self
    }

    fn environment_path(&self) -> PathBuf {
        let mut s = self.store_path.as_os_str().to_os_string();
        s.push(".environments.json");
        PathBuf::from(s)
    }

    fn read_sessions(&self) -> Vec<ClaudeCloudSession> {
        let Ok(text) = fs::read_to_string(&self.store_path) else {
            return Vec::new();
        };
        let Ok(parsed) = serde_json::from_str::<Value>(&text) else {
            return Vec::new();
        };
        let Some(items) = parsed.get("sessions").and_then(Value::as_array) else {
            return Vec::new();
        };
        items.iter().filter_map(session_from_json).collect()
    }

    fn write_sessions(&self, sessions: &[ClaudeCloudSession]) -> Result<(), String> {
        if let Some(parent) = self.store_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let body = serde_json::to_string_pretty(&json!({
            "sessions": sessions.iter().map(session_to_json).collect::<Vec<_>>()
        }))
        .map_err(|e| e.to_string())?;
        atomic_write(&self.store_path, &body)
    }

    fn read_environments(&self) -> Vec<(String, String)> {
        let Ok(text) = fs::read_to_string(self.environment_path()) else {
            return Vec::new();
        };
        let Ok(Value::Array(items)) = serde_json::from_str::<Value>(&text) else {
            return Vec::new();
        };
        items
            .iter()
            .filter_map(|entry| {
                let obj = entry.as_object()?;
                let folder = obj.get("folder")?.as_str()?;
                let id = obj.get("id")?.as_str()?;
                if folder.is_empty() || folder.len() > 4_000 || !is_cloud_environment_id(id) {
                    return None;
                }
                Some((folder.to_string(), id.to_string()))
            })
            .take(MAX_KEPT)
            .collect()
    }

    fn remember_environment(&self, folder: &str, id: Option<&str>) -> Result<(), String> {
        let _guard = self.env_lock.lock().unwrap_or_else(|e| e.into_inner());
        let saved = self.read_environments();
        let previous: Vec<(String, String)> = saved
            .iter()
            .filter(|(f, _)| f != folder)
            .cloned()
            .collect();
        if id.is_none() && previous.len() == saved.len() {
            return Ok(());
        }
        let mut next = Vec::with_capacity(previous.len() + 1);
        if let Some(env_id) = id {
            next.push((folder.to_string(), env_id.to_string()));
        }
        next.extend(previous);
        next.truncate(MAX_KEPT);
        let env_path = self.environment_path();
        if let Some(parent) = env_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let items: Vec<Value> = next
            .into_iter()
            .map(|(f, i)| json!({ "folder": f, "id": i }))
            .collect();
        let body = serde_json::to_string(&items).map_err(|e| e.to_string())?;
        atomic_write(&env_path, &body)
    }

    pub fn environment(&self, folder: &str) -> Option<String> {
        self.read_environments()
            .into_iter()
            .find(|(f, _)| f == folder)
            .map(|(_, id)| id)
    }

    fn launch_of(&self) -> Result<(String, Vec<String>, Option<BTreeMap<String, String>>), String> {
        if !matches!(self.platform, CloudPlatform::Win32 | CloudPlatform::Darwin) {
            return Err("Claude's cloud opens from here on Windows and macOS so far. Run claude --cloud in a terminal in this folder.".into());
        }
        let Some(launch) = (self.discover)() else {
            return Err("Claude Code is not installed here. Settings > AI agents shows how to add it.".into());
        };
        let under_own_node = launch.env.is_some();
        if self.platform != CloudPlatform::Win32 || under_own_node {
            Ok((launch.executable_path, launch.prefix_args, launch.env))
        } else {
            Ok((launch.discovered_path, Vec::new(), launch.env))
        }
    }

    fn start_unseen(
        &self,
        folder: &str,
        task: &str,
        choice: Option<&CloudChoice>,
    ) -> Option<CloudStartReading> {
        if self.platform != CloudPlatform::Win32 {
            return None;
        }
        let terminal = self.terminal.as_ref()?;
        let (file, prefix, env_opt) = self.launch_of().ok()?;
        let prefix_refs: Vec<&str> = prefix.iter().map(String::as_str).collect();
        let cmd_line = windows_command_line(
            &file,
            &prefix_refs,
            CloudAction::Start,
            &WindowsCommandOptions {
                stay: Some(false),
                choice,
                ..Default::default()
            },
        )
        .ok()?;
        let mut env = env_opt.unwrap_or_default();
        env.insert(TASK_VARIABLE.into(), task.into());
        match terminal(&PseudoTerminalRun {
            line: format!("cmd.exe {cmd_line}"),
            cwd: folder.into(),
            env,
            seconds: 60,
            stop_when: ASKS_THE_PERSON.into(),
        }) {
            PseudoTerminalResult::Ok { drawn } => Some(read_cloud_start(&drawn)),
            PseudoTerminalResult::Err => None,
        }
    }

    pub fn start(
        &self,
        folder: &str,
        prompt: &str,
        teammate_id: Option<&str>,
        choice: Option<&CloudChoice>,
    ) -> ClaudeCloudAnswer {
        let task = cloud_task_text(prompt);
        if task.is_empty() {
            return ClaudeCloudAnswer::Err {
                message: "Describe the task first. Nothing was sent.".into(),
            };
        }
        if let Some(env) = choice.and_then(|c| c.environment.as_deref())
            && !is_cloud_environment_id(env)
        {
            return ClaudeCloudAnswer::Err {
                message: "Use an environment ID beginning ccpool_ with 1 to 80 letters, digits, underscores or hyphens, or leave it empty. Nothing was sent.".into(),
            };
        }
        let _ = self.remember_environment(folder, choice.and_then(|c| c.environment.as_deref()));
        let unseen = self.start_unseen(folder, &task, choice);
        if let Some(CloudStartReading::Failed {
            message: Some(ref msg),
        }) = unseen
        {
            return ClaudeCloudAnswer::Err {
                message: format!("Claude Code did not start a cloud session: {msg}"),
            };
        }
        let mut asked = false;
        if !matches!(unseen, Some(CloudStartReading::Created { .. })) {
            let (file, prefix, _env) = match self.launch_of() {
                Ok(v) => v,
                Err(message) => return ClaudeCloudAnswer::Err { message },
            };
            let prefix_refs: Vec<&str> = prefix.iter().map(String::as_str).collect();
            let line = match windows_command_line(
                &file,
                &prefix_refs,
                CloudAction::Start,
                &WindowsCommandOptions {
                    choice,
                    ..Default::default()
                },
            ) {
                Ok(l) => l,
                Err(message) => return ClaudeCloudAnswer::Err { message },
            };
            if let Some(run_window) = self.run_window.as_ref()
                && let Err(message) = run_window("cmd.exe", &[line], Path::new(folder))
            {
                return ClaudeCloudAnswer::Err { message };
            }
            asked = matches!(unseen, Some(CloudStartReading::Asks));
        }
        static SEQ: AtomicU64 = AtomicU64::new(1);
        let id = format!("cc_{:020x}", SEQ.fetch_add(1, Ordering::Relaxed));
        let (session_id, url, title, note) = match unseen {
            Some(CloudStartReading::Created {
                session_id,
                url,
                title,
                note,
            }) => (Some(session_id), Some(url), title, note),
            _ => (
                None,
                None,
                None,
                if asked {
                    Some("Claude Code asks whether you trust this folder first. Answer it in the window that opened; the session starts once you do.".into())
                } else {
                    None
                },
            ),
        };
        let session = ClaudeCloudSession {
            id,
            started_at: (self.now)(),
            prompt: task.chars().take(300).collect(),
            folder: folder.into(),
            teammate_id: teammate_id.map(str::to_string),
            session_id,
            url,
            title,
            note,
        };
        let mut sessions = self.read_sessions();
        sessions.insert(0, session.clone());
        sessions.truncate(MAX_KEPT);
        let _ = self.write_sessions(&sessions);
        ClaudeCloudAnswer::Ok { session }
    }

    /// A follow-up to the session itself: `claude -p --cloud <id> --output-format json "..."`.
    pub fn send(&self, id: &str, message: &str) -> CloudSendResult {
        let sessions = self.read_sessions();
        let Some(session) = sessions.into_iter().find(|s| s.id == id) else {
            return CloudSendResult::Err {
                message: "Locust does not know this session's id, so it cannot send to it. Send from claude.ai or the Claude app.".into(),
            };
        };
        let Some(session_id) = session.session_id.as_deref() else {
            return CloudSendResult::Err {
                message: "Locust does not know this session's id, so it cannot send to it. Send from claude.ai or the Claude app.".into(),
            };
        };
        let text = cloud_task_text(message);
        if text.is_empty() {
            return CloudSendResult::Err {
                message: "Write something to send first.".into(),
            };
        }
        let (file, prefix, env_opt) = match self.launch_of() {
            Ok(v) => v,
            Err(message) => return CloudSendResult::Err { message },
        };
        let Some(exec) = self.exec.as_ref() else {
            return CloudSendResult::Err {
                message: UNCONFIRMED_SEND_MESSAGE.into(),
            };
        };
        let mut env = env_opt.unwrap_or_default();
        env.insert(TASK_VARIABLE.into(), text.clone());
        let ran = if self.platform == CloudPlatform::Win32 {
            let prefix_refs: Vec<&str> = prefix.iter().map(String::as_str).collect();
            let line = match windows_command_line(
                &file,
                &prefix_refs,
                CloudAction::Send,
                &WindowsCommandOptions {
                    session: Some(session_id),
                    stay: Some(false),
                    ..Default::default()
                },
            ) {
                Ok(l) => l,
                Err(message) => return CloudSendResult::Err { message },
            };
            exec(
                "cmd.exe",
                &[line],
                &ExecOptions {
                    cwd: session.folder,
                    env,
                    verbatim: true,
                },
            )
        } else {
            let mut args = prefix;
            args.extend([
                "-p".into(),
                "--cloud".into(),
                session_id.into(),
                "--output-format".into(),
                "json".into(),
                text,
            ]);
            exec(
                &file,
                &args,
                &ExecOptions {
                    cwd: session.folder,
                    env,
                    verbatim: false,
                },
            )
        };

        // W3: the documented receipt on stdout confirms the send; stderr and terminal prose do not.
        let receipt_source = ran.stdout.as_deref().unwrap_or(&ran.output);
        let mut receipt: Option<serde_json::Map<String, Value>> = None;
        for line in receipt_source.trim().lines().rev() {
            if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(line) {
                receipt = Some(map);
                break;
            }
        }
        if let Some(ref map) = receipt
            && map.get("ok") == Some(&Value::Bool(false))
            && let Some(err) = map.get("error").and_then(Value::as_str)
        {
            return CloudSendResult::Err {
                message: err.to_string(),
            };
        }
        let receipt_ok = receipt
            .as_ref()
            .is_some_and(|map| map.get("ok") == Some(&Value::Bool(true)));
        if ran.code == 0 && receipt_ok {
            return CloudSendResult::Ok;
        }
        let plain = plain_terminal_text(&ran.output);
        if let Some(caps) = error_line_re().captures(&plain) {
            let said = caps[1].trim();
            if !said.is_empty() && !receipt_ok {
                let truncated: String = said.chars().take(300).collect();
                return CloudSendResult::Err {
                    message: format!("Claude Code did not send that: {truncated}"),
                };
            }
        }
        CloudSendResult::Err {
            message: UNCONFIRMED_SEND_MESSAGE.into(),
        }
    }
}

fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    static TMP_SEQ: AtomicU64 = AtomicU64::new(0);
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(format!(".{}.{seq}.tmp", std::process::id()));
    let tmp_path = PathBuf::from(tmp);
    fs::write(&tmp_path, content).map_err(|e| e.to_string())?;
    fs::rename(&tmp_path, path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        e.to_string()
    })
}

fn session_to_json(session: &ClaudeCloudSession) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("id".into(), json!(session.id));
    obj.insert("startedAt".into(), json!(session.started_at));
    obj.insert("prompt".into(), json!(session.prompt));
    obj.insert("folder".into(), json!(session.folder));
    if let Some(tm) = &session.teammate_id {
        obj.insert("teammateId".into(), json!(tm));
    }
    if let Some(sid) = &session.session_id {
        obj.insert("sessionId".into(), json!(sid));
    }
    if let Some(url) = &session.url {
        obj.insert("url".into(), json!(url));
    }
    if let Some(title) = &session.title {
        obj.insert("title".into(), json!(title));
    }
    if let Some(note) = &session.note {
        obj.insert("note".into(), json!(note));
    }
    Value::Object(obj)
}

fn session_from_json(value: &Value) -> Option<ClaudeCloudSession> {
    static ID_RE: OnceLock<Regex> = OnceLock::new();
    let id_re = ID_RE.get_or_init(|| Regex::new(r"^cc_[a-f0-9]{1,32}$").expect("id regex"));
    let obj = value.as_object()?;
    let id = obj.get("id")?.as_str()?;
    if !id_re.is_match(id) {
        return None;
    }
    let started_at = obj.get("startedAt")?.as_str()?;
    if chrono::DateTime::parse_from_rfc3339(started_at).is_err() {
        return None;
    }
    let prompt = obj.get("prompt")?.as_str()?;
    if prompt.len() > 300 {
        return None;
    }
    let folder = obj.get("folder")?.as_str()?;
    if folder.is_empty() {
        return None;
    }
    let session_id = match obj.get("sessionId") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if session_id_re().is_match(s) => Some(s.clone()),
        _ => return None,
    };
    let url = match obj.get("url") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.starts_with("https://claude.ai/code/") => Some(s.clone()),
        _ => return None,
    };
    let title = match obj.get("title") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.len() <= 120 => Some(s.clone()),
        _ => return None,
    };
    let note = match obj.get("note") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.len() <= 400 => Some(s.clone()),
        _ => return None,
    };
    let teammate_id = match obj.get("teammateId") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) => Some(s.clone()),
        _ => return None,
    };
    Some(ClaudeCloudSession {
        id: id.into(),
        started_at: started_at.into(),
        prompt: prompt.into(),
        folder: folder.into(),
        teammate_id,
        session_id,
        url,
        title,
        note,
    })
}

#[cfg(test)]
#[path = "claude_cloud_send_tests.rs"]
mod send_tests;

#[cfg(test)]
#[path = "claude_cloud_env_tests.rs"]
mod env_tests;

//! Claude Code cloud sessions (`apps/desktop/src/main/claude-cloud.ts` and
//! `apps/desktop/src/shared/claude-cloud.ts`): starting a cloud session with
//! optional model, effort and per-folder organization environment ID, and
//! confirming follow-up sends strictly from Claude Code's JSON receipt on stdout.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};

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

/// pseudo-terminal.ts `plainTerminalText`, its four passes in its order.
fn terminal_passes() -> &'static [Regex; 4] {
    static RE: OnceLock<[Regex; 4]> = OnceLock::new();
    RE.get_or_init(|| {
        [
            Regex::new(r"\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)").expect("osc regex"),
            Regex::new(r"\x1b\[[0-9;?<>=]*[ -/]*[@-~]").expect("csi regex"),
            Regex::new(r"\x1b[()][0-9A-Za-z]|\x1b[=>78MDEc]").expect("esc regex"),
            // What is left of the terminal's own controls (a shift-in, a bell): never words.
            Regex::new(r"[\x00-\x08\x0b-\x1f\x7f]").expect("control regex"),
        ]
    })
}

fn any_ws_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\s+").expect("ws regex"))
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

/// What a terminal drew, as text: colours, cursor moves and titles taken out.
pub fn plain_terminal_text(drawn: &str) -> String {
    let [osc, csi, esc, control] = terminal_passes();
    let text = osc.replace_all(drawn, "");
    let text = csi.replace_all(&text, "");
    let text = esc.replace_all(&text, "").replace('\r', "");
    control.replace_all(&text, "").into_owned()
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
                let collapsed = any_ws_re().replace_all(&joined, " ");
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

/// Where Claude Code keeps a folder's transcripts: `projects/` and the folder's path with every other character a
/// dash (measured: `C:\\...\\.claude\\worktrees\\x` is `C--...--claude-worktrees-x`).
pub fn transcript_folder_of(claude_home: &Path, folder: &str) -> PathBuf {
    let slug: String = folder
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    claude_home.join("projects").join(slug)
}

/// The worktree a session is read into, by Locust's own id for it: never a name the person chose.
pub fn reading_worktree_name(id: &str) -> String {
    let bare = id.strip_prefix("cc_").unwrap_or(id);
    format!("locust-cloud-{}", bare.chars().take(12).collect::<String>())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadingProblem {
    Asks,
    LongPaths,
}

/// What `claude --teleport <id> --worktree <name>` drew, for what only the person can do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeleportReading {
    pub problem: Option<ReadingProblem>,
    pub no_branch: bool,
    pub gone: bool,
    pub error: Option<String>,
}

pub fn read_teleport(drawn: &str) -> TeleportReading {
    static ERROR: OnceLock<Regex> = OnceLock::new();
    static GONE: OnceLock<Regex> = OnceLock::new();
    static LONG: OnceLock<Regex> = OnceLock::new();
    static NO_BRANCH: OnceLock<Regex> = OnceLock::new();
    let text = plain_terminal_text(drawn);
    // "Error: ..." and, since 2.1.29x, "Error creating worktree: ...".
    let error = ERROR
        .get_or_init(|| Regex::new(r"\bError(?: [a-z]+){0,4}:[ \t]*(.+)").expect("error regex"))
        .captures(&text)
        .map(|c| c[1].trim().chars().take(300).collect::<String>());
    TeleportReading {
        // Measured 10/03 (0.564): an archived or deleted session -- Claude Code makes the worktree first, then says this.
        gone: GONE.get_or_init(|| Regex::new(r"(?i)Session not found").expect("gone regex")).is_match(&text),
        problem: if asks_re().is_match(&text) {
            Some(ReadingProblem::Asks)
        } else if LONG.get_or_init(|| Regex::new(r"(?i)Filename too long").expect("long regex")).is_match(&text) {
            Some(ReadingProblem::LongPaths)
        } else {
            None
        },
        // Measured 10/02: a session that changed nothing pushed no branch, and Claude Code says so.
        no_branch: NO_BRANCH
            .get_or_init(|| Regex::new(r"(?i)resumed without branch|Failed to checkout branch").expect("branch regex"))
            .is_match(&text),
        error,
    }
}

/// One exchange of a session read here: what was asked, what Claude said in words, when, on which model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudExchange {
    pub prompt: String,
    pub answer: Option<String>,
    pub at: String,
    pub model: Option<String>,
}

/// What a check of a cloud session read (`ClaudeCloudReading`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaudeCloudReading {
    Ok {
        exchanges: Vec<CloudExchange>,
        checked_at: String,
        diff: Option<String>,
        change_too_big: bool,
        note: Option<String>,
    },
    Err {
        message: String,
    },
}

/// git's answer: its exit code and its two streams apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRan {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// The bound on a change Locust reads into the panel.
const MAX_DIFF: usize = 4 * 1024 * 1024;
const MAX_EXCHANGES: usize = 40;

/// The reads of cloud sessions under way, by Locust's id: never two reads of one session at once (W4). Shared by
/// every `ClaudeCloud` the app makes, so a second Check joins the first one's teleport instead of starting its own.
#[derive(Default)]
pub struct CloudReads {
    under_way: Mutex<HashMap<String, Arc<ReadUnderWay>>>,
}

#[derive(Default)]
struct ReadUnderWay {
    answer: Mutex<Option<ClaudeCloudReading>>,
    done: Condvar,
    /// How many later checks are waiting on this one.
    waiting: AtomicUsize,
}

/// Ends a read: its answer to everyone waiting, and its place in the ledger freed, even if the read panicked.
struct ReadEnds<'r> {
    reads: &'r CloudReads,
    id: &'r str,
    read: Arc<ReadUnderWay>,
}

impl Drop for ReadEnds<'_> {
    fn drop(&mut self) {
        self.reads.under_way.lock().unwrap_or_else(|e| e.into_inner()).remove(self.id);
        let mut answer = self.read.answer.lock().unwrap_or_else(|e| e.into_inner());
        if answer.is_none() {
            *answer = Some(ClaudeCloudReading::Err {
                message: "Claude Code could not be reached to read it. Check again.".into(),
            });
        }
        self.read.done.notify_all();
    }
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
    /// Windows: the command line, as CreateProcess takes it. Empty where `argv` is what runs.
    pub line: String,
    /// macOS: the program and its arguments, run as they are (`claude_cloud::run_in_pseudo_terminal`). Empty on Windows.
    pub argv: Vec<String>,
    pub cwd: String,
    pub env: BTreeMap<String, String>,
    /// Variables of the app's own environment the run must not inherit (Electron sets them to undefined).
    pub env_remove: Vec<String>,
    pub seconds: u64,
    pub stop_when: String,
    /// Done once a transcript appears in this folder, not at the words (`stopWhenWritten`).
    pub stop_when_written: Option<String>,
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

/// How Claude Code's own window is opened (Electron's `open`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKind {
    /// Windows: `cmd.exe` in a console of its own (detached), its one argument passed verbatim.
    Console,
    /// macOS: a script in the person's terminal app (`openInScriptTerminal`).
    ScriptTerminal,
    /// A visible, detached Windows Terminal tab for a cloud copy.
    WindowsTerminal,
    /// A new console for a cloud copy when Windows Terminal is unavailable.
    DetachedConsole,
}

/// A window for Claude Code: what runs, where, with which variables beyond the app's own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowRun {
    pub kind: WindowKind,
    pub file: String,
    pub args: Vec<String>,
    pub cwd: String,
    pub env: BTreeMap<String, String>,
}

pub const WINDOW_NOT_OPENED: &str =
    "The window for Claude Code could not be opened. Run claude --cloud in a terminal in this folder.";
pub const NOT_KNOWN_ANY_MORE: &str = "Locust does not know that cloud session any more.";

/// The answer of a send, a home or an apply: done, or what to say.
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
    run_window: Option<Box<dyn Fn(&WindowRun) -> Result<(), String> + 'a>>,
    detached_policy: Option<Box<dyn Fn() -> crate::org_policy::PolicyState + 'a>>,
    git_changes: Box<dyn Fn(&str) -> Option<usize> + 'a>,
    git: Box<dyn Fn(&[&str], &str, Option<&str>) -> GitRan + 'a>,
    claude_home: PathBuf,
    process_env: Box<dyn Fn() -> Vec<(String, String)> + 'a>,
    exchanges_of: Box<dyn Fn(&Path) -> Vec<CloudExchange> + 'a>,
    reads: Arc<CloudReads>,
    /// Between looks for the transcript, ten looks in all.
    transcript_pause: std::time::Duration,
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
            detached_policy: None,
            git_changes: Box::new(git_changes_in),
            git: Box::new(git_run),
            claude_home: default_claude_home(),
            process_env: Box::new(|| std::env::vars().collect()),
            exchanges_of: Box::new(transcript_exchanges),
            reads: Arc::default(),
            transcript_pause: std::time::Duration::from_millis(500),
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

    /// Claude Code's own window. Without one, a window that is needed is said not to have opened.
    pub fn with_run_window(
        mut self,
        run_window: impl Fn(&WindowRun) -> Result<(), String> + 'a,
    ) -> Self {
        self.run_window = Some(Box::new(run_window));
        self
    }

    /// Files changed or untracked in a folder; None when it is not a git checkout.
    pub fn with_git_changes(mut self, git_changes: impl Fn(&str) -> Option<usize> + 'a) -> Self {
        self.git_changes = Box::new(git_changes);
        self
    }

    pub fn with_git(mut self, git: impl Fn(&[&str], &str, Option<&str>) -> GitRan + 'a) -> Self {
        self.git = Box::new(git);
        self
    }

    /// Where Claude Code keeps its transcripts (`projects/` inside it).
    pub fn with_claude_home(mut self, claude_home: impl Into<PathBuf>) -> Self {
        self.claude_home = claude_home.into();
        self
    }

    /// The app's own environment, as a read is started from it.
    pub fn with_process_env(mut self, env: impl Fn() -> Vec<(String, String)> + 'a) -> Self {
        self.process_env = Box::new(env);
        self
    }

    /// The app's one ledger of reads under way, shared by every `ClaudeCloud` it makes.
    pub fn with_reads(mut self, reads: Arc<CloudReads>) -> Self {
        self.reads = reads;
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

    /// Claude Code in a window of its own, in `folder`: `--cloud <task>` or `--teleport [session]`.
    fn open(
        &self,
        folder: &str,
        action: CloudAction,
        task: Option<&str>,
        session_id: Option<&str>,
        choice: Option<&CloudChoice>,
    ) -> Result<(), String> {
        let (file, prefix, env) = self.launch_of()?;
        let start = action == CloudAction::Start;
        let run = if self.platform != CloudPlatform::Win32 {
            let mut args = prefix;
            if start {
                args.extend(choice_args(choice)?);
                args.push("--cloud".into());
                args.push(task.unwrap_or("").into());
            } else {
                args.push("--teleport".into());
                args.extend(session_id.map(str::to_string));
            }
            WindowRun {
                kind: WindowKind::ScriptTerminal,
                file,
                args,
                cwd: folder.into(),
                env: env.unwrap_or_default(),
            }
        } else {
            let prefix_refs: Vec<&str> = prefix.iter().map(String::as_str).collect();
            let line = windows_command_line(
                &file,
                &prefix_refs,
                action,
                &WindowsCommandOptions {
                    session: session_id,
                    choice,
                    ..Default::default()
                },
            )?;
            let mut env = env.unwrap_or_default();
            if start {
                env.insert(TASK_VARIABLE.into(), cloud_task_text(task.unwrap_or("")));
            }
            WindowRun {
                kind: WindowKind::Console,
                file: "cmd.exe".into(),
                args: vec![line],
                cwd: folder.into(),
                env,
            }
        };
        match self.run_window.as_ref() {
            Some(run_window) => run_window(&run),
            None => Err(WINDOW_NOT_OPENED.into()),
        }
    }

    /// The sessions sent from `folder`, newest first.
    pub fn list(&self, folder: &str) -> Vec<ClaudeCloudSession> {
        self.read_sessions()
            .into_iter()
            .filter(|session| session.folder == folder)
            .collect()
    }

    /// Every session sent from any folder, newest first: what the door's list_conversations shows (`list` is one
    /// folder's).
    pub fn all(&self) -> Vec<ClaudeCloudSession> {
        self.read_sessions()
    }

    /// `claude --teleport <id>` in the session's folder. Claude Code refuses a folder with changes in it, so this
    /// says so first rather than opening a window that only refuses.
    pub fn home(&self, id: &str) -> CloudSendResult {
        let Some(session) = self.read_sessions().into_iter().find(|s| s.id == id) else {
            return CloudSendResult::Err {
                message: NOT_KNOWN_ANY_MORE.into(),
            };
        };
        if let Some(changed) = (self.git_changes)(&session.folder)
            && changed > 0
        {
            return CloudSendResult::Err {
                message: format!(
                    "Claude Code brings a cloud session home only into a folder with no changes, and this one has {changed} changed file{}. Commit or set them aside, then bring it home.",
                    if changed == 1 { "" } else { "s" }
                ),
            };
        }
        match self.open(
            &session.folder,
            CloudAction::Home,
            None,
            session.session_id.as_deref(),
            None,
        ) {
            Ok(()) => CloudSendResult::Ok,
            Err(message) => CloudSendResult::Err { message },
        }
    }

    /// Continue the cloud session in a fresh, detached checkout. Its work never comes into
    /// the person's folder or Locust's reading checkout, and a previously opened copy stays put.
    pub fn continue_in_terminal(&self, id: &str) -> CloudSendResult {
        match self.open_detached_copy(id) {
            Ok(()) => CloudSendResult::Ok,
            Err(message) => CloudSendResult::Err { message },
        }
    }

    fn open_detached_copy(&self, id: &str) -> Result<(), String> {
        let invalid = || "Locust does not know a valid id for this cloud session. Open it on claude.ai.".to_owned();
        let mut matching = self.read_sessions().into_iter().filter(|s| s.id == id);
        let session = matching.next().ok_or_else(invalid)?;
        if matching.next().is_some() {
            return Err(invalid());
        }
        let session_id = session.session_id.as_deref()
            .filter(|id| session_id_re().is_match(id)).ok_or_else(invalid)?;
        if self.platform == CloudPlatform::Linux {
            return Err("Continue in terminal is not available in this Linux build. Open the session on claude.ai.".into());
        }
        if let Some(policy) = &self.detached_policy {
            let policy = policy();
            if let Some(why) = crate::org_policy::unmetered_refusal(
                Some(&policy), "claude", None, "auto", true, false,
            ).or_else(|| crate::org_policy::terminal_refusal(Some(&policy))) {
                return Err(why);
            }
        }
        let (file, prefix, env) = self.launch_of()?;
        let open = self.run_window.as_ref().ok_or("The terminal could not be opened. Open this session on claude.ai.")?;
        // Native IDs pad a counter, so the reading worktree's first-12 convention would
        // give many different sessions the same terminal path. Keep the full validated ID.
        let bare = session.id.strip_prefix("cc_").ok_or_else(invalid)?;
        let name = format!("locust-cloud-{bare}-here");
        let place = self.worktree_of(&session.folder, &name)
            .ok_or("This folder is not a git checkout. Open this session on claude.ai.")?;
        let where_ = place.to_str().ok_or("The terminal worktree's path cannot be read.")?;
        let cannot_create = || "The terminal worktree could not be created. An earlier copy may still be open; finish there or open the session on claude.ai.".to_owned();
        reserve_detached_copy(&place).map_err(|_| cannot_create())?;
        let made = (self.git)(&["worktree", "add", "--detach", where_, "HEAD"], &session.folder, None);
        if made.code != 0 {
            // Only an empty reservation of ours can be removed here. In particular, never force
            // removal of a path Git already knows about or of a folder holding someone's work.
            let _ = fs::remove_dir(&place);
            return Err(cannot_create());
        }

        let opened = self.open_copy_window(open, where_, file, prefix, env, session_id);
        if opened.is_err() {
            // Without force, Git preserves the checkout if the failed launch nevertheless changed it.
            let removed = (self.git)(&["worktree", "remove", where_], &session.folder, None);
            if removed.code != 0 {
                return Err(format!("The terminal could not be opened. Locust kept its worktree at {where_}. Open this session on claude.ai."));
            }
        }
        opened
    }

    fn open_copy_window(
        &self,
        open: &dyn Fn(&WindowRun) -> Result<(), String>,
        cwd: &str,
        file: String,
        prefix: Vec<String>,
        env: Option<BTreeMap<String, String>>,
        id: &str,
    ) -> Result<(), String> {
        let verb = ["--teleport".to_owned(), id.to_owned()];
        let failed = || "The terminal could not be opened. Open this session on claude.ai.".to_owned();
        if self.platform == CloudPlatform::Darwin {
            return open(&WindowRun {
                kind: WindowKind::ScriptTerminal, file, cwd: cwd.into(),
                args: prefix.into_iter().chain(verb).collect(), env: env.unwrap_or_default(),
            }).map_err(|_| failed());
        }

        use crate::runtime_terminal::{TerminalLaunch, console_line, terminal_program, windows_terminal_args};
        let program = terminal_program(&TerminalLaunch {
            command_name: "claude".into(), executable_path: file, prefix_args: prefix, env: None,
        }, &verb);
        if env.is_none()
            && let Some(args) = windows_terminal_args(&program, cwd, "Locust · Claude cloud")
        {
            let wt = (self.process_env)().into_iter()
                .find(|(key, value)| key.eq_ignore_ascii_case("LOCALAPPDATA") && !value.is_empty())
                .map_or_else(|| "wt.exe".into(), |(_, local)| {
                    format!("{}\\Microsoft\\WindowsApps\\wt.exe", local.trim_end_matches(['\\', '/']))
                });
            if open(&WindowRun {
                kind: WindowKind::WindowsTerminal, file: wt, args, cwd: cwd.into(),
                env: BTreeMap::new(),
            }).is_ok() {
                return Ok(());
            }
        }
        // cmd expands percent and can expand bang even inside quotes. Do not pass a launch path
        // that cannot be put on this command line literally. The id already passed its strict check.
        if std::iter::once(&program.file)
            .chain(&program.args)
            .any(|word| word.contains(['"', '%', '!', '^', '&', '|', '<', '>', '\r', '\n', '\0']))
        {
            return Err(failed());
        }
        open(&WindowRun {
            kind: WindowKind::DetachedConsole, file: "cmd.exe".into(), cwd: cwd.into(),
            args: vec![console_line(&program, &verb)], env: env.unwrap_or_default(),
        }).map_err(|_| failed())
    }

    /// Where `--worktree <name>` puts the session: `.claude/worktrees/<name>` in the repository's main checkout
    /// (measured 10/02: from a linked worktree it landed beside git's common directory, not in the folder itself).
    fn worktree_of(&self, folder: &str, name: &str) -> Option<PathBuf> {
        let common = (self.git)(&["rev-parse", "--path-format=absolute", "--git-common-dir"], folder, None);
        if common.code != 0 {
            return None;
        }
        let dir = common.stdout.trim();
        if dir.is_empty() {
            return None;
        }
        let dir = Path::new(folder).join(dir);
        Some(dir.parent()?.join(".claude").join("worktrees").join(name))
    }

    /// The reading's worktree and its branch, gone. Claude Code locks the worktrees it makes, hence the second --force.
    fn remove_reading(&self, folder: &str, name: &str) {
        let Some(place) = self.worktree_of(folder, name) else {
            return;
        };
        let place = place.to_string_lossy().into_owned();
        (self.git)(&["worktree", "remove", "--force", "--force", &place], folder, None);
        (self.git)(&["worktree", "prune"], folder, None);
        (self.git)(&["branch", "-D", &format!("worktree-{name}")], folder, None);
    }

    /// The transcript Claude Code wrote for the worktree, the newest since `since`.
    fn transcript_for(&self, name: &str, since: std::time::SystemTime) -> Option<PathBuf> {
        let projects = self.claude_home.join("projects");
        let floor = since.checked_sub(std::time::Duration::from_secs(5)).unwrap_or(since);
        let mut best: Option<(PathBuf, std::time::SystemTime)> = None;
        let tail = format!("-claude-worktrees-{name}");
        for dir in fs::read_dir(&projects).ok()?.flatten() {
            if !dir.file_name().to_string_lossy().ends_with(&tail) {
                continue;
            }
            for file in fs::read_dir(dir.path()).into_iter().flatten().flatten() {
                let path = file.path();
                if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let Some(at) = file.metadata().ok().and_then(|m| m.modified().ok()) else {
                    continue;
                };
                if at >= floor && best.as_ref().is_none_or(|(_, b)| at > *b) {
                    best = Some((path, at));
                }
            }
        }
        best.map(|(path, _)| path)
    }

    /// What the session's branch changed, from where it parted from the folder's commit. None: nothing.
    fn change_in(&self, place: &str, folder: &str) -> Option<String> {
        let head = (self.git)(&["rev-parse", "HEAD"], folder, None);
        if head.code != 0 {
            return None;
        }
        let base = (self.git)(&["merge-base", "HEAD", head.stdout.trim()], place, None);
        if base.code != 0 {
            return None;
        }
        let diff = (self.git)(&["diff", "--binary", base.stdout.trim(), "HEAD"], place, None);
        (diff.code == 0 && !diff.stdout.trim().is_empty()).then_some(diff.stdout)
    }

    /// WHAT THE SESSION DID, READ HERE (0.558): `claude --teleport <id> --worktree <name>` in a terminal nobody
    /// sees fetches the session's conversation and checks its branch out beside the folder, the folder untouched.
    /// The transcript it saved is read as an imported session is; its change is the branch against the folder's
    /// commit, kept until Apply or Forget. Every check is a fresh teleport; nothing polls.
    /// Overlapping checks of one session share one teleport; once it ends, the next check starts afresh.
    pub fn check(&self, id: &str) -> ClaudeCloudReading {
        let mut under_way = self.reads.under_way.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(read) = under_way.get(id).cloned() {
            drop(under_way);
            read.waiting.fetch_add(1, Ordering::SeqCst);
            let mut answer = read.answer.lock().unwrap_or_else(|e| e.into_inner());
            while answer.is_none() {
                answer = read.done.wait(answer).unwrap_or_else(|e| e.into_inner());
            }
            return answer.clone().unwrap_or_else(|| unreachable!());
        }
        let read = Arc::new(ReadUnderWay::default());
        under_way.insert(id.to_string(), read.clone());
        drop(under_way);
        let ends = ReadEnds { reads: &self.reads, id, read };
        let reading = self.check_once(id);
        *ends.read.answer.lock().unwrap_or_else(|e| e.into_inner()) = Some(reading.clone());
        drop(ends);
        reading
    }

    fn check_once(&self, id: &str) -> ClaudeCloudReading {
        let fail = |message: &str| ClaudeCloudReading::Err { message: message.into() };
        let Some(session) = self.read_sessions().into_iter().find(|s| s.id == id) else {
            return fail(NOT_KNOWN_ANY_MORE);
        };
        let Some(session_id) = session.session_id.clone() else {
            return fail("Locust does not know this session\u{2019}s id, so it cannot read it here. See it on claude.ai.");
        };
        let Some(terminal) = self
            .terminal
            .as_ref()
            .filter(|_| matches!(self.platform, CloudPlatform::Win32 | CloudPlatform::Darwin))
        else {
            return fail("Reading a cloud session here works on Windows and macOS so far. See it on claude.ai, or bring it home.");
        };
        let (file, prefix, launch_env) = match self.launch_of() {
            Ok(v) => v,
            Err(message) => return ClaudeCloudReading::Err { message },
        };
        let name = reading_worktree_name(&session.id);
        let Some(place) = self.worktree_of(&session.folder, &name) else {
            return fail("This folder is not a git checkout, so Claude Code cannot bring the session into it. See it on claude.ai.");
        };
        self.remove_reading(&session.folder, &name);
        let began = std::time::SystemTime::now();
        let (line, argv) = if self.platform == CloudPlatform::Win32 {
            let prefix_refs: Vec<&str> = prefix.iter().map(String::as_str).collect();
            match windows_command_line(
                &file,
                &prefix_refs,
                CloudAction::Read,
                &WindowsCommandOptions {
                    session: Some(&session_id),
                    stay: Some(false),
                    worktree: Some(&name),
                    ..Default::default()
                },
            ) {
                Ok(line) => (format!("cmd.exe {line}"), Vec::new()),
                Err(message) => return ClaudeCloudReading::Err { message },
            }
        } else {
            // No shell between: the words go as they are (the store keeps a session id only when it is one).
            let mut argv = vec![file.clone()];
            argv.extend(prefix.iter().cloned());
            argv.extend(["--teleport".into(), session_id.clone(), "--worktree".into(), name.clone()]);
            (String::new(), argv)
        };
        // Run from inside Claude Code (a drive, a teammate), its markers would turn transcript saving off.
        let env_remove = (self.process_env)()
            .into_iter()
            .map(|(key, _)| key)
            .filter(|key| {
                let upper = key.to_ascii_uppercase();
                upper.starts_with("CLAUDE") && upper != "CLAUDE_CONFIG_DIR"
            })
            .collect();
        let drawn = match terminal(&PseudoTerminalRun {
            line,
            argv,
            cwd: session.folder.clone(),
            env: launch_env.unwrap_or_default(),
            env_remove,
            seconds: 120,
            stop_when: format!("{ASKS_THE_PERSON}|Filename too long"),
            // Done when its transcript is written: measured 10/02, that comes a few seconds AFTER it says
            // "Session resumed", and ending it at the words left none.
            stop_when_written: Some(
                transcript_folder_of(&self.claude_home, &place.to_string_lossy())
                    .to_string_lossy()
                    .into_owned(),
            ),
        }) {
            PseudoTerminalResult::Ok { drawn } => drawn,
            PseudoTerminalResult::Err => return fail("Claude Code could not be run to read it. See it on claude.ai."),
        };
        let said = read_teleport(&drawn);
        if let Some(problem) = said.problem {
            self.remove_reading(&session.folder, &name);
            return fail(match problem {
                ReadingProblem::Asks => "Claude Code asks whether you trust this folder first. Open Claude Code in this folder once and answer it, then check again.",
                ReadingProblem::LongPaths => "Git could not check the session out here: some of its paths are longer than Windows allows. Turning on long paths for this repository fixes it: git config core.longpaths true",
            });
        }
        if said.gone {
            // Its worktree was made before Claude Code looked the session up: nothing came in.
            self.remove_reading(&session.folder, &name);
            return fail("Claude Code says this session is not found in Claude\u{2019}s cloud: it was archived or deleted. See it on claude.ai, or forget it here.");
        }
        let mut transcript = None;
        for tries in 0..10 {
            transcript = self.transcript_for(&name, began);
            if transcript.is_some() {
                break;
            }
            if tries < 9 {
                std::thread::sleep(self.transcript_pause);
            }
        }
        // Measured 10/02: in two runs of six the session came in (its branch checked out) but no transcript was
        // saved within two minutes, for no reason it drew. Its change is still shown.
        if transcript.is_none() && !place.is_dir() {
            self.remove_reading(&session.folder, &name);
            return ClaudeCloudReading::Err {
                message: match said.error {
                    Some(error) => format!("Claude Code did not bring the session in: {error}"),
                    None => "Claude Code did not bring the session in. See it on claude.ai.".into(),
                },
            };
        }
        let mut exchanges = transcript.as_deref().map(|path| (self.exchanges_of)(path)).unwrap_or_default();
        let keep_from = exchanges.len().saturating_sub(MAX_EXCHANGES);
        let exchanges: Vec<CloudExchange> = exchanges
            .drain(keep_from..)
            .map(|exchange| CloudExchange {
                prompt: exchange.prompt.chars().take(4_000).collect(),
                answer: exchange.answer.map(|answer| answer.chars().take(16_000).collect()),
                at: exchange.at,
                model: exchange.model,
            })
            .collect();
        let change = if said.no_branch {
            None
        } else {
            self.change_in(&place.to_string_lossy(), &session.folder)
        };
        let too_big = change.as_ref().is_some_and(|c| c.len() > MAX_DIFF);
        ClaudeCloudReading::Ok {
            exchanges,
            checked_at: (self.now)(),
            diff: change.filter(|_| !too_big),
            change_too_big: too_big,
            note: if said.no_branch {
                Some("Claude Code could not check out its branch, so there is no change to show here. A session that changed nothing has none.".into())
            } else if transcript.is_none() {
                Some("Claude Code brought it in but did not save its conversation this time. Check again to read it.".into())
            } else {
                None
            },
        }
    }

    /// The change last read comes into the folder, not committed; the reading's worktree goes.
    pub fn apply(&self, id: &str) -> CloudSendResult {
        let fail = |message: String| CloudSendResult::Err { message };
        let Some(session) = self.read_sessions().into_iter().find(|s| s.id == id) else {
            return fail(NOT_KNOWN_ANY_MORE.into());
        };
        let name = reading_worktree_name(&session.id);
        let Some(place) = self.worktree_of(&session.folder, &name).filter(|p| p.is_dir()) else {
            return fail("Check it again first: Locust no longer has its change.".into());
        };
        let Some(change) = self.change_in(&place.to_string_lossy(), &session.folder) else {
            return fail("It has no change to bring in.".into());
        };
        // `git apply` changes nothing unless every file applies.
        let applied = (self.git)(&["apply", "--whitespace=nowarn", "-"], &session.folder, Some(&change));
        if applied.code != 0 {
            let why = applied
                .stderr
                .lines()
                .find(|line| !line.trim().is_empty())
                .map(|line| {
                    let line = line.trim_start();
                    let line = line.strip_prefix("error:").map(str::trim_start).unwrap_or(line);
                    line.trim().chars().take(200).collect::<String>()
                });
            return fail(format!(
                "Its change does not apply cleanly to this folder{}. Nothing was changed.",
                why.map(|why| format!(" ({why})")).unwrap_or_default()
            ));
        }
        self.remove_reading(&session.folder, &name);
        CloudSendResult::Ok
    }

    /// The session leaves the list, and its reading's worktree goes with it.
    pub fn forget(&self, id: &str) {
        let sessions = self.read_sessions();
        if let Some(session) = sessions.iter().find(|s| s.id == id)
            && session.session_id.is_some()
        {
            self.remove_reading(&session.folder, &reading_worktree_name(&session.id));
        }
        let kept: Vec<ClaudeCloudSession> = sessions.into_iter().filter(|s| s.id != id).collect();
        let _ = self.write_sessions(&kept);
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
            argv: Vec::new(),
            cwd: folder.into(),
            env,
            env_remove: Vec::new(),
            seconds: 60,
            stop_when: ASKS_THE_PERSON.into(),
            stop_when_written: None,
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
            // No terminal to hide in, or a question only the person answers: Claude Code's own window.
            if let Err(message) = self.open(folder, CloudAction::Start, Some(&task), None, choice) {
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

fn default_claude_home() -> PathBuf {
    match std::env::var_os("CLAUDE_CONFIG_DIR") {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".claude"),
    }
}

/// A transcript's exchanges, read as an imported Claude Code session is (session-import's reader).
fn transcript_exchanges(path: &Path) -> Vec<CloudExchange> {
    crate::session_import::session_exchanges(crate::session_import::ImportRuntime::Claude, path)
        .unwrap_or_default()
        .into_iter()
        .map(|exchange| CloudExchange {
            prompt: exchange.prompt,
            answer: exchange.answer,
            at: exchange.finished,
            model: exchange.model,
        })
        .collect()
}

/// Where the window keeps its cloud sessions (Electron's `userData/claude-cloud.json`).
pub fn store_path_of(store: &crate::store::Store) -> PathBuf {
    store.root().join("claude-cloud.json")
}

/// This computer's Claude Code for the window (Electron's `createClaudeCloud` in index.ts): found on PATH, its
/// sessions kept in the window's store, and its own window opened in the person's terminal app, the task passed as
/// a word of the script it runs. A session is read in a hidden terminal (`run_in_pseudo_terminal`); a start still
/// opens Claude Code's window on macOS.
pub fn on_this_computer(store: &crate::store::Store, reads: Arc<CloudReads>) -> ClaudeCloud<'static> {
    on_this_computer_with(store, reads, || {
        let path = crate::runtime::discovery::find_named("claude").ok()?.to_string_lossy().into_owned();
        Some(ClaudeLaunch { discovered_path: path.clone(), executable_path: path, prefix_args: Vec::new(), env: None })
    })
}

/// `on_this_computer` with the Claude Code it finds given, so a test can put a script where Claude Code would be.
pub(crate) fn on_this_computer_with(
    store: &crate::store::Store,
    reads: Arc<CloudReads>,
    discover: impl Fn() -> Option<ClaudeLaunch> + 'static,
) -> ClaudeCloud<'static> {
    let platform = if cfg!(windows) {
        CloudPlatform::Win32
    } else if cfg!(target_os = "macos") {
        CloudPlatform::Darwin
    } else {
        CloudPlatform::Linux
    };
    let windows = store.clone();
    let mut cloud = ClaudeCloud::new(store_path_of(store), platform, discover)
    .with_reads(reads)
    .with_exec(run_exec)
    .with_terminal(run_in_pseudo_terminal)
    .with_run_window(move |run: &WindowRun| {
        #[cfg(windows)]
        if matches!(run.kind, WindowKind::WindowsTerminal | WindowKind::DetachedConsole) {
            return spawn_cloud_copy_terminal(run);
        }
        if run.kind != WindowKind::ScriptTerminal {
            return Err(WINDOW_NOT_OPENED.into());
        }
        let script = windows
            .attachments_dir()
            .map_err(|_| WINDOW_NOT_OPENED.to_string())?
            .join(format!("claude-cloud-{}.command", crate::invariants::unique_nanos()));
        let env: Vec<(&str, &str)> = run.env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        crate::runtime_terminal::open_with_env(Path::new(&run.file), &run.args, Path::new(&run.cwd), &script, &windows, &env)
    });
    cloud.detached_policy = Some(Box::new(crate::org_policy::current));
    cloud
}

/// Reserve only a fresh destination. Do not follow a redirected .claude/worktrees directory
/// or let Git adopt an existing empty folder. Git will populate our own empty reservation.
fn reserve_detached_copy(place: &Path) -> std::io::Result<()> {
    let missing = || std::io::Error::other("No cloud worktree directory.");
    let worktrees = place.parent().ok_or_else(missing)?;
    let claude = worktrees.parent().ok_or_else(missing)?;
    for parent in [claude, worktrees] {
        match fs::create_dir(parent) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists
                && fs::symlink_metadata(parent)?.file_type().is_dir() => {}
            Err(error) => return Err(error),
        }
        // In particular reject Windows junctions and any other path redirected outside
        // the canonical checkout, not just Unix symbolic links.
        let expected = parent.parent().ok_or_else(missing)?.canonicalize()?.join(
            parent.file_name().ok_or_else(missing)?,
        );
        if parent.canonicalize()? != expected {
            return Err(missing());
        }
    }
    fs::create_dir(place)
}

#[cfg(windows)]
fn spawn_cloud_copy_terminal(run: &WindowRun) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let mut command = std::process::Command::new(&run.file);
    command.current_dir(&run.cwd).envs(&run.env);
    if run.kind == WindowKind::DetachedConsole {
        command.creation_flags(0x0000_0010); // CREATE_NEW_CONSOLE
        for arg in &run.args {
            command.raw_arg(arg);
        }
    } else {
        command.args(&run.args);
    }
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    std::thread::spawn(move || { let _ = child.wait(); });
    Ok(())
}

/// Claude Code run to its end with nothing on stdin, for a follow-up's receipt (Electron's `execFile`): stdout apart,
/// and stdout then stderr together as what it said. Verbatim arguments are Windows' own command line, passed as is.
pub fn run_exec(file: &str, args: &[String], options: &ExecOptions) -> ExecReply {
    let mut command = std::process::Command::new(file);
    #[cfg(windows)]
    if options.verbatim {
        use std::os::windows::process::CommandExt;
        for arg in args {
            command.raw_arg(arg);
        }
    } else {
        command.args(args);
    }
    #[cfg(not(windows))]
    command.args(args);
    command.envs(&options.env).stdin(std::process::Stdio::null());
    if !options.cwd.is_empty() {
        command.current_dir(&options.cwd);
    }
    match command.output() {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
            let output = format!("{stdout}{}", String::from_utf8_lossy(&out.stderr));
            ExecReply { code: out.status.code().unwrap_or(-1), output, stdout: Some(stdout) }
        }
        Err(e) => ExecReply { code: -1, output: e.to_string(), stdout: None },
    }
}

/// git, its output apart, with what it reads on stdin.
fn git_run(args: &[&str], cwd: &str, input: Option<&str>) -> GitRan {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let child = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let Ok(mut child) = child else {
        return GitRan { code: -1, stdout: String::new(), stderr: String::new() };
    };
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.unwrap_or("").as_bytes());
    }
    match child.wait_with_output() {
        Ok(out) => GitRan {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        },
        Err(_) => GitRan { code: -1, stdout: String::new(), stderr: String::new() },
    }
}

/// Files changed or untracked in `folder` (teleport refuses either); None when it is not a git checkout.
fn git_changes_in(folder: &str) -> Option<usize> {
    let out = std::process::Command::new("git")
        .args(["status", "--porcelain"])
        .env(crate::git_quiet::NO_OPTIONAL_LOCKS.0, crate::git_quiet::NO_OPTIONAL_LOCKS.1)
        .current_dir(folder)
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(
        String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count(),
    )
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

#[path = "claude_cloud_pty.rs"]
mod pty;
pub use pty::run_in_pseudo_terminal;

#[cfg(all(test, unix))]
#[path = "claude_cloud_pty_tests.rs"]
mod pty_tests;

#[cfg(test)]
#[path = "claude_cloud_send_tests.rs"]
mod send_tests;

#[cfg(test)]
#[path = "claude_cloud_env_tests.rs"]
mod env_tests;

#[cfg(test)]
#[path = "claude_cloud_id_tests.rs"]
mod id_tests;

#[cfg(test)]
#[path = "claude_cloud_read_tests.rs"]
mod read_tests;

#[cfg(test)]
#[path = "claude_cloud_once_tests.rs"]
mod once_tests;

#[cfg(test)]
#[path = "claude_cloud_task_tests.rs"]
mod task_tests;

#[cfg(test)]
#[path = "claude_cloud_exec_tests.rs"]
mod exec_tests;

#[cfg(test)]
#[path = "claude_cloud_detached_tests.rs"]
mod detached_tests;

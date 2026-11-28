//! Cloud computer turns (`apps/desktop/src/main/cloud-turns.ts`,
//! `apps/desktop/src/shared/cloud-computer.ts`, and `applyHere`/`carriedPath`
//! from `apps/desktop/src/main/cloud-computer.ts`):
//! manages a conversation's copy on a repository's cloud computer, keeps its
//! change there until Apply / Discard / Pull Request, and carries only
//! `.locust/attachments/` files into the copy.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use chrono::{DateTime, Months, Utc};
use regex::Regex;
use serde_json::{Value, json};

use crate::cloud_tasks::{Ran, Runner, folder_on_github};
use crate::folder_commit::branch_from_subject;

pub const CLOUD_TASK_ROOT: &str = "/workspaces/.locust";
const MAX_TURNS: usize = 500;
const READY_FOR: Duration = Duration::from_secs(10 * 60);
const CLOUD_IDLE_MS: i64 = 600_000;

fn version_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\d+\.\d+\.\d+$").expect("version regex"))
}

fn task_id_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^t_[a-z0-9]{6,32}$").expect("task id regex"))
}

pub fn safe_branch_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('-')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'/' | b'-'))
}

pub fn cloud_task_dir(task_id: &str) -> Result<String, String> {
    if !task_id_re().is_match(task_id) {
        return Err("That is not a task Locust made.".into());
    }
    Ok(format!("{CLOUD_TASK_ROOT}/{task_id}"))
}

/// Under `.locust/attachments/` and nowhere else, with no step back out.
pub fn carried_path(path: &str) -> bool {
    const PREFIX: &str = ".locust/attachments/";
    if !path.starts_with(PREFIX) || path.contains('\\') {
        return false;
    }
    path[PREFIX.len()..]
        .split('/')
        .all(|part| !part.is_empty() && part != "." && part != "..")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudComputerState {
    Available,
    Starting,
    Stopped,
    Other,
}

pub fn cloud_state_of(state: &str) -> CloudComputerState {
    match state.to_ascii_lowercase().as_str() {
        "available" => CloudComputerState::Available,
        "shutdown" | "shuttingdown" | "archived" => CloudComputerState::Stopped,
        "starting" | "provisioning" | "queued" | "created" | "rebuilding" => {
            CloudComputerState::Starting
        }
        _ => CloudComputerState::Other,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudAgent {
    Claude,
    Codex,
}

impl CloudAgent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudStartPhase {
    CloudMaking,
    CloudWaking,
    CloudInstalling,
    CloudBringing,
}

impl CloudStartPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CloudMaking => "cloud-making",
            Self::CloudWaking => "cloud-waking",
            Self::CloudInstalling => "cloud-installing",
            Self::CloudBringing => "cloud-bringing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Codespace {
    pub name: String,
    pub display_name: String,
    pub state: String,
    pub repository: String,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudChangeFile {
    pub path: String,
    pub added: u64,
    pub removed: u64,
    pub binary: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudChange {
    pub patch: String,
    pub files: Vec<CloudChangeFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudPullRequest {
    pub number: u64,
    pub url: String,
    pub into: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudTurnState {
    Working,
    There,
    Home,
    Pr,
    None,
    Carried,
    Discarded,
    Failed,
}

impl CloudTurnState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::There => "there",
            Self::Home => "home",
            Self::Pr => "pr",
            Self::None => "none",
            Self::Carried => "carried",
            Self::Discarded => "discarded",
            Self::Failed => "failed",
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "working" => Self::Working,
            "there" => Self::There,
            "home" => Self::Home,
            "pr" => Self::Pr,
            "none" => Self::None,
            "carried" => Self::Carried,
            "discarded" => Self::Discarded,
            _ => Self::Failed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudTurnOutcome {
    Home,
    Pr,
    Discarded,
}

impl CloudTurnOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Pr => "pr",
            Self::Discarded => "discarded",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "home" => Some(Self::Home),
            "pr" => Some(Self::Pr),
            "discarded" => Some(Self::Discarded),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicCloudTurn {
    pub mission_id: String,
    pub codespace: String,
    pub repo: String,
    pub branch: Option<String>,
    pub task_id: String,
    pub state: CloudTurnState,
    pub files: Option<Vec<CloudChangeFile>>,
    pub applied_at: Option<String>,
    pub problem: Option<String>,
    pub pull_request: Option<CloudPullRequest>,
    pub open_pull_request: Option<CloudPullRequest>,
    pub outcome: Option<CloudTurnOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudSetup {
    pub repo: String,
    pub branch: Option<String>,
    pub permission: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudSignIn {
    pub computer: String,
    pub repo: String,
    pub agent: CloudAgent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudPrepared {
    Ok {
        dir: String,
        codespace: String,
        bin: Option<String>,
    },
    Err {
        message: String,
        setup: Option<CloudSetup>,
        sign_in: Option<CloudSignIn>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettleTarget {
    Here,
    PullRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullRequestTarget {
    New {
        from: Option<String>,
        wanted: String,
    },
    Add {
        branch: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushedBranch {
    pub branch: String,
    pub into: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedPullRequest {
    pub number: u64,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarriedFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentInstalled {
    pub version: String,
    pub bin: String,
}

pub trait CloudComputers {
    fn find(&self, repo: &str) -> Result<Option<Codespace>, String>;
    fn make(&self, repo: &str, branch: Option<&str>) -> Result<String, String>;
    fn when_ready(&self, name: &str) -> Result<(), String>;
    fn ensure_open_code(&self, name: &str, version: &str) -> Result<String, String>;
    fn ensure_agent(
        &self,
        name: &str,
        agent: CloudAgent,
        version: &str,
    ) -> Result<AgentInstalled, String>;
    fn agent_signed_in(&self, name: &str, agent: CloudAgent) -> Result<bool, String>;
    fn bring(&self, name: &str, repo: &str, folder: &Path, task_id: &str)
    -> Result<String, String>;
    fn counts(&self, name: &str, task_id: &str, base: &str)
    -> Result<Vec<CloudChangeFile>, String>;
    fn change(&self, name: &str, task_id: &str, base: &str) -> Result<CloudChange, String>;
    fn settle(&self, name: &str, task_id: &str, went: SettleTarget) -> Result<String, String>;
    fn discard(&self, name: &str, task_id: &str, base: &str) -> Result<(), String>;
    fn carry(&self, name: &str, task_id: &str, files: &[CarriedFile]) -> Result<(), String>;
    fn push_change(
        &self,
        name: &str,
        repo: &str,
        task_id: &str,
        base: &str,
        target: &PullRequestTarget,
        message: &str,
    ) -> Result<PushedBranch, String>;
    fn open_pull_request(
        &self,
        repo: &str,
        head: &str,
        base: &str,
        title: &str,
        body: &str,
    ) -> Result<OpenedPullRequest, String>;
    fn pull_request_open(&self, repo: &str, number: u64) -> Result<bool, String>;
    fn drop_branch(&self, repo: &str, branch: &str) -> Result<(), String>;
}

#[derive(Debug, Clone)]
pub struct PrepareInput<'a> {
    pub mission_id: &'a str,
    pub folder: &'a Path,
    pub follows: Option<&'a str>,
    pub open_code_version: Option<&'a str>,
    pub agent: Option<(CloudAgent, Option<&'a str>)>,
    pub set_up: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudTurnAction {
    Ok { turn: PublicCloudTurn },
    Err { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudPullRequestAction {
    Ok { turn: PublicCloudTurn, added: bool },
    Err { message: String },
}

#[derive(Debug, Clone)]
struct StoredPr {
    number: u64,
    url: String,
    branch: String,
    into: String,
}

#[derive(Debug, Clone)]
struct StoredTask {
    task_id: String,
    codespace: String,
    repo: String,
    branch: Option<String>,
    folder: String,
    base: String,
    created_at: String,
    pull_request: Option<StoredPr>,
}

#[derive(Debug, Clone)]
struct StoredTurn {
    mission_id: String,
    task_id: String,
    began: Option<String>,
    state: CloudTurnState,
    files: Option<Vec<CloudChangeFile>>,
    applied_at: Option<String>,
    problem: Option<String>,
    pull_request: Option<CloudPullRequest>,
    outcome: Option<CloudTurnOutcome>,
    at: String,
}

#[derive(Debug, Clone, Default)]
struct CloudStore {
    tasks: BTreeMap<String, StoredTask>,
    turns: BTreeMap<String, StoredTurn>,
    usage: UsageMonths,
    use_since: Option<String>,
}

type UsageSpan = (String, String);
type UsageMonths = BTreeMap<String, BTreeMap<String, Vec<UsageSpan>>>;

#[derive(Debug, Clone, PartialEq)]
pub struct CloudUsage {
    pub hours: f64,
    pub words: String,
    pub by_computer: BTreeMap<String, f64>,
    pub since: Option<String>,
}

fn time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value).ok().map(|t| t.with_timezone(&Utc))
}

fn cloud_month(at: DateTime<Utc>) -> String {
    at.format("%Y-%m").to_string()
}

fn hours(spans: &[UsageSpan], now: DateTime<Utc>) -> f64 {
    let mut spans: Vec<_> = spans.iter().filter_map(|(from, to)| {
        let from = time(from)?.timestamp_millis();
        let to = time(to)?.timestamp_millis();
        (to >= from).then_some((from, to))
    }).collect();
    spans.sort_unstable();
    let until = now.timestamp_millis();
    let mut open: Option<(i64, i64)> = None;
    let mut counted = 0_i64;
    for (from, to) in spans {
        let end = to.saturating_add(CLOUD_IDLE_MS).min(until).max(from);
        if let Some((start, last)) = open {
            if from <= last {
                open = Some((start, last.max(end)));
                continue;
            }
            counted += last - start;
        }
        open = Some((from, end));
    }
    if let Some((from, to)) = open {
        counted += to - from;
    }
    counted as f64 / 3_600_000.0
}

fn hours_words(hours: f64) -> String {
    if hours < 0.1 {
        "a few minutes".into()
    } else if hours < 1.0 {
        format!("{} minutes", (hours * 60.0).round())
    } else if hours < 10.0 {
        format!("{:.1} hours", (hours * 10.0).round() / 10.0)
    } else {
        format!("{} hours", hours.round())
    }
}

/// Locust's recorded time, not GitHub's bill. Reading it never wakes a provider.
pub fn recorded_usage(file: &Path, now: DateTime<Utc>) -> Option<CloudUsage> {
    let store = read_store_at(file);
    let month = cloud_month(now);
    let computers = store.usage.get(&month)?;
    if computers.is_empty() {
        return None;
    }
    let by_computer: BTreeMap<_, _> = computers.iter().map(|(name, spans)| {
        (name.clone(), hours(spans, now))
    }).collect();
    let hours = by_computer.values().sum();
    Some(CloudUsage {
        hours,
        words: hours_words(hours),
        by_computer,
        since: store.use_since.filter(|s| time(s).is_some_and(|at| cloud_month(at) == month)),
    })
}

#[derive(Debug, Clone)]
struct ReadyEntry {
    version: String,
    at: Instant,
    bin: Option<String>,
}

/// Applies a patch to `folder` via `--check --binary` then `--binary`, cleaning up the temp patch file.
pub fn apply_here(
    folder: &Path,
    patch: &str,
    git_run: &dyn Fn(&[&str], &Path) -> Result<Ran, String>,
) -> Result<(), String> {
    if patch.trim().is_empty() {
        return Ok(());
    }
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let patch_file =
        std::env::temp_dir().join(format!("locust-cloud-{}-{n}.patch", std::process::id()));
    fs::write(&patch_file, patch).map_err(|e| e.to_string())?;
    let path_str = patch_file.to_string_lossy().to_string();
    let result = (|| {
        let check = git_run(&["apply", "--check", "--binary", &path_str], folder)?;
        if check.code != 0 {
            let why = last_word(&check);
            return Err(format!(
                "It no longer fits this folder: {why}. Show the change to take what you need by hand."
            ));
        }
        let applied = git_run(&["apply", "--binary", &path_str], folder)?;
        if applied.code != 0 {
            let why = last_word(&applied);
            return Err(format!("It could not be applied: {why}"));
        }
        Ok(())
    })();
    let _ = fs::remove_file(&patch_file);
    result
}

fn last_word(ran: &Ran) -> String {
    let combined = format!("{}\n{}", ran.stdout, ran.stderr);
    combined
        .lines()
        .map(str::trim)
        .filter(|part| {
            !part.is_empty() && !part.to_ascii_lowercase().starts_with("shell closed: exit status")
        })
        .last()
        .unwrap_or("It did not say why.")
        .chars()
        .take(300)
        .collect()
}

fn read_store_at(file: &Path) -> CloudStore {
    let Ok(text) = fs::read_to_string(file) else {
        return CloudStore::default();
    };
    let Ok(val) = serde_json::from_str::<Value>(&text) else {
        return CloudStore::default();
    };
    store_from_json(&val)
}

/// `of`, read from the store file alone, for a reader with no cloud computer at hand: the turn's task and codespace
/// when it was sent to the cloud computer (the context menu leaves such a turn out, 0.761).
pub fn recorded_of(file: &Path, mission_id: &str) -> Option<(String, String)> {
    let store = read_store_at(file);
    let turn = store.turns.get(mission_id)?;
    let task = store.tasks.get(&turn.task_id)?;
    Some((task.task_id.clone(), task.codespace.clone()))
}

pub struct CloudTurns<'a, C: CloudComputers> {
    file: PathBuf,
    computers: &'a C,
    git: Runner<'a>,
    git_run: Runner<'a>,
    on_change: Option<Box<dyn Fn(&PublicCloudTurn) + 'a>>,
    now: Box<dyn Fn() -> String + 'a>,
    ready: Mutex<HashMap<String, ReadyEntry>>,
    store_lock: Mutex<()>,
}

impl<'a, C: CloudComputers> CloudTurns<'a, C> {
    pub fn new(
        file: impl Into<PathBuf>,
        computers: &'a C,
        git: Runner<'a>,
        git_run: Runner<'a>,
    ) -> Self {
        Self {
            file: file.into(),
            computers,
            git,
            git_run,
            on_change: None,
            now: Box::new(|| Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
            ready: Mutex::new(HashMap::new()),
            store_lock: Mutex::new(()),
        }
    }

    pub fn with_on_change(mut self, f: impl Fn(&PublicCloudTurn) + 'a) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    pub fn with_now(mut self, now: impl Fn() -> String + 'a) -> Self {
        self.now = Box::new(now);
        self
    }

    fn read_store(&self) -> CloudStore {
        read_store_at(&self.file)
    }

    /// Count a stretch before telling its caller the remote step is done, even if the step was refused.
    fn on_computer<T>(&self, name: &str, work: impl FnOnce() -> T) -> T {
        let from = (self.now)();
        let result = work();
        self.note_use(name, &from, &(self.now)());
        result
    }

    fn note_use(&self, name: &str, from: &str, to: &str) {
        let Some((begin, end)) = time(from).zip(time(to)) else { return };
        if name.is_empty() || begin > end {
            return;
        }
        let month = cloud_month(begin);
        let _guard = self.store_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut store = self.read_store();
        let newest = store.usage.keys().next_back().map_or(month.as_str(), |s| s.as_str().max(month.as_str()));
        let newest = newest.to_owned();
        let previous = time(&format!("{newest}-01T00:00:00Z"))
            .and_then(|at| at.checked_sub_months(Months::new(1))).map(cloud_month);
        if month != newest && previous.as_deref() != Some(&month) {
            return; // A late receipt must not evict newer months.
        }
        store.usage.retain(|key, _| key == &newest || previous.as_ref() == Some(key));
        let spans = store.usage.entry(month).or_default().entry(name.to_owned()).or_default();
        let iso = |at: DateTime<Utc>| at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        if let Some((last_from, last_to)) = spans.last_mut()
            && let Some((a, b)) = time(last_from).zip(time(last_to))
            && b.timestamp_millis().saturating_add(CLOUD_IDLE_MS) >= begin.timestamp_millis()
            && end.timestamp_millis().saturating_add(CLOUD_IDLE_MS) >= a.timestamp_millis()
        {
            *last_from = iso(a.min(begin));
            *last_to = iso(b.max(end));
        } else {
            spans.push((iso(begin), iso(end)));
        }
        if store.use_since.as_deref().and_then(time).is_none_or(|at| begin < at) {
            store.use_since = Some(iso(begin));
        }
        let _ = self.write_store(&store);
    }

    fn write_store(&self, store: &CloudStore) -> Result<(), String> {
        let mut turns: Vec<StoredTurn> = store.turns.values().cloned().collect();
        turns.sort_by(|a, b| a.at.cmp(&b.at));
        if turns.len() > MAX_TURNS {
            turns = turns.split_off(turns.len() - MAX_TURNS);
        }
        let kept_tasks: std::collections::HashSet<&str> =
            turns.iter().map(|t| t.task_id.as_str()).collect();
        let trimmed = CloudStore {
            tasks: store
                .tasks
                .iter()
                .filter(|(k, _)| kept_tasks.contains(k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            turns: turns
                .into_iter()
                .map(|t| (t.mission_id.clone(), t))
                .collect(),
            usage: store.usage.clone(),
            use_since: store.use_since.clone(),
        };
        if let Some(parent) = self.file.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let body = serde_json::to_string_pretty(&store_to_json(&trimmed))
            .map_err(|e| e.to_string())?;
        static TMP_SEQ: AtomicU64 = AtomicU64::new(0);
        let n = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
        let mut tmp = self.file.as_os_str().to_os_string();
        tmp.push(format!(".{}.{n}.tmp", std::process::id()));
        let tmp_path = PathBuf::from(tmp);
        fs::write(&tmp_path, body).map_err(|e| e.to_string())?;
        fs::rename(&tmp_path, &self.file).map_err(|e| {
            let _ = fs::remove_file(&tmp_path);
            e.to_string()
        })
    }

    fn tell(&self, store: &CloudStore, mission_id: &str) {
        if let Some(cb) = &self.on_change
            && let Some(turn) = store.turns.get(mission_id)
            && let Some(shown) = public_turn(store, turn)
        {
            cb(&shown);
        }
    }

    pub fn of(&self, mission_id: &str) -> Option<(String, String)> {
        recorded_of(&self.file, mission_id)
    }

    pub fn prepare(
        &self,
        input: PrepareInput<'_>,
        say: &mut dyn FnMut(CloudStartPhase),
    ) -> CloudPrepared {
        let began = (self.now)();
        let (agent_rt, version_opt) = match input.agent {
            Some((rt, v)) => (Some(rt), v),
            None => (None, input.open_code_version),
        };
        let version = match version_opt {
            Some(v) if version_re().is_match(v) => v,
            _ => {
                let who = agent_rt.map(CloudAgent::display_name).unwrap_or("OpenCode");
                return CloudPrepared::Err {
                    message: format!(
                        "Locust could not tell which {who} is installed here, so it cannot put the same one on your cloud computer."
                    ),
                    setup: None,
                    sign_in: None,
                };
            }
        };

        let where_gh = folder_on_github(input.folder, self.git);
        let Some(repo) = where_gh.repo else {
            let name = input
                .folder
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("This folder");
            return CloudPrepared::Err {
                message: format!(
                    "{name} is not on GitHub. Your cloud computer works on a copy of a GitHub repository: push this folder to GitHub first, or open one that is."
                ),
                setup: None,
                sign_in: None,
            };
        };
        let branch = where_gh.branch;
        let folder_str = input.folder.to_string_lossy().to_string();

        let store = self.read_store();
        let earlier = input
            .follows
            .and_then(|f| store.turns.get(f))
            .and_then(|t| store.tasks.get(&t.task_id))
            .cloned();

        let mut space = match self.computers.find(&repo) {
            Ok(Some(s)) => s,
            Ok(None) => {
                if !input.set_up {
                    return CloudPrepared::Err {
                        message: "Your cloud computer for this repository is not set up yet."
                            .into(),
                        setup: Some(CloudSetup {
                            repo,
                            branch,
                            permission: false,
                        }),
                        sign_in: None,
                    };
                }
                say(CloudStartPhase::CloudMaking);
                let name = match self.computers.make(&repo, branch.as_deref()) {
                    Ok(n) => n,
                    Err(msg) => {
                        return CloudPrepared::Err {
                            message: msg,
                            setup: None,
                            sign_in: None,
                        };
                    }
                };
                Codespace {
                    name,
                    display_name: String::new(),
                    state: "Starting".into(),
                    repository: repo.clone(),
                    last_used_at: None,
                }
            }
            Err(msg) => {
                return CloudPrepared::Err {
                    message: msg,
                    setup: None,
                    sign_in: None,
                };
            }
        };

        if cloud_state_of(&space.state) != CloudComputerState::Available
            && !space.display_name.is_empty()
        {
            say(CloudStartPhase::CloudWaking);
        }

        let key = match agent_rt {
            Some(rt) => format!("{}/{}", space.name, rt.as_str()),
            None => space.name.clone(),
        };
        let cached = self
            .ready
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
            .cloned();
        let needs_check = match &cached {
            None => true,
            Some(c) => {
                c.version != version
                    || c.at.elapsed() > READY_FOR
                    || cloud_state_of(&space.state) != CloudComputerState::Available
                    || (agent_rt.is_some() && c.bin.is_none())
            }
        };
        let bin = if needs_check {
            if let Err(msg) = self.on_computer(&space.name, || self.computers.when_ready(&space.name)) {
                self.ready.lock().unwrap_or_else(|e| e.into_inner()).clear();
                return CloudPrepared::Err {
                    message: msg,
                    setup: None,
                    sign_in: None,
                };
            }
            say(CloudStartPhase::CloudInstalling);
            let bin = if let Some(rt) = agent_rt {
                let installed = match self.on_computer(&space.name, || self.computers.ensure_agent(&space.name, rt, version)) {
                    Ok(i) => i,
                    Err(msg) => {
                        return CloudPrepared::Err {
                            message: msg,
                            setup: None,
                            sign_in: None,
                        };
                    }
                };
                let signed_in = match self.on_computer(&space.name, || self.computers.agent_signed_in(&space.name, rt)) {
                    Ok(s) => s,
                    Err(msg) => {
                        return CloudPrepared::Err {
                            message: msg,
                            setup: None,
                            sign_in: None,
                        };
                    }
                };
                if !signed_in {
                    return CloudPrepared::Err {
                        message: format!(
                            "{} is not signed in on your cloud computer yet.",
                            rt.display_name()
                        ),
                        setup: None,
                        sign_in: Some(CloudSignIn {
                            computer: space.name,
                            repo,
                            agent: rt,
                        }),
                    };
                }
                Some(installed.bin)
            } else {
                if let Err(msg) = self.on_computer(&space.name, || self.computers.ensure_open_code(&space.name, version)) {
                    return CloudPrepared::Err {
                        message: msg,
                        setup: None,
                        sign_in: None,
                    };
                }
                None
            };
            space.state = "Available".into();
            self.ready.lock().unwrap_or_else(|e| e.into_inner()).insert(
                key,
                ReadyEntry {
                    version: version.into(),
                    at: Instant::now(),
                    bin: bin.clone(),
                },
            );
            bin
        } else {
            cached.and_then(|c| c.bin)
        };

        let task = match earlier {
            Some(prev) if prev.codespace == space.name && prev.folder == folder_str => prev,
            _ => {
                say(CloudStartPhase::CloudBringing);
                static TASK_SEQ: AtomicU64 = AtomicU64::new(1);
                let n = TASK_SEQ.fetch_add(1, Ordering::Relaxed);
                let task_id = format!("t_{n:012x}");
                let base = match self.on_computer(&space.name, || self.computers.bring(&space.name, &repo, input.folder, &task_id)) {
                    Ok(b) => b,
                    Err(msg) => {
                        return CloudPrepared::Err {
                            message: msg,
                            setup: None,
                            sign_in: None,
                        };
                    }
                };
                StoredTask {
                    task_id,
                    codespace: space.name.clone(),
                    repo,
                    branch,
                    folder: folder_str,
                    base,
                    created_at: (self.now)(),
                    pull_request: None,
                }
            }
        };

        {
            let _guard = self.store_lock.lock().unwrap_or_else(|e| e.into_inner());
            let mut current = self.read_store();
            current.tasks.insert(task.task_id.clone(), task.clone());
            current.turns.insert(
                input.mission_id.to_string(),
                StoredTurn {
                    mission_id: input.mission_id.to_string(),
                    task_id: task.task_id.clone(),
                    began: Some(began),
                    state: CloudTurnState::Working,
                    files: None,
                    applied_at: None,
                    problem: None,
                    pull_request: None,
                    outcome: None,
                    at: (self.now)(),
                },
            );
            let _ = self.write_store(&current);
            self.tell(&current, input.mission_id);
        }

        let dir = match cloud_task_dir(&task.task_id) {
            Ok(d) => d,
            Err(message) => {
                return CloudPrepared::Err {
                    message,
                    setup: None,
                    sign_in: None,
                };
            }
        };
        CloudPrepared::Ok {
            dir,
            codespace: space.name,
            bin,
        }
    }

    pub fn settled(&self, mission_id: &str) {
        let store = self.read_store();
        let Some(turn) = store.turns.get(mission_id).cloned() else {
            return;
        };
        let Some(task) = store.tasks.get(&turn.task_id).cloned() else {
            return;
        };
        if turn.state != CloudTurnState::Working {
            return;
        }
        if let Some(began) = &turn.began {
            self.note_use(&task.codespace, began, &(self.now)());
        }
        let next = match self.on_computer(&task.codespace, || {
            self.computers.counts(&task.codespace, &task.task_id, &task.base)
        }) {
            Ok(files) => StoredTurn {
                state: if files.is_empty() {
                    CloudTurnState::None
                } else {
                    CloudTurnState::There
                },
                files: Some(files),
                at: (self.now)(),
                ..turn
            },
            Err(msg) => StoredTurn {
                state: CloudTurnState::Failed,
                problem: Some(msg),
                at: (self.now)(),
                ..turn
            },
        };
        let _guard = self.store_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut current = self.read_store();
        let mut carried_ids = Vec::new();
        for other in current.turns.values_mut() {
            if other.task_id == task.task_id
                && other.mission_id != mission_id
                && other.state == CloudTurnState::There
            {
                other.state = CloudTurnState::Carried;
                carried_ids.push(other.mission_id.clone());
            }
        }
        current.turns.insert(mission_id.to_string(), next);
        let _ = self.write_store(&current);
        for other_id in &carried_ids {
            self.tell(&current, other_id);
        }
        self.tell(&current, mission_id);
    }

    pub fn turns(&self, mission_ids: &[&str]) -> Vec<PublicCloudTurn> {
        let store = self.read_store();
        mission_ids
            .iter()
            .filter_map(|id| {
                let turn = store.turns.get(*id)?;
                public_turn(&store, turn)
            })
            .collect()
    }

    pub fn apply(&self, mission_id: &str) -> CloudTurnAction {
        let store = self.read_store();
        let Some(turn) = store.turns.get(mission_id).cloned() else {
            return CloudTurnAction::Err {
                message: "That change is not waiting on your cloud computer any more.".into(),
            };
        };
        let Some(task) = store.tasks.get(&turn.task_id).cloned() else {
            return CloudTurnAction::Err {
                message: "That change is not waiting on your cloud computer any more.".into(),
            };
        };
        if turn.state != CloudTurnState::There {
            return CloudTurnAction::Err {
                message: "That change is not waiting on your cloud computer any more.".into(),
            };
        }
        let found = match self.on_computer(&task.codespace, || {
            self.computers.change(&task.codespace, &task.task_id, &task.base)
        }) {
            Ok(c) => c,
            Err(message) => return CloudTurnAction::Err { message },
        };
        if let Err(message) = apply_here(Path::new(&task.folder), &found.patch, self.git_run) {
            return CloudTurnAction::Err { message };
        }
        let new_base = match self.on_computer(&task.codespace, || {
            self.computers.settle(&task.codespace, &task.task_id, SettleTarget::Here)
        }) {
            Ok(b) => b,
            Err(message) => return CloudTurnAction::Err { message },
        };

        let _guard = self.store_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut current = self.read_store();
        let resolved_ids = resolve_carried(&mut current, &task.task_id, CloudTurnOutcome::Home);
        if let Some(stored_task) = current.tasks.get_mut(&task.task_id) {
            stored_task.base = new_base;
        }
        let now_str = (self.now)();
        current.turns.insert(
            mission_id.to_string(),
            StoredTurn {
                state: CloudTurnState::Home,
                files: Some(found.files),
                applied_at: Some(now_str.clone()),
                at: now_str,
                ..turn
            },
        );
        let _ = self.write_store(&current);
        for other_id in &resolved_ids {
            self.tell(&current, other_id);
        }
        self.tell(&current, mission_id);
        let shown = public_turn(&current, &current.turns[mission_id]).expect("turn exists");
        CloudTurnAction::Ok { turn: shown }
    }

    pub fn discard(&self, mission_id: &str) -> CloudTurnAction {
        let store = self.read_store();
        let Some(turn) = store.turns.get(mission_id).cloned() else {
            return CloudTurnAction::Err {
                message: "That change is not waiting on your cloud computer any more.".into(),
            };
        };
        let Some(task) = store.tasks.get(&turn.task_id).cloned() else {
            return CloudTurnAction::Err {
                message: "That change is not waiting on your cloud computer any more.".into(),
            };
        };
        if turn.state != CloudTurnState::There {
            return CloudTurnAction::Err {
                message: "That change is not waiting on your cloud computer any more.".into(),
            };
        }
        if let Err(message) = self.on_computer(&task.codespace, || {
            self.computers.discard(&task.codespace, &task.task_id, &task.base)
        }) {
            return CloudTurnAction::Err { message };
        }
        let _guard = self.store_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut current = self.read_store();
        let resolved_ids =
            resolve_carried(&mut current, &task.task_id, CloudTurnOutcome::Discarded);
        current.turns.insert(
            mission_id.to_string(),
            StoredTurn {
                state: CloudTurnState::Discarded,
                at: (self.now)(),
                ..turn
            },
        );
        let _ = self.write_store(&current);
        for other_id in &resolved_ids {
            self.tell(&current, other_id);
        }
        self.tell(&current, mission_id);
        let shown = public_turn(&current, &current.turns[mission_id]).expect("turn exists");
        CloudTurnAction::Ok { turn: shown }
    }

    pub fn pull_request(&self, mission_id: &str, message: &str) -> CloudPullRequestAction {
        let store = self.read_store();
        let Some(turn) = store.turns.get(mission_id).cloned() else {
            return CloudPullRequestAction::Err {
                message: "That change is not waiting on your cloud computer any more.".into(),
            };
        };
        let Some(task) = store.tasks.get(&turn.task_id).cloned() else {
            return CloudPullRequestAction::Err {
                message: "That change is not waiting on your cloud computer any more.".into(),
            };
        };
        if turn.state != CloudTurnState::There {
            return CloudPullRequestAction::Err {
                message: "That change is not waiting on your cloud computer any more.".into(),
            };
        }
        let text = message.trim();
        let mut lines = text.split('\n');
        let subject = lines.next().unwrap_or("").trim();
        if subject.is_empty() {
            return CloudPullRequestAction::Err {
                message: "A pull request needs a title: the message's first line.".into(),
            };
        }
        let rest_body = lines.collect::<Vec<_>>().join("\n");
        let body = rest_body.trim();

        let open = match &task.pull_request {
            Some(pr) => match self.computers.pull_request_open(&task.repo, pr.number) {
                Ok(true) => Some(pr.clone()),
                Ok(false) => None,
                Err(message) => return CloudPullRequestAction::Err { message },
            },
            None => None,
        };
        let from = task
            .branch
            .as_deref()
            .filter(|b| safe_branch_name(b))
            .map(str::to_string);
        let target = match &open {
            None => PullRequestTarget::New {
                from,
                wanted: branch_from_subject(subject),
            },
            Some(pr) => PullRequestTarget::Add {
                branch: pr.branch.clone(),
            },
        };
        let pushed = match self.on_computer(&task.codespace, || self.computers.push_change(
            &task.codespace,
            &task.repo,
            &task.task_id,
            &task.base,
            &target,
            text,
        )) {
            Ok(p) => p,
            Err(message) => return CloudPullRequestAction::Err { message },
        };

        let request = match open {
            Some(ref pr) => pr.clone(),
            None => {
                let pr_body = if body.is_empty() { subject } else { body };
                match self.computers.open_pull_request(
                    &task.repo,
                    &pushed.branch,
                    &pushed.into,
                    subject,
                    pr_body,
                ) {
                    Ok(opened) => StoredPr {
                        number: opened.number,
                        url: opened.url,
                        branch: pushed.branch,
                        into: pushed.into,
                    },
                    Err(message) => {
                        let _ = self.computers.drop_branch(&task.repo, &pushed.branch);
                        return CloudPullRequestAction::Err { message };
                    }
                }
            }
        };

        let new_base = match self.on_computer(&task.codespace, || {
            self.computers.settle(&task.codespace, &task.task_id, SettleTarget::PullRequest)
        }) {
            Ok(b) => b,
            Err(message) => return CloudPullRequestAction::Err { message },
        };

        let _guard = self.store_lock.lock().unwrap_or_else(|e| e.into_inner());
        let mut current = self.read_store();
        let resolved_ids = resolve_carried(&mut current, &task.task_id, CloudTurnOutcome::Pr);
        if let Some(stored_task) = current.tasks.get_mut(&task.task_id) {
            stored_task.base = new_base;
            stored_task.pull_request = Some(request.clone());
        }
        current.turns.insert(
            mission_id.to_string(),
            StoredTurn {
                state: CloudTurnState::Pr,
                pull_request: Some(CloudPullRequest {
                    number: request.number,
                    url: request.url,
                    into: Some(request.into),
                }),
                at: (self.now)(),
                ..turn
            },
        );
        let _ = self.write_store(&current);
        for other_id in &resolved_ids {
            self.tell(&current, other_id);
        }
        self.tell(&current, mission_id);
        let shown = public_turn(&current, &current.turns[mission_id]).expect("turn exists");
        CloudPullRequestAction::Ok {
            turn: shown,
            added: open.is_some(),
        }
    }

    pub fn carry(&self, mission_id: &str, folder: &Path, files: &[&str]) {
        let wanted: Vec<&str> = files.iter().copied().filter(|p| carried_path(p)).collect();
        if wanted.is_empty() {
            return;
        }
        let store = self.read_store();
        let Some(turn) = store.turns.get(mission_id) else {
            return;
        };
        let Some(task) = store.tasks.get(&turn.task_id) else {
            return;
        };
        let mut loaded = Vec::new();
        for rel in wanted {
            let mut full = folder.to_path_buf();
            for part in rel.split('/') {
                full.push(part);
            }
            if let Ok(bytes) = fs::read(&full) {
                loaded.push(CarriedFile {
                    path: rel.to_string(),
                    bytes,
                });
            }
        }
        let _ = self.on_computer(&task.codespace, || {
            self.computers.carry(&task.codespace, &task.task_id, &loaded)
        });
    }
}

fn resolve_carried(
    store: &mut CloudStore,
    task_id: &str,
    outcome: CloudTurnOutcome,
) -> Vec<String> {
    let mut told = Vec::new();
    for other in store.turns.values_mut() {
        if other.task_id == task_id
            && other.state == CloudTurnState::Carried
            && other.outcome.is_none()
        {
            other.outcome = Some(outcome);
            told.push(other.mission_id.clone());
        }
    }
    told
}

fn public_turn(store: &CloudStore, turn: &StoredTurn) -> Option<PublicCloudTurn> {
    let task = store.tasks.get(&turn.task_id)?;
    let open_pull_request = if turn.state == CloudTurnState::There {
        task.pull_request.as_ref().map(|pr| CloudPullRequest {
            number: pr.number,
            url: pr.url.clone(),
            into: None,
        })
    } else {
        None
    };
    Some(PublicCloudTurn {
        mission_id: turn.mission_id.clone(),
        codespace: task.codespace.clone(),
        repo: task.repo.clone(),
        branch: task.branch.clone(),
        task_id: task.task_id.clone(),
        state: turn.state,
        files: turn.files.clone(),
        applied_at: turn.applied_at.clone(),
        problem: turn.problem.clone(),
        pull_request: turn.pull_request.clone(),
        open_pull_request,
        outcome: turn.outcome,
    })
}

fn store_to_json(store: &CloudStore) -> Value {
    let mut tasks = serde_json::Map::new();
    for (k, t) in &store.tasks {
        let mut obj = serde_json::Map::new();
        obj.insert("taskId".into(), json!(t.task_id));
        obj.insert("codespace".into(), json!(t.codespace));
        obj.insert("repo".into(), json!(t.repo));
        if let Some(b) = &t.branch {
            obj.insert("branch".into(), json!(b));
        }
        obj.insert("folder".into(), json!(t.folder));
        obj.insert("base".into(), json!(t.base));
        obj.insert("createdAt".into(), json!(t.created_at));
        if let Some(pr) = &t.pull_request {
            obj.insert(
                "pullRequest".into(),
                json!({
                    "number": pr.number,
                    "url": pr.url,
                    "branch": pr.branch,
                    "into": pr.into,
                }),
            );
        }
        tasks.insert(k.clone(), Value::Object(obj));
    }

    let mut turns = serde_json::Map::new();
    for (k, t) in &store.turns {
        let mut obj = serde_json::Map::new();
        obj.insert("missionId".into(), json!(t.mission_id));
        obj.insert("taskId".into(), json!(t.task_id));
        if let Some(b) = &t.began {
            obj.insert("began".into(), json!(b));
        }
        obj.insert("state".into(), json!(t.state.as_str()));
        if let Some(files) = &t.files {
            let arr: Vec<Value> = files
                .iter()
                .map(|f| {
                    let mut m = serde_json::Map::new();
                    m.insert("path".into(), json!(f.path));
                    m.insert("added".into(), json!(f.added));
                    m.insert("removed".into(), json!(f.removed));
                    if f.binary {
                        m.insert("binary".into(), json!(true));
                    }
                    Value::Object(m)
                })
                .collect();
            obj.insert("files".into(), Value::Array(arr));
        }
        if let Some(a) = &t.applied_at {
            obj.insert("appliedAt".into(), json!(a));
        }
        if let Some(p) = &t.problem {
            obj.insert("problem".into(), json!(p));
        }
        if let Some(pr) = &t.pull_request {
            let mut m = serde_json::Map::new();
            m.insert("number".into(), json!(pr.number));
            m.insert("url".into(), json!(pr.url));
            if let Some(into) = &pr.into {
                m.insert("into".into(), json!(into));
            }
            obj.insert("pullRequest".into(), Value::Object(m));
        }
        if let Some(o) = t.outcome {
            obj.insert("outcome".into(), json!(o.as_str()));
        }
        obj.insert("at".into(), json!(t.at));
        turns.insert(k.clone(), Value::Object(obj));
    }

    let mut out = json!({
        "tasks": Value::Object(tasks),
        "turns": Value::Object(turns),
        "use": store.usage,
    });
    if let Some(since) = &store.use_since {
        out["useSince"] = json!(since);
    }
    out
}

fn store_from_json(value: &Value) -> CloudStore {
    let mut store = CloudStore {
        usage: value.get("use").cloned().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default(),
        use_since: value.get("useSince").and_then(Value::as_str).map(str::to_owned),
        ..CloudStore::default()
    };
    if let Some(tasks_obj) = value.get("tasks").and_then(Value::as_object) {
        for (k, v) in tasks_obj {
            let Some(obj) = v.as_object() else { continue };
            let pr = obj.get("pullRequest").and_then(Value::as_object).and_then(|p| {
                Some(StoredPr {
                    number: p.get("number")?.as_u64()?,
                    url: p.get("url")?.as_str()?.to_string(),
                    branch: p.get("branch")?.as_str()?.to_string(),
                    into: p.get("into")?.as_str()?.to_string(),
                })
            });
            store.tasks.insert(
                k.clone(),
                StoredTask {
                    task_id: obj.get("taskId").and_then(Value::as_str).unwrap_or(k).into(),
                    codespace: obj
                        .get("codespace")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .into(),
                    repo: obj
                        .get("repo")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .into(),
                    branch: obj
                        .get("branch")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    folder: obj
                        .get("folder")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .into(),
                    base: obj
                        .get("base")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .into(),
                    created_at: obj
                        .get("createdAt")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .into(),
                    pull_request: pr,
                },
            );
        }
    }
    if let Some(turns_obj) = value.get("turns").and_then(Value::as_object) {
        for (k, v) in turns_obj {
            let Some(obj) = v.as_object() else { continue };
            let files = obj.get("files").and_then(Value::as_array).map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let m = item.as_object()?;
                        Some(CloudChangeFile {
                            path: m.get("path")?.as_str()?.to_string(),
                            added: m.get("added")?.as_u64()?,
                            removed: m.get("removed")?.as_u64()?,
                            binary: m.get("binary").and_then(Value::as_bool).unwrap_or(false),
                        })
                    })
                    .collect()
            });
            let pr = obj.get("pullRequest").and_then(Value::as_object).and_then(|p| {
                Some(CloudPullRequest {
                    number: p.get("number")?.as_u64()?,
                    url: p.get("url")?.as_str()?.to_string(),
                    into: p.get("into").and_then(Value::as_str).map(str::to_string),
                })
            });
            store.turns.insert(
                k.clone(),
                StoredTurn {
                    mission_id: obj
                        .get("missionId")
                        .and_then(Value::as_str)
                        .unwrap_or(k)
                        .into(),
                    task_id: obj
                        .get("taskId")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .into(),
                    began: obj.get("began").and_then(Value::as_str).map(str::to_string),
                    state: obj
                        .get("state")
                        .and_then(Value::as_str)
                        .map(CloudTurnState::from_str)
                        .unwrap_or(CloudTurnState::Failed),
                    files,
                    applied_at: obj
                        .get("appliedAt")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    problem: obj
                        .get("problem")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    pull_request: pr,
                    outcome: obj
                        .get("outcome")
                        .and_then(Value::as_str)
                        .and_then(CloudTurnOutcome::from_str),
                    at: obj
                        .get("at")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .into(),
                },
            );
        }
    }
    store
}

#[cfg(test)]
#[path = "cloud_turns_tests.rs"]
mod tests;

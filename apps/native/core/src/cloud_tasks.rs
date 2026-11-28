//! Codex cloud tasks (`apps/desktop/src/main/cloud-tasks.ts`): reading what
//! `codex cloud` and `git` say about a folder, its environment, a started task,
//! and a task's status.

use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use regex::Regex;

/// A command's result; `code == 0` is success.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ran {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Ran {
    pub fn ok(stdout: impl Into<String>) -> Self {
        Self {
            code: 0,
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    pub fn new(code: i32, stdout: impl Into<String>, stderr: impl Into<String>) -> Self {
        Self {
            code,
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }
}

/// Command runner seam: every `codex cloud` and `git` call goes through this.
pub type Runner<'a> = &'a dyn Fn(&[&str], &Path) -> Result<Ran, String>;

fn github_remote_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(?:https?://(?:[^@/]+@)?github\.com/|git@github\.com:|ssh://git@github\.com/)([A-Za-z0-9_.-]+)/([A-Za-z0-9_.-]+?)(?:\.git)?/?$")
            .expect("github remote regex")
    })
}

fn task_url_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"https://chatgpt\.com/codex/tasks/(task_[A-Za-z0-9_]+)").expect("task url regex")
    })
}

fn no_env_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)no cloud environments are available|environment '.*' not found")
            .expect("no env regex")
    })
}

fn config_warn_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)unrecognized configuration|is ignored").expect("config warn regex")
    })
}

fn status_head_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^\s*\[([A-Z_]+)\]\s*").expect("status head regex"))
}

fn status_counts_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\+(\d+)/-(\d+)\s*•\s*(\d+)\s+files?").expect("counts regex"))
}

fn status_title_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^(.+?)\s+[\w.-]+/[\w.-]+\s{2,}").expect("status title regex")
    })
}

/// `owner/repo` from a GitHub remote URL, https or ssh; `None` for anything else.
pub fn github_repo_of(remote: &str) -> Option<String> {
    let url = remote.trim();
    let caps = github_remote_re().captures(url)?;
    Some(format!("{}/{}", &caps[1], &caps[2]))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudStartProblem {
    NoEnvironment { repo: String },
    Other { said: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartAnswer {
    Ok { task_id: String, url: String },
    Err(CloudStartProblem),
}

/// What `codex cloud exec` answered: the task, or why there is none.
pub fn start_answer(ran: &Ran, repo: &str) -> StartAnswer {
    let said = format!("{}\n{}", ran.stdout, ran.stderr);
    if let Some(caps) = task_url_re().captures(&said) {
        return StartAnswer::Ok {
            task_id: caps[1].to_string(),
            url: caps[0].to_string(),
        };
    }
    if no_env_re().is_match(&said) {
        return StartAnswer::Err(CloudStartProblem::NoEnvironment {
            repo: repo.to_string(),
        });
    }
    let line = said
        .lines()
        .map(str::trim)
        .filter(|part| !part.is_empty() && !config_warn_re().is_match(part))
        .last()
        .unwrap_or("Codex did not say why.");
    let truncated: String = line.chars().take(300).collect();
    StartAnswer::Err(CloudStartProblem::Other { said: truncated })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudState {
    Pending,
    Ready,
    Failed,
    Applied,
    Unknown,
}

impl CloudState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Applied => "applied",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_str_opt(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "ready" => Some(Self::Ready),
            "failed" => Some(Self::Failed),
            "applied" => Some(Self::Applied),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudStatus {
    pub state: CloudState,
    pub title: Option<String>,
    pub added: Option<u64>,
    pub removed: Option<u64>,
    pub files: Option<u64>,
}

/// What `codex cloud status` printed, read into its parts.
pub fn status_answer(text: &str) -> CloudStatus {
    let head = text
        .lines()
        .find(|line| status_head_re().is_match(line))
        .unwrap_or("");
    let word = status_head_re()
        .captures(head)
        .map(|caps| caps[1].to_ascii_lowercase())
        .unwrap_or_default();
    let state = match word.as_str() {
        "pending" | "running" | "queued" | "in_progress" => CloudState::Pending,
        "ready" | "completed" | "done" => CloudState::Ready,
        "failed" | "error" | "cancelled" => CloudState::Failed,
        "applied" => CloudState::Applied,
        _ => CloudState::Unknown,
    };
    let counts = status_counts_re().captures(text);
    let rest = status_head_re().replace(head, "");
    let title_raw = status_title_re()
        .captures(&rest)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str())
        .unwrap_or(&rest)
        .trim();
    let title = if title_raw.is_empty() {
        None
    } else {
        Some(title_raw.to_string())
    };
    CloudStatus {
        state,
        title,
        added: counts.as_ref().and_then(|c| c[1].parse().ok()),
        removed: counts.as_ref().and_then(|c| c[2].parse().ok()),
        files: counts.as_ref().and_then(|c| c[3].parse().ok()),
    }
}

/// A Codex cloud task persisted by the cloud task service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CloudTask {
    pub task_id: String,
    pub url: String,
    pub runtime: String,
    pub repo: String,
    pub branch: Option<String>,
    pub prompt: String,
    pub folder: String,
    pub created_at: String,
    pub teammate_id: Option<String>,
    pub status: CloudStatus,
    pub applied_at: Option<String>,
}

/// Where the folder stands with GitHub: its repository, branch, and commits GitHub does not have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderOnGitHub {
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub unpushed: Option<u64>,
    pub dirty: bool,
}

pub fn folder_on_github(folder: &Path, git: Runner<'_>) -> FolderOnGitHub {
    let remote = git(&["remote", "get-url", "origin"], folder).ok();
    let branch = git(&["rev-parse", "--abbrev-ref", "HEAD"], folder).ok();
    let ahead = git(&["rev-list", "--count", "@{u}..HEAD"], folder).ok();
    let status = git(&["status", "--porcelain"], folder).ok();

    let name = branch
        .filter(|r| r.code == 0)
        .map(|r| r.stdout.trim().to_string())
        .filter(|s| !s.is_empty() && s != "HEAD");
    let repo = remote
        .filter(|r| r.code == 0)
        .and_then(|r| github_repo_of(&r.stdout));
    let unpushed = ahead.filter(|r| r.code == 0).and_then(|r| {
        let trimmed = r.stdout.trim();
        if !trimmed.is_empty() && trimmed.bytes().all(|b| b.is_ascii_digit()) {
            trimmed.parse::<u64>().ok()
        } else {
            None
        }
    });
    let dirty = status
        .filter(|r| r.code == 0)
        .is_some_and(|r| !r.stdout.trim().is_empty());

    FolderOnGitHub {
        repo,
        branch: name,
        unpushed,
        dirty,
    }
}

/// Whether Codex Cloud has an environment for a repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudEnvironment {
    Ready,
    Missing,
    Unknown,
}

pub fn environment_answer(ran: &Ran) -> CloudEnvironment {
    if ran.code == 0 {
        return CloudEnvironment::Ready;
    }
    let said = format!("{}\n{}", ran.stderr, ran.stdout);
    if no_env_re().is_match(&said) {
        CloudEnvironment::Missing
    } else {
        CloudEnvironment::Unknown
    }
}

pub fn environment_of(codex: Runner<'_>, repo: &str, cwd: &Path) -> CloudEnvironment {
    match codex(&["cloud", "list", "--env", repo, "--limit", "1"], cwd) {
        Ok(ran) => environment_answer(&ran),
        Err(_) => CloudEnvironment::Unknown,
    }
}

/// Said to the person when a cloud task cannot start, in their terms.
pub fn refusal_for(problem: &CloudStartProblem) -> String {
    match problem {
        CloudStartProblem::NoEnvironment { repo } => format!(
            "Codex Cloud has no environment for {repo} yet. In the Codex app, open Settings > Legacy Codex Cloud and create one for that repository, then send again."
        ),
        CloudStartProblem::Other { said } => {
            format!("Codex Cloud did not start the task: {said}")
        }
    }
}

fn codex_log_line_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\[\d{4}-\d{2}-\d{2}T[\d:.]+(?:Z|[+-]\d{2}:\d{2})\] [a-z_]+: ")
            .expect("codex log line regex")
    })
}

/// More than this was added: not a few Codex lines, and left alone rather than read in part.
const MOST_CHECKED_BYTES: u64 = 1024 * 1024;

/// For a test: the log check, alone.
pub fn reads_like_codex_log(text: &str) -> bool {
    text.lines()
        .filter(|line| !line.is_empty())
        .all(|line| codex_log_line_re().is_match(line))
}

fn folder_lock(cwd: &Path) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let map = LOCKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = map.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .entry(cwd.to_path_buf())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}

fn same_inode(a: &fs::Metadata, b: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        a.ino() == b.ino()
    }
    #[cfg(not(unix))]
    {
        let _ = (a, b);
        true
    }
}

fn take_back_codex_lines(path: &Path, before: Option<u64>) {
    let Ok(link) = fs::symlink_metadata(path) else {
        return;
    };
    if !link.is_file() {
        return;
    }
    let from = before.unwrap_or(0);
    if link.len() <= from {
        return;
    }
    let added = link.len() - from;
    if added > MOST_CHECKED_BYTES {
        return;
    }
    let Ok(mut file) = OpenOptions::new().read(true).write(true).open(path) else {
        return;
    };
    let mut empty_now = false;
    {
        let Ok(opened) = file.metadata() else {
            return;
        };
        if !same_inode(&opened, &link) || opened.len() != link.len() {
            return;
        }
        if file.seek(SeekFrom::Start(from)).is_err() {
            return;
        }
        let mut buf = vec![0u8; added as usize];
        if file.read_exact(&mut buf).is_err() {
            return;
        }
        let Ok(text) = std::str::from_utf8(&buf) else {
            return;
        };
        let lines: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();
        if lines.is_empty() || !lines.iter().all(|line| codex_log_line_re().is_match(line)) {
            return;
        }
        let Ok(after_read) = file.metadata() else {
            return;
        };
        if after_read.len() != link.len() {
            return;
        }
        if file.set_len(from).is_ok() {
            empty_now = before.is_none();
        }
    }
    drop(file);
    if empty_now {
        if let Ok(left) = fs::symlink_metadata(path)
            && left.is_file()
            && same_inode(&left, &link)
            && left.len() == 0
        {
            let _ = fs::remove_file(path);
        }
    }
}

/// Runs `codex cloud` in `cwd` serialized per folder, removing any Codex debug lines
/// added to `error.log` while never touching lines that do not read like Codex's.
pub fn run_leaving_no_log<F>(args: &[&str], cwd: &Path, run: F) -> Result<Ran, String>
where
    F: FnOnce(&[&str], &Path) -> Result<Ran, String>,
{
    let lock = folder_lock(cwd);
    let _guard = lock.lock().unwrap_or_else(|e| e.into_inner());
    let path = cwd.join("error.log");
    let before = fs::symlink_metadata(&path)
        .ok()
        .filter(|m| m.is_file())
        .map(|m| m.len());
    let result = run(args, cwd);
    take_back_codex_lines(&path, before);
    result
}

/// Wraps a `Runner` so every call leaves `error.log` in its `cwd` as it found it.
pub fn leaving_no_log<'a, F>(run: F) -> impl Fn(&[&str], &Path) -> Result<Ran, String> + 'a
where
    F: Fn(&[&str], &Path) -> Result<Ran, String> + 'a,
{
    move |args: &[&str], cwd: &Path| run_leaving_no_log(args, cwd, |a, c| run(a, c))
}

#[cfg(test)]
#[path = "cloud_tasks_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "cloud_tasks_log_tests.rs"]
mod log_tests;

#[cfg(test)]
#[path = "sol_cleanup_sweep_tests.rs"]
mod sol_cleanup_sweep_tests;

//! Commit, push, pull request: the folder's changes, saved in git by the person, from the conversation that made them.
//! A port of Electron's apps/desktop/src/main/folder-commit.ts and shared/folder-commit.ts (0.680 onward).
//!
//! Nothing happens without a press, and what a press would commit is listed before it. The commit is made as the
//! person (their git name, their hooks). A commit their hook refuses leaves the staging area exactly as it was.
//! `.locust/` (teammates' branches, comparison copies) is never committed. What reaches the network (push, `gh`)
//! never stops to ask in a terminal nobody can see.

use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};

/// Every git Locust runs on its own is told this first (main/git-guard.ts OWN_GIT_CONFIG): a repository's own config
/// can name a program for `git status` to run.
const OWN_GIT_CONFIG: [&str; 2] = ["-c", "core.fsmonitor=false"];
/// Never `.locust/`: teammates' branches and comparison copies live there (NOT_LOCUST).
const NOT_LOCUST: [&str; 3] = ["--", ".", ":(exclude).locust"];
/// Untracked files bigger than this are left out of a commit (worktrees.ts CHECKPOINT_MAX_FILE_BYTES).
pub const CHECKPOINT_MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
const LOCAL_TIMEOUT: Duration = Duration::from_secs(30);
const SLOW_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const NETWORK_TIMEOUT: Duration = Duration::from_secs(120);

/// A branch name Locust hands to git: never one that starts with a dash or carries anything unusual (SAFE_BRANCH).
pub fn safe_branch(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else { return false };
    name.len() <= 200
        && (first.is_ascii_alphanumeric() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | '-'))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

impl ChangeStatus {
    /// `statusOf`: the porcelain code's meaning.
    fn of(code: &str) -> Self {
        if code == "??" || code.contains('A') {
            Self::Added
        } else if code.contains('D') {
            Self::Deleted
        } else if code.contains('R') {
            Self::Renamed
        } else {
            Self::Modified
        }
    }
    /// The letter the commit panel shows (STATUS_LETTER).
    pub fn letter(self) -> &'static str {
        match self {
            Self::Added => "A",
            Self::Modified => "M",
            Self::Deleted => "D",
            Self::Renamed => "R",
        }
    }
    pub fn word(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Modified => "modified",
            Self::Deleted => "deleted",
            Self::Renamed => "renamed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    pub status: ChangeStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Remote {
    /// Always `origin`: the remote a push and a pull request go to.
    pub name: String,
    /// The branch already tracks one there, so a plain push knows where to go.
    pub upstream: bool,
    /// The remote's default branch, the base of a pull request.
    pub default_branch: String,
    /// On GitHub, and the person's `gh` is signed in: a pull request can be opened.
    pub pull_requests: bool,
}

/// Why a plain commit cannot be made here (BlockedWhy).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blocked {
    Merging,
    Rebasing,
    Detached,
    Conflicted,
    BranchName,
}

/// `blockedSentence`, word for word.
pub fn blocked_sentence(why: Blocked) -> &'static str {
    match why {
        Blocked::Merging => "A merge is in progress in this folder. Finish or abort it in git first.",
        Blocked::Rebasing => "A rebase is in progress in this folder. Finish or abort it in git first.",
        Blocked::Detached => "This folder is not on a branch, so a commit would belong to none. Switch to a branch first.",
        Blocked::Conflicted => "Some files still have unresolved conflicts. Resolve them first.",
        Blocked::BranchName => "This branch's name is not one Locust will pass to git (it starts with a dash, or has unusual characters). Rename the branch first.",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nothing {
    NotARepository,
    Clean,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderChanges {
    None(Nothing),
    Blocked { why: Blocked, files: Vec<String> },
    Changes { branch: String, files: Vec<ChangedFile>, remote: Option<Remote> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Then {
    Commit,
    Push,
    PullRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitResult {
    Done {
        sha: String,
        branch: String,
        files: usize,
        /// Untracked files too big to commit, left as they are.
        left_out: Vec<String>,
        /// A branch made for the pull request, the checkout now on it.
        new_branch: Option<String>,
        pushed: Option<bool>,
        pull_request_url: Option<String>,
        /// The commit was made; what came after it was not.
        after: Option<String>,
    },
    Refused(String),
}

/// The message a commit starts with (commitDraft): the first ask as the subject, later asks as the body, and who did
/// the work as a trailer. Edited freely before Commit.
pub fn commit_draft(asks: &[String], teammate: Option<&str>) -> String {
    let lines: Vec<String> = asks
        .iter()
        .map(|ask| ask.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|ask| !ask.is_empty())
        .collect();
    let clip = |text: &str, max: usize| -> String {
        if text.chars().count() <= max {
            text.to_owned()
        } else {
            let cut: String = text.chars().take(max - 1).collect();
            format!("{}…", cut.trim_end())
        }
    };
    let subject = match lines.first() {
        Some(first) => match first_sentence(first) {
            Some(sentence) if sentence.chars().count() <= 72 => clip(sentence, 72),
            _ => clip(first, 72),
        },
        None => clip(&teammate.map_or_else(|| "Changes".to_owned(), |t| format!("{t}'s changes")), 72),
    };
    let body = if lines.len() > 1 {
        format!(
            "\n\n{}",
            lines.iter().map(|line| format!("- {}", clip(line, 100))).collect::<Vec<_>>().join("\n")
        )
    } else {
        String::new()
    };
    let trailer = teammate.map_or_else(String::new, |t| format!("\n\nLocust-Teammate: {t}"));
    format!("{subject}{body}{trailer}")
}

/// The first sentence, as `/^.+?[.!?]['"’”)]?(?=\s|$)/` finds it: an end mark followed by space or the end, so the dot in
/// README.md is not a sentence's end.
fn first_sentence(text: &str) -> Option<&str> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for (i, &(_, c)) in chars.iter().enumerate() {
        if i == 0 || !matches!(c, '.' | '!' | '?') {
            continue;
        }
        let mut end = i + 1;
        if chars.get(end).is_some_and(|&(_, q)| matches!(q, '\'' | '"' | '’' | '”' | ')')) {
            end += 1;
        }
        if chars.get(end).is_none_or(|&(_, next)| next.is_whitespace()) {
            let byte = chars.get(end).map_or(text.len(), |&(b, _)| b);
            return Some(&text[..byte]);
        }
    }
    None
}

/// `fix the cart total` -> `locust/fix-the-cart-total` (branchFromSubject).
pub fn branch_from_subject(subject: &str) -> String {
    let lowered = subject.to_lowercase();
    let mut slug = String::new();
    let mut dash = false;
    for c in lowered.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            slug.push(c);
            dash = false;
        } else if !dash {
            slug.push('-');
            dash = true;
        }
    }
    let words: Vec<&str> = slug.trim_matches('-').split('-').take(6).collect();
    let mut name: String = words.join("-").chars().take(48).collect();
    while name.ends_with('-') {
        name.pop();
    }
    format!("locust/{}", if name.is_empty() { "changes" } else { &name })
}

/// `Fixes #N` once, after the body (shared/github-issue.ts withFixesLine).
pub fn with_fixes_line(body: &str, number: Option<u32>) -> String {
    let Some(n) = number.filter(|n| *n >= 1) else { return body.to_owned() };
    let line = format!("Fixes #{n}");
    if body.contains(&line) {
        body.to_owned()
    } else if body.trim().is_empty() {
        line
    } else {
        format!("{}\n\n{line}", body.trim())
    }
}

/// GitHub by the URL's own host (cloud-tasks.ts githubRepoOf): `owner/name`, or none.
pub fn github_repo_of(remote: &str) -> Option<String> {
    let url = remote.trim();
    let rest = ["https://github.com/", "http://github.com/", "git@github.com:", "ssh://git@github.com/"]
        .iter()
        .find_map(|prefix| url.strip_prefix(prefix))
        .or_else(|| {
            // https://user@github.com/ form.
            let scheme = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
            let (user, after) = scheme.split_once('@')?;
            (!user.contains('/')).then_some(())?;
            after.strip_prefix("github.com/")
        })?;
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let (owner, name) = rest.split_once('/')?;
    let ok = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
    (ok(owner) && ok(name)).then(|| format!("{owner}/{name}"))
}

/// `git status --porcelain=v1 -z` entries (worktrees.ts statusEntriesOf): a rename's second path is skipped.
pub fn status_entries(porcelain_z: &str) -> Vec<(String, String)> {
    let parts: Vec<&str> = porcelain_z.split('\0').collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        let entry = parts[i];
        if entry.len() >= 4 && entry.is_char_boundary(2) && entry.is_char_boundary(3) {
            let code = &entry[..2];
            out.push((code.to_owned(), entry[3..].to_owned()));
            if code.starts_with('R') || code.starts_with('C') {
                i += 1;
            }
        }
        i += 1;
    }
    out
}

/// One program run to its end: stdout on success, stderr (or stdout) as the error otherwise. Never succeeds on a
/// non-zero exit (unlike changes::git, which keeps a diff's output): a refused commit must read as refused.
fn run(program: &str, args: &[String], cwd: &Path, timeout: Duration, quiet_network: bool) -> Result<String, String> {
    let mut command = Command::new(program);
    command.args(args).current_dir(cwd).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if quiet_network {
        // Nothing may stop to ask in a terminal nobody can see: a missing credential is an error, not a hang.
        command.env("GIT_TERMINAL_PROMPT", "0").env("GH_PROMPT_DISABLED", "1").env("GH_NO_UPDATE_NOTIFIER", "1");
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().ok_or("No output")?;
    let stderr = child.stderr.take().ok_or("No error output")?;
    let out = std::thread::spawn(move || {
        let mut b = Vec::new();
        stdout.take(64 * 1024 * 1024).read_to_end(&mut b).map(|_| b)
    });
    let err = std::thread::spawn(move || {
        let mut b = Vec::new();
        stderr.take(1024 * 1024).read_to_end(&mut b).map(|_| b)
    });
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{program} took longer than {} s and was stopped.", timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let output = out.join().map_err(|_| "output reader failed")?.map_err(|e| e.to_string())?;
    let error = err.join().map_err(|_| "error reader failed")?.map_err(|e| e.to_string())?;
    if status.success() {
        Ok(String::from_utf8_lossy(&output).into_owned())
    } else {
        let said = String::from_utf8_lossy(&error).trim().to_owned();
        let said = if said.is_empty() { String::from_utf8_lossy(&output).trim().to_owned() } else { said };
        Err(said.chars().take(400).collect())
    }
}

/// What reaches the network: git (push) and gh. Swapped in tests, as Electron's runNetwork is.
pub trait Network: Send + Sync {
    fn run(&self, program: &str, args: &[String], cwd: &Path) -> Result<String, String>;
}

pub struct RealNetwork;

impl Network for RealNetwork {
    fn run(&self, program: &str, args: &[String], cwd: &Path) -> Result<String, String> {
        let mut all: Vec<String> = Vec::new();
        if program == "git" {
            all.extend(OWN_GIT_CONFIG.iter().map(|s| (*s).to_owned()));
        }
        all.extend(args.iter().cloned());
        run(program, &all, cwd, NETWORK_TIMEOUT, true)
    }
}

fn strs(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| (*s).to_owned()).collect()
}

fn git_with(folder: &Path, args: &[&str], timeout: Duration) -> Result<String, String> {
    let mut all = strs(&OWN_GIT_CONFIG);
    all.extend(strs(args));
    run("git", &all, folder, timeout, false)
}

fn git(folder: &Path, args: &[&str]) -> Result<String, String> {
    git_with(folder, args, LOCAL_TIMEOUT)
}

fn norm(path: &str) -> String {
    let joined = path.replace('\\', "/");
    let mut out = String::new();
    for c in joined.chars() {
        if !(c == '/' && out.ends_with('/')) {
            out.push(c);
        }
    }
    out.trim_end_matches('/').to_lowercase()
}

fn is_top(folder: &Path) -> bool {
    if git(folder, &["rev-parse", "--is-inside-work-tree"]).map(|s| s.trim().to_owned()).as_deref() != Ok("true") {
        return false;
    }
    let top = git(folder, &["rev-parse", "--show-toplevel"]).map(|s| s.trim().to_owned()).unwrap_or_default();
    let here = folder.canonicalize().unwrap_or_else(|_| folder.to_owned());
    !top.is_empty() && norm(&top) == norm(&here.to_string_lossy())
}

fn same_files(left: &[String], right: &[String]) -> bool {
    let mut a = left.to_vec();
    let mut b = right.to_vec();
    a.sort();
    b.sort();
    a == b
}

struct CachedPullRequest {
    at: Instant,
    branch: String,
    found: Option<crate::pull_requests::FolderPullRequest>,
}

pub struct FolderCommits {
    network: Box<dyn Network>,
    /// `gh auth status` for five minutes at a time: it reaches GitHub, and the panel asks often.
    signed_in: Mutex<Option<(Instant, bool)>>,
    /// The last look at each folder's pull request, for a minute.
    pull_requests: Mutex<HashMap<PathBuf, CachedPullRequest>>,
}

impl Default for FolderCommits {
    fn default() -> Self {
        Self::new(Box::new(RealNetwork))
    }
}

impl FolderCommits {
    pub fn new(network: Box<dyn Network>) -> Self {
        Self {
            network,
            signed_in: Mutex::new(None),
            pull_requests: Mutex::new(HashMap::new()),
        }
    }

    /// The pull request for the branch the folder is on (`pull_requests.rs`), or `None`:
    /// not a GitHub folder, `gh` not signed in, on the default branch, or no pull request for this branch.
    pub fn pull_request(&self, folder: &Path) -> Option<crate::pull_requests::FolderPullRequest> {
        let branch = git(folder, &["symbolic-ref", "--short", "-q", "HEAD"]).ok()?.trim().to_owned();
        if branch.is_empty() || !safe_branch(&branch) {
            return None;
        }
        {
            let kept = crate::invariants::lock(&self.pull_requests);
            if let Some(entry) = kept.get(folder)
                && entry.branch == branch
                && entry.at.elapsed() < Duration::from_millis(crate::pull_requests::PULL_REQUEST_CACHE_MS)
            {
                return entry.found.clone();
            }
        }
        let remote = self.remote_of(folder, &branch);
        let found = if remote.as_ref().is_some_and(|r| r.pull_requests && branch != r.default_branch) {
            self.network
                .run("gh", &strs(&["pr", "view", "--json", crate::pull_requests::PULL_REQUEST_FIELDS]), folder)
                .ok()
                .and_then(|json| crate::pull_requests::pull_request_of(&json))
        } else {
            None
        };
        crate::invariants::lock(&self.pull_requests).insert(
            folder.to_path_buf(),
            CachedPullRequest {
                at: Instant::now(),
                branch,
                found: found.clone(),
            },
        );
        found
    }

    /// After a sign-in in Settings, the next look asks `gh` again rather than its answer from before.
    pub fn forget_sign_in(&self) {
        *crate::invariants::lock(&self.signed_in) = None;
        crate::invariants::lock(&self.pull_requests).clear();
    }

    fn gh_signed_in(&self, folder: &Path) -> bool {
        let mut kept = self.signed_in.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((at, ok)) = *kept
            && at.elapsed() < Duration::from_secs(5 * 60)
        {
            return ok;
        }
        let ok = self.network.run("gh", &strs(&["auth", "status", "--hostname", "github.com"]), folder).is_ok();
        *kept = Some((Instant::now(), ok));
        ok
    }

    fn remote_of(&self, folder: &Path, branch: &str) -> Option<Remote> {
        let url = git(folder, &["remote", "get-url", "origin"]).map(|s| s.trim().to_owned()).unwrap_or_default();
        if url.is_empty() {
            return None;
        }
        let upstream_ref = format!("{branch}@{{upstream}}");
        let upstream = git(folder, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", &upstream_ref]).is_ok();
        let head = git(folder, &["symbolic-ref", "--short", "-q", "refs/remotes/origin/HEAD"]).map(|s| s.trim().to_owned()).unwrap_or_default();
        let mut default_branch = head.strip_prefix("origin/").unwrap_or(&head).to_owned();
        if default_branch.is_empty() {
            // A clone that never set origin/HEAD: whichever of the usual names the remote has.
            let known = git(folder, &["branch", "-r", "--format=%(refname:short)"]).unwrap_or_default();
            default_branch = if known.lines().any(|l| l == "origin/main") {
                "main".into()
            } else if known.lines().any(|l| l == "origin/master") {
                "master".into()
            } else {
                "main".into()
            };
        }
        let github = github_repo_of(&url).is_some() && safe_branch(&default_branch);
        Some(Remote {
            name: "origin".into(),
            upstream,
            pull_requests: github && self.gh_signed_in(folder),
            default_branch,
        })
    }

    /// What a commit would take now (`changes`).
    pub fn changes(&self, folder: &Path) -> FolderChanges {
        // The folder must be the repository's top, or a commit would take changes the person never saw here.
        if !is_top(folder) {
            return FolderChanges::None(Nothing::NotARepository);
        }
        let mut status_args = vec!["status", "--porcelain=v1", "-z", "--untracked-files=all"];
        status_args.extend(NOT_LOCUST);
        let Ok(porcelain) = git(folder, &status_args) else {
            return FolderChanges::None(Nothing::NotARepository);
        };
        let entries = status_entries(&porcelain);
        let git_dir = git(folder, &["rev-parse", "--absolute-git-dir"]).map(|s| s.trim().to_owned()).unwrap_or_default();
        let has = |name: &str| !git_dir.is_empty() && Path::new(&git_dir).join(name).exists();
        let conflicted: Vec<String> = entries
            .iter()
            .filter(|(code, _)| code.contains('U') || code == "AA" || code == "DD")
            .map(|(_, path)| path.clone())
            .collect();
        if has("MERGE_HEAD") {
            return FolderChanges::Blocked { why: Blocked::Merging, files: conflicted };
        }
        if has("rebase-merge") || has("rebase-apply") {
            return FolderChanges::Blocked { why: Blocked::Rebasing, files: conflicted };
        }
        if !conflicted.is_empty() {
            return FolderChanges::Blocked { why: Blocked::Conflicted, files: conflicted };
        }
        if entries.is_empty() {
            return FolderChanges::None(Nothing::Clean);
        }
        let branch = git(folder, &["symbolic-ref", "--short", "-q", "HEAD"]).map(|s| s.trim().to_owned()).unwrap_or_default();
        if branch.is_empty() {
            return FolderChanges::Blocked { why: Blocked::Detached, files: Vec::new() };
        }
        if !safe_branch(&branch) {
            return FolderChanges::Blocked { why: Blocked::BranchName, files: Vec::new() };
        }
        let remote = self.remote_of(folder, &branch);
        FolderChanges::Changes {
            files: entries.into_iter().map(|(code, path)| ChangedFile { status: ChangeStatus::of(&code), path }).collect(),
            branch,
            remote,
        }
    }

    /// Commit what was shown, then push or open a pull request if asked (`commit`). `shown` binds the commit to the
    /// list the person saw: a file that changed or appeared since is not taken unseen.
    pub fn commit(&self, folder: &Path, message: &str, then: Then, shown: Option<&[String]>, fixes: Option<u32>) -> CommitResult {
        let (branch_now, files_now, remote) = match self.changes(folder) {
            FolderChanges::Changes { branch, files, remote } => (branch, files, remote),
            FolderChanges::None(_) => return CommitResult::Refused("There is nothing to commit in this folder now.".into()),
            FolderChanges::Blocked { why, .. } => return CommitResult::Refused(blocked_sentence(why).into()),
        };
        let paths: Vec<String> = files_now.iter().map(|f| f.path.clone()).collect();
        if let Some(shown) = shown
            && !same_files(shown, &paths)
        {
            return CommitResult::Refused(
                "The folder changed since this was opened, so nothing was committed. Look at the list again, then commit.".into(),
            );
        }
        let text = message.trim();
        if text.is_empty() {
            return CommitResult::Refused("A commit needs a message.".into());
        }
        if then != Then::Commit && remote.is_none() {
            return CommitResult::Refused("This folder has no remote named origin to push to.".into());
        }
        if then == Then::PullRequest && !remote.as_ref().is_some_and(|r| r.pull_requests) {
            return CommitResult::Refused("A pull request needs a GitHub remote and the GitHub CLI signed in (gh auth login).".into());
        }
        // A pull request from the default branch goes from a branch of its own, made first, so the commit is never
        // left on the person's main unpushed.
        let mut branch = branch_now.clone();
        let mut new_branch: Option<String> = None;
        if then == Then::PullRequest && remote.as_ref().is_some_and(|r| r.default_branch == branch) {
            let wanted = branch_from_subject(text.lines().next().unwrap_or(text));
            let mut name = wanted.clone();
            let mut n = 2;
            while git(folder, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{name}")]).is_ok() {
                name = format!("{wanted}-{n}");
                n += 1;
            }
            if let Err(error) = git(folder, &["switch", "-q", "-c", &name]) {
                // A post-checkout hook fails AFTER the switch: back to where it was, the new branch gone.
                let on_now = git(folder, &["symbolic-ref", "--short", "-q", "HEAD"]).map(|s| s.trim().to_owned()).unwrap_or_default();
                if on_now == name {
                    let _ = git(folder, &["switch", "-q", &branch_now]);
                    let _ = git(folder, &["branch", "-q", "-D", &name]);
                }
                return CommitResult::Refused(format!("git could not make a branch for the pull request: {}", or_no_reason(&error)));
            }
            branch = name.clone();
            new_branch = Some(name);
        }
        let back_to_start = |made: &Option<String>| {
            if let Some(made) = made {
                let _ = git(folder, &["switch", "-q", &branch_now]);
                let _ = git(folder, &["branch", "-q", "-D", made]);
            }
        };
        // What is staged now, to put back exactly if the commit is refused.
        let saved = match git(folder, &["write-tree"]) {
            Ok(tree) => tree.trim().to_owned(),
            Err(error) => {
                back_to_start(&new_branch);
                return CommitResult::Refused(format!("git could not read what is staged: {}", or_no_reason(&error)));
            }
        };
        let mut left_out: Vec<String> = Vec::new();
        let mut staged: Option<String> = None;
        let attempt = (|| -> Result<(), String> {
            // Already staged under .locust/: taken off the index, never committed.
            let locust_staged = git(folder, &["diff", "--cached", "--name-only", "-z", "--", ".locust"])?;
            if locust_staged.split('\0').any(|f| !f.is_empty()) {
                git(folder, &["reset", "-q", "--", ".locust"])?;
            }
            let mut add = vec!["add", "-A"];
            add.extend(NOT_LOCUST);
            git_with(folder, &add, SLOW_TIMEOUT)?;
            for file in files_now.iter().filter(|f| f.status == ChangeStatus::Added) {
                let bytes = std::fs::metadata(folder.join(&file.path)).map(|m| m.len()).unwrap_or(0);
                if bytes > CHECKPOINT_MAX_FILE_BYTES {
                    left_out.push(file.path.clone());
                }
            }
            if !left_out.is_empty() {
                let mut reset = vec!["reset", "-q", "--"];
                reset.extend(left_out.iter().map(String::as_str));
                git(folder, &reset)?;
            }
            staged = Some(git(folder, &["write-tree"])?.trim().to_owned());
            git_with(folder, &["commit", "-q", "-m", text], SLOW_TIMEOUT)?;
            Ok(())
        })();
        if let Err(said) = attempt {
            // Put back only if the index is still what Locust staged: a person who staged something else while a
            // slow hook ran keeps it, and is told.
            let current = git(folder, &["write-tree"]).ok().map(|t| t.trim().to_owned());
            let ours = staged.is_none() || current == staged;
            if ours {
                let _ = git(folder, &["read-tree", &saved]);
            }
            back_to_start(&new_branch);
            let kept = if ours { "" } else { " What is staged changed while the commit ran, so Locust left it as it is." };
            let lower = said.to_lowercase();
            let message = if lower.contains("tell me who you are") || lower.contains("user.email") || lower.contains("user.name") {
                "git does not know your name and email yet, so it cannot make the commit as you. Set them with git config user.name and user.email, then commit again.".to_owned()
            } else if lower.contains("nothing to commit") || lower.contains("nothing added to commit") {
                "There was nothing left to commit.".to_owned()
            } else {
                format!("Your commit hooks refused the commit: {}", said.trim_start_matches("git commit:").trim_start())
            };
            return CommitResult::Refused(format!("{message}{kept}"));
        }
        let sha = git(folder, &["rev-parse", "HEAD"]).map(|s| s.trim().to_owned()).unwrap_or_default();
        let files = files_now.len() - left_out.len();
        let done = |pushed: Option<bool>, pull_request_url: Option<String>, after: Option<String>| CommitResult::Done {
            sha: sha.clone(),
            branch: branch.clone(),
            files,
            left_out: left_out.clone(),
            new_branch: new_branch.clone(),
            pushed,
            pull_request_url,
            after,
        };
        if then == Then::Commit {
            return done(None, None, None);
        }
        let mut push = strs(&["push"]);
        if !(remote.as_ref().is_some_and(|r| r.upstream) && new_branch.is_none()) {
            push.push("-u".into());
        }
        // A full refspec: a branch name can never be read as a flag.
        push.push("origin".into());
        push.push(format!("refs/heads/{branch}:refs/heads/{branch}"));
        if let Err(error) = self.network.run("git", &push, folder) {
            return done(Some(false), None, Some(format!("The push was refused: {}", or_no_reason(&error))));
        }
        if then == Then::Push {
            return done(Some(true), None, None);
        }
        let mut parts = text.splitn(2, '\n');
        let subject = parts.next().unwrap_or(text);
        let body = parts.next().unwrap_or("").trim();
        let base = remote.as_ref().map(|r| r.default_branch.clone()).unwrap_or_default();
        // Each value joined to its flag, so none of them can be read as a flag of its own.
        let create = vec![
            "pr".to_owned(),
            "create".to_owned(),
            format!("--title={subject}"),
            format!("--body={}", with_fixes_line(if body.is_empty() { subject } else { body }, fixes)),
            format!("--head={branch}"),
            format!("--base={base}"),
        ];
        match self.network.run("gh", &create, folder) {
            Ok(said) => {
                // Electron: /https:\/\/\S+\/pull\/\d+/
                let url = pull_request_url_of(&said);
                crate::invariants::lock(&self.pull_requests).remove(folder);
                done(Some(true), url, None)
            }
            Err(error) => done(Some(true), None, Some(format!("GitHub did not open the pull request: {}", or_no_reason(&error)))),
        }
    }
}

/// The pull request URL `gh pr create` printed, as Electron's `/https:\/\/\S+\/pull\/\d+/` finds it.
pub fn pull_request_url_of(said: &str) -> Option<String> {
    let mut search = 0;
    while let Some(rel) = said[search..].find("https://") {
        let start = search + rel;
        let rest = &said[start..];
        let token_end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
        let token = &rest[..token_end];
        if let Some(pull) = token.find("/pull/") {
            let after = &token[pull + "/pull/".len()..];
            let digits = after.chars().take_while(|c| c.is_ascii_digit()).count();
            if digits > 0 {
                let end = pull + "/pull/".len() + digits;
                return Some(token[..end].to_owned());
            }
        }
        search = start + token_end.max(1);
    }
    None
}

fn or_no_reason(error: &str) -> &str {
    if error.trim().is_empty() { "no reason given" } else { error.trim() }
}

/// What the commit panel says after a commit (CommitOutcome).
pub fn outcome_words(result: &CommitResult) -> Vec<String> {
    match result {
        CommitResult::Refused(message) => vec![message.clone()],
        CommitResult::Done { sha, branch, files, left_out, new_branch, pushed, pull_request_url, after } => {
            let count = format!("{files} {}", if *files == 1 { "file" } else { "files" });
            let short: String = sha.chars().take(7).collect();
            let mut first = format!("Committed {count} to {branch} as {short}.");
            if let Some(made) = new_branch {
                first.push_str(&format!(" Your folder is on {made} now."));
            }
            if *pushed == Some(true) {
                first.push_str(" Pushed to origin.");
            }
            let mut out = vec![first];
            if let Some(url) = pull_request_url {
                out.push(format!("Pull request opened: {}", url.strip_prefix("https://github.com/").unwrap_or(url)));
            }
            if !left_out.is_empty() {
                out.push(format!("Left out, too big to commit: {}.", left_out.join(", ")));
            }
            if let Some(after) = after {
                out.push(after.clone());
            }
            out
        }
    }
}

#[cfg(test)]
mod tests;

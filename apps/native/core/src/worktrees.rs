//! A TEAMMATE'S OWN COPY OF THE PROJECT (Electron `main/worktrees.ts`, `shared/worktree-name.ts`).
//!
//! A teammate set to work on its own branch gets a git worktree under the project's `.locust/worktrees/<teammate id>`,
//! on a `locust/<name>` branch, so its turns never touch the person's checkout. Each turn's changes are committed on
//! that branch (`checkpoint`), and the person reviews the branch whole or turn by turn (`review`, `turn_diff`).
//!
//! Every git command runs with Locust's own config first (`core.fsmonitor=false`, git-guard.ts), never runs a program
//! the repository names to draw a diff (`--no-ext-diff --no-textconv`), and is bounded in time.

use std::path::{Path, PathBuf};
use std::time::Duration;

pub const WORKTREE_DIR: &str = ".locust/worktrees";
pub const BRANCH_PREFIX: &str = "locust/";
const GIT_TIMEOUT: Duration = Duration::from_secs(20);
/// Making a tree checks out the whole project, which for a big one is not a twenty-second job.
const CHECKOUT_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// A new file larger than this is left out of a turn's commit, and the receipt names it.
pub const CHECKPOINT_MAX_FILE_BYTES: u64 = 50 * 1024 * 1024;
/// A branch diff larger than this is not shown whole.
pub const REVIEW_MAX_DIFF_BYTES: usize = 2 * 1024 * 1024;
const OWN_GIT_CONFIG: [&str; 2] = ["-c", "core.fsmonitor=false"];
const PLAIN_DIFF: [&str; 3] = ["--no-color", "--no-ext-diff", "--no-textconv"];
pub const TURN_LOG_FORMAT: &str = "--format=%x1e%H%x1f%s%x1f%aI%x1f%P";

// ---- names (shared/worktree-name.ts) --------------------------------------------------------------------------------

fn fold(text: &str) -> String {
    let lower = text.to_lowercase();
    let mut out = String::new();
    let mut dash = false;
    for c in lower.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-') {
            out.push(c);
            dash = false;
        } else if !dash {
            out.push('-');
            dash = true;
        }
    }
    let mut s = out.trim_matches(|c| c == '-' || c == '.').to_string();
    while s.contains("..") {
        s = s.replace("..", ".");
    }
    if let Some(stripped) = s.strip_suffix(".lock") {
        s = stripped.to_string();
    }
    s.chars().take(40).collect()
}

/// branchNameFor: `locust/<name>`, folded to what git accepts; the teammate id when the name folds to nothing.
pub fn branch_name_for(name: &str, teammate_id: Option<&str>) -> String {
    let slug = fold(name);
    if !slug.is_empty() {
        return format!("{BRANCH_PREFIX}{slug}");
    }
    let own = teammate_id.map(fold).unwrap_or_default();
    format!("{BRANCH_PREFIX}{}", if own.is_empty() { "teammate".into() } else { own })
}

/// distinctBranchNameFor: for a second teammate whose name folds to a branch already checked out.
pub fn distinct_branch_name_for(name: &str, teammate_id: &str) -> String {
    let id = fold(teammate_id.strip_prefix("tm_").unwrap_or(teammate_id));
    format!("{}-{}", branch_name_for(name, Some(teammate_id)), if id.is_empty() { "teammate" } else { &id })
}

// ---- reading git's output -------------------------------------------------------------------------------------------

/// parseGitVersion: `2.39.5` out of `git version 2.39.5 (Apple Git-154)`.
pub fn parse_git_version(output: &str) -> Option<String> {
    crate::invariants::regex(r"git version (\d+\.\d+(?:\.\d+)?)").captures(output).map(|c| c[1].to_string())
}

/// Worktrees need git 2.5.
pub fn git_version_supports_worktrees(version: &str) -> bool {
    let mut parts = version.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    let (major, minor) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    major > 2 || (major == 2 && minor >= 5)
}

/// One of Locust's own trees.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorktreeInfo {
    pub teammate_id: String,
    pub path: String,
    pub branch: String,
}

/// parseWorktreeList: only the trees Locust made under the folder, out of `git worktree list --porcelain`.
pub fn parse_worktree_list(output: &str, root: &Path, directory: &str) -> Vec<WorktreeInfo> {
    let base = root.join(directory).to_string_lossy().replace('\\', "/").to_lowercase();
    let mut out = Vec::new();
    let (mut path, mut branch): (Option<String>, Option<String>) = (None, None);
    let mut flush = |path: &mut Option<String>, branch: &mut Option<String>| {
        if let Some(p) = path.take() {
            let normal = p.replace('\\', "/");
            if normal.to_lowercase().starts_with(&format!("{base}/")) {
                let teammate_id = normal[base.len() + 1..].split('/').next().unwrap_or("").to_string();
                if !teammate_id.is_empty() {
                    out.push(WorktreeInfo { teammate_id, path: normal, branch: branch.clone().unwrap_or_default() });
                }
            }
        }
        *branch = None;
    };
    for line in output.split('\n') {
        if let Some(p) = line.strip_prefix("worktree ") {
            flush(&mut path, &mut branch);
            path = Some(p.trim().to_string());
        } else if let Some(b) = line.strip_prefix("branch ") {
            branch = Some(b.trim().trim_start_matches("refs/heads/").to_string());
        } else if line.trim().is_empty() {
            flush(&mut path, &mut branch);
        }
    }
    flush(&mut path, &mut branch);
    out
}

/// recordedBranchOf: the branch git records for the tree at `path`, or "".
pub fn recorded_branch_of(porcelain: &str, path: &Path) -> String {
    let want = path.to_string_lossy().to_lowercase();
    for block in porcelain.replace("\r\n", "\n").split("\n\n") {
        let lines: Vec<&str> = block.lines().collect();
        let Some(at) = lines.iter().find_map(|l| l.strip_prefix("worktree ")) else { continue };
        if at.to_lowercase() != want {
            continue;
        }
        return lines.iter().find_map(|l| l.strip_prefix("branch ")).unwrap_or("").trim_start_matches("refs/heads/").to_string();
    }
    String::new()
}

/// statusEntriesOf: `git status --porcelain -z`, a rename's or copy's old name skipped.
pub fn status_entries_of(porcelain_z: &str) -> Vec<(String, String)> {
    let parts: Vec<&str> = porcelain_z.split('\0').collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < parts.len() {
        let entry = parts[i];
        i += 1;
        if entry.len() < 4 {
            continue;
        }
        let code = entry[..2].to_string();
        out.push((code.clone(), entry[3..].to_string()));
        if code.starts_with('R') || code.starts_with('C') {
            i += 1;
        }
    }
    out
}

/// changedPathsOf: the paths of `git status --porcelain`, a quoted one unquoted, as git names them.
pub fn changed_paths_of(porcelain: &str) -> Vec<String> {
    porcelain
        .split('\n')
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| l.len() > 3)
        .map(|l| {
            let p = &l[3..];
            p.strip_prefix('"').and_then(|q| q.strip_suffix('"')).unwrap_or(p).to_string()
        })
        .collect()
}

/// One turn committed on a branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchTurn {
    pub sha: String,
    pub subject: String,
    pub at: String,
    pub files: Vec<String>,
    pub merge: bool,
}

/// branchTurnsOf: the turns of `git log --name-only TURN_LOG_FORMAT`, as git prints them (newest first).
pub fn branch_turns_of(log: &str) -> Vec<BranchTurn> {
    log.split('\x1e')
        .map(|r| r.replace('\r', "").trim().to_string())
        .filter(|r| !r.is_empty())
        .filter_map(|record| {
            let mut lines = record.split('\n');
            let head = lines.next().unwrap_or("");
            let mut fields = head.split('\x1f');
            let sha = fields.next().unwrap_or("").to_string();
            let subject = fields.next().unwrap_or("").to_string();
            let at = fields.next().unwrap_or("").to_string();
            let parents = fields.next().unwrap_or("");
            let merge = parents.split_whitespace().count() > 1;
            let files = lines.map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).collect();
            let full = sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
            full.then_some(BranchTurn { sha, subject, at, files, merge })
        })
        .collect()
}

/// worktreeAddArgs: `worktree add` on a new branch from `from` (HEAD or a recorded commit), or on one that exists.
pub fn worktree_add_args(path: &str, branch: &str, branch_exists: bool, from: &str) -> Result<Vec<String>, String> {
    if from != "HEAD" && !(from.len() == 40 && from.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())) {
        return Err("That is not a commit Locust recorded.".into());
    }
    let mut args: Vec<String> = Vec::new();
    if cfg!(windows) {
        args.extend(["-c".into(), "core.longpaths=true".into()]);
    }
    args.extend(["worktree".into(), "add".into()]);
    if branch_exists {
        args.extend([path.into(), branch.into()]);
    } else {
        args.extend(["-b".into(), branch.into(), path.into(), from.into()]);
    }
    Ok(args)
}

// ---- running git ------------------------------------------------------------------------------------------------------

/// A git command that did not succeed: what it said, what it printed, and its exit code (None when it was stopped).
#[derive(Clone, Debug)]
pub struct GitError {
    pub message: String,
    pub stdout: String,
    pub exit: Option<i32>,
}

/// git with Locust's own config first, bounded by `limit`.
pub fn run_git(args: &[&str], cwd: &Path, limit: Duration) -> Result<String, GitError> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    let first = args.first().copied().unwrap_or("");
    let fail = |message: String| GitError { message, stdout: String::new(), exit: None };
    let mut child = Command::new("git")
        .args(OWN_GIT_CONFIG)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| fail(format!("git {first}: {e}")))?;
    let (mut out, mut err) = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let (mut o, mut e) = (Vec::new(), Vec::new());
        let _ = (&mut out).take(64 * 1024 * 1024).read_to_end(&mut o);
        let _ = (&mut err).take(1024 * 1024).read_to_end(&mut e);
        let _ = tx.send((o, e));
    });
    match rx.recv_timeout(limit) {
        Ok((o, e)) => {
            let status = child.wait().map_err(|e| fail(format!("git {first}: {e}")))?;
            let stdout = String::from_utf8_lossy(&o).into_owned();
            if status.success() {
                Ok(stdout)
            } else {
                let said = String::from_utf8_lossy(&e).trim().chars().take(300).collect::<String>();
                Err(GitError { message: format!("git {first}: {said}"), stdout, exit: status.code() })
            }
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(fail(format!("git {first}: took longer than {} s and was stopped.", limit.as_secs())))
        }
    }
}

// ---- landing (Electron's LandBlock, LandPreview, LandResult) ------------------------------------------------------

/// What stands in the way of landing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LandBlock {
    Nothing,
    Detached,
    OldGit(Option<String>),
    Unsaved(Vec<String>),
    Markers(Vec<String>),
    MarkersUnchecked,
    YourChanges(Vec<String>),
    Conflicts(Vec<String>),
    Merging,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LandPreview {
    pub branch: String,
    pub onto: Option<String>,
    pub files: Vec<String>,
    pub draft: String,
    pub block: Option<LandBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LandResult {
    Landed { sha: String, onto: String, files: Vec<String>, branch_reset: bool },
    Blocked(LandBlock),
    Refused(String),
}

/// Landing checks for conflicts with `git merge-tree --write-tree`, which needs git 2.38.
pub fn git_version_can_land(version: &str) -> bool {
    let mut parts = version.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
    let (major, minor) = (parts.next().unwrap_or(0), parts.next().unwrap_or(0));
    major > 2 || (major == 2 && minor >= 38)
}

/// landingDraft: one ask plainly, several as a list, with the teammate and each route as trailers.
pub fn landing_draft(name: &str, subjects: &[String], routes: &[String]) -> String {
    let first = subjects.first().cloned().unwrap_or_else(|| format!("{name}'s work"));
    let body = if subjects.len() <= 1 { String::new() } else { format!("\n\n{}", subjects.iter().map(|s| format!("- {s}")).collect::<Vec<_>>().join("\n")) };
    let mut seen: Vec<String> = Vec::new();
    for route in routes.iter().map(|r| r.trim()).filter(|r| !r.is_empty()) {
        if !seen.iter().any(|s| s == route) {
            seen.push(route.to_string());
        }
    }
    let mut trailers = vec![format!("Locust-Teammate: {name}")];
    trailers.extend(seen.into_iter().map(|r| format!("Locust-Route: {r}")));
    format!("{first}{body}\n\n{}", trailers.join("\n"))
}

// ---- the manager ------------------------------------------------------------------------------------------------------

/// Whether own copies can be made in this folder, and why not.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Probe {
    pub repository: bool,
    pub git_version: Option<String>,
    pub reason: Option<String>,
}

/// What a turn's commit says (Electron's CheckpointMessage).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckpointMessage {
    pub subject: String,
    pub body: Option<String>,
    pub trailers: Vec<(String, String)>,
    pub author_name: String,
    pub author_email: String,
}

/// What committing a turn came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Checkpoint {
    /// The turn changed nothing.
    Clean,
    /// Every change was a file too big to keep.
    Skipped { skipped: Vec<(String, u64)> },
    Committed {
        sha: String,
        branch: String,
        files: Vec<String>,
        skipped: Vec<(String, u64)>,
        /// The turn finished a merge begun to resolve a conflict (`start_resolving`).
        merge_finished: bool,
        /// After a merge: the files still carrying conflict markers; None when they could not be checked.
        still_marked: Option<Vec<String>>,
    },
}

/// A teammate's branch against where it left the person's branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BranchReview {
    pub branch: String,
    pub against: Option<String>,
    pub base: String,
    /// Oldest first.
    pub turns: Vec<BranchTurn>,
    /// None when too big to show whole, or unreadable.
    pub diff: Option<String>,
    pub uncommitted: Vec<String>,
}

/// The copy has uncommitted work, and removing it would delete it.
pub fn has_changes_message(changes: &[String]) -> String {
    let (n, one) = (changes.len(), changes.len() == 1);
    format!(
        "It has {n} uncommitted {}, and removing it would delete {}.",
        if one { "change" } else { "changes" },
        if one { "it" } else { "them" }
    )
}

fn safe_teammate_directory(teammate_id: &str) -> Result<&str, String> {
    let ok = (1..=64).contains(&teammate_id.len()) && teammate_id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if ok { Ok(teammate_id) } else { Err("That teammate id cannot name a worktree.".into()) }
}

fn same_folder(a: &Path, b: &Path) -> bool {
    let spell = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()).to_string_lossy().replace('\\', "/").to_lowercase();
    spell(a) == spell(b)
}

/// The admin folder a tree's `.git` file names, only when it is inside this repository's `.git/worktrees`.
fn admin_directory_of(git_file: &Path, root: &Path) -> Option<PathBuf> {
    let text = std::fs::read_to_string(git_file).ok()?;
    let named = text.lines().find_map(|l| l.strip_prefix("gitdir:")).map(str::trim)?;
    let admin = git_file.parent()?.join(named);
    let admin = std::fs::canonicalize(&admin).unwrap_or(admin);
    let worktrees = std::fs::canonicalize(root.join(".git").join("worktrees")).unwrap_or_else(|_| root.join(".git").join("worktrees"));
    let inside = admin.strip_prefix(&worktrees).ok()?;
    (!inside.as_os_str().is_empty()).then_some(admin)
}

pub struct Manager {
    root: PathBuf,
    trees: String,
}

impl Manager {
    /// A manager for the project at `workspace` (its real spelling, as git prints it).
    pub fn new(workspace: &Path) -> Self {
        Manager { root: std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf()), trees: WORKTREE_DIR.into() }
    }

    fn git(&self, args: &[&str], cwd: &Path) -> Result<String, String> {
        run_git(args, cwd, GIT_TIMEOUT).map_err(|e| e.message)
    }

    fn tree_path(&self, teammate_id: &str) -> Result<PathBuf, String> {
        Ok(self.root.join(&self.trees).join(safe_teammate_directory(teammate_id)?))
    }

    pub fn probe(&self) -> Probe {
        let Ok(version_text) = self.git(&["--version"], &self.root) else {
            return Probe { repository: false, git_version: None, reason: Some("git was not found on this machine.".into()) };
        };
        let git_version = parse_git_version(&version_text);
        if let Some(v) = &git_version
            && !git_version_supports_worktrees(v)
        {
            return Probe { repository: false, reason: Some(format!("git {v} is older than 2.5, which worktrees need.")), git_version };
        }
        match self.git(&["rev-parse", "--show-toplevel"], &self.root) {
            Ok(top) if same_folder(Path::new(top.trim()), &self.root) => Probe { repository: true, git_version, reason: None },
            Ok(_) => Probe {
                repository: false,
                git_version,
                reason: Some("The project folder is inside a repository but is not its root. Own copies need the folder to be the repository root.".into()),
            },
            Err(_) => Probe { repository: false, git_version, reason: Some("The project folder is not a git repository.".into()) },
        }
    }

    /// `.locust/` goes in the repository's own exclude file, once, so the trees never show as changes.
    fn exclude_locust_directory(&self) -> Result<(), String> {
        let info = self.root.join(".git").join("info");
        let exclude = info.join("exclude");
        let current = std::fs::read_to_string(&exclude).unwrap_or_default();
        if current.lines().any(|l| matches!(l.trim(), ".locust/" | "/.locust/" | ".locust")) {
            return Ok(());
        }
        std::fs::create_dir_all(&info).map_err(|e| e.to_string())?;
        let sep = if !current.is_empty() && !current.ends_with('\n') { "\n" } else { "" };
        std::fs::write(&exclude, format!("{current}{sep}.locust/\n")).map_err(|e| e.to_string())
    }

    /// A tree left half-made (git's `initializing` lock) is finished, keeping what is in it, rather than handed out:
    /// a half-made tree reads every file as deleted, and committing it would delete the project on its branch.
    fn finish_making(&self, path: &Path, admin: &Path) -> Result<(), String> {
        let _ = std::fs::remove_file(admin.join("index.lock"));
        run_git(&["reset", "-q"], path, CHECKOUT_TIMEOUT).map_err(|e| e.message)?;
        let _ = run_git(&["checkout-index", "-a", "-q"], path, CHECKOUT_TIMEOUT);
        let missing = run_git(&["ls-files", "--deleted"], path, CHECKOUT_TIMEOUT).map_err(|e| e.message)?;
        if !missing.trim().is_empty() {
            return Err("This teammate's own branch was left half-made, and Locust could not finish it. Remove it on the Worktrees screen and it will be made again.".into());
        }
        self.git(&["worktree", "unlock", &path.to_string_lossy()], &self.root).map(|_| ())
    }

    fn locked_initializing(admin: &Path) -> bool {
        std::fs::read_to_string(admin.join("locked")).is_ok_and(|l| l.trim() == "initializing")
    }

    /// The teammate's tree, made if it is not there yet: under `.locust/worktrees/<id>` on its own branch.
    /// Asking again returns the same tree.
    pub fn ensure(&self, teammate_id: &str, name: &str, from: Option<&str>) -> Result<PathBuf, String> {
        let path = self.tree_path(teammate_id)?;
        if path.join(".git").is_file() {
            let admin = admin_directory_of(&path.join(".git"), &self.root);
            match admin {
                Some(admin) if Self::locked_initializing(&admin) => {
                    self.finish_making(&path, &admin)?;
                    return Ok(path);
                }
                _ => return Ok(path),
            }
        }
        let ready = self.probe();
        if !ready.repository {
            return Err(ready.reason.unwrap_or_else(|| "Own copies are not available in this folder.".into()));
        }
        std::fs::create_dir_all(self.root.join(&self.trees)).map_err(|e| e.to_string())?;
        self.exclude_locust_directory()?;
        let named = branch_name_for(name, Some(teammate_id));
        let checked_out: Vec<String> = self
            .git(&["worktree", "list", "--porcelain"], &self.root)
            .map(|l| l.lines().filter_map(|x| x.strip_prefix("branch ")).map(|b| b.trim().trim_start_matches("refs/heads/").to_string()).collect())
            .unwrap_or_default();
        let branch = if checked_out.contains(&named) { distinct_branch_name_for(name, teammate_id) } else { named };
        let branch_exists = self.git(&["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")], &self.root).is_ok();
        let path_text = path.to_string_lossy().into_owned();
        let args = worktree_add_args(&path_text, &branch, branch_exists, from.unwrap_or("HEAD"))?;
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        if let Err(error) = run_git(&args, &self.root, CHECKOUT_TIMEOUT) {
            // A checkout hook that failed after git made a usable tree: kept only when the tree is verified finished.
            let finished = error.exit.is_some_and(|c| c > 0) && {
                let listing = self.git(&["worktree", "list", "--porcelain"], &self.root).unwrap_or_default();
                recorded_branch_of(&listing, &path) == branch
                    && path.join(".git").is_file()
                    && admin_directory_of(&path.join(".git"), &self.root).is_some_and(|a| !Self::locked_initializing(&a))
                    && self.git(&["rev-parse", "--verify", "HEAD"], &path).is_ok()
            };
            if !finished {
                return Err(error.message);
            }
        }
        Ok(path)
    }

    /// The trees Locust made in this folder.
    pub fn list(&self) -> Vec<WorktreeInfo> {
        self.git(&["worktree", "list", "--porcelain"], &self.root)
            .map(|l| parse_worktree_list(&l, &self.root, &self.trees))
            .unwrap_or_default()
    }

    /// Remove a teammate's copy. A clean one goes at once, with git's own unforced remove. One with uncommitted work
    /// is refused, naming the changes, unless `discard` names exactly those changes; one made since is refused again.
    pub fn remove(&self, teammate_id: &str, discard: Option<&[String]>) -> Result<(), String> {
        let path = self.tree_path(teammate_id)?;
        let changes = changed_paths_of(&self.git(&["status", "--porcelain", "--untracked-files=all"], &path)?);
        let path_text = path.to_string_lossy().into_owned();
        if changes.is_empty() {
            return self.git(&["worktree", "remove", &path_text], &self.root).map(|_| ());
        }
        let agreed: std::collections::HashSet<&String> = discard.unwrap_or(&[]).iter().collect();
        if discard.is_none() || changes.len() != agreed.len() || changes.iter().any(|c| !agreed.contains(c)) {
            return Err(has_changes_message(&changes));
        }
        self.git(&["worktree", "remove", "--force", &path_text], &self.root).map(|_| ())
    }

    /// The uncommitted changes in a teammate's copy, as git status names them: what removing it would delete.
    pub fn uncommitted(&self, teammate_id: &str) -> Result<Vec<String>, String> {
        let path = self.tree_path(teammate_id)?;
        Ok(changed_paths_of(&self.git(&["status", "--porcelain", "--untracked-files=all"], &path)?))
    }

    /// filesWithMarkers: which of `files` at `reference` still carry conflict markers; None when git could not say.
    fn files_with_markers(&self, cwd: &Path, reference: &str, files: &[String]) -> Option<Vec<String>> {
        if files.is_empty() {
            return Some(Vec::new());
        }
        let mut args: Vec<&str> = vec!["grep", "-l", "-E", "-e", "^(<<<<<<<|>>>>>>>)( |$)", reference, "--"];
        args.extend(files.iter().map(String::as_str));
        let prefix = format!("{reference}:");
        match run_git(&args, cwd, GIT_TIMEOUT) {
            Ok(said) => Some(
                said.lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .map(|l| l.strip_prefix(&prefix).unwrap_or(l).to_string())
                    .collect(),
            ),
            // grep's "found nothing" is exit 1 with nothing printed.
            Err(e) if e.exit == Some(1) && e.stdout.trim().is_empty() => Some(Vec::new()),
            Err(_) => None,
        }
    }

    /// What landing the branch would do, and what stands in its way, before anything is touched.
    pub fn land_preview(&self, teammate_id: &str, name: &str) -> Result<LandPreview, String> {
        self.gather_landing(teammate_id, name).map(|(preview, _)| preview)
    }

    fn gather_landing(&self, teammate_id: &str, name: &str) -> Result<(LandPreview, PathBuf), String> {
        let path = self.made_tree(teammate_id)?;
        let branch = self.git(&["symbolic-ref", "--short", "-q", "HEAD"], &path)?.trim().to_string();
        let onto_name = self.git(&["symbolic-ref", "--short", "-q", "HEAD"], &self.root).unwrap_or_default().trim().to_string();
        let onto = (!onto_name.is_empty()).then_some(onto_name);
        let base = self.git(&["merge-base", "HEAD", &branch], &self.root)?.trim().to_string();
        let files: Vec<String> =
            self.git(&["diff", "--name-only", "-z", &base, &branch], &self.root)?.split('\0').filter(|f| !f.is_empty()).map(str::to_owned).collect();
        let range = format!("{base}..{branch}");
        let mut subjects: Vec<String> = branch_turns_of(&self.git(&["log", "--name-only", TURN_LOG_FORMAT, &range], &self.root)?)
            .into_iter()
            .filter(|t| !t.merge)
            .map(|t| t.subject)
            .collect();
        subjects.reverse();
        let routes: Vec<String> = self
            .git(&["log", "--format=%(trailers:key=Locust-Route,valueonly,separator=%x0a)", &range], &self.root)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect();
        let draft = landing_draft(name, &subjects, &routes);
        let preview = |block: Option<LandBlock>| LandPreview { branch: branch.clone(), onto: onto.clone(), files: files.clone(), draft: draft.clone(), block };
        let version = parse_git_version(&self.git(&["--version"], &self.root).unwrap_or_default());
        if !version.as_deref().is_some_and(git_version_can_land) {
            return Ok((preview(Some(LandBlock::OldGit(version))), path));
        }
        if onto.is_none() {
            return Ok((preview(Some(LandBlock::Detached)), path));
        }
        if self.git(&["rev-parse", "-q", "--verify", "MERGE_HEAD"], &path).is_ok() {
            return Ok((preview(Some(LandBlock::Merging)), path));
        }
        let unsaved: Vec<String> = status_entries_of(&self.git(&["status", "--porcelain", "-z", "--untracked-files=all"], &path)?).into_iter().map(|(_, p)| p).collect();
        if !unsaved.is_empty() {
            return Ok((preview(Some(LandBlock::Unsaved(unsaved))), path));
        }
        if files.is_empty() {
            return Ok((preview(Some(LandBlock::Nothing)), path));
        }
        match self.files_with_markers(&self.root, &branch, &files) {
            None => return Ok((preview(Some(LandBlock::MarkersUnchecked)), path)),
            Some(marked) if !marked.is_empty() => return Ok((preview(Some(LandBlock::Markers(marked))), path)),
            Some(_) => {}
        }
        let yours: Vec<String> = status_entries_of(&self.git(&["status", "--porcelain", "-z", "--untracked-files=all"], &self.root)?)
            .into_iter()
            .map(|(_, p)| p)
            .filter(|p| files.contains(p))
            .collect();
        if !yours.is_empty() {
            return Ok((preview(Some(LandBlock::YourChanges(yours))), path));
        }
        // git merge-tree checks the merge without touching the checkout (git 2.38).
        let conflicts: Vec<String> = match run_git(&["merge-tree", "--write-tree", "--name-only", "--no-messages", "HEAD", &branch], &self.root, GIT_TIMEOUT) {
            Ok(_) => Vec::new(),
            Err(e) => {
                let mut named: Vec<String> = e.stdout.lines().skip(1).map(str::trim).filter(|l| !l.is_empty()).map(str::to_owned).collect();
                named.dedup();
                if named.is_empty() { vec!["(git could not check for conflicts)".into()] } else { named }
            }
        };
        if !conflicts.is_empty() {
            return Ok((preview(Some(LandBlock::Conflicts(conflicts))), path));
        }
        Ok((preview(None), path))
    }

    /// Land the branch on the person's branch as one commit of theirs (their name, their hooks), then start the
    /// teammate's branch from it. Anything in the way is said and nothing is touched; a refused commit puts the
    /// checkout back exactly.
    pub fn land(&self, teammate_id: &str, name: &str, message: &str) -> Result<LandResult, String> {
        let (preview, path) = self.gather_landing(teammate_id, name)?;
        let onto = match (&preview.block, &preview.onto) {
            (None, Some(onto)) => onto.clone(),
            (block, _) => return Ok(LandResult::Blocked(block.clone().unwrap_or(LandBlock::Detached))),
        };
        let text = if message.trim().is_empty() { preview.draft.clone() } else { message.trim().to_string() };
        let put_back = || {
            let _ = run_git(&["reset", "-q", "--merge", "HEAD"], &self.root, GIT_TIMEOUT);
            let _ = std::fs::remove_file(self.root.join(".git").join("SQUASH_MSG"));
        };
        if let Err(e) = run_git(&["merge", "--squash", &preview.branch], &self.root, CHECKOUT_TIMEOUT) {
            put_back();
            return Ok(LandResult::Refused(format!("git could not stage the landing: {}", e.message)));
        }
        if let Err(e) = run_git(&["commit", "-q", "-m", &text], &self.root, CHECKOUT_TIMEOUT) {
            put_back();
            let said = e.message.clone();
            let lower = said.to_lowercase();
            return Ok(LandResult::Refused(if lower.contains("tell me who you are") || lower.contains("user.email") || lower.contains("user.name") {
                "git does not know your name and email yet, so it cannot make the commit as you. Set them with git config user.name and user.email, then land again.".into()
            } else {
                format!("Your commit hooks refused the landing commit: {}", said.trim_start_matches("git commit: "))
            }));
        }
        let sha = self.git(&["rev-parse", "HEAD"], &self.root)?.trim().to_string();
        let branch_reset = self.git(&["reset", "-q", "--keep", &sha], &path).is_ok();
        Ok(LandResult::Landed { sha, onto, files: preview.files, branch_reset })
    }

    /// Hand a conflict back to the teammate: merge the person's branch into its copy, uncommitted, so its next turn
    /// resolves it there. Returns the files left in conflict.
    pub fn start_resolving(&self, teammate_id: &str) -> Result<Vec<String>, String> {
        let path = self.made_tree(teammate_id)?;
        let onto = self.git(&["symbolic-ref", "--short", "-q", "HEAD"], &self.root)?.trim().to_string();
        let _ = run_git(&["merge", "--no-ff", "--no-commit", &onto], &path, CHECKOUT_TIMEOUT);
        Ok(self.git(&["diff", "--name-only", "-z", "--diff-filter=U"], &path)?.split('\0').filter(|f| !f.is_empty()).map(str::to_owned).collect())
    }

    /// The tree, only when it is whole: a half-made one is never committed or read.
    fn made_tree(&self, teammate_id: &str) -> Result<PathBuf, String> {
        let path = self.tree_path(teammate_id)?;
        if !path.join(".git").is_file() {
            return Err("This teammate has no own branch in this folder.".into());
        }
        match admin_directory_of(&path.join(".git"), &self.root) {
            Some(admin) if !Self::locked_initializing(&admin) => Ok(path),
            _ => Err("This teammate's own branch is not finished being made.".into()),
        }
    }

    /// Commit everything the turn changed, new files too, as the teammate, past any hook that would refuse, and never
    /// on the person's branch. A new file over 50 MB is left out and named.
    pub fn checkpoint(&self, teammate_id: &str, message: &CheckpointMessage) -> Result<Checkpoint, String> {
        let path = self.made_tree(teammate_id)?;
        let entries = status_entries_of(&self.git(&["status", "--porcelain", "-z", "--untracked-files=all"], &path)?);
        let merging = self.git(&["rev-parse", "-q", "--verify", "MERGE_HEAD"], &path).is_ok();
        if entries.is_empty() && !merging {
            return Ok(Checkpoint::Clean);
        }
        let skipped: Vec<(String, u64)> = entries
            .iter()
            .filter(|(code, _)| code == "??")
            .filter_map(|(_, p)| {
                let bytes = std::fs::metadata(path.join(p)).map(|m| m.len()).unwrap_or(0);
                (bytes > CHECKPOINT_MAX_FILE_BYTES).then(|| (p.clone(), bytes))
            })
            .collect();
        run_git(&["add", "-A"], &path, CHECKOUT_TIMEOUT).map_err(|e| e.message)?;
        if !skipped.is_empty() {
            let mut args = vec!["reset", "-q", "--"];
            args.extend(skipped.iter().map(|(p, _)| p.as_str()));
            self.git(&args, &path)?;
        }
        let files: Vec<String> =
            self.git(&["diff", "--cached", "--name-only", "-z"], &path)?.split('\0').filter(|f| !f.is_empty()).map(str::to_owned).collect();
        if files.is_empty() && !merging {
            return Ok(if skipped.is_empty() { Checkpoint::Clean } else { Checkpoint::Skipped { skipped } });
        }
        let trailers = message
            .trailers
            .iter()
            .map(|(k, v)| format!("{k}: {}", v.split_whitespace().collect::<Vec<_>>().join(" ")))
            .collect::<Vec<_>>()
            .join("\n");
        let (name_cfg, email_cfg) = (format!("user.name={}", message.author_name), format!("user.email={}", message.author_email));
        let mut args: Vec<&str> =
            vec!["-c", &name_cfg, "-c", &email_cfg, "-c", "commit.gpgsign=false", "commit", "--no-verify", "-q", "-m", &message.subject];
        if let Some(body) = message.body.as_deref().filter(|b| !b.trim().is_empty()) {
            args.extend(["-m", body]);
        }
        if !trailers.is_empty() {
            args.extend(["-m", &trailers]);
        }
        run_git(&args, &path, CHECKOUT_TIMEOUT).map_err(|e| e.message)?;
        let sha = self.git(&["rev-parse", "HEAD"], &path)?.trim().to_string();
        let branch = self.git(&["symbolic-ref", "--short", "-q", "HEAD"], &path).unwrap_or_default().trim().to_string();
        let still_marked = if merging { self.files_with_markers(&path, "HEAD", &files) } else { Some(Vec::new()) };
        Ok(Checkpoint::Committed { sha, branch, files, skipped, merge_finished: merging, still_marked })
    }

    /// The branch against where it left the person's branch: whole, and turn by turn, oldest first.
    pub fn review(&self, teammate_id: &str) -> Result<BranchReview, String> {
        let path = self.made_tree(teammate_id)?;
        let branch = self.git(&["symbolic-ref", "--short", "-q", "HEAD"], &path)?.trim().to_string();
        let against = self.git(&["symbolic-ref", "--short", "-q", "HEAD"], &self.root).unwrap_or_default().trim().to_string();
        let base = self.git(&["merge-base", "HEAD", &branch], &self.root)?.trim().to_string();
        let range = format!("{base}..{branch}");
        let mut turns = branch_turns_of(&self.git(&["log", "--name-only", TURN_LOG_FORMAT, &range], &self.root)?);
        turns.reverse();
        let mut diff_args: Vec<&str> = vec!["diff"];
        diff_args.extend(PLAIN_DIFF);
        diff_args.extend([base.as_str(), branch.as_str()]);
        let diff = self.git(&diff_args, &self.root).ok().filter(|d| d.len() <= REVIEW_MAX_DIFF_BYTES);
        let uncommitted = status_entries_of(&self.git(&["status", "--porcelain", "-z", "--untracked-files=all"], &path)?)
            .into_iter()
            .map(|(_, p)| p)
            .collect();
        Ok(BranchReview { branch, against: (!against.is_empty()).then_some(against), base, turns, diff, uncommitted })
    }

    /// One turn's diff, only for a turn on this branch.
    pub fn turn_diff(&self, teammate_id: &str, sha: &str) -> Result<String, String> {
        let not_here = || "That is not a turn on this branch.".to_string();
        if !(sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())) {
            return Err(not_here());
        }
        let path = self.made_tree(teammate_id)?;
        let branch = self.git(&["symbolic-ref", "--short", "-q", "HEAD"], &path)?.trim().to_string();
        let base = self.git(&["merge-base", "HEAD", &branch], &self.root)?.trim().to_string();
        let turns = self.git(&["rev-list", &format!("{base}..{branch}")], &self.root)?;
        if !turns.lines().any(|l| l.trim() == sha) {
            return Err(not_here());
        }
        let mut args: Vec<&str> = vec!["show"];
        args.extend(PLAIN_DIFF);
        args.extend(["--format=", sha]);
        self.git(&args, &self.root)
    }
}

// ---- a turn's commit and its receipt (Electron main/turn-checkpoint.ts) ---------------------------------------------

/// The receipt's diagnostic code in the turn's stream.
pub const CHECKPOINT_CODE: &str = "host.turn_checkpoint";
const SUBJECT_MAX: usize = 72;
const SUMMARY_MAX: usize = 200;
/// A routine step's own words follow this (shared/hand-off.ts STEP_MARK).
const STEP_MARK: &str = "Your step: ";
/// A tagged turn's context starts here (shared/tagging.ts CONTEXT_MARK).
const CONTEXT_MARK: &str = "\n\n(You were tagged in ";

/// How a turn ended, for its commit's trailer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnOutcome {
    Completed,
    Failed,
    Stopped,
}

impl TurnOutcome {
    fn word(self) -> &'static str {
        match self {
            TurnOutcome::Completed => "completed",
            TurnOutcome::Failed => "failed",
            TurnOutcome::Stopped => "stopped",
        }
    }
}

/// The person's words, not the host text around them: a routine step's own words, and a tag's context cut off.
fn persons_words(prompt: &str) -> &str {
    let stepped = match prompt.rfind(&format!("\n\n{STEP_MARK}")) {
        Some(at) => &prompt[at + 2 + STEP_MARK.len()..],
        None => prompt,
    };
    match stepped.find(CONTEXT_MARK) {
        Some(at) => &stepped[..at],
        None => stepped,
    }
}

/// firstLine: the first sentence when it fits, else the line when it fits, else cut at a word, never mid-word.
fn first_line(text: &str, max: usize) -> String {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    let sentence = crate::invariants::regex(r"^.+?[.!?](?:\s|$)").find(line).map(|m| m.as_str().trim_end().to_string());
    if let Some(sentence) = sentence.filter(|s| s.chars().count() <= max) {
        return sentence;
    }
    if line.chars().count() <= max {
        return line.to_string();
    }
    let cut: String = line.chars().take(max - 1).collect();
    let kept = match cut.rfind(' ') {
        Some(space) if space > max / 2 => &cut[..space],
        _ => cut.as_str(),
    };
    format!("{}…", kept.trim_end())
}

/// routeLabel: the agent and its model, the agent named once.
pub fn route_label(runtime: &str, model: Option<&str>) -> String {
    match model {
        None => runtime.to_string(),
        Some(m) => format!("{runtime} / {}", m.strip_prefix(&format!("{runtime}/")).unwrap_or(m)),
    }
}

/// checkpointMessage: the subject from what the person asked, the answer's first line as the body, and trailers that
/// name the teammate, the conversation, the route and how the turn ended. The author is the teammate.
#[allow(clippy::too_many_arguments)]
pub fn checkpoint_message(
    prompt: &str,
    answer: Option<&str>,
    teammate_id: &str,
    teammate_name: &str,
    mission_id: &str,
    runtime: &str,
    model: Option<&str>,
    outcome: TurnOutcome,
) -> CheckpointMessage {
    let asked = first_line(persons_words(prompt), SUBJECT_MAX);
    CheckpointMessage {
        subject: if asked.is_empty() { format!("{teammate_name}'s turn") } else { asked },
        body: answer.filter(|a| !a.trim().is_empty()).map(|a| first_line(a, SUMMARY_MAX)),
        trailers: vec![
            ("Locust-Teammate".into(), teammate_name.into()),
            ("Locust-Mission".into(), mission_id.into()),
            ("Locust-Route".into(), route_label(runtime, model)),
            ("Locust-Turn".into(), outcome.word().into()),
        ],
        author_name: teammate_name.into(),
        author_email: format!("{teammate_id}@teammates.locust"),
    }
}

fn file_list(files: &[String]) -> String {
    if files.len() <= 4 { files.join(", ") } else { format!("{} and {} more", files[..3].join(", "), files.len() - 3) }
}

fn megabytes(bytes: u64) -> String {
    format!("{} MB", (bytes as f64 / (1024.0 * 1024.0)).round() as u64)
}

/// checkpointSentence: what the receipt says. Nothing when the turn changed nothing; a failure keeps the files.
pub fn checkpoint_sentence(result: &Result<Checkpoint, String>) -> Option<String> {
    let left = |skipped: &[(String, u64)]| {
        if skipped.is_empty() {
            String::new()
        } else {
            format!(" Left out, too big to commit: {}.", skipped.iter().map(|(p, b)| format!("{p} ({})", megabytes(*b))).collect::<Vec<_>>().join(", "))
        }
    };
    match result {
        Ok(Checkpoint::Clean) => None,
        Err(message) => Some(format!("This turn's changes could not be saved on the teammate's branch: {message} They are still in its folder.")),
        Ok(Checkpoint::Skipped { skipped }) => Some(format!("Nothing from this turn was saved on the branch.{}", left(skipped))),
        Ok(Checkpoint::Committed { sha, branch, skipped, merge_finished: true, still_marked, .. }) => {
            let short = &sha[..sha.len().min(12)];
            Some(match still_marked {
                Some(marked) if !marked.is_empty() => {
                    let many = marked.len() > 1;
                    format!(
                        "Saved this turn on {branch} as {short}, but {} still {} conflict markers, so it cannot land until {} resolved.{}",
                        file_list(marked),
                        if many { "have" } else { "has" },
                        if many { "they are" } else { "it is" },
                        left(skipped)
                    )
                }
                None => format!(
                    "Saved this turn on {branch} as {short}. Locust could not check it for conflict markers, so it does not say it can land; Land checks again first.{}",
                    left(skipped)
                ),
                Some(_) => format!("Saved this turn on {branch} as {short}: the merge is finished, so it can land now.{}", left(skipped)),
            })
        }
        Ok(Checkpoint::Committed { sha, branch, files, skipped, .. }) => {
            let n = files.len();
            Some(format!(
                "Saved this turn on {branch} as {}: {n} {} ({}).{}",
                &sha[..sha.len().min(12)],
                if n == 1 { "file" } else { "files" },
                file_list(files),
                left(skipped)
            ))
        }
    }
}

#[cfg(test)]
#[path = "worktrees_tests.rs"]
mod tests;

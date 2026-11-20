//! Electron's `sols-review-is-answered.test.ts` (0.683), the cases about committing a folder: an adversarial review
//! found ways the commit could do the wrong thing, and each case here is one of them, in its own words. Real git in
//! temporary repositories, a stand-in for the network that records what it was asked.
//!
//! Not carried here: cases 1 and 2 and the last case are about `leavingNoLog`, which cuts the `error.log` Codex's
//! cloud commands leave. Rust runs no Codex cloud command and cuts no log anywhere, so there is no log to cut. Case 9
//! is in `worktrees_markers_tests.rs`, beside the private `files_with_markers` it checks.
use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Temp(PathBuf);
impl Temp {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "locust-sol-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).expect("temp folder");
        Self(p.canonicalize().expect("canonical temp"))
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn sh(args: &[&str], cwd: &Path) -> String {
    let out = Command::new("git").args(args).current_dir(cwd).output().expect("git runs");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// Electron's `repository()`: one commit of `cart.py` on main, no remote.
fn repository() -> Temp {
    let folder = Temp::new("ws");
    sh(&["init", "-q", "-b", "main"], &folder.0);
    sh(&["config", "user.email", "you@example.test"], &folder.0);
    sh(&["config", "user.name", "You"], &folder.0);
    sh(&["config", "commit.gpgsign", "false"], &folder.0);
    fs::write(folder.0.join("cart.py"), "x = 0\n").expect("write");
    sh(&["add", "."], &folder.0);
    sh(&["commit", "-q", "-m", "first"], &folder.0);
    folder
}

fn hook(folder: &Path, name: &str, body: &str) {
    let path = folder.join(".git/hooks").join(name);
    fs::write(&path, body).expect("hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
}

/// Electron's `noNetwork` (every call fails), or its `signedIn` (gh auth answers, nothing else does). Every call is
/// recorded either way.
struct Net {
    signed_in: bool,
    calls: Arc<Mutex<Vec<Vec<String>>>>,
}
impl Network for Net {
    fn run(&self, program: &str, args: &[String], _cwd: &Path) -> Result<String, String> {
        let mut call = vec![program.to_owned()];
        call.extend(args.iter().cloned());
        self.calls.lock().expect("calls").push(call);
        if self.signed_in && program == "gh" && args.first().map(String::as_str) == Some("auth") {
            Ok("ok".into())
        } else {
            Err("no network in this test".into())
        }
    }
}

fn commits(signed_in: bool) -> (FolderCommits, Arc<Mutex<Vec<Vec<String>>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    (FolderCommits::new(Box::new(Net { signed_in, calls: calls.clone() })), calls)
}

fn shown(files: &[&str]) -> Vec<String> {
    files.iter().map(|f| (*f).to_owned()).collect()
}

// case: 3. a branch named like a flag is refused before git is ever asked to push it
#[test]
fn a_branch_named_like_a_flag_is_refused_before_git_is_ever_asked_to_push_it() {
    let folder = repository();
    sh(&["symbolic-ref", "HEAD", "refs/heads/--mirror"], &folder.0);
    fs::write(folder.0.join("cart.py"), "x = 1\n").expect("write");
    let (c, calls) = commits(false);
    assert_eq!(c.changes(&folder.0), FolderChanges::Blocked { why: Blocked::BranchName, files: Vec::new() });
    let refused = c.commit(&folder.0, "x", Then::Push, Some(&shown(&["cart.py"])), None);
    assert_eq!(refused, CommitResult::Refused(blocked_sentence(Blocked::BranchName).into()));
    assert!(calls.lock().expect("calls").iter().all(|call| !call.iter().any(|a| a == "push")), "no push was asked for");
    assert_eq!(sh(&["rev-list", "--count", "main"], &folder.0).trim(), "1", "nothing was committed");
}

// case: 4. a .locust file already staged is never committed
#[test]
fn a_dot_locust_file_already_staged_is_never_committed() {
    let folder = repository();
    fs::create_dir_all(folder.0.join(".locust")).expect("dir");
    fs::write(folder.0.join(".locust/private.json"), "{}").expect("write");
    sh(&["add", "-f", ".locust/private.json"], &folder.0);
    fs::write(folder.0.join("cart.py"), "x = 1\n").expect("write");
    let (c, _) = commits(false);
    let done = c.commit(&folder.0, "Change x", Then::Commit, Some(&shown(&["cart.py"])), None);
    assert!(matches!(done, CommitResult::Done { .. }), "{done:?}");
    assert_eq!(sh(&["show", "--name-only", "--format=", "HEAD"], &folder.0).trim(), "cart.py");
}

// case: 5. a refused commit leaves alone what the person staged while the hook ran, and says so
#[test]
fn a_refused_commit_leaves_alone_what_the_person_staged_while_the_hook_ran_and_says_so() {
    let folder = repository();
    fs::write(folder.0.join("cart.py"), "x = 1\n").expect("write");
    // The hook stands in for the person staging something new meanwhile, then refuses.
    hook(&folder.0, "pre-commit", "#!/bin/sh\necho theirs > theirs.md\ngit add theirs.md\necho refused >&2\nexit 1\n");
    let (c, _) = commits(false);
    let refused = c.commit(&folder.0, "Change x", Then::Commit, Some(&shown(&["cart.py"])), None);
    let CommitResult::Refused(message) = refused else { panic!("{refused:?}") };
    assert!(message.contains("What is staged changed while the commit ran"), "{message}");
    let staged = sh(&["diff", "--cached", "--name-only"], &folder.0);
    assert!(staged.lines().any(|l| l == "theirs.md"), "{staged}");
    assert_eq!(sh(&["rev-list", "--count", "HEAD"], &folder.0).trim(), "1");
}

// case: 6. the commit is of the files the person was shown, or nothing
#[test]
fn the_commit_is_of_the_files_the_person_was_shown_or_nothing() {
    let folder = repository();
    fs::write(folder.0.join("cart.py"), "x = 1\n").expect("write");
    let (c, _) = commits(false);
    // Shown cart.py; then a file appears before the press.
    fs::write(folder.0.join("late.md"), "late\n").expect("write");
    let refused = c.commit(&folder.0, "Change x", Then::Commit, Some(&shown(&["cart.py"])), None);
    assert!(matches!(&refused, CommitResult::Refused(m) if m.contains("changed since this was opened")), "{refused:?}");
    assert_eq!(sh(&["rev-list", "--count", "HEAD"], &folder.0).trim(), "1");
}

// case: 8. a checkout hook that fails after the switch leaves the person on their branch, and no new one
#[test]
fn a_checkout_hook_that_fails_after_the_switch_leaves_the_person_on_their_branch_and_no_new_one() {
    let folder = repository();
    let bare = Temp::new("remote");
    sh(&["init", "-q", "--bare", "-b", "main"], &bare.0);
    sh(&["remote", "add", "origin", "https://github.com/someone/pebble.git"], &folder.0);
    sh(&["remote", "set-url", "--push", "origin", &bare.0.to_string_lossy()], &folder.0);
    sh(&["push", "-q", "-u", "origin", "main"], &folder.0);
    sh(&["remote", "set-head", "origin", "main"], &folder.0);
    hook(&folder.0, "post-checkout", "#!/bin/sh\nexit 1\n");
    fs::write(folder.0.join("cart.py"), "x = 1\n").expect("write");
    let (c, _) = commits(true);
    let refused = c.commit(&folder.0, "Change x", Then::PullRequest, Some(&shown(&["cart.py"])), None);
    assert!(matches!(refused, CommitResult::Refused(_)), "{refused:?}");
    assert_eq!(sh(&["symbolic-ref", "--short", "HEAD"], &folder.0).trim(), "main");
    assert_eq!(sh(&["branch", "--list", "locust/*"], &folder.0).trim(), "");
}

// case: 10. a URL that only mentions github.com is not GitHub
#[test]
fn a_url_that_only_mentions_github_com_is_not_github() {
    let folder = repository();
    sh(&["remote", "add", "origin", "https://evil.example/github.com/owner/repo.git"], &folder.0);
    fs::write(folder.0.join("cart.py"), "x = 1\n").expect("write");
    let (c, _) = commits(true);
    let FolderChanges::Changes { remote: Some(remote), .. } = c.changes(&folder.0) else { panic!("changes with a remote") };
    assert!(!remote.pull_requests);
    // The control: the same folder with GitHub's own host does offer pull requests, so gh really was signed in.
    sh(&["remote", "set-url", "origin", "https://github.com/owner/repo.git"], &folder.0);
    let FolderChanges::Changes { remote: Some(remote), .. } = c.changes(&folder.0) else { panic!("changes with a remote") };
    assert!(remote.pull_requests);
}

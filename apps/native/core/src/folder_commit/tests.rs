//! Ports apps/desktop/src/main/the-folder-commits-as-you.test.ts: temp repositories only, real git, a stand-in gh.
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
            "locust-commit-{tag}-{}-{}",
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

/// A repository with a GitHub fetch URL and a local bare repository as its push URL, like the TS fixture.
fn repository() -> (Temp, Temp) {
    let folder = Temp::new("ws");
    let bare = Temp::new("remote");
    sh(&["init", "-q", "--bare", "-b", "main"], &bare.0);
    sh(&["init", "-q", "-b", "main"], &folder.0);
    sh(&["config", "user.email", "you@example.test"], &folder.0);
    sh(&["config", "user.name", "You"], &folder.0);
    sh(&["config", "commit.gpgsign", "false"], &folder.0);
    fs::write(folder.0.join("cart.py"), "def total(items):\n    return 0\n").expect("write");
    sh(&["add", "."], &folder.0);
    sh(&["commit", "-q", "-m", "first"], &folder.0);
    sh(&["remote", "add", "origin", "https://github.com/someone/pebble.git"], &folder.0);
    sh(&["remote", "set-url", "--push", "origin", &bare.0.to_string_lossy()], &folder.0);
    sh(&["push", "-q", "-u", "origin", "main"], &folder.0);
    sh(&["remote", "set-head", "origin", "main"], &folder.0);
    (folder, bare)
}

/// Real git for pushes; a stand-in gh that is signed in and opens pull request 7.
struct Fake(Arc<Mutex<Vec<Vec<String>>>>);
impl Network for Fake {
    fn run(&self, program: &str, args: &[String], cwd: &Path) -> Result<String, String> {
        let mut call = vec![program.to_owned()];
        call.extend(args.iter().cloned());
        self.0.lock().expect("calls").push(call);
        if program == "git" {
            let out = Command::new("git").args(args).current_dir(cwd).output().map_err(|e| e.to_string())?;
            return if out.status.success() {
                Ok(String::from_utf8_lossy(&out.stdout).into_owned())
            } else {
                Err(String::from_utf8_lossy(&out.stderr).into_owned())
            };
        }
        match args.first().map(String::as_str) {
            Some("auth") => Ok("Logged in".into()),
            Some("pr") => Ok("https://github.com/someone/pebble/pull/7\n".into()),
            _ => Err("unexpected gh call".into()),
        }
    }
}

fn commits() -> (FolderCommits, Arc<Mutex<Vec<Vec<String>>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    (FolderCommits::new(Box::new(Fake(calls.clone()))), calls)
}

#[test]
fn lists_what_a_commit_would_take_never_dot_locust() {
    let (folder, _bare) = repository();
    fs::write(folder.0.join("cart.py"), "def total(items):\n    return sum(items)\n").expect("write");
    fs::write(folder.0.join("notes.md"), "new\n").expect("write");
    fs::create_dir_all(folder.0.join(".locust/worktrees/tm_wren")).expect("dir");
    fs::write(folder.0.join(".locust/worktrees/tm_wren/x.txt"), "not yours to commit").expect("write");
    let (c, _) = commits();
    let seen = c.changes(&folder.0);
    let FolderChanges::Changes { branch, files, remote } = seen else { panic!("{seen:?}") };
    assert_eq!(branch, "main");
    assert_eq!(
        remote,
        Some(Remote { name: "origin".into(), upstream: true, default_branch: "main".into(), pull_requests: true })
    );
    assert_eq!(
        files,
        vec![
            ChangedFile { path: "cart.py".into(), status: ChangeStatus::Modified },
            ChangedFile { path: "notes.md".into(), status: ChangeStatus::Added },
        ]
    );
}

#[test]
fn commits_everything_as_you_with_your_hooks_and_says_what_it_did() {
    let (folder, _bare) = repository();
    fs::write(folder.0.join("cart.py"), "def total(items):\n    return sum(items)\n").expect("write");
    fs::create_dir_all(folder.0.join(".locust")).expect("dir");
    fs::write(folder.0.join(".locust/keep.txt"), "no").expect("write");
    let (c, _) = commits();
    let done = c.commit(&folder.0, "Fix the cart total\n\nLocust-Teammate: Wren", Then::Commit, None, None);
    let CommitResult::Done { branch, files, .. } = &done else { panic!("{done:?}") };
    assert_eq!((branch.as_str(), *files), ("main", 1));
    assert_eq!(sh(&["log", "-1", "--format=%an <%ae>|%s"], &folder.0).trim(), "You <you@example.test>|Fix the cart total");
    assert_eq!(sh(&["show", "--name-only", "--format=", "HEAD"], &folder.0).trim(), "cart.py");
    assert_eq!(c.changes(&folder.0), FolderChanges::None(Nothing::Clean));
    assert!(outcome_words(&done)[0].starts_with("Committed 1 file to main as "));
}

#[test]
fn a_hook_that_refuses_leaves_what_you_had_staged_exactly_as_it_was() {
    let (folder, _bare) = repository();
    fs::write(folder.0.join("cart.py"), "def total(items):\n    return sum(items)\n").expect("write");
    fs::write(folder.0.join("staged.md"), "staged\n").expect("write");
    fs::write(folder.0.join("loose.md"), "loose\n").expect("write");
    sh(&["add", "staged.md"], &folder.0);
    let hook = folder.0.join(".git/hooks/pre-commit");
    fs::write(&hook, "#!/bin/sh\necho \"lint failed\" >&2\nexit 1\n").expect("hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let before = sh(&["status", "--porcelain"], &folder.0);
    let head = sh(&["rev-parse", "HEAD"], &folder.0);
    let (c, _) = commits();
    let refused = c.commit(&folder.0, "Try", Then::Commit, None, None);
    let CommitResult::Refused(message) = refused else { panic!("{refused:?}") };
    assert!(message.starts_with("Your commit hooks refused the commit: ") && message.contains("lint failed"), "{message}");
    assert_eq!(sh(&["status", "--porcelain"], &folder.0), before);
    assert_eq!(sh(&["rev-parse", "HEAD"], &folder.0), head);
}

#[test]
fn commit_and_push_sends_it_to_the_branch_it_tracks() {
    let (folder, bare) = repository();
    fs::write(folder.0.join("cart.py"), "def total(items):\n    return sum(items)\n").expect("write");
    let (c, calls) = commits();
    let done = c.commit(&folder.0, "Fix the cart total", Then::Push, None, None);
    assert!(matches!(done, CommitResult::Done { pushed: Some(true), .. }), "{done:?}");
    let push = calls.lock().expect("calls").iter().find(|call| call.get(1).map(String::as_str) == Some("push")).cloned();
    // A full refspec: a branch name can never be read as a flag.
    assert_eq!(push, Some(strs(&["git", "push", "origin", "refs/heads/main:refs/heads/main"])));
    assert_eq!(sh(&["log", "-1", "--format=%s", "main"], &bare.0).trim(), "Fix the cart total");
}

#[test]
fn a_pull_request_from_main_goes_from_a_branch_of_its_own_and_main_is_left_where_it_was() {
    let (folder, bare) = repository();
    let main = sh(&["rev-parse", "main"], &folder.0);
    fs::write(folder.0.join("cart.py"), "def total(items):\n    return sum(items)\n").expect("write");
    let (c, calls) = commits();
    let done = c.commit(&folder.0, "Fix the cart total\n\n- Fix the cart total", Then::PullRequest, None, None);
    let CommitResult::Done { branch, new_branch, pushed, pull_request_url, .. } = &done else { panic!("{done:?}") };
    assert_eq!(branch, "locust/fix-the-cart-total");
    assert_eq!(new_branch.as_deref(), Some("locust/fix-the-cart-total"));
    assert_eq!(*pushed, Some(true));
    assert_eq!(pull_request_url.as_deref(), Some("https://github.com/someone/pebble/pull/7"));
    assert_eq!(sh(&["rev-parse", "main"], &folder.0), main);
    assert_eq!(sh(&["symbolic-ref", "--short", "HEAD"], &folder.0).trim(), "locust/fix-the-cart-total");
    assert_eq!(sh(&["log", "-1", "--format=%s", "locust/fix-the-cart-total"], &bare.0).trim(), "Fix the cart total");
    let pr = calls.lock().expect("calls").iter().find(|call| call[0] == "gh" && call.get(1).map(String::as_str) == Some("pr")).cloned();
    assert_eq!(
        pr,
        Some(strs(&["gh", "pr", "create", "--title=Fix the cart total", "--body=- Fix the cart total", "--head=locust/fix-the-cart-total", "--base=main"]))
    );
}

#[test]
fn a_pull_request_for_an_issue_says_which_issue_it_fixes() {
    let (folder, _bare) = repository();
    fs::write(folder.0.join("cart.py"), "def total(items):\n    return sum(items)\n").expect("write");
    let (c, calls) = commits();
    let done = c.commit(&folder.0, "Fix the cart total\n\n- Fix the cart total", Then::PullRequest, None, Some(123));
    assert!(matches!(done, CommitResult::Done { .. }), "{done:?}");
    let pr = calls.lock().expect("calls").iter().find(|call| call[0] == "gh" && call.get(1).map(String::as_str) == Some("pr")).cloned();
    assert_eq!(
        pr,
        Some(strs(&["gh", "pr", "create", "--title=Fix the cart total", "--body=- Fix the cart total\n\nFixes #123", "--head=locust/fix-the-cart-total", "--base=main"]))
    );
}

#[test]
fn a_refused_push_keeps_the_commit_and_says_so() {
    let (folder, _bare) = repository();
    let nowhere = folder.0.join("no-such-remote");
    sh(&["remote", "set-url", "--push", "origin", &nowhere.to_string_lossy()], &folder.0);
    fs::write(folder.0.join("cart.py"), "def total(items):\n    return sum(items)\n").expect("write");
    let (c, _) = commits();
    let done = c.commit(&folder.0, "Fix", Then::Push, None, None);
    let CommitResult::Done { pushed, after, .. } = &done else { panic!("{done:?}") };
    assert_eq!(*pushed, Some(false));
    assert!(after.as_deref().is_some_and(|a| a.starts_with("The push was refused")), "{after:?}");
}

#[test]
fn no_pull_request_without_github_and_nothing_at_all_mid_merge() {
    let (folder, _bare) = repository();
    sh(&["remote", "set-url", "origin", "https://gitlab.com/someone/pebble.git"], &folder.0);
    fs::write(folder.0.join("cart.py"), "x\n").expect("write");
    let (c, _) = commits();
    let seen = c.changes(&folder.0);
    assert!(matches!(&seen, FolderChanges::Changes { remote: Some(r), .. } if !r.pull_requests), "{seen:?}");
    assert!(matches!(c.commit(&folder.0, "x", Then::PullRequest, None, None), CommitResult::Refused(_)));
    let head = sh(&["rev-parse", "HEAD"], &folder.0);
    fs::write(folder.0.join(".git/MERGE_HEAD"), head).expect("merge head");
    assert!(matches!(c.changes(&folder.0), FolderChanges::Blocked { why: Blocked::Merging, .. }));
}

#[test]
fn a_folder_inside_a_repository_not_its_top_offers_nothing() {
    let (folder, _bare) = repository();
    fs::create_dir_all(folder.0.join("sub")).expect("dir");
    fs::write(folder.0.join("sub/a.txt"), "a").expect("write");
    let (c, _) = commits();
    assert_eq!(c.changes(&folder.0.join("sub")), FolderChanges::None(Nothing::NotARepository));
}

#[test]
fn a_file_that_appeared_since_the_list_was_shown_is_not_committed_unseen() {
    let (folder, _bare) = repository();
    fs::write(folder.0.join("cart.py"), "def total(items):\n    return sum(items)\n").expect("write");
    let (c, _) = commits();
    let shown = vec!["cart.py".to_owned()];
    fs::write(folder.0.join("surprise.txt"), "new since").expect("write");
    let refused = c.commit(&folder.0, "Fix", Then::Commit, Some(&shown), None);
    assert!(matches!(&refused, CommitResult::Refused(m) if m.starts_with("The folder changed since this was opened")), "{refused:?}");
    assert_eq!(sh(&["rev-list", "--count", "HEAD"], &folder.0).trim(), "1");
}

#[test]
fn drafts_from_the_asks_and_names_a_branch_from_the_subject() {
    let asks = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    assert_eq!(
        commit_draft(&asks(&["Fix the cart total", "and add a test"]), Some("Wren")),
        "Fix the cart total\n\n- Fix the cart total\n- and add a test\n\nLocust-Teammate: Wren"
    );
    assert_eq!(commit_draft(&[], None), "Changes");
    // A long ask's first sentence, when it fits; README.md's dot is not a sentence's end.
    assert_eq!(
        commit_draft(&asks(&["Add one line at the end of README.md: 'Hello from Wren.' Change nothing else."]), None),
        "Add one line at the end of README.md: 'Hello from Wren.'"
    );
    assert_eq!(branch_from_subject("Fix the cart total!"), "locust/fix-the-cart-total");
    assert_eq!(branch_from_subject("???"), "locust/changes");
}

#[test]
fn branch_names_and_github_urls_are_checked_as_electron_checks_them() {
    assert!(safe_branch("main") && safe_branch("locust/fix-1") && safe_branch("_x"));
    assert!(!safe_branch("-x") && !safe_branch("") && !safe_branch("a b") && !safe_branch("a;b"));
    assert_eq!(github_repo_of("https://github.com/someone/pebble.git").as_deref(), Some("someone/pebble"));
    assert_eq!(github_repo_of("git@github.com:someone/pebble.git").as_deref(), Some("someone/pebble"));
    assert_eq!(github_repo_of("https://token@github.com/someone/pebble").as_deref(), Some("someone/pebble"));
    // The host is GitHub's own, not a path that mentions it.
    assert_eq!(github_repo_of("https://evil.example/github.com/owner/repo.git"), None);
    assert_eq!(github_repo_of("https://gitlab.com/someone/pebble.git"), None);
    assert_eq!(with_fixes_line("", Some(3)), "Fixes #3");
    assert_eq!(with_fixes_line("body\n\nFixes #3", Some(3)), "body\n\nFixes #3");
    assert_eq!(with_fixes_line("body", None), "body");
}

#[test]
fn every_blocked_reason_has_electrons_sentence() {
    for why in [Blocked::Merging, Blocked::Rebasing, Blocked::Detached, Blocked::Conflicted, Blocked::BranchName] {
        assert!(blocked_sentence(why).ends_with('.'));
    }
    assert_eq!(blocked_sentence(Blocked::Conflicted), "Some files still have unresolved conflicts. Resolve them first.");
}

#[test]
fn a_failed_switch_hook_undoes_the_pull_request_branch() {
    let (folder, _bare) = repository();
    let main = sh(&["rev-parse", "main"], &folder.0);
    fs::write(folder.0.join("cart.py"), "def total(items):\n    return sum(items)\n").expect("write");
    let hook = folder.0.join(".git/hooks/post-checkout");
    fs::write(&hook, "#!/bin/sh\necho \"checkout refused\" >&2\nexit 1\n").expect("hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).expect("chmod");
    }
    let (c, _) = commits();
    let refused = c.commit(&folder.0, "Fix the cart total", Then::PullRequest, None, None);
    let CommitResult::Refused(message) = refused else { panic!("{refused:?}") };
    assert!(
        message.starts_with("git could not make a branch for the pull request:") && message.contains("checkout refused"),
        "{message}"
    );
    assert_eq!(sh(&["symbolic-ref", "--short", "HEAD"], &folder.0).trim(), "main");
    assert_eq!(sh(&["rev-parse", "main"], &folder.0), main);
    assert!(!folder.0.join(".git/refs/heads/locust/fix-the-cart-total").exists());
    assert_eq!(sh(&["rev-list", "--count", "HEAD"], &folder.0).trim(), "1");
}

#[test]
fn already_staged_dot_locust_is_taken_off_and_never_committed() {
    let (folder, _bare) = repository();
    fs::write(folder.0.join("cart.py"), "def total(items):\n    return sum(items)\n").expect("write");
    fs::create_dir_all(folder.0.join(".locust")).expect("dir");
    fs::write(folder.0.join(".locust/keep.txt"), "secret-to-teammates\n").expect("write");
    // Force-add so it is staged despite any ignore; Locust must still reset it.
    sh(&["add", "-f", ".locust/keep.txt"], &folder.0);
    assert!(sh(&["diff", "--cached", "--name-only"], &folder.0).contains(".locust/keep.txt"));
    let (c, _) = commits();
    let done = c.commit(&folder.0, "Fix the cart total", Then::Commit, None, None);
    let CommitResult::Done { files, .. } = &done else { panic!("{done:?}") };
    assert_eq!(*files, 1);
    assert_eq!(sh(&["show", "--name-only", "--format=", "HEAD"], &folder.0).trim(), "cart.py");
    assert!(!sh(&["ls-tree", "-r", "--name-only", "HEAD"], &folder.0).contains(".locust"));
}

#[test]
fn outcome_words_match_electron_commit_outcome_lines() {
    let done = CommitResult::Done {
        sha: "4f2c9a1b7e0d3c5a8f6e2d1c0b9a8f7e6d5c4b3a".into(),
        branch: "locust/fix-the-cart-total".into(),
        files: 4,
        left_out: vec!["big.bin".into()],
        new_branch: Some("locust/fix-the-cart-total".into()),
        pushed: Some(true),
        pull_request_url: Some("https://github.com/someone/pebble/pull/7".into()),
        after: Some("The push was refused: denied".into()),
    };
    assert_eq!(
        outcome_words(&done),
        vec![
            "Committed 4 files to locust/fix-the-cart-total as 4f2c9a1. Your folder is on locust/fix-the-cart-total now. Pushed to origin.".to_owned(),
            "Pull request opened: someone/pebble/pull/7".to_owned(),
            "Left out, too big to commit: big.bin.".to_owned(),
            "The push was refused: denied".to_owned(),
        ]
    );
    assert_eq!(
        outcome_words(&CommitResult::Refused("A commit needs a message.".into())),
        vec!["A commit needs a message.".to_owned()]
    );
}

#[test]
fn pull_request_url_needs_https_and_a_pull_number() {
    assert_eq!(
        pull_request_url_of("https://github.com/someone/pebble/pull/7\n"),
        Some("https://github.com/someone/pebble/pull/7".into())
    );
    assert_eq!(pull_request_url_of("https://evil.example/pull/\n"), None);
    // Electron's /\d+/ stops at the digits; trailing junk in the token is not kept.
    assert_eq!(
        pull_request_url_of("see https://github.com/someone/pebble/pull/7x later"),
        Some("https://github.com/someone/pebble/pull/7".into())
    );
}

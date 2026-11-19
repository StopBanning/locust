//! Two of Electron's worktree files, against real git and real hooks in temporary repositories, with Electron's values:
//! - `main/a-half-made-worktree-is-finished-not-trusted.test.ts` (code review B4, main-stores 3);
//! - `main/a-worktree-survives-only-a-finished-checkout.test.ts`: a checkout hook that fails after git made the tree.
//! Electron's diagnostic `note` callback has no Rust counterpart; its injected `runGit` is `Manager::with_git_program`.
//! Nothing here touches a repository outside its temporary folder.

use crate::worktrees::{worktree_add_args, worktree_add_args_for_platform, Manager};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn temp(prefix: &str) -> Temp {
    let p = std::env::temp_dir().join(format!("{prefix}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(&p).unwrap();
    Temp(std::fs::canonicalize(&p).unwrap())
}

fn git(args: &[&str], cwd: &Path) -> String {
    let out = std::process::Command::new("git").args(args).current_dir(cwd).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

// ---- a tree whose making was cut short ----

/// The test file's `repository(files)`: NOTES.md and `files` files under src/, one commit on main.
fn repository_of(files: usize) -> Temp {
    let t = temp("locust-halfmade");
    git(&["init", "-q", "-b", "main"], &t.0);
    git(&["config", "user.email", "smoke@locust.test"], &t.0);
    git(&["config", "user.name", "Locust smoke"], &t.0);
    std::fs::create_dir_all(t.0.join("src")).unwrap();
    std::fs::write(t.0.join("NOTES.md"), "# notes\n").unwrap();
    for index in 0..files {
        std::fs::write(t.0.join("src").join(format!("file{index}.txt")), format!("line {index}\n")).unwrap();
    }
    git(&["add", "-A"], &t.0);
    git(&["commit", "-q", "-m", "first"], &t.0);
    t
}

#[test]
fn a_tree_whose_making_was_cut_short_is_finished_keeping_what_is_in_it_rather_than_handed_out_half_made() {
    let root = repository_of(30);
    let manager = Manager::new(&root.0);
    let path = manager.ensure("tm_wren", "Wren", None).unwrap();
    let admin = root.0.join(".git").join("worktrees").join("tm_wren");

    // The state a killed add leaves, as measured.
    std::fs::remove_file(admin.join("index")).unwrap();
    std::fs::write(admin.join("index.lock"), "").unwrap();
    std::fs::write(admin.join("locked"), "initializing\n").unwrap();
    for index in 5..30 {
        std::fs::remove_file(path.join("src").join(format!("file{index}.txt"))).unwrap();
    }
    // And work a teammate did there, under an older build.
    std::fs::write(path.join("src").join("file0.txt"), "changed by Wren\n").unwrap();
    std::fs::write(path.join("WREN.md"), "a new file\n").unwrap();

    assert_eq!(manager.ensure("tm_wren", "Wren", None).unwrap(), path);
    for index in 0..30 {
        assert!(path.join("src").join(format!("file{index}.txt")).is_file(), "file{index}.txt");
    }
    assert_eq!(std::fs::read_to_string(path.join("src").join("file0.txt")).unwrap(), "changed by Wren\n");
    let mut status: Vec<String> = git(&["status", "--porcelain"], &path).lines().filter(|l| !l.is_empty()).map(str::to_owned).collect();
    status.sort();
    assert_eq!(status, [" M src/file0.txt", "?? WREN.md"]);
    assert!(!admin.join("locked").exists());
    assert!(!admin.join("index.lock").exists());
}

// "leaves a finished tree exactly as it is": no git at all for a tree that is already whole. Electron counts its
// injected runGit's calls; here the manager runs a stand-in that writes down each call and then runs git.
#[test]
fn a_finished_tree_is_left_exactly_as_it_is_with_no_git_at_all() {
    use std::os::unix::fs::PermissionsExt;
    let root = repository_of(3);
    let tools = temp("locust-halfmade-git");
    let calls = tools.0.join("calls.txt");
    let script = tools.0.join("counting-git");
    std::fs::write(&script, format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec git \"$@\"\n", calls.display())).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let manager = Manager::new(&root.0).with_git_program(script.to_string_lossy().into_owned());
    let path = manager.ensure("tm_wren", "Wren", None).unwrap();
    assert!(std::fs::read_to_string(&calls).unwrap_or_default().contains("worktree add"), "the stand-in runs the add");
    std::fs::remove_file(&calls).unwrap();

    assert_eq!(manager.ensure("tm_wren", "Wren", None).unwrap(), path);
    assert_eq!(std::fs::read_to_string(&calls).unwrap_or_default(), "");
}

#[test]
fn a_dot_git_file_that_points_outside_the_repository_is_left_alone() {
    let root = repository_of(1);
    let elsewhere = temp("locust-halfmade-elsewhere");
    std::fs::write(elsewhere.0.join("locked"), "initializing\n").unwrap();
    std::fs::write(elsewhere.0.join("index.lock"), "not ours\n").unwrap();
    let path = root.0.join(".locust").join("worktrees").join("tm_wren");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join(".git"), format!("gitdir: {}\n", elsewhere.0.display())).unwrap();

    let _ = Manager::new(&root.0).ensure("tm_wren", "Wren", None);

    // Whatever that is, it is not a tree this manager made: its lock stays.
    assert_eq!(std::fs::read_to_string(elsewhere.0.join("index.lock")).unwrap(), "not ours\n");
}

// ---- only a finished worktree survives a checkout-hook failure ----

/// The test file's `repository(hook)`: file.txt committed on main, and `hook` as the repository's post-checkout.
fn repository_with_hook(hook: &str) -> Temp {
    use std::os::unix::fs::PermissionsExt;
    let t = temp("locust-checkout-hook");
    git(&["init", "-q", "-b", "main"], &t.0);
    git(&["config", "user.name", "Locust test"], &t.0);
    git(&["config", "user.email", "test@locust.test"], &t.0);
    git(&["config", "core.autocrlf", "false"], &t.0);
    std::fs::write(t.0.join("file.txt"), "checked out\n").unwrap();
    git(&["add", "file.txt"], &t.0);
    git(&["-c", "commit.gpgsign=false", "commit", "-q", "-m", "fixture"], &t.0);
    let hooks = t.0.join(".git").join("hooks");
    std::fs::create_dir_all(&hooks).unwrap();
    std::fs::write(hooks.join("post-checkout"), format!("#!/bin/sh\n{hook}\n")).unwrap();
    std::fs::set_permissions(hooks.join("post-checkout"), std::fs::Permissions::from_mode(0o755)).unwrap();
    t
}

// case: accepts and uses the real worktree, keeping full hook stderr in the diagnostic callback
#[test]
fn a_hook_that_fails_after_a_finished_checkout_leaves_a_tree_that_is_used() {
    let stderr = format!("hook diagnostic: {}", "x".repeat(400));
    let root = repository_with_hook(&format!("printf '%s\\n' '{stderr}' >&2\nexit 1"));
    let notes: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let notes_cb = notes.clone();
    let path = Manager::new(&root.0)
        .with_note(move |msg| notes_cb.lock().unwrap().push(msg.to_string()))
        .ensure("tm_hook", "Hook", None)
        .expect("the finished tree is accepted");
    assert_eq!(std::fs::read_to_string(path.join("file.txt")).unwrap(), "checked out\n");
    assert_eq!(git(&["symbolic-ref", "--short", "HEAD"], &path).trim(), "locust/hook");
    let head = git(&["rev-parse", "--verify", "HEAD"], &path);
    assert!(head.trim().len() == 40 && head.trim().bytes().all(|b| b.is_ascii_hexdigit()), "{head}");
    let recorded = notes.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    assert!(recorded[0].contains(&stderr), "full hook stderr must be kept, got len {}", recorded[0].len());
    std::fs::write(path.join("file.txt"), "usable after the hook\n").unwrap();
    assert!(git(&["status", "--porcelain"], &path).contains(" M file.txt"));
}

// case: refuses a hook failure that removes the worktree
#[test]
fn a_hook_failure_that_removes_the_worktree_is_refused() {
    let root = repository_with_hook(
        "tree=\"$PWD\"\ncommon=$(git rev-parse --git-common-dir)\ncd \"$common/..\" || exit 2\nunset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE\ngit worktree remove --force \"$tree\" || exit 2\necho removed-worktree >&2\nexit 1",
    );
    let notes: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let notes_cb = notes.clone();
    let refused = Manager::new(&root.0)
        .with_note(move |msg| notes_cb.lock().unwrap().push(msg.to_string()))
        .ensure("tm_hook", "Hook", None)
        .unwrap_err();
    assert!(refused.contains("removed-worktree"), "{refused}");
    assert!(!root.0.join(".locust").join("worktrees").join("tm_hook").exists());
    assert!(!git(&["worktree", "list", "--porcelain"], &root.0).contains("refs/heads/locust/hook"));
    assert!(notes.lock().unwrap().is_empty());
}

// case: refuses a hook that leaves HEAD on the wrong branch
#[test]
fn a_hook_that_leaves_head_on_the_wrong_branch_is_refused() {
    let root = repository_with_hook("git symbolic-ref HEAD refs/heads/main\necho wrong-branch >&2\nexit 1");
    let notes: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let notes_cb = notes.clone();
    let refused = Manager::new(&root.0)
        .with_note(move |msg| notes_cb.lock().unwrap().push(msg.to_string()))
        .ensure("tm_hook", "Hook", None)
        .unwrap_err();
    assert!(refused.contains("wrong-branch"), "{refused}");
    assert!(notes.lock().unwrap().is_empty());
}

// case: requires HEAD verification to succeed before accepting a nonzero add
#[test]
fn requires_head_verification_to_succeed_before_accepting_a_nonzero_add() {
    use std::os::unix::fs::PermissionsExt;
    let root = repository_with_hook("echo failed-hook >&2\nexit 1");
    let tools = temp("locust-head-fail-git");
    let calls = tools.0.join("calls.txt");
    let script = tools.0.join("head-fail-git");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s|%s\\n' \"$PWD\" \"$*\" >> '{}'\ncase \"$*\" in\n  *\"rev-parse --verify HEAD\"*)\n    echo 'unreadable HEAD' >&2\n    exit 1\n    ;;\nesac\nexec git \"$@\"\n",
            calls.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let notes: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let notes_cb = notes.clone();
    let refused = Manager::new(&root.0)
        .with_git_program(script.to_string_lossy().into_owned())
        .with_note(move |msg| notes_cb.lock().unwrap().push(msg.to_string()))
        .ensure("tm_hook", "Hook", None)
        .unwrap_err();
    assert!(refused.contains("failed-hook"), "{refused}");
    let log = std::fs::read_to_string(&calls).unwrap();
    let expected_tree = root.0.join(".locust").join("worktrees").join("tm_hook");
    assert!(
        log.lines().any(|l| l.contains("rev-parse --verify HEAD") && l.starts_with(&expected_tree.to_string_lossy().into_owned())),
        "expected rev-parse --verify HEAD in {}: {log}",
        expected_tree.display()
    );
    assert!(notes.lock().unwrap().is_empty());
}

// case: does not treat a timeout as a finished hook failure
#[test]
fn does_not_treat_a_timeout_as_a_finished_hook_failure() {
    use std::os::unix::fs::PermissionsExt;
    let root = repository_with_hook("exit 0");
    let tools = temp("locust-timeout-git");
    let script = tools.0.join("slow-add-git");
    std::fs::write(
        &script,
        "#!/bin/sh\ncase \"$*\" in\n  *\"worktree add\"*)\n    git \"$@\"\n    sleep 5\n    exit 0\n    ;;\nesac\nexec git \"$@\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let notes: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let notes_cb = notes.clone();
    let refused = Manager::new(&root.0)
        .with_git_program(script.to_string_lossy().into_owned())
        .with_checkout_timeout(Duration::from_millis(600))
        .with_note(move |msg| notes_cb.lock().unwrap().push(msg.to_string()))
        .ensure("tm_hook", "Hook", None)
        .unwrap_err();
    assert!(refused.contains("took longer than"), "{refused}");
    assert!(notes.lock().unwrap().is_empty());
}

// case: keeps an initializing checkout refused even if its recorded branch and HEAD exist
#[test]
fn an_initializing_checkout_stays_refused_even_when_its_recorded_branch_and_head_exist() {
    let root = repository_with_hook("admin=$(git rev-parse --git-dir)\nprintf \"initializing\\n\" > \"$admin/locked\"\necho unfinished-hook >&2\nexit 1");
    let notes: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let notes_cb = notes.clone();
    let refused = Manager::new(&root.0)
        .with_note(move |msg| notes_cb.lock().unwrap().push(msg.to_string()))
        .ensure("tm_hook", "Hook", None)
        .unwrap_err();
    assert!(refused.contains("unfinished-hook"), "{refused}");
    assert!(notes.lock().unwrap().is_empty());
}

// case: does not fail an otherwise usable tree if the diagnostics callback fails
#[test]
fn does_not_fail_an_otherwise_usable_tree_if_the_diagnostics_callback_fails() {
    let root = repository_with_hook("echo hook-failed >&2\nexit 1");
    let called = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let called_cb = called.clone();
    let path = Manager::new(&root.0)
        .with_note(move |_| {
            called_cb.fetch_add(1, Ordering::Relaxed);
            panic!("log unavailable");
        })
        .ensure("tm_hook", "Hook", None)
        .expect("a failing diagnostic callback must not fail a usable worktree");
    assert_eq!(std::fs::read_to_string(path.join("file.txt")).unwrap(), "checked out\n");
    assert_eq!(called.load(Ordering::Relaxed), 1);
}

// case: uses longpaths only on add commands without changing core.longpaths config
#[test]
fn a_worktree_add_leaves_core_longpaths_config_as_it_was() {
    use std::os::unix::fs::PermissionsExt;
    let root = repository_with_hook("exit 0");
    git(&["config", "core.longpaths", "false"], &root.0);
    let tools = temp("locust-longpaths-git");
    let calls = tools.0.join("calls.txt");
    let script = tools.0.join("recording-git");
    std::fs::write(&script, format!("#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nexec git \"$@\"\n", calls.display())).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let manager = Manager::new(&root.0).with_git_program(script.to_string_lossy().into_owned());
    let path = manager.ensure("tm_hook", "Hook", None).unwrap();
    assert_eq!(git(&["config", "--local", "--get", "core.longpaths"], &root.0).trim(), "false");
    manager.remove("tm_hook", None).unwrap();
    assert_eq!(manager.ensure("tm_hook", "Hook", None).unwrap(), path);
    let log = std::fs::read_to_string(&calls).unwrap();
    let adds: Vec<&str> = log.lines().filter(|l| l.contains("worktree add")).collect();
    assert_eq!(adds.len(), 2, "{log}");
    let p = path.to_string_lossy();
    assert!(adds[0].ends_with(&worktree_add_args(&p, "locust/hook", false, "HEAD").unwrap().join(" ")));
    assert!(adds[1].ends_with(&worktree_add_args(&p, "locust/hook", true, "HEAD").unwrap().join(" ")));
    let with_lp: Vec<&str> = log.lines().filter(|l| l.contains("core.longpaths=true")).collect();
    if cfg!(windows) {
        assert_eq!(with_lp, adds);
    } else {
        assert!(with_lp.is_empty(), "{with_lp:?}");
    }
    assert_eq!(git(&["config", "--local", "--get", "core.longpaths"], &root.0).trim(), "false");
}

// case: adds the longpaths flag only for Windows (%s), on both branch forms
#[test]
fn adds_the_longpaths_flag_only_for_windows_on_both_branch_forms() {
    for (platform, is_win) in [("win32", true), ("darwin", false), ("linux", false)] {
        let mut new_branch: Vec<&str> = if is_win { vec!["-c", "core.longpaths=true"] } else { vec![] };
        new_branch.extend(["worktree", "add", "-b", "locust/test", "/tree", "HEAD"]);
        assert_eq!(
            worktree_add_args_for_platform("/tree", "locust/test", false, "HEAD", is_win).unwrap(),
            new_branch,
            "new branch on {platform}"
        );
        let mut existing_branch: Vec<&str> = if is_win { vec!["-c", "core.longpaths=true"] } else { vec![] };
        existing_branch.extend(["worktree", "add", "/tree", "locust/test"]);
        assert_eq!(
            worktree_add_args_for_platform("/tree", "locust/test", true, "HEAD", is_win).unwrap(),
            existing_branch,
            "existing branch on {platform}"
        );
    }
}

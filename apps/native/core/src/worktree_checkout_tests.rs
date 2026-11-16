//! Two of Electron's worktree files, against real git and real hooks in temporary repositories, with Electron's values:
//! - `main/a-half-made-worktree-is-finished-not-trusted.test.ts` (code review B4, main-stores 3);
//! - `main/a-worktree-survives-only-a-finished-checkout.test.ts`: a checkout hook that fails after git made the tree.
//! Electron's diagnostic `note` callback and its injected `runGit` have no Rust counterpart (see the ledger rows).
//! Nothing here touches a repository outside its temporary folder.

use crate::worktrees::{worktree_add_args, Manager};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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

#[test]
fn a_hook_that_fails_after_a_finished_checkout_leaves_a_tree_that_is_used() {
    let stderr = format!("hook diagnostic: {}", "x".repeat(400));
    let root = repository_with_hook(&format!("printf '%s\\n' '{stderr}' >&2\nexit 1"));
    let path = Manager::new(&root.0).ensure("tm_hook", "Hook", None).expect("the finished tree is accepted");
    assert_eq!(std::fs::read_to_string(path.join("file.txt")).unwrap(), "checked out\n");
    assert_eq!(git(&["symbolic-ref", "--short", "HEAD"], &path).trim(), "locust/hook");
    let head = git(&["rev-parse", "--verify", "HEAD"], &path);
    assert!(head.trim().len() == 40 && head.trim().bytes().all(|b| b.is_ascii_hexdigit()), "{head}");
    std::fs::write(path.join("file.txt"), "usable after the hook\n").unwrap();
    assert!(git(&["status", "--porcelain"], &path).contains(" M file.txt"));
}

#[test]
fn a_hook_failure_that_removes_the_worktree_is_refused() {
    let root = repository_with_hook(
        "tree=\"$PWD\"\ncommon=$(git rev-parse --git-common-dir)\ncd \"$common/..\" || exit 2\nunset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE\ngit worktree remove --force \"$tree\" || exit 2\necho removed-worktree >&2\nexit 1",
    );
    let refused = Manager::new(&root.0).ensure("tm_hook", "Hook", None).unwrap_err();
    assert!(refused.contains("removed-worktree"), "{refused}");
    assert!(!root.0.join(".locust").join("worktrees").join("tm_hook").exists());
    assert!(!git(&["worktree", "list", "--porcelain"], &root.0).contains("refs/heads/locust/hook"));
}

#[test]
fn a_hook_that_leaves_head_on_the_wrong_branch_is_refused() {
    let root = repository_with_hook("git symbolic-ref HEAD refs/heads/main\necho wrong-branch >&2\nexit 1");
    let refused = Manager::new(&root.0).ensure("tm_hook", "Hook", None).unwrap_err();
    assert!(refused.contains("wrong-branch"), "{refused}");
}

#[test]
fn an_initializing_checkout_stays_refused_even_when_its_recorded_branch_and_head_exist() {
    let root = repository_with_hook("admin=$(git rev-parse --git-dir)\nprintf \"initializing\\n\" > \"$admin/locked\"\necho unfinished-hook >&2\nexit 1");
    let refused = Manager::new(&root.0).ensure("tm_hook", "Hook", None).unwrap_err();
    assert!(refused.contains("unfinished-hook"), "{refused}");
}

#[test]
fn a_worktree_add_leaves_core_longpaths_config_as_it_was() {
    let root = repository_with_hook("exit 0");
    git(&["config", "core.longpaths", "false"], &root.0);
    let manager = Manager::new(&root.0);
    let path = manager.ensure("tm_hook", "Hook", None).unwrap();
    assert_eq!(git(&["config", "--local", "--get", "core.longpaths"], &root.0).trim(), "false");
    manager.remove("tm_hook", None).unwrap();
    assert_eq!(manager.ensure("tm_hook", "Hook", None).unwrap(), path);
    assert_eq!(git(&["config", "--local", "--get", "core.longpaths"], &root.0).trim(), "false");
}

#[test]
fn off_windows_a_worktree_add_carries_no_longpaths_flag_on_both_branch_forms() {
    assert!(!cfg!(windows), "this case is the macOS and Linux one");
    assert_eq!(worktree_add_args("/tree", "locust/test", false, "HEAD").unwrap(), ["worktree", "add", "-b", "locust/test", "/tree", "HEAD"]);
    assert_eq!(worktree_add_args("/tree", "locust/test", true, "HEAD").unwrap(), ["worktree", "add", "/tree", "locust/test"]);
}

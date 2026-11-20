//! Electron's `a-folder-cannot-make-locust-run-its-program.test.ts` (2 cases): a repository's own config can name a
//! program for git to run, and `core.fsmonitor` runs on every `git status`. Locust's own looks never run it.
use std::path::{Path, PathBuf};

static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

struct Temp(PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn git(args: &[&str], cwd: &Path) {
    let out = std::process::Command::new("git").args(args).current_dir(cwd).output().unwrap();
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// A real repository whose config names an fsmonitor that leaves a mark, as Electron's `hostile()` makes it.
fn hostile() -> (Temp, PathBuf) {
    let p = std::env::temp_dir().join(format!("locust-hostile-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    std::fs::create_dir_all(&p).unwrap();
    let root = Temp(std::fs::canonicalize(&p).unwrap());
    let marker = root.0.join("fsmonitor-ran.txt");
    git(&["init", "-q", "-b", "main"], &root.0);
    git(&["config", "user.email", "smoke@locust.test"], &root.0);
    git(&["config", "user.name", "Locust smoke"], &root.0);
    std::fs::write(root.0.join("NOTES.md"), "# notes\n").unwrap();
    git(&["add", "NOTES.md"], &root.0);
    git(&["commit", "-q", "-m", "first"], &root.0);
    git(&["config", "core.fsmonitor", &format!("echo ran >> \"{}\"; exit 1", marker.display())], &root.0);
    (root, marker)
}

#[test]
fn is_looked_at_by_locust_without_running_it() {
    let (root, marker) = hostile();
    assert!(crate::disk_observation::snapshot_workspace(&root.0, &crate::disk_observation::DefaultDiskHost::new()).is_some());
    assert!(!marker.exists(), "Locust's look ran the folder's program");
    // The control: the fixture is real, and plain `git status` runs it.
    git(&["status"], &root.0);
    assert!(marker.exists(), "the fixture's program should run under plain git status");
}

#[test]
fn has_a_teammates_worktree_checked_without_running_it() {
    let (root, marker) = hostile();
    let manager = crate::worktrees::Manager::new(&root.0);
    let path = manager.ensure("tm_wren", "Wren", None).expect("a worktree");
    let _ = std::fs::remove_file(&marker);
    // Removing looks at the worktree's changes first, with `git status`.
    manager.remove("tm_wren", None).expect("removed");
    assert!(!marker.exists(), "Locust's look at the worktree ran the folder's program");
    assert!(!path.exists(), "the worktree is gone");
}

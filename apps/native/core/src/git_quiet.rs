//! git as Locust runs it never takes git's *optional* locks.
//!
//! `git status` and the other readers refresh the index's cached file times when they can, and to do that they take
//! `.git/index.lock` for a moment. Locust reads the person's folder often (the changes and commit panels, a teammate's
//! branch, Scout's look), so one of those reads could hold the lock at the instant something else writes: a landing
//! commit, the person's own `git commit` in a terminal, an editor. That write then fails with "Unable to create
//! index.lock: File exists" (seen in `review_changes_shows_a_teammates_branch_turn_by_turn_and_lands_it_as_one_commit`).
//! `GIT_OPTIONAL_LOCKS=0` makes the readers skip that refresh, as editors that watch a repository do. Locks a write
//! needs are not optional and are still taken.
//!
//! Electron's git calls do not set it; nothing a person sees changes, only that Locust's reads no longer get in the
//! way of a write.

/// The environment every git Locust starts is given.
pub const NO_OPTIONAL_LOCKS: (&str, &str) = ("GIT_OPTIONAL_LOCKS", "0");

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    /// A repository whose one file was written again with the same words after it was committed: its cached file
    /// times are stale, so a `git status` that may take the optional lock rewrites `.git/index`.
    fn stale_repository(name: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "git-quiet-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let git = |args: &[&str]| {
            let out = Command::new("git").args(args).current_dir(&root).output().unwrap();
            assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        };
        git(&["init", "-q", "-b", "main"]);
        std::fs::write(root.join("a.txt"), "same\n").unwrap();
        git(&["add", "a.txt"]);
        git(&["-c", "user.name=T", "-c", "user.email=t@example.invalid", "commit", "-q", "-m", "one"]);
        // A later file time, the same words: only the cached times are out of date.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(root.join("a.txt"), "same\n").unwrap();
        root
    }

    fn index(root: &Path) -> Vec<u8> {
        std::fs::read(root.join(".git/index")).unwrap()
    }

    /// The control: plain git, which may take the lock, does rewrite this index. Without it the tests below prove nothing.
    #[test]
    fn a_plain_git_status_rewrites_a_stale_index() {
        let root = stale_repository("control");
        let before = index(&root);
        let out = Command::new("git").args(["status", "--porcelain"]).env_remove("GIT_OPTIONAL_LOCKS").current_dir(&root).output().unwrap();
        assert!(out.status.success());
        assert_ne!(index(&root), before, "plain git status refreshed the index, taking index.lock to do it");
    }

    #[test]
    fn the_changes_panels_git_leaves_the_index_alone() {
        let root = stale_repository("changes");
        let before = index(&root);
        crate::changes::git(&root, &["status", "--porcelain"]).unwrap();
        assert_eq!(index(&root), before);
    }

    #[test]
    fn a_teammates_branch_read_leaves_the_index_alone() {
        let root = stale_repository("worktrees");
        let before = index(&root);
        crate::worktrees::run_git(&["status", "--porcelain", "-z", "--untracked-files=all"], &root, std::time::Duration::from_secs(60)).unwrap();
        assert_eq!(index(&root), before);
    }
}

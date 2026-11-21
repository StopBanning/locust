//! The files a message attached, placed where THIS run can read them (Electron's `main/attachments-for-run.ts`, M16).
//!
//! The message names its files relative to the project folder, and copies of outside files land in
//! `<project>/.locust/attachments`. A teammate with Own branch runs in `<project>/.locust/worktrees/<id>`, and one with
//! its own folder runs somewhere else entirely, so the paths pointed at nothing -- and Claude Code and OpenCode refuse
//! reads outside the folder they run in, so the file never reached the model.
//!
//! So, when the run's folder is not the project folder:
//!
//!   - a copy under `.locust/attachments` is copied to the same path in the run's folder, and the line is unchanged;
//!   - a project file the run's folder also has is left alone: in a worktree that is the teammate's own branch's copy
//!     of it, the one it works on;
//!   - a project file the run's folder does not have is copied into that folder's `.locust/attachments`, and the line
//!     names the copy.
//!
//! Only what the runtime is SENT changes; the recorded prompt keeps the project's paths, which is what the thread's
//! tiles open. A file that cannot be placed is left as it was named: the runtime's own read then says what is missing,
//! which is more than a silent drop would. (Electron also opens an attached PDF for the agent here, 0.713; that part is
//! not carried yet.)

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use crate::attachments::{ATTACHMENT_DIR, attachment_destination, exclude_with, split_attachments, with_attachments};

/// The prompt to send a run in `run_cwd`, its files placed there from `workspace` (above). A message without
/// attachments, or a run in the project folder itself, comes back exactly as given.
pub fn attachments_for_run(prompt: &str, workspace: &Path, run_cwd: &Path) -> String {
    let (text, attachments) = split_attachments(prompt);
    if attachments.is_empty() || same_path(workspace, run_cwd) {
        return prompt.to_owned();
    }
    let placed = placed_for_run(&attachments, workspace, run_cwd);
    if placed == attachments {
        return prompt.to_owned();
    }
    with_attachments(&text, &placed)
}

/// The message's files, placed where a run outside the project folder reads them (above).
fn placed_for_run(attachments: &[String], workspace: &Path, run_cwd: &Path) -> Vec<String> {
    let mut taken: HashSet<String> = HashSet::new();
    let mut placed = Vec::with_capacity(attachments.len());
    let mut copied = false;
    for path in attachments {
        // An absolute path replaces the base in Rust's `join` (Node's appends it): either way it is no file inside the
        // project, so it is left as named.
        let source = workspace.join(path);
        if !inside(workspace, &source) {
            placed.push(path.clone());
            continue;
        }
        if path.starts_with(&format!("{ATTACHMENT_DIR}/")) {
            let destination = run_cwd.join(path);
            if copy_to(&source, &destination) {
                copied = true;
            }
            placed.push(path.clone());
            continue;
        }
        if run_cwd.join(path).is_file() {
            placed.push(path.clone());
            continue;
        }
        // A name that is no file name stays as the person named it, like a file that could not be copied.
        let Ok(relative) = attachment_destination(path, &taken) else {
            placed.push(path.clone());
            continue;
        };
        let name = relative[ATTACHMENT_DIR.len() + 1..].to_owned();
        taken.insert(name);
        if copy_to(&source, &run_cwd.join(&relative)) {
            copied = true;
            placed.push(relative);
        } else {
            placed.push(path.clone());
        }
    }
    if copied {
        keep_out_of_git(run_cwd);
    }
    placed
}

fn copy_to(source: &Path, destination: &Path) -> bool {
    destination.parent().is_some_and(|parent| std::fs::create_dir_all(parent).is_ok()) && std::fs::copy(source, destination).is_ok()
}

/// A path with `.` and `..` resolved by its words alone, as Node's `relative` sees it.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn same_path(a: &Path, b: &Path) -> bool {
    lexical(a) == lexical(b)
}

/// Strictly inside `root`: the root itself, and anything that climbs out of it, are not.
fn inside(root: &Path, path: &Path) -> bool {
    let (root, path) = (lexical(root), lexical(path));
    path != root && path.starts_with(&root)
}

/// `name`, or `name-2`, `name-3`, ... before its extension, whichever is not taken yet.
/// `.locust/` out of the run folder's commits, the way the project's copies are kept out: `.git/info/exclude`, never
/// the owner's `.gitignore`. A worktree's `.git` is a file and its exclude is the project's, which already has the line;
/// a folder that is not a repository has nothing to do. Not writable: the copy still reaches the run.
fn keep_out_of_git(folder: &Path) {
    if !folder.join(".git").is_dir() {
        return;
    }
    let exclude = folder.join(".git").join("info").join("exclude");
    let current = std::fs::read_to_string(&exclude).unwrap_or_default();
    if let Some(next) = exclude_with(&current) {
        let _ = exclude.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(&exclude, next);
    }
}

#[cfg(test)]
#[path = "attachments_for_run_tests.rs"]
mod attachments_for_run_tests;

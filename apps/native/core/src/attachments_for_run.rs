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
//! which is more than a silent drop would.
//!
//! And a PDF, wherever the run is, is opened for the agent first (0.713, `crate::pdf::readable_pdf`): its text and page
//! pictures go into the run folder's `.locust/attachments`, and a paragraph between the file list and the person's
//! words says where (`readable_pdf_note`). A PDF that cannot be read is sent as it was, and the runtime tries it its own
//! way. Unlike Electron, a PDF named by an absolute path is opened too: the native composer names a dropped or pasted
//! file where it is rather than copying it into `.locust/attachments` first, and Electron's copy would have been opened.

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use crate::attachments::{ATTACHMENT_DIR, attachment_destination, basename, exclude_with, extname, split_attachments, with_attachments_note};
use crate::pdf::{ReadablePdf, is_pdf_path, readable_pdf_note};

/// The prompt to send a run in `run_cwd`, its files placed there from `workspace` and its PDFs opened for the agent
/// (above). A message without attachments, or one with nothing to move and no PDF opened, comes back exactly as given.
pub fn attachments_for_run(prompt: &str, workspace: &Path, run_cwd: &Path) -> String {
    attachments_for_run_with(prompt, workspace, run_cwd, |folder, pdf, name| {
        crate::pdf::readable_pdf(folder, pdf, name, crate::pdf::MAX_PICTURES).ok()
    })
}

/// `attachments_for_run` with the PDF opener given (Electron's `readablePdf` argument): the app's is PDFKit's.
pub fn attachments_for_run_with(
    prompt: &str,
    workspace: &Path,
    run_cwd: &Path,
    mut readable_pdf: impl FnMut(&Path, &str, &str) -> Option<ReadablePdf>,
) -> String {
    let (text, attachments) = split_attachments(prompt);
    if attachments.is_empty() {
        return prompt.to_owned();
    }
    let placed = if same_path(workspace, run_cwd) { attachments.clone() } else { placed_for_run(&attachments, workspace, run_cwd) };
    let mut readables = Vec::new();
    let mut names: HashSet<String> = HashSet::new();
    for path in &placed {
        let source = run_cwd.join(path);
        if !is_pdf_path(path) || !(Path::new(path).is_absolute() || inside(run_cwd, &source)) {
            continue;
        }
        let name = distinct_name(basename(path), &names);
        names.insert(name.clone());
        if let Some(readable) = readable_pdf(run_cwd, path, &name) {
            readables.push(readable);
        }
    }
    // A PDF opened into `.locust/`: out of git, as every copy is.
    if !readables.is_empty() {
        keep_out_of_git(run_cwd);
    }
    let note = readable_pdf_note(&readables);
    if placed == attachments && note.is_none() {
        return prompt.to_owned();
    }
    with_attachments_note(&text, &placed, note.as_deref())
}

/// Electron's `distinctName`: the name, or the first `-2`, `-3` before its extension that is not taken.
fn distinct_name(name: &str, taken: &HashSet<String>) -> String {
    if !taken.contains(name) {
        return name.to_owned();
    }
    let extension = extname(name);
    let stem = &name[..name.len() - extension.len()];
    (2..).map(|n| format!("{stem}-{n}{extension}")).find(|candidate| !taken.contains(candidate)).unwrap_or_default()
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

/// `.locust/` out of the run folder's commits, the way the project's copies are kept out: `.git/info/exclude`, never
/// the owner's `.gitignore`. A worktree's `.git` is a file and its exclude is the project's, which already has the line;
/// a folder that is not a repository has nothing to do. Not writable: the copy still reaches the run.
pub(crate) fn keep_out_of_git(folder: &Path) {
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

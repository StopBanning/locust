//! COMPARISON COPIES & WORKTREES (Electron `main/compare-copies.ts`).
//!
//! Reuses `routine_copy.rs` for tree measuring, copying, and hashing.

use crate::compare::{CompareSlotId, compare_tree_id, is_commit_sha};
use crate::routine_copy::{
    BringIn, CopyChanges, LIMITS, Limits, MAX_COPIED_FILE_BYTES, copy_tree, hash_of, hash_tree,
    hash_tree_opts, measure, too_big,
};
use crate::store::Store;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub const COMPARE_ROOT_ENV: &str = "LOCUST_COMPARE_ROOT";
pub const COPY_DIFF_CHARS: usize = 6_000;
pub const COPY_DIFF_CUT: &str = "[Changes cut here; the rest of this column's diff is not shown.]";

/// Special case (`compare-copies.ts:291, 325`): `/dev/null` on Unix/macOS.
const EMPTY_FILE: &str = "/dev/null";

pub fn compare_root_from(default_root: &Path, env_val: Option<&str>) -> PathBuf {
    match env_val.map(str::trim).filter(|s| !s.is_empty()) {
        Some(path) => PathBuf::from(path),
        None => default_root.join("compare"),
    }
}

pub fn compare_root(store: &Store) -> PathBuf {
    let env_val = std::env::var(COMPARE_ROOT_ENV).ok();
    compare_root_from(store.root(), env_val.as_deref())
}

fn safe_name(compare_id: &str, slot: CompareSlotId) -> Result<String, String> {
    let valid = compare_id
        .strip_prefix("cmp_")
        .is_some_and(|rest| (1..=40).contains(&rest.len()) && rest.bytes().all(|b| b.is_ascii_alphanumeric()));
    if !valid {
        return Err("That comparison cannot name a copy.".into());
    }
    Ok(compare_tree_id(compare_id, slot))
}

fn manifest_path(root: &Path, compare_id: &str, slot: CompareSlotId) -> Result<PathBuf, String> {
    Ok(root.join(format!("{}.manifest.json", safe_name(compare_id, slot)?)))
}

pub fn copy_refusal(folder: &Path, limits: Option<Limits>) -> Option<String> {
    let limits = limits.unwrap_or(LIMITS);
    let ok = folder
        .canonicalize()
        .ok()
        .and_then(|src| measure(&src, limits).ok())
        .unwrap_or(false);
    if ok {
        None
    } else {
        Some(format!(
            "This folder is too big to give each model its own copy ({}).",
            too_big(limits)
        ))
    }
}

pub fn make_compare_copy(
    folder: &Path,
    compare_id: &str,
    slot: CompareSlotId,
    root: &Path,
    limits: Option<Limits>,
) -> Result<PathBuf, String> {
    let name = safe_name(compare_id, slot)?;
    let target = root.join(&name);
    if fs::symlink_metadata(&target).is_ok_and(|m| m.is_dir()) {
        return Ok(target);
    }
    let source = folder
        .canonicalize()
        .map_err(|_| "The folder to copy could not be read.".to_string())?;
    let limits = limits.unwrap_or(LIMITS);
    if !measure(&source, limits).unwrap_or(false) {
        return Err(format!(
            "It answers in a copy of the folder, and this folder is too big to copy ({}).",
            too_big(limits)
        ));
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    copy_tree(&source, &target).map_err(|e| format!("A copy of the folder could not be made: {e}"))?;
    let manifest = hash_tree(&target).map_err(|e| e.to_string())?;
    fs::write(
        manifest_path(root, compare_id, slot)?,
        serde_json::to_string(&manifest).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(target)
}

pub fn copy_changes(compare_id: &str, slot: CompareSlotId, root: &Path) -> Result<CopyChanges, String> {
    let name = safe_name(compare_id, slot)?;
    let target = root.join(name);
    let before: BTreeMap<String, String> = serde_json::from_str(
        &fs::read_to_string(manifest_path(root, compare_id, slot)?)
            .map_err(|_| "The copy's record could not be read.".to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let now = hash_tree(&target).map_err(|e| e.to_string())?;
    let changed = now
        .iter()
        .filter(|(p, h)| before.get(*p) != Some(h))
        .map(|(p, _)| p.clone())
        .collect();
    let deleted = before
        .keys()
        .filter(|p| !now.contains_key(*p))
        .cloned()
        .collect();
    Ok(CopyChanges { changed, deleted })
}

fn in_folder(folder: &Path, rel_path: &str) -> Result<PathBuf, String> {
    let parts: Vec<&str> = rel_path.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == ".." || p.contains('\\') || p.contains(':'))
    {
        return Err("A copy named a file outside the folder.".into());
    }
    Ok(parts.iter().fold(folder.to_path_buf(), |at, p| at.join(p)))
}

pub fn bring_in_copy(
    folder: &Path,
    compare_id: &str,
    slot: CompareSlotId,
    root: &Path,
) -> Result<BringIn, String> {
    let name = safe_name(compare_id, slot)?;
    let target = root.join(name);
    let before: BTreeMap<String, String> = serde_json::from_str(
        &fs::read_to_string(manifest_path(root, compare_id, slot)?)
            .map_err(|_| "The copy's record could not be read.".to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let CopyChanges { changed, deleted } = copy_changes(compare_id, slot, root)?;
    let mut touched: Vec<String> = changed.iter().chain(&deleted).cloned().collect();
    if touched.is_empty() {
        return Ok(BringIn::Nothing);
    }
    let mut yours = Vec::new();
    for path in &touched {
        let current = hash_of(&in_folder(folder, path)?).ok();
        if current.as_ref() != before.get(path) {
            yours.push(path.clone());
        }
    }
    if !yours.is_empty() {
        return Ok(BringIn::YourChanges(yours));
    }
    for path in &changed {
        let to = in_folder(folder, path)?;
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let from = path.split('/').fold(target.clone(), |at, p| at.join(p));
        fs::copy(from, &to).map_err(|e| e.to_string())?;
    }
    for path in &deleted {
        let _ = fs::remove_file(in_folder(folder, path)?);
    }
    touched.sort();
    Ok(BringIn::Brought(touched))
}

pub fn remove_compare_copies(compare_id: &str, root: &Path) -> Result<(), String> {
    let valid = compare_id
        .strip_prefix("cmp_")
        .is_some_and(|rest| (1..=40).contains(&rest.len()) && rest.bytes().all(|b| b.is_ascii_alphanumeric()));
    if !valid {
        return Ok(());
    }
    let prefix = format!("{compare_id}-");
    let Ok(entries) = fs::read_dir(root) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else { continue };
        if name_str.starts_with(&prefix) {
            let path = entry.path();
            if path.is_dir() {
                let _ = fs::remove_dir_all(path);
            } else {
                let _ = fs::remove_file(path);
            }
        }
    }
    Ok(())
}

pub fn bounded_copy_diff(text: &str) -> String {
    if text.len() <= COPY_DIFF_CHARS {
        text.to_owned()
    } else {
        let mut end = COPY_DIFF_CHARS;
        while end > 0 && !text.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}\n{COPY_DIFF_CUT}", &text[..end])
    }
}

fn run_git_output(args: &[&str], cwd: &Path) -> Result<(i32, Vec<u8>, String), String> {
    let out = Command::new("git")
        .args(["-c", "core.fsmonitor=false"])
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("git could not run: {e}"))?;
    let code = out.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    Ok((code, out.stdout, stderr))
}

fn run_git_text(args: &[&str], cwd: &Path) -> Result<String, String> {
    let (code, stdout, stderr) = run_git_output(args, cwd)?;
    if code != 0 {
        return Err(if stderr.trim().is_empty() {
            format!("git exited with {code}")
        } else {
            stderr.trim().to_owned()
        });
    }
    Ok(String::from_utf8_lossy(&stdout).into_owned())
}

pub fn count_copy_line_changes_with(
    folder: &Path,
    target: &Path,
    root: &Path,
    changes: &CopyChanges,
    mut run_git: impl FnMut(&[&str], &Path) -> Result<String, String>,
) -> Result<(usize, Option<usize>, Option<usize>), String> {
    let mut added = 0usize;
    let mut removed = 0usize;
    let mut counted = false;

    let mut count = |from: &str, to: &str| {
        if let Ok(printed) = run_git(
            &["diff", "--no-index", "--numstat", "--no-color", "--no-ext-diff", "--", from, to],
            root,
        ) {
            for line in printed.split('\n') {
                let mut parts = line.split('\t');
                let (Some(plus), Some(minus)) = (parts.next(), parts.next()) else {
                    continue;
                };
                counted = true;
                if let Ok(n) = plus.trim().parse::<usize>() {
                    added += n;
                }
                if let Ok(n) = minus.trim().parse::<usize>() {
                    removed += n;
                }
            }
        }
    };

    for path in &changes.changed {
        let mine = in_folder(folder, path)?;
        let from_str = if mine.exists() {
            mine.to_string_lossy().into_owned()
        } else {
            EMPTY_FILE.to_owned()
        };
        let to = in_folder(target, path)?;
        count(&from_str, &to.to_string_lossy());
    }
    for path in &changes.deleted {
        let mine = in_folder(folder, path)?;
        count(&mine.to_string_lossy(), EMPTY_FILE);
    }

    let files = changes.changed.len() + changes.deleted.len();
    if counted {
        Ok((files, Some(added), Some(removed)))
    } else {
        Ok((files, None, None))
    }
}

pub fn copy_line_changes(
    folder: &Path,
    compare_id: &str,
    slot: CompareSlotId,
    root: &Path,
) -> Result<(usize, Option<usize>, Option<usize>), String> {
    let name = safe_name(compare_id, slot)?;
    let target = root.join(name);
    let changes = copy_changes(compare_id, slot, root)?;
    count_copy_line_changes_with(folder, &target, root, &changes, |args, cwd| {
        let (_, stdout, _) = run_git_output(args, cwd)?;
        Ok(String::from_utf8_lossy(&stdout).into_owned())
    })
}

fn local_file(root: &Path, rel_path: &str) -> Result<PathBuf, String> {
    let parts: Vec<&str> = rel_path.split('/').collect();
    if parts
        .iter()
        .any(|p| p.is_empty() || *p == "." || *p == ".." || p.contains('\\') || p.contains(':'))
    {
        return Err("A change names a file outside the copy.".into());
    }
    let mut full = root.to_path_buf();
    for part in parts {
        full = full.join(part);
        if fs::symlink_metadata(&full).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("A changed file is a link, not a local file.".into());
        }
    }
    Ok(full)
}

pub fn copy_diff(
    folder: &Path,
    compare_id: &str,
    slot: CompareSlotId,
    root: &Path,
    base: Option<&str>,
) -> Result<String, String> {
    let name = safe_name(compare_id, slot)?;
    let target = root.join(name);
    if !fs::symlink_metadata(&target).is_ok_and(|m| m.is_dir()) {
        return Err("The column's copy is no longer here.".into());
    }
    if let Some(b) = base
        && !is_commit_sha(b)
    {
        return Err("The comparison's starting point is missing.".into());
    }

    let before: Option<BTreeMap<String, String>> = if base.is_none() {
        let raw = fs::read_to_string(manifest_path(root, compare_id, slot)?)
            .map_err(|_| "The copy's record could not be read.".to_string())?;
        Some(serde_json::from_str(&raw).map_err(|e| e.to_string())?)
    } else {
        None
    };

    // (added, deleted)
    let mut files: BTreeMap<String, (bool, bool)> = BTreeMap::new();
    let flags = ["--no-color", "--no-ext-diff", "--no-textconv", "--no-renames", "--no-relative"];

    if let Some(b) = base {
        let mut args = vec!["diff"];
        args.extend_from_slice(&flags);
        args.extend_from_slice(&["--name-status", "-z", b, "--"]);
        let (_, out, err) = run_git_output(&args, &target)?;
        if !err.trim().is_empty() && out.is_empty() {
            return Err(err);
        }
        let entries: Vec<&[u8]> = out.split(|&byte| byte == 0).collect();
        let mut i = 0;
        while i + 1 < entries.len() {
            let kind = String::from_utf8_lossy(entries[i]);
            let path = String::from_utf8_lossy(entries[i + 1]);
            if !matches!(kind.as_ref(), "A" | "M" | "D" | "T") || path.is_empty() {
                return Err("The column's changes could not be read.".into());
            }
            files.insert(path.into_owned(), (kind == "A", kind == "D"));
            i += 2;
        }
        let (_, untracked, _) = run_git_output(&["ls-files", "--others", "--exclude-standard", "-z"], &target)?;
        for entry in untracked.split(|&byte| byte == 0) {
            if entry.is_empty() {
                continue;
            }
            let path = String::from_utf8_lossy(entry).into_owned();
            files.insert(path, (true, false));
        }
    } else {
        let before = before.as_ref().unwrap();
        let now = hash_tree_opts(&target, true).map_err(|e| e.to_string())?;
        for (path, h) in &now {
            if before.get(path) != Some(h) {
                files.insert(path.clone(), (!before.contains_key(path), false));
            }
        }
        for path in before.keys() {
            if !now.contains_key(path) {
                files.insert(path.clone(), (false, true));
            }
        }
    }

    let mut text = String::new();
    for (path, (added, deleted)) in files {
        if text.len() > COPY_DIFF_CHARS {
            break;
        }
        let file = local_file(&target, &path)?;
        let source = if before.is_none() || added {
            PathBuf::from(EMPTY_FILE)
        } else {
            local_file(folder, &path)?
        };
        let check_source = || -> Result<(), String> {
            if let Some(b) = &before
                && !added
                && hash_of(&source).ok().as_ref() != b.get(&path)
            {
                return Err("The source changed after the column was copied, so its original diff cannot be read.".into());
            }
            Ok(())
        };
        check_source()?;

        let too_big_file = fs::metadata(&file).is_ok_and(|m| m.len() > MAX_COPIED_FILE_BYTES);
        let patch = if too_big_file {
            "File too large; contents not shown.".to_owned()
        } else if let Some(b) = base
            && !added
        {
            let literal = format!(":(literal){path}");
            let mut args = vec!["diff"];
            args.extend_from_slice(&flags);
            args.extend_from_slice(&[b, "--", &literal]);
            run_git_text(&args, &target)?
        } else {
            let from_arg = if added {
                EMPTY_FILE.to_owned()
            } else {
                source.to_string_lossy().into_owned()
            };
            let to_arg = if deleted {
                EMPTY_FILE.to_owned()
            } else {
                file.to_string_lossy().into_owned()
            };
            let mut args = vec!["-c", "core.autocrlf=false", "-c", "core.safecrlf=false", "diff"];
            args.extend_from_slice(&flags);
            args.extend_from_slice(&["--no-index", "--", &from_arg, &to_arg]);
            let (code, stdout, stderr) = run_git_output(&args, &target)?;
            let out_str = String::from_utf8_lossy(&stdout).into_owned();
            if code == 0 {
                out_str
            } else if code == 1 && !out_str.is_empty() && stderr.trim().is_empty() {
                out_str
            } else {
                return Err(if stderr.trim().is_empty() {
                    format!("git diff exited with {code}")
                } else {
                    stderr.trim().to_owned()
                });
            }
        };
        check_source()?;

        let is_binary = patch.contains('\0')
            || patch.lines().any(|l| {
                l.starts_with("Binary files ") || l.starts_with("GIT binary patch") || l.starts_with("Binary file;")
            });
        let body = if is_binary {
            "Binary file; contents not shown.".to_owned()
        } else if patch == "File too large; contents not shown." {
            patch
        } else {
            let modes: Vec<&str> = patch
                .lines()
                .filter(|l| {
                    l.starts_with("old mode ")
                        || l.starts_with("new mode ")
                        || l.starts_with("new file mode ")
                        || l.starts_with("deleted file mode ")
                })
                .collect();
            let hunk = patch
                .lines()
                .position(|l| l.starts_with("@@ "))
                .map(|idx| patch.lines().skip(idx).collect::<Vec<_>>().join("\n"))
                .unwrap_or_default();
            let mut parts = Vec::new();
            if !modes.is_empty() {
                parts.push(modes.join("\n"));
            }
            if !hunk.is_empty() {
                parts.push(hunk);
            }
            parts.join("\n")
        };

        let tag = if added {
            " (added)"
        } else if deleted {
            " (deleted)"
        } else {
            ""
        };
        let quoted_path = serde_json::to_string(&path).unwrap_or_else(|_| format!("\"{path}\""));
        text.push_str(&format!("File {quoted_path}{tag}:\n{body}\n\n"));
    }

    Ok(bounded_copy_diff(text.trim()))
}

// ---- Git worktrees for compare columns (outside the project folder) ------------------------------------------------

pub fn is_git_repo(folder: &Path) -> bool {
    run_git_text(&["rev-parse", "--is-inside-work-tree"], folder)
        .map(|s| s.trim() == "true")
        .unwrap_or(false)
}

pub fn git_head(folder: &Path) -> Option<String> {
    let sha = run_git_text(&["rev-parse", "HEAD"], folder).ok()?;
    let trimmed = sha.trim().to_owned();
    is_commit_sha(&trimmed).then_some(trimmed)
}

pub fn ensure_compare_worktree(
    folder: &Path,
    compare_id: &str,
    slot: CompareSlotId,
    root: &Path,
    base: Option<&str>,
) -> Result<PathBuf, String> {
    let name = safe_name(compare_id, slot)?;
    let target = root.join(name);
    if fs::symlink_metadata(&target).is_ok_and(|m| m.is_dir()) {
        return Ok(target);
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let target_str = target.to_string_lossy().into_owned();
    let mut args = vec!["worktree", "add", "--detach", &target_str];
    if let Some(b) = base {
        args.push(b);
    }
    run_git_text(&args, folder)?;
    Ok(target)
}

pub fn discard_compare_worktrees(
    folder: &Path,
    compare_id: &str,
    slots: &[CompareSlotId],
    root: &Path,
) {
    // Special case (a-kept-comparison-leaves-no-branches): read git's worktree record before pruning/removing
    // so the `locust/compare-*` branch is still deleted even if the worktree folder was already removed.
    let porcelain = run_git_text(&["worktree", "list", "--porcelain"], folder).unwrap_or_default();
    let mut branches = Vec::new();
    for &slot in slots {
        if let Ok(name) = safe_name(compare_id, slot) {
            let target = root.join(&name);
            let recorded = crate::worktrees::recorded_branch_of(&porcelain, &target);
            if !recorded.is_empty() {
                branches.push(recorded);
            } else {
                branches.push(format!("locust/{name}"));
            }
            let target_str = target.to_string_lossy().into_owned();
            let _ = run_git_text(&["worktree", "remove", "--force", &target_str], folder);
        }
    }
    let _ = run_git_text(&["worktree", "prune"], folder);
    for branch in branches {
        if branch.starts_with("locust/") {
            let _ = run_git_text(&["branch", "-D", &branch], folder);
        }
    }
}

/// The page name that can be read in this column's copy, then in the folder after Keep (`compare-page-path.ts`).
pub fn compare_page_path(requested: &str, copy: &Path) -> Option<String> {
    let req_path = Path::new(requested);
    if !req_path.is_absolute() {
        return Some(requested.to_owned());
    }
    let rel = req_path.strip_prefix(copy).ok()?;
    if rel.as_os_str().is_empty() {
        return None;
    }
    if rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return None;
    }
    Some(rel.to_string_lossy().into_owned())
}

/// Resolves a column's HTML page for preview: checks its copy first, refuses non-kept columns after Keep,
/// and falls back to the workspace folder for the kept column alone (`main/index.ts:6948`).
pub fn resolve_compare_page_path(
    requested: &str,
    folder: &Path,
    compare: &crate::compare::Compare,
    slot: CompareSlotId,
    copy_root: &Path,
) -> Result<PathBuf, String> {
    let name = safe_name(&compare.compare_id, slot)?;
    let modern_copy = copy_root.join(&name);
    let legacy_copy = folder.join(".locust").join("compare").join(&name);
    let copy = if compare.changes_in == Some(crate::compare::ChangesIn::Copy) || modern_copy.exists() {
        modern_copy.clone()
    } else {
        legacy_copy
    };
    let Some(column_path) = compare_page_path(requested, &modern_copy)
        .or_else(|| compare_page_path(requested, &copy))
    else {
        return Ok(PathBuf::from(requested));
    };
    let in_copy = copy.join(&column_path);
    if in_copy.is_file() {
        return Ok(in_copy);
    }
    if compare.kept.as_ref().is_some_and(|k| k.slot != slot) {
        return Err(
            "Its copy was removed when you kept another model, so this page is no longer on this computer."
                .into(),
        );
    }
    Ok(folder.join(column_path))
}

/// What a comparison's copy counts and brings in: everything but what a tool made (Electron's NOT_BYPRODUCTS, 0.451).
/// In Auto a column runs its code, and a one-line fix counted a second file, its `__pycache__`, which Keep would have
/// put in the person's folder. Narrow on purpose: `dist/` or `build/` can be real source.
pub const NOT_BYPRODUCTS: [&str; 7] = [
    ".",
    ":(exclude,glob)**/__pycache__/**",
    ":(exclude,glob)**/*.pyc",
    ":(exclude,glob)**/.pytest_cache/**",
    ":(exclude,glob)**/.mypy_cache/**",
    ":(exclude,glob)**/.ruff_cache/**",
    ":(exclude,glob)**/node_modules/**",
];

/// Never runs a program the repository's config names to draw a diff.
const PLAIN_DIFF: [&str; 3] = ["--no-color", "--no-ext-diff", "--no-textconv"];

/// "+12 -3 in 2 files" from `git diff --numstat`; a binary file ("-") counts as a file with no lines (treeChangesOf).
pub fn tree_changes_of(numstat: &str) -> (usize, usize, usize) {
    let (mut files, mut added, mut removed) = (0, 0, 0);
    for line in numstat.lines() {
        let mut parts = line.split('\t');
        let (plus, minus, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next());
        if path.is_none_or(str::is_empty) {
            continue;
        }
        files += 1;
        added += plus.parse::<usize>().unwrap_or(0);
        removed += minus.parse::<usize>().unwrap_or(0);
    }
    (files, added, removed)
}

/// Where a worktree column was cut from: the comparison's recorded base, a commit.
fn worktree_base(base: Option<&str>) -> Result<&str, String> {
    base.filter(|b| is_commit_sha(b)).ok_or_else(|| "The comparison's starting point is missing.".to_string())
}

/// What a worktree column changed, saved or not, new files included, and nothing a tool made (Electron's
/// `changes`). Staging everything in the column's own index is what lets new files count.
pub fn compare_worktree_changes(compare_id: &str, slot: CompareSlotId, root: &Path, base: Option<&str>) -> Result<(usize, usize, usize), String> {
    let target = root.join(safe_name(compare_id, slot)?);
    let base = worktree_base(base)?;
    run_git_text(&["add", "-A"], &target)?;
    let mut args = vec!["diff"];
    args.extend_from_slice(&PLAIN_DIFF);
    args.extend_from_slice(&["--cached", "--numstat", base, "--"]);
    args.extend_from_slice(&NOT_BYPRODUCTS);
    Ok(tree_changes_of(&run_git_text(&args, &target)?))
}

/// Keep this one, for a column that worked in a git worktree (Electron's `bringIn`): its changes from where it was cut
/// go into the folder uncommitted, as a teammate editing the folder leaves them. It refuses, touching nothing, when the
/// person has changed a file it changes (`YourChanges`), or when the folder has moved on so the change no longer
/// applies (an error carrying git's reason). Never a file copied over the person's own: `git apply --check` first.
pub fn bring_in_compare_worktree(folder: &Path, compare_id: &str, slot: CompareSlotId, root: &Path, base: Option<&str>) -> Result<BringIn, String> {
    let name = safe_name(compare_id, slot)?;
    let target = root.join(&name);
    let base = worktree_base(base)?;
    run_git_text(&["add", "-A"], &target)?;
    let mut changed = vec!["diff", "--cached", "--name-only", "-z", base, "--"];
    changed.extend_from_slice(&NOT_BYPRODUCTS);
    let (_, out, _) = run_git_output(&changed, &target)?;
    let files: Vec<String> = out.split(|&b| b == 0).filter(|s| !s.is_empty()).map(|s| String::from_utf8_lossy(s).into_owned()).collect();
    if files.is_empty() {
        return Ok(BringIn::Nothing);
    }
    let status = run_git_text(&["status", "--porcelain", "-z", "--untracked-files=all"], folder)?;
    let yours: Vec<String> = crate::worktrees::status_entries_of(&status).into_iter().map(|(_, path)| path).filter(|p| files.contains(p)).collect();
    if !yours.is_empty() {
        return Ok(BringIn::YourChanges(yours));
    }
    // A patch file, not a pipe: a big change must not meet a buffer limit.
    let git_dir = PathBuf::from(run_git_text(&["rev-parse", "--absolute-git-dir"], folder)?.trim());
    let patch = git_dir.join(format!("locust-bring-in-{name}.patch"));
    let patch_str = patch.to_string_lossy().into_owned();
    let output = format!("--output={patch_str}");
    let result = (|| {
        let mut diff = vec!["diff"];
        diff.extend_from_slice(&PLAIN_DIFF);
        diff.extend_from_slice(&["--cached", "--binary", output.as_str(), base, "--"]);
        diff.extend_from_slice(&NOT_BYPRODUCTS);
        run_git_text(&diff, &target)?;
        if let Err(why) = run_git_text(&["apply", "--check", &patch_str], folder) {
            return Err(why.trim_start_matches("git apply:").trim().to_owned());
        }
        run_git_text(&["apply", &patch_str], folder).map(|_| ())
    })();
    let _ = fs::remove_file(&patch);
    result?;
    Ok(BringIn::Brought(files))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "locust-cmp-copy-{name}-{}-{}",
            std::process::id(),
            crate::invariants::unique_nanos()
        ));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_compare_root_follows_the_environment() {
        let def = PathBuf::from("/tmp/native-data");
        assert_eq!(compare_root_from(&def, None), def.join("compare"));
        assert_eq!(compare_root_from(&def, Some("")), def.join("compare"));
        assert_eq!(compare_root_from(&def, Some("  ")), def.join("compare"));
        assert_eq!(
            compare_root_from(&def, Some("  /tmp/drive-scratch/compare  ")),
            PathBuf::from("/tmp/drive-scratch/compare")
        );

        let src = temp_dir("env-src");
        let root = temp_dir("env-root");
        fs::write(src.join("page.html"), "<h1>was</h1>\n").unwrap();
        let copy = make_compare_copy(&src, "cmp_drive1", CompareSlotId::A, &root, None).unwrap();
        assert!(copy.starts_with(&root));
        fs::write(copy.join("page.html"), "<h1>now</h1>\n").unwrap();
        assert_eq!(
            copy_changes("cmp_drive1", CompareSlotId::A, &root).unwrap(),
            CopyChanges { changed: vec!["page.html".into()], deleted: vec![] }
        );
        assert_eq!(
            bring_in_copy(&src, "cmp_drive1", CompareSlotId::A, &root).unwrap(),
            BringIn::Brought(vec!["page.html".into()])
        );
        assert_eq!(fs::read_to_string(src.join("page.html")).unwrap(), "<h1>now</h1>\n");
        remove_compare_copies("cmp_drive1", &root).unwrap();
        assert!(!copy.exists());

        let _ = fs::remove_dir_all(&src);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_compare_column_works_outside_the_folder() {
        let project = temp_dir("wt-proj");
        let elsewhere = temp_dir("wt-root");
        run_git_text(&["init", "-q", "-b", "main"], &project).unwrap();
        run_git_text(&["config", "user.email", "smoke@locust.test"], &project).unwrap(); // made-up test address; public-guard: allow
        run_git_text(&["config", "user.name", "Locust smoke"], &project).unwrap();
        fs::write(project.join("index.html"), "<h1>Todo</h1>\n").unwrap();
        run_git_text(&["add", "-A"], &project).unwrap();
        run_git_text(&["commit", "-q", "-m", "first"], &project).unwrap();

        let column = ensure_compare_worktree(&project, "cmp_abc123", CompareSlotId::B, &elsewhere, None).unwrap();
        let project_canon = project.canonicalize().unwrap();
        let column_canon = column.canonicalize().unwrap();
        let elsewhere_canon = elsewhere.canonicalize().unwrap();
        assert!(!column_canon.starts_with(&project_canon));
        assert!(column_canon.starts_with(&elsewhere_canon));
        assert!(fs::read_to_string(column.join("index.html")).unwrap().contains("Todo"));
        let listed = run_git_text(&["worktree", "list", "--porcelain"], &project).unwrap();
        assert!(listed.contains(&*column_canon.to_string_lossy()));

        discard_compare_worktrees(&project, "cmp_abc123", &[CompareSlotId::B], &elsewhere);
        let _ = fs::remove_dir_all(&project);
        let _ = fs::remove_dir_all(&elsewhere);
    }

    #[test]
    fn copy_diff_reads_git_and_plain_copies() {
        // Plain copy + source changed refusal + oversized file + unsafe path
        let folder = temp_dir("diff-plain-src");
        let root = temp_dir("diff-plain-root");
        fs::write(folder.join("sum.js"), "export const sum = (a, b) => a - b\n").unwrap();
        fs::write(folder.join("old.txt"), "remove this\n").unwrap();

        let copy = make_compare_copy(&folder, "cmp_plain1", CompareSlotId::B, &root, None).unwrap();
        assert_eq!(copy_diff(&folder, "cmp_plain1", CompareSlotId::B, &root, None).unwrap(), "");

        fs::write(copy.join("sum.js"), "export const sum = (a, b) => a + b\n").unwrap();
        fs::write(copy.join("new.txt"), "new work\n").unwrap();
        fs::remove_file(copy.join("old.txt")).unwrap();

        let patch = copy_diff(&folder, "cmp_plain1", CompareSlotId::B, &root, None).unwrap();
        assert!(patch.contains("-export const sum = (a, b) => a - b"));
        assert!(patch.contains("+export const sum = (a, b) => a + b"));
        assert!(patch.contains("File \"new.txt\" (added):"));
        assert!(patch.contains("File \"old.txt\" (deleted):"));
        assert!(!patch.contains(&*copy.to_string_lossy()));
        assert!(!patch.contains(&*folder.to_string_lossy()));

        let (files, added, removed) = copy_line_changes(&folder, "cmp_plain1", CompareSlotId::B, &root).unwrap();
        assert_eq!(files, 3);
        assert!(added.is_some() && removed.is_some());

        // Moving source refuses diff
        fs::write(folder.join("sum.js"), "the person's newer code\n").unwrap();
        let err = copy_diff(&folder, "cmp_plain1", CompareSlotId::B, &root, None).unwrap_err();
        assert!(err.contains("source changed"));

        let _ = fs::remove_dir_all(&folder);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn kept_comparison_leaves_no_branches_even_if_folder_removed_first() {
        let folder = temp_dir("kept-branches-repo");
        let root = temp_dir("kept-branches-root");
        let git = |args: &[&str]| {
            let status = Command::new("git")
                .args(args)
                .current_dir(&folder)
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?} failed");
        };
        git(&["init", "-q", "-b", "main"]);
        git(&["config", "user.email", "smoke@locust.test"]);
        git(&["config", "user.name", "Locust smoke"]);
        fs::write(folder.join("NOTES.md"), "# notes\n").unwrap();
        git(&["add", "NOTES.md"]);
        git(&["commit", "-q", "-m", "first"]);

        let base = git_head(&folder).unwrap();
        let wt_a = ensure_compare_worktree(&folder, "cmp_x", CompareSlotId::A, &root, Some(&base)).unwrap();
        let wt_b = ensure_compare_worktree(&folder, "cmp_x", CompareSlotId::B, &root, Some(&base)).unwrap();
        run_git_text(&["checkout", "-q", "-b", "locust/cmp_x-a"], &wt_a).unwrap();
        run_git_text(&["checkout", "-q", "-b", "locust/cmp_x-b"], &wt_b).unwrap();
        let branches_before = run_git_text(&["branch", "--format=%(refname:short)"], &folder).unwrap();
        assert!(branches_before.lines().any(|b| b.starts_with("locust/cmp_x-")));

        // Simulate folder removed before discard
        fs::remove_dir_all(&wt_a).unwrap();
        discard_compare_worktrees(&folder, "cmp_x", &[CompareSlotId::A, CompareSlotId::B], &root);
        let branches_after = run_git_text(&["branch", "--format=%(refname:short)"], &folder).unwrap();
        assert_eq!(branches_after.lines().collect::<Vec<_>>(), vec!["main"]);

        let _ = fs::remove_dir_all(&folder);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_kept_page_forgets_its_removed_copy() {
        let root = temp_dir("kept-page");
        let folder = root.join("folder");
        let copy = root.join("copy-a");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("index.html"), "<h1>kept</h1>").unwrap();

        assert_eq!(compare_page_path("site/index.html", &copy), Some("site/index.html".into()));
        assert_eq!(
            compare_page_path(&copy.join("site").join("index.html").to_string_lossy(), &copy),
            Some("site/index.html".into())
        );
        assert_eq!(compare_page_path(&root.join("index.html").to_string_lossy(), &copy), None);
        assert_eq!(
            compare_page_path(&root.join("copy-a-other").join("index.html").to_string_lossy(), &copy),
            None
        );
        assert_eq!(
            compare_page_path(&copy.join("..notes.html").to_string_lossy(), &copy),
            Some("..notes.html".into())
        );

        let cmp = crate::compare::Compare {
            compare_id: "cmp_page1".into(),
            teammate_id: None,
            prompt: "Build a page".into(),
            created_at: "2026-11-13T00:00:00.000Z".into(),
            slots: vec![
                crate::compare::CompareSlot {
                    slot: CompareSlotId::A,
                    route: crate::compare::CompareRoute::new("codex", "m1"),
                    mission_ids: vec!["m_a".into()],
                    refused: None,
                    retried: Vec::new(),
                    folder: None,
                },
                crate::compare::CompareSlot {
                    slot: CompareSlotId::B,
                    route: crate::compare::CompareRoute::new("claude", "m2"),
                    mission_ids: vec!["m_b".into()],
                    refused: None,
                    retried: Vec::new(),
                    folder: None,
                },
            ],
            changes: true,
            changes_in: Some(crate::compare::ChangesIn::Copy),
            blind: false,
            kept: Some(crate::compare::CompareKept {
                slot: CompareSlotId::A,
                at: "2026-11-13T00:01:00.000Z".into(),
                brought: Some(vec!["index.html".into()]),
            }),
            judge: None,
            base: None,
            rerun_of: None,
            reruns: Vec::new(),
            settled: false,
            point_drafts: Vec::new(),
        };
        let copy_root = root.join("copies");
        let requested_a = copy_root.join("cmp_page1-a").join("index.html");
        let resolved_a = resolve_compare_page_path(
            &requested_a.to_string_lossy(),
            &folder,
            &cmp,
            CompareSlotId::A,
            &copy_root,
        )
        .unwrap();
        assert_eq!(resolved_a, folder.join("index.html"));

        let requested_b = copy_root.join("cmp_page1-b").join("index.html");
        let err_b = resolve_compare_page_path(
            &requested_b.to_string_lossy(),
            &folder,
            &cmp,
            CompareSlotId::B,
            &copy_root,
        )
        .unwrap_err();
        assert!(err_b.contains("removed when you kept another model"));

        let _ = fs::remove_dir_all(&root);
    }
}

/// Electron's a-kept-column-brings-its-changes-in.test.ts, against real git: a comparison that edits gives each column
/// its own worktree, cut from the person's last commit, and Keep this one puts the kept column's changes into the
/// folder uncommitted.
#[cfg(test)]
mod worktree_keep_tests {
    use super::*;

    struct Two {
        root: PathBuf,
        folder: PathBuf,
        copies: PathBuf,
        base: String,
        a: PathBuf,
    }
    impl Drop for Two {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn git(args: &[&str], cwd: &Path) -> String {
        let out = std::process::Command::new("git").args(args).current_dir(cwd).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// A repository and two columns' copies: A fixed cart.py and added notes.md; B rewrote README.md.
    fn two_columns() -> Two {
        let root = std::env::temp_dir().join(format!("locust-bring-in-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
        let (folder, copies) = (root.join("project"), root.join("compare"));
        fs::create_dir_all(&folder).unwrap();
        git(&["init", "-q", "-b", "main"], &folder);
        git(&["config", "user.email", "person@locust.test"], &folder); // public-guard: allow
        git(&["config", "user.name", "The person"], &folder);
        fs::write(folder.join("cart.py"), "def total(items):\n    return 0\n").unwrap();
        fs::write(folder.join("README.md"), "mine\n").unwrap();
        git(&["add", "."], &folder);
        git(&["commit", "-q", "-m", "first"], &folder);
        let base = git(&["rev-parse", "HEAD"], &folder).trim().to_owned();
        let a = ensure_compare_worktree(&folder, "cmp_1", CompareSlotId::A, &copies, Some(&base)).unwrap();
        let b = ensure_compare_worktree(&folder, "cmp_1", CompareSlotId::B, &copies, Some(&base)).unwrap();
        fs::write(a.join("cart.py"), "def total(items):\n    return sum(items)\n").unwrap();
        fs::write(a.join("notes.md"), "summed items\n").unwrap();
        fs::write(b.join("README.md"), "rewritten\n").unwrap();
        Two { root, folder, copies, base, a }
    }

    fn status(folder: &Path) -> Vec<String> {
        let mut lines: Vec<String> = git(&["status", "--porcelain"], folder).lines().map(str::to_owned).collect();
        lines.sort();
        lines
    }

    #[test]
    fn counts_what_each_copy_changed_saved_or_not_new_files_included() {
        let t = two_columns();
        assert_eq!(compare_worktree_changes("cmp_1", CompareSlotId::A, &t.copies, Some(&t.base)).unwrap(), (2, 2, 1));
        assert_eq!(compare_worktree_changes("cmp_1", CompareSlotId::B, &t.copies, Some(&t.base)).unwrap(), (1, 1, 1));
    }

    #[test]
    fn puts_the_kept_copys_changes_into_the_folder_uncommitted_and_nothing_of_the_others() {
        let t = two_columns();
        let head = git(&["rev-parse", "HEAD"], &t.folder);
        assert_eq!(bring_in_compare_worktree(&t.folder, "cmp_1", CompareSlotId::A, &t.copies, Some(&t.base)).unwrap(), BringIn::Brought(vec!["cart.py".into(), "notes.md".into()]));
        assert_eq!(fs::read_to_string(t.folder.join("cart.py")).unwrap(), "def total(items):\n    return sum(items)\n");
        assert_eq!(fs::read_to_string(t.folder.join("notes.md")).unwrap(), "summed items\n");
        assert_eq!(fs::read_to_string(t.folder.join("README.md")).unwrap(), "mine\n");
        // Nothing committed, nothing staged: as a teammate editing the folder leaves it.
        assert_eq!(git(&["rev-parse", "HEAD"], &t.folder), head);
        assert_eq!(git(&["diff", "--cached", "--name-only"], &t.folder).trim(), "");
        assert_eq!(status(&t.folder), vec![" M cart.py".to_owned(), "?? notes.md".to_owned()]);
    }

    #[test]
    fn refuses_touching_nothing_when_the_person_has_changed_a_file_it_changes() {
        let t = two_columns();
        fs::write(t.folder.join("README.md"), "my own edit\n").unwrap();
        assert_eq!(bring_in_compare_worktree(&t.folder, "cmp_1", CompareSlotId::B, &t.copies, Some(&t.base)).unwrap(), BringIn::YourChanges(vec!["README.md".into()]));
        assert_eq!(fs::read_to_string(t.folder.join("README.md")).unwrap(), "my own edit\n");
    }

    #[test]
    fn refuses_touching_nothing_when_the_folder_moved_on_and_it_no_longer_applies() {
        let t = two_columns();
        fs::write(t.folder.join("cart.py"), "def total(items):\n    return len(items)\n").unwrap();
        git(&["commit", "-q", "-am", "mine moved on"], &t.folder);
        assert!(bring_in_compare_worktree(&t.folder, "cmp_1", CompareSlotId::A, &t.copies, Some(&t.base)).is_err());
        assert_eq!(fs::read_to_string(t.folder.join("cart.py")).unwrap(), "def total(items):\n    return len(items)\n");
        assert!(!t.folder.join("notes.md").exists());
        assert!(status(&t.folder).is_empty());
    }

    #[test]
    fn neither_counts_nor_brings_in_what_a_tool_made_like_a_python_cache() {
        let t = two_columns();
        fs::create_dir_all(t.a.join("__pycache__")).unwrap();
        fs::write(t.a.join("__pycache__").join("cart.cpython-314.pyc"), "bytes").unwrap();
        fs::create_dir_all(t.a.join("node_modules").join("left-pad")).unwrap();
        fs::write(t.a.join("node_modules").join("left-pad").join("index.js"), "x").unwrap();
        assert_eq!(compare_worktree_changes("cmp_1", CompareSlotId::A, &t.copies, Some(&t.base)).unwrap(), (2, 2, 1));
        assert_eq!(bring_in_compare_worktree(&t.folder, "cmp_1", CompareSlotId::A, &t.copies, Some(&t.base)).unwrap(), BringIn::Brought(vec!["cart.py".into(), "notes.md".into()]));
        assert!(!t.folder.join("__pycache__").exists());
        assert!(!t.folder.join("node_modules").exists());
    }

    #[test]
    fn says_so_when_a_copy_changed_nothing() {
        let t = two_columns();
        ensure_compare_worktree(&t.folder, "cmp_1", CompareSlotId::C, &t.copies, Some(&t.base)).unwrap();
        assert_eq!(bring_in_compare_worktree(&t.folder, "cmp_1", CompareSlotId::C, &t.copies, Some(&t.base)).unwrap(), BringIn::Nothing);
        assert!(status(&t.folder).is_empty());
    }

    #[test]
    fn discards_a_copy_whatever_is_in_it_and_leaves_the_others() {
        let t = two_columns();
        discard_compare_worktrees(&t.folder, "cmp_1", &[CompareSlotId::A], &t.copies);
        assert!(!t.a.exists());
        assert!(t.copies.join(safe_name("cmp_1", CompareSlotId::B).unwrap()).exists());
        // Discarding what is already gone is not an error.
        discard_compare_worktrees(&t.folder, "cmp_1", &[CompareSlotId::A], &t.copies);
    }

    #[test]
    fn reads_numstat_a_binary_file_counting_as_a_file_with_no_lines() {
        assert_eq!(tree_changes_of("3\t1\tcart.py\n-\t-\tlogo.png\n1\t0\tnotes.md\n"), (3, 4, 1));
        assert_eq!(tree_changes_of(""), (0, 0, 0));
    }

    #[test]
    fn refuses_bring_in_copy_over_own_edits_or_same_name_file_and_checks_lines_and_upfront_limit() {
        let mk = |name: &str| {
            let d = std::env::temp_dir().join(format!(
                "locust-cmp-more-{name}-{}-{}",
                std::process::id(),
                crate::invariants::unique_nanos()
            ));
            fs::create_dir_all(&d).unwrap();
            d
        };
        // 1) Refusal over the person's own edits in plain copy
        let folder = mk("own-edits-src");
        let root = mk("own-edits-root");
        fs::write(folder.join("page.html"), "<h1>before</h1>\n").unwrap();
        let copy = make_compare_copy(&folder, "cmp_own1", CompareSlotId::A, &root, None).unwrap();
        fs::write(copy.join("page.html"), "<h1>from-copy</h1>\n").unwrap();
        fs::write(folder.join("page.html"), "<h1>person-edited</h1>\n").unwrap();
        assert_eq!(
            bring_in_copy(&folder, "cmp_own1", CompareSlotId::A, &root).unwrap(),
            BringIn::YourChanges(vec!["page.html".into()])
        );
        assert_eq!(fs::read_to_string(folder.join("page.html")).unwrap(), "<h1>person-edited</h1>\n");

        // 2) Refusal when a same-name file was created in source meanwhile
        fs::write(copy.join("added.txt"), "copy added\n").unwrap();
        fs::write(folder.join("added.txt"), "person added meanwhile\n").unwrap();
        assert_eq!(
            bring_in_copy(&folder, "cmp_own1", CompareSlotId::A, &root).unwrap(),
            BringIn::YourChanges(vec!["added.txt".into(), "page.html".into()])
        );

        // 3) Git line counting via copy_line_changes
        let line_src = mk("line-src");
        let line_root = mk("line-root");
        fs::write(line_src.join("notes.txt"), "one\ntwo\n").unwrap();
        let line_copy = make_compare_copy(&line_src, "cmp_lines1", CompareSlotId::A, &line_root, None).unwrap();
        fs::write(line_copy.join("notes.txt"), "one\ntwo\nthree\n").unwrap();
        fs::write(line_copy.join("brand_new.txt"), "alpha\nbeta\n").unwrap();
        let (files, added, removed) = copy_line_changes(&line_src, "cmp_lines1", CompareSlotId::A, &line_root).unwrap();
        assert_eq!(files, 2);
        assert_eq!(added, Some(3));
        assert_eq!(removed, Some(0));

        // 4) Up-front refusal when source exceeds copy limit
        let tight = Limits { files: 1, bytes: 1_000_000 };
        assert_eq!(copy_refusal(&line_src, Some(tight)), None);
        fs::write(line_src.join("second.txt"), "more\n").unwrap();
        let refusal = copy_refusal(&line_src, Some(tight));
        assert!(refusal.as_deref().is_some_and(|s| s.contains("too big to give each model its own copy")));

        let _ = fs::remove_dir_all(&folder);
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&line_src);
        let _ = fs::remove_dir_all(&line_root);
    }
}

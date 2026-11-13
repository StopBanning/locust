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

pub fn copy_line_changes(
    folder: &Path,
    compare_id: &str,
    slot: CompareSlotId,
    root: &Path,
) -> Result<(usize, Option<usize>, Option<usize>), String> {
    let name = safe_name(compare_id, slot)?;
    let target = root.join(name);
    let CopyChanges { changed, deleted } = copy_changes(compare_id, slot, root)?;
    let mut added = 0usize;
    let mut removed = 0usize;
    let mut counted = false;

    let mut count = |from: &str, to: &str| {
        if let Ok((_, stdout, _)) = run_git_output(
            &["diff", "--no-index", "--numstat", "--no-color", "--no-ext-diff", "--", from, to],
            root,
        ) {
            let printed = String::from_utf8_lossy(&stdout);
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

    for path in &changed {
        let mine = in_folder(folder, path)?;
        let from_str = if mine.exists() {
            mine.to_string_lossy().into_owned()
        } else {
            EMPTY_FILE.to_owned()
        };
        let to = in_folder(&target, path)?;
        count(&from_str, &to.to_string_lossy());
    }
    for path in &deleted {
        let mine = in_folder(folder, path)?;
        count(&mine.to_string_lossy(), EMPTY_FILE);
    }

    let files = changed.len() + deleted.len();
    if counted {
        Ok((files, Some(added), Some(removed)))
    } else {
        Ok((files, None, None))
    }
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
    for &slot in slots {
        if let Ok(name) = safe_name(compare_id, slot) {
            let target = root.join(name);
            let target_str = target.to_string_lossy().into_owned();
            let _ = run_git_text(&["worktree", "remove", "--force", &target_str], folder);
        }
    }
    let _ = run_git_text(&["worktree", "prune"], folder);
}

pub fn bring_in_compare_worktree(
    folder: &Path,
    compare_id: &str,
    slot: CompareSlotId,
    root: &Path,
    base: Option<&str>,
) -> Result<BringIn, String> {
    let name = safe_name(compare_id, slot)?;
    let target = root.join(name);
    let base_ref = base.ok_or_else(|| "The comparison's starting point is missing.".to_string())?;
    if !is_commit_sha(base_ref) {
        return Err("The comparison's starting point is missing.".into());
    }
    let flags = ["--no-color", "--no-ext-diff", "--no-textconv", "--no-renames", "--no-relative"];
    let mut changed_args = vec!["diff"];
    changed_args.extend_from_slice(&flags);
    changed_args.extend_from_slice(&["--name-only", "-z", base_ref, "--"]);
    let (_, out, _) = run_git_output(&changed_args, &target)?;
    let mut touched: Vec<String> = out
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect();
    let (_, untracked, _) = run_git_output(&["ls-files", "--others", "--exclude-standard", "-z"], &target)?;
    for entry in untracked.split(|&b| b == 0).filter(|s| !s.is_empty()) {
        let p = String::from_utf8_lossy(entry).into_owned();
        if !touched.contains(&p) {
            touched.push(p);
        }
    }
    if touched.is_empty() {
        return Ok(BringIn::Nothing);
    }

    // Check if the person has local uncommitted changes in `folder` touching any of `touched`.
    let (_, dirty_out, _) = run_git_output(&["status", "--porcelain", "-z"], folder)?;
    let dirty: Vec<String> = dirty_out
        .split(|&b| b == 0)
        .filter(|s| s.len() > 3)
        .map(|s| String::from_utf8_lossy(&s[3..]).into_owned())
        .collect();
    let conflicts: Vec<String> = touched
        .iter()
        .filter(|p| dirty.contains(p))
        .cloned()
        .collect();
    if !conflicts.is_empty() {
        return Ok(BringIn::YourChanges(conflicts));
    }

    for path in &touched {
        let src = local_file(&target, path)?;
        let dst = in_folder(folder, path)?;
        if src.exists() {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(&src, &dst).map_err(|e| e.to_string())?;
        } else {
            let _ = fs::remove_file(&dst);
        }
    }
    touched.sort();
    Ok(BringIn::Brought(touched))
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
}

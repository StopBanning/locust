//! Read-only workspace listing for @ and calls, following main/workspace-files.ts.
use std::{fs, path::Path, process::Command};
pub const MAX: usize = 50_000;
const SKIP: &[&str] = &[
    ".git",
    "node_modules",
    "dist",
    "out",
    "build",
    "release",
    ".next",
    "target",
    "__pycache__",
    ".venv",
    "venv",
    ".locust",
    ".cache",
    "coverage",
];

/// What git listed, as the box offers it: forward slashes, never a path that climbs out, a whole path, a drive path or
/// Locust's own folder, and at most `MAX` (said as truncated).
pub fn from_git<'a>(listed: impl Iterator<Item = &'a str>) -> (Vec<String>, bool) {
    let mut paths: Vec<_> = listed
        .map(|p| p.replace('\\', "/"))
        .filter(|p| {
            !p.is_empty()
                && !p.starts_with('/')
                && !p.split('/').any(|v| v == ".." || v == ".locust")
                && !(p.as_bytes().get(1) == Some(&b':'))
        })
        .collect();
    let truncated = paths.len() > MAX;
    paths.truncate(MAX);
    (paths, truncated)
}

pub fn list(root: &Path) -> (Vec<String>, bool) {
    if let Ok(output) = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(root)
        .output()
        && output.status.success()
    {
        return from_git(String::from_utf8_lossy(&output.stdout).split('\0'));
    }
    list_without_git(root)
}

/// The read-only walk used when no external command should run.
pub fn list_without_git(root: &Path) -> (Vec<String>, bool) {
    fn walk(
        root: &Path,
        relative: &str,
        depth: usize,
        paths: &mut Vec<String>,
        truncated: &mut bool,
    ) {
        if depth > 8 || *truncated {
            return;
        }
        let Ok(entries) = fs::read_dir(root.join(relative)) else {
            return;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = if relative.is_empty() {
                name.clone()
            } else {
                format!("{relative}/{name}")
            };
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() && !SKIP.contains(&name.as_str()) {
                walk(root, &path, depth + 1, paths, truncated);
            } else if kind.is_file() {
                if paths.len() == MAX {
                    *truncated = true;
                    return;
                }
                paths.push(path);
            }
            if *truncated {
                return;
            }
        }
    }
    let mut paths = vec![];
    let mut truncated = false;
    walk(root, "", 0, &mut paths, &mut truncated);
    (paths, truncated)
}

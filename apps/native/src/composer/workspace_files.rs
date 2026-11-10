//! Read-only workspace listing for @, following main/workspace-files.ts.
use std::{fs, path::Path, process::Command};
const MAX: usize = 50_000;
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
        let mut paths: Vec<_> = String::from_utf8_lossy(&output.stdout)
            .split('\0')
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
        return (paths, truncated);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn walk_excludes_builds_and_symlinks() {
        let root = std::env::temp_dir().join(format!("locust-file-list-{}", std::process::id()));
        fs::create_dir_all(root.join("node_modules")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("README.md"), "").unwrap();
        fs::write(root.join("node_modules/no"), "").unwrap();
        fs::write(root.join("src/main.rs"), "").unwrap();
        let (paths, partial) = list(&root);
        assert!(!partial);
        assert_eq!(paths, vec!["README.md", "src/main.rs"]);
        fs::remove_dir_all(root).unwrap();
    }
}

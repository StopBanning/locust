//! A message that names its project starts there — port of `main/named-folder.ts`.

use std::path::{Path, PathBuf};

const MOST_PATHS: usize = 12;
const MOST_LEVELS: usize = 64;

pub trait NamedFolderFiles {
    fn exists(&self, path: &Path) -> bool;
    fn is_directory(&self, path: &Path) -> bool;
    fn read(&self, path: &Path) -> Result<String, String>;
}

pub struct RealFiles;

impl NamedFolderFiles for RealFiles {
    fn exists(&self, path: &Path) -> bool {
        path.exists()
    }
    fn is_directory(&self, path: &Path) -> bool {
        path.is_dir()
    }
    fn read(&self, path: &Path) -> Result<String, String> {
        std::fs::read_to_string(path).map_err(|e| e.to_string())
    }
}

fn push_path(found: &mut Vec<String>, raw: &str) {
    let path =
        raw.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']', '}']);
    if path.len() > 3 && !found.iter().any(|p| p == path) {
        found.push(path.to_owned());
    }
}

fn bare_ok_before(text: &str, at: usize, windows: bool) -> bool {
    if at == 0 {
        return true;
    }
    let Some(prev) = text[..at].chars().next_back() else {
        return true;
    };
    if windows {
        !prev.is_ascii_alphanumeric() && prev != '_' && prev != '"' && prev != '\'' && prev != '`'
    } else {
        !matches!(
            prev,
            'a'..='z'
                | 'A'..='Z'
                | '0'..='9'
                | '_'
                | ':'
                | '.'
                | '/'
                | '~'
                | '"'
                | '\''
                | '`'
        )
    }
}

/// Absolute paths a message names (quoted first, then bare).
/// Lookbehind from the TypeScript source is done by hand — the `regex` crate has none.
pub fn paths_named_in(text: &str, windows: bool) -> Vec<String> {
    let mut found = Vec::new();
    if windows {
        let quoted = regex::Regex::new(r#"["'`]([A-Za-z]:[\\/][^"'`\r\n]*)["'`]"#).expect("re");
        for m in quoted.captures_iter(text) {
            push_path(&mut found, m.get(1).map(|m| m.as_str()).unwrap_or(""));
            if found.len() == MOST_PATHS {
                return found;
            }
        }
        let bare = regex::Regex::new(r#"([A-Za-z]:[\\/][^\s"'`<>|*?]*)"#).expect("re");
        for m in bare.find_iter(text) {
            if !bare_ok_before(text, m.start(), true) {
                continue;
            }
            push_path(&mut found, m.as_str());
            if found.len() == MOST_PATHS {
                return found;
            }
        }
    } else {
        let quoted = regex::Regex::new(r#"["'`](/[^"'`\r\n]+)["'`]"#).expect("re");
        for m in quoted.captures_iter(text) {
            push_path(&mut found, m.get(1).map(|m| m.as_str()).unwrap_or(""));
            if found.len() == MOST_PATHS {
                return found;
            }
        }
        let bare = regex::Regex::new(r#"(/[^\s"'`<>|*?]+)"#).expect("re");
        for m in bare.find_iter(text) {
            if !bare_ok_before(text, m.start(), false) {
                continue;
            }
            push_path(&mut found, m.as_str());
            if found.len() == MOST_PATHS {
                return found;
            }
        }
    }
    found
}

pub fn checkout_of(path: &Path, files: &dyn NamedFolderFiles) -> Option<PathBuf> {
    if !files.exists(path) {
        return None;
    }
    let mut folder = if files.is_directory(path) {
        path.to_path_buf()
    } else {
        path.parent()?.to_path_buf()
    };
    for _ in 0..MOST_LEVELS {
        if files.exists(&folder.join(".git")) {
            return Some(folder);
        }
        let up = folder.parent()?.to_path_buf();
        if up == folder {
            return None;
        }
        folder = up;
    }
    None
}

pub fn repository_of(checkout: &Path, files: &dyn NamedFolderFiles) -> Option<PathBuf> {
    let dot_git = checkout.join(".git");
    if files.is_directory(&dot_git) {
        return Some(dot_git);
    }
    let text = files.read(&dot_git).ok()?;
    let named = text.lines().find_map(|line| {
        line.strip_prefix("gitdir:")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    })?;
    let git_dir = checkout.join(named);
    let as_str = git_dir.to_string_lossy();
    if regex::Regex::new(r#"[\\/]worktrees[\\/][^\\/]+[\\/]?$"#)
        .ok()?
        .is_match(&as_str)
    {
        git_dir.parent()?.parent().map(|p| p.to_path_buf())
    } else {
        Some(git_dir)
    }
}

pub struct NamedFolderContext<'a> {
    pub current: &'a Path,
    pub is_no_project: &'a dyn Fn(&Path) -> bool,
    pub same: &'a dyn Fn(&Path, &Path) -> bool,
    pub windows: bool,
    pub files: &'a dyn NamedFolderFiles,
}

pub fn project_named_in(text: &str, context: &NamedFolderContext<'_>) -> Option<PathBuf> {
    let nowhere = (context.is_no_project)(context.current);
    let here = if nowhere {
        None
    } else {
        checkout_of(context.current, context.files)
    };
    if !nowhere && here.is_none() {
        return None;
    }
    let ours = here
        .as_ref()
        .and_then(|h| repository_of(h, context.files));
    for path in paths_named_in(text, context.windows) {
        let Some(project) = checkout_of(Path::new(&path), context.files) else {
            continue;
        };
        if (context.is_no_project)(&project) {
            continue;
        }
        if (context.same)(&project, context.current) {
            continue;
        }
        if let Some(here) = &here
            && (context.same)(&project, here)
        {
            continue;
        }
        if nowhere {
            return Some(project);
        }
        let theirs = repository_of(&project, context.files);
        if let (Some(ours), Some(theirs)) = (ours.as_ref(), theirs.as_ref())
            && (context.same)(ours, theirs)
        {
            return Some(project);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MapFiles {
        dirs: HashMap<String, bool>,
        files: HashMap<String, String>,
    }

    impl NamedFolderFiles for MapFiles {
        fn exists(&self, path: &Path) -> bool {
            let key = path.to_string_lossy().into_owned();
            self.dirs.contains_key(&key) || self.files.contains_key(&key)
        }
        fn is_directory(&self, path: &Path) -> bool {
            self.dirs
                .get(&path.to_string_lossy().into_owned())
                .copied()
                .unwrap_or(false)
        }
        fn read(&self, path: &Path) -> Result<String, String> {
            self.files
                .get(&path.to_string_lossy().into_owned())
                .cloned()
                .ok_or_else(|| "missing".into())
        }
    }

    fn map_files() -> MapFiles {
        let mut dirs = HashMap::new();
        let mut files = HashMap::new();
        for p in [
            "/Users/colin/work/app",
            "/Users/colin/work/app/src",
            "/Users/colin/work/other",
            "/Users/colin",
            "/tmp",
        ] {
            dirs.insert(p.into(), true);
        }
        dirs.insert("/Users/colin/work/app/.git".into(), true);
        dirs.insert("/Users/colin/work/other/.git".into(), true);
        files.insert(
            "/Users/colin/work/app/src/main.rs".into(),
            "fn main() {}".into(),
        );
        MapFiles { dirs, files }
    }

    #[test]
    fn paths_named_in_quoted_and_bare() {
        let text = "look at `/Users/colin/work/app/src/main.rs` and /Users/colin/work/other/README.md.";
        let paths = paths_named_in(text, false);
        assert_eq!(
            paths,
            vec![
                "/Users/colin/work/app/src/main.rs",
                "/Users/colin/work/other/README.md"
            ]
        );
    }

    #[test]
    fn checkout_and_project_from_no_project_window() {
        let files = map_files();
        assert_eq!(
            checkout_of(Path::new("/Users/colin/work/app/src/main.rs"), &files),
            Some(PathBuf::from("/Users/colin/work/app"))
        );
        let is_no = |p: &Path| p == Path::new("/Users/colin") || p == Path::new("/tmp");
        let same = |a: &Path, b: &Path| a == b;
        let ctx = NamedFolderContext {
            current: Path::new("/Users/colin"),
            is_no_project: &is_no,
            same: &same,
            windows: false,
            files: &files,
        };
        assert_eq!(
            project_named_in(
                "please open `/Users/colin/work/app/src/main.rs`",
                &ctx
            ),
            Some(PathBuf::from("/Users/colin/work/app"))
        );
        let ctx_in_project = NamedFolderContext {
            current: Path::new("/Users/colin/work/app"),
            is_no_project: &is_no,
            same: &same,
            windows: false,
            files: &files,
        };
        assert!(
            project_named_in(
                "see `/Users/colin/work/other/README.md`",
                &ctx_in_project
            )
            .is_none()
        );
    }

    #[test]
    fn worktree_repository_resolves_shared_git() {
        let mut files = map_files();
        files.dirs.insert("/Users/colin/work/app-wt".into(), true);
        files.files.insert(
            "/Users/colin/work/app-wt/.git".into(),
            "gitdir: /Users/colin/work/app/.git/worktrees/app-wt\n".into(),
        );
        files
            .dirs
            .insert("/Users/colin/work/app/.git/worktrees/app-wt".into(), true);
        assert_eq!(
            repository_of(Path::new("/Users/colin/work/app-wt"), &files),
            Some(PathBuf::from("/Users/colin/work/app/.git"))
        );
        let _ = RealFiles;
    }
}

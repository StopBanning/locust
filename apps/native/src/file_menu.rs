//! File viewer context-menu / reveal — port of `shared/file-menu.ts` and `main/reveal-file.ts`.

use std::path::{Component, Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileMenuId {
    Attach,
    Vscode,
    Cursor,
    Default,
    Reveal,
    CopyPath,
    CopyRelative,
    CopyName,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMenuChoice {
    pub id: FileMenuId,
    pub label: &'static str,
}

const VIEWABLE: &[&str] = &[
    "txt", "md", "markdown", "log", "csv", "tsv", "json", "yaml", "yml", "pdf", "rtf", "docx",
    "xlsx", "pptx", "odt", "ods", "odp", "html", "htm", "png", "jpg", "jpeg", "gif", "webp", "bmp",
    "ico", "svg", "avif", "mp3", "wav", "ogg", "flac", "m4a", "mp4", "mov", "webm", "mkv",
];

pub fn file_base_name(path: &str) -> String {
    let base = path.replace('\\', "/");
    let name = base.rsplit('/').next().unwrap_or(path);
    if name.is_empty() {
        path.to_owned()
    } else {
        name.to_owned()
    }
}

pub fn file_extension(path: &str) -> String {
    let base = file_base_name(path);
    match base.rfind('.') {
        Some(dot) if dot > 0 => base[dot + 1..].to_ascii_lowercase(),
        _ => String::new(),
    }
}

pub fn default_app_may_open(path: &str) -> bool {
    VIEWABLE.iter().any(|e| *e == file_extension(path))
}

fn split_path(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in value.replace('\\', "/").split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            out.pop();
            continue;
        }
        out.push(part.to_owned());
    }
    out
}

pub fn rooted_path(path: &str, folder: Option<&str>) -> String {
    let slash = path.trim().replace('\\', "/");
    let Some(folder) = folder.filter(|f| !f.trim().is_empty()) else {
        return path.to_owned();
    };
    if slash.starts_with('/') || (slash.len() >= 3 && slash.as_bytes()[1] == b':' && slash.as_bytes()[2] == b'/')
    {
        return path.to_owned();
    }
    let base = folder.trim().trim_end_matches(['/', '\\']);
    format!("{base}/{}", slash.trim_start_matches("./"))
}

pub fn path_inside_folder(candidate: &str, folder: Option<&str>, platform: &str) -> bool {
    let Some(folder) = folder.filter(|f| !f.trim().is_empty()) else {
        return false;
    };
    if candidate.trim().is_empty() {
        return false;
    }
    let fold = |part: &str| {
        if platform == "win32" {
            part.to_ascii_lowercase()
        } else {
            part.to_owned()
        }
    };
    let inner = split_path(candidate);
    let outer = split_path(folder);
    if outer.is_empty() || inner.len() < outer.len() {
        return false;
    }
    outer
        .iter()
        .enumerate()
        .all(|(i, part)| fold(inner.get(i).map(String::as_str).unwrap_or("")) == fold(part))
}

pub fn system_path(path: &str, platform: &str) -> String {
    if platform == "win32" {
        path.replace('/', "\\")
    } else {
        path.to_owned()
    }
}

pub fn relative_to_folder(candidate: &str, folder: Option<&str>, platform: &str) -> String {
    if folder.is_none() || !path_inside_folder(candidate, folder, platform) {
        return candidate.to_owned();
    }
    let rest = split_path(candidate)
        .into_iter()
        .skip(split_path(folder.unwrap_or("")).len())
        .collect::<Vec<_>>()
        .join("/");
    if rest.is_empty() {
        file_base_name(candidate)
    } else {
        rest
    }
}

pub fn looks_like_file_path(text: &str) -> bool {
    let value = text.trim();
    if value.is_empty() || value.len() > 400 || value.contains(char::is_whitespace) || value.contains("://")
    {
        return false;
    }
    value.contains('/') || value.contains('\\')
}

pub fn file_menu_choices(
    path: &str,
    folder: Option<&str>,
    platform: &str,
    editors: &[&str],
    in_browser: bool,
) -> Vec<FileMenuChoice> {
    let copies = vec![
        FileMenuChoice {
            id: FileMenuId::CopyPath,
            label: "Copy path",
        },
        FileMenuChoice {
            id: FileMenuId::CopyRelative,
            label: "Copy relative path",
        },
        FileMenuChoice {
            id: FileMenuId::CopyName,
            label: "Copy file name",
        },
    ];
    if !path_inside_folder(path, folder, platform) {
        return copies;
    }
    let mut open = vec![FileMenuChoice {
        id: FileMenuId::Attach,
        label: "Add to my next message",
    }];
    if editors.contains(&"vscode") {
        open.push(FileMenuChoice {
            id: FileMenuId::Vscode,
            label: "Open in VS Code",
        });
    }
    if editors.contains(&"cursor") {
        open.push(FileMenuChoice {
            id: FileMenuId::Cursor,
            label: "Open in Cursor",
        });
    }
    if in_browser {
        open.extend(copies);
        return open;
    }
    if default_app_may_open(path) {
        open.push(FileMenuChoice {
            id: FileMenuId::Default,
            label: "Open in the default app",
        });
    }
    let reveal = match platform {
        "darwin" => "Show in Finder",
        "linux" => "Show in the file manager",
        _ => "Show in Explorer",
    };
    open.push(FileMenuChoice {
        id: FileMenuId::Reveal,
        label: reveal,
    });
    open.extend(copies);
    open
}

/// Path arithmetic containment (`reveal-file.ts` `contains`).
pub fn contains(parent: &Path, child: &Path) -> bool {
    let from = normalize(parent);
    let to = normalize(child);
    if from == to {
        return true;
    }
    to.starts_with(&from) && to.components().count() > from.components().count()
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RevealDecision {
    Ok { path: PathBuf, named: bool },
    NoPath,
    Outside { path: String },
}

/// Decide whether a reveal request may be honoured (`decideReveal`).
pub fn decide_reveal(requested: &str, roots: &[PathBuf]) -> RevealDecision {
    let requested = requested.trim();
    if requested.is_empty() {
        return RevealDecision::NoPath;
    }
    let path = PathBuf::from(requested);
    if !path.is_absolute() {
        return RevealDecision::Outside {
            path: requested.into(),
        };
    }
    if roots.iter().any(|root| contains(root, &path) || root == &path) {
        RevealDecision::Ok {
            path,
            named: false,
        }
    } else {
        RevealDecision::Outside {
            path: requested.into(),
        }
    }
}

/// Reveal in Finder / file manager. Never opens/runs the file (`reveal-file.ts` rule).
pub fn reveal_path(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let mut cmd = std::process::Command::new("/usr/bin/open");
        cmd.args(["-R", &path.to_string_lossy()]);
        crate::runtime::process::own_group(&mut cmd);
        let status = cmd.status().map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("Locust could not show that file.".into())
        }
    }
    #[cfg(target_os = "linux")]
    {
        let parent = path.parent().unwrap_or(path);
        let mut cmd = std::process::Command::new("xdg-open");
        cmd.arg(parent);
        crate::runtime::process::own_group(&mut cmd);
        let status = cmd.status().map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("Locust could not show that file.".into())
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = path;
        Err("Locust could not show that file.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_words_match_electron() {
        let inside = file_menu_choices(
            "/work/app/src/a.ts",
            Some("/work/app"),
            "darwin",
            &["vscode", "cursor"],
            false,
        );
        let labels: Vec<_> = inside.iter().map(|c| c.label).collect();
        assert!(labels.contains(&"Add to my next message"));
        assert!(labels.contains(&"Open in VS Code"));
        assert!(labels.contains(&"Open in Cursor"));
        assert!(labels.contains(&"Show in Finder"));
        assert!(labels.contains(&"Copy path"));
        assert!(!labels.iter().any(|l| *l == "Open in the default app")); // .ts not viewable

        let pdf = file_menu_choices(
            "/work/app/doc.pdf",
            Some("/work/app"),
            "darwin",
            &[],
            false,
        );
        assert!(pdf.iter().any(|c| c.label == "Open in the default app"));

        let outside = file_menu_choices("/etc/passwd", Some("/work/app"), "darwin", &["vscode"], false);
        assert_eq!(outside.len(), 3);
        assert!(outside.iter().all(|c| matches!(
            c.id,
            FileMenuId::CopyPath | FileMenuId::CopyRelative | FileMenuId::CopyName
        )));
    }

    #[test]
    fn containment_and_reveal() {
        assert!(contains(Path::new("/work/shop"), Path::new("/work/shop/a.ts")));
        assert!(!contains(
            Path::new("/work/shop"),
            Path::new("/work/shop-secrets/a.ts")
        ));
        assert!(matches!(
            decide_reveal("", &[PathBuf::from("/work")]),
            RevealDecision::NoPath
        ));
        assert!(matches!(
            decide_reveal("/work/a.ts", &[PathBuf::from("/work")]),
            RevealDecision::Ok { .. }
        ));
        assert!(matches!(
            decide_reveal("/etc/passwd", &[PathBuf::from("/work")]),
            RevealDecision::Outside { .. }
        ));
    }

    #[test]
    fn relative_and_system_paths() {
        assert_eq!(
            relative_to_folder("/work/app/src/a.ts", Some("/work/app"), "darwin"),
            "src/a.ts"
        );
        assert_eq!(system_path(r"C:/work/a.ts", "win32"), r"C:\work\a.ts");
        assert!(looks_like_file_path("src/main.rs"));
        assert!(!looks_like_file_path("hello world"));
    }
}

//! Which folder the teammates work in at startup (Electron `main/workspace.ts`).
//!
//! Resolves `--workspace=<abs>`, the launch folder (excluding the install folder, home, agent settings folders, and
//! Windows' own folder), the remembered `workspace.json` folder, or the default `~/Documents/Locust` folder.

use crate::thread_view::folder_caution;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const WORKSPACE_ARGUMENT: &str = "--workspace=";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceSource {
    Argument,
    LaunchFolder,
    Remembered,
    Default,
    None,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceResolution {
    pub path: Option<String>,
    pub source: WorkspaceSource,
}

pub struct ResolveOptions<'a> {
    pub argv: &'a [String],
    pub cwd: &'a str,
    pub install_directory: Option<&'a str>,
    pub remembered: Option<&'a str>,
    pub default_workspace: Option<&'a str>,
    pub win32: bool,
    pub home_directory: Option<&'a str>,
    pub system_directory: Option<&'a str>,
}

/// The id a mission records for the folder it ran in (`ws_` + 32 hex chars of SHA-256).
pub fn workspace_id_for(workspace_path: &str) -> String {
    let digest = Sha256::digest(workspace_path.as_bytes());
    let hex: String = digest.iter().take(16).map(|b| format!("{b:02x}")).collect();
    format!("ws_{hex}")
}

fn is_absolute_for_platform(path: &str, win32: bool) -> bool {
    if win32 {
        let b = path.as_bytes();
        path.starts_with("\\\\")
            || path.starts_with("//")
            || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/'))
    } else {
        path.starts_with('/')
    }
}

fn resolve_lexical(path: &str, win32: bool) -> String {
    if win32 {
        let normalized = path.replace('/', "\\");
        let (prefix, rest) = if normalized.as_bytes().get(1) == Some(&b':') {
            (&normalized[..2], &normalized[2..])
        } else {
            ("", normalized.as_str())
        };
        let mut parts: Vec<&str> = Vec::new();
        for part in rest.split('\\') {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                _ => parts.push(part),
            }
        }
        if !prefix.is_empty() {
            if parts.is_empty() {
                format!("{prefix}\\")
            } else {
                format!("{prefix}\\{}", parts.join("\\"))
            }
        } else if normalized.starts_with("\\\\") {
            format!("\\\\{}", parts.join("\\"))
        } else {
            parts.join("\\")
        }
    } else {
        let mut parts: Vec<&str> = Vec::new();
        for part in path.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                _ => parts.push(part),
            }
        }
        if path.starts_with('/') {
            format!("/{}", parts.join("/"))
        } else {
            parts.join("/")
        }
    }
}

/// Whether `candidate` is `directory` or somewhere under it. Case-blind on Windows.
pub fn is_inside_directory(candidate: &str, directory: Option<&str>, win32: bool) -> bool {
    let Some(directory) = directory else {
        return false;
    };
    let sep = if win32 { '\\' } else { '/' };
    let fold = |value: &str| {
        let resolved = resolve_lexical(value, win32);
        let trimmed = resolved.trim_end_matches(['\\', '/']).to_owned();
        if win32 {
            trimmed.to_lowercase()
        } else {
            trimmed
        }
    };
    let inner = fold(candidate);
    let outer = fold(directory);
    inner == outer || inner.starts_with(&format!("{outer}{sep}"))
}

/// Why a folder is no place for teammates to work, or `None` when it is fine.
pub fn not_a_teammate_folder(
    candidate: &str,
    home_directory: Option<&str>,
    win32: bool,
    system_directory: Option<&str>,
) -> Option<String> {
    folder_caution(candidate, home_directory, win32, system_directory)
}

/// Which folder the teammates work in, and how the app came to know it.
pub fn resolve_workspace_path(options: &ResolveOptions<'_>) -> WorkspaceResolution {
    if let Some(arg) = options
        .argv
        .iter()
        .find_map(|entry| entry.strip_prefix(WORKSPACE_ARGUMENT))
        && !arg.is_empty()
        && is_absolute_for_platform(arg, options.win32)
    {
        return WorkspaceResolution {
            path: Some(resolve_lexical(arg, options.win32)),
            source: WorkspaceSource::Argument,
        };
    }

    if !is_inside_directory(options.cwd, options.install_directory, options.win32)
        && not_a_teammate_folder(
            options.cwd,
            options.home_directory,
            options.win32,
            options.system_directory,
        )
        .is_none()
    {
        return WorkspaceResolution {
            path: Some(resolve_lexical(options.cwd, options.win32)),
            source: WorkspaceSource::LaunchFolder,
        };
    }

    if let Some(remembered) = options.remembered
        && is_absolute_for_platform(remembered, options.win32)
        && !is_inside_directory(remembered, options.install_directory, options.win32)
    {
        return WorkspaceResolution {
            path: Some(resolve_lexical(remembered, options.win32)),
            source: WorkspaceSource::Remembered,
        };
    }

    if let Some(default_ws) = options.default_workspace
        && is_absolute_for_platform(default_ws, options.win32)
        && !is_inside_directory(default_ws, options.install_directory, options.win32)
    {
        return WorkspaceResolution {
            path: Some(resolve_lexical(default_ws, options.win32)),
            source: WorkspaceSource::Default,
        };
    }

    WorkspaceResolution {
        path: None,
        source: WorkspaceSource::None,
    }
}

/// The folder chosen last time, or `None` when there is none or the file is unreadable.
pub fn read_remembered_workspace(file: &Path) -> Option<String> {
    let text = std::fs::read_to_string(file).ok()?;
    let parsed: Value = serde_json::from_str(&text).ok()?;
    let path = parsed.get("path")?.as_str()?;
    (!path.is_empty()).then(|| path.to_owned())
}

pub fn write_remembered_workspace(file: &Path, path: &str) -> Result<(), String> {
    let record = json!({
        "schemaVersion": 1,
        "path": path,
    });
    let json = serde_json::to_string_pretty(&record)
        .map_err(|e| format!("Could not serialize workspace.json: {e}"))?;
    std::fs::write(file, format!("{json}\n"))
        .map_err(|e| format!("Could not write workspace.json: {e}"))
}

/// Resolve the workspace folder for the current process at startup.
/// The folder the running program sits in (Electron's `dirname(app.getPath('exe'))`): what a teammate's folder may
/// never be inside. None when the program's path is unknown.
pub fn startup_install_directory(exe: Option<&Path>) -> Option<String> {
    let dir = exe?.parent()?;
    (dir.parent().is_some()).then(|| dir.to_string_lossy().into_owned())
}

pub fn resolve_startup_workspace(remembered: Option<&str>) -> Option<String> {
    let argv: Vec<String> = std::env::args().collect();
    let cwd = std::env::current_dir()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok();
    let default_ws = home
        .as_deref()
        .map(|h| Path::new(h).join("Documents").join("Locust").to_string_lossy().into_owned());
    let system_dir = std::env::var("SystemRoot").ok();
    // index.ts: the install folder is the one the app's own program sits in (`dirname(app.getPath('exe'))`). A bundled
    // .app launched from Finder starts in "/", which folder_caution already refuses as a launch folder; "/" must never
    // be the install folder, or every remembered folder sits inside it and is thrown away (live hour, 26 Nov).
    let install_dir = startup_install_directory(std::env::current_exe().ok().as_deref());
    resolve_workspace_path(&ResolveOptions {
        argv: &argv,
        cwd: &cwd,
        install_directory: install_dir.as_deref(),
        remembered,
        default_workspace: default_ws.as_deref(),
        win32: cfg!(windows),
        home_directory: home.as_deref(),
        system_directory: system_dir.as_deref(),
    })
    .path
}

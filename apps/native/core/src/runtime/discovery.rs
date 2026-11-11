//! path-locator.ts: PATH takes precedence; relative and empty entries are ignored.
use std::{
    io,
    path::{Path, PathBuf},
};
fn executable(path: &Path) -> bool {
    let Ok(meta) = path.metadata() else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}
pub fn find() -> io::Result<PathBuf> {
    find_named("codex")
}
pub fn find_named(name: &str) -> io::Result<PathBuf> {
    if !name
        .bytes()
        .next()
        .is_some_and(|b| b.is_ascii_alphanumeric())
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err(io::Error::other("Invalid executable command name"));
    }
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    };
    let paths = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&paths) {
        if !dir.is_absolute() {
            continue;
        }
        let path = dir.join(&file);
        if executable(&path) {
            return Ok(path);
        }
    }
    let label = match name {
        "claude" => "Claude Code",
        "codex" => "Codex",
        other => other,
    };
    Err(io::Error::other(format!(
        "{label} was not found on PATH. Install the official CLI and log in using the CLI itself."
    )))
}

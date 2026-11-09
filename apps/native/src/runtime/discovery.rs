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
        return meta.permissions().mode() & 0o111 != 0;
    }
    #[cfg(not(unix))]
    {
        true
    }
}
pub fn find() -> io::Result<PathBuf> {
    let paths = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&paths) {
        if !dir.is_absolute() {
            continue;
        }
        let path = dir.join(if cfg!(windows) { "codex.exe" } else { "codex" });
        if executable(&path) {
            return Ok(path);
        }
    }
    Err(io::Error::other(
        "Codex was not found on PATH. Install the official Codex CLI and log in using Codex itself.",
    ))
}

//! path-locator.ts: PATH takes precedence; relative and empty entries are ignored. Then npm's own bin directory, then
//! (on Windows) the per-user install roots a runtime's own installer creates. On Windows a `.cmd` or `.ps1` shim is
//! found too, and an npm shim is resolved past cmd.exe to what it wraps.
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
    if !safe_name(name) {
        return Err(io::Error::other("Invalid executable command name"));
    }
    find_beyond_path(name, &std::env::var_os("PATH").unwrap_or_default(), crate::shell_path::npm_bin_directory)
}

/// path-locator.ts `SAFE_COMMAND_NAME`.
fn safe_name(name: &str) -> bool {
    name.bytes().next().is_some_and(|b| b.is_ascii_alphanumeric())
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

/// PATH first, then npm's own bin directory (path-locator.ts: `npmBinDirectory`, read only when PATH has missed), so a
/// CLI installed under a moved npm prefix is found, then this machine's install roots. The error is PATH's.
pub(crate) fn find_beyond_path(name: &str, paths: &std::ffi::OsStr, npm_bin: impl FnOnce() -> Option<String>) -> io::Result<PathBuf> {
    let path = paths.to_string_lossy().into_owned();
    let npm_bin = std::cell::Cell::new(Some(npm_bin));
    let env = |key: &str| if key.eq_ignore_ascii_case("PATH") { Some(path.clone()) } else { std::env::var(key).ok() };
    let is_executable = |candidate: &str| executable(Path::new(candidate));
    let read_file = |candidate: &str| std::fs::read_to_string(candidate).ok();
    let npm = || npm_bin.take().and_then(|f| f());
    let look = Look {
        windows: cfg!(windows),
        env: &env,
        is_executable: &is_executable,
        read_dir: &versions_in,
        read_file: &read_file,
        npm_bin: &npm,
    };
    locate(name, &look).and_then(|launch| launch.program().map(PathBuf::from)).ok_or_else(|| not_found(name))
}

/// The directories inside `directory`, each with when it was last written (path-locator.ts `defaultReadDirectory` and
/// the `stat` beside it: an unreadable one is the oldest).
fn versions_in(directory: &str) -> Vec<(String, u128)> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| {
            let at = e
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            (e.file_name().to_string_lossy().into_owned(), at)
        })
        .collect()
}

/// `find_named` over a given PATH (its name already checked), for a test to hand it one: the first absolute entry holding `name` as an
/// executable file; relative and empty entries are never searched.
pub(crate) fn find_in(name: &str, paths: &std::ffi::OsStr) -> io::Result<PathBuf> {
    find_beyond_path(name, paths, || None)
}

fn not_found(name: &str) -> io::Error {
    let label = match name {
        "claude" => "Claude Code",
        "codex" => "Codex",
        other => other,
    };
    io::Error::other(format!("{label} was not found on PATH. Install the official CLI and log in using the CLI itself."))
}

/// How a found command is started (types.ts `ExecutableLaunch['kind']`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchKind {
    Native,
    NodeShim,
    CmdShim,
    PowershellShim,
}

/// types.ts `ExecutableLaunch`: what was found (the path a person would look for) and what actually runs, with the
/// arguments that go in front of the runtime's own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub discovered: String,
    pub executable: String,
    pub prefix_args: Vec<String>,
    pub kind: LaunchKind,
}

impl Launch {
    /// The one file a bare `Command::new` can start for this launch, with no arguments in front: the program itself
    /// when it is native (an npm shim around a binary included), the `.cmd` for a shim (Windows starts a `.cmd`
    /// through cmd.exe on its own), and nothing for a PowerShell script, which needs a host in front of it.
    pub fn program(&self) -> Option<&str> {
        match self.kind {
            LaunchKind::Native => Some(&self.executable),
            LaunchKind::NodeShim | LaunchKind::CmdShim => Some(&self.discovered),
            LaunchKind::PowershellShim => None,
        }
    }
}

/// What the locator may read (path-locator.ts `PathExecutableLocatorOptions`): this machine's, or a test's.
pub struct Look<'a> {
    pub windows: bool,
    /// An environment variable; on Windows a test's lookup must ignore case, as Windows does.
    pub env: &'a dyn Fn(&str) -> Option<String>,
    pub is_executable: &'a dyn Fn(&str) -> bool,
    /// The directories in a folder, each with when it was written.
    pub read_dir: &'a dyn Fn(&str) -> Vec<(String, u128)>,
    /// A `.cmd` shim's text; None for anything that cannot be read.
    pub read_file: &'a dyn Fn(&str) -> Option<String>,
    /// npm's real bin directory, asked only when PATH has missed.
    pub npm_bin: &'a dyn Fn() -> Option<String>,
}

/// Where a CLI installs itself when it does not put itself on PATH (path-locator.ts `WINDOWS_INSTALL_ROOTS`): the
/// official per-user roots, by exact command name. A `versioned` root holds one folder per version, newest first.
const WINDOWS_INSTALL_ROOTS: &[(&str, &str, &[&str], bool)] = &[
    ("codex", "LOCALAPPDATA", &["OpenAI", "Codex", "bin"], true),
    ("codex", "APPDATA", &["npm"], false),
    ("claude", "APPDATA", &["npm"], false),
    ("claude", "LOCALAPPDATA", &["Programs", "claude"], false),
    ("cursor-agent", "LOCALAPPDATA", &["cursor-agent"], false),
    ("gemini", "APPDATA", &["npm"], false),
    ("muse", "LOCALAPPDATA", &["Programs", "muse"], false),
    ("opencode", "APPDATA", &["npm"], false),
    ("copilot", "APPDATA", &["npm"], false),
    ("copilot", "LOCALAPPDATA", &["copilot"], false),
    // A `.ps1` and a `.cmd` need a host, and the host is the one Windows ships before whatever PATH offers first.
    ("powershell", "SystemRoot", &["System32", "WindowsPowerShell", "v1.0"], false),
    ("cmd", "SystemRoot", &["System32"], false),
];

/// The line npm writes into a `.cmd` shim around a script, global (`node_modules\…`) or not (`..\…`).
const NPM_SHIM_SCRIPT: &str = r#""%dp0%\\((?:node_modules|\.\.)\\[^"\r\n]+\.[cm]?js)"\s+%\*"#;
/// The line npm writes around a native binary: the `.exe` is the program, and cmd.exe was only in the way.
const NPM_SHIM_BINARY: &str = r#"(?i)"%dp0%\\((?:node_modules|\.\.)\\[^"\r\n]+\.exe)"\s+%\*"#;

/// path-locator.ts `find`: PATH, then npm's bin directory, then the install roots, each resolved COMPLETELY (shims
/// included) before the next is tried, so a runtime the shell finds always beats a guessed copy.
pub fn locate(name: &str, look: &Look) -> Option<Launch> {
    if !safe_name(name) {
        return None;
    }
    let on_path = path_directories(look);
    if let Some(found) = resolve_within(name, &on_path, look) {
        return Some(found);
    }
    let npm = (look.npm_bin)().filter(|dir| is_absolute(dir, look.windows));
    if let Some(found) = npm.and_then(|dir| resolve_within(name, &[dir], look)) {
        return Some(found);
    }
    resolve_within(name, &install_directories(name, look), look)
}

fn path_directories(look: &Look) -> Vec<String> {
    let value = (look.env)("PATH").unwrap_or_default();
    let delimiter = if look.windows { ';' } else { ':' };
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for raw in value.split(delimiter) {
        let trimmed = raw.trim();
        let entry = if trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"') {
            &trimmed[1..trimmed.len() - 1]
        } else {
            trimmed
        };
        // Empty and relative entries mean the current directory; discovery never depends on it.
        if entry.is_empty() || !is_absolute(entry, look.windows) {
            continue;
        }
        let normalized = normalize(entry, look.windows);
        let key = if look.windows { normalized.to_lowercase() } else { normalized.clone() };
        if seen.insert(key) {
            out.push(normalized);
        }
    }
    out
}

fn install_directories(name: &str, look: &Look) -> Vec<String> {
    if !look.windows {
        return Vec::new();
    }
    let mut found = Vec::new();
    for (command, base, segments, versioned) in WINDOWS_INSTALL_ROOTS {
        if *command != name {
            continue;
        }
        let Some(base) = (look.env)(base).filter(|b| !b.is_empty() && is_absolute(b, true)) else {
            continue;
        };
        let directory = segments.iter().fold(base, |dir, part| join(&dir, part, true));
        if !versioned {
            found.push(directory);
            continue;
        }
        let mut versions = (look.read_dir)(&directory);
        versions.sort_by(|a, b| b.1.cmp(&a.1));
        found.extend(versions.into_iter().map(|(version, _)| join(&directory, &version, true)));
    }
    found
}

fn launch(discovered: &str, executable: &str, prefix_args: Vec<String>, kind: LaunchKind) -> Launch {
    Launch { discovered: discovered.to_owned(), executable: executable.to_owned(), prefix_args, kind }
}

/// path-locator.ts `locateNativeWindowsCommand`: `name.exe`, then `name.com`, in each directory in turn.
fn native_windows(name: &str, directories: &[String], look: &Look) -> Option<String> {
    let names = if name.rfind('.').is_some_and(|i| i > 0) {
        vec![name.to_owned()]
    } else {
        vec![format!("{name}.exe"), format!("{name}.com")]
    };
    directories
        .iter()
        .flat_map(|dir| names.iter().map(move |n| join(dir, n, true)))
        .find(|candidate| (look.is_executable)(candidate))
}

fn resolve_within(name: &str, directories: &[String], look: &Look) -> Option<Launch> {
    if !look.windows {
        return directories
            .iter()
            .map(|dir| join(dir, name, false))
            .find(|candidate| (look.is_executable)(candidate))
            .map(|found| launch(&found, &found, Vec::new(), LaunchKind::Native));
    }
    if let Some(native) = native_windows(name, directories, look) {
        return Some(launch(&native, &native, Vec::new(), LaunchKind::Native));
    }
    // `.cmd` BEFORE `.ps1`, the order Windows uses: npm's `.ps1` cannot take a bare `-`.
    for dir in directories {
        let script = join(dir, &format!("{name}.cmd"), true);
        if !(look.is_executable)(&script) {
            continue;
        }
        let text = (look.read_file)(&script);
        let wrapped = |pattern: &'static str| {
            text.as_deref()
                .and_then(|t| crate::invariants::regex(pattern).captures(t).map(|c| join(dir, &c[1], true)))
        };
        if let Some(binary) = wrapped(NPM_SHIM_BINARY).filter(|b| (look.is_executable)(b)) {
            return Some(launch(&script, &binary, Vec::new(), LaunchKind::Native));
        }
        if let Some(target) = wrapped(NPM_SHIM_SCRIPT) {
            let beside = join(dir, "node.exe", true);
            let node = if (look.is_executable)(&beside) { Some(beside) } else { native_windows("node", directories, look) };
            if let Some(node) = node {
                return Some(launch(&script, &node, vec![target], LaunchKind::NodeShim));
            }
        }
        let shells = [install_directories("cmd", look), directories.to_vec()].concat();
        let Some(shell) = native_windows("cmd", &shells, look) else {
            continue;
        };
        // /d skips AutoRun, /s settles how the quoted path is parsed, /c runs it and exits.
        let prefix = ["/d", "/s", "/c", &script].map(str::to_owned).to_vec();
        return Some(launch(&script, &shell, prefix, LaunchKind::CmdShim));
    }
    for dir in directories {
        let script = join(dir, &format!("{name}.ps1"), true);
        if !(look.is_executable)(&script) {
            continue;
        }
        let hosts = [install_directories("powershell", look), directories.to_vec()].concat();
        let Some(host) = native_windows("pwsh", &hosts, look).or_else(|| native_windows("powershell", &hosts, look)) else {
            continue;
        };
        let prefix = ["-NoProfile", "-NonInteractive", "-File", &script].map(str::to_owned).to_vec();
        return Some(launch(&script, &host, prefix, LaunchKind::PowershellShim));
    }
    None
}

fn is_absolute(path: &str, windows: bool) -> bool {
    if !windows {
        return path.starts_with('/');
    }
    let b = path.as_bytes();
    matches!(b.first(), Some(b'\\' | b'/'))
        || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/'))
}

/// `win32.join` / `posix.join`: the parts joined, then normalized.
fn join(folder: &str, name: &str, windows: bool) -> String {
    let sep = if windows { '\\' } else { '/' };
    normalize(&format!("{folder}{sep}{name}"), windows)
}

/// `win32.normalize` / `posix.normalize` for an absolute path: one separator, `.` dropped, `..` taking its parent, no
/// trailing separator. A Windows UNC path keeps its leading pair.
fn normalize(path: &str, windows: bool) -> String {
    let sep = if windows { '\\' } else { '/' };
    let text = if windows { path.replace('/', "\\") } else { path.to_owned() };
    let (root, rest) = if windows && text.starts_with("\\\\") {
        ("\\\\".to_owned(), &text[2..])
    } else if windows && text.len() >= 2 && text.as_bytes()[1] == b':' {
        let drive = &text[..2];
        let rest = &text[2..];
        if rest.starts_with('\\') { (format!("{drive}\\"), &rest[1..]) } else { (drive.to_owned(), rest) }
    } else if text.starts_with(sep) {
        (sep.to_string(), &text[1..])
    } else {
        (String::new(), text.as_str())
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in rest.split(sep) {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|p| *p != "..") {
                    parts.pop();
                } else if root.is_empty() {
                    parts.push("..");
                }
            }
            other => parts.push(other),
        }
    }
    format!("{root}{}", parts.join(&sep.to_string()))
}

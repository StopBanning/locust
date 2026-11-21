//! Editors found on PATH, and how to start one on a file (Electron's `main/installed-editors.ts`).
//!
//! Nothing is guessed from an install folder: if `code` or `cursor` is not on PATH, that editor is not offered.
//! A Windows `.cmd` shim runs through System32's cmd.exe with every part quoted and every cmd metacharacter escaped,
//! the line handed over verbatim (`runtime-adapters/cmd-line.ts`): a teammate's file named `a&calc&.txt` must never
//! run `calc` from "Open in VS Code" (Electron's 0.836 review).

use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorId {
    Vscode,
    Cursor,
}

impl EditorId {
    pub fn as_str(self) -> &'static str {
        match self {
            EditorId::Vscode => "vscode",
            EditorId::Cursor => "cursor",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledEditor {
    pub id: EditorId,
    pub command: String,
}

fn names(id: EditorId, platform: &str) -> &'static [&'static str] {
    match (platform == "win32", id) {
        (true, EditorId::Vscode) => &["code.cmd", "Code.cmd", "code.exe", "Code.exe"],
        (true, EditorId::Cursor) => &["cursor.cmd", "Cursor.cmd", "cursor.exe", "Cursor.exe"],
        (false, EditorId::Vscode) => &["code"],
        (false, EditorId::Cursor) => &["cursor"],
    }
}

/// `win32.join(folder, name)` / `posix.join(folder, name)`: one separator between the parts, repeats folded.
fn join(folder: &str, name: &str, platform: &str) -> String {
    let (sep, joined) = if platform == "win32" {
        ('\\', format!("{folder}\\{name}").replace('/', "\\"))
    } else {
        ('/', format!("{folder}/{name}"))
    };
    let mut out = String::with_capacity(joined.len());
    for (i, c) in joined.chars().enumerate() {
        // A Windows UNC path keeps its leading pair.
        if c == sep && out.ends_with(sep) && !(platform == "win32" && i == 1) {
            continue;
        }
        out.push(c);
    }
    if folder.is_empty() { name.to_owned() } else { out }
}

fn is_absolute(path: &str, platform: &str) -> bool {
    if platform != "win32" {
        return path.starts_with('/');
    }
    let b = path.as_bytes();
    matches!(b.first(), Some(b'\\' | b'/'))
        || (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && matches!(b[2], b'\\' | b'/'))
}

/// The editors whose command is on `path` (Electron's `editorsOnPath`), VS Code first.
pub fn editors_on_path(platform: &str, path: Option<&str>, exists: &dyn Fn(&str) -> bool) -> Vec<InstalledEditor> {
    let delimiter = if platform == "win32" { ';' } else { ':' };
    let folders: Vec<&str> = path.unwrap_or("").split(delimiter).collect();
    let mut found = Vec::new();
    for id in [EditorId::Vscode, EditorId::Cursor] {
        let command = names(id, platform).iter().find_map(|name| {
            folders
                .iter()
                .map(|folder| join(folder, name, platform))
                .find(|file| is_absolute(file, platform) && exists(file))
        });
        if let Some(command) = command {
            found.push(InstalledEditor { id, command });
        }
    }
    found
}

/// This machine's editors, read once from the PATH Locust adopted at launch.
pub fn installed_editors() -> &'static [InstalledEditor] {
    static FOUND: OnceLock<Vec<InstalledEditor>> = OnceLock::new();
    FOUND.get_or_init(|| {
        let path = std::env::var("PATH").ok();
        editors_on_path(this_platform(), path.as_deref(), &|file| std::path::Path::new(file).is_file())
    })
}

/// Node's `process.platform` for this build.
pub fn this_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "win32"
    } else {
        "linux"
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorSpawn {
    pub file: String,
    pub args: Vec<String>,
    /// The one argument is the whole command line, handed to cmd.exe as it is.
    pub verbatim: bool,
}

fn is_cmd_shim(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    lower.ends_with(".cmd") || lower.ends_with(".bat")
}

const CMD_META: &[char] = &['(', ')', ']', '[', '%', '!', '^', '"', '`', '<', '>', '&', '|', ';', ',', ' ', '*', '?'];

fn caret_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for c in text.chars() {
        if CMD_META.contains(&c) {
            out.push('^');
        }
        out.push(c);
    }
    out
}

/// cmd-line.ts `escapeArgument`, as its two replaces behave: a quote is escaped, a backslash just before one is
/// doubled, a trailing backslash is doubled; then the whole is quoted and caret-escaped.
fn escape_argument(argument: &str) -> String {
    let chars: Vec<char> = argument.chars().collect();
    let mut escaped = String::with_capacity(argument.len() + 8);
    let mut i = 0;
    while i < chars.len() {
        match (chars[i], chars.get(i + 1)) {
            ('\\', Some('"')) => {
                escaped.push_str("\\\\\\\"");
                i += 2;
            }
            ('"', _) => {
                escaped.push_str("\\\"");
                i += 1;
            }
            (c, _) => {
                escaped.push(c);
                i += 1;
            }
        }
    }
    if escaped.ends_with('\\') {
        escaped.push('\\');
    }
    caret_escape(&format!("\"{escaped}\""))
}

/// cmd-line.ts `cmdLauncherLine`: `/d /s /c "<launcher> <args>"`. A line break cannot pass through cmd.exe (it ends the
/// command there), so an argument with one is refused rather than cut short.
pub fn cmd_launcher_line(launcher: &str, args: &[&str]) -> Result<String, String> {
    if std::iter::once(&launcher).chain(args.iter()).any(|part| part.contains(['\r', '\n'])) {
        return Err("This runtime is started through a .cmd launcher, and cmd.exe cannot pass a line break in an argument. Nothing was started.".into());
    }
    let parts: Vec<String> = std::iter::once(caret_escape(launcher)).chain(args.iter().map(|a| escape_argument(a))).collect();
    Ok(format!("/d /s /c \"{}\"", parts.join(" ")))
}

/// How to start an editor on a file (Electron's `editorSpawn`). A file is always one argument; a Windows shim runs
/// through System32's cmd.exe, never a bare `cmd.exe` a working folder could plant. None when the file's name holds a
/// line break a shim cannot be handed.
pub fn editor_spawn(command: &str, file: &str, platform: &str, system_root: Option<&str>) -> Option<EditorSpawn> {
    if platform != "win32" || !is_cmd_shim(command) {
        return Some(EditorSpawn { file: command.to_owned(), args: vec![file.to_owned()], verbatim: false });
    }
    let root = system_root.unwrap_or("C:\\Windows");
    let cmd = join(&join(root, "System32", "win32"), "cmd.exe", "win32");
    let line = cmd_launcher_line(command, &[file]).ok()?;
    Some(EditorSpawn { file: cmd, args: vec![line], verbatim: true })
}

/// Start an editor on a file and do not wait for it (Electron's `launchEditor`).
pub fn launch_editor(command: &str, file: &str) -> std::io::Result<()> {
    let system_root = std::env::var("SystemRoot").or_else(|_| std::env::var("SYSTEMROOT")).ok();
    let Some(launch) = editor_spawn(command, file, this_platform(), system_root.as_deref()) else {
        return Err(std::io::Error::other("cmd.exe cannot be handed a file name with a line break"));
    };
    let mut c = std::process::Command::new(&launch.file);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
        if launch.verbatim {
            for a in &launch.args {
                c.raw_arg(a);
            }
        } else {
            c.args(&launch.args);
        }
    }
    #[cfg(not(windows))]
    c.args(&launch.args);
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    let mut child = c.spawn()?;
    // Not waited for; reaped on a thread so it never lingers as a zombie.
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[cfg(test)]
#[path = "installed_editors_tests.rs"]
mod tests;

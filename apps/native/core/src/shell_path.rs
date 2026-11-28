//! THE PATH A TERMINAL HAS: a port of Electron's apps/desktop/src/main/shell-path.ts.
//!
//! An app opened from the Dock or Finder is started by launchd with the bare system PATH
//! (/usr/bin:/bin:/usr/sbin:/sbin), not the one the person's shell builds, so every CLI installed by Homebrew, npm or
//! its own installer reads "not found on PATH" while it works in Terminal. Found live on 11/11: the native window,
//! opened from Finder with Electron quit, could not find OpenCode at ~/.opencode/bin. The fix goes where Electron's
//! does: the login shell is asked for its PATH once, at launch, and the usual install folders follow it in case the
//! shell cannot be asked. Linux has the same gap, narrower: a launcher gets ~/.profile's PATH but not ~/.bashrc's.
use std::{
    io::Read,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

/// Where macOS installers put the CLIs Locust runs (macInstallFolders).
pub fn mac_install_folders(home: &str) -> Vec<String> {
    vec![
        "/opt/homebrew/bin".into(),
        "/usr/local/bin".into(),
        format!("{home}/.local/bin"),
        format!("{home}/.npm-global/bin"),
        format!("{home}/.opencode/bin"),
        format!("{home}/.claude/local"),
        format!("{home}/.bun/bin"),
        format!("{home}/.volta/bin"),
    ]
}

/// Where Linux installers put them, for when the shell cannot be asked (linuxInstallFolders).
pub fn linux_install_folders(home: &str) -> Vec<String> {
    vec![
        "/usr/local/bin".into(),
        format!("{home}/.local/bin"),
        format!("{home}/.npm-global/bin"),
        format!("{home}/.opencode/bin"),
        format!("{home}/.claude/local"),
        format!("{home}/.bun/bin"),
        format!("{home}/.volta/bin"),
        "/home/linuxbrew/.linuxbrew/bin".into(),
        "/snap/bin".into(),
    ]
}

/// The lists joined in first-seen order, no repeats, no empty entries (mergedPath).
pub fn merged_path(lists: &[Option<&str>]) -> String {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<&str> = Vec::new();
    for list in lists.iter().flatten() {
        for entry in list.split(':') {
            let directory = entry.trim();
            if directory.is_empty() || !seen.insert(directory) {
                continue;
            }
            out.push(directory);
        }
    }
    out.join(":")
}

/// What the login shell says between the markers: a profile that prints a greeting must not end up inside PATH.
pub fn between_markers(said: &str) -> Option<String> {
    const MARK: &str = "__LOCUST_PATH__";
    let start = said.find(MARK)? + MARK.len();
    let end = said[start..].find(MARK)? + start;
    let value = said[start..end].trim();
    (!value.is_empty()).then(|| value.to_owned())
}

/// Ask the login shell for its PATH, within three seconds, or none (loginShellPath: `execFileSync`'s 3 s timeout).
pub fn login_shell_path(shell: Option<&str>, fallback: &str) -> Option<String> {
    login_shell_path_within(shell, fallback, Duration::from_secs(3))
}

/// `login_shell_path` with its time to answer given: the app's is three seconds.
fn login_shell_path_within(shell: Option<&str>, fallback: &str, budget: Duration) -> Option<String> {
    login_shell_path_with_runner(shell, fallback, |chosen, args| run_login_shell(chosen, args, budget))
}

fn login_shell_path_with_runner(
    shell: Option<&str>,
    fallback: &str,
    run: impl FnOnce(&str, &[&str]) -> Option<String>,
) -> Option<String> {
    let chosen = shell.filter(|s| s.starts_with('/')).unwrap_or(fallback);
    let args = ["-ilc", "printf \"__LOCUST_PATH__%s__LOCUST_PATH__\" \"$PATH\""];
    between_markers(&run(chosen, &args)?)
}

fn run_login_shell(chosen: &str, args: &[&str], budget: Duration) -> Option<String> {
    let mut child = Command::new(chosen)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut said = String::new();
        let _ = (&mut stdout).take(1024 * 1024).read_to_string(&mut said);
        said
    });
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < budget => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    reader.join().ok()
}

/// The PATH Locust should run with (shellPath): the shell's first, then what the app was given, then the usual install
/// folders. Only macOS and Linux; elsewhere the given PATH is returned as it is.
pub fn shell_path(
    os: &str,
    current: Option<&str>,
    home: &str,
    ask: &dyn Fn(&str) -> Option<String>,
) -> Option<String> {
    if os != "macos" && os != "linux" {
        return current.map(str::to_owned);
    }
    // zsh is every Mac's shell since Catalina; bash is the one a Linux without $SHELL is sure to have.
    let from_shell = ask(if os == "macos" { "/bin/zsh" } else { "/bin/bash" });
    let folders = if os == "macos" { mac_install_folders(home) } else { linux_install_folders(home) }.join(":");
    Some(merged_path(&[from_shell.as_deref(), current, Some(&folders)]))
}

/// Set PATH for this process at launch, before any thread starts (Electron does it at the top of main/index.ts).
pub fn adopt_terminal_path() {
    let current = std::env::var("PATH").ok();
    let home = std::env::var("HOME").unwrap_or_default();
    let shell = std::env::var("SHELL").ok();
    if let Some(path) = shell_path(std::env::consts::OS, current.as_deref(), &home, &|fallback| {
        login_shell_path(shell.as_deref(), fallback)
    }) {
        // SAFETY: called first thing in main, before any other thread exists, so nothing reads the environment
        // concurrently.
        unsafe { std::env::set_var("PATH", path) };
    }
    // npm's own prefix, asked now and awaited only by a lookup PATH has missed (index.ts: not awaited at launch).
    start_npm_bin_directory();
}

/// Long enough for a cold npm on Windows, short enough not to hold up a launch (npm-prefix.ts NPM_PREFIX_TIMEOUT_MS).
pub const NPM_PREFIX_TIMEOUT: Duration = Duration::from_secs(10);

/// `posix.join(prefix, "bin")`: the segments joined by one `/`, `.` dropped and `..` taking the one before it.
fn posix_join_bin(prefix: &str) -> String {
    let absolute = prefix.starts_with('/');
    let mut parts: Vec<&str> = Vec::new();
    for part in prefix.split('/').chain(std::iter::once("bin")) {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|last| *last != "..") => {
                parts.pop();
            }
            ".." if absolute => {}
            other => parts.push(other),
        }
    }
    let joined = parts.join("/");
    if absolute { format!("/{joined}") } else { joined }
}

/// The bin directory for an npm prefix (npmBinDirectoryFor): the prefix itself on Windows, where npm puts its shims
/// beside node_modules, and `<prefix>/bin` everywhere else. None for an empty prefix: `bin` alone is a RELATIVE path.
pub fn npm_bin_directory_for(prefix: &str, os: &str) -> Option<String> {
    let trimmed = prefix.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(if os == "windows" { trimmed.to_owned() } else { posix_join_bin(trimmed) })
}

/// What npm said its prefix is, as the bin directory to search (readNpmBinDirectory). None for every failure -- npm
/// absent, a non-zero exit, a timeout -- and for an answer that is no path: the first line is the answer, and `null`
/// or `undefined` is what npm prints for a config it does not have.
pub fn read_npm_bin_directory(os: &str, run: &dyn Fn() -> Result<String, String>) -> Option<String> {
    let said = run().ok()?;
    let first = said.split('\n').next().unwrap_or("").trim();
    if first.is_empty() || first == "null" || first == "undefined" {
        return None;
    }
    npm_bin_directory_for(first, os)
}

/// Run `npm config get prefix` through the shell, as Electron does (Windows will not start `npm.cmd` without one),
/// within `budget`, killing the whole tree it started when it does not answer: stdout only, its exit must be 0.
/// `path` is the PATH it runs with, None for this process's own.
pub fn ask_npm_prefix_within(path: Option<&std::ffi::OsStr>, budget: Duration) -> Result<String, String> {
    let mut command = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/d", "/s", "/c", "npm config get prefix"]);
        c
    } else {
        let mut c = Command::new("/bin/sh");
        c.args(["-c", "npm config get prefix"]);
        c
    };
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    if let Some(path) = path {
        command.env("PATH", path);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    let mut stdout = child.stdout.take().ok_or("npm gave no output")?;
    let reader = std::thread::spawn(move || {
        let mut said = String::new();
        let _ = (&mut stdout).take(1024 * 1024).read_to_string(&mut said);
        said
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < budget => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                // A TREE kill: killing the shell alone left a hung npm shim and its sleep running (npm-prefix.ts).
                #[cfg(unix)]
                if let Ok(pid) = i32::try_from(child.id()) {
                    // SAFETY: the group was made for this child alone by process_group(0) above.
                    unsafe { libc::killpg(pid, libc::SIGKILL) };
                }
                let _ = child.kill();
                let _ = child.wait();
                return Err("npm did not answer".into());
            }
        }
    };
    let said = reader.join().map_err(|_| "npm's output could not be read")?;
    if status.success() { Ok(said) } else { Err(format!("npm exited {:?}", status.code())) }
}

static NPM_BIN_DIRECTORY: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
static NPM_BIN_ASKED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Ask npm for its prefix once, on its own thread, so a launch never waits on it.
pub fn start_npm_bin_directory() {
    NPM_BIN_ASKED.store(true, std::sync::atomic::Ordering::SeqCst);
    std::thread::spawn(npm_bin_directory);
}

/// npm's bin directory on this computer, waiting for the answer if it is still coming. None when the app never asked
/// (a test, or a command run without adopting the terminal's PATH), so nothing but the app's own launch runs npm.
pub fn npm_bin_directory() -> Option<String> {
    if !NPM_BIN_ASKED.load(std::sync::atomic::Ordering::SeqCst) {
        return None;
    }
    NPM_BIN_DIRECTORY
        .get_or_init(|| read_npm_bin_directory(std::env::consts::OS, &|| ask_npm_prefix_within(None, NPM_PREFIX_TIMEOUT)))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    // case: on macOS puts the login shell's PATH first, then what it was given, then the usual install folders
    #[test]
    fn on_macos_the_login_shells_path_comes_first_then_what_it_was_given_then_the_usual_folders() {
        let path = shell_path("macos", Some("/usr/bin:/bin:/usr/sbin:/sbin"), "/Users/ian", &|_| {
            Some("/opt/homebrew/bin:/Users/ian/.npm-global/bin:/usr/bin".into())
        })
        .expect("a PATH");
        let parts: Vec<&str> = path.split(':').collect();
        assert_eq!(&parts[..3], ["/opt/homebrew/bin", "/Users/ian/.npm-global/bin", "/usr/bin"]);
        assert!(parts.contains(&"/Users/ian/.local/bin") && parts.contains(&"/usr/local/bin"));
        assert!(parts.contains(&"/Users/ian/.opencode/bin"));
        let unique: std::collections::HashSet<_> = parts.iter().collect();
        assert_eq!(unique.len(), parts.len());
    }

    // case: still finds the usual folders when the shell cannot be asked
    #[test]
    fn still_finds_the_usual_folders_when_the_shell_cannot_be_asked() {
        let path = shell_path("macos", Some("/usr/bin:/bin"), "/Users/ian", &|_| None).expect("a PATH");
        assert!(path.contains("/opt/homebrew/bin"));
        assert!(path.starts_with("/usr/bin:/bin:"));
    }

    // case: is left alone on Windows
    #[test]
    fn is_left_alone_on_windows() {
        assert_eq!(shell_path("windows", Some("C:\\Windows"), "", &|_| Some("/nope".into())).as_deref(), Some("C:\\Windows"));
    }

    #[test]
    fn reads_only_what_is_between_the_markers_whatever_a_shell_profile_prints() {
        assert_eq!(
            between_markers("Welcome back!\n__LOCUST_PATH__/opt/homebrew/bin:/usr/bin__LOCUST_PATH__").as_deref(),
            Some("/opt/homebrew/bin:/usr/bin")
        );
        assert_eq!(between_markers("timed out"), None);
        assert_eq!(between_markers("__LOCUST_PATH____LOCUST_PATH__"), None);
    }

    // case: merges without repeats or empty entries
    #[test]
    fn merges_without_repeats_or_empty_entries() {
        assert_eq!(merged_path(&[Some("/a:/b"), Some("/b::/c"), None]), "/a:/b:/c");
    }

    #[test]
    fn on_linux_the_linux_folders_follow_and_never_homebrews_mac_folder() {
        let path = shell_path("linux", Some("/usr/bin:/bin"), "/home/ian", &|_| {
            Some("/home/ian/.nvm/versions/node/v24.1.0/bin:/usr/bin:/bin".into())
        })
        .expect("a PATH");
        let parts: Vec<&str> = path.split(':').collect();
        assert_eq!(&parts[..3], ["/home/ian/.nvm/versions/node/v24.1.0/bin", "/usr/bin", "/bin"]);
        assert!(parts.contains(&"/home/ian/.local/bin") && parts.contains(&"/home/ian/.opencode/bin"));
        assert!(parts.contains(&"/home/linuxbrew/.linuxbrew/bin"));
        assert!(!parts.contains(&"/opt/homebrew/bin"));
    }

    #[test]
    fn asks_bash_when_the_session_names_no_shell_and_zsh_on_a_mac() {
        let asked = std::cell::RefCell::new(Vec::new());
        let _ = shell_path("linux", None, "/home/ian", &|fallback| {
            asked.borrow_mut().push(fallback.to_owned());
            None
        });
        let _ = shell_path("macos", None, "/Users/ian", &|fallback| {
            asked.borrow_mut().push(fallback.to_owned());
            None
        });
        assert_eq!(*asked.borrow(), ["/bin/bash", "/bin/zsh"]);
    }

    #[test]
    fn a_real_login_shell_answers_with_a_path() {
        // The Mac's own zsh, asked the way the app asks it: a real answer, not a guess.
        // What this checks is that a real login shell answers with its PATH, read between the markers. How long the
        // app gives it (3 s, as Electron) is the app's to choose; a zsh starting on a machine running four builds can
        // take longer, so the test gives it the load limit.
        if std::path::Path::new("/bin/zsh").exists() {
            let start = std::time::Instant::now();
            let said = login_shell_path_within(Some("/bin/zsh"), "/bin/zsh", crate::load_waits::UNDER_LOAD).expect("zsh answers");
            crate::load_waits::note_time("a_real_login_shell_answers_with_a_path", start.elapsed());
            assert!(said.contains("/usr/bin"), "{said}");
        }
    }
}

#[cfg(test)]
#[path = "agy_on_a_mac_tests.rs"]
mod agy_on_a_mac_tests;

#[cfg(test)]
#[path = "npm_prefix_tests.rs"]
mod npm_prefix_tests;

#[cfg(test)]
#[path = "shell_path_case_tests.rs"]
mod shell_path_case_tests;

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

/// Ask the login shell for its PATH, within three seconds, or none (loginShellPath).
pub fn login_shell_path(shell: Option<&str>, fallback: &str) -> Option<String> {
    let chosen = shell.filter(|s| s.starts_with('/')).unwrap_or(fallback);
    let mut child = Command::new(chosen)
        .args(["-ilc", "printf \"__LOCUST_PATH__%s__LOCUST_PATH__\" \"$PATH\""])
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
            Ok(None) if start.elapsed() < Duration::from_secs(3) => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    between_markers(&reader.join().ok()?)
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
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn still_finds_the_usual_folders_when_the_shell_cannot_be_asked() {
        let path = shell_path("macos", Some("/usr/bin:/bin"), "/Users/ian", &|_| None).expect("a PATH");
        assert!(path.contains("/opt/homebrew/bin"));
        assert!(path.starts_with("/usr/bin:/bin:"));
    }

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
        if std::path::Path::new("/bin/zsh").exists() {
            let said = login_shell_path(Some("/bin/zsh"), "/bin/zsh").expect("zsh answers");
            assert!(said.contains("/usr/bin"), "{said}");
        }
    }
}

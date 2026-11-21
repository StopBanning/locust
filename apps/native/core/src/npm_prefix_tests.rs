//! A runtime installed into a moved npm prefix must still be findable (Electron's main/npm-prefix.test.ts).
//!
//! The usual install folders name npm's DEFAULT prefix, and moving it is ordinary: corporate Windows, every
//! nvm-style version manager, and the remedy Locust itself prints when an install fails on permissions
//! (`npm config set prefix`). Without asking npm, the person is sent to install a thing they have just installed.
use super::*;
use std::path::{Path, PathBuf};

static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A folder of its own under the temp dir, removed when dropped.
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            ".npm-prefix-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    /// An executable script called `name` in `dir` (made if missing), saying `body` to /bin/sh.
    fn script(&self, dir: &str, name: &str, body: &str) -> PathBuf {
        let folder = self.0.join(dir);
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }
    /// A PATH naming only these folders of the scratch, then the system's own (for `sh`, `sleep`, `cat`).
    fn path(&self, dirs: &[&str]) -> std::ffi::OsString {
        let mut all: Vec<PathBuf> = dirs.iter().map(|d| self.0.join(d)).collect();
        all.extend([PathBuf::from("/usr/bin"), PathBuf::from("/bin")]);
        std::env::join_paths(all).unwrap()
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// case: is the prefix itself on Windows
#[test]
fn is_the_prefix_itself_on_windows() {
    // npm puts its shims directly in the prefix on Windows, beside node_modules.
    assert_eq!(
        npm_bin_directory_for("C:\\Users\\a\\AppData\\Local\\npm-global", "windows").as_deref(),
        Some("C:\\Users\\a\\AppData\\Local\\npm-global")
    );
}

// case: is <prefix>/bin everywhere else
#[test]
fn is_prefix_bin_everywhere_else() {
    assert_eq!(npm_bin_directory_for("/home/a/.npm-global", "linux").as_deref(), Some("/home/a/.npm-global/bin"));
    assert_eq!(npm_bin_directory_for("/usr/local", "macos").as_deref(), Some("/usr/local/bin"));
    // posix.join's own answers for a prefix written with a trailing or doubled slash, or a dot.
    assert_eq!(npm_bin_directory_for("/usr/local/", "macos").as_deref(), Some("/usr/local/bin"));
    assert_eq!(npm_bin_directory_for("/opt//npm/./x/..", "linux").as_deref(), Some("/opt/npm/bin"));
    assert_eq!(npm_bin_directory_for("/", "linux").as_deref(), Some("/bin"));
}

// case: refuses an empty prefix rather than returning a bare bin path
#[test]
fn refuses_an_empty_prefix_rather_than_returning_a_bare_bin_path() {
    // `bin` alone is a RELATIVE path, resolved against whatever directory the app started in.
    assert_eq!(npm_bin_directory_for("", "linux"), None);
    assert_eq!(npm_bin_directory_for("   ", "windows"), None);
}

// case: reads the path npm printed
#[test]
fn reads_the_path_npm_printed() {
    assert_eq!(read_npm_bin_directory("windows", &|| Ok("C:\\npm-global\r\n".into())).as_deref(), Some("C:\\npm-global"));
}

// case: takes the first line and trims it
#[test]
fn takes_the_first_line_and_trims_it() {
    // npm prints the value alone, but a machine with a noisy profile can add to the stream.
    assert_eq!(
        read_npm_bin_directory("linux", &|| Ok("/opt/npm\nnpm notice something\n".into())).as_deref(),
        Some("/opt/npm/bin")
    );
    assert_eq!(read_npm_bin_directory("linux", &|| Ok("  /opt/npm  \n".into())).as_deref(), Some("/opt/npm/bin"));
}

// case: answers undefined when npm says null
#[test]
fn answers_none_when_npm_says_null() {
    // A directory called "null" is not a place to look for anything.
    assert_eq!(read_npm_bin_directory("linux", &|| Ok("null\n".into())), None);
    assert_eq!(read_npm_bin_directory("linux", &|| Ok("undefined\n".into())), None);
    assert_eq!(read_npm_bin_directory("linux", &|| Ok("\n".into())), None);
    // Trimmed before it is compared: a padded or Windows-ended `null` is still npm saying it has none.
    assert_eq!(read_npm_bin_directory("linux", &|| Ok("  null \r\n".into())), None);
}

// case: answers undefined when npm cannot be asked at all
#[test]
fn answers_none_when_npm_cannot_be_asked_at_all() {
    // A machine with no npm is not a machine with a broken Locust: every failure lands here, never in a panic.
    assert_eq!(read_npm_bin_directory("linux", &|| Err("spawn npm ENOENT".into())), None);
}

// The real ask, of a fake npm on a PATH of its own: what it prints when it exits 0, nothing when it does not.
#[cfg(unix)]
#[test]
fn the_real_ask_reads_npm_through_the_shell_and_only_a_clean_exit() {
    let scratch = Scratch::new();
    scratch.script("good", "npm", r#"[ "$*" = "config get prefix" ] || exit 9; echo /opt/moved"#);
    scratch.script("bad", "npm", "echo /opt/moved; exit 1");
    let good = scratch.path(&["good"]);
    let said = ask_npm_prefix_within(Some(&good), crate::load_waits::UNDER_LOAD);
    assert_eq!(said.as_deref(), Ok("/opt/moved\n"));
    assert_eq!(read_npm_bin_directory("macos", &|| said.clone()).as_deref(), Some("/opt/moved/bin"));
    let bad = scratch.path(&["bad"]);
    assert!(ask_npm_prefix_within(Some(&bad), crate::load_waits::UNDER_LOAD).is_err());
    let none = scratch.path(&["empty"]);
    assert!(ask_npm_prefix_within(Some(&none), crate::load_waits::UNDER_LOAD).is_err());
}

// A hung npm is given up on in time, and what it started goes with it (npm-prefix.ts's tree kill).
#[cfg(unix)]
#[test]
fn a_hung_npm_is_given_up_on_and_its_whole_tree_ended() {
    let scratch = Scratch::new();
    let pid_file = scratch.0.join("sleep.pid");
    scratch.script("hung", "npm", &format!("sleep 60 &\necho $! > '{}'\nwait", pid_file.display()));
    let path = scratch.path(&["hung"]);
    let start = Instant::now();
    let said = ask_npm_prefix_within(Some(&path), Duration::from_millis(1500));
    assert_eq!(said, Err("npm did not answer".into()));
    assert!(start.elapsed() < Duration::from_secs(30), "{:?}", start.elapsed());
    let pid: i32 = std::fs::read_to_string(&pid_file).expect("the fake npm started its sleep").trim().parse().unwrap();
    // The sleep it started is gone too.
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        // SAFETY: signal 0 only asks whether the process exists; the orphaned sleep is reaped by init once killed.
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        if !alive {
            break;
        }
        assert!(Instant::now() < deadline, "the hung npm's sleep {pid} outlived it");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn found(name: &str, paths: &std::ffi::OsStr, npm_bin: Option<&Path>) -> Option<PathBuf> {
    let asked = std::cell::Cell::new(false);
    let answer = crate::runtime::discovery::find_beyond_path(name, paths, || {
        asked.set(true);
        npm_bin.map(|p| p.display().to_string())
    })
    .ok();
    // npm is consulted only when PATH missed.
    if answer.as_ref().is_some_and(|p| !npm_bin.is_some_and(|dir| p.starts_with(dir))) {
        assert!(!asked.get(), "npm's prefix was read though PATH had {name}");
    }
    answer
}

// A CLI installed under a moved prefix, outside PATH, is found there; PATH still comes first.
#[cfg(unix)]
#[test]
fn a_cli_under_a_moved_npm_prefix_is_found_after_path_misses() {
    let scratch = Scratch::new();
    let moved = scratch.script("moved/bin", "kilo", "echo moved");
    let on_path = scratch.script("path", "kilo", "echo path");
    let npm_bin = scratch.0.join("moved/bin");
    // Not on PATH: found in npm's bin.
    assert_eq!(found("kilo", &scratch.path(&["elsewhere"]), Some(&npm_bin)), Some(moved));
    // On PATH too: PATH's.
    assert_eq!(found("kilo", &scratch.path(&["path"]), Some(&npm_bin)), Some(on_path));
    // Neither: not found, with PATH's own words.
    let missed = crate::runtime::discovery::find_beyond_path("kilo", &scratch.path(&["elsewhere"]), || None).unwrap_err();
    assert!(missed.to_string().contains("was not found on PATH"), "{missed}");
    assert_eq!(found("codex", &scratch.path(&["elsewhere"]), Some(&npm_bin)), None);
    // A relative answer is never searched.
    assert_eq!(found("kilo", &scratch.path(&["elsewhere"]), Some(Path::new("moved/bin"))), None);
}

// Nothing but the app's launch asks npm: a lookup in a process that never adopted the terminal's PATH runs no npm.
#[test]
fn npm_is_not_asked_unless_the_launch_asked_it() {
    if !NPM_BIN_ASKED.load(std::sync::atomic::Ordering::SeqCst) {
        assert_eq!(npm_bin_directory(), None);
        assert!(NPM_BIN_DIRECTORY.get().is_none());
    }
}

// The app's own lookup goes there once the launch has asked: discovery's `find_named`, which every agent row uses.
#[cfg(unix)]
#[test]
fn find_named_looks_in_npms_bin_once_the_launch_has_asked() {
    let scratch = Scratch::new();
    // A name no other test looks up, so another test sharing this process finds nothing new here.
    let probe = scratch.script("moved/bin", "locust-npm-prefix-probe", "true");
    let dir = scratch.0.join("moved/bin").display().to_string();
    assert!(NPM_BIN_DIRECTORY.set(Some(dir)).is_ok(), "only this test answers for npm");
    NPM_BIN_ASKED.store(true, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(crate::runtime::discovery::find_named("locust-npm-prefix-probe").ok(), Some(probe));
}

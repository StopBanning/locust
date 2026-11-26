//! `a-beta-can-report-itself.test.ts`, the engine's half: the log writer the window calls (`note`), its roll, its
//! lines, the panic hook, the child the window starts, the watch on a window that stops answering and the run's first
//! line. Electron's cases read `main/index.ts`; these drive the writer and read the file it leaves in a temporary data
//! folder.
use super::*;
use crate::report_problem::MAX_LOG_BYTES;

static HOOK: Mutex<()> = Mutex::new(());
static TEMP: AtomicU64 = AtomicU64::new(0);

struct Dir(PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn dir() -> Dir {
    let path = std::env::temp_dir().join(format!("lrep-{}-{}", std::process::id(), TEMP.fetch_add(1, Ordering::Relaxed)));
    fs::create_dir_all(&path).unwrap();
    Dir(path)
}

fn log(root: &Path) -> String {
    fs::read_to_string(log_path(root)).unwrap_or_default()
}

fn rolled(root: &Path) -> Option<String> {
    fs::read_to_string(root.join(format!("{LOG_FILE}.1"))).ok()
}

/// Run `f` with the hook `begin` or `install_panic_hook` leaves, then put the default hook back.
fn with_hook(f: impl FnOnce()) {
    let _held = HOOK.lock().unwrap_or_else(|e| e.into_inner());
    f();
    drop(std::panic::take_hook());
}

fn panic_on(thread: &str, words: &'static str) {
    let _ = std::thread::Builder::new().name(thread.into()).spawn(move || std::panic::panic_any(words)).unwrap().join();
}

// ---- the log rolls rather than growing for ever ----

// case: does not roll a file that does not exist yet
#[test]
fn a_first_line_rolls_nothing() {
    let d = dir();
    assert!(!should_roll(None));
    note(&d.0, "start", "Locust 1");
    assert!(log(&d.0).contains("start: Locust 1"));
    assert_eq!(rolled(&d.0), None, "nothing to roll on the first run");
}

// case: does not roll a small one
#[test]
fn a_small_log_is_appended_to() {
    let d = dir();
    note(&d.0, "one", "first");
    note(&d.0, "two", "second");
    let text = log(&d.0);
    assert!(text.contains("one: first") && text.contains("two: second"));
    assert_eq!(rolled(&d.0), None);
}

// case: rolls at the cap
#[test]
fn a_log_at_its_cap_rolls_before_the_line() {
    for size in [MAX_LOG_BYTES, MAX_LOG_BYTES + 1] {
        let d = dir();
        fs::write(log_path(&d.0), vec![b'x'; size as usize]).unwrap();
        note(&d.0, "after", "the roll");
        assert_eq!(rolled(&d.0).map(|t| t.len() as u64), Some(size), "the full log moved to .1");
        assert!(log(&d.0).ends_with("after: the roll\n") && log(&d.0).len() < 100, "a fresh log holds the new line");
    }
}

// case: keeps one generation, so a crash loop cannot fill a disk
#[test]
fn one_roll_replaces_the_older_one() {
    let d = dir();
    fs::write(d.0.join(format!("{LOG_FILE}.1")), "the oldest").unwrap();
    fs::write(log_path(&d.0), vec![b'y'; MAX_LOG_BYTES as usize]).unwrap();
    note(&d.0, "loop", "again");
    let held = rolled(&d.0).unwrap();
    assert!(!held.contains("the oldest") && held.len() as u64 == MAX_LOG_BYTES, "the oldest was dropped");
    let mut names: Vec<String> = fs::read_dir(&d.0).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    assert_eq!(names, [LOG_FILE.to_owned(), format!("{LOG_FILE}.1")], "two files, ever");
}

// case: still writes the line when the roll fails
#[cfg(unix)]
#[test]
fn a_failed_roll_still_writes() {
    let d = dir();
    // A folder in `.1`'s place, with something in it: the rename onto it fails.
    fs::create_dir_all(d.0.join(format!("{LOG_FILE}.1/held"))).unwrap();
    fs::write(log_path(&d.0), vec![b'z'; MAX_LOG_BYTES as usize]).unwrap();
    note(&d.0, "trouble", "still said");
    assert!(log(&d.0).ends_with("trouble: still said\n"), "appending to an oversized log beats losing the line");
}

// ---- a line always says something ----

// case: carries the time, the label and the detail
#[test]
fn a_line_has_when_what_and_detail() {
    let d = dir();
    let at = DateTime::parse_from_rfc3339("2026-09-15T10:00:00.000Z").unwrap().with_timezone(&Utc);
    note_at(&d.0, at, "start", "Locust 0.138.0");
    assert_eq!(log(&d.0), "2026-09-15T10:00:00.000Z start: Locust 0.138.0\n");
}

// case: never writes a blank detail, which reads as a truncated file
#[test]
fn a_blank_detail_says_so() {
    let d = dir();
    note(&d.0, "x", "   ");
    assert!(log(&d.0).ends_with("x: (no detail)\n"));
}

// ---- the crashes the main process cannot throw ----

// case: records a renderer that died
#[test]
fn a_panic_on_the_window_thread_is_written() {
    let d = dir();
    with_hook(|| {
        install_panic_hook(d.0.clone());
        panic_on("main", "the view could not be built");
    });
    let text = log(&d.0);
    assert!(text.contains("panic: the window stopped: thread 'main' panicked at "), "{text}");
    assert!(text.contains("the view could not be built"), "a literal from the source is kept: {text}");
}

// case: records a child process that died
#[cfg(unix)]
#[test]
fn the_engine_child_dying_is_written() {
    use std::os::unix::fs::PermissionsExt;
    let d = dir();
    let exe = d.0.join("dies.sh");
    fs::write(&exe, "#!/bin/sh\nkill -KILL $$\n").unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o755)).unwrap();
    // The window starts its engine this way; this one dies before it answers.
    assert!(crate::engine_process::join_or_start(&d.0, &exe).is_err());
    let end = Instant::now() + Duration::from_secs(3);
    while !log(&d.0).contains("child-process-gone") && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(
        log(&d.0).contains("child-process-gone: Locust's engine was killed (reason=killed exitCode=137)"),
        "{}",
        log(&d.0)
    );
}

// case: records a window that stopped answering, and that it came back
#[test]
fn a_window_busy_too_long_is_written_and_its_return() {
    let d = dir();
    let watch = Arc::new(Watch::default());
    let watcher = watch_answering(Arc::clone(&watch), d.0.clone(), Duration::from_millis(60), Duration::from_millis(10)).unwrap();
    {
        let _busy = watch.answering();
        std::thread::sleep(Duration::from_millis(300));
    }
    std::thread::sleep(Duration::from_millis(100));
    {
        let _quick = watch.answering();
    }
    std::thread::sleep(Duration::from_millis(50));
    watch.stop();
    watcher.join().unwrap();
    let text = log(&d.0);
    assert_eq!(text.matches("unresponsive: a window stopped answering").count(), 1, "{text}");
    assert_eq!(text.matches("responsive: it started answering again").count(), 1, "{text}");
    assert!(text.find("unresponsive").unwrap() < text.find(" responsive:").unwrap());
}

// case: does not log a window being closed on purpose
#[cfg(unix)]
#[test]
fn a_clean_exit_writes_nothing() {
    let d = dir();
    let child = std::process::Command::new("true").spawn().unwrap();
    watch_child(d.0.clone(), "Locust's engine", child);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(log(&d.0), "", "a log full of clean exits is a log nobody reads");
    let ok = std::process::Command::new("true").status().unwrap();
    assert!(!note_child_gone(&d.0, "Locust's engine", &ok));
    let failed = std::process::Command::new("false").status().unwrap();
    assert!(note_child_gone(&d.0, "Locust's engine", &failed));
    assert!(log(&d.0).contains("exited abnormally (reason=abnormal-exit exitCode=1)"));
}

// case: says what a death was, in words and in the raw token
#[test]
fn a_death_is_named_in_words_and_token() {
    let oom = describe_gone("the window", Some("oom"), None);
    assert!(oom.contains("ran out of memory") && oom.contains("reason=oom"));
    assert!(describe_gone("the window", Some("crashed"), Some(133)).contains("exitCode=133"));
}

// case: does not pretend to recognise a reason it does not
#[test]
fn an_unknown_reason_is_said_as_unknown() {
    let said = describe_gone("the window", Some("something-new"), None);
    assert!(said.contains("unrecognised") && said.contains("reason=something-new"));
}

// ---- every run says which version it was ----

// case: names the build, the platform and whether it was packaged
#[test]
fn the_first_line_names_build_platform_and_packaging() {
    let d = dir();
    with_hook(|| begin(&d.0, "0.138.0", true));
    let first = log(&d.0);
    assert!(first.contains(" start: Locust 0.138.0"), "{first}");
    assert!(first.contains(std::env::consts::OS.replace("macos", "darwin").as_str()), "{first}");
    assert!(first.contains("packaged"), "{first}");
}

// Every run's first line says which commit it was built from, so a fault in the log is tied to a build.
#[test]
fn the_first_line_names_the_commit() {
    let d = dir();
    with_hook(|| begin(&d.0, "0.138.0", true));
    let first = log(&d.0);
    // This build's commit: the one it was given at build time (build.rs), or dev.
    let commit = crate::build_info::commit();
    assert!(first.trim_end().ends_with(&format!(", packaged, commit {commit})")), "{first}");
    assert!(commit == "dev" || commit.bytes().all(|b| b.is_ascii_hexdigit()), "commit-shaped, never a name: {commit}");
}

// case: distinguishes a development run, which reports different bugs
#[test]
fn a_development_run_says_so() {
    let d = dir();
    with_hook(|| begin(&d.0, "0", false));
    assert!(log(&d.0).contains("development"));
}

// case: is built from no path, at the call site where a path could get in
#[test]
fn the_first_line_carries_no_path_or_name() {
    let d = dir();
    with_hook(|| begin(&d.0, "0.138.0", true));
    let first = log(&d.0);
    assert!(!first.contains(&d.0.to_string_lossy().into_owned()), "no data folder: {first}");
    if let Some(home) = std::env::var_os("HOME").filter(|h| h.len() > 1) {
        assert!(!first.contains(&home.to_string_lossy().into_owned()), "no home folder: {first}");
    }
    if let Some(user) = std::env::var_os("USER").filter(|u| u.len() > 2) {
        assert!(!first.contains(&format!("/{}", user.to_string_lossy())), "no user name: {first}");
    }
    assert!(first.contains("0.138.0"), "the guard fails loudly if the line is gutted");
}

// ---- what never goes in ----

#[test]
fn keys_home_and_run_time_messages_stay_out() {
    let d = dir();
    let key = format!("{}-ant-api03-{}", "sk", "Q".repeat(40));
    let home = std::env::var("HOME").unwrap_or_default();
    note(&d.0, "trouble", &format!("failed with {key} in {home}/Projects/secret-plan"));
    let text = log(&d.0);
    assert!(!text.contains(&key), "{text}");
    if home.len() > 1 {
        assert!(!text.contains(&home) && text.contains("~/Projects/secret-plan"), "{text}");
    }
    let conversation = String::from("my private words");
    with_hook(|| {
        install_panic_hook(d.0.clone());
        let words = conversation.clone();
        let _ = std::thread::Builder::new().name("turn".into()).spawn(move || panic!("bad reply {words}")).unwrap().join();
    });
    let text = log(&d.0);
    assert!(text.contains("thread 'turn' panicked") && !text.contains("my private words"), "{text}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(log_path(&d.0)).unwrap().permissions().mode() & 0o777, 0o600);
    }
}

//! The hidden terminal on macOS and Linux (pseudo-terminal.ts's helper, there Windows-only), driven by small shell
//! scripts through `run_in_pseudo_terminal` itself. Claude Code never runs.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::{PseudoTerminalResult, PseudoTerminalRun, run_in_pseudo_terminal};

struct Temp(PathBuf);

impl Temp {
    fn new(label: &str) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("locust-cloud-pty-{label}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("temp");
        Self(path.canonicalize().expect("real path"))
    }
    fn text(&self) -> String {
        self.0.to_string_lossy().into_owned()
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn sh(script: &str, seconds: u64) -> PseudoTerminalRun {
    PseudoTerminalRun {
        line: String::new(),
        argv: vec!["/bin/sh".into(), "-c".into(), script.into()],
        cwd: String::new(),
        env: BTreeMap::new(),
        env_remove: Vec::new(),
        seconds,
        stop_when: String::new(),
        stop_when_written: None,
    }
}

fn drawn(result: PseudoTerminalResult) -> String {
    match result {
        PseudoTerminalResult::Ok { drawn } => drawn,
        PseudoTerminalResult::Err => panic!("the hidden terminal did not run"),
    }
}

/// A process is gone once it cannot be signalled (it was ended and reaped, here or by launchd).
fn gone(pid: i32) -> bool {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        if unsafe { libc::kill(pid, 0) } != 0 {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn pid_in(path: &std::path::Path) -> i32 {
    std::fs::read_to_string(path).expect("pid written").trim().parse().expect("a pid")
}

#[test]
fn gives_back_what_a_program_drew_in_a_terminal_nobody_sees_once_it_ends() {
    let started = Instant::now();
    let text = drawn(run_in_pseudo_terminal(&sh(r"printf 'hello \033[31mred\033[0m\n'; [ -t 1 ] && echo a-terminal", 10)));
    assert!(text.contains("hello \u{1b}[31mred\u{1b}[0m"), "{text:?} after {:?}", started.elapsed());
    // It ran in a terminal, which `claude --teleport` insists on.
    assert!(text.contains("a-terminal"), "{text:?}");
    // It ended on its own: nothing waited out the ten seconds.
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn a_program_that_ends_on_its_own_is_not_ended_again_and_what_it_left_running_is_left_alone() {
    // The helper ends the program only when it has not exited (`if (!exited) taskkill`): what it started in the
    // background is its own business once it is done.
    let place = Temp::new("ended");
    let kid_file = place.0.join("kid");
    let text = drawn(run_in_pseudo_terminal(&sh(
        &format!("trap '' HUP; sleep 30 </dev/null >/dev/null 2>&1 & echo $! > '{}'; printf 'done\\n'", kid_file.display()),
        10,
    )));
    assert!(text.contains("done"), "{text:?}");
    let kid = pid_in(&kid_file);
    // A process just killed is a zombie until launchd reaps it, and a zombie still answers kill(pid, 0).
    std::thread::sleep(Duration::from_millis(300));
    let stat = std::process::Command::new("/bin/ps").args(["-o", "stat=", "-p", &kid.to_string()]).output().unwrap();
    let stat = String::from_utf8_lossy(&stat.stdout).trim().to_string();
    let alive = !stat.is_empty() && !stat.starts_with('Z');
    unsafe { libc::kill(kid, libc::SIGKILL) };
    assert!(alive, "what the program left running was ended too");
}

#[test]
fn runs_the_words_as_they_are_in_the_folder_with_the_runs_variables_and_without_the_removed_ones() {
    let folder = Temp::new("cwd");
    let mut run = sh("", 10);
    run.argv = vec![
        "/bin/sh".into(),
        "-c".into(),
        r#"printf '[%s][%s][%s][%s]\n' "$(pwd -P)" "$LOCUST_PTY_SAYS" "${HOME-gone}" "$1""#.into(),
        "zero".into(),
        "two words; $(not run)".into(),
    ];
    run.cwd = folder.text();
    run.env.insert("LOCUST_PTY_SAYS".into(), "kept".into());
    run.env_remove.push("HOME".into());
    let text = drawn(run_in_pseudo_terminal(&run));
    assert!(text.contains(&format!("[{}][kept][gone][two words; $(not run)]", folder.text())), "{text:?}");
}

#[test]
fn ends_and_reaps_a_program_still_running_when_its_seconds_are_up_and_keeps_what_it_drew() {
    let place = Temp::new("timeout");
    let pid_file = place.0.join("pid");
    let started = Instant::now();
    let text = drawn(run_in_pseudo_terminal(&sh(
        &format!("echo $$ > '{}'; printf 'started\\n'; exec sleep 60", pid_file.display()),
        1,
    )));
    assert!(started.elapsed() < Duration::from_secs(5), "waited {:?}", started.elapsed());
    assert!(text.contains("started"), "{text:?}");
    assert!(gone(pid_in(&pid_file)), "the program was left running");
}

#[test]
fn a_program_that_never_exits_and_ignores_hangups_is_ended_with_everything_it_started() {
    let place = Temp::new("never");
    let (pid_file, kid_file) = (place.0.join("pid"), place.0.join("kid"));
    let mut run = sh(
        &format!(
            "trap '' HUP TERM INT; sleep 600 & echo $! > '{}'; echo $$ > '{}'; printf 'Quick safety check\\n'; while :; do sleep 1; done",
            kid_file.display(),
            pid_file.display()
        ),
        30,
    );
    run.stop_when = super::ASKS_THE_PERSON.into();
    let started = Instant::now();
    let text = drawn(run_in_pseudo_terminal(&run));
    // Stopped at the words, long before its thirty seconds.
    assert!(started.elapsed() < Duration::from_secs(10), "waited {:?}", started.elapsed());
    assert!(text.contains("Quick safety check"), "{text:?}");
    assert!(gone(pid_in(&pid_file)), "the program was left running");
    assert!(gone(pid_in(&kid_file)), "what it started was left running");
}

#[test]
fn stops_once_a_transcript_is_written_in_the_folder_and_no_longer_grows() {
    let place = Temp::new("written");
    let transcripts = place.0.join("projects");
    let pid_file = place.0.join("pid");
    let mut run = sh(
        &format!(
            "echo $$ > '{pid}'; printf 'Session resumed\\n'; mkdir -p '{dir}'; printf '{{}}\\n' > '{dir}/a.jsonl'; while :; do sleep 1; done",
            pid = pid_file.display(),
            dir = transcripts.display()
        ),
        60,
    );
    // The words come first and are not the signal.
    run.stop_when = "Quick safety check".into();
    run.stop_when_written = Some(transcripts.to_string_lossy().into_owned());
    let started = Instant::now();
    let text = drawn(run_in_pseudo_terminal(&run));
    assert!(started.elapsed() < Duration::from_secs(10), "waited {:?}", started.elapsed());
    assert!(text.contains("Session resumed"), "{text:?} after {:?}", started.elapsed());
    assert!(gone(pid_in(&pid_file)), "the program was left running");
}

#[test]
fn answers_the_terminals_one_question_where_the_cursor_is() {
    // A program that asks waits for the answer; without one it would sit out its seconds.
    let started = Instant::now();
    let text = drawn(run_in_pseudo_terminal(&sh(
        r#"stty -icanon -echo min 1; printf '\033[6n'; reply=$(dd bs=1 count=6 2>/dev/null); printf 'got:%s\n' "$reply" | tr '\033' E"#,
        10,
    )));
    assert!(text.contains("got:E[1;1R"), "{text:?}");
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn runs_nothing_without_words_to_run_or_with_a_stop_that_is_not_a_pattern() {
    let place = Temp::new("refused");
    let mark = place.0.join("ran");
    let mut empty = sh("", 5);
    empty.argv.clear();
    empty.line = "cmd.exe /d /c \"claude --teleport\"".into();
    assert!(matches!(run_in_pseudo_terminal(&empty), PseudoTerminalResult::Err));
    let mut bad = sh(&format!("touch '{}'", mark.display()), 5);
    bad.stop_when = "(unclosed".into();
    assert!(matches!(run_in_pseudo_terminal(&bad), PseudoTerminalResult::Err));
    assert!(!mark.exists());
}

// A transcript file that is there but still empty is not "written": Claude Code has not said anything yet.
#[test]
fn an_empty_transcript_is_not_taken_as_written() {
    let place = Temp::new("empty-transcript");
    let transcripts = place.0.join("projects");
    let mut run = sh(&format!("mkdir -p '{dir}'; : > '{dir}/a.jsonl'; while :; do sleep 1; done", dir = transcripts.display()), 4);
    run.stop_when_written = Some(transcripts.to_string_lossy().into_owned());
    let started = Instant::now();
    let _ = drawn(run_in_pseudo_terminal(&run));
    assert!(started.elapsed() >= Duration::from_millis(3500), "stopped after {:?} on an empty file", started.elapsed());
}

// The stop words are found through the terminal's colours: Claude Code draws its question with styling between the words.
#[test]
fn finds_the_stop_words_through_the_terminals_colours() {
    let mut run = sh(r"printf 'Quick \033[1msafety\033[0m check\n'; while :; do sleep 1; done", 20);
    run.stop_when = "Quick safety check".into();
    let started = Instant::now();
    let text = drawn(run_in_pseudo_terminal(&run));
    assert!(started.elapsed() < Duration::from_secs(8), "waited {:?}: {text:?}", started.elapsed());
}

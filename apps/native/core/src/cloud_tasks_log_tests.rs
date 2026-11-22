use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{Ran, leaving_no_log, reads_like_codex_log, run_leaving_no_log};

const CODEX: &str = "[2026-10-07T00:53:04.543477800+00:00] startup: base_url=https://chatgpt.com/backend-api path_style=wham\n[2026-10-07T00:53:04.544123400+00:00] auth: mode=ChatGPT account_id=0000\n";

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{n}", std::process::id()));
        fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn append_log(cwd: &Path, lines: &str) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(cwd.join("error.log"))
        .expect("open error.log");
    file.write_all(lines.as_bytes()).expect("append error.log");
}

#[test]
// case: a log the call made is gone afterwards, and the answer is unchanged
fn a_log_the_call_made_is_gone_afterwards_and_the_answer_is_unchanged() {
    let dir = TempDir::new("locust-cloud-log");
    let runner = leaving_no_log(|_args: &[&str], cwd: &Path| {
        append_log(cwd, CODEX);
        Ok(Ran::new(0, "ok", ""))
    });
    let ran = runner(&["cloud", "list"], &dir.path).expect("ran");
    assert_eq!(ran, Ran::new(0, "ok", ""));
    assert!(!dir.path.join("error.log").exists());
}

#[test]
// case: the person's own error.log keeps its lines; only Codex's are taken off
fn the_persons_own_error_log_keeps_its_lines_only_codexs_are_taken_off() {
    let dir = TempDir::new("locust-cloud-log-own");
    fs::write(dir.path.join("error.log"), "my app crashed\n").expect("write own log");
    let runner = leaving_no_log(|_args: &[&str], cwd: &Path| {
        append_log(cwd, CODEX);
        Ok(Ran::new(0, "ok", ""))
    });
    let _ = runner(&["cloud", "list"], &dir.path).expect("ran");
    assert_eq!(
        fs::read_to_string(dir.path.join("error.log")).expect("read log"),
        "my app crashed\n"
    );
}

#[test]
// case: what does not read like Codex is never removed
fn what_does_not_read_like_codex_is_never_removed() {
    let dir = TempDir::new("locust-cloud-log-other");
    let runner = leaving_no_log(|_args: &[&str], cwd: &Path| {
        append_log(cwd, "something else wrote this\n");
        Ok(Ran::new(0, "ok", ""))
    });
    let _ = runner(&["cloud", "list"], &dir.path).expect("ran");
    assert_eq!(
        fs::read_to_string(dir.path.join("error.log")).expect("read log"),
        "something else wrote this\n"
    );
}

#[test]
// case: also when the call fails, and with two calls on one folder at once
fn also_when_the_call_fails_and_with_two_calls_on_one_folder_at_once() {
    let dir = TempDir::new("locust-cloud-log-fail");
    let err = run_leaving_no_log(&["cloud", "status"], &dir.path, |_args, cwd| {
        append_log(cwd, CODEX);
        Err("codex went away".into())
    })
    .unwrap_err();
    assert!(err.contains("codex went away"));
    assert!(!dir.path.join("error.log").exists());

    let shared_dir = Arc::new(dir.path.clone());
    let handles: Vec<_> = (0..3)
        .map(|_| {
            let cwd = Arc::clone(&shared_dir);
            std::thread::spawn(move || {
                run_leaving_no_log(&["cloud", "list"], &cwd, |_args, dir| {
                    append_log(dir, CODEX);
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    Ok(Ran::new(0, "ok", ""))
                })
                .expect("concurrent run")
            })
        })
        .collect();
    for handle in handles {
        let _ = handle.join().expect("thread join");
    }
    assert!(!dir.path.join("error.log").exists());
}

#[test]
// case: recognises Codex's lines and nothing else
fn recognises_codexs_lines_and_nothing_else() {
    assert!(reads_like_codex_log(CODEX));
    assert!(!reads_like_codex_log(&format!("{CODEX}my own line\n")));
}

use super::*;
use std::io::Write;

const CODEX_LINE: &str = "[2026-10-07T00:53:04.543477800+00:00] startup: base_url=https://chatgpt.com/backend-api path_style=wham\n";

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("locust-sols-cleanup-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
        fs::create_dir(&dir).unwrap();
        Self(dir)
    }

    fn log(&self) -> PathBuf { self.0.join("error.log") }

    fn after_runner(&self, write: impl FnOnce(&mut fs::File)) {
        run_leaving_no_log(&["cloud", "list"], &self.0, |_, cwd| {
            let mut file = OpenOptions::new().create(true).append(true).open(cwd.join("error.log")).unwrap();
            write(&mut file);
            Ok(Ran::new(0, "", ""))
        }).unwrap();
    }
}
impl Drop for Scratch {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}

// case: 1. more than the check reads is never cut: Codex lines past 1 MiB, then the person's own, are all kept
#[test]
fn sweep_sol_cleanup_keeps_an_oversized_mixed_log_whole() {
    let scratch = Scratch::new();
    let many = CODEX_LINE.repeat(1_100_000 / CODEX_LINE.len() + 1) + "my app's own record\n";
    scratch.after_runner(|file| file.write_all(many.as_bytes()).unwrap());
    assert_eq!(fs::read_to_string(scratch.log()).unwrap(), many);
}

// case: 2. a line the person's program adds after the check is never cut
#[test]
fn sweep_sol_cleanup_preserves_an_append_after_the_initial_length_check() {
    let scratch = Scratch::new();
    fs::write(scratch.log(), "mine\n").unwrap();
    scratch.after_runner(|file| {
        file.write_all(CODEX_LINE.as_bytes()).unwrap();
        file.write_all(b"also mine\n").unwrap();
    });
    assert_eq!(fs::read_to_string(scratch.log()).unwrap(), format!("mine\n{CODEX_LINE}also mine\n"));
}

// case: the log a cleanup leaves for a call that made it is gone, as before
#[test]
fn sweep_sol_cleanup_removes_only_the_logs_made_by_this_call() {
    let scratch = Scratch::new();
    fs::write(scratch.0.join("neighbor.log"), "not Codex\n").unwrap();
    scratch.after_runner(|file| file.write_all(CODEX_LINE.as_bytes()).unwrap());
    assert_eq!(fs::metadata(scratch.log()).unwrap_err().kind(), std::io::ErrorKind::NotFound);
    assert_eq!(fs::read_to_string(scratch.0.join("neighbor.log")).unwrap(), "not Codex\n");
}

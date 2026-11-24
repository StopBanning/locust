//! a-cloud-session-is-read-only-once-at-a-time.test.ts (W4: "Never two reads of one session at once"). Fakes only:
//! Claude Code's teleport is a terminal that waits to be let go, and git answers without running.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::{ClaudeCloud, ClaudeLaunch, CloudPlatform, CloudReads, GitRan, PseudoTerminalResult, PseudoTerminalRun};

struct Root(PathBuf);

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn cloud<'a>(
    root: &Path,
    reads: &Arc<CloudReads>,
    calls: Arc<AtomicUsize>,
    release: Option<Arc<Mutex<mpsc::Receiver<()>>>>,
) -> ClaudeCloud<'a> {
    let git_dir = root.join(".git").to_string_lossy().into_owned();
    ClaudeCloud::new(root.join("sessions.json"), CloudPlatform::Win32, || {
        Some(ClaudeLaunch { discovered_path: "fake".into(), executable_path: "fake".into(), prefix_args: Vec::new(), env: None })
    })
    .with_reads(reads.clone())
    .with_git(move |_: &[&str], _: &str, _: Option<&str>| GitRan { code: 0, stdout: git_dir.clone(), stderr: String::new() })
    .with_terminal(move |_: &PseudoTerminalRun| {
        calls.fetch_add(1, Ordering::SeqCst);
        if let Some(release) = &release {
            let _ = release.lock().unwrap().recv();
        }
        PseudoTerminalResult::Ok { drawn: "Quick safety check".into() }
    })
}

fn waiting_on(reads: &CloudReads, id: &str) -> usize {
    reads.under_way.lock().unwrap().get(id).map_or(0, |read| read.waiting.load(Ordering::SeqCst))
}

fn until(what: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !what() {
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
// case: Overlapping reads share one teleport and the session lock is released afterward.
fn overlapping_reads_share_one_teleport_and_the_session_lock_is_released_afterward() {
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let root = Root(std::env::temp_dir().join(format!("locust-cloud-lock-{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed))));
    std::fs::create_dir_all(&root.0).unwrap();
    let sessions = serde_json::json!({ "sessions": [{ "id": "cc_abcdef", "folder": root.0.to_string_lossy(), "prompt": "Test",
        "startedAt": chrono::Utc::now().to_rfc3339(), "sessionId": "session_0123456789" }] });
    std::fs::write(root.0.join("sessions.json"), sessions.to_string()).unwrap();

    let reads = Arc::new(CloudReads::default());
    let calls = Arc::new(AtomicUsize::new(0));
    let (finish, wait) = mpsc::channel();
    let wait = Arc::new(Mutex::new(wait));
    let first = std::thread::spawn({
        let (path, reads, calls, wait) = (root.0.clone(), reads.clone(), calls.clone(), wait.clone());
        move || cloud(&path, &reads, calls, Some(wait)).check("cc_abcdef")
    });
    until(|| calls.load(Ordering::SeqCst) == 1);
    let second = std::thread::spawn({
        let (path, reads, calls) = (root.0.clone(), reads.clone(), calls.clone());
        move || cloud(&path, &reads, calls, None).check("cc_abcdef")
    });
    until(|| waiting_on(&reads, "cc_abcdef") == 1);
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    finish.send(()).unwrap();
    let first = first.join().unwrap();
    assert_eq!(second.join().unwrap(), first);
    cloud(&root.0, &reads, calls.clone(), None).check("cc_abcdef");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

use crate::voice_assets::{Asset, download};
use std::{
    fs,
    io::{self, Read},
    path::PathBuf,
    sync::{atomic::AtomicBool, atomic::Ordering},
};

const GOOD: Asset = Asset {
    url: "https://example.test/model",
    bytes: 4,
    sha256: "770e607624d689265ca6c44884d0807d9b054d23c473c106c72be9de08b7376c",
};

struct Folder(PathBuf);
impl Folder {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "locust-voice-unchecked-{}-{}",
            std::process::id(),
            crate::invariants::unique_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        Self(dir)
    }
    fn model(&self) -> PathBuf {
        self.0.join("model")
    }
    fn partial(&self) -> PathBuf {
        self.0.join("model.part")
    }
}
impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn response(data: &'static [u8]) -> impl FnOnce(&str) -> io::Result<Box<dyn Read>> {
    move |url| {
        assert_eq!(url, "https://example.test/model");
        Ok(Box::new(data))
    }
}

// case: refuses and deletes a SHA-256 mismatch, including stale cached files
#[test]
fn removes_same_length_cached_and_downloaded_wrong_digest() {
    let dir = Folder::new();
    fs::write(dir.model(), b"old!").unwrap();
    let error = download(
        GOOD,
        &dir.model(),
        &AtomicBool::new(false),
        response(b"evil"),
        |_| {},
    )
    .unwrap_err();
    assert!(error.to_string().contains("SHA-256"));
    assert!(!dir.model().exists());
    assert!(!dir.partial().exists());
}

// case: promotes only the checked file and reuses it without a connection
#[test]
fn promotes_verified_download_then_reuses_it_offline() {
    let dir = Folder::new();
    let cancel = AtomicBool::new(false);
    let mut progress = Vec::new();
    download(GOOD, &dir.model(), &cancel, response(b"good"), |bytes| {
        assert!(
            !dir.model().exists(),
            "must not publish bytes before checking the digest"
        );
        progress.push(bytes);
    })
    .unwrap();
    assert_eq!(fs::read(dir.model()).unwrap(), b"good");
    assert_eq!(progress, [4]);
    assert!(!dir.partial().exists());
    download(
        GOOD,
        &dir.model(),
        &cancel,
        |_| panic!("a checked file needs no network"),
        |bytes| {
            assert_eq!(bytes, 4);
        },
    )
    .unwrap();
    assert_eq!(fs::read(dir.model()).unwrap(), b"good");
}

// case: restarts an interrupted partial file cleanly
#[test]
fn removes_interrupted_partial_before_cancel_and_restarts() {
    let dir = Folder::new();
    fs::write(dir.partial(), b"interrupted").unwrap();
    let cancel = AtomicBool::new(true);
    let error = download(
        GOOD,
        &dir.model(),
        &cancel,
        |_| panic!("pre-cancelled"),
        |_| {},
    )
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::Interrupted);
    assert!(!dir.partial().exists());
    assert!(!dir.model().exists());
    cancel.store(false, Ordering::Release);
    download(GOOD, &dir.model(), &cancel, response(b"good"), |_| {}).unwrap();
    assert_eq!(fs::read(dir.model()).unwrap(), b"good");
}

// case: refuses an oversized response and removes it
#[test]
fn removes_partial_when_the_response_exceeds_the_pin() {
    let dir = Folder::new();
    let error = download(
        GOOD,
        &dir.model(),
        &AtomicBool::new(false),
        response(b"too large"),
        |_| {},
    )
    .unwrap_err();
    assert!(error.to_string().contains("larger than expected"));
    assert!(!dir.partial().exists());
    assert!(!dir.model().exists());
}

// case: runs a stand-in executable and returns its text
#[cfg(unix)]
#[test]
fn local_process_receives_the_whisper_flags_and_trims_its_words() {
    let mut command = std::process::Command::new("/bin/sh");
    command.args([
        "-c",
        r#"
        [ "$*" = '-m model -f speech.wav -t 2 -l en -ng -nt -np' ] || exit 9
        printf '  Local words.\n'
    "#,
        "voice-cli",
    ]);
    assert_eq!(
        super::run_local_cli(
            command,
            "speech.wav".as_ref(),
            "model".as_ref(),
            &AtomicBool::new(false),
            std::time::Duration::from_secs(3)
        )
        .unwrap(),
        "Local words."
    );
}

#[cfg(unix)]
fn hanging_cli(marker: &std::path::Path) -> std::process::Command {
    let mut cmd = std::process::Command::new("python3");
    // The same process writes a second receipt only if allowed to finish on its own.
    cmd.args([
        "-c",
        "import os, pathlib, sys, time; marker = pathlib.Path(sys.argv[1]); marker.write_text(str(os.getpid())); time.sleep(1.8); marker.with_extension('finished').write_text('finished')",
    ])
    .arg(marker);
    cmd
}

#[cfg(unix)]
fn gone(marker: &std::path::Path) {
    let pid = fs::read_to_string(marker).expect("the stand-in has started");
    let probe = std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg("kill -0 \"$1\" 2>/dev/null")
        .arg("probe")
        .arg(pid)
        .status()
        .unwrap();
    assert!(
        !probe.success(),
        "the worker must already have exited and been reaped"
    );
}

// case: stops a hung executable at its time limit and waits for termination
#[cfg(unix)]
#[test]
fn local_process_is_killed_and_reaped_at_its_deadline() {
    let dir = Folder::new();
    let marker = dir.0.join("pid");
    let error = super::run_local_cli(
        hanging_cli(&marker),
        "".as_ref(),
        "".as_ref(),
        &AtomicBool::new(false),
        std::time::Duration::from_secs(1),
    )
    .unwrap_err();
    assert!(error.contains("took too long"), "{error}");
    gone(&marker);
}

// case: cancels a running executable without returning text
#[cfg(unix)]
#[test]
fn local_process_cancel_stops_and_reaps_the_running_child() {
    let dir = Folder::new();
    let marker = dir.0.join("pid");
    let cancel = AtomicBool::new(false);
    std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            super::run_local_cli(
                hanging_cli(&marker),
                "".as_ref(),
                "".as_ref(),
                &cancel,
                std::time::Duration::from_secs(4),
            )
        });
        let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !fs::read_to_string(&marker).is_ok_and(|p| !p.is_empty()) {
            assert!(
                std::time::Instant::now() < until,
                "the stand-in did not start"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        cancel.store(true, Ordering::Release);
        let error = worker.join().unwrap().unwrap_err();
        assert!(error.contains("cancelled"), "{error}");
        gone(&marker);
        assert!(!marker.with_extension("finished").exists(), "cancelling must stop the running child before it finishes naturally");
    });
}

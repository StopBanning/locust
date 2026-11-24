use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Profile(PathBuf, Store);
impl Profile {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("locust-voice-preview-{}", unique_nanos()));
        let store =
            Store::with_codex(path.join("native"), path.join("electron"), Vec::new()).unwrap();
        fs::create_dir_all(store.root()).unwrap();
        Self(path, store)
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn webm(size: usize) -> Vec<u8> {
    let mut bytes = vec![7; size];
    bytes[..4].copy_from_slice(&[0x1a, 0x45, 0xdf, 0xa3]);
    bytes
}

fn wave() -> Vec<u8> {
    let mut bytes = vec![0; 200];
    bytes[..4].copy_from_slice(b"RIFF");
    bytes[8..12].copy_from_slice(b"WAVE");
    bytes
}

// case: keeps a short recording for one of the nine and gives it back whole
#[test]
fn a_voice_preview_reopens_whole() {
    let p = Profile::new();
    assert_eq!(saved(&p.1, "sol"), None);
    let expected = webm(2_000);
    save(&p.1, "sol", &expected).unwrap();
    assert_eq!(saved(&p.1, "sol"), Some(expected));
    assert_eq!(saved(&p.1, "cove"), None);
}

// case: refuses any other voice, and anything that is not a short WebM recording
#[test]
fn a_greeting_requires_a_known_voice_and_bounded_webm() {
    let p = Profile::new();
    for (voice, bytes) in [
        ("alloy", webm(2000)),
        ("../sol", webm(2000)),
        ("sol", webm(500)),
        ("sol", webm(1_600_000)),
        ("sol", vec![1; 2000]),
        ("sol", b"not audio".to_vec()),
    ] {
        assert!(save(&p.1, voice, &bytes).is_err());
    }
    assert_eq!(saved(&p.1, "../sol"), None);
    assert!(!p.1.root().join(DIRECTORY).exists());
    save(&p.1, "sol", &webm(2_000)).unwrap();
    fs::write(p.1.root().join(DIRECTORY).join("sol.webm"), b"wrong WebM").unwrap();
    assert_eq!(saved(&p.1, "sol"), None);
}

// case: makes the hello once, by one caller at a time, and again only after a failure
#[test]
fn the_first_successful_trigger_is_reused_even_by_concurrent_callers() {
    let p = Arc::new(Profile::new());
    let attempts = Arc::new(AtomicUsize::new(0));
    assert!(
        trigger(&p.1, |_| {
            attempts.fetch_add(1, Ordering::SeqCst);
            Err(io::Error::other("no speech here"))
        })
        .is_err()
    );
    let callers: Vec<_> = (0..2)
        .map(|_| {
            let p = p.clone();
            let attempts = attempts.clone();
            std::thread::spawn(move || {
                trigger(&p.1, |where_| {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    fs::write(where_, wave())
                })
                .unwrap()
            })
        })
        .collect();
    for caller in callers {
        assert_eq!(caller.join().unwrap(), wave());
    }
    assert_eq!(trigger(&p.1, |_| panic!("already cached")).unwrap(), wave());
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

// case: hands PowerShell its script encoded, so no quote in the path is ever parsed
#[test]
fn windows_receives_the_script_as_one_encoded_argument() {
    let target = Path::new("C:\\Users\\O'Brien\\AppData\\Locust\\hello.wav");
    let script = windows_script(target).unwrap();
    assert!(
        script
            .contains("SetOutputToWaveFile('C:\\Users\\O''Brien\\AppData\\Locust\\hello.wav', $f)")
    );
    assert!(script.contains("Speak('Hello?')"));
    let command = speak_command("win32", target).unwrap();
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(
        args[..3],
        ["-NoProfile", "-NonInteractive", "-EncodedCommand"]
    );
    assert_eq!(args.len(), 4);
    assert!(!args[3].to_str().unwrap().contains("O'Brien"));
}

// case: asks the Mac's own say for the preview's hello, and PowerShell on Windows
#[test]
fn the_hello_is_spoken_by_say_on_a_mac_and_powershell_on_windows() {
    let mac = speak_command("darwin", Path::new("/tmp/a b/hello.wav")).unwrap();
    assert_eq!(mac.get_program(), "/usr/bin/say");
    let args: Vec<_> = mac.get_args().collect();
    assert_eq!(
        args,
        [
            "-o",
            "/tmp/a b/hello.wav",
            "--file-format=WAVE",
            "--data-format=LEI16@24000",
            "Hello?"
        ]
    );
    let windows = speak_command("win32", Path::new("C:\\x\\hello.wav")).unwrap();
    assert_eq!(windows.get_program(), "powershell.exe");
    assert!(speak_command("linux", Path::new("/tmp/hello.wav")).is_none());
}

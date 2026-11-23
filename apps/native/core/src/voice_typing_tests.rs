use super::*;
use crate::own_models::NoBox;
use std::{fs, path::PathBuf, sync::atomic::AtomicUsize};

const KEY: &str = "fake-unit-key-not-a-credential";

struct Profile(PathBuf, Store);
impl Profile {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "locust-voice-typing-{}",
            crate::invariants::unique_nanos()
        ));
        let store = Store::with_codex(root.join("native"), root.join("electron"), vec![]).unwrap();
        fs::create_dir_all(store.root()).unwrap();
        Self(root, store)
    }
    fn voice(&self, platform: &'static str) -> VoiceSettings<'_> {
        VoiceSettings::new(&self.1, &BoxForTest, platform)
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct BoxForTest;
impl SecretBox for BoxForTest {
    fn available(&self) -> bool {
        true
    }
    fn seal(&self, text: &str) -> Result<String, String> {
        Ok(text.chars().rev().collect())
    }
    fn open(&self, sealed: &str) -> Result<String, String> {
        self.seal(sealed)
    }
}
struct Fake<F>(F);
impl<F> TranscriptionTransport for Fake<F>
where
    F: Fn(TranscriptionRequest<'_>) -> Result<(u16, Vec<u8>), ()> + Send + Sync,
{
    fn send(&self, request: TranscriptionRequest<'_>) -> Result<(u16, Vec<u8>), ()> {
        (self.0)(request)
    }
}
struct Offline;
impl TranscriptionTransport for Offline {
    fn send(&self, _: TranscriptionRequest<'_>) -> Result<(u16, Vec<u8>), ()> {
        panic!("voice sent a request without permission or a key")
    }
}

fn success(text: &str) -> Result<(u16, Vec<u8>), ()> {
    Ok((200, json!({"text":text}).to_string().into_bytes()))
}

fn sample() -> Vec<u8> {
    wav(&[0, 1, -1, i16::MAX, i16::MIN]).unwrap()
}

// case: defaults existing profiles to Fast, without creating anything
#[test]
fn windows_reads_fast_without_creating_the_settings_file() {
    let p = Profile::new();
    assert_eq!(
        p.voice("win32").settings(),
        Settings {
            mode: Mode::Fast,
            has_key: false,
            openai_consent: false
        }
    );
    assert!(!p.1.root().join(FILE).exists());
}

// case: opens a Mac on your OpenAI account, where Fast and Accurate cannot run, and reads a stored local choice so
#[test]
fn mac_reads_a_kept_local_choice_as_openai() {
    let p = Profile::new();
    assert_eq!(p.voice("darwin").settings().mode, Mode::OpenAi);
    p.voice("win32").save(&json!({"mode":"accurate"})).unwrap();
    assert_eq!(p.voice("darwin").settings().mode, Mode::OpenAi);
    assert_eq!(p.voice("win32").settings().mode, Mode::Accurate);
}

// case: round trips all three choices through a reopened store
#[test]
fn all_modes_reopen() {
    let p = Profile::new();
    for mode in [Mode::Accurate, Mode::OpenAi, Mode::Fast] {
        p.voice("win32")
            .save(&json!({"mode":mode.as_str()}))
            .unwrap();
        assert_eq!(p.voice("win32").settings().mode, mode);
    }
}

// case: returns only saved status, never the key or ciphertext, even on consent/save
#[test]
fn public_voice_settings_and_backups_exclude_the_credential() {
    let p = Profile::new();
    let voice = p.voice("win32");
    let saved = voice.save(&json!({"mode":"openai","key":KEY})).unwrap();
    let allowed = voice.consent().unwrap();
    let reopened = p.voice("win32").settings();
    let shown = format!("{saved:?}{allowed:?}{reopened:?}");
    assert!(!shown.contains(KEY));
    assert!(!shown.contains(&KEY.chars().rev().collect::<String>()));
    assert!(
        !fs::read_to_string(p.1.root().join(FILE))
            .unwrap()
            .contains(KEY)
    );
    assert_eq!(voice.open_key(&voice.read()).as_deref(), Some(KEY));
    assert!(reopened.openai_consent);
    assert!(!crate::profile_backup::carries(FILE));
}

// case: refuses plaintext storage when OS encryption is unavailable
#[test]
fn no_key_storage_without_os_protection() {
    let p = Profile::new();
    let voice = VoiceSettings::new(&p.1, &NoBox, "darwin");
    assert!(
        voice
            .save(&json!({"key":KEY}))
            .unwrap_err()
            .contains("not saved")
    );
    assert!(!voice.settings().has_key);
    assert!(!p.1.root().join(FILE).exists());
}

// case: key removal clears consent and never echoes an invalid input
#[test]
fn removing_the_key_clears_consent_and_invalid_values_stay_private() {
    let p = Profile::new();
    let voice = p.voice("win32");
    voice.save(&json!({"mode":"openai","key":KEY})).unwrap();
    voice.consent().unwrap();
    voice.save(&json!({"key":""})).unwrap();
    assert_eq!(
        voice.settings(),
        Settings {
            mode: Mode::OpenAi,
            has_key: false,
            openai_consent: false
        }
    );
    assert!(
        !voice
            .save(&json!({"key":format!("{KEY}\n")}))
            .unwrap_err()
            .contains(KEY)
    );
    assert!(
        voice
            .save(&json!({"mode":"https://elsewhere.example.test"}))
            .is_err()
    );
}

// case: cannot consent without the selected OpenAI mode and saved key
#[test]
fn consent_needs_mode_and_key() {
    let p = Profile::new();
    let voice = p.voice("win32");
    assert!(voice.consent().is_err());
    voice.save(&json!({"key":KEY})).unwrap();
    assert!(voice.consent().is_err());
}

// case: main refuses upload before consent, and stale local capture cannot switch to cloud
#[test]
fn rejects_an_upload_without_consent_or_from_a_stale_local_capture() {
    let p = Profile::new();
    let voice = p.voice("win32");
    let cancel = AtomicBool::new(false);
    voice.save(&json!({"mode":"openai","key":KEY})).unwrap();
    assert!(
        voice
            .transcribe(&sample(), Mode::OpenAi, &cancel, &Offline)
            .is_err()
    );
    voice.consent().unwrap();
    assert!(
        voice
            .transcribe(&sample(), Mode::Fast, &cancel, &Offline)
            .is_err()
    );
    let called = AtomicUsize::new(0);
    let fake = Fake(|_: TranscriptionRequest<'_>| {
        called.fetch_add(1, Ordering::Relaxed);
        success("Allowed words.")
    });
    assert_eq!(
        voice
            .transcribe(&sample(), Mode::OpenAi, &cancel, &fake)
            .unwrap(),
        "Allowed words."
    );
    assert_eq!(called.load(Ordering::Relaxed), 1);
}

// case: sends a multipart WAV and model, with the key only in Authorization
#[test]
fn sends_the_recording_only_to_the_fixed_endpoint() {
    let wav = sample();
    let called = AtomicUsize::new(0);
    let fake = Fake(|req: TranscriptionRequest<'_>| {
        called.fetch_add(1, Ordering::Relaxed);
        assert_eq!(req.url, "https://api.openai.com/v1/audio/transcriptions");
        assert_eq!(req.authorization, format!("Bearer {KEY}"));
        assert_eq!(req.timeout_seconds, 120);
        assert!(req.body.windows(wav.len()).any(|window| window == wav));
        let body = String::from_utf8_lossy(&req.body);
        for section in [
            "name=\"file\"; filename=\"speech.wav\"\r\nContent-Type: audio/wav",
            "name=\"model\"\r\n\r\ngpt-4o-transcribe",
            "name=\"response_format\"\r\n\r\njson",
            "name=\"language\"\r\n\r\nen",
        ] {
            assert!(body.contains(section));
        }
        assert!(!body.contains(KEY));
        assert!(body.starts_with(&format!("--{}\r\n", req.multipart_boundary)));
        Ok((
            200,
            json!({"text":"Editable words.","extra":KEY})
                .to_string()
                .into_bytes(),
        ))
    });
    assert_eq!(
        transcribe_openai(&wav, Some(KEY), &AtomicBool::new(false), &fake).unwrap(),
        "Editable words."
    );
    assert_eq!(called.load(Ordering::Relaxed), 1);
}

// case: missing key makes no request
#[test]
fn no_key_blocks_the_request() {
    assert!(
        transcribe_openai(&sample(), None, &AtomicBool::new(false), &Offline)
            .unwrap_err()
            .contains("Add your OpenAI API key")
    );
}

// case: status %s returns no text and never leaks the raw body
#[test]
fn failures_never_return_the_http_body() {
    for status in [401, 403, 429, 500] {
        let err = transcribe_openai(
            &sample(),
            Some(KEY),
            &AtomicBool::new(false),
            &Fake(|_: TranscriptionRequest<'_>| Ok((status, KEY.as_bytes().to_vec()))),
        )
        .unwrap_err();
        assert!(!err.contains(KEY));
        assert!(err.contains("Your message is unchanged"));
        assert_eq!(
            err.contains("refused your API key"),
            status == 401 || status == 403
        );
    }
}

// case: does not echo fetch exceptions or a key reflected as transcript
#[test]
fn reflected_credentials_and_request_errors_are_discarded() {
    for fake in [
        Fake((|_: TranscriptionRequest<'_>| Err(())) as fn(TranscriptionRequest<'_>) -> _),
        Fake(|_| success(KEY)),
    ] {
        let error =
            transcribe_openai(&sample(), Some(KEY), &AtomicBool::new(false), &fake).unwrap_err();
        assert!(!error.contains(KEY));
    }
}

// case: cancellation discards a late fake response
#[test]
fn a_cancellation_wins_over_a_returned_transcript() {
    let cancel = AtomicBool::new(false);
    let fake = Fake(|request: TranscriptionRequest<'_>| {
        request.cancel.store(true, Ordering::Release);
        success("Too late.")
    });
    assert_eq!(
        transcribe_openai(&sample(), Some(KEY), &cancel, &fake),
        Err(CANCELLED.into())
    );
}

// case: types through the OpenAI route once allowed, as on Windows
#[test]
fn mac_transcription_uses_the_same_consented_route() {
    let p = Profile::new();
    let voice = p.voice("darwin");
    voice.save(&json!({"mode":"openai","key":KEY})).unwrap();
    assert!(!voice.ready(Mode::OpenAi));
    voice.consent().unwrap();
    assert!(voice.ready(Mode::OpenAi));
    let fake = Fake(|_: TranscriptionRequest<'_>| success("Typed on a Mac."));
    assert_eq!(
        voice
            .transcribe(&sample(), Mode::OpenAi, &AtomicBool::new(false), &fake)
            .unwrap(),
        "Typed on a Mac."
    );
}

// case: accepts only bounded canonical 16-kHz mono PCM
#[test]
fn wav_must_be_short_mono_pcm() {
    let valid = wav(&[0; SAMPLE_RATE]).unwrap();
    assert!(valid_wav(&valid));
    let mut stereo = valid.clone();
    stereo[22] = 2;
    assert!(!valid_wav(&stereo));
    assert!(!valid_wav(b"speech.wav"));
    assert!(!valid_wav(&vec![0; 2_000_000]));
}


// case: says Fast and Accurate run on Windows, and names the Mac’s own dictation, downloading nothing
#[test]
fn mac_refuses_both_local_modes_without_opening_or_downloading_anything() {
    let profile = Profile::new();
    for mode in [Mode::Fast, Mode::Accurate] {
        profile
            .voice("win32")
            .save(&json!({"mode":mode.as_str()}))
            .unwrap();
        let mac = profile.voice("darwin");
        assert!(!mac.ready(mode));
        let refusal = mac
            .transcribe(&sample(), mode, &AtomicBool::new(false), &Offline)
            .unwrap_err();
        assert!(refusal.starts_with(
            "Fast and Accurate voice typing run on Windows for now. On a Mac, use its own dictation here"
        ));
        assert!(!profile.1.root().join("voice").exists());
    }
}

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

// case: inserts at the cursor, replaces a selection, and preserves surrounding edits
#[test]
fn words_go_in_at_the_cursor_or_over_the_selection_within_the_limit() {
    assert_eq!(
        insert_text("before  after", "words", 7, 7, 8000),
        ("before words after".to_owned(), 12)
    );
    assert_eq!(
        insert_text("before old after", "new", 7, 10, 8000),
        ("before new after".to_owned(), 10)
    );
    assert_eq!(insert_text("abcd", "123456", 2, 2, 6), ("ab12cd".to_owned(), 4));
}

// Same case as wav_must_be_short_mono_pcm, at its edge: a minute of speech is a WAV, one sample more is refused.
#[test]
fn wav_is_refused_one_sample_past_a_minute() {
    assert!(wav(&vec![0; MAX_PCM_SAMPLES]).is_some());
    assert!(wav(&vec![0; MAX_PCM_SAMPLES + 1]).is_none());
}

/// The app's real HTTPS client (voice_http), pointed at a stand-in on this machine instead of OpenAI's address.
struct Here {
    url: &'static str,
    asked: Mutex<Vec<&'static str>>,
}
impl Here {
    fn at(url: String) -> Self {
        Self { url: Box::leak(url.into_boxed_str()), asked: Mutex::default() }
    }
}
impl TranscriptionTransport for Here {
    fn send(&self, request: TranscriptionRequest<'_>) -> Result<(u16, Vec<u8>), ()> {
        self.asked.lock().unwrap().push(request.url);
        crate::voice_http::UreqTranscriptionTransport::new()
            .send(TranscriptionRequest { url: self.url, ..request })
    }
}

/// A stand-in for OpenAI that answers one request with `reply` and hands back its head (lower-cased) and body.
fn stand_in(reply: String) -> (String, std::thread::JoinHandle<(String, Vec<u8>)>) {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/audio/transcriptions", listener.local_addr().unwrap());
    let seen = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let (mut head, mut length) = (String::new(), 0usize);
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line.trim_end().is_empty() {
                break;
            }
            let line = line.to_ascii_lowercase();
            if let Some(value) = line.strip_prefix("content-length:") {
                length = value.trim().parse().unwrap();
            }
            head.push_str(&line);
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        stream.write_all(reply.as_bytes()).unwrap();
        (head, body)
    });
    (url, seen)
}

fn answer(status: &str, body: &str) -> String {
    format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len())
}

fn allowed_mac(p: &Profile) -> VoiceSettings<'_> {
    let voice = p.voice("darwin");
    voice.save(&json!({"mode":"openai","key":KEY})).unwrap();
    voice.consent().unwrap();
    voice
}

// case: types through the OpenAI route once allowed, as on Windows
#[test]
fn a_mac_types_through_the_apps_own_https_client_with_electrons_request() {
    let p = Profile::new();
    let voice = allowed_mac(&p);
    let (url, seen) = stand_in(answer("200 OK", r#"{"text":"Typed on a Mac."}"#));
    let here = Here::at(url);
    let typed = voice.transcribe(&sample(), Mode::OpenAi, &AtomicBool::new(false), &here);
    assert_eq!(typed.unwrap(), "Typed on a Mac.");
    assert_eq!(*here.asked.lock().unwrap(), ["https://api.openai.com/v1/audio/transcriptions"]);
    let (head, body) = seen.join().unwrap();
    let body = String::from_utf8_lossy(&body);
    assert!(head.starts_with("post /v1/audio/transcriptions http/1.1"), "{head}");
    assert!(head.contains(&format!("authorization: bearer {KEY}\r\n")), "{head}");
    assert!(head.contains("content-type: multipart/form-data; boundary=locust-"), "{head}");
    assert!(!head.contains("cookie:"), "{head}");
    for field in [
        "name=\"file\"; filename=\"speech.wav\"\r\nContent-Type: audio/wav",
        "name=\"model\"\r\n\r\ngpt-4o-transcribe\r\n",
        "name=\"response_format\"\r\n\r\njson\r\n",
        "name=\"language\"\r\n\r\nen\r\n",
    ] {
        assert!(body.contains(field), "missing {field:?}");
    }
}

#[test]
fn the_apps_own_client_says_openais_refusals_in_electrons_words() {
    let p = Profile::new();
    let voice = allowed_mac(&p);
    let cancel = AtomicBool::new(false);
    for (status, said) in [
        ("401 Unauthorized", "OpenAI refused your API key. Check it in Settings > General. Your message is unchanged."),
        ("403 Forbidden", "OpenAI refused your API key. Check it in Settings > General. Your message is unchanged."),
        ("500 Internal Server Error", "OpenAI could not transcribe this recording. Check your API account and try again. Your message is unchanged."),
    ] {
        let (url, seen) = stand_in(answer(status, r#"{"error":{"message":"no"}}"#));
        let refused = voice.transcribe(&sample(), Mode::OpenAi, &cancel, &Here::at(url));
        assert_eq!(refused.unwrap_err(), said, "{status}");
        seen.join().unwrap();
    }
    // Nobody answering is "could not be reached", in Electron's words.
    let gone = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/audio/transcriptions", gone.local_addr().unwrap());
    drop(gone);
    let unreached = voice.transcribe(&sample(), Mode::OpenAi, &cancel, &Here::at(url));
    assert_eq!(unreached.unwrap_err(), "OpenAI could not be reached. Your message is unchanged; try again.");
}

#[test]
fn the_apps_own_client_never_carries_the_key_past_a_redirect_or_sends_a_long_recording() {
    let p = Profile::new();
    let voice = allowed_mac(&p);
    let cancel = AtomicBool::new(false);
    let elsewhere = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    elsewhere.set_nonblocking(true).unwrap();
    let moved = format!(
        "HTTP/1.1 302 Found\r\nLocation: http://{}/v1/audio/transcriptions\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        elsewhere.local_addr().unwrap()
    );
    let (url, seen) = stand_in(moved);
    let redirected = voice.transcribe(&sample(), Mode::OpenAi, &cancel, &Here::at(url));
    seen.join().unwrap();
    assert_eq!(redirected.unwrap_err(), UNREACHABLE);
    assert!(elsewhere.accept().is_err(), "the key followed the redirect");
    // Over a minute is refused before anything is sent.
    let too_long = wav(&vec![0; MAX_PCM_SAMPLES]).unwrap();
    let mut longer = too_long.clone();
    longer.extend_from_slice(&[0, 0]);
    let closed = format!("http://{}/v1/audio/transcriptions", elsewhere.local_addr().unwrap());
    drop(elsewhere);
    let here = Here::at(closed);
    assert_eq!(
        voice.transcribe(&longer, Mode::OpenAi, &cancel, &here).unwrap_err(),
        "Voice typing needs a recording of at most one minute. Your message is unchanged."
    );
    assert!(here.asked.lock().unwrap().is_empty());
}

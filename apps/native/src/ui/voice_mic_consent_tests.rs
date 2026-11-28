use super::*;
use locust_core::{
    own_models::SecretBox, voice_audio::Playback, voice_typing::TranscriptionRequest,
};
use rune::{Env, Runtime};
use std::{path::PathBuf, sync::atomic::AtomicUsize, time::Instant};

struct Profile(PathBuf);
impl Drop for Profile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Secrets;
impl SecretBox for Secrets {
    fn available(&self) -> bool {
        true
    }
    fn seal(&self, _: &str) -> Result<String, String> {
        Ok("local-handle".into())
    }
    fn open(&self, _: &str) -> Result<String, String> {
        Ok("made-up-test-only".into())
    }
}

struct Mic;
impl Recording for Mic {
    fn finish(self: Box<Self>) -> Result<Vec<i16>, String> {
        Ok(vec![1, 2, 3])
    }
}
struct Recorder(Arc<AtomicUsize>);
impl Audio for Recorder {
    fn record(&self, _: &AtomicBool) -> Result<Box<dyn Recording>, String> {
        self.0.fetch_add(1, Ordering::Release);
        Ok(Box::new(Mic))
    }
    fn call(&self, _: &AtomicBool) -> Result<Box<dyn AudioPeer>, String> {
        unreachable!()
    }
    fn play(&self, _: &[u8]) -> Result<Box<dyn Playback>, String> {
        unreachable!()
    }
}

struct NoUpload;
impl TranscriptionTransport for NoUpload {
    fn send(&self, _: TranscriptionRequest<'_>) -> Result<(u16, Vec<u8>), ()> {
        panic!("recording has not been stopped; nothing may upload")
    }
}

fn drawing(s: &State, _: &mut Cx<State>) -> El {
    let p = Palette::new(true);
    col()
        .gap(10.0)
        .padding(10.0)
        .children(controls(s, &p))
        .children(notices(s, &p))
}

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("missing {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
}

fn wait(rt: &mut Runtime<State>, key: &str) {
    let until = Instant::now() + Duration::from_secs(5);
    while rt.find(key).is_none() {
        rt.frame(0.05);
        assert!(
            Instant::now() < until,
            "missing {key} after microphone reply"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

// case: asks to allow OpenAI there, as ever
#[test]
fn voice_typing_only_records_after_accepting_the_local_cost_and_upload_notice() {
    let profile = Profile(std::env::temp_dir().join(format!(
        "locust-voice-consent-{}",
        locust_core::invariants::unique_nanos()
    )));
    let store = store::Store::new(profile.0.join("native"), profile.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.root()).unwrap();
    let settings = VoiceSettings::new(&store, &Secrets, "darwin");
    settings
        .save(&serde_json::json!({"key": "made-up-test-only", "mode": "openai"}))
        .unwrap();
    let opened = Arc::new(AtomicUsize::new(0));
    let mut s = State::default();
    s.data = Some(store.clone());
    s.voice.platform = Some("darwin");
    s.voice.secrets = Some(Arc::new(Secrets));
    s.voice.transcriber = Some(Arc::new(NoUpload));
    s.voice.audio = Some(Arc::new(Recorder(opened.clone())));
    let mut rt = Runtime::new(s, drawing, (850.0, 450.0), Env::new(true, 1.0));
    rt.poll = Some(tick);
    rt.frame(0.05);

    click(&mut rt, "voice-typing");
    wait(&mut rt, "voice-consent-allow");
    assert!(rt.elements().into_iter().any(|e| {
        !e.exiting && e.text.as_deref() == Some("Voice typing will send your recordings to OpenAI using your API key. OpenAI API charges apply. Allow this for future recordings?")
    }));
    assert_eq!(opened.load(Ordering::Acquire), 0);
    assert!(!settings.settings().openai_consent);
    click(&mut rt, "voice-consent-not-now");
    assert!(rt.state.voice.consent_for.is_none());
    assert_eq!(opened.load(Ordering::Acquire), 0);
    assert!(!settings.settings().openai_consent);

    click(&mut rt, "voice-typing");
    wait(&mut rt, "voice-consent-allow");
    click(&mut rt, "voice-consent-allow");
    assert!(
        settings.settings().openai_consent,
        "the actual Allow click must persist consent"
    );
    let until = Instant::now() + Duration::from_secs(5);
    while !matches!(
        rt.state.voice.capture.as_ref().map(|c| &c.state),
        Some(CaptureState::Recording(..))
    ) {
        rt.frame(0.05);
        assert!(Instant::now() < until, "Allow must open the microphone");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(opened.load(Ordering::Acquire), 1);
    assert!(rt.state.draft.text.is_empty());
}

use super::*;
use locust_core::{
    own_models::SecretBox, voice_audio::Playback, voice_typing::TranscriptionRequest,
};
use rune::{Env, Runtime};
use serde_json::json;
use std::{path::PathBuf, sync::atomic::AtomicUsize};

const WAIT: Duration = Duration::from_secs(60);

struct Profile(PathBuf);
impl Drop for Profile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Keychain;
impl SecretBox for Keychain {
    fn available(&self) -> bool {
        true
    }
    fn seal(&self, _: &str) -> Result<String, String> {
        Ok("fake-sealed-token".into())
    }
    fn open(&self, _: &str) -> Result<String, String> {
        Ok("synthetic-only".into())
    }
}

#[derive(Default)]
struct Activity {
    opened: AtomicUsize,
    finished: AtomicUsize,
    released: AtomicUsize,
}

struct Mic(Arc<Activity>);
impl Drop for Mic {
    fn drop(&mut self) {
        self.0.released.fetch_add(1, Ordering::Relaxed);
    }
}
impl Recording for Mic {
    fn finish(self: Box<Self>) -> Result<Vec<i16>, String> {
        self.0.finished.fetch_add(1, Ordering::Relaxed);
        Ok(vec![1, 2, -3])
    }
}

struct AudioFixture(Arc<Activity>);
impl Audio for AudioFixture {
    fn record(&self, _: &AtomicBool) -> Result<Box<dyn Recording>, String> {
        self.0.opened.fetch_add(1, Ordering::Relaxed);
        Ok(Box::new(Mic(self.0.clone())))
    }
    fn call(&self, _: &AtomicBool) -> Result<Box<dyn AudioPeer>, String> {
        Err("No peer in this fixture.".into())
    }
    fn play(&self, _: &[u8]) -> Result<Box<dyn Playback>, String> {
        Err("No speaker in this fixture.".into())
    }
}

struct Words {
    status: u16,
    called: AtomicUsize,
    wait: Option<std::sync::Mutex<Receiver<()>>>,
}

impl TranscriptionTransport for Words {
    fn send(&self, _: TranscriptionRequest<'_>) -> Result<(u16, Vec<u8>), ()> {
        self.called.fetch_add(1, Ordering::Release);
        if let Some(wait) = &self.wait {
            wait.lock().unwrap().recv_timeout(WAIT).unwrap();
        }
        Ok((self.status, br#"{"text":"Local words."}"#.to_vec()))
    }
}

fn state(platform: &'static str, with_key: bool, consent: bool) -> (State, Profile, Arc<Activity>) {
    let root = std::env::temp_dir().join(format!(
        "locust-native-voice-window-{}",
        locust_core::invariants::unique_nanos()
    ));
    let store =
        store::Store::with_codex(root.join("native"), root.join("electron"), vec![]).unwrap();
    std::fs::create_dir_all(store.root()).unwrap();
    let settings = VoiceSettings::new(&store, &Keychain, platform);
    let mut change = json!({"mode":"openai"});
    if with_key {
        change["key"] = json!("synthetic-only");
    }
    settings.save(&change).unwrap();
    if consent {
        settings.consent().unwrap();
    }
    let activity = Arc::new(Activity::default());
    let mut s = State::default();
    s.data = Some(store);
    s.voice.platform = Some(platform);
    s.voice.audio = Some(Arc::new(AudioFixture(activity.clone())));
    s.voice.secrets = Some(Arc::new(Keychain));
    s.home.folder_path = Some(root.to_string_lossy().into_owned());
    (s, Profile(root), activity)
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    let palette = Palette::new(true);
    col()
        .gap(10.0)
        .padding(10.0)
        .children(controls(s, &palette))
        .children(notices(s, &palette))
        .child(
            text_input(&s.draft, type_::BODY)
                .key("draft")
                .on_edit(|s: &mut State| &mut s.draft),
        )
}

fn window(s: State) -> Runtime<State> {
    let mut rt = Runtime::new(s, view, (750.0, 500.0), Env::new(true, 1.0));
    rt.on_key = Some(on_key);
    rt.poll = Some(tick);
    rt.frame(0.0);
    rt
}

fn click(rt: &mut Runtime<State>, key: &str) {
    rt.frame(0.05);
    let (x, y) = rt.find(key).unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
}

fn pump(rt: &mut Runtime<State>, what: &str, mut condition: impl FnMut(&State) -> bool) {
    let mut time = 0.0;
    let until = std::time::Instant::now() + WAIT;
    loop {
        time += 0.05;
        rt.frame(time);
        if condition(&rt.state) {
            return;
        }
        assert!(
            std::time::Instant::now() < until,
            "timed out waiting for {what}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn open(rt: &mut Runtime<State>) {
    click(rt, "voice-typing");
    pump(rt, "the microphone to record", |s| {
        s.voice
            .capture
            .as_ref()
            .is_some_and(|capture| matches!(capture.state, CaptureState::Recording(..)))
    });
}

fn words(status: u16) -> Arc<Words> {
    Arc::new(Words {
        status,
        called: AtomicUsize::new(0),
        wait: None,
    })
}

// case: does not draw a microphone on %s
#[test]
fn unsupported_systems_do_not_offer_audio() {
    for platform in ["linux", "unknown"] {
        let (s, _profile, _) = state(platform, false, false);
        assert!(window(s).find("voice-typing").is_none());
    }
}

// case: draws it on a Mac too
#[test]
fn mac_draws_voice_typing_and_talk_together() {
    let (s, _profile, _) = state("darwin", false, false);
    let rt = window(s);
    let mic = rt.find("voice-typing").unwrap();
    let talk = rt.find("talk").unwrap();
    assert!(mic.0 < talk.0);
    assert!((mic.1 - talk.1).abs() < 1.0);
}

// case: draws it on Windows and loads nothing on mount
#[test]
fn drawing_audio_controls_does_not_open_the_microphone() {
    let (mut s, _profile, activity) = state("win32", true, true);
    let http = words(200);
    s.voice.transcriber = Some(http.clone());
    assert!(window(s).find("voice-typing").is_some());
    assert_eq!(activity.opened.load(Ordering::Relaxed), 0);
    assert_eq!(http.called.load(Ordering::Relaxed), 0);
}

// case: Esc cancels recording and inserts nothing
#[test]
fn escape_releases_the_recording() {
    let (mut s, _profile, activity) = state("darwin", true, true);
    let http = words(200);
    s.voice.transcriber = Some(http.clone());
    let mut rt = window(s);
    open(&mut rt);
    rt.key(Key::Escape.into());
    rt.frame(0.10);
    assert!(rt.state.voice.capture.is_none());
    assert_eq!(activity.released.load(Ordering::Relaxed), 1);
    assert_eq!(http.called.load(Ordering::Relaxed), 0);
    assert!(rt.state.draft.text.is_empty());
}

// case: Esc while transcription is pending discards even a late success
#[test]
fn escape_during_transcription_discards_a_late_result() {
    let (mut s, _profile, _) = state("darwin", true, true);
    let (resume, waiting) = mpsc::channel();
    let http = Arc::new(Words {
        status: 200,
        called: AtomicUsize::new(0),
        wait: Some(std::sync::Mutex::new(waiting)),
    });
    s.voice.transcriber = Some(http.clone());
    let mut rt = window(s);
    open(&mut rt);
    click(&mut rt, "voice-typing");
    pump(&mut rt, "fake transcription", |_| {
        http.called.load(Ordering::Acquire) == 1
    });
    rt.key(Key::Escape.into());
    resume.send(()).unwrap();
    rt.frame(0.15);
    assert!(rt.state.voice.capture.is_none());
    assert!(rt.state.draft.text.is_empty());
}

// case: a second press inserts returned words once, not a send
#[test]
fn the_second_press_inserts_only_in_the_draft() {
    let (mut s, _profile, activity) = state("darwin", true, true);
    let http = words(200);
    s.voice.transcriber = Some(http.clone());
    s.draft = rune::TextEdit::new("before old after");
    s.draft.anchor = 7;
    s.draft.focus = 10;
    let mut rt = window(s);
    open(&mut rt);
    click(&mut rt, "voice-typing");
    pump(&mut rt, "words inserted into the draft", |s| {
        s.voice.capture.is_none()
    });
    assert_eq!(rt.state.draft.text, "before Local words. after");
    assert_eq!(activity.finished.load(Ordering::Relaxed), 1);
    assert_eq!(http.called.load(Ordering::Relaxed), 1);
    assert!(!rt.state.sending.is_pending());
}

// case: a refused OpenAI key leaves the existing draft alone
#[test]
fn a_refusal_preserves_the_existing_draft() {
    let (mut s, _profile, _) = state("darwin", true, true);
    s.voice.transcriber = Some(words(401));
    s.draft = rune::TextEdit::new("leave me");
    let mut rt = window(s);
    open(&mut rt);
    click(&mut rt, "voice-typing");
    pump(&mut rt, "refusal", |s| s.voice.capture.is_none());
    assert_eq!(rt.state.draft.text, "leave me");
    assert!(
        rt.state
            .voice
            .note
            .as_deref()
            .unwrap()
            .contains("refused your API key")
    );
}

// case: unconsented cloud and a missing key never open the microphone
#[test]
fn cloud_capture_waits_for_key_and_consent() {
    for has_key in [false, true] {
        let (mut s, _profile, activity) = state("darwin", has_key, false);
        s.voice.transcriber = Some(words(200));
        let mut rt = window(s);
        click(&mut rt, "voice-typing");
        pump(&mut rt, "a setup message", |s| s.voice.note.is_some());
        assert_eq!(activity.opened.load(Ordering::Relaxed), 0);
    }
}

// case: inserts at the cursor, replaces a selection, and preserves surrounding edits
#[test]
fn spoken_words_go_in_at_the_cursor_within_the_prompt_limit() {
    let (mut s, _profile, _) = state("darwin", true, true);
    s.voice.transcriber = Some(words(200));
    s.draft = rune::TextEdit::new("before  after");
    s.draft.set_caret(7);
    let mut rt = window(s);
    open(&mut rt);
    click(&mut rt, "voice-typing");
    pump(&mut rt, "words at the cursor", |s| s.voice.capture.is_none());
    assert_eq!(rt.state.draft.text, "before Local words. after");
    assert_eq!(rt.state.draft.range(), (19, 19));

    // 8,000 is Electron's MAX_PROMPT_LENGTH: a box five short of it takes five characters of what was said.
    let (mut s, _profile, _) = state("darwin", true, true);
    s.voice.transcriber = Some(words(200));
    s.draft = rune::TextEdit::new("x".repeat(7_995));
    s.draft.set_caret(0);
    let mut rt = window(s);
    open(&mut rt);
    click(&mut rt, "voice-typing");
    pump(&mut rt, "words cut to the limit", |s| s.voice.capture.is_none());
    assert_eq!(rt.state.draft.text, format!("Local{}", "x".repeat(7_995)));
}

fn a11y<'a>(tree: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    if tree["key"] == key {
        return Some(tree);
    }
    tree["children"].as_array()?.iter().find_map(|child| a11y(child, key))
}

// case: is a button named Talk, saying what the call can and cannot do
#[test]
fn talk_is_a_button_saying_what_a_call_can_and_cannot_do() {
    let (s, _profile, _) = state("win32", false, false);
    let rt = window(s);
    let talk = rt
        .elements()
        .into_iter()
        .find(|item| item.key.as_deref() == Some("talk"))
        .unwrap();
    assert_eq!(talk.role, "button");
    assert_eq!(talk.text.as_deref(), Some("Talk"));
    let tree = rt.accessibility_json();
    let node = a11y(&tree, "talk").unwrap();
    assert_eq!(
        node["description"],
        "Talk with ChatGPT’s voice. It can read this folder and change nothing."
    );
    assert!(node["states"].as_array().unwrap().iter().any(|state| state == "unchecked"));
    // No strip until a call starts.
    assert!(!rt.elements().into_iter().any(|item| {
        item.text.as_deref() == Some("Voice call") || item.label.as_deref() == Some("Voice call")
    }));
}

// case: is not there where no call can be placed
#[test]
fn talk_is_not_drawn_where_no_call_can_be_placed() {
    let (s, _profile, _) = state("linux", false, false);
    assert!(window(s).find("talk").is_none());

    // No folder picked: Electron's `available: false, why: 'Pick a folder first.'`.
    let (mut s, _profile, _) = state("win32", false, false);
    s.home.folder_path = None;
    assert!(window(s).find("talk").is_none());

    // Nothing to place a call with: Electron's missing `api`.
    let (mut s, _profile, _) = state("win32", false, false);
    s.voice.audio = None;
    assert!(window(s).find("talk").is_none());
}

// case: sits right after voice typing in the chat box
#[test]
fn talk_sits_right_after_voice_typing_in_the_chat_box() {
    let (s, _profile, _) = state("win32", false, false);
    let rt = window(s);
    let buttons: Vec<_> = rt
        .elements()
        .into_iter()
        .filter(|item| item.role == "button")
        .filter_map(|item| item.key)
        .collect();
    let mic = buttons.iter().position(|key| key == "voice-typing").unwrap();
    assert_eq!(buttons.get(mic + 1).map(String::as_str), Some("talk"));
    // And the box draws the pair beside the field, before its send button, as Composer.tsx does.
    let composer = include_str!("thread_view.rs");
    let field = composer.find(".child(composer)\n").unwrap();
    let pair = composer.find(".children(crate::voice_panel::controls(s, p))").unwrap();
    let send = composer[pair..].find(".child(button)").unwrap() + pair;
    assert!(field < pair && pair < send);
    assert!(!composer[field..send].contains("button("));
}

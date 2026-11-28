use super::*;
use rune::{Env, Runtime};
use std::path::PathBuf;

struct Profile(PathBuf);
impl Drop for Profile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn settings(profile: &Profile, platform: &'static str) -> Runtime<State> {
    let store =
        crate::store::Store::new(profile.0.join("native"), profile.0.join("electron")).unwrap();
    std::fs::create_dir_all(store.root()).unwrap();
    let mut s = State::default();
    s.data = Some(store);
    s.settings_panel.voice.platform = Some(platform);
    crate::pages::close_all(&mut s);
    crate::settings_panel::open(&mut s);
    s.settings_panel.page = Some(crate::settings_pages::SettingsPageId::App);
    let mut rt = Runtime::new(s, crate::view, (1200.0, 2800.0), Env::new(true, 1.0));
    for _ in 0..20 {
        rt.frame(0.05);
    }
    rt
}

fn click(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt
        .find(key)
        .unwrap_or_else(|| panic!("missing {key} on real Settings page"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..20 {
        rt.frame(0.05);
    }
}

fn words(rt: &Runtime<State>, words: &str) -> bool {
    rt.elements()
        .iter()
        .any(|el| !el.exiting && el.text.as_deref() == Some(words))
}

// case: is there on a Mac too, and so is Talk's voice in Settings (the first Mac run, 2026-10-30)
#[test]
fn voice_choice_is_in_mac_talk_settings_but_not_linux() {
    let profile = Profile(std::env::temp_dir().join(format!(
        "locust-talk-choice-{}",
        locust_core::invariants::unique_nanos()
    )));
    let mut mac = settings(&profile, "darwin");
    assert!(words(&mac, "Talk’s voice"));
    assert!(words(&mac, "Cove"));
    composer_uses_voice(&profile, "cove");
    click(&mut mac, "settings-talk-voice");
    for (id, title) in [
        ("juniper", "Juniper — Open and upbeat"),
        ("maple", "Maple — Cheerful and candid"),
        ("spruce", "Spruce — Calm and affirming"),
        ("ember", "Ember — Confident and optimistic"),
        ("vale", "Vale — Bright and inquisitive"),
        ("breeze", "Breeze — Animated and earnest"),
        ("arbor", "Arbor — Easygoing and versatile"),
        ("sol", "Sol — Savvy and relaxed"),
        ("cove", "Cove — Composed and direct"),
    ] {
        assert!(mac.find(&format!("settings-talk-voice-{id}")).is_some());
        assert!(words(&mac, title));
    }
    click(&mut mac, "settings-talk-voice-sol");
    assert!(words(&mac, "Sol"));
    let reopened = settings(&profile, "darwin");
    assert!(
        words(&reopened, "Sol"),
        "the real option button must save the choice"
    );
    assert_eq!(
        locust_core::talk_shortcut::chosen_voice(reopened.state.data.as_ref().unwrap()).as_deref(),
        Some("sol")
    );
    assert!(
        settings(&profile, "win32")
            .find("settings-talk-voice")
            .is_some()
    );
    assert!(
        settings(&profile, "linux")
            .find("settings-talk-voice")
            .is_none()
    );
    composer_uses_voice(&profile, "sol");
}

fn composer_uses_voice(profile: &Profile, expected: &str) {
    let mut s = State::default();
    s.data = Some(
        crate::store::Store::new(profile.0.join("native"), profile.0.join("electron")).unwrap(),
    );
    s.teammates.push(Teammate {
        id: "tm_voice_fixture".into(),
        name: "A teammate".into(),
        role: "Writes".into(),
        runtime: Some("codex".into()),
        model: None,
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    });
    s.conversations
        .insert("tm_voice_fixture".into(), Conversation::default());
    s.voice.platform = Some("darwin");
    s.home.folder_path = Some(profile.0.to_string_lossy().into_owned());
    let mut composer = Runtime::new(
        s,
        |s, _| col().children(controls(s, &Palette::new(true))),
        (600.0, 300.0),
        Env::new(true, 1.0),
    );
    composer.frame(0.05);
    assert!(words(&composer, "Talk"));
    assert!(composer.find("voice-typing").is_some());
    // Run the actual Talk button, but never poll the peer result or open any network.
    click(&mut composer, "talk");
    let call = composer
        .state
        .voice
        .call
        .as_ref()
        .expect("Talk starts opening");
    let TalkState::Opening(_, options) = &call.state else {
        panic!("just opening Talk")
    };
    assert_eq!(
        options.voice.as_deref(),
        Some(expected),
        "Talk starts with the Settings choice"
    );
}

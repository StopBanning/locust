//! Settings › General › Voice typing, drawn and pressed through rune: the picker, the key field and where the key goes.
//! A stand-in keychain throughout; no real key is ever used.
use super::*;
use crate::settings_pages::SettingsPageId;
use rune::{ElementInfo, Env, Runtime};
use std::sync::{Arc, Mutex};

/// A stand-in for the keychain: it keeps what it is given in memory and hands back a handle, never the key.
struct Keychain(Mutex<Vec<String>>);
impl locust_core::own_models::SecretBox for Keychain {
    fn available(&self) -> bool {
        true
    }
    fn seal(&self, text: &str) -> Result<String, String> {
        let mut kept = self.0.lock().unwrap();
        kept.push(text.to_owned());
        Ok(format!("keychain:stand-in-{}", kept.len()))
    }
    fn open(&self, sealed: &str) -> Result<String, String> {
        let n: usize = sealed.strip_prefix("keychain:stand-in-").and_then(|n| n.parse().ok()).ok_or("not ours")?;
        self.0.lock().unwrap().get(n - 1).cloned().ok_or_else(|| "gone".into())
    }
}

const MADE_UP: &str = "sk-made-up-for-a-test-not-a-key"; // public-guard: allow

fn view(s: &State, _: &mut Cx<State>) -> El {
    crate::settings_panel::view(s, &Palette::new(true))
}

/// Settings opened as the app opens it (`settings_panel::open`, which reads voice typing), on `platform`.
fn window(root: &std::path::Path, platform: &'static str) -> (Runtime<State>, Arc<Keychain>) {
    let keychain = Arc::new(Keychain(Mutex::new(vec![])));
    locust_core::own_models::use_secrets(keychain.clone());
    let mut s = State::default();
    s.data = Some(crate::store::Store::new(root.join("native"), root.join("electron")).unwrap());
    s.settings_panel.voice.platform = Some(platform);
    crate::settings_panel::open(&mut s);
    s.settings_panel.page = Some(SettingsPageId::App);
    let mut rt = Runtime::new(s, view, (1200.0, 9000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    (rt, keychain)
}

fn dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("voice-settings-{name}-{}", locust_core::invariants::unique_nanos()))
}

fn shown(rt: &Runtime<State>) -> Vec<ElementInfo> {
    rt.elements().into_iter().filter(|e| !e.exiting).collect()
}

fn keyed(rt: &Runtime<State>, key: &str) -> Option<ElementInfo> {
    shown(rt).into_iter().find(|e| e.key.as_deref() == Some(key))
}

/// The accessibility tree's states for `key` ("checked", "disabled", ...), as a screen reader is told them.
fn states(rt: &Runtime<State>, key: &str) -> Vec<String> {
    fn find(node: &serde_json::Value, key: &str) -> Option<serde_json::Value> {
        if node["key"] == key {
            return Some(node.clone());
        }
        node["children"].as_array()?.iter().find_map(|n| find(n, key))
    }
    let node = find(&rt.accessibility_json(), key).unwrap_or_else(|| panic!("no {key} in the accessibility tree"));
    node["states"].as_array().map(|s| s.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

fn has_state(rt: &Runtime<State>, key: &str, state: &str) -> bool {
    states(rt, key).iter().any(|s| s == state)
}

fn said(rt: &Runtime<State>) -> Vec<String> {
    shown(rt).into_iter().filter_map(|e| e.text).collect()
}

fn press(rt: &mut Runtime<State>, key: &str) {
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
}

/// What voice typing reads (the microphone's own reader), on `platform`.
fn saved(root: &std::path::Path, platform: &str, keychain: &Keychain) -> locust_core::voice_typing::Settings {
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    locust_core::voice_typing::VoiceSettings::new(&store, keychain, platform).settings()
}

// Not an Electron case of its own (the one-line case holds the line's shape): VoiceSettings.tsx's picker on a Mac, where
// Fast and Accurate say they are Windows only and cannot be picked, and the key is asked for.
#[test]
fn on_a_mac_only_your_openai_account_can_be_picked_and_the_key_is_asked_for() {
    let root = dir("mac");
    let (rt, _) = window(&root, "darwin");
    let all = said(&rt);
    assert!(all.iter().any(|t| t == HEADING) && all.iter().any(|t| t == LEDE), "{all:?}");
    let picker = shown(&rt).into_iter().find(|e| e.label.as_deref() == Some("Voice typing model")).expect("the model picker");
    assert_eq!(picker.role, "radiogroup");
    for (key, words, off) in [
        ("settings-voice-mode-fast", "Fast \u{2014} 41 MB (Windows only for now)", true),
        ("settings-voice-mode-accurate", "Accurate \u{2014} 69 MB, slower (Windows only for now)", true),
        ("settings-voice-mode-openai", "Your OpenAI account", false),
    ] {
        let choice = keyed(&rt, key).unwrap_or_else(|| panic!("no {key}"));
        assert_eq!(choice.label.as_deref(), Some(words));
        assert_eq!(has_state(&rt, key, "disabled"), off, "{key}");
    }
    // A Mac reads Your OpenAI account whatever was saved (voice_typing.rs `public`).
    assert!(has_state(&rt, "settings-voice-mode-openai", "checked"));
    assert!(all.iter().any(|t| t == "Add your OpenAI API key. A ChatGPT subscription does not include API usage."));
    let field = keyed(&rt, "settings-voice-key").expect("the key field");
    assert_eq!(field.label.as_deref(), Some("OpenAI voice API key"));
    assert!(has_state(&rt, "settings-voice-save-key", "disabled"), "nothing typed, nothing to save");
    assert!(keyed(&rt, "settings-voice-remove-key").is_none(), "no key kept, nothing to remove");
    let _ = std::fs::remove_dir_all(&root);
}

// Not an Electron case of its own: Save key sends the key to the keychain (a stand-in here), the file keeps only the
// handle, the field is cleared and the key is never on screen; a new key needs consent again; Remove key forgets it.
#[test]
fn a_saved_key_goes_to_the_keychain_and_the_file_keeps_only_its_handle() {
    let root = dir("key");
    let (mut rt, keychain) = window(&root, "darwin");
    // Consent given for an earlier key does not carry over to a new one.
    {
        let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
        let voice = locust_core::voice_typing::VoiceSettings::new(&store, keychain.as_ref(), "darwin");
        voice.save(&serde_json::json!({"key": "sk-an-earlier-made-up-one"})).unwrap(); // public-guard: allow
        voice.consent().unwrap();
        keychain.0.lock().unwrap().clear();
    }
    press(&mut rt, "settings-voice-key");
    rt.paste(MADE_UP);
    rt.frame(0.016);
    assert!(!said(&rt).iter().any(|t| t.contains(MADE_UP)), "masked as it is typed");
    press(&mut rt, "settings-voice-save-key");
    assert_eq!(keychain.0.lock().unwrap().clone(), vec![MADE_UP.to_owned()], "the key went to the keychain");
    let file = std::fs::read_to_string(root.join("native").join("voice-settings.json")).unwrap();
    assert!(!file.contains(MADE_UP) && file.contains("keychain:stand-in-1"), "{file}");
    let now = saved(&root, "darwin", &keychain);
    assert!(now.has_key && !now.openai_consent, "{now:?}");
    assert!(rt.state.settings_panel.voice.key.text.is_empty(), "the field is cleared once saved");
    let all = said(&rt);
    assert!(all.iter().any(|t| t == "API key saved on this computer."), "{all:?}");
    assert!(all.iter().any(|t| t == "API key saved. It is locked to this Mac's keychain."), "{all:?}");
    assert!(!all.iter().any(|t| t.contains(MADE_UP)), "never on screen");
    assert_eq!(keyed(&rt, "settings-voice-message").unwrap().role, "status");

    press(&mut rt, "settings-voice-remove-key");
    assert!(!saved(&root, "darwin", &keychain).has_key);
    let all = said(&rt);
    assert!(all.iter().any(|t| t == "API key removed."), "{all:?}");
    assert!(keyed(&rt, "settings-voice-remove-key").is_none());
    let _ = std::fs::remove_dir_all(&root);
}

// Not an Electron case of its own: on Windows Fast is the first choice, and a pick is saved where the microphone reads it.
#[test]
fn on_windows_a_pick_is_saved_where_voice_typing_reads_it() {
    let root = dir("win");
    let (mut rt, keychain) = window(&root, "win32");
    assert!(has_state(&rt, "settings-voice-mode-fast", "checked"));
    assert_eq!(keyed(&rt, "settings-voice-mode-fast").unwrap().label.as_deref(), Some("Fast \u{2014} 41 MB"));
    assert!(!has_state(&rt, "settings-voice-mode-accurate", "disabled"));
    assert!(keyed(&rt, "settings-voice-key").is_none(), "no key field until Your OpenAI account is picked");
    press(&mut rt, "settings-voice-mode-accurate");
    assert_eq!(saved(&root, "win32", &keychain).mode, locust_core::voice_typing::Mode::Accurate);
    press(&mut rt, "settings-voice-mode-openai");
    assert_eq!(saved(&root, "win32", &keychain).mode, locust_core::voice_typing::Mode::OpenAi);
    assert!(has_state(&rt, "settings-voice-mode-openai", "checked"));
    assert!(keyed(&rt, "settings-voice-key").is_some(), "the key is asked for");
    assert!(said(&rt).iter().any(|t| t == "Add your OpenAI API key. A ChatGPT subscription does not include API usage."));
    let _ = std::fs::remove_dir_all(&root);
}

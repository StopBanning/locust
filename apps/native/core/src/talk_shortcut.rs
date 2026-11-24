//! Talk's Windows shortcut syntax and registration lifecycle. The window supplies the native registrar.

use crate::{invariants::regex, store::Store};
use serde_json::{Value, json};

pub const DEFAULT: &str = "Ctrl+Alt+Space";
const FILE: &str = "voice-call.json";

fn safe_key(key: &str) -> bool {
    regex(r"\A(?:[A-Z0-9]|F(?:[1-9]|1[0-9]|2[0-4])|Space)\z").is_match(key)
}

pub fn valid(keys: &str) -> bool {
    matches!(keys.split_once('+'), Some(("Ctrl" | "Alt", _)))
        && regex(
            r"\A(?:Ctrl\+)?(?:Alt\+)?(?:Shift\+)?(?:[A-Z0-9]|F(?:[1-9]|1[0-9]|2[0-4])|Space)\z",
        )
        .is_match(keys)
}

pub fn from_press(code: &str, ctrl: bool, alt: bool, shift: bool, meta: bool) -> Option<String> {
    if meta || (!ctrl && !alt) {
        return None;
    }
    let key = code
        .strip_prefix("Key")
        .filter(|letter| letter.len() == 1 && letter.as_bytes()[0].is_ascii_uppercase())
        .or_else(|| {
            code.strip_prefix("Digit")
                .filter(|digit| digit.len() == 1 && digit.as_bytes()[0].is_ascii_digit())
        })
        .or_else(|| {
            ((code.starts_with('F') && code.len() > 1 || code == "Space") && safe_key(code))
                .then_some(code)
        })?;
    let modifiers = [
        if ctrl { "Ctrl+" } else { "" },
        if alt { "Alt+" } else { "" },
        if shift { "Shift+" } else { "" },
    ];
    Some(format!("{}{key}", modifiers.concat()))
}

pub trait Registration {
    fn register(&mut self, keys: &str, pressed: Box<dyn Fn() + Send + Sync>) -> bool;
    fn unregister(&mut self, keys: &str);
}

pub trait Preferences {
    fn read(&self) -> Value;
    fn write(&mut self, keys: Option<&str>) -> Result<(), String>;
}

impl Preferences for Store {
    fn read(&self) -> Value {
        settings(self)["shortcut"].clone()
    }
    fn write(&mut self, keys: Option<&str>) -> Result<(), String> {
        keep(
            self,
            "shortcut",
            keys.map_or(json!(false), |keys| json!(keys)),
        )
    }
}

fn settings(store: &Store) -> Value {
    store
        .check(&store.root().join(FILE))
        .ok()
        .and_then(|path| std::fs::read(path).ok())
        .and_then(|body| serde_json::from_slice(&body).ok())
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}))
}

fn keep(store: &Store, key: &str, value: Value) -> Result<(), String> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _write = crate::invariants::lock(&LOCK);
    let mut settings = settings(store);
    settings[key] = value;
    store.save_json(FILE, &settings)
}

pub fn chosen_voice(store: &Store) -> Option<String> {
    settings(store)["voice"]
        .as_str()
        .filter(|voice| crate::voice_call_rules::is_voice(voice))
        .map(str::to_owned)
}

pub fn choose_voice(store: &Store, voice: &str) -> Result<(), String> {
    if !crate::voice_call_rules::is_voice(voice) {
        return Err("Choose one of the nine ChatGPT voices.".into());
    }
    keep(store, "voice", json!(voice))
}

#[derive(Debug, PartialEq, Eq)]
pub struct Status {
    pub keys: Option<String>,
    pub working: bool,
    pub why: Option<String>,
}

pub struct Shortcut<R: Registration, P> {
    registration: R,
    preferences: P,
    held: Option<String>,
    refused: Option<String>,
    drive_reason: Option<String>,
    drive_keys: Option<String>,
    pressed: std::sync::Arc<dyn Fn() + Send + Sync>,
}

impl<R: Registration, P: Preferences> Shortcut<R, P> {
    pub fn new(
        registration: R,
        preferences: P,
        pressed: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            registration,
            preferences,
            pressed: std::sync::Arc::new(pressed),
            held: None,
            refused: None,
            drive_reason: None,
            drive_keys: None,
        }
    }

    pub fn in_drive(mut self, reason: String, keys: Option<String>) -> Self {
        self.drive_reason = Some(reason);
        self.drive_keys = keys.filter(|keys| valid(keys));
        self
    }

    fn wanted(&self) -> Option<String> {
        if let Some(keys) = &self.drive_keys {
            return Some(keys.clone());
        }
        let saved = self.preferences.read();
        if saved == false {
            return None;
        }
        Some(
            saved
                .as_str()
                .filter(|keys| valid(keys))
                .unwrap_or(DEFAULT)
                .into(),
        )
    }

    pub fn ensure(&mut self, ready: bool) {
        if ready
            && (self.drive_reason.is_none() || self.drive_keys.is_some())
            && let Some(keys) = self.wanted()
        {
            self.take(keys);
        }
    }

    pub fn set(&mut self, keys: &Value) -> Result<Status, String> {
        let wanted = if keys == false {
            None
        } else if let Some(keys) = keys.as_str().filter(|keys| valid(keys)) {
            Some(keys)
        } else {
            return Ok(self.status());
        };
        // A drive's slot cannot change the person's settings or claim their keys.
        if self.drive_reason.is_none() {
            self.preferences.write(wanted)?;
            if let Some(keys) = wanted {
                self.take(keys.into());
            } else {
                self.release();
            }
        }
        Ok(self.status())
    }

    pub fn status(&self) -> Status {
        let keys = self.wanted();
        let working = keys.is_some() && keys == self.held;
        let why = if working || keys.is_none() {
            None
        } else if self.drive_keys.is_none() && self.drive_reason.is_some() {
            self.drive_reason.clone()
        } else if let Some(refused) = keys.as_ref().filter(|_| keys == self.refused) {
            Some(format!(
                "Another app is using {refused}, so Windows will not give it to Locust. Choose other keys."
            ))
        } else {
            Some("It starts working once Talk is ready on this computer: Codex signed in with ChatGPT, and a folder open.".into())
        };
        Status { keys, working, why }
    }

    fn take(&mut self, keys: String) {
        if self.held.as_ref() == Some(&keys) {
            return;
        }
        self.release();
        let pressed = self.pressed.clone();
        if self
            .registration
            .register(&keys, Box::new(move || pressed()))
        {
            self.held = Some(keys);
        } else {
            self.refused = Some(keys);
        }
    }
}

impl<R: Registration, P> Shortcut<R, P> {
    pub fn release(&mut self) {
        if let Some(held) = self.held.take() {
            self.registration.unregister(&held);
        }
        self.refused = None;
    }
}

impl<R: Registration, P> Drop for Shortcut<R, P> {
    fn drop(&mut self) {
        self.release();
    }
}

#[cfg(test)]
#[path = "talk_shortcut_tests.rs"]
mod tests;

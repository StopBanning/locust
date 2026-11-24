//! Voice typing's profile state and consent-checked transcription boundary.

use crate::{invariants::lock, own_models::SecretBox, store::Store};
use serde_json::{Value, json};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};

const FILE: &str = "voice-settings.json";
const OPENAI_URL: &str = "https://api.openai.com/v1/audio/transcriptions";
const MODEL: &str = "gpt-4o-transcribe";
const SAVE_FAILED: &str = "Voice typing settings could not be saved. Your message is unchanged.";
const CANCELLED: &str = "Voice typing was cancelled. Nothing was inserted.";
const UNREACHABLE: &str = "OpenAI could not be reached. Your message is unchanged. Try again.";
static SETTINGS_WRITES: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Fast,
    Accurate,
    OpenAi,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fast => "fast",
            Self::Accurate => "accurate",
            Self::OpenAi => "openai",
        }
    }

    pub fn parse(mode: &str) -> Option<Self> {
        match mode {
            "fast" => Some(Self::Fast),
            "accurate" => Some(Self::Accurate),
            "openai" => Some(Self::OpenAi),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Settings {
    pub mode: Mode,
    pub has_key: bool,
    pub openai_consent: bool,
}

struct Stored {
    mode: Mode,
    sealed: Option<String>,
    consent: bool,
}

impl Stored {
    fn public(&self, platform: &str) -> Settings {
        Settings {
            mode: if platform == "win32" {
                self.mode
            } else {
                Mode::OpenAi
            },
            has_key: self.sealed.is_some(),
            openai_consent: self.consent,
        }
    }
}

pub struct VoiceSettings<'a> {
    store: &'a Store,
    secrets: &'a dyn SecretBox,
    platform: &'a str,
}

impl<'a> VoiceSettings<'a> {
    pub fn new(store: &'a Store, secrets: &'a dyn SecretBox, platform: &'a str) -> Self {
        Self {
            store,
            secrets,
            platform,
        }
    }

    fn read(&self) -> Stored {
        let parsed = self
            .store
            .check(&self.store.root().join(FILE))
            .ok()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .unwrap_or_default();
        Stored {
            mode: parsed["mode"]
                .as_str()
                .and_then(Mode::parse)
                .unwrap_or(Mode::Fast),
            sealed: parsed["sealedKey"]
                .as_str()
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
            consent: parsed["openaiConsent"] == true,
        }
    }

    pub fn settings(&self) -> Settings {
        self.read().public(self.platform)
    }

    pub fn save(&self, change: &Value) -> Result<Settings, String> {
        let change = change.as_object().ok_or(SAVE_FAILED)?;
        let mode = change
            .get("mode")
            .map(|mode| mode.as_str().and_then(Mode::parse).ok_or(SAVE_FAILED))
            .transpose()?;
        let key = change
            .get("key")
            .map(|key| {
                key.as_str()
                    .filter(|key| {
                        key.encode_utf16().count() <= 4096 && !key.contains(['\r', '\n', '\0'])
                    })
                    .ok_or(SAVE_FAILED)
            })
            .transpose()?;

        let _write = lock(&SETTINGS_WRITES);
        let mut stored = self.read();
        let previous = stored.sealed.clone();
        if let Some(mode) = mode {
            stored.mode = mode;
        }
        if let Some(key) = key.map(str::trim) {
            if key.is_empty() {
                stored.sealed = None;
                stored.consent = false;
            } else {
                if !self.secrets.available() {
                    return Err("The operating system could not protect your API key. The key was not saved.".into());
                }
                stored.sealed = Some(self.secrets.seal(key).map_err(|_| SAVE_FAILED)?);
                stored.consent = false;
            }
        }
        let saved = self.persist(&stored);
        if stored.sealed != previous {
            let unused = if saved.is_ok() {
                previous
            } else {
                stored.sealed.clone()
            };
            if let Some(unused) = unused {
                self.secrets.forget(&unused);
            }
        }
        saved
    }

    pub fn consent(&self) -> Result<Settings, String> {
        let _write = lock(&SETTINGS_WRITES);
        let mut stored = self.read();
        let settings = stored.public(self.platform);
        if settings.mode != Mode::OpenAi || !settings.has_key {
            return Err(
                "Save your OpenAI API key in Settings > General first. Your message is unchanged."
                    .into(),
            );
        }
        stored.consent = true;
        self.persist(&stored)
    }

    fn persist(&self, value: &Stored) -> Result<Settings, String> {
        let mut json = json!({"mode": value.mode.as_str(), "openaiConsent": value.consent});
        if let Some(sealed) = &value.sealed {
            json["sealedKey"] = Value::String(sealed.clone());
        }
        self.store.save_json(FILE, &json).map_err(|_| SAVE_FAILED)?;
        Ok(value.public(self.platform))
    }

    fn open_key(&self, stored: &Stored) -> Option<String> {
        let sealed = stored.sealed.as_deref()?;
        if !self.secrets.available() {
            return None;
        }
        self.secrets
            .open(sealed)
            .ok()
            .filter(|key| !key.is_empty() && !key.contains(['\r', '\n', '\0']))
    }

    pub fn ready(&self, mode: Mode) -> bool {
        let _read = lock(&SETTINGS_WRITES);
        let stored = self.read();
        let settings = stored.public(self.platform);
        mode == Mode::OpenAi
            && settings.mode == mode
            && settings.openai_consent
            && self.open_key(&stored).is_some()
    }

    pub fn transcribe(
        &self,
        wav: &[u8],
        captured_as: Mode,
        cancel: &AtomicBool,
        http: &dyn TranscriptionTransport,
    ) -> Result<String, String> {
        if !valid_wav(wav) {
            return Err(
                "Voice typing needs a recording of at most one minute. Your message is unchanged."
                    .into(),
            );
        }
        if captured_as != Mode::OpenAi && self.platform != "win32" {
            return Err(local_only(self.platform));
        }
        let key = {
            let _read = lock(&SETTINGS_WRITES);
            let stored = self.read();
            let settings = stored.public(self.platform);
            if captured_as != settings.mode {
                return Err("Voice typing changed in Settings. Your message is unchanged. Press the microphone again.".into());
            }
            if captured_as != Mode::OpenAi {
                return Err(
                    "Voice typing needs its checked local files. Your message is unchanged.".into(),
                );
            }
            if !settings.openai_consent {
                return Err("Allow sending audio to OpenAI at the microphone first. Your message is unchanged.".into());
            }
            self.open_key(&stored)
        };
        transcribe_openai(wav, key.as_deref(), cancel, http)
    }
}

pub fn local_only(platform: &str) -> String {
    let follow = if platform == "darwin" {
        "On a Mac, use its own dictation here (the microphone key, or Fn twice), or choose Your OpenAI account under Voice typing in Settings."
    } else {
        "Choose Your OpenAI account under Voice typing in Settings."
    };
    format!(
        "Fast and Accurate voice typing run on Windows for now. {follow} Your message is unchanged."
    )
}

/// voice-pcm.ts `insertVoiceText`: the words, trimmed, go between what is before `start` and what is after `end` (byte
/// offsets on character boundaries), cut so the whole stays within `limit` UTF-16 units; the cursor lands after them.
pub fn insert_text(value: &str, words: &str, start: usize, end: usize, limit: usize) -> (String, usize) {
    let (before, after) = (&value[..start], &value[end..]);
    let room = limit.saturating_sub(before.encode_utf16().count() + after.encode_utf16().count());
    let inserted = crate::people::bounded(words, room);
    (format!("{before}{inserted}{after}"), before.len() + inserted.len())
}

pub const SAMPLE_RATE: usize = 16_000;
pub const MAX_PCM_SAMPLES: usize = SAMPLE_RATE * 60;

pub fn wav(samples: &[i16]) -> Option<Vec<u8>> {
    if samples.is_empty() || samples.len() > MAX_PCM_SAMPLES {
        return None;
    }
    let data = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    // PCM, one channel, 16 kHz, two bytes per frame and 16 bits per sample.
    out.extend_from_slice(&[
        16, 0, 0, 0, 1, 0, 1, 0, 128, 62, 0, 0, 0, 125, 0, 0, 2, 0, 16, 0,
    ]);
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    out.extend(samples.iter().flat_map(|sample| sample.to_le_bytes()));
    Some(out)
}

pub fn valid_wav(bytes: &[u8]) -> bool {
    let len = bytes.len();
    (46..=44 + MAX_PCM_SAMPLES * 2).contains(&len)
        && len % 2 == 0
        && &bytes[..4] == b"RIFF"
        && bytes[4..8] == ((len - 8) as u32).to_le_bytes()
        && &bytes[8..16] == b"WAVEfmt "
        && bytes[16..36]
            == [
                16, 0, 0, 0, 1, 0, 1, 0, 128, 62, 0, 0, 0, 125, 0, 0, 2, 0, 16, 0,
            ]
        && &bytes[36..40] == b"data"
        && bytes[40..44] == ((len - 44) as u32).to_le_bytes()
}

/// Holds user audio and an API key. Keep these values out of logs and public settings.
pub struct TranscriptionRequest<'a> {
    pub url: &'static str,
    pub authorization: String,
    pub multipart_boundary: String,
    pub body: Vec<u8>,
    pub timeout_seconds: u64,
    pub cancel: &'a AtomicBool,
}

/// Only implementations that omit ambient credentials and reject redirects may send these requests.
pub trait TranscriptionTransport: Send + Sync {
    fn send(&self, request: TranscriptionRequest<'_>) -> Result<(u16, Vec<u8>), ()>;
}

fn transcribe_openai(
    wav: &[u8],
    key: Option<&str>,
    cancel: &AtomicBool,
    http: &dyn TranscriptionTransport,
) -> Result<String, String> {
    if cancel.load(Ordering::Acquire) {
        return Err(CANCELLED.into());
    }
    let key =
        key.ok_or("Add your OpenAI API key in Settings > General. Your message is unchanged.")?;
    let boundary = format!("locust-{:x}", crate::invariants::unique_nanos());
    let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"speech.wav\"\r\nContent-Type: audio/wav\r\n\r\n").into_bytes();
    body.extend_from_slice(wav);
    for (name, value) in [
        ("model", MODEL),
        ("response_format", "json"),
        ("language", "en"),
    ] {
        body.extend_from_slice(
            format!(
                "\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let response = http.send(TranscriptionRequest {
        url: OPENAI_URL,
        authorization: format!("Bearer {key}"),
        multipart_boundary: boundary,
        body,
        timeout_seconds: 120,
        cancel,
    });
    if cancel.load(Ordering::Acquire) {
        return Err(CANCELLED.into());
    }
    let (status, bytes) = response.map_err(|_| UNREACHABLE)?;
    if matches!(status, 401 | 403) {
        return Err("OpenAI refused your API key. Check it in Settings > General. Your message is unchanged.".into());
    }
    if !(200..300).contains(&status) {
        return Err("OpenAI could not transcribe this recording. Check your API account and try again. Your message is unchanged.".into());
    }
    let reply: Value = serde_json::from_slice(&bytes).map_err(|_| UNREACHABLE)?;
    let text = reply["text"]
        .as_str()
        .filter(|text| text.encode_utf16().count() <= 256_000 && !text.contains(key))
        .ok_or(UNREACHABLE)?;
    Ok(text.into())
}

#[cfg(test)]
#[path = "voice_typing_tests.rs"]
mod tests;

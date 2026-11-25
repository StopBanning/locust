//! Settings › General › Voice typing (Screens.tsx's `data-setting="voice-typing"` line and VoiceSettings.tsx): the name
//! and what each choice costs on the left; on the right, the model picker ("Voice typing model"), and once Your OpenAI
//! account is picked, where the key is and the masked field that saves or replaces it. Saved through
//! `voice_typing::VoiceSettings`, which the microphone reads; the key goes to the operating system's box
//! (`own_models::secrets()`, this Mac's keychain), and the settings file keeps only what the box handed back.

use super::*;
use locust_core::voice_typing::{Mode, Settings, VoiceSettings};

pub(super) const HEADING: &str = "Voice typing";
pub(super) const LEDE: &str = "Fast and Accurate run on this computer (a one-time 41 or 69 MB download; Accurate is slower). Your OpenAI account sends audio to OpenAI on your API key, which OpenAI bills.";
const NO_KEY: &str = "Add your OpenAI API key. A ChatGPT subscription does not include API usage.";

#[derive(Default)]
pub(super) struct Line {
    /// What was read back (None until read, as Electron's select waits disabled for `settings()`).
    pub settings: Option<Settings>,
    /// The key being typed; cleared after every save, as Electron's `finally { setKey('') }`.
    pub key: TextEdit,
    /// What the last save said (`role="status"`).
    pub message: String,
    /// The platform in Node's names; None is this computer (tests name one).
    pub platform: Option<&'static str>,
}

fn platform(s: &State) -> &'static str {
    s.settings_panel.voice.platform.unwrap_or_else(locust_core::machine_words::this_platform)
}

/// The box keys go to. The window's tests never reach the real keychain: without a stand-in, nothing keeps a key.
fn secrets() -> std::sync::Arc<dyn locust_core::own_models::SecretBox> {
    if cfg!(test) && !locust_core::own_models::has_stand_in() {
        return std::sync::Arc::new(locust_core::own_models::NoBox);
    }
    locust_core::own_models::secrets()
}

/// Read what voice typing has saved (when Settings opens).
pub(super) fn read(s: &mut State) {
    let Some(store) = s.data.clone() else { return };
    let box_ = secrets();
    s.settings_panel.voice.settings = Some(VoiceSettings::new(&store, box_.as_ref(), platform(s)).settings());
}

/// VoiceSettings.tsx `save`: a mode, or a key (empty removes it). Says what happened, and clears the field.
fn save(s: &mut State, change: serde_json::Value) {
    let Some(store) = s.data.clone() else { return };
    let box_ = secrets();
    let keyed = change.get("key").and_then(|k| k.as_str()).map(|k| !k.is_empty());
    let answer = VoiceSettings::new(&store, box_.as_ref(), platform(s)).save(&change);
    let line = &mut s.settings_panel.voice;
    match answer {
        Ok(settings) => {
            line.settings = Some(settings);
            line.message = match keyed {
                None => String::new(),
                Some(true) => "API key saved on this computer.".into(),
                Some(false) => "API key removed.".into(),
            };
        }
        Err(why) => line.message = why,
    }
    line.key = TextEdit::default();
}

pub(super) fn section(s: &State, p: &Palette, first: bool) -> El {
    use crate::look::controls::{self, Tone};
    let l = crate::look::controls::Look::new(p.dark);
    let line = &s.settings_panel.voice;
    let platform = platform(s);
    // Fast and Accurate run whisper.cpp's Windows build; elsewhere they say so and cannot be picked (0.843, 0.849).
    let windows_only = if platform != "win32" { " (Windows only for now)" } else { "" };
    let chosen = line.settings.as_ref().map(|v| v.mode).unwrap_or(Mode::Fast);
    let ready = line.settings.is_some();
    let mut picker = col().gap(6.0).role("radiogroup").label("Voice typing model");
    for (mode, label) in [
        (Mode::Fast, format!("Fast \u{2014} 41 MB{windows_only}")),
        (Mode::Accurate, format!("Accurate \u{2014} 69 MB, slower{windows_only}")),
        (Mode::OpenAi, "Your OpenAI account".to_owned()),
    ] {
        let off = !ready || (mode != Mode::OpenAi && !windows_only.is_empty());
        let on = chosen == mode;
        let mut choice = controls::button(&l, &label, if on { Tone::Active } else { Tone::Neutral }, off)
            .key(format!("settings-voice-mode-{}", mode.as_str()))
            .role("radio")
            .checked(on)
            .disabled(off);
        if !off {
            choice = choice.on_press(move |s: &mut State| save(s, serde_json::json!({"mode": mode.as_str()})));
        }
        picker = picker.child(choice);
    }
    let mut control = col().key("settings-voice-typing").w(320.0).gap(8.0).child(picker);
    if let Some(settings) = line.settings.as_ref().filter(|v| v.mode == Mode::OpenAi) {
        let note = if settings.has_key {
            format!("API key saved. It is locked to {}.", locust_core::machine_words::voice_key_locked_to(Some(platform)))
        } else {
            NO_KEY.to_owned()
        };
        let typed = !line.key.text.trim().is_empty();
        let mut buttons = row().gap(6.0).child({
            let b = controls::button(&l, "Save key", Tone::Neutral, !typed).key("settings-voice-save-key").disabled(!typed);
            if typed {
                b.on_press(|s: &mut State| {
                    let key = s.settings_panel.voice.key.text.clone();
                    save(s, serde_json::json!({ "key": key }));
                })
            } else {
                b
            }
        });
        if settings.has_key {
            buttons = buttons.child(
                controls::button(&l, "Remove key", Tone::Neutral, false)
                    .key("settings-voice-remove-key")
                    .on_press(|s: &mut State| save(s, serde_json::json!({ "key": "" }))),
            );
        }
        control = control
            .child(text(note).key("settings-voice-key-note").font(l.ty.body).color(l.c("text-muted")).wrap())
            .child(
                text_input(&line.key, l.ty.body)
                    .key("settings-voice-key")
                    .single_line()
                    .secure()
                    .label("OpenAI voice API key")
                    .placeholder(if settings.has_key { "Replace saved key" } else { "API key" }, l.c("text-muted"))
                    .w_full()
                    .h(32.0)
                    .padding(6.0)
                    .radius(6.0)
                    .fill(l.c("bg-control"))
                    .color(l.c("text-primary"))
                    .on_edit(|s: &mut State| &mut s.settings_panel.voice.key),
            )
            .child(buttons);
    }
    if !line.message.is_empty() {
        control = control.child(
            text(line.message.clone()).key("settings-voice-message").role("status").font(l.ty.body).color(l.c("text-muted")).wrap(),
        );
    }
    let words = col()
        .gap(4.0)
        .child(text(HEADING.to_owned()).role("heading").level(2).font(l.ty.control).color(l.c("text-primary")))
        .child(text(LEDE.to_owned()).font(l.ty.body).color(l.c("text-secondary")).wrap());
    crate::settings_panel::line(&l, "settings-line-voice-typing", 10.0, first, words, control)
}

#[cfg(test)]
#[path = "voice_settings_tests.rs"]
mod tests;

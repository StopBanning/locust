//! The native window owns its recording, playback and peer connection. Dropping a lease closes its resources.

use std::sync::atomic::{AtomicBool, Ordering};

pub trait Recording: Send {
    /// Return at most one minute of 16 kHz mono PCM. Dropping this lease also releases the microphone.
    fn finish(self: Box<Self>) -> Result<Vec<i16>, String>;
}

pub trait Playback: Send {
    /// Player leases may be dropped to stop immediately.
    fn finished(&self) -> bool;
}

pub trait AudioPeer: Send {
    /// The media peer starts with a local offer and owns its microphone and output.
    fn offer(&self) -> &str;
    fn accept(&mut self, answer: &str) -> Result<(), String>;
}

/// Hardware and WebRTC can be substituted independently of Codex, the recorder model and the window.
pub trait Audio: Send + Sync {
    fn record(&self, cancel: &AtomicBool) -> Result<Box<dyn Recording>, String>;
    fn call(&self, cancel: &AtomicBool) -> Result<Box<dyn AudioPeer>, String>;
    fn play(&self, audio: &[u8]) -> Result<Box<dyn Playback>, String>;
}

pub fn talk_runs_on(platform: &str) -> bool {
    matches!(platform, "win32" | "darwin")
}

pub fn open_recording(
    audio: &dyn Audio,
    cancel: &AtomicBool,
) -> Result<Box<dyn Recording>, String> {
    if cancel.load(Ordering::Acquire) {
        return Err("Voice typing was cancelled. Nothing was inserted.".into());
    }
    let recording = audio.record(cancel)?;
    if cancel.load(Ordering::Acquire) {
        return Err("Voice typing was cancelled. Nothing was inserted.".into());
    }
    Ok(recording)
}

pub fn window_has_microphone(platform: &str, owned: bool, main: bool, media: &[&str]) -> bool {
    talk_runs_on(platform)
        && owned
        && main
        && !media.is_empty()
        && media.iter().all(|&kind| kind == "audio")
}

#[cfg(test)]
mod tests {
    use super::*;

    // case: runs on Windows and a Mac, and not yet on Linux
    #[test]
    fn talk_platforms() {
        for platform in ["win32", "darwin"] {
            assert!(talk_runs_on(platform));
        }
        for platform in ["linux", ""] {
            assert!(!talk_runs_on(platform));
        }
    }

    // case: gives the window the microphone on a Mac, and still only Locust's own audio
    #[test]
    fn grants_only_the_native_windows_audio() {
        assert!(window_has_microphone("darwin", true, true, &["audio"]));
        assert!(window_has_microphone("win32", true, true, &["audio"]));
        assert!(!window_has_microphone("linux", true, true, &["audio"]));
        assert!(!window_has_microphone("darwin", true, false, &["audio"]));
        assert!(!window_has_microphone("darwin", true, true, &["video"]));
    }

    // Electron's refusal table (voice-typing-refuses-unchecked-files.test.ts): another sender, a subframe, a camera,
    // audio with video, unknown media, and a request naming no media at all, which `all` alone would let through.
    #[test]
    fn refuses_other_senders_frames_and_media_and_a_request_for_none() {
        assert!(!window_has_microphone("darwin", false, true, &["audio"]));
        assert!(!window_has_microphone("darwin", true, false, &["audio"]));
        assert!(!window_has_microphone("darwin", true, true, &["video"]));
        assert!(!window_has_microphone("darwin", true, true, &["audio", "video"]));
        assert!(!window_has_microphone("darwin", true, true, &["unknown"]));
        assert!(!window_has_microphone("darwin", true, true, &[]));
    }

    // case: allows audio only from the owned main frame
    #[test]
    fn the_own_main_frame_can_record() {
        assert!(window_has_microphone("darwin", true, true, &["audio"]));
    }
}

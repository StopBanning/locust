//! The microphone's samples and loudness, following shared/voice-pcm.ts `voiceWav` and renderer voiceLevel.ts.
//! The device itself is behind `voice_audio::Audio`; nothing here opens one.

/// voice-pcm.ts `voiceWav`'s samples: each clamped to -1..1, then scaled by 32,768 below zero and 32,767 above, and
/// rounded as JavaScript's `Math.round` does (a half goes up).
pub fn pcm(samples: &[f32]) -> Vec<i16> {
    samples
        .iter()
        .map(|&sample| {
            let sample = sample.clamp(-1.0, 1.0);
            let scaled = f64::from(sample) * if sample < 0.0 { 32_768.0 } else { 32_767.0 };
            // NaN stays NaN through clamp and becomes 0 here, as DataView.setInt16(NaN) writes 0.
            (scaled + 0.5).floor() as i16
        })
        .collect()
}

/// voiceLevel.ts: the microphone's loudness for whatever draws it. A listener hears the level at once when it
/// subscribes and on every change after; a level that is not a number is 0, and it is kept within 0..1.
#[derive(Default)]
pub struct Level {
    level: f32,
    next: u64,
    listeners: Vec<(u64, Box<dyn FnMut(f32) + Send>)>,
}

impl Level {
    pub fn get(&self) -> f32 {
        self.level
    }
    pub fn subscribe(&mut self, mut listener: impl FnMut(f32) + Send + 'static) -> u64 {
        listener(self.level);
        self.next += 1;
        self.listeners.push((self.next, Box::new(listener)));
        self.next
    }
    pub fn unsubscribe(&mut self, id: u64) {
        self.listeners.retain(|(each, _)| *each != id);
    }
    pub fn set(&mut self, rms: f32) {
        self.level = if rms.is_finite() {
            rms.clamp(0.0, 1.0)
        } else {
            0.0
        };
        for (_, listener) in &mut self.listeners {
            listener(self.level);
        }
    }
}

/// What records, behind the platform (voiceCapture.ts's AudioContext, worklet and getUserMedia): the processing is
/// readied before the device opens, since a microphone that starts speaking at once would otherwise lose its first
/// word, and closing stops every track, the port and the context. Tests replace it; the real one is the platform's.
pub trait Graph: Send {
    /// Make the processing ready to take samples (the context, its worklet, resumed).
    fn ready(&mut self) -> Result<(), String>;
    /// Open the microphone. It may be granted after the person has already pressed Escape.
    fn open(&mut self) -> Result<(), String>;
    /// Stop the device's tracks and close the port and context. Called at most once.
    fn close(&mut self);
}

/// voiceCapture.ts `stop` after a cancel.
pub const CANCELLED: &str = "Voice typing was cancelled. Nothing was inserted.";

/// One recording (voiceCapture.ts `startVoiceCapture`): at most a minute of samples, the level said as they come, and
/// the graph closed exactly once however it ends.
pub struct Capture {
    graph: Box<dyn Graph>,
    level: std::sync::Arc<std::sync::Mutex<Level>>,
    samples: Vec<f32>,
    closed: bool,
}

impl Capture {
    /// Readies the graph, then opens the device. A refusal, or a cancel that lands while the device was being granted,
    /// closes everything already opened and returns the reason.
    pub fn start(
        graph: Box<dyn Graph>,
        cancel: &std::sync::atomic::AtomicBool,
        level: std::sync::Arc<std::sync::Mutex<Level>>,
    ) -> Result<Self, String> {
        use std::sync::atomic::Ordering;
        let mut capture = Self {
            graph,
            level,
            samples: Vec::new(),
            closed: false,
        };
        let cancelled = || {
            if cancel.load(Ordering::Acquire) {
                Err(CANCELLED.to_owned())
            } else {
                Ok(())
            }
        };
        // A refusal returns early and drops `capture`, which closes whatever was already opened.
        capture.graph.ready()?;
        cancelled()?;
        capture.graph.open()?;
        cancelled()?;
        Ok(capture)
    }

    /// Samples from the device: kept up to a minute, and their loudness (root mean square) said. True once the minute
    /// is full (voiceCapture.ts `atLimit`).
    pub fn push(&mut self, chunk: &[f32]) -> bool {
        if self.closed {
            return false;
        }
        let room = crate::voice_typing::MAX_PCM_SAMPLES.saturating_sub(self.samples.len());
        let chunk = &chunk[..chunk.len().min(room)];
        self.samples.extend_from_slice(chunk);
        let rms = if chunk.is_empty() {
            0.0
        } else {
            (chunk.iter().map(|value| value * value).sum::<f32>() / chunk.len() as f32).sqrt()
        };
        crate::invariants::lock(&self.level).set(rms);
        self.samples.len() >= crate::voice_typing::MAX_PCM_SAMPLES
    }

    /// The recording as a WAV, and everything closed.
    pub fn stop(&mut self) -> Result<Vec<u8>, String> {
        if self.closed {
            return Err(CANCELLED.into());
        }
        let samples = std::mem::take(&mut self.samples);
        self.close();
        crate::voice_typing::wav(&pcm(&samples)).ok_or_else(|| "No speech was recorded.".into())
    }

    /// Nothing kept, everything closed.
    pub fn cancel(&mut self) {
        self.samples.clear();
        self.close();
    }

    fn close(&mut self) {
        if std::mem::replace(&mut self.closed, true) {
            return;
        }
        self.graph.close();
        crate::invariants::lock(&self.level).set(0.0);
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
#[path = "voice_capture_tests.rs"]
mod tests;

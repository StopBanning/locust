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
        self.level = if rms.is_finite() { rms.clamp(0.0, 1.0) } else { 0.0 };
        for (_, listener) in &mut self.listeners {
            listener(self.level);
        }
    }
}

#[cfg(test)]
#[path = "voice_capture_tests.rs"]
mod tests;

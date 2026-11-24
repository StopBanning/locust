use super::*;
use crate::voice_typing::wav;
use std::sync::{Arc, Mutex};

// case: produces canonical mono PCM and clamps live RMS for subscribers
#[test]
fn samples_become_canonical_mono_pcm_and_the_level_is_clamped_for_listeners() {
    let wav = wav(&pcm(&[-2.0, 0.0, 2.0])).unwrap();
    assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16000);
    assert_eq!(i16::from_le_bytes([wav[44], wav[45]]), -32768);
    assert_eq!(i16::from_le_bytes([wav[48], wav[49]]), 32767);

    let mut level = Level::default();
    let heard = Arc::new(Mutex::new(Vec::new()));
    let kept = heard.clone();
    let id = level.subscribe(move |rms| kept.lock().unwrap().push(rms));
    level.set(0.4);
    level.set(2.0);
    level.set(f32::NAN);
    level.unsubscribe(id);
    level.set(0.2);
    assert_eq!(*heard.lock().unwrap(), vec![0.0, 0.4, 1.0, 0.0]);
    assert_eq!(level.get(), 0.2);
}

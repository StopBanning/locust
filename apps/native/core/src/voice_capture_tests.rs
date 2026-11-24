use super::*;
use crate::voice_typing::wav;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

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

#[derive(Default)]
struct Seen {
    order: Vec<&'static str>,
    closed: usize,
}

/// The graph's stand-in: says what was done, in order, and how many times it was closed.
struct FakeGraph {
    seen: Arc<Mutex<Seen>>,
    open: Box<dyn FnMut() -> Result<(), String> + Send>,
}
impl Graph for FakeGraph {
    fn ready(&mut self) -> Result<(), String> {
        self.seen.lock().unwrap().order.push("ready");
        Ok(())
    }
    fn open(&mut self) -> Result<(), String> {
        self.seen.lock().unwrap().order.push("mic");
        (self.open)()
    }
    fn close(&mut self) {
        self.seen.lock().unwrap().closed += 1;
    }
}

fn graph(
    open: impl FnMut() -> Result<(), String> + Send + 'static,
) -> (Box<dyn Graph>, Arc<Mutex<Seen>>) {
    let seen = Arc::new(Mutex::new(Seen::default()));
    (
        Box::new(FakeGraph {
            seen: seen.clone(),
            open: Box::new(open),
        }),
        seen,
    )
}

/// The level now, read before any assert so a failing one never leaves the lock poisoned for the capture's own drop.
fn heard(level: &Mutex<Level>) -> f32 {
    level.lock().unwrap().get()
}

// case: readies the graph before opening audio and stops tracks, context, and port
#[test]
fn the_graph_is_ready_before_the_microphone_and_closed_once_on_stop() {
    let (fake, seen) = graph(|| Ok(()));
    let level = Arc::new(Mutex::new(Level::default()));
    let mut capture = Capture::start(fake, &AtomicBool::new(false), level.clone()).unwrap();
    assert_eq!(seen.lock().unwrap().order, ["ready", "mic"]);
    capture.push(&[0.5, -0.5]);
    assert_eq!(heard(&level), 0.5);
    let wav = capture.stop().unwrap();
    assert_eq!(wav.len(), 48);
    assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16000);
    drop(capture);
    assert_eq!(seen.lock().unwrap().closed, 1);
    assert_eq!(heard(&level), 0.0);
}

// case: closes the audio graph when microphone access is denied
#[test]
fn a_denied_microphone_closes_the_graph() {
    let (fake, seen) = graph(|| Err("Microphone denied".into()));
    let refused = Capture::start(fake, &AtomicBool::new(false), Arc::default())
        .err()
        .unwrap();
    assert!(refused.contains("denied"));
    assert_eq!(seen.lock().unwrap().closed, 1);
}

// case: stops even a device granted after Escape while its prompt was pending
#[test]
fn a_device_granted_after_escape_is_stopped() {
    let cancel = Arc::new(AtomicBool::new(false));
    let escaped = cancel.clone();
    // Escape lands while the prompt is up; the device is granted anyway.
    let (fake, seen) = graph(move || {
        escaped.store(true, Ordering::Release);
        Ok(())
    });
    assert!(Capture::start(fake, &cancel, Arc::default()).is_err());
    assert_eq!(seen.lock().unwrap().order, ["ready", "mic"]);
    assert_eq!(seen.lock().unwrap().closed, 1);
}

// case: cancel returns no WAV and clears RMS
#[test]
fn cancel_keeps_nothing_and_clears_the_level() {
    let (fake, seen) = graph(|| Ok(()));
    let level = Arc::new(Mutex::new(Level::default()));
    let mut capture = Capture::start(fake, &AtomicBool::new(false), level.clone()).unwrap();
    capture.push(&[1.0]);
    assert_eq!(heard(&level), 1.0);
    capture.cancel();
    assert!(capture.stop().unwrap_err().contains("cancelled"));
    assert_eq!(heard(&level), 0.0);
    drop(capture);
    assert_eq!(seen.lock().unwrap().closed, 1);
}

// A capture keeps at most a minute (voiceCapture.ts atLimit): what arrives past it is dropped, the push says the minute
// is full, and the recording still becomes a WAV.
#[test]
fn a_capture_keeps_one_minute_and_says_when_it_is_full() {
    let (fake, _seen) = graph(|| Ok(()));
    let mut capture = Capture::start(fake, &AtomicBool::new(false), Arc::default()).unwrap();
    assert!(!capture.push(&vec![0.0; crate::voice_typing::MAX_PCM_SAMPLES - 10]));
    assert!(capture.push(&[0.0; 100]));
    let wav = capture.stop().expect("a full minute is still a recording");
    assert_eq!(wav.len(), 44 + crate::voice_typing::MAX_PCM_SAMPLES * 2);
}

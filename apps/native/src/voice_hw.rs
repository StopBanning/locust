//! Native hardware audio (`voice_audio::Audio`) built on `cpal` and `rubato`.
//! Microphone recording runs capture setup (`Graph::ready`) before opening the input stream (`Graph::open`),
//! downmixes and resamples to 16 kHz mono PCM capped at 60 seconds, and releases the device on finish or drop.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use locust_core::{
    voice_audio::{Audio, AudioPeer, Playback, Recording},
    voice_capture::{CANCELLED, Capture, Graph, Level, pcm},
    voice_typing::{MAX_PCM_SAMPLES, SAMPLE_RATE},
};
use rubato::{
    Async, FixedAsync, Indexing, PolynomialDegree, Resampler,
    audioadapter_buffers::direct::InterleavedSlice,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
};
use std::thread::{self, JoinHandle};

const RESAMPLE_CHUNK: usize = 1024;
const INPUT_CHANNEL_BOUND: usize = 64;

/// Pure downmix + resample pipeline converting interleaved `f32` audio at `src_rate` Hz (`channels` channels)
/// into 16 kHz mono `f32` samples for voice typing.
pub struct MicPipeline {
    channels: usize,
    src_rate: u32,
    pending: Vec<f32>,
    resampler: Option<Async<f32>>,
    out_scratch: Vec<f32>,
}

impl MicPipeline {
    pub fn new(src_rate: u32, channels: usize) -> Result<Self, String> {
        if src_rate == 0 || channels == 0 {
            return Err("Invalid microphone audio format.".into());
        }
        let (resampler, out_scratch) = if src_rate == SAMPLE_RATE as u32 {
            (None, Vec::new())
        } else {
            let ratio = SAMPLE_RATE as f64 / f64::from(src_rate);
            let resampler = Async::<f32>::new_poly(
                ratio,
                1.1,
                PolynomialDegree::Cubic,
                RESAMPLE_CHUNK,
                1,
                FixedAsync::Input,
            )
            .map_err(|_| "Could not prepare microphone resampler.".to_owned())?;
            let max_out = resampler.output_frames_max();
            (Some(resampler), vec![0.0; max_out])
        };
        Ok(Self {
            channels,
            src_rate,
            pending: Vec::new(),
            resampler,
            out_scratch,
        })
    }

    /// Downmix interleaved input samples to mono and emit all available 16 kHz mono samples.
    pub fn push(&mut self, interleaved: &[f32]) -> Vec<f32> {
        let frames = interleaved.len() / self.channels;
        if frames == 0 {
            return Vec::new();
        }
        if self.channels == 1 {
            self.pending.extend_from_slice(&interleaved[..frames]);
        } else {
            let inv = 1.0 / self.channels as f32;
            self.pending.reserve(frames);
            for frame in interleaved[..frames * self.channels].chunks_exact(self.channels) {
                let sum: f32 = frame.iter().copied().sum();
                self.pending.push(sum * inv);
            }
        }
        let Some(resampler) = self.resampler.as_mut() else {
            return std::mem::take(&mut self.pending);
        };
        let mut out = Vec::new();
        while self.pending.len() >= resampler.input_frames_next() {
            let needed = resampler.input_frames_next();
            let input = &self.pending[..needed];
            let Ok(in_buf) = InterleavedSlice::new(input, 1, needed) else {
                break;
            };
            let out_cap = self.out_scratch.len();
            let Ok(mut out_buf) = InterleavedSlice::new_mut(&mut self.out_scratch, 1, out_cap)
            else {
                break;
            };
            let Ok((consumed, written)) =
                resampler.process_into_buffer(&in_buf, &mut out_buf, None)
            else {
                break;
            };
            out.extend_from_slice(&self.out_scratch[..written]);
            self.pending.drain(..consumed);
        }
        out
    }

    /// Flush any remaining buffered samples at the end of a recording.
    pub fn finish(&mut self) -> Vec<f32> {
        let Some(resampler) = self.resampler.as_mut() else {
            return std::mem::take(&mut self.pending);
        };
        if self.pending.is_empty() {
            return Vec::new();
        }
        let partial = self.pending.len();
        let needed = resampler.input_frames_next();
        let mut padded = std::mem::take(&mut self.pending);
        padded.resize(needed, 0.0);
        let Ok(in_buf) = InterleavedSlice::new(&padded, 1, needed) else {
            return Vec::new();
        };
        let out_cap = self.out_scratch.len();
        let Ok(mut out_buf) = InterleavedSlice::new_mut(&mut self.out_scratch, 1, out_cap) else {
            return Vec::new();
        };
        let indexing = Indexing::new().partial_len(partial);
        let Ok((_, written)) = resampler.process_into_buffer(&in_buf, &mut out_buf, Some(&indexing))
        else {
            return Vec::new();
        };
        let expected = ((partial as f64) * (SAMPLE_RATE as f64 / f64::from(self.src_rate))).round()
            as usize;
        let keep = written.min(expected);
        self.out_scratch[..keep].to_vec()
    }
}

/// Append 16 kHz mono samples into `out` while enforcing the 60-second `MAX_PCM_SAMPLES` cap.
/// Returns `true` once the cap is reached.
pub fn push_capped(out: &mut Vec<f32>, chunk: &[f32]) -> bool {
    let room = MAX_PCM_SAMPLES.saturating_sub(out.len());
    let take = chunk.len().min(room);
    out.extend_from_slice(&chunk[..take]);
    out.len() >= MAX_PCM_SAMPLES
}

/// Run interleaved chunks through a `MicPipeline` with the 60-second cap and convert to 16-bit PCM.
#[cfg(test)]
fn collect_pcm(pipeline: &mut MicPipeline, chunks: &[&[f32]]) -> Vec<i16> {
    let mut mono_16k = Vec::new();
    for chunk in chunks {
        let out = pipeline.push(chunk);
        if push_capped(&mut mono_16k, &out) {
            return pcm(&mono_16k);
        }
    }
    let tail = pipeline.finish();
    push_capped(&mut mono_16k, &tail);
    pcm(&mono_16k)
}

/// VoiceButton.tsx's words when the microphone could not be had (refused, missing or busy): Electron says the same
/// sentence for each, since the person's next step is the same.
pub const MIC_REFUSED: &str =
    "Voice typing could not finish. Your message is unchanged; check microphone access and try again.";

fn map_cpal_error(_err: &cpal::Error) -> String {
    MIC_REFUSED.into()
}

/// Whether the system lets Locust have the microphone, asked before any device opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Allowed,
    Denied,
    /// The person pressed Escape while the system was still asking.
    Cancelled,
}

/// macOS: AVCaptureDevice's authorization for audio. Not yet asked, it asks (the system prompt, with Info.plist's
/// NSMicrophoneUsageDescription) and waits for the answer or a cancel; denied or restricted is refused at once, so a
/// refused microphone is said instead of recording silence.
#[cfg(target_os = "macos")]
fn system_access(cancel: &AtomicBool) -> Access {
    use objc2::runtime::Bool;
    use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
    let Some(audio) = (unsafe { AVMediaTypeAudio }) else {
        return Access::Denied;
    };
    let status = unsafe { AVCaptureDevice::authorizationStatusForMediaType(audio) };
    if status == AVAuthorizationStatus::Authorized {
        return Access::Allowed;
    }
    if status != AVAuthorizationStatus::NotDetermined {
        return Access::Denied;
    }
    let (send, answer) = mpsc::channel::<bool>();
    let send = Mutex::new(send);
    let handler = block2::RcBlock::new(move |granted: Bool| {
        if let Ok(send) = send.lock() {
            let _ = send.send(granted.as_bool());
        }
    });
    unsafe { AVCaptureDevice::requestAccessForMediaType_completionHandler(audio, &handler) };
    loop {
        if cancel.load(Ordering::Acquire) {
            return Access::Cancelled;
        }
        match answer.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(true) => return Access::Allowed,
            Ok(false) | Err(mpsc::RecvTimeoutError::Disconnected) => return Access::Denied,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
}

/// Elsewhere the device's own refusal (cpal's error) is what says the microphone was refused.
#[cfg(not(target_os = "macos"))]
fn system_access(_cancel: &AtomicBool) -> Access {
    Access::Allowed
}

/// The collector a device's graph hands its 16 kHz samples to, and the flag that stops it.
#[derive(Clone, Default)]
pub struct Collector {
    stop: Arc<AtomicBool>,
    samples: Arc<Mutex<Option<JoinHandle<Vec<i16>>>>>,
}

type AccessCheck = Box<dyn Fn(&AtomicBool) -> Access + Send + Sync>;
type Device = Box<dyn Fn(&Collector) -> Result<Box<dyn Graph>, String> + Send + Sync>;

/// The default input device through cpal, as a capture graph: nothing opens until `Graph::open`.
fn cpal_device(collector: &Collector) -> Result<Box<dyn Graph>, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| MIC_REFUSED.to_owned())?;
    let config = device
        .default_input_config()
        .map_err(|err| map_cpal_error(&err))?;
    Ok(Box::new(CpalGraph {
        device,
        config,
        stop: collector.stop.clone(),
        sender: None,
        collector: collector.samples.clone(),
        stream: None,
    }))
}

struct CpalGraph {
    device: cpal::Device,
    config: cpal::SupportedStreamConfig,
    stop: Arc<AtomicBool>,
    sender: Option<SyncSender<Vec<f32>>>,
    collector: Arc<Mutex<Option<JoinHandle<Vec<i16>>>>>,
    stream: Option<cpal::Stream>,
}

impl Graph for CpalGraph {
    fn ready(&mut self) -> Result<(), String> {
        let channels = usize::from(self.config.channels());
        let src_rate = self.config.sample_rate();
        let mut pipeline = MicPipeline::new(src_rate, channels)?;
        let (tx, rx): (SyncSender<Vec<f32>>, Receiver<Vec<f32>>) =
            mpsc::sync_channel(INPUT_CHANNEL_BOUND);
        let stop = self.stop.clone();
        let handle = thread::spawn(move || {
            let mut mono_16k = Vec::new();
            while let Ok(chunk) = rx.recv() {
                let resampled = pipeline.push(&chunk);
                if push_capped(&mut mono_16k, &resampled) {
                    stop.store(true, Ordering::Release);
                    break;
                }
                if stop.load(Ordering::Acquire) {
                    break;
                }
            }
            let tail = pipeline.finish();
            push_capped(&mut mono_16k, &tail);
            pcm(&mono_16k)
        });
        self.sender = Some(tx);
        if let Ok(mut slot) = self.collector.lock() {
            *slot = Some(handle);
        }
        Ok(())
    }

    fn open(&mut self) -> Result<(), String> {
        let tx = self
            .sender
            .clone()
            .ok_or_else(|| "Microphone capture was not prepared.".to_owned())?;
        let stop = self.stop.clone();
        let stream_config: cpal::StreamConfig = self.config.clone().into();
        let err_stop = stop.clone();
        let err_fn = move |_err: cpal::Error| {
            err_stop.store(true, Ordering::Release);
        };
        let stream = match self.config.sample_format() {
            cpal::SampleFormat::F32 => self.device.build_input_stream(
                stream_config,
                move |data: &[f32], _| {
                    if !stop.load(Ordering::Acquire) {
                        let _ = tx.try_send(data.to_vec());
                    }
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::I16 => self.device.build_input_stream(
                stream_config,
                move |data: &[i16], _| {
                    if !stop.load(Ordering::Acquire) {
                        let samples: Vec<f32> =
                            data.iter().map(|&s| f32::from(s) / 32_768.0).collect();
                        let _ = tx.try_send(samples);
                    }
                },
                err_fn,
                None,
            ),
            cpal::SampleFormat::U16 => self.device.build_input_stream(
                stream_config,
                move |data: &[u16], _| {
                    if !stop.load(Ordering::Acquire) {
                        let samples: Vec<f32> = data
                            .iter()
                            .map(|&s| (f32::from(s) - 32_768.0) / 32_768.0)
                            .collect();
                        let _ = tx.try_send(samples);
                    }
                },
                err_fn,
                None,
            ),
            _ => return Err("Unsupported microphone sample format.".into()),
        }
        .map_err(|err| map_cpal_error(&err))?;
        stream.play().map_err(|err| map_cpal_error(&err))?;
        self.stream = Some(stream);
        Ok(())
    }

    fn close(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Drop the cpal stream first so the OS releases the microphone immediately.
        self.stream.take();
        self.sender.take();
    }
}

struct NativeRecording {
    capture: Option<Capture>,
    collector: Collector,
}

impl Drop for NativeRecording {
    fn drop(&mut self) {
        // Tell the collector to stop; `capture`, the first field, is dropped right after this, which closes the stream.
        self.collector.stop.store(true, Ordering::Release);
    }
}

impl Recording for NativeRecording {
    fn finish(mut self: Box<Self>) -> Result<Vec<i16>, String> {
        self.collector.stop.store(true, Ordering::Release);
        // Dropping Capture closes the cpal stream and drops the channel sender so the collector exits.
        self.capture.take();
        let handle = self
            .collector
            .samples
            .lock()
            .ok()
            .and_then(|mut slot| slot.take());
        match handle {
            Some(join) => join
                .join()
                .map_err(|_| "Voice typing could not finish recording.".to_owned()),
            None => Ok(Vec::new()),
        }
    }
}

/// Platform audio implementation backed by `cpal` and `rubato`: the system's permission, then the default input.
pub struct NativeAudio {
    access: AccessCheck,
    device: Device,
}

impl Default for NativeAudio {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeAudio {
    pub fn new() -> Self {
        Self {
            access: Box::new(system_access),
            device: Box::new(cpal_device),
        }
    }

    /// The same recorder over a stand-in permission and device (tests: no microphone is needed).
    #[cfg(test)]
    fn with(access: AccessCheck, device: Device) -> Self {
        Self { access, device }
    }
}

impl Audio for NativeAudio {
    fn record(&self, cancel: &AtomicBool) -> Result<Box<dyn Recording>, String> {
        match (self.access)(cancel) {
            Access::Allowed => {}
            Access::Denied => return Err(MIC_REFUSED.into()),
            Access::Cancelled => return Err(CANCELLED.into()),
        }
        let collector = Collector::default();
        let graph = (self.device)(&collector)?;
        let level = Arc::new(Mutex::new(Level::default()));
        // Capture::start enforces Graph::ready() before Graph::open() and closes on cancel or error.
        let capture = Capture::start(graph, cancel, level)?;
        Ok(Box::new(NativeRecording {
            capture: Some(capture),
            collector,
        }))
    }

    fn call(&self, _cancel: &AtomicBool) -> Result<Box<dyn AudioPeer>, String> {
        Err("Talk is not here yet.".into())
    }

    fn play(&self, audio: &[u8]) -> Result<Box<dyn Playback>, String> {
        crate::voice_hw_play::play_audio(audio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_core::voice_typing::{valid_wav, wav};
    use std::f32::consts::TAU;

    // The 60-second cap stops the recording, not only trims it: push_capped says "full" exactly when the cap is
    // reached, which is what makes the collector stop the microphone (merge check, Voice 3).
    #[test]
    fn the_sixty_second_cap_says_full_once_reached_and_keeps_nothing_past_it() {
        let mut out = Vec::new();
        assert!(!push_capped(&mut out, &vec![0.0; MAX_PCM_SAMPLES - 1]), "a sample short of the cap is not full");
        assert!(push_capped(&mut out, &[0.0, 0.0, 0.0]), "reaching the cap is full");
        assert_eq!(out.len(), MAX_PCM_SAMPLES, "and nothing past the cap is kept");
        assert!(push_capped(&mut out, &[0.0]), "still full after");
        assert_eq!(out.len(), MAX_PCM_SAMPLES);
    }

    fn sine_interleaved(rate: u32, channels: usize, seconds: f32, freq: f32) -> Vec<f32> {
        let frames = (rate as f32 * seconds).round() as usize;
        let mut out = Vec::with_capacity(frames * channels);
        for i in 0..frames {
            let sample = (TAU * freq * (i as f32) / (rate as f32)).sin() * 0.5;
            for _ in 0..channels {
                out.push(sample);
            }
        }
        out
    }

    #[test]
    fn resamples_48khz_stereo_to_16khz_mono_valid_wav() {
        let stereo = sine_interleaved(48_000, 2, 1.0, 440.0);
        let mut pipeline = MicPipeline::new(48_000, 2).unwrap();
        let chunks: Vec<&[f32]> = stereo.chunks(4096).collect();
        let samples = collect_pcm(&mut pipeline, &chunks);
        // 1 second at 16 kHz = 16,000 samples (allow small filter delay/rounding tolerance).
        assert!(
            (samples.len() as isize - SAMPLE_RATE as isize).abs() <= 256,
            "expected ~{SAMPLE_RATE} samples, got {}",
            samples.len()
        );
        assert!(samples.iter().any(|&s| s.abs() > 1_000));
        let bytes = wav(&samples).unwrap();
        assert!(valid_wav(&bytes));
    }

    #[test]
    fn resamples_44100hz_mono_to_16khz_mono_valid_wav() {
        let mono = sine_interleaved(44_100, 1, 1.0, 440.0);
        let mut pipeline = MicPipeline::new(44_100, 1).unwrap();
        let chunks: Vec<&[f32]> = mono.chunks(2048).collect();
        let samples = collect_pcm(&mut pipeline, &chunks);
        assert!(
            (samples.len() as isize - SAMPLE_RATE as isize).abs() <= 256,
            "expected ~{SAMPLE_RATE} samples, got {}",
            samples.len()
        );
        assert!(samples.iter().any(|&s| s.abs() > 1_000));
        let bytes = wav(&samples).unwrap();
        assert!(valid_wav(&bytes));
    }

    #[test]
    fn downmix_averages_stereo_channels() {
        let mut pipeline = MicPipeline::new(16_000, 2).unwrap();
        // Left = 1.0, Right = -1.0 -> average 0.0; Left = 0.8, Right = 0.2 -> average 0.5.
        let samples = collect_pcm(&mut pipeline, &[&[1.0, -1.0, 0.8, 0.2]]);
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0], 0);
        assert!((samples[1] - 16_384).abs() <= 2);
    }

    #[test]
    fn caps_recording_at_sixty_seconds_and_produces_valid_wav() {
        let mut pipeline = MicPipeline::new(16_000, 1).unwrap();
        // Use a chunk size that does not divide MAX_PCM_SAMPLES so the cap must truncate mid-chunk.
        let chunk = vec![0.25f32; SAMPLE_RATE + 137];
        let chunks: Vec<&[f32]> = (0..65).map(|_| chunk.as_slice()).collect();
        let samples = collect_pcm(&mut pipeline, &chunks);
        assert_eq!(samples.len(), MAX_PCM_SAMPLES);
        let bytes = wav(&samples).unwrap();
        assert!(valid_wav(&bytes));
    }

    #[test]
    fn rejects_zero_rate_or_channels() {
        assert!(MicPipeline::new(0, 1).is_err());
        assert!(MicPipeline::new(48_000, 0).is_err());
    }

    /// What the stand-in device was asked to do, in order.
    type Seen = Arc<Mutex<Vec<&'static str>>>;

    /// A stand-in microphone: its graph hands back `samples` once closed, and its open can refuse or be granted
    /// after an Escape.
    struct FakeGraph {
        seen: Seen,
        collector: Collector,
        samples: Vec<i16>,
        open: Result<(), String>,
        escape_on_open: Option<Arc<AtomicBool>>,
    }

    impl Graph for FakeGraph {
        fn ready(&mut self) -> Result<(), String> {
            self.seen.lock().unwrap().push("ready");
            let samples = self.samples.clone();
            let stop = self.collector.stop.clone();
            *self.collector.samples.lock().unwrap() = Some(thread::spawn(move || {
                while !stop.load(Ordering::Acquire) {
                    thread::yield_now();
                }
                samples
            }));
            Ok(())
        }
        fn open(&mut self) -> Result<(), String> {
            self.seen.lock().unwrap().push("open");
            if let Some(escape) = &self.escape_on_open {
                escape.store(true, Ordering::Release);
            }
            self.open.clone()
        }
        fn close(&mut self) {
            self.seen.lock().unwrap().push("close");
        }
    }

    fn stand_in(access: Access, open: Result<(), String>, escape_on_open: Option<Arc<AtomicBool>>) -> (NativeAudio, Seen) {
        let seen: Seen = Arc::default();
        let device_seen = seen.clone();
        let audio = NativeAudio::with(
            Box::new(move |_| access),
            Box::new(move |collector| {
                device_seen.lock().unwrap().push("device");
                Ok(Box::new(FakeGraph {
                    seen: device_seen.clone(),
                    collector: collector.clone(),
                    samples: vec![1, -1, 2],
                    open: open.clone(),
                    escape_on_open: escape_on_open.clone(),
                }) as Box<dyn Graph>)
            }),
        );
        (audio, seen)
    }

    // case: readies the graph before opening audio and stops tracks, context, and port
    #[test]
    fn the_recorder_readies_before_it_opens_and_closes_once_when_finished() {
        let (audio, seen) = stand_in(Access::Allowed, Ok(()), None);
        let cancel = AtomicBool::new(false);
        let recording = audio.record(&cancel).unwrap();
        assert_eq!(*seen.lock().unwrap(), ["device", "ready", "open"]);
        assert_eq!(recording.finish().unwrap(), [1, -1, 2]);
        assert_eq!(*seen.lock().unwrap(), ["device", "ready", "open", "close"]);
    }

    // case: closes the audio graph when microphone access is denied
    #[test]
    fn a_refused_microphone_is_said_in_electrons_words_and_opens_nothing() {
        let (audio, seen) = stand_in(Access::Denied, Ok(()), None);
        let refused = audio.record(&AtomicBool::new(false)).err().unwrap();
        assert_eq!(refused, "Voice typing could not finish. Your message is unchanged; check microphone access and try again.");
        assert!(seen.lock().unwrap().is_empty(), "the device was touched after the system refused it");
        // The device itself refusing (cpal's error) closes what was readied.
        let (audio, seen) = stand_in(Access::Allowed, Err(MIC_REFUSED.into()), None);
        assert_eq!(audio.record(&AtomicBool::new(false)).err().unwrap(), MIC_REFUSED);
        assert_eq!(*seen.lock().unwrap(), ["device", "ready", "open", "close"]);
    }

    // case: stops even a device granted after Escape while its prompt was pending
    #[test]
    fn a_microphone_granted_after_escape_is_closed_and_nothing_is_recorded() {
        let cancel = Arc::new(AtomicBool::new(false));
        let (audio, seen) = stand_in(Access::Allowed, Ok(()), Some(cancel.clone()));
        let refused = audio.record(&cancel).err().unwrap();
        assert_eq!(refused, CANCELLED);
        assert_eq!(*seen.lock().unwrap(), ["device", "ready", "open", "close"]);
        // Escape while the system was still asking opens no device at all.
        let (audio, seen) = stand_in(Access::Cancelled, Ok(()), None);
        assert_eq!(audio.record(&AtomicBool::new(true)).err().unwrap(), CANCELLED);
        assert!(seen.lock().unwrap().is_empty());
    }

    // case: cancel returns no WAV and clears RMS
    #[test]
    fn a_cancelled_recording_closes_the_microphone_once() {
        let (audio, seen) = stand_in(Access::Allowed, Ok(()), None);
        let recording = audio.record(&AtomicBool::new(false)).unwrap();
        drop(recording);
        assert_eq!(*seen.lock().unwrap(), ["device", "ready", "open", "close"]);
    }

    #[test]
    fn talk_call_returns_plain_not_here_yet_error() {
        let audio = NativeAudio::new();
        let cancel = AtomicBool::new(false);
        let err = audio.call(&cancel).err().unwrap();
        assert_eq!(err, "Talk is not here yet.");
    }
}

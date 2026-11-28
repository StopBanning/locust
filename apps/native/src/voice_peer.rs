//! Cancellable PCM path behind the window's Talk peer. A media transport and the physical devices are separate:
//! the local loopback tests use this same peer without a microphone, speaker, account, or network.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TryRecvError},
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use locust_core::voice_audio::AudioPeer;

pub(super) const RATE: u32 = 48_000;
const FRAME: usize = 960; // 20 ms at 48 kHz.
const QUEUE: usize = 8;
pub(super) const CANCELLED: &str = "The call was ended.";
pub(super) const CANNOT_START: &str = "The call could not start. Try again.";
pub(super) const UNAVAILABLE: &str = "Talk is not here yet.";

pub(super) type Factory =
    Box<dyn Fn(&AtomicBool) -> Result<Box<dyn Transport>, String> + Send + Sync>;

/// The offer and answer belong to a transport. Media methods must not block. Their PCM is 48 kHz mono f32.
pub(super) trait Transport: Send {
    fn offer(&self) -> &str;
    fn accept(&mut self, answer: &str) -> Result<(), String>;
    fn send(&mut self, samples: &[f32]) -> Result<(), String>;
    fn receive(&mut self) -> Result<Option<Vec<f32>>, String>;
}

pub(super) trait Devices: Send + Sync {
    fn microphone(&self, input: Input) -> Result<Box<dyn Send>, String>;
    fn speaker(&self, output: Output) -> Result<Box<dyn Send>, String>;
}

pub(super) struct Input {
    tx: SyncSender<Vec<f32>>,
    connected: Arc<AtomicBool>,
    faulted: Arc<AtomicBool>,
}

impl Input {
    pub(super) fn samples(&self, pcm: &[f32]) {
        if !self.connected.load(Ordering::Acquire) {
            return;
        }
        // Never wait on the audio thread; let a slow peer drop samples rather than build delay.
        for chunk in pcm.chunks(FRAME).take(QUEUE) {
            if self.tx.try_send(chunk.to_vec()).is_err() {
                break;
            }
        }
    }
}

pub(super) struct Output {
    rx: Receiver<Vec<f32>>,
    pending: std::vec::IntoIter<f32>,
    connected: Arc<AtomicBool>,
    faulted: Arc<AtomicBool>,
}

impl Output {
    pub(super) fn next(&mut self) -> f32 {
        if !self.connected.load(Ordering::Acquire) {
            return 0.0;
        }
        if let Some(sample) = self.pending.next() {
            return sample;
        }
        let Ok(chunk) = self.rx.try_recv() else {
            return 0.0;
        };
        self.pending = chunk.into_iter();
        self.pending.next().unwrap_or(0.0)
    }
}

pub(super) fn check_cancel(cancel: &AtomicBool) -> Result<(), String> {
    if cancel.load(Ordering::Acquire) {
        return Err(CANCELLED.into());
    }
    Ok(())
}

pub(super) fn open(
    transport: Box<dyn Transport>,
    devices: &dyn Devices,
    cancel: &AtomicBool,
) -> Result<Box<dyn AudioPeer>, String> {
    check_cancel(cancel)?;
    let offer = transport.offer().to_owned();
    if offer.trim().is_empty() {
        return Err(CANNOT_START.into());
    }
    let connected = Arc::new(AtomicBool::new(false));
    let faulted = Arc::new(AtomicBool::new(false));
    let (input_tx, input_rx) = mpsc::sync_channel(QUEUE);
    let microphone = devices.microphone(Input {
        tx: input_tx,
        connected: connected.clone(),
        faulted: faulted.clone(),
    })?;
    check_cancel(cancel)?;

    let (output_tx, output_rx) = mpsc::sync_channel(QUEUE);
    let speaker = devices
        .speaker(Output {
            rx: output_rx,
            pending: Vec::new().into_iter(),
            connected: connected.clone(),
            faulted: faulted.clone(),
        })
        .map_err(|_| CANNOT_START.to_owned())?;
    check_cancel(cancel)?;

    Ok(Box::new(Peer {
        offer,
        opening: Some(Opening {
            transport,
            input_rx,
            output_tx,
            faulted,
        }),
        worker: None,
        microphone: Some(microphone),
        speaker: Some(speaker),
        connected,
        stop: Arc::new(AtomicBool::new(false)),
    }))
}

struct Opening {
    transport: Box<dyn Transport>,
    input_rx: Receiver<Vec<f32>>,
    output_tx: SyncSender<Vec<f32>>,
    faulted: Arc<AtomicBool>,
}

struct Peer {
    offer: String,
    opening: Option<Opening>,
    worker: Option<JoinHandle<()>>,
    microphone: Option<Box<dyn Send>>,
    speaker: Option<Box<dyn Send>>,
    connected: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
}

impl AudioPeer for Peer {
    fn offer(&self) -> &str {
        &self.offer
    }

    fn accept(&mut self, answer: &str) -> Result<(), String> {
        let Some(mut opening) = self.opening.take() else {
            return Err(CANNOT_START.into());
        };
        if opening.faulted.load(Ordering::Acquire) {
            self.close();
            return Err(CANNOT_START.into());
        }
        if let Err(why) = opening.transport.accept(answer) {
            self.close();
            return Err(why);
        }
        let stop = self.stop.clone();
        let connected = self.connected.clone();
        // Signal only after the remote answer was accepted. A microphone callback before this drops its samples.
        self.connected.store(true, Ordering::Release);
        let streams = (self.microphone.take(), self.speaker.take());
        let worker = thread::Builder::new()
            .name("talk-audio".into())
            .spawn(move || {
                relay(&mut opening, &stop);
                connected.store(false, Ordering::Release);
                drop(streams);
            });
        match worker {
            Ok(worker) => {
                self.worker = Some(worker);
                Ok(())
            }
            Err(_) => {
                self.close();
                Err(CANNOT_START.into())
            }
        }
    }
}

fn relay(wire: &mut Opening, stop: &AtomicBool) {
    while !stop.load(Ordering::Acquire) && !wire.faulted.load(Ordering::Acquire) {
        for _ in 0..QUEUE {
            match wire.input_rx.try_recv() {
                Ok(pcm) => {
                    if wire.transport.send(&pcm).is_err() {
                        return;
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        for _ in 0..QUEUE {
            match wire.transport.receive() {
                Ok(Some(mut pcm)) => {
                    pcm.truncate(FRAME);
                    if !pcm.is_empty() && wire.output_tx.try_send(pcm).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(_) => return,
            }
        }
        thread::park_timeout(Duration::from_millis(5));
    }
}

impl Peer {
    fn close(&mut self) {
        self.connected.store(false, Ordering::Release);
        self.stop.store(true, Ordering::Release);
        self.microphone.take();
        self.speaker.take();
        self.opening.take();
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.close();
    }
}

/// The first slice negotiates 48 kHz. A device that does not advertise it is refused rather than played at the
/// wrong speed. Resampling, echo cancellation and a deployed WebRTC transport are separate work.
pub(super) struct CpalDevices;

impl Devices for CpalDevices {
    fn microphone(&self, input: Input) -> Result<Box<dyn Send>, String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let mic = cpal::default_host()
            .default_input_device()
            .ok_or(CANNOT_START)?;
        let cfg = mic
            .supported_input_configs()
            .map_err(|_| CANNOT_START)?
            .find_map(call_config)
            .ok_or(CANNOT_START)?;
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => input_stream(&mic, cfg, input, |v: f32| v),
            cpal::SampleFormat::I16 => {
                input_stream(&mic, cfg, input, |v: i16| f32::from(v) / 32_768.0)
            }
            cpal::SampleFormat::U16 => input_stream(&mic, cfg, input, |v: u16| {
                (f32::from(v) - 32_768.0) / 32_768.0
            }),
            _ => return Err(CANNOT_START.into()),
        }
        .map_err(|_| CANNOT_START)?;
        stream.play().map_err(|_| CANNOT_START)?;
        Ok(Box::new(stream))
    }

    fn speaker(&self, output: Output) -> Result<Box<dyn Send>, String> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        let speaker = cpal::default_host()
            .default_output_device()
            .ok_or(CANNOT_START)?;
        let cfg = speaker
            .supported_output_configs()
            .map_err(|_| CANNOT_START)?
            .find_map(call_config)
            .ok_or(CANNOT_START)?;
        let stream = match cfg.sample_format() {
            cpal::SampleFormat::F32 => output_stream(&speaker, cfg, output, |v| v),
            cpal::SampleFormat::I16 => {
                output_stream(&speaker, cfg, output, |v| (v * 32_767.0) as i16)
            }
            cpal::SampleFormat::U16 => {
                output_stream(&speaker, cfg, output, |v| (v * 32_767.0 + 32_768.0) as u16)
            }
            _ => return Err(CANNOT_START.into()),
        }
        .map_err(|_| CANNOT_START)?;
        stream.play().map_err(|_| CANNOT_START)?;
        Ok(Box::new(stream))
    }
}

fn input_stream<T: cpal::SizedSample>(
    device: &cpal::Device,
    cfg: cpal::SupportedStreamConfig,
    input: Input,
    sample: impl Fn(T) -> f32 + Send + 'static,
) -> Result<cpal::Stream, cpal::Error> {
    use cpal::traits::DeviceTrait;
    let ch = usize::from(cfg.channels());
    let faulted = input.faulted.clone();
    device.build_input_stream(
        cfg.into(),
        move |data: &[T], _| {
            for chunk in data.chunks(FRAME * ch).take(QUEUE) {
                let mut mono = [0.0; FRAME];
                let mut frames = chunk.chunks_exact(ch);
                let n = frames.len();
                for (out, frame) in mono.iter_mut().zip(&mut frames) {
                    *out = frame.iter().map(|v| sample(*v)).sum::<f32>() / ch as f32;
                }
                input.samples(&mono[..n]);
            }
        },
        move |_| faulted.store(true, Ordering::Release),
        None,
    )
}

fn output_stream<T: cpal::SizedSample>(
    device: &cpal::Device,
    cfg: cpal::SupportedStreamConfig,
    mut output: Output,
    sample: impl Fn(f32) -> T + Send + 'static,
) -> Result<cpal::Stream, cpal::Error> {
    use cpal::traits::DeviceTrait;
    let ch = usize::from(cfg.channels());
    let faulted = output.faulted.clone();
    device.build_output_stream(
        cfg.into(),
        move |data: &mut [T], _| {
            for frame in data.chunks_mut(ch) {
                let pcm = output.next();
                frame.fill(sample(if pcm.is_finite() {
                    pcm.clamp(-1.0, 1.0)
                } else {
                    0.0
                }));
            }
        },
        move |_| faulted.store(true, Ordering::Release),
        None,
    )
}

fn call_config(cfg: cpal::SupportedStreamConfigRange) -> Option<cpal::SupportedStreamConfig> {
    ((1..=64).contains(&cfg.channels())
        && matches!(
            cfg.sample_format(),
            cpal::SampleFormat::F32 | cpal::SampleFormat::I16 | cpal::SampleFormat::U16
        ))
    .then(|| cfg.try_with_sample_rate(RATE))
    .flatten()
}

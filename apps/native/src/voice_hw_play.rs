//! Native audio playback (`voice_audio::Playback`) for WAV and WebM/Opus previews (`voice_preview.rs`).
//! Decodes in memory (MIT/Apache crates only: `webm-iterable` + `opus2` with `backend-mousiki`),
//! adapts rate/channels via `rubato`, and streams to the default `cpal` output device.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use locust_core::voice_audio::Playback;
use opus2::{Channels as OpusChannels, Decoder as OpusDecoder};
use rubato::{
    Async, FixedAsync, Indexing, PolynomialDegree, Resampler,
    audioadapter_buffers::direct::InterleavedSlice,
};
use std::{
    io::Cursor,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use webm_iterable::{
    WebmIterator,
    matroska_spec::{Block, MatroskaSpec, SimpleBlock},
};

/// Maximum accepted encoded audio input size (1.5 MB, matching `voice_preview.rs`).
pub const MAX_AUDIO_BYTES: usize = 1_500_000;
const OPUS_SAMPLE_RATE: u32 = 48_000;
/// Maximum Opus frame size at 48 kHz (120 ms = 5760 frames per channel).
const MAX_OPUS_FRAME_SAMPLES: usize = 5_760;
const MAX_DECODED_SAMPLES: usize = 48_000 * 2 * 120;
const RESAMPLE_CHUNK: usize = 1024;

/// Decoded interleaved `f32` PCM in `-1.0..=1.0`.
#[derive(Debug, Clone)]
pub struct DecodedAudio {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

/// Decode a WAV (`RIFF....WAVE`, 16-bit PCM) or WebM/Opus (`0x1a 0x45 0xdf 0xa3`) preview into PCM.
pub fn decode_audio(bytes: &[u8]) -> Result<DecodedAudio, String> {
    if bytes.is_empty() || bytes.len() > MAX_AUDIO_BYTES {
        return Err("Audio preview is empty or too large.".into());
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE" {
        decode_wav(bytes)
    } else if bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        decode_webm_opus(bytes)
    } else {
        Err("Unsupported audio preview format.".into())
    }
}

fn decode_wav(bytes: &[u8]) -> Result<DecodedAudio, String> {
    let mut pos = 12usize;
    let mut channels: Option<u16> = None;
    let mut sample_rate: Option<u32> = None;
    let mut data_slice: Option<&[u8]> = None;
    while pos + 8 <= bytes.len() {
        let id = &bytes[pos..pos + 4];
        let size = u32::from_le_bytes(bytes[pos + 4..pos + 8].try_into().unwrap()) as usize;
        pos += 8;
        if pos.saturating_add(size) > bytes.len() {
            return Err("WAV preview is truncated.".into());
        }
        let payload = &bytes[pos..pos + size];
        if id == b"fmt " {
            if payload.len() < 16 {
                return Err("WAV format header is invalid.".into());
            }
            let audio_format = u16::from_le_bytes([payload[0], payload[1]]);
            let ch = u16::from_le_bytes([payload[2], payload[3]]);
            let rate = u32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]);
            let bits = u16::from_le_bytes([payload[14], payload[15]]);
            if audio_format != 1 || !(1..=2).contains(&ch) || rate == 0 || bits != 16 {
                return Err("Unsupported WAV format.".into());
            }
            channels = Some(ch);
            sample_rate = Some(rate);
        } else if id == b"data" {
            data_slice = Some(payload);
        }
        pos += size + (size & 1);
    }
    let (Some(channels), Some(sample_rate), Some(data)) = (channels, sample_rate, data_slice)
    else {
        return Err("WAV preview is missing format or audio data.".into());
    };
    if data.is_empty() || data.len() % (usize::from(channels) * 2) != 0 {
        return Err("WAV preview has invalid sample data.".into());
    }
    let sample_count = data.len() / 2;
    if sample_count > MAX_DECODED_SAMPLES {
        return Err("WAV preview is too long.".into());
    }
    let samples: Vec<f32> = data
        .chunks_exact(2)
        .map(|pair| f32::from(i16::from_le_bytes([pair[0], pair[1]])) / 32_768.0)
        .collect();
    Ok(DecodedAudio {
        sample_rate,
        channels,
        samples,
    })
}

fn decode_webm_opus(bytes: &[u8]) -> Result<DecodedAudio, String> {
    let mut channels: u16 = 1;
    let mut packets: Vec<Vec<u8>> = Vec::new();
    let iter = WebmIterator::new(Cursor::new(bytes), &[]);
    for tag in iter {
        let tag = tag.map_err(|_| "WebM preview is unreadable.".to_owned())?;
        match &tag {
            MatroskaSpec::Channels(ch) => {
                if *ch == 2 {
                    channels = 2;
                } else if *ch == 1 {
                    channels = 1;
                }
            }
            MatroskaSpec::SimpleBlock(_) => {
                let block = SimpleBlock::try_from(&tag)
                    .map_err(|_| "WebM audio block is invalid.".to_owned())?;
                let frames = block
                    .read_frame_data()
                    .map_err(|_| "WebM audio frame is invalid.".to_owned())?;
                for frame in frames {
                    if !frame.data.is_empty() {
                        packets.push(frame.data.to_vec());
                    }
                }
            }
            MatroskaSpec::Block(_) => {
                let block = Block::try_from(&tag)
                    .map_err(|_| "WebM audio block is invalid.".to_owned())?;
                let frames = block
                    .read_frame_data()
                    .map_err(|_| "WebM audio frame is invalid.".to_owned())?;
                for frame in frames {
                    if !frame.data.is_empty() {
                        packets.push(frame.data.to_vec());
                    }
                }
            }
            _ => {}
        }
    }
    if packets.is_empty() {
        return Err("WebM preview has no audio frames.".into());
    }
    let opus_channels = if channels == 2 {
        OpusChannels::Stereo
    } else {
        OpusChannels::Mono
    };
    let mut decoder = OpusDecoder::new(OPUS_SAMPLE_RATE, opus_channels)
        .map_err(|_| "Could not initialize Opus decoder.".to_owned())?;
    let mut scratch = vec![0.0f32; MAX_OPUS_FRAME_SAMPLES * usize::from(channels)];
    let mut samples = Vec::new();
    for packet in &packets {
        let frames = decoder
            .decode_float(packet, &mut scratch, false)
            .map_err(|_| "WebM Opus frame could not be decoded.".to_owned())?;
        let count = frames * usize::from(channels);
        if samples.len().saturating_add(count) > MAX_DECODED_SAMPLES {
            return Err("WebM preview is too long.".into());
        }
        samples.extend_from_slice(&scratch[..count]);
    }
    if samples.is_empty() {
        return Err("WebM preview decoded to silence.".into());
    }
    Ok(DecodedAudio {
        sample_rate: OPUS_SAMPLE_RATE,
        channels,
        samples,
    })
}

/// Convert decoded audio to the output device's sample rate and channel count.
pub fn prepare_output(
    decoded: &DecodedAudio,
    out_rate: u32,
    out_channels: u16,
) -> Result<Vec<f32>, String> {
    if out_rate == 0 || out_channels == 0 || decoded.sample_rate == 0 || decoded.channels == 0 {
        return Err("Invalid audio output format.".into());
    }
    let src_ch = usize::from(decoded.channels);
    let dst_ch = usize::from(out_channels);
    let frames = decoded.samples.len() / src_ch;
    let mut adapted = Vec::with_capacity(frames * dst_ch);
    for frame in decoded.samples[..frames * src_ch].chunks_exact(src_ch) {
        if src_ch == dst_ch {
            adapted.extend_from_slice(frame);
        } else if src_ch == 1 {
            for _ in 0..dst_ch {
                adapted.push(frame[0]);
            }
        } else {
            let avg = frame.iter().copied().sum::<f32>() / src_ch as f32;
            for _ in 0..dst_ch {
                adapted.push(avg);
            }
        }
    }
    if decoded.sample_rate == out_rate {
        return Ok(adapted);
    }
    let ratio = f64::from(out_rate) / f64::from(decoded.sample_rate);
    let mut resampler = Async::<f32>::new_poly(
        ratio,
        1.1,
        PolynomialDegree::Cubic,
        RESAMPLE_CHUNK,
        dst_ch,
        FixedAsync::Input,
    )
    .map_err(|_| "Could not prepare playback resampler.".to_owned())?;
    let max_out_frames = resampler.output_frames_max();
    let mut out_scratch = vec![0.0f32; max_out_frames * dst_ch];
    let mut out = Vec::new();
    let mut cursor = 0usize;
    let total_frames = adapted.len() / dst_ch;
    while total_frames.saturating_sub(cursor) >= resampler.input_frames_next() {
        let needed = resampler.input_frames_next();
        let slice = &adapted[cursor * dst_ch..(cursor + needed) * dst_ch];
        let in_buf = InterleavedSlice::new(slice, dst_ch, needed)
            .map_err(|_| "Could not wrap playback input buffer.".to_owned())?;
        let mut out_buf = InterleavedSlice::new_mut(&mut out_scratch, dst_ch, max_out_frames)
            .map_err(|_| "Could not wrap playback output buffer.".to_owned())?;
        let (consumed, written) = resampler
            .process_into_buffer(&in_buf, &mut out_buf, None)
            .map_err(|_| "Could not resample playback audio.".to_owned())?;
        out.extend_from_slice(&out_scratch[..written * dst_ch]);
        cursor += consumed;
    }
    let remaining = total_frames.saturating_sub(cursor);
    if remaining > 0 {
        let needed = resampler.input_frames_next();
        let mut padded = vec![0.0f32; needed * dst_ch];
        padded[..remaining * dst_ch].copy_from_slice(&adapted[cursor * dst_ch..]);
        let in_buf = InterleavedSlice::new(&padded, dst_ch, needed)
            .map_err(|_| "Could not wrap final playback buffer.".to_owned())?;
        let mut out_buf = InterleavedSlice::new_mut(&mut out_scratch, dst_ch, max_out_frames)
            .map_err(|_| "Could not wrap final playback output buffer.".to_owned())?;
        let indexing = Indexing::new().partial_len(remaining);
        let (_, written) = resampler
            .process_into_buffer(&in_buf, &mut out_buf, Some(&indexing))
            .map_err(|_| "Could not flush playback resampler.".to_owned())?;
        let expected = (remaining as f64 * ratio).round() as usize;
        let keep = written.min(expected);
        out.extend_from_slice(&out_scratch[..keep * dst_ch]);
    }
    Ok(out)
}

/// Pure playback cursor shared between the `cpal` output callback and in-memory sink tests.
pub struct PlaybackSource {
    samples: Vec<f32>,
    pos: usize,
    done: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
}

impl PlaybackSource {
    pub fn new(samples: Vec<f32>, done: Arc<AtomicBool>, stopped: Arc<AtomicBool>) -> Self {
        if samples.is_empty() {
            done.store(true, Ordering::Release);
        }
        Self {
            samples,
            pos: 0,
            done,
            stopped,
        }
    }

    pub fn fill_f32(&mut self, out: &mut [f32]) {
        if self.stopped.load(Ordering::Acquire) || self.pos >= self.samples.len() {
            out.fill(0.0);
            self.done.store(true, Ordering::Release);
            return;
        }
        let remaining = self.samples.len() - self.pos;
        let count = out.len().min(remaining);
        out[..count].copy_from_slice(&self.samples[self.pos..self.pos + count]);
        out[count..].fill(0.0);
        self.pos += count;
        if self.pos >= self.samples.len() {
            self.done.store(true, Ordering::Release);
        }
    }

    fn fill_i16(&mut self, out: &mut [i16]) {
        let mut buf = vec![0.0f32; out.len()];
        self.fill_f32(&mut buf);
        for (dst, &src) in out.iter_mut().zip(&buf) {
            *dst = (src.clamp(-1.0, 1.0) * 32_767.0).round() as i16;
        }
    }

    fn fill_u16(&mut self, out: &mut [u16]) {
        let mut buf = vec![0.0f32; out.len()];
        self.fill_f32(&mut buf);
        for (dst, &src) in out.iter_mut().zip(&buf) {
            *dst = ((src.clamp(-1.0, 1.0) * 32_767.0) + 32_768.0).round() as u16;
        }
    }
}

struct NativePlayback {
    stream: Option<cpal::Stream>,
    done: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
}

impl Drop for NativePlayback {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Release);
        self.done.store(true, Ordering::Release);
        self.stream.take();
    }
}

impl Playback for NativePlayback {
    fn finished(&self) -> bool {
        self.done.load(Ordering::Acquire)
    }
}

/// Decode WAV or WebM/Opus audio and play it on the default `cpal` output device.
pub fn play_audio(audio: &[u8]) -> Result<Box<dyn Playback>, String> {
    let decoded = decode_audio(audio)?;
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "No audio output device is available.".to_owned())?;
    let config = device
        .default_output_config()
        .map_err(|_| "Could not read audio output configuration.".to_owned())?;
    let samples = prepare_output(&decoded, config.sample_rate(), config.channels())?;
    let done = Arc::new(AtomicBool::new(false));
    let stopped = Arc::new(AtomicBool::new(false));
    let mut source = PlaybackSource::new(samples, done.clone(), stopped.clone());
    let stream_config: cpal::StreamConfig = config.clone().into();
    let err_done = done.clone();
    let err_fn = move |_err: cpal::Error| {
        err_done.store(true, Ordering::Release);
    };
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            stream_config,
            move |data: &mut [f32], _| source.fill_f32(data),
            err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_output_stream(
            stream_config,
            move |data: &mut [i16], _| source.fill_i16(data),
            err_fn,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_output_stream(
            stream_config,
            move |data: &mut [u16], _| source.fill_u16(data),
            err_fn,
            None,
        ),
        _ => return Err("Unsupported audio output sample format.".into()),
    }
    .map_err(|_| "Could not open audio output stream.".to_owned())?;
    stream
        .play()
        .map_err(|_| "Could not start audio playback.".to_owned())?;
    Ok(Box::new(NativePlayback {
        stream: Some(stream),
        done,
        stopped,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice_hw::NativeAudio;
    use locust_core::{voice_audio::Audio, voice_typing::wav};
    use opus2::{Application, Encoder as OpusEncoder};
    use std::f32::consts::TAU;
    use webm_iterable::{
        WebmWriter,
        matroska_spec::{Frame, Master},
    };

    fn make_webm_opus(frames_48k: usize) -> Vec<u8> {
        let mut encoder =
            OpusEncoder::new(OPUS_SAMPLE_RATE, OpusChannels::Mono, Application::Voip).unwrap();
        let frame_size = 960usize; // 20 ms at 48 kHz
        let mut out = Cursor::new(Vec::new());
        let mut writer = WebmWriter::new(&mut out);
        writer
            .write(&MatroskaSpec::Ebml(Master::Start))
            .unwrap();
        writer
            .write(&MatroskaSpec::DocType("webm".into()))
            .unwrap();
        writer.write(&MatroskaSpec::Ebml(Master::End)).unwrap();
        writer
            .write(&MatroskaSpec::Segment(Master::Start))
            .unwrap();
        writer
            .write(&MatroskaSpec::Tracks(Master::Start))
            .unwrap();
        writer
            .write(&MatroskaSpec::TrackEntry(Master::Start))
            .unwrap();
        writer.write(&MatroskaSpec::TrackNumber(1)).unwrap();
        writer.write(&MatroskaSpec::TrackType(2)).unwrap();
        writer
            .write(&MatroskaSpec::CodecID("A_OPUS".into()))
            .unwrap();
        writer
            .write(&MatroskaSpec::Audio(Master::Start))
            .unwrap();
        writer.write(&MatroskaSpec::Channels(1)).unwrap();
        writer
            .write(&MatroskaSpec::SamplingFrequency(48_000.0))
            .unwrap();
        writer.write(&MatroskaSpec::Audio(Master::End)).unwrap();
        writer
            .write(&MatroskaSpec::TrackEntry(Master::End))
            .unwrap();
        writer
            .write(&MatroskaSpec::Tracks(Master::End))
            .unwrap();
        writer
            .write(&MatroskaSpec::Cluster(Master::Start))
            .unwrap();
        writer.write(&MatroskaSpec::Timestamp(0)).unwrap();

        let mut pkt = vec![0u8; 4000];
        let mut i = 0usize;
        let mut ts: i16 = 0;
        while i < frames_48k {
            let mut pcm = vec![0.0f32; frame_size];
            for (k, sample) in pcm.iter_mut().enumerate() {
                let n = i + k;
                *sample = (TAU * 440.0 * (n as f32) / 48_000.0).sin() * 0.4;
            }
            let len = encoder.encode_float(&pcm, &mut pkt).unwrap();
            let block = SimpleBlock::new_uncheked(&pkt[..len], 1, ts, false, None, false, true);
            let spec: MatroskaSpec = block.into();
            writer.write(&spec).unwrap();
            i += frame_size;
            ts = ts.saturating_add(20);
        }
        writer
            .write(&MatroskaSpec::Cluster(Master::End))
            .unwrap();
        writer
            .write(&MatroskaSpec::Segment(Master::End))
            .unwrap();
        let _ = Frame { data: &[] };
        out.into_inner()
    }

    #[test]
    fn decodes_wav_resamples_and_drains_into_in_memory_sink() {
        let pcm_16k: Vec<i16> = (0..1_600)
            .map(|i| ((TAU * 440.0 * (i as f32) / 16_000.0).sin() * 16_000.0) as i16)
            .collect();
        let wav_bytes = wav(&pcm_16k).unwrap();
        let decoded = decode_audio(&wav_bytes).unwrap();
        assert_eq!(decoded.sample_rate, 16_000);
        assert_eq!(decoded.channels, 1);
        assert_eq!(decoded.samples.len(), 1_600);

        let prepared = prepare_output(&decoded, 48_000, 2).unwrap();
        // 0.1 s at 48 kHz stereo = 9,600 samples.
        assert!((prepared.len() as isize - 9_600).abs() <= 512);

        let done = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));
        let mut source = PlaybackSource::new(prepared.clone(), done.clone(), stopped);
        assert!(!done.load(Ordering::Acquire));

        let mut sink = Vec::new();
        let mut buf = [0.0f32; 512];
        while !done.load(Ordering::Acquire) {
            source.fill_f32(&mut buf);
            sink.extend_from_slice(&buf);
        }
        assert!(done.load(Ordering::Acquire));
        assert!(sink.iter().any(|&s| s.abs() > 0.1));
    }

    #[test]
    fn decodes_webm_opus_preview_and_drains_into_in_memory_sink() {
        let webm = make_webm_opus(9_600); // 200 ms at 48 kHz
        assert!(webm.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]));
        let decoded = decode_audio(&webm).unwrap();
        assert_eq!(decoded.sample_rate, 48_000);
        assert_eq!(decoded.channels, 1);
        assert_eq!(decoded.samples.len(), 9_600);
        assert!(decoded.samples.iter().any(|&s| s.abs() > 0.05));

        let done = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));
        let mut source = PlaybackSource::new(decoded.samples, done.clone(), stopped);
        let mut buf = [0.0f32; 1024];
        let mut chunks = 0usize;
        while !done.load(Ordering::Acquire) {
            source.fill_f32(&mut buf);
            chunks += 1;
        }
        assert!(chunks >= 9);
        assert!(done.load(Ordering::Acquire));
    }

    #[test]
    fn rejects_corrupt_empty_and_oversized_inputs_without_opening_speaker() {
        let audio = NativeAudio::new();
        assert_eq!(
            audio.play(&[]).err().as_deref(),
            Some("Audio preview is empty or too large.")
        );
        assert_eq!(
            audio.play(b"not-wav-or-webm").err().as_deref(),
            Some("Unsupported audio preview format.")
        );
        let oversized = vec![0u8; MAX_AUDIO_BYTES + 1];
        assert_eq!(
            audio.play(&oversized).err().as_deref(),
            Some("Audio preview is empty or too large.")
        );
        // Corrupt RIFF/WAVE header.
        assert!(audio.play(b"RIFF\x04\x00\x00\x00WAVEfmt ").is_err());
        // Corrupt WebM header with no audio frames.
        assert!(audio.play(&[0x1a, 0x45, 0xdf, 0xa3, 0x00, 0x00]).is_err());
    }

    #[test]
    fn stopping_playback_outputs_silence_and_marks_finished() {
        let done = Arc::new(AtomicBool::new(false));
        let stopped = Arc::new(AtomicBool::new(false));
        let mut source = PlaybackSource::new(vec![0.5f32; 4096], done.clone(), stopped.clone());
        let mut buf = [0.0f32; 256];
        source.fill_f32(&mut buf);
        assert!(!done.load(Ordering::Acquire));
        assert!(buf.iter().all(|&s| (s - 0.5).abs() < 1e-6));

        // Simulate Drop of NativePlayback stopping early.
        stopped.store(true, Ordering::Release);
        source.fill_f32(&mut buf);
        assert!(done.load(Ordering::Acquire));
        assert!(buf.iter().all(|&s| s == 0.0));
    }
}

//! A short greeting recorded for a voice, and the synthetic question that prompts it.

use crate::{
    invariants::{lock, unique_nanos},
    store::Store,
    voice_call_rules::is_voice,
};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};

const DIRECTORY: &str = "voice-previews";
const MAX_BYTES: usize = 1_500_000;
static TRIGGER: Mutex<()> = Mutex::new(());

pub fn valid_greeting(bytes: &[u8]) -> bool {
    (1_000..=MAX_BYTES).contains(&bytes.len()) && bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3])
}

fn valid_wave(bytes: &[u8]) -> bool {
    (45..=MAX_BYTES).contains(&bytes.len())
        && bytes.starts_with(b"RIFF")
        && &bytes[8..12] == b"WAVE"
}

fn read(store: &Store, path: &Path) -> Option<Vec<u8>> {
    let path = store.check(path).ok()?;
    let mut file = File::open(path).ok()?;
    if !file.metadata().ok()?.is_file() || file.metadata().ok()?.len() > MAX_BYTES as u64 {
        return None;
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    Some(bytes)
}

pub fn saved(store: &Store, voice: &str) -> Option<Vec<u8>> {
    if !is_voice(voice) {
        return None;
    }
    let bytes = read(
        store,
        &store.root().join(DIRECTORY).join(format!("{voice}.webm")),
    )?;
    valid_greeting(&bytes).then_some(bytes)
}

pub fn save(store: &Store, voice: &str, bytes: &[u8]) -> io::Result<()> {
    if !is_voice(voice) || !valid_greeting(bytes) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "A greeting needs a short WebM for one of the nine voices.",
        ));
    }
    let dir = store.check(&store.root().join(DIRECTORY))?;
    fs::create_dir_all(&dir)?;
    let path = store.check(&dir.join(format!("{voice}.webm")))?;
    let (mut file, temporary) = temp(store, &dir, "webm")?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temporary.0, path)
}

/// A greeting request is made once per profile. A failed attempt is tried again, and readers only see whole WAVs.
pub fn trigger(store: &Store, speak: impl FnOnce(&Path) -> io::Result<()>) -> io::Result<Vec<u8>> {
    let _serial = lock(&TRIGGER);
    let dir = store.check(&store.root().join(DIRECTORY))?;
    let path = store.check(&dir.join("hello.wav"))?;
    if let Some(bytes) = read(store, &path).filter(|bytes| valid_wave(bytes)) {
        return Ok(bytes);
    }
    fs::create_dir_all(&dir)?;
    let (file, temporary) = temp(store, &dir, "wav")?;
    drop(file);
    speak(&temporary.0)?;
    let bytes = read(store, &temporary.0)
        .filter(|bytes| valid_wave(bytes))
        .ok_or_else(|| io::Error::other("The computer could not make a preview greeting."))?;
    fs::rename(&temporary.0, path)?;
    Ok(bytes)
}

struct Temporary(PathBuf);
impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn temp(store: &Store, dir: &Path, extension: &str) -> io::Result<(File, Temporary)> {
    let filename = format!(
        "preview-{}-{}.{extension}",
        std::process::id(),
        unique_nanos()
    );
    let file = store.create_new_file_in(dir, &filename)?;
    Ok((file, Temporary(dir.join(filename))))
}

fn windows_script(path: &Path) -> Option<String> {
    let quoted = path.to_str()?.replace('\'', "''");
    Some(format!(
        "Add-Type -AssemblyName System.Speech; \
        $s = New-Object System.Speech.Synthesis.SpeechSynthesizer; \
        $f = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(24000, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono); \
        $s.SetOutputToWaveFile('{quoted}', $f); $s.Speak('Hello?'); $s.Dispose()"
    ))
}

pub fn speak_command(platform: &str, path: &Path) -> Option<Command> {
    if platform == "darwin" {
        let mut command = Command::new("/usr/bin/say");
        command.arg("-o").arg(path).args([
            "--file-format=WAVE",
            "--data-format=LEI16@24000",
            "Hello?",
        ]);
        return Some(command);
    }
    if platform != "win32" {
        return None;
    }
    let source = windows_script(path)?;
    let wide: Vec<_> = source.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut encoded = crate::routine_hooks::base64url_encode(&wide)
        .replace('-', "+")
        .replace('_', "/");
    while encoded.len() % 4 != 0 {
        encoded.push('=');
    }
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-EncodedCommand"])
        .arg(encoded);
    Some(command)
}

#[cfg(test)]
#[path = "voice_preview_tests.rs"]
mod tests;

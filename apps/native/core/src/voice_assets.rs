//! The Windows transcription model files, accepted only when their bytes match pinned digests.

use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::Path,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Clone, Copy)]
pub struct Asset {
    pub url: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}

pub const ACCURATE: Asset = Asset {
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-base.en-q5_1.bin",
    bytes: 59_721_011,
    sha256: "4baf70dd0d7c4247ba2b81fafd9c01005ac77c2f9ef064e00dcf195d0e2fdd2f",
};

pub const FAST: Asset = Asset {
    url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-tiny.en-q5_1.bin",
    bytes: 32_166_155,
    sha256: "c77c5766f1cef09b6b7d47f21b546cbddd4157886b3b5d6d4f709e91e66c7c2b",
};

pub const ARCHIVE: Asset = Asset {
    url: "https://github.com/ggml-org/whisper.cpp/releases/download/b5454/whisper-bin-x64.zip",
    bytes: 8_928_640,
    sha256: "6ba69e3482d7826214f90a6a9c84ca07782aec1e1d0c6a7c30c994fd5d816ccb",
};

static INSTALL: Mutex<()> = Mutex::new(());
const WRONG_HASH: &str =
    "The voice typing file did not pass its SHA-256 check. Try Download again.";
const CANCELLED: &str = "The voice typing download was cancelled.";

fn check_cancel(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Acquire) {
        Err(io::Error::new(io::ErrorKind::Interrupted, CANCELLED))
    } else {
        Ok(())
    }
}

fn checked_file(asset: Asset, path: &Path, cancel: &AtomicBool) -> io::Result<bool> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() != asset.bytes {
        return Ok(false);
    }
    let file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut bytes = [0; 32 * 1024];
    let mut read = 0u64;
    let mut file = file.take(asset.bytes.saturating_add(1));
    loop {
        check_cancel(cancel)?;
        let n = file.read(&mut bytes)?;
        if n == 0 {
            break;
        }
        read += n as u64;
        digest.update(&bytes[..n]);
    }
    Ok(read == asset.bytes && format!("{:x}", digest.finalize()) == asset.sha256)
}

fn remove(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}

/// Stream a pinned asset to a temporary sibling. The existing destination can be reused only after checking its
/// present contents. A cancelled or invalid download leaves neither a partial nor a new destination.
pub fn download(
    asset: Asset,
    path: &Path,
    cancel: &AtomicBool,
    fetch: impl FnOnce(&str) -> io::Result<Box<dyn Read>>,
    mut progress: impl FnMut(u64),
) -> io::Result<()> {
    let _install = crate::invariants::lock(&INSTALL);
    let leaf = path.file_name().ok_or(io::Error::new(
        io::ErrorKind::InvalidInput,
        "The voice typing file has no name.",
    ))?;
    let mut name = leaf.to_os_string();
    name.push(".part");
    let partial = path.with_file_name(name);
    remove(&partial)?;
    check_cancel(cancel)?;
    match checked_file(asset, path, cancel) {
        Ok(true) => {
            progress(asset.bytes);
            return Ok(());
        }
        Ok(false) => remove(path)?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let result = (|| {
        let mut body = fetch(asset.url)?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)?;
        let mut digest = Sha256::new();
        let mut bytes = [0; 32 * 1024];
        let mut size = 0u64;
        loop {
            check_cancel(cancel)?;
            let n = body.read(&mut bytes)?;
            if n == 0 {
                break;
            }
            size += n as u64;
            if size > asset.bytes {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "The voice typing download was larger than expected.",
                ));
            }
            digest.update(&bytes[..n]);
            file.write_all(&bytes[..n])?;
            progress(size);
        }
        check_cancel(cancel)?;
        if size != asset.bytes || format!("{:x}", digest.finalize()) != asset.sha256 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, WRONG_HASH));
        }
        file.sync_all()?;
        drop(file);
        fs::rename(&partial, path)
    })();
    if result.is_err() {
        let _ = remove(&partial);
    }
    result
}

#[cfg(test)]
#[path = "voice_assets_tests.rs"]
mod tests;

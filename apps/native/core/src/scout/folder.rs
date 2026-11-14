//! Folder mark for Scout's check: last commit and changed/new file sizes and times.
//!
//! Port of Electron's `main/scout-folder.ts`.

use std::path::Path;
use std::time::UNIX_EPOCH;

use sha2::{Digest, Sha256};

const MOST_FILES: usize = 200;

fn is_porcelain_entry(entry: &str) -> bool {
    let bytes = entry.as_bytes();
    if bytes.len() < 4 || bytes[2] != b' ' {
        return false;
    }
    const STATUS_CHARS: &[u8] = b" MADRCUT?!";
    STATUS_CHARS.contains(&bytes[0]) && STATUS_CHARS.contains(&bytes[1])
}

/// Computes the 32-hex-char mark for `folder`, or `None` when not a git checkout.
/// Notice `git` takes `&[&str]` arguments and `&Path` working directory.
pub fn folder_mark_of<F>(folder: &Path, mut git: F) -> Option<String>
where
    F: FnMut(&[&str], &Path) -> Result<String, String>,
{
    let head = git(&["rev-parse", "HEAD"], folder).ok()?;
    let head = head.trim();
    if head.is_empty() {
        return None;
    }
    let status = git(&["status", "--porcelain", "-z"], folder).unwrap_or_default();
    let entries: Vec<&str> = status
        .split('\0')
        .filter(|entry| is_porcelain_entry(entry))
        .take(MOST_FILES)
        .collect();
    let mut parts = Vec::with_capacity(entries.len() + 1);
    parts.push(head.to_owned());
    for entry in entries {
        let rel = &entry[3..];
        let stamped = match std::fs::metadata(folder.join(rel)) {
            Ok(info) => {
                let mtime_ms = info
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| (d.as_secs_f64() * 1000.0).round() as u128)
                    .unwrap_or(0);
                format!("{}:{mtime_ms}", info.len())
            }
            Err(_) => "-".to_owned(),
        };
        parts.push(format!("{entry}\t{stamped}"));
    }
    let mut hasher = Sha256::new();
    hasher.update(parts.join("\n").as_bytes());
    let hex: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    Some(hex[..32].to_owned())
}

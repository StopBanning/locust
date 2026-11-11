//! Typed commands belong to the person; never pass through agent command guards.
use crate::runtime::process;
use std::{
    io::Read,
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::Instant,
};
pub const MODE_NOTE: &str = "Command mode. This runs in this folder. Escape leaves it.";
pub fn bang(text: &str) -> Option<&str> {
    text.strip_prefix('!').map(str::trim)
}
pub fn validate(command: &str, folder: Option<&Path>) -> Result<(), String> {
    if command.trim().is_empty() {
        return Err("Type a command after !. Nothing was run.".into());
    }
    if command.encode_utf16().count() > 16_000 {
        return Err("That command is too long. Nothing was run.".into());
    }
    if folder.is_none() {
        return Err("Choose a folder first. Nothing was run.".into());
    }
    Ok(())
}

/// Electron `omittedNote`: one sentence for both line and byte omissions.
pub fn omitted_note(omitted_lines: usize, omitted_bytes: usize) -> Option<String> {
    if omitted_lines == 0 && omitted_bytes == 0 {
        return None;
    }
    let mut parts = Vec::new();
    if omitted_lines > 0 {
        parts.push(format!(
            "{omitted_lines} {}",
            if omitted_lines == 1 { "line" } else { "lines" }
        ));
    }
    if omitted_bytes > 0 {
        parts.push(if omitted_bytes >= 1024 {
            format!(
                "{} KB",
                ((omitted_bytes as f64 / 1024.).round() as usize).max(1)
            )
        } else {
            format!("{omitted_bytes} bytes")
        });
    }
    Some(format!("Left out {} from the start.", parts.join(" and ")))
}
pub struct Running {
    child: Child,
    raw: Arc<Mutex<Vec<u8>>>,
    dropped: Arc<std::sync::atomic::AtomicUsize>,
    done: Arc<std::sync::atomic::AtomicUsize>,
    reaped: bool,
    pub stopped: bool,
    began: Instant,
}
pub struct ResultRow {
    pub output: String,
    pub omitted: Option<String>,
    pub exit: Option<i32>,
    pub stopped: bool,
    pub elapsed: u64,
}
impl Running {
    pub fn start(command: &str, folder: &Path) -> Result<Self, String> {
        validate(command, Some(folder))?;
        crate::terminal::check_policy()?;
        let mut cmd = Command::new(crate::terminal::shell());
        cmd.args(["-l", "-c", command]).current_dir(folder);
        let mut child =
            process::spawn(&mut cmd, Stdio::null(), Stdio::piped()).map_err(|e| e.to_string())?;
        let raw = Arc::new(Mutex::new(Vec::new()));
        let dropped = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let done = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        for pipe in [
            child
                .stdout
                .take()
                .map(|p| Box::new(p) as Box<dyn Read + Send>),
            child
                .stderr
                .take()
                .map(|p| Box::new(p) as Box<dyn Read + Send>),
        ]
        .into_iter()
        .flatten()
        {
            let raw = raw.clone();
            let dropped = dropped.clone();
            let done = done.clone();
            std::thread::spawn(move || {
                let mut pipe = pipe;
                let mut buf = [0; 8192];
                while let Ok(n) = pipe.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    let mut held = crate::invariants::lock(&raw);
                    held.extend_from_slice(&buf[..n]);
                    if held.len() > 1_000_000 {
                        let n = held.len() - 500_000;
                        held.drain(..n);
                        dropped.fetch_add(n, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                done.fetch_add(1, std::sync::atomic::Ordering::Release);
            });
        }
        Ok(Self {
            child,
            raw,
            dropped,
            done,
            reaped: false,
            stopped: false,
            began: Instant::now(),
        })
    }
    pub fn stop(&mut self) {
        if !self.reaped {
            self.stopped = true;
            process::end_terminal_tree(self.child.id());
            // Match Electron: if the tree release misses the shell, kill it directly.
            let _ = self.child.kill();
        }
    }
    pub fn poll(&mut self) -> Result<Option<ResultRow>, String> {
        if self.reaped || self.done.load(std::sync::atomic::Ordering::Acquire) < 2 {
            return Ok(None);
        }
        let Some(status) = self.child.try_wait().map_err(|e| e.to_string())? else {
            return Ok(None);
        };
        self.reaped = true;
        let raw = crate::invariants::lock(&self.raw);
        let (output, lines, bytes) = cap(&String::from_utf8_lossy(&raw));
        let dropped = self.dropped.load(std::sync::atomic::Ordering::Relaxed);
        Ok(Some(ResultRow {
            output,
            omitted: omitted_note(lines, bytes.saturating_add(dropped)),
            exit: if self.stopped { None } else { status.code() },
            stopped: self.stopped,
            elapsed: self.began.elapsed().as_millis() as u64,
        }))
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        if !self.reaped {
            process::end_terminal_tree(self.child.id());
            process::kill_reap(&mut self.child);
        }
    }
}
/// Last 200 lines and last 32 KiB of those (Electron `capCommandOutput`).
/// Returns `(text, omitted_lines, omitted_bytes)` over the UTF-8 view of `raw`.
pub fn cap(raw: &str) -> (String, usize, usize) {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<_> = normalized.split('\n').collect();
    let mut count = lines.len().saturating_sub(200);
    let mut text = lines[count..].join("\n");
    let mut bytes = normalized.len() - text.len();
    while text.len() > 32768 {
        let drop = if let Some(i) = text.find('\n') {
            count += 1;
            i + 1
        } else {
            let mut n = text.len() - 32768;
            while !text.is_char_boundary(n) {
                n += 1;
            }
            n
        };
        bytes += drop;
        text.drain(..drop);
    }
    (text, count, bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bang_is_only_at_start_and_escape_keeps_words() {
        assert_eq!(bang("! echo hi"), Some("echo hi"));
        assert_eq!(bang(" !echo"), None);
        assert_eq!("!echo hi".strip_prefix('!'), Some("echo hi"));
    }
    #[test]
    fn bounds_and_exact_refusals() {
        assert_eq!(
            validate("", None).unwrap_err(),
            "Type a command after !. Nothing was run."
        );
        assert!(validate(&"x".repeat(16001), Some(Path::new("/"))).is_err());
        assert_eq!(
            validate("echo hi", None).unwrap_err(),
            "Choose a folder first. Nothing was run."
        );
    }
    #[test]
    fn output_tail_and_unicode() {
        let (text, lines, bytes) = cap(&(0..250).map(|n| format!("{n}\n")).collect::<String>());
        assert!(text.starts_with("51\n"));
        assert_eq!(lines, 51);
        assert!(bytes > 0);
        let note = omitted_note(lines, bytes).unwrap();
        assert!(note.starts_with("Left out 51 lines and "));
        assert!(note.ends_with(" from the start."));
        assert_eq!(note.matches("Left out").count(), 1);
        let (text, lines, bytes) = cap(&"é".repeat(20000));
        assert!(text.len() <= 32768);
        assert!(omitted_note(lines, bytes).is_some());
    }
    #[test]
    fn flood_bytes_merge_into_one_omitted_note() {
        // Electron: omittedNote(cappedLines, cappedBytes + streamDropped) — one sentence.
        let (text, lines, bytes) = cap(&(0..250).map(|n| format!("{n}\n")).collect::<String>());
        assert!(!text.is_empty());
        let note = omitted_note(lines, bytes + 1500).unwrap();
        assert!(note.starts_with("Left out 51 lines and "));
        assert!(note.ends_with(" from the start."));
        assert_eq!(note.matches("Left out").count(), 1);
        assert_eq!(
            omitted_note(0, 500).as_deref(),
            Some("Left out 500 bytes from the start.")
        );
        assert_eq!(
            omitted_note(0, 1024).as_deref(),
            Some("Left out 1 KB from the start.")
        );
    }
    #[test]
    fn command_runs_in_temp_folder_and_reports_exit() {
        let dir = std::env::temp_dir();
        let mut r = Running::start("printf own-command", &dir).unwrap();
        let until = Instant::now() + std::time::Duration::from_secs(3);
        loop {
            if let Some(result) = r.poll().unwrap() {
                assert_eq!(result.output, "own-command");
                assert_eq!(result.exit, Some(0));
                assert!(!result.stopped);
                break;
            }
            assert!(Instant::now() < until);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    #[test]
    fn stop_kills_and_records_stopped_without_exit() {
        let dir = std::env::temp_dir();
        let marker = dir.join(format!("own-stop-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let mut r = Running::start(
            &format!("trap 'touch {}' TERM; sleep 60", marker.display()),
            &dir,
        )
        .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        r.stop();
        let until = Instant::now() + std::time::Duration::from_secs(3);
        let result = loop {
            if let Some(result) = r.poll().unwrap() {
                break result;
            }
            assert!(Instant::now() < until, "stopped command did not finish");
            std::thread::sleep(std::time::Duration::from_millis(5));
        };
        assert!(result.stopped);
        assert_eq!(result.exit, None);
        // SIGKILL from the tree end: the trap never runs and the sleep is gone.
        assert!(!marker.exists());
        let _ = std::fs::remove_file(&marker);
    }
}

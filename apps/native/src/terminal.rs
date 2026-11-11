//! A real PTY with bounded ANSI state. The owner keeps the child unreaped until poll/drop.
use crate::runtime::process;
use portable_pty::{CommandBuilder, MasterPty, PtySize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub fn shell() -> PathBuf {
    std::env::var_os("SHELL")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.is_file())
        .unwrap_or_else(|| "/bin/sh".into())
}
pub struct Terminal {
    pub folder: PathBuf,
    pub screen: Arc<Mutex<vt100::Parser>>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    eof: Arc<AtomicBool>,
    pub exit: Option<u32>,
    size: (u16, u16),
}
impl Terminal {
    pub fn open(folder: &Path, file: &Path, args: &[&str]) -> Result<Self, String> {
        check_policy()?;
        if !folder.is_absolute() || !folder.is_dir() {
            return Err("Choose a project folder first. Your terminals are unchanged.".into());
        }
        let pair = portable_pty::native_pty_system()
            .openpty(size(80, 24))
            .map_err(|e| e.to_string())?;
        // Acquire handles before starting, so a failed setup cannot orphan a shell.
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;
        let mut cmd = CommandBuilder::new(file);
        cmd.args(args);
        cmd.cwd(folder);
        cmd.env("TERM", "xterm-256color");
        let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
        drop(pair.slave);
        let screen = Arc::new(Mutex::new(parser(24, 80)));
        let drawn = screen.clone();
        let eof = Arc::new(AtomicBool::new(false));
        let ended = eof.clone();
        std::thread::spawn(move || {
            let mut bytes = [0; 8192];
            while let Ok(n) = reader.read(&mut bytes) {
                if n == 0 {
                    break;
                }
                crate::invariants::lock(&drawn).process(&bytes[..n]);
            }
            ended.store(true, Ordering::Release);
        });
        Ok(Self {
            folder: folder.into(),
            screen,
            master: pair.master,
            writer,
            child,
            eof,
            exit: None,
            size: (80, 24),
        })
    }
    pub fn input(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.writer
            .write_all(bytes)
            .and_then(|_| self.writer.flush())
            .map_err(|e| e.to_string())
    }
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), String> {
        let cols = cols.clamp(2, 400);
        let rows = rows.clamp(2, 200);
        if self.size != (cols, rows) {
            self.master
                .resize(size(cols, rows))
                .map_err(|e| e.to_string())?;
            crate::invariants::lock(&self.screen).set_size(nz(rows), nz(cols));
            self.size = (cols, rows);
        }
        Ok(())
    }
    pub fn poll(&mut self) -> Result<(), String> {
        if self.exit.is_none()
            && self.eof.load(Ordering::Acquire)
            && let Some(status) = self.child.try_wait().map_err(|e| e.to_string())?
        {
            self.exit = Some(status.exit_code());
        }
        Ok(())
    }
}
fn size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}
impl Drop for Terminal {
    fn drop(&mut self) {
        if self.exit.is_none() {
            if let Some(pid) = self.child.process_id() {
                process::end_terminal_tree(pid);
            }
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pty_printf_ansi_and_exit() {
        let dir = std::env::temp_dir();
        let mut terminal = Terminal::open(
            &dir,
            Path::new("/bin/sh"),
            &["-c", r"printf '\033[31mhello\033[0m\n'"],
        )
        .unwrap();
        terminal.resize(40, 10).unwrap();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(3);
        while terminal.exit.is_none() {
            terminal.poll().unwrap();
            assert!(std::time::Instant::now() < until);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let parser = crate::invariants::lock(&terminal.screen);
        assert!(parser.screen().contents().contains("hello"));
        assert_eq!(
            parser.screen().cell(0, 0).unwrap().fgcolor(),
            vt100::Color::Idx(1)
        );
        assert_eq!(terminal.exit, Some(0));
    }
    #[test]
    fn ansi_overwrite_unicode_and_scrollback() {
        let mut p = parser(2, 10);
        p.process(b"old\rnew\r\nnext\r\nlast");
        assert_eq!(p.screen().contents(), "next\nlast");
        p.screen_mut().set_scrollback(1);
        assert_eq!(p.screen().contents(), "new\nnext");
        p.screen_mut().set_scrollback(0);
        p.process("\x1b[2J\x1b[Hé".as_bytes());
        assert_eq!(p.screen().cell(0, 0).unwrap().contents(), "é");
    }
    #[test]
    fn ansi_hostile_sequences_do_not_panic() {
        let mut p = parser(24, 80);
        for seq in [
            &b"\x1b]0;title with \x07 BEL"[..],
            &b"\x1b]0;unterminated osc"[..],
            &b"\x1b[999999H\x1b[999999;999999H"[..],
            &b"\x1b[38;2;255;0;0mrgb\x1b[0m"[..],
            &b"\x1b[48;5;255mx\x1b[0m"[..],
            &b"\x1b["[..],
            &b"\x1b[?1049h\x1b[?1049l"[..],
            &b"\x1b[?2004h\x1b[200~\x1b[201~\x1b[?2004l"[..],
            &b"\0\0\0"[..],
            &[0xff, 0xfe, 0xfd][..],
            &b"\x1b[3J\x1b[H\x1b[2J"[..],
            &b"\x1b(B\x1b)0\x0en\x0f"[..],
            &b"\x1b[6n\x1b[c\x1b[>c"[..],
        ] {
            p.process(seq);
        }
        let mut flood = vec![0x1b, b'['];
        flood.extend(std::iter::repeat_n(b';', 4096));
        flood.extend(b"0m\x1b[0m".iter().copied().cycle().take(10_000));
        p.process(&flood);
        let _ = p.screen().contents();
        let _ = p.screen().cursor_position();
        let _ = p.screen().size();
    }
    #[test]
    fn drop_kills_and_reaps_pty_shell() {
        let dir = std::env::temp_dir();
        let marker = dir.join(format!("p9-pty-drop-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let script = format!(
            "echo $$ > {}; exec sleep 120",
            marker.display()
        );
        let terminal = Terminal::open(&dir, Path::new("/bin/sh"), &["-c", &script]).unwrap();
        let until = std::time::Instant::now() + std::time::Duration::from_secs(2);
        let pid = loop {
            if let Ok(body) = std::fs::read_to_string(&marker)
                && let Ok(pid) = body.trim().parse::<u32>()
            {
                break pid;
            }
            assert!(std::time::Instant::now() < until, "pty did not publish pid");
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        drop(terminal);
        let listed = std::process::Command::new("/bin/ps")
            .args(["-p", &pid.to_string(), "-o", "pid="])
            .output()
            .map(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
            .unwrap_or(true);
        let _ = std::fs::remove_file(&marker);
        assert!(!listed, "pty shell {pid} still listed after drop");
    }
}

pub fn nz(n: u16) -> std::num::NonZeroU16 {
    std::num::NonZeroU16::new(n).unwrap_or(std::num::NonZeroU16::MIN)
}
pub fn parser(rows: u16, cols: u16) -> vt100::Parser {
    vt100::Parser::new(nz(rows), nz(cols), 10_000)
}

pub fn policy_refusal(policy: &serde_json::Value, path: &Path) -> Option<String> {
    let refusal = |key: &str, why: &str| {
        Some(format!(
            "{why} Set by your organization ({key} in {}). Nothing was started.",
            path.display()
        ))
    };
    if policy["terminal"] == false {
        return refusal("terminal", "The terminal panel is off.");
    }
    for key in ["allowedAgents", "allowedModels", "monthlyLimitUsd"] {
        if !policy[key].is_null() {
            return refusal(
                key,
                "An interactive shell cannot enforce this organization restriction. Use a conversation in Locust.",
            );
        }
    }
    if policy["highestMode"].as_str().is_some_and(|s| s != "auto") {
        return refusal(
            "highestMode",
            "An interactive shell cannot enforce the highest permission mode. Use a conversation in Locust.",
        );
    }
    None
}
pub fn check_policy() -> Result<(), String> {
    let path = Path::new(if cfg!(target_os = "macos") {
        "/Library/Application Support/Locust/policy.json"
    } else {
        "/etc/locust/policy.json"
    });
    let body = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(format!(
                "Organization policy could not be read ({}): {e}. Nothing was started.",
                path.display()
            ));
        }
    };
    let value: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|e| format!("Organization policy could not be read: {e}. Nothing was started."))?;
    if let Some(e) = policy_refusal(&value, path) {
        return Err(e);
    }
    Ok(())
}

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
        Self::open_unless(check_policy().err(), folder, file, args)
    }
    /// Opens a shell unless `refused` says why not: the refusal, or a folder that is not one, comes before anything is
    /// spawned (Electron's terminal host refuses a direct request the same way).
    pub(crate) fn open_unless(refused: Option<String>, folder: &Path, file: &Path, args: &[&str]) -> Result<Self, String> {
        if let Some(refused) = refused {
            return Err(refused);
        }
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
#[path = "a_terminal_belongs_to_the_person_tests.rs"]
mod a_terminal_belongs_to_the_person_tests;

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
        let marker = dir.join(format!("pty-drop-{}", std::process::id()));
        let _ = std::fs::remove_file(&marker);
        let script = format!("echo $$ > {}; exec sleep 120", marker.display());
        let start = std::time::Instant::now();
        let terminal = Terminal::open(&dir, Path::new("/bin/sh"), &["-c", &script]).unwrap();
        // The shell writes its pid once it is scheduled: two seconds was not always enough on a loaded machine.
        let mut pid = 0;
        crate::load_waits::wait_for("the pty shell to publish its pid", || {
            pid = std::fs::read_to_string(&marker).ok().and_then(|body| body.trim().parse().ok()).unwrap_or(0);
            pid != 0
        });
        // Drop kills the shell and waits for it, so it is gone the moment drop returns: no wait here.
        drop(terminal);
        crate::load_waits::note_time("drop_kills_and_reaps_pty_shell", start.elapsed());
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

/// Whether the organization lets a terminal open, read from the machine's policy file each time it is asked
/// (`policyTerminalRefusal` over Electron's policy reader): refused, in words, when the terminal is off, when the policy
/// limits agents, models, spend or modes a shell cannot enforce, or when the policy cannot be used.
pub fn check_policy() -> Result<(), String> {
    check_policy_at(&crate::org_policy::policy_path(if cfg!(target_os = "macos") { "darwin" } else { "linux" }, None))
}

/// [`check_policy`] over the policy file at `path`.
pub(crate) fn check_policy_at(path: &str) -> Result<(), String> {
    let state = crate::org_policy::read_policy(path, &|p| std::fs::read(p), &mut |_| {});
    match crate::org_policy::terminal_refusal(Some(&state)) {
        Some(refused) => Err(refused),
        None => Ok(()),
    }
}

/// Which conversation or teammate a terminal opens for (Electron `TerminalContext`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalContext {
    pub mission_id: Option<String>,
    pub teammate_id: Option<String>,
}

/// A teammate as the terminal's folder needs it: its id, its own folder if it has one, and whether it works in a
/// worktree of its own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalTeammate {
    pub teammate_id: String,
    pub folder: Option<String>,
    pub worktree: bool,
}

/// What the host knows, handed in (Electron `terminalFolder`'s options).
pub struct TerminalFolderFacts<'a> {
    pub current: &'a dyn Fn() -> Option<String>,
    /// A conversation's workspace id, or `None` for a conversation Locust does not know.
    pub mission: &'a dyn Fn(&str) -> Option<String>,
    pub folder_of: &'a dyn Fn(&str) -> Option<String>,
    pub owner_of: &'a dyn Fn(&str) -> Option<String>,
    pub teammates: &'a dyn Fn() -> Vec<TerminalTeammate>,
    /// The worktrees under a folder: each one's teammate and path.
    pub trees: &'a dyn Fn(&str) -> Vec<(String, String)>,
}

/// The folder a terminal opens in, resolved from Locust's own state, never guessed (Electron `terminalFolder`): the
/// current project before a first message, a conversation's workspace afterwards, the teammate's own folder when it has
/// one, and its worktree when it works in one; an unknown conversation, teammate or worktree is `None`. A worktree is
/// never made just to open a shell.
pub fn terminal_folder(context: &TerminalContext, facts: &TerminalFolderFacts) -> Option<String> {
    let mut folder = (facts.current)();
    let mut owner = context.teammate_id.clone();
    if let Some(mission_id) = &context.mission_id {
        let workspace = (facts.mission)(mission_id)?;
        folder = (facts.folder_of)(&workspace);
        owner = (facts.owner_of)(mission_id);
    }
    let teammate = match &owner {
        Some(owner) => Some((facts.teammates)().into_iter().find(|one| &one.teammate_id == owner)?),
        None => None,
    };
    let root = teammate.as_ref().and_then(|t| t.folder.clone()).or(folder)?;
    let Some(teammate) = teammate.filter(|t| t.worktree) else {
        return Some(root);
    };
    (facts.trees)(&root).into_iter().find(|(id, _)| *id == teammate.teammate_id).map(|(_, path)| path)
}

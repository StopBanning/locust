//! Muse Code exec driver. Ports createMuseExecCommand (prompt-file) + the-prompt-muse-cannot-be-told.
use super::process::{Control, interrupt_process, kill_process};
use super::{Event, Runtime, Turn, muse_events::MuseNormalizer};
use crate::approval::Mode;
use serde_json::Value;
use std::{
    collections::VecDeque,
    io::{self, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError},
    },
    time::{Duration, Instant},
};

const MAX_LINE: u64 = 8 * 1024 * 1024;
pub const PROMPT_FILE_PLACEHOLDER: &str = "__LOCUST_PROMPT_FILE__";
pub const MUSE_EFFORTS: &[&str] = &[
    "none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra",
];

pub struct Muse {
    child: Child,
    control: Arc<Control>,
    lines: Receiver<io::Result<Value>>,
    normal: MuseNormalizer,
    queued: VecDeque<Event>,
    stopping: Option<Instant>,
    cancel: Option<Arc<AtomicBool>>,
    exited: bool,
    exit_code: Option<i32>,
    prompt_file: Option<PathBuf>,
}

fn err(message: impl ToString) -> io::Error {
    io::Error::other(message.to_string())
}

pub fn sandbox_for(mode: Mode) -> &'static str {
    match mode {
        Mode::Auto => "full-access",
        Mode::AcceptEdits | Mode::ApproveEach => "workspace-write",
        Mode::Ask | Mode::Plan => "read-only",
    }
}

pub fn exec_args(
    workspace: &Path,
    mode: Mode,
    model: Option<&str>,
    effort: Option<&str>,
    resume: Option<&str>,
) -> io::Result<Vec<String>> {
    let sandbox = sandbox_for(mode);
    let mut args = vec![
        "exec".into(),
        "--json".into(),
        "--prompt-file".into(),
        PROMPT_FILE_PLACEHOLDER.into(),
        "--workspace".into(),
        workspace.to_string_lossy().into_owned(),
    ];
    if let Some(model) = model.map(str::trim).filter(|m| !m.is_empty()) {
        args.push("--model".into());
        args.push(model.into());
    }
    if let Some(effort) = effort.map(str::trim).filter(|e| !e.is_empty()) {
        if !MUSE_EFFORTS.contains(&effort) {
            return Err(err(format!(
                "Muse Code takes no reasoning effort called \"{effort}\""
            )));
        }
        args.push("--reasoning-effort".into());
        args.push(effort.into());
    }
    if let Some(resume) = resume.map(str::trim).filter(|s| !s.is_empty()) {
        args.push("--session-id".into());
        args.push(resume.into());
    }
    if sandbox == "read-only" {
        args.push("--disable-write".into());
        args.push("--disable-shell".into());
    } else {
        args.push("--approval-mode".into());
        args.push("never".into());
        if sandbox == "full-access" {
            args.push("--disable-sandbox".into());
        }
    }
    Ok(args)
}

pub fn command(
    folder: &Path,
    mode: Mode,
    model: Option<&str>,
    effort: Option<&str>,
    resume: Option<&str>,
) -> io::Result<Command> {
    if mode == Mode::ApproveEach {
        return Err(err(
            "Muse does not support Approve-each in the native driver.",
        ));
    }
    let mut c = Command::new(super::discovery::find_named("muse")?);
    c.args(exec_args(folder, mode, model, effort, resume)?)
        .current_dir(folder);
    Ok(c)
}

impl Muse {
    #[cfg(test)]
    #[cfg(test)]
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        Self::spawn_with_prompt_file(command, None)
    }
    pub fn spawn_with_prompt_file(
        command: &mut Command,
        prompt_file: Option<PathBuf>,
    ) -> io::Result<Self> {
        let mut child = super::process::spawn(command, Stdio::null(), Stdio::null())?;
        let control = Arc::new(Control::default());
        let pid = child.id();
        control.watch(pid);
        // spawn configured a piped stdout; no prior take occurs.
        let output = child
            .stdout
            .take()
            // Invariant: spawn configured this pipe and it has not been taken.
            .expect("spawn configured this pipe and it has not been taken");
        let (tx, lines) = mpsc::sync_channel(256);
        let reader_control = control.clone();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                let mut line = Vec::new();
                match super::process::read_line(&mut reader, &mut line) {
                    Ok(0) => break,
                    Ok(_) if line.len() as u64 > MAX_LINE => {
                        let _ = tx.try_send(Err(err("Muse stream-json line exceeds 8 MiB")));
                        break;
                    }
                    Ok(_) => {
                        let trimmed = if line.last() == Some(&b'\n') {
                            &line[..line.len() - 1]
                        } else {
                            &line[..]
                        };
                        let trimmed = if trimmed.last() == Some(&b'\r') {
                            &trimmed[..trimmed.len() - 1]
                        } else {
                            trimmed
                        };
                        if trimmed.is_empty() {
                            continue;
                        }
                        // Strip UTF-8 BOM that PowerShell redirects sometimes put on line 1.
                        let trimmed = if trimmed.starts_with(&[0xEF, 0xBB, 0xBF]) {
                            &trimmed[3..]
                        } else {
                            trimmed
                        };
                        let value = serde_json::from_slice::<Value>(trimmed)
                            .map_err(|_| err("Muse sent malformed json"));
                        match tx.try_send(value) {
                            Ok(()) => {}
                            Err(mpsc::TrySendError::Full(_)) => {
                                reader_control.fail("Muse notification queue overflow");
                                break;
                            }
                            Err(mpsc::TrySendError::Disconnected(_)) => break,
                        }
                    }
                    Err(e) => {
                        let _ = tx.try_send(Err(e));
                        break;
                    }
                }
            }
        });
        Ok(Self {
            child,
            control,
            lines,
            normal: MuseNormalizer::default(),
            queued: VecDeque::new(),
            stopping: None,
            cancel: None,
            exited: false,
            exit_code: None,
            prompt_file,
        })
    }
    pub fn with_cancel(mut self, flag: Arc<AtomicBool>) -> Self {
        self.cancel = Some(flag);
        self
    }
    fn cancelled(&self) -> bool {
        crate::run_cli::interrupted()
            || self
                .cancel
                .as_ref()
                .is_some_and(|f| f.load(Ordering::Relaxed))
    }
    fn receive(&self) -> io::Result<Option<Value>> {
        if let Some(message) = self.control.failure() {
            return Err(err(message));
        }
        match self.lines.recv_timeout(Duration::from_millis(50)) {
            Ok(v) => v.map(Some),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(_) => Err(err("Muse transport disconnected")),
        }
    }
    fn poll_exit(&mut self) {
        if self.exited {
            return;
        }
        if let Ok(Some(status)) = self.child.try_wait() {
            self.exited = true;
            self.exit_code = status.code();
        }
    }
    fn await_exit(&mut self, budget: Duration) {
        let deadline = Instant::now() + budget;
        while !self.exited && Instant::now() < deadline {
            self.poll_exit();
            if self.exited {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        self.poll_exit();
    }
}

impl Runtime for Muse {
    fn start(&mut self, turn: &Turn) -> io::Result<()> {
        if turn.mode == Mode::ApproveEach {
            return Err(err(
                "Muse does not support Approve-each in the native driver.",
            ));
        }
        if turn.prompt.trim().is_empty() {
            return Err(err("Prompt must be nonempty"));
        }
        // Prompt already substituted by the host when using hooks; for real launches the
        // command was built with a placeholder the host must rewrite before spawn.
        // Synthetic fakes ignore argv. Keep a prompt file for cleanliness when present.
        let _ = turn;
        Ok(())
    }
    fn next_event(&mut self) -> io::Result<Option<Event>> {
        if self.normal.ended {
            return Ok(self.queued.pop_front());
        }
        if let Some(message) = self.control.failure() {
            self.queued.clear();
            let d = self
                .normal
                .diagnostic("error", "native.transport", &message);
            self.queued.extend(
                self.normal
                    .finish(self.stopping.is_some() || self.cancelled(), self.exit_code),
            );
            return Ok(Some(d));
        }
        if self
            .stopping
            .is_some_and(|t| t.elapsed() >= Duration::from_secs(5))
        {
            self.queued.clear();
            kill_process(self.child.id());
            self.queued.extend(self.normal.finish(true, self.exit_code));
            return Ok(self.queued.pop_front());
        }
        if let Some(e) = self.queued.pop_front() {
            return Ok(Some(e));
        }
        self.poll_exit();
        match self.receive() {
            Ok(Some(v)) => {
                self.queued.extend(self.normal.accept(&v));
                Ok(self.queued.pop_front())
            }
            Ok(None) => Ok(None),
            Err(_) => {
                while let Ok(Ok(v)) = self.lines.try_recv() {
                    self.queued.extend(self.normal.accept(&v));
                }
                if let Some(e) = self.queued.pop_front() {
                    return Ok(Some(e));
                }
                self.await_exit(Duration::from_millis(500));
                self.queued.extend(
                    self.normal
                        .finish(self.stopping.is_some() || self.cancelled(), self.exit_code),
                );
                Ok(self.queued.pop_front())
            }
        }
    }
    fn stop(&mut self) -> io::Result<()> {
        if self.stopping.is_some() {
            return Ok(());
        }
        let now = Instant::now();
        self.stopping = Some(now);
        self.control.cancel_at(now + Duration::from_secs(5));
        interrupt_process(self.child.id());
        Ok(())
    }
}
impl Drop for Muse {
    fn drop(&mut self) {
        if let Some(path) = self.prompt_file.take() {
            let _ = std::fs::remove_file(path);
        }
        self.control.close_and_kill(self.child.id());
        super::process::kill_reap(&mut self.child);
    }
}

/// Substitute the prompt-file placeholder and write the prompt (the-prompt-muse-cannot-be-told).
pub fn prepare_prompt_file(
    command: &mut Command,
    prompt: &str,
    store: &crate::store::Store,
) -> io::Result<PathBuf> {
    let dir = store
        .ledger_dir()?
        .parent()
        .ok_or_else(|| io::Error::other("Native ledger has no parent directory"))?
        .join(format!(
            "muse-prompt-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_nanos()
        ));
    let path = dir.join("prompt.txt");
    store
        .create_new_file_in(&dir, "prompt.txt")?
        .write_all(prompt.as_bytes())?;
    let args: Vec<_> = command
        .get_args()
        .map(|a| {
            if a == PROMPT_FILE_PLACEHOLDER {
                path.as_os_str().to_owned()
            } else {
                a.to_owned()
            }
        })
        .collect();
    let rebuilt = super::process::copy_command(command);
    // copy_command includes argv; rebuild only after capturing all launch settings.
    let mut replacement = Command::new(rebuilt.get_program());
    replacement.args(args);
    if let Some(cwd) = rebuilt.get_current_dir() {
        replacement.current_dir(cwd);
    }
    for (k, v) in rebuilt.get_envs() {
        if let Some(v) = v {
            replacement.env(k, v);
        } else {
            replacement.env_remove(k);
        }
    }
    *command = replacement;
    Ok(path)
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    #[test]
    fn prompt_goes_in_a_file_not_argv() {
        let args = exec_args(Path::new("/tmp/ws"), Mode::Ask, None, None, None).unwrap();
        assert!(
            args.windows(2)
                .any(|w| w == ["--prompt-file", PROMPT_FILE_PLACEHOLDER])
        );
        assert!(args.contains(&"--disable-write".into()));
        assert!(args.contains(&"--disable-shell".into()));
        assert!(!args.iter().any(|a| a == "--yolo"));
    }
    #[test]
    fn auto_disables_sandbox() {
        let args = exec_args(
            Path::new("/tmp/ws"),
            Mode::Auto,
            Some("muse-1"),
            Some("high"),
            None,
        )
        .unwrap();
        assert!(args.contains(&"--disable-sandbox".into()));
        assert!(args.windows(2).any(|w| w == ["--approval-mode", "never"]));
        assert!(args.windows(2).any(|w| w == ["--reasoning-effort", "high"]));
    }
    #[test]
    fn unknown_effort_is_refused() {
        assert!(exec_args(Path::new("/tmp"), Mode::Ask, None, Some("ludicrous"), None).is_err());
    }
}

//! Shared child-process mechanics. Drivers retain protocol parsing and lifecycle policy.
use std::{
    io::{self, BufRead, BufReader, Read},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};
pub(crate) fn copy_command(c: &Command) -> Command {
    let mut out = Command::new(c.get_program());
    out.args(c.get_args());
    if let Some(dir) = c.get_current_dir() {
        out.current_dir(dir);
    }
    for (k, v) in c.get_envs() {
        if let Some(v) = v {
            out.env(k, v);
        } else {
            out.env_remove(k);
        }
    }
    out
}

pub(crate) struct Process {
    pub child: Child,
    pub lines: Receiver<String>,
    pub stderr: Arc<Mutex<String>>,
    pub overflow: Arc<AtomicBool>,
    closed: Arc<AtomicBool>,
    lock: Arc<Mutex<()>>,
    reaped: bool,
}
impl Process {
    pub fn spawn(c: &mut Command) -> io::Result<Self> {
        let mut child = spawn(c, Stdio::null(), Stdio::piped())?;
        let closed = Arc::new(AtomicBool::new(false));
        let lock = Arc::new(Mutex::new(()));
        let overflow = Arc::new(AtomicBool::new(false));
        let pid = child.id();
        let stderr = stderr_tail(
            child
                .stderr
                .take()
                // Invariant: spawn configured this pipe and it has not been taken.
                .expect("spawn configured this pipe and it has not been taken"),
        );
        let (tx, lines) = mpsc::sync_channel(256);
        let out = child
            .stdout
            .take()
            // Invariant: spawn configured this pipe and it has not been taken.
            .expect("spawn configured this pipe and it has not been taken");
        let done = closed.clone();
        let guard = lock.clone();
        let full = overflow.clone();
        std::thread::spawn(move || {
            let mut r = BufReader::new(out);
            loop {
                let mut b = vec![];
                match read_line(&mut r, &mut b) {
                    Ok(0) | Err(_) => break,
                    Ok(_) if b.len() > 8 * 1024 * 1024 => {
                        full.store(true, Ordering::Relaxed);
                        break;
                    }
                    _ => {}
                }
                match tx.try_send(String::from_utf8_lossy(&b).into_owned()) {
                    Ok(()) => {}
                    Err(mpsc::TrySendError::Full(_)) => {
                        full.store(true, Ordering::Relaxed);
                        break;
                    }
                    Err(_) => break,
                }
            }
            if full.load(Ordering::Relaxed) {
                let _g = crate::invariants::lock(&guard);
                if !done.load(Ordering::Relaxed) {
                    kill(pid);
                }
            }
        });
        Ok(Self {
            child,
            lines,
            stderr,
            overflow,
            closed,
            lock,
            reaped: false,
        })
    }
    pub fn exited(&mut self) -> io::Result<Option<i32>> {
        if self.reaped {
            return Ok(Some(-1));
        }
        let _g = crate::invariants::lock(&self.lock);
        if let Some(status) = self.child.try_wait()? {
            self.closed.store(true, Ordering::Relaxed);
            self.reaped = true;
            return Ok(Some(status.code().unwrap_or(-1)));
        }
        Ok(None)
    }
    pub fn stop(&mut self) -> bool {
        let _g = crate::invariants::lock(&self.lock);
        self.closed.store(true, Ordering::Relaxed);
        if self.reaped {
            return true;
        }
        kill(self.child.id());
        let _ = self.child.kill();
        self.reaped = confirm_exit(&mut self.child, Duration::from_secs(1));
        self.reaped
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        self.stop();
        if !self.reaped {
            let _ = self.child.wait();
        }
    }
}

/// Kill the owned process group. ACP/OpenCode intentionally signal only the group.
pub(crate) fn kill(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
}
pub(crate) fn kill_process(pid: u32) {
    kill(pid);
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
}
pub(crate) fn interrupt_process(pid: u32) {
    #[cfg(unix)]
    unsafe {
        libc::kill(-(pid as i32), libc::SIGINT);
        libc::kill(pid as i32, libc::SIGINT);
    }
}
pub(crate) fn own_group(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
}
#[derive(Default)]
pub(crate) struct ControlState {
    pub failure: Option<String>,
    pub deadline: Option<Instant>,
    pub closed: bool,
}
#[derive(Default)]
pub(crate) struct Control {
    pub state: Mutex<ControlState>,
    pub wake: std::sync::Condvar,
}
impl Control {
    pub fn fail(&self, message: impl ToString) {
        let mut state = crate::invariants::lock(&self.state);
        if state.failure.is_none() {
            state.failure = Some(message.to_string());
        }
        self.wake.notify_one();
    }
    pub fn failure(&self) -> Option<String> {
        crate::invariants::lock(&self.state).failure.clone()
    }
    pub fn cancel_at(&self, deadline: Instant) {
        crate::invariants::lock(&self.state).deadline = Some(deadline);
        self.wake.notify_one();
    }
    /// Hold the same lock as the watchdog until it can no longer signal this PID.
    pub fn close_and_kill(&self, pid: u32) {
        let mut state = crate::invariants::lock(&self.state);
        state.closed = true;
        self.wake.notify_one();
        kill_process(pid);
    }
    pub fn watch(self: &Arc<Self>, pid: u32) {
        let watchdog = self.clone();
        std::thread::spawn(move || {
            let mut state = crate::invariants::lock(&watchdog.state);
            loop {
                if state.closed {
                    break;
                }
                if state.failure.is_some() || state.deadline.is_some_and(|d| Instant::now() >= d) {
                    kill_process(pid);
                    break;
                }
                let wait = state
                    .deadline
                    .map(|d| d.saturating_duration_since(Instant::now()))
                    .unwrap_or(Duration::from_secs(60));
                state = watchdog
                    .wake
                    .wait_timeout(state, wait)
                    .expect("internal state mutation must not panic")
                    .0;
            }
        });
    }
}
/// Bound allocation even when a child never writes a newline. Protocol parsing stays with the driver.
pub(crate) fn read_line(reader: &mut impl BufRead, line: &mut Vec<u8>) -> io::Result<usize> {
    reader.take(8 * 1024 * 1024 + 1).read_until(b'\n', line)
}
/// Read the same 16 KiB display tail used by ACP and OpenCode, with bounded chunks.
pub(crate) fn stderr_tail(mut pipe: impl Read + Send + 'static) -> Arc<Mutex<String>> {
    let tail = Arc::new(Mutex::new(String::new()));
    let output = tail.clone();
    std::thread::spawn(move || {
        let mut bytes = [0u8; 8192];
        while let Ok(n) = pipe.read(&mut bytes) {
            if n == 0 {
                break;
            }
            let mut held = crate::invariants::lock(&output);
            held.push_str(&String::from_utf8_lossy(&bytes[..n]));
            *held = super::events::text(&held, 16384);
        }
    });
    tail
}

/// Configure each runtime's pipe policy, then isolate the child from the host process group.
pub(crate) fn spawn(command: &mut Command, stdin: Stdio, stderr: Stdio) -> io::Result<Child> {
    own_group(command);
    command
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn()
}
/// Call while holding the driver's lifecycle lock, before the child can be reaped/reused.
pub(crate) fn kill_reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}
/// Returns false when exit has not been confirmed within the caller's existing budget.
/// Callers report this as terminationUnconfirmed rather than inventing a successful exit.
pub(crate) fn confirm_exit(child: &mut Child, timeout: Duration) -> bool {
    let until = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return true,
            Err(_) => return false,
            Ok(None) => {}
        }
        if Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Copilot reports refusals from the beginning of stderr, unlike the other drivers.
/// Keep its line-boundary prefix policy, but bound an unterminated line like stdout.
pub(crate) fn stderr_prefix(pipe: impl Read + Send + 'static) -> Arc<Mutex<String>> {
    let output = Arc::new(Mutex::new(String::new()));
    let buffer = output.clone();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(pipe);
        loop {
            let mut line = Vec::new();
            match read_line(&mut reader, &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    // Preserve read_line(String)'s refusal to capture invalid UTF-8.
                    let Ok(line) = String::from_utf8(line) else {
                        break;
                    };
                    let mut held = crate::invariants::lock(&buffer);
                    if held.len() < 64 * 1024 {
                        held.push_str(&line);
                    }
                }
            }
        }
    });
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unterminated_output_cannot_force_unbounded_allocation() {
        let mut reader = BufReader::new(io::repeat(b'x').take(32 * 1024 * 1024));
        let mut line = Vec::new();
        let read = read_line(&mut reader, &mut line).unwrap();
        assert_eq!(read, 8 * 1024 * 1024 + 1);
        assert_eq!(line.len(), read);
    }
    #[test]
    fn overflow_kills_owned_child_without_event_polling() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "while :; do echo record; done"]);
        let mut child = Process::spawn(&mut command).unwrap();
        let until = Instant::now() + Duration::from_secs(5);
        while !child.overflow.load(Ordering::Relaxed) {
            assert!(
                Instant::now() < until,
                "reader did not detect bounded queue overflow"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(confirm_exit(&mut child.child, Duration::from_secs(1)));
        child.reaped = true;
    }
}

/// ACP's grace period is three seconds, and it signals only the process group.
/// Its reap lock prevents the delayed signal from reaching a reused PID.
pub(crate) fn watch_group(
    pid: u32,
    closed: Arc<AtomicBool>,
    reap_lock: Arc<Mutex<()>>,
    delay: Duration,
) {
    std::thread::spawn(move || {
        std::thread::sleep(delay);
        let _guard = crate::invariants::lock(&reap_lock);
        if !closed.load(Ordering::Relaxed) {
            kill(pid);
        }
    });
}

//! A COMMAND IN A TERMINAL NOBODY SEES, on macOS and Linux (pseudo-terminal.ts's helper; there a Windows pseudo
//! console reached through PowerShell, here a pty from portable-pty). `claude --teleport` refuses anything but an
//! interactive terminal, so it runs inside one no window shows. What it draws comes back; the terminal's one question
//! (where is the cursor?) is answered, since a program that asks it waits for the answer. When what it draws matches
//! `stop_when`, a transcript in `stop_when_written` is there and no longer growing, or `seconds` pass, the program and
//! everything it started are ended and reaped, and nothing is answered.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant, SystemTime};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use regex::Regex;

use super::{PseudoTerminalResult, PseudoTerminalRun};

/// A tenth of a second between looks, as the helper's `WaitForSingleObject(pi.hProcess, 100)`.
const TICK: Duration = Duration::from_millis(100);

/// Runs `run.argv` in a hidden terminal and gives back what it drew. A run with no argv (Windows' `line`, which only
/// a pseudo console takes as is) or a `stop_when` that is not a pattern is not started.
pub fn run_in_pseudo_terminal(run: &PseudoTerminalRun) -> PseudoTerminalResult {
    let Some((program, args)) = run.argv.split_first() else {
        return PseudoTerminalResult::Err;
    };
    // The helper's .NET Regex throws on a bad pattern, and the run comes back not ok.
    let stop = match run.stop_when.as_str() {
        "" => None,
        pattern => match Regex::new(pattern) {
            Ok(stop) => Some(stop),
            Err(_) => return PseudoTerminalResult::Err,
        },
    };
    let csi = Regex::new(r"\x1b\[[0-9;?<>=]*[ -/]*[@-~]").expect("csi");
    let began = SystemTime::now() - Duration::from_secs(5);
    // One terminal made and started at a time in this process: portable-pty marks a new pty close-on-exec only after
    // making it, and starts the program through a fork that lists and closes descriptors before exec. Two at once
    // (two reads, or a read beside a test) measured a program dead before it drew anything, about one run in ten.
    static STARTING: Mutex<()> = Mutex::new(());
    let starting = STARTING.lock().unwrap_or_else(|e| e.into_inner());
    let Ok(pair) = native_pty_system().openpty(PtySize { rows: 50, cols: 400, pixel_width: 0, pixel_height: 0 }) else {
        return PseudoTerminalResult::Err;
    };
    let mut command = CommandBuilder::new(program);
    command.args(args);
    if !run.cwd.is_empty() {
        command.cwd(&run.cwd);
    }
    for key in &run.env_remove {
        command.env_remove(key);
    }
    for (key, value) in &run.env {
        command.env(key, value);
    }
    let Ok(mut child) = pair.slave.spawn_command(command) else {
        return PseudoTerminalResult::Err;
    };
    // Only the program holds the terminal's other end, so the reader sees its end when the program's tree ends.
    drop(pair.slave);
    drop(starting);
    let drawn = Arc::new(Mutex::new(Vec::<u8>::new()));
    let (read_done, reader_ended) = mpsc::channel::<()>();
    if let Ok(mut reader) = pair.master.try_clone_reader() {
        let drawn = drawn.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            while let Ok(n) = reader.read(&mut buf) {
                if n == 0 {
                    break;
                }
                drawn.lock().unwrap_or_else(|e| e.into_inner()).extend_from_slice(&buf[..n]);
            }
            let _ = read_done.send(());
        });
    }
    let mut input = pair.master.take_writer().ok();
    let mut answered = false;
    let mut exited = false;
    let mut seen: i64 = -1;
    for tick in 0..run.seconds.saturating_mul(10) {
        if exited_within(child.as_mut(), TICK) {
            exited = true;
            break;
        }
        // A file the program writes, there and no longer growing: what was waited for is done.
        if let Some(watch) = run.stop_when_written.as_deref().filter(|_| tick % 10 == 9) {
            let now = newest_length(Path::new(watch), began);
            if now > 0 && now == seen {
                break;
            }
            seen = now;
        }
        let text = String::from_utf8_lossy(&drawn.lock().unwrap_or_else(|e| e.into_inner())).into_owned();
        if !answered && text.contains("\x1b[6n") {
            if let Some(input) = input.as_mut() {
                let _ = input.write_all(b"\x1b[1;1R");
                let _ = input.flush();
            }
            answered = true;
        }
        if stop.as_ref().is_some_and(|stop| stop.is_match(&csi.replace_all(&text, ""))) {
            break;
        }
    }
    if !exited {
        // taskkill /T /F: the program and everything it started, past any signal it ignores.
        if let Some(pid) = child.process_id() {
            crate::runtime::process::end_terminal_tree(pid);
        }
        let _ = child.kill();
    }
    let _ = child.wait();
    drop(input);
    // reader.Join(1000): a grandchild still holding the terminal does not hold the answer up.
    let _ = reader_ended.recv_timeout(Duration::from_secs(1));
    let bytes = drawn.lock().unwrap_or_else(|e| e.into_inner()).clone();
    PseudoTerminalResult::Ok { drawn: String::from_utf8_lossy(&bytes).into_owned() }
}

/// True once the program has ended, looking every hundredth of a second for at most `wait`.
fn exited_within(child: &mut (dyn portable_pty::Child + Send + Sync), wait: Duration) -> bool {
    let until = Instant::now() + wait;
    loop {
        if matches!(child.try_wait(), Ok(Some(_))) {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The length of the longest `.jsonl` in `folder` written since `began`; -1 when there is none.
fn newest_length(folder: &Path, began: SystemTime) -> i64 {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return -1;
    };
    entries
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jsonl"))
        .filter_map(|entry| entry.metadata().ok())
        .filter(|meta| meta.is_file() && meta.modified().is_ok_and(|at| at >= began))
        .map(|meta| meta.len() as i64)
        .max()
        .unwrap_or(-1)
}

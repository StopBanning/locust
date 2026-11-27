//! ROW 4, TWO AT ONCE: again and again (50 times in the long form), a second engine on the same data folder is refused
//! with "Another Locust engine is already running." (engine_process.rs) and the lock is free again after the first
//! exits, whether it quit or was killed (SIGKILL, which leaves its socket file behind).
//!
//! Each round: engine A (this binary again, `engine_child`, a real `Server::start`) takes the folder; this process and a
//! second child are both refused; A exits (quitting on even rounds, killed on odd ones); this process then takes the
//! lock and lets it go; and two children started at once race for it, of which exactly one may win.
use super::*;
use crate::engine_process::{LOCK, SOCKET, Server};
use std::io::{BufRead, BufReader};
use std::process::Stdio;

const CHILD_TEST: &str = "soak_tests::two_at_once::engine_child";
const REFUSED: &str = "Another Locust engine is already running.";

/// The child, by SOAK_MODE: `hold` takes the folder and keeps it until its stdin closes; `race` tries once and, when
/// it wins, holds for a moment so the other racer is sure to find it taken.
#[test]
#[ignore = "the engine half of two_at_once_*; runs only as their child (SOAK_CHILD)"]
fn engine_child() {
    if !is_child() {
        return;
    }
    let dir = PathBuf::from(std::env::var("SOAK_DIR").unwrap());
    match Server::start(&dir) {
        Ok(server) => {
            println!("SOAK took=1");
            if std::env::var("SOAK_MODE").as_deref() == Ok("race") {
                std::thread::sleep(Duration::from_millis(800));
            } else {
                let mut rest = String::new();
                let _ = std::io::Read::read_to_string(&mut std::io::stdin(), &mut rest);
            }
            drop(server);
        }
        Err(e) => println!("SOAK refused={}", e.replace(' ', "_")),
    }
}

fn spawn(dir: &Path, mode: &str) -> std::process::Child {
    child(CHILD_TEST, &[("SOAK_DIR", dir.to_string_lossy().into()), ("SOAK_MODE", mode.into())])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap()
}

/// What a child said: Ok(()) when it took the folder, Err(words) when it was refused.
fn answer(out: &mut BufReader<std::process::ChildStdout>) -> Result<(), String> {
    let mut line = String::new();
    loop {
        line.clear();
        if out.read_line(&mut line).unwrap() == 0 {
            return Err("the child ended without saying".into());
        }
        if line.starts_with("SOAK took=") {
            return Ok(());
        }
        if let Some(words) = line.trim().strip_prefix("SOAK refused=") {
            return Err(words.replace('_', " "));
        }
    }
}

/// One round; returns what went wrong, if anything.
fn round(i: usize) -> Result<String, String> {
    let t = Temp::new("two-at-once");
    let dir = t.native();
    let started = Instant::now();

    let mut a = spawn(&dir, "hold");
    let mut a_out = BufReader::new(a.stdout.take().unwrap());
    answer(&mut a_out).map_err(|e| format!("round {i}: the first engine could not start: {e}"))?;

    // A second engine in this process, and one in another, both refused in Electron's words.
    match Server::start(&dir) {
        Ok(_) => return Err(format!("round {i}: a second engine (in-process) started beside the first")),
        Err(e) if e != REFUSED => return Err(format!("round {i}: the second engine was refused in other words: {e:?}")),
        Err(_) => {}
    }
    let mut b = spawn(&dir, "race");
    let b_said = answer(&mut BufReader::new(b.stdout.take().unwrap()));
    b.wait().unwrap();
    match b_said {
        Ok(()) => return Err(format!("round {i}: a second engine (another process) started beside the first")),
        Err(e) if e != REFUSED => return Err(format!("round {i}: the second process was refused in other words: {e:?}")),
        Err(_) => {}
    }

    // The first exits: quits on even rounds, is killed on odd ones.
    let how = if i % 2 == 0 {
        drop(a.stdin.take());
        "quit"
    } else {
        a.kill().unwrap();
        "killed"
    };
    a.wait().unwrap();
    let stale_socket = dir.join(SOCKET).exists();

    // Free again: this process takes it at once, and lets it go.
    let server = Server::start(&dir).map_err(|e| format!("round {i}: after the first engine {how}, the lock was not free: {e}"))?;
    if !server.socket().exists() {
        return Err(format!("round {i}: the engine that took the lock after a {how} engine has no socket"));
    }
    drop(server);

    // Two at once: exactly one wins.
    let mut racers = [spawn(&dir, "race"), spawn(&dir, "race")];
    let said: Vec<Result<(), String>> = racers.iter_mut().map(|r| answer(&mut BufReader::new(r.stdout.take().unwrap()))).collect();
    for r in &mut racers {
        r.wait().unwrap();
    }
    let won = said.iter().filter(|s| s.is_ok()).count();
    if won != 1 {
        return Err(format!("round {i}: two engines started at once, {won} took the folder: {said:?}"));
    }
    if let Some(Err(e)) = said.iter().find(|s| s.is_err())
        && e != REFUSED
    {
        return Err(format!("round {i}: the racer that lost was refused in other words: {e:?}"));
    }
    // And the lock file is the one lock: still there, still free.
    if !dir.join(LOCK).exists() {
        return Err(format!("round {i}: the lock file is gone"));
    }
    Ok(format!("round {i}: first engine {how} (socket left behind: {stale_socket}), {:.0} ms", since(started)))
}

fn rounds(n: usize) {
    let mut faults = Vec::new();
    let start = Instant::now();
    for i in 0..n {
        match round(i) {
            Ok(line) => eprintln!("{line}"),
            Err(f) => {
                eprintln!("FAULT {f}");
                faults.push(f);
            }
        }
    }
    eprintln!("two at once: {n} rounds in {:.1} s, {} faults", start.elapsed().as_secs_f64(), faults.len());
    assert!(faults.is_empty(), "{faults:#?}");
}

#[test]
fn two_at_once_short() {
    rounds(6);
}

#[test]
#[ignore = "long form (50 rounds): cargo test -p locust-core two_at_once_long -- --ignored --nocapture"]
fn two_at_once_long() {
    rounds(50);
}

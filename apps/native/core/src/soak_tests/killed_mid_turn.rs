//! ROW 3, KILLED MID-TURN: the engine process is killed (SIGKILL) at many points of a turn a fake agent is writing (20
//! in the long form). Each time the conversation's file then reads as whole or as a cut-off last line (the kinds
//! `history_check` names), never as unreadable, and the next start shows the turn as ended, not running: the list and
//! the conversation's reader say so, and a follow-up is taken and completes.
//!
//! The engine is this test binary again (`engine_child`), started on one turn of the soak fake's `long` scenario (200
//! deltas and 20 command rows over ~2 s). A first run is left to finish, to learn how many lines the turn's ledger ends
//! with; each kill then waits for the ledger to reach its own share of those lines, so the kills fall across the whole
//! turn: before the agent says anything, mid-answer, mid-command, at its last records.
use super::*;
use std::io::{BufRead, BufReader};
use std::process::Stdio;

const CHILD_TEST: &str = "soak_tests::killed_mid_turn::engine_child";

/// The child: one turn in the folder SOAK_DIR, its id printed as soon as it is started, then (when it is not killed)
/// its ledger's line count once it ends.
#[test]
#[ignore = "the engine half of killed_mid_turn_*; runs only as their child (SOAK_CHILD)"]
fn engine_child() {
    if !is_child() {
        return;
    }
    // Not a `Temp`: the parent owns the folder and reads it after this process is gone.
    let dir = PathBuf::from(std::env::var("SOAK_DIR").unwrap());
    let store = Store::new(dir.join("native"), dir.join("electron")).unwrap();
    let _g = native::hooks::install(store.clone(), fake("long"));
    let teammate = Teammate { folder: Some(dir.join("work").to_string_lossy().into()), ..soak_teammate() };
    let started = native::send(Send {
        teammate,
        conversation: None,
        prompt: "Soak: a turn to be killed".into(),
        model: Some("synthetic-model".into()),
        effort: None,
    })
    .unwrap();
    println!("SOAK started={}", started.conversation_id);
    finished(&started.conversation_id);
    let lines = fs::read_to_string(store.write_path(&started.conversation_id).unwrap()).unwrap().lines().count();
    println!("SOAK lines={lines}");
}

fn soak_teammate() -> Teammate {
    Teammate {
        id: "tm_soak".into(),
        name: "Soak".into(),
        role: "Code & Migrations".into(),
        runtime: Some("codex".into()),
        model: Some("synthetic-model".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    }
}

/// Lines in a file so far (0 when it is not there yet), counting a last line with no end.
fn lines_in(path: &Path) -> usize {
    fs::read(path).map(|b| b.split(|c| *c == b'\n').filter(|l| !l.is_empty()).count()).unwrap_or(0)
}

/// Starts the engine child in `t`, returns it and its turn's id once it has said it.
fn start_engine(t: &Temp, agent_pid: &Path) -> (std::process::Child, String, BufReader<std::process::ChildStdout>) {
    let mut c = child(CHILD_TEST, &[("SOAK_DIR", t.0.to_string_lossy().into()), ("SOAK_PID_FILE", agent_pid.to_string_lossy().into())]);
    let mut engine = c.stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    let mut out = BufReader::new(engine.stdout.take().unwrap());
    let mut line = String::new();
    let id = loop {
        line.clear();
        assert!(out.read_line(&mut line).unwrap() > 0, "the engine child ended before starting its turn");
        if let Some(id) = line.trim().strip_prefix("SOAK started=") {
            break id.to_owned();
        }
    };
    (engine, id, out)
}

/// The turn left to finish: how many lines its ledger ends with.
fn calibrate() -> usize {
    let t = Temp::new("killed-calibrate");
    let (mut engine, _, mut out) = start_engine(&t, &t.0.join("agent.pid"));
    let mut rest = String::new();
    std::io::Read::read_to_string(&mut out, &mut rest).unwrap();
    assert!(engine.wait().unwrap().success());
    let lines = rest.lines().find_map(|l| l.strip_prefix("SOAK lines=")).unwrap().parse().unwrap();
    eprintln!("killed mid-turn: a whole turn writes {lines} ledger lines");
    lines
}

/// Gone, or a zombie: a process that runs nothing any more (an orphan's zombie waits for init to reap it).
fn process_gone(pid: i32) -> bool {
    match fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(stat) => stat.rfind(')').is_some_and(|i| stat[i + 1..].trim_start().starts_with('Z')),
        // SAFETY: signal 0 only asks whether the process exists.
        Err(_) => unsafe { libc::kill(pid, 0) != 0 },
    }
}

/// The pids whose parent is `pid` (Linux; empty elsewhere).
fn children_of(pid: i32) -> Vec<i32> {
    let parent = pid.to_string();
    fs::read_dir("/proc")
        .map(|d| {
            d.flatten()
                .filter_map(|e| {
                    let stat = fs::read_to_string(e.path().join("stat")).ok()?;
                    let rest = &stat[stat.rfind(')')? + 1..];
                    (rest.split_whitespace().nth(1)? == parent).then(|| e.file_name().to_str()?.parse().ok())?
                })
                .collect()
        })
        .unwrap_or_default()
}

struct Kill {
    at_lines: usize,
    file_lines: usize,
    kind: String,
    agent_outlived_ms: Option<f64>,
    /// A follow-up sent at the next start: taken and completed, or why not.
    follow_up: Result<(), String>,
}

/// Kills the engine once its turn's ledger has `at` lines, then reads what it left, as the next start would.
fn kill_at(at: usize) -> Result<Kill, String> {
    let t = Temp::new("killed");
    let pid_file = t.0.join("agent.pid");
    let (mut engine, id, _out) = start_engine(&t, &pid_file);
    let ledger = t.store().write_path(&id).unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    while lines_in(&ledger) < at {
        if Instant::now() > deadline {
            let _ = engine.kill();
            return Err(format!("the ledger never reached {at} lines ({} so far)", lines_in(&ledger)));
        }
        std::thread::yield_now();
    }
    // The agents the engine is running, read just before the kill (the pid file is a fallback for a slow /proc).
    let mut agents = children_of(engine.id() as i32);
    if agents.is_empty()
        && let Some(pid) = fs::read_to_string(&pid_file).ok().and_then(|p| p.trim().parse::<i32>().ok())
    {
        agents.push(pid);
    }
    // SIGKILL: no handler, no shutdown, nothing written after this instant.
    engine.kill().unwrap();
    engine.wait().unwrap();
    let killed = Instant::now();

    // The agent the engine started must not outlive it for long: it loses its stdin and stdout with the engine.
    let agent_outlived_ms = (!agents.is_empty()).then(|| {
        while !agents.iter().all(|&p| process_gone(p)) && killed.elapsed() < Duration::from_secs(10) {
            std::thread::sleep(Duration::from_millis(5));
        }
        for &pid in agents.iter().filter(|&&p| !process_gone(p)) {
            // SAFETY: an agent this test's child started; nothing else has its pid while it is alive.
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
        since(killed)
    });

    // What the file reads as: history_check's kinds.
    let check = crate::history_check::check_history(&t.ledger_dir()).map_err(|e| e.to_string())?;
    let kind = if check.read_whole == 1 {
        "read whole".to_owned()
    } else {
        check.findings.keys().next().cloned().unwrap_or_else(|| "no file".into())
    };
    let file_lines = lines_in(&ledger);
    let allowed = kind == "read whole"
        || kind == "damaged: only the part before the fault was read (truncated-tail)"
        || kind.starts_with("read with zero turns");
    if !allowed {
        return Err(format!("killed at {at} lines: the file reads as {kind:?}"));
    }
    if agent_outlived_ms.is_some_and(|ms| ms >= 10_000.0) {
        return Err(format!("killed at {at} lines: the agent was still running 10 s after the engine died"));
    }

    // The next start: an engine on the same folder (this process; the killed one's pid is gone).
    let _g = native::hooks::install(t.store(), fake("quick"));
    let listed = native::list_recent(50, true).ok_or("nothing listed")?;
    let row = listed.summaries.iter().find(|s| s.mission_id == id).ok_or_else(|| format!("killed at {at} lines: the turn is not listed"))?;
    if row.running {
        return Err(format!("killed at {at} lines: the list shows the killed turn as running"));
    }
    let snap = native::session(t.electron().join("mission-ledger"), id.clone()).poll().map_err(|e| e.to_string())?;
    if snap.running {
        return Err(format!("killed at {at} lines: the conversation shows the killed turn as running"));
    }
    let follow_up = (|| {
        let next = native::send(job(&t, Some(&id), "Soak: after the crash"))
            .map_err(|e| format!("killed at {at} lines: a follow-up was refused: {e}"))?;
        finished(&next.conversation_id);
        let after = native::session(t.electron().join("mission-ledger"), id.clone()).poll().map_err(|e| e.to_string())?;
        if after.running || after.tip == id {
            return Err(format!("killed at {at} lines: the follow-up did not become the conversation's newest turn"));
        }
        Ok(())
    })();
    Ok(Kill { at_lines: at, file_lines, kind, agent_outlived_ms, follow_up })
}

/// Kills at `points` places across a turn. The row's own rule (the file reads whole or cut off, the next start shows
/// the turn ended) always holds the test; a refused follow-up does too when `follow_up` is set.
fn kills(points: usize, follow_up: bool) {
    let whole = calibrate();
    let mut faults = Vec::new();
    let mut refused = 0;
    let mut kinds: std::collections::BTreeMap<String, usize> = Default::default();
    for i in 0..points {
        // From the first line (the header) to the one before the last: all while the turn is being written.
        let at = 1 + i * (whole - 2) / (points - 1).max(1);
        match kill_at(at) {
            Ok(k) => {
                eprintln!(
                    "kill {i:2}: at {:3} lines, file has {:3}, reads as {:?}, agent gone {}, follow-up {}",
                    k.at_lines,
                    k.file_lines,
                    k.kind,
                    k.agent_outlived_ms.map_or("(not started)".into(), |ms| format!("{ms:.0} ms after")),
                    match &k.follow_up {
                        Ok(()) => "taken and completed".to_owned(),
                        Err(e) => format!("FAILED: {e}"),
                    }
                );
                *kinds.entry(k.kind).or_default() += 1;
                if let Err(e) = k.follow_up {
                    refused += 1;
                    if follow_up {
                        faults.push(e);
                    }
                }
            }
            Err(f) => {
                eprintln!("kill {i:2}: FAULT {f}");
                faults.push(f);
            }
        }
    }
    eprintln!("killed mid-turn: {points} kills, {kinds:?}, {refused} follow-ups refused, {} faults", faults.len());
    assert!(faults.is_empty(), "{faults:#?}");
}

#[test]
fn killed_mid_turn_short() {
    kills(4, false);
}

/// SOAK-NOTES row 3 (fixed): within 15 minutes of the crash, a follow-up typed in the conversation was refused as "still
/// running" while the list and the conversation showed the turn ended. `native::prior_of` read the turn as running from
/// its file's age alone (`ledger::ACTIVE_WINDOW`); it now asks `orphaned`, as the list and the reader do.
#[test]
fn killed_mid_turn_then_a_follow_up_is_taken() {
    kills(2, true);
}

#[test]
#[ignore = "long form (20 kills across a turn): cargo test -p locust-core killed_mid_turn_long -- --ignored --nocapture"]
fn killed_mid_turn_long() {
    kills(20, false);
}

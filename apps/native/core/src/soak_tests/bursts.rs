//! ROW 2, BURSTS: a fake agent prints 50,000 records as fast as it can while the engine's reader is held slow, once for
//! each runtime reader that has a queue: the shared process-runner bound (records.rs, 2,048, Claude Code here), Codex's
//! app-server (codex.rs, 4,096), ACP (acp.rs, 20,000) and OpenCode 1's server (opencode_serve.rs, 20,000). The run
//! either keeps every record or ends with the sentence Electron uses at Electron's bound. It never hangs and never drops
//! records silently.
//!
//! Every record carries its number (`<17>`). What the run kept is read back from its events: the distinct numbers seen.
//! "Every record" is all of 0..n. A run that stopped keeps a whole prefix 0..k (no hole before the stop) and says
//! Electron's sentence (packages/runtime-adapters: claude-events.ts, codex-app-server-run.ts, acp-run.ts,
//! opencode-serve-run.ts). "Never hangs": the run ends within `UNDER_LOAD` of the reader starting to read.
use super::*;
use crate::{
    approval::Mode,
    runtime::{Runtime, Turn, acp::Acp, claude::Claude, codex::Codex, opencode::OpenCode},
};

const N: usize = 50_000;
const ELECTRON_CLAUDE: &str = "Claude Code sent output faster than Locust could record it, so the run was stopped rather than leave a gap in its record. Sending it again usually works.";
const ELECTRON_CODEX: &str = "Codex sent output faster than Locust could record it, so the run was stopped rather than leave a gap in its record. Sending it again usually works.";
const ELECTRON_ACP: &str = "The agent sent output faster than Locust could record it, so the run was stopped rather than leave a gap in its record. Sending it again usually works.";
const ELECTRON_OPENCODE: &str = "OpenCode sent output faster than Locust could record it, so the run was stopped rather than leave a gap in its record. Sending it again usually works.";

fn fixture(name: &str) -> String {
    format!("{}/src/soak_tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Reader {
    /// Reads nothing for two seconds after the start, then as fast as it can: the engine busy writing a batch.
    Held,
    /// Reads one event a millisecond... at most: a sleep of 1 ms after every 100 events, from the start.
    Slow,
}

struct Outcome {
    end: String,
    message: String,
    /// Distinct record numbers kept, in the order first seen.
    seen: Vec<usize>,
    events: usize,
    took_ms: f64,
    /// Of that, the time spent inside the engine's `next_event` (the rest is this test reading the events).
    engine_ms: f64,
    /// The largest event payload, in bytes.
    biggest: usize,
}

/// The record numbers (`<17>`) in a text, in order.
fn numbers(text: &str, out: &mut Vec<usize>) {
    let b = text.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'<' {
            let mut j = i + 1;
            while j < b.len() && b[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && j < b.len() && b[j] == b'>' {
                out.push(text[i + 1..j].parse().unwrap());
                i = j;
            }
        }
        i += 1;
    }
}

fn turn(folder: &Path, id: &str) -> Turn {
    Turn {
        prompt: "Synthetic prompt".into(),
        folder: folder.to_path_buf(),
        model: None,
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: format!("soak_burst_{id}"),
        run_id: format!("run_soak_burst_{id}"),
    }
}

fn drive(rt: &mut dyn Runtime, t: &Turn, reader: Reader) -> Outcome {
    rt.start(t).unwrap();
    if reader == Reader::Held {
        std::thread::sleep(Duration::from_secs(2));
    }
    let start = Instant::now();
    let mut raw = Vec::new();
    let mut events = 0;
    let mut engine = Duration::ZERO;
    let mut biggest = 0;
    loop {
        assert!(
            start.elapsed() < crate::load_waits::UNDER_LOAD,
            "the run hung: no ending {:?} after the reader began ({events} events read)",
            crate::load_waits::UNDER_LOAD
        );
        let asked = Instant::now();
        let next = rt.next_event().unwrap();
        engine += asked.elapsed();
        let Some(e) = next else { continue };
        events += 1;
        if reader == Reader::Slow && events % 100 == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
        let text = e.payload.to_string();
        biggest = biggest.max(text.len());
        numbers(&text, &mut raw);
        if matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled") {
            let mut seen = Vec::new();
            let mut had = std::collections::HashSet::new();
            for n in raw {
                if had.insert(n) {
                    seen.push(n);
                }
            }
            return Outcome {
                end: e.kind.clone(),
                message: e.payload["message"].as_str().unwrap_or_default().to_owned(),
                seen,
                events,
                took_ms: since(start),
                engine_ms: engine.as_secs_f64() * 1000.0,
                biggest,
            };
        }
    }
}

/// Kept every record, or stopped with Electron's sentence and a whole prefix: the line for the notes, or what is wrong.
fn judge(who: &str, reader: Reader, n: usize, bound: usize, electron: &str, o: &Outcome) -> Result<String, String> {
    let line = format!(
        "{who} {reader:?} n={n} bound={bound}: {} after {:.0} ms reading ({:.0} ms in the engine), {} events (largest {} B), {} records kept{}",
        o.end,
        o.took_ms,
        o.engine_ms,
        o.events,
        o.biggest,
        o.seen.len(),
        if o.message.is_empty() { String::new() } else { format!(", said: {:?}", o.message) }
    );
    eprintln!("{line}");
    // Kept in order, no hole: the k records kept are exactly 0..k.
    if !o.seen.iter().enumerate().all(|(i, &v)| i == v) {
        return Err(format!("{who}: the records kept are not a whole prefix (a record dropped silently): {line}"));
    }
    if o.end == "run.completed" {
        if o.seen.len() != n {
            return Err(format!("{who}: the run completed without every record: {line}"));
        }
        return Ok(line);
    }
    if o.end != "run.failed" || o.seen.len() >= n {
        return Err(format!("{who}: neither every record nor a stop: {line}"));
    }
    // Held, nothing is read until the queue has overflowed, so no more than the bound can have been kept. A slow reader
    // takes records out while they come, so it can keep more than the bound before the queue fills.
    if reader == Reader::Held && o.seen.len() > bound {
        return Err(format!("{who}: more records kept than Electron's bound before stopping: {line}"));
    }
    if o.message != electron {
        return Err(format!("{who}: the run stopped without Electron's sentence ({electron:?}): {line}"));
    }
    Ok(line)
}

// ---- the four readers ------------------------------------------------------------------------------------------

fn claude(n: usize, reader: Reader) -> Result<String, String> {
    let t = Temp::new("burst-claude");
    let mut c = Command::new("python3");
    c.arg(fixture("soak-claude-burst.py")).env("SOAK_RECORDS", n.to_string()).current_dir(t.0.join("work"));
    let mut rt = Claude::spawn(&mut c).unwrap();
    let o = drive(&mut rt, &turn(&t.0.join("work"), "claude"), reader);
    judge("claude (records.rs)", reader, n, crate::runtime::records::MAX_QUEUED_RECORDS, ELECTRON_CLAUDE, &o)
}

fn codex(n: usize, reader: Reader) -> Result<String, String> {
    let t = Temp::new("burst-codex");
    let mut c = Command::new("python3");
    c.arg(FAKE).arg("burst").env("SOAK_RECORDS", n.to_string()).current_dir(t.0.join("work"));
    let mut rt = Codex::spawn(&mut c).unwrap();
    let o = drive(&mut rt, &turn(&t.0.join("work"), "codex"), reader);
    judge("codex (codex.rs)", reader, n, crate::runtime::codex::MAX_QUEUED_RECORDS, ELECTRON_CODEX, &o)
}

fn acp(n: usize, reader: Reader) -> Result<String, String> {
    let t = Temp::new("burst-acp");
    let mut c = Command::new("python3");
    c.arg(fixture("soak-acp-burst.py")).env("SOAK_RECORDS", n.to_string()).current_dir(t.0.join("work"));
    let mut rt = Acp::spawn(&mut c).unwrap();
    let o = drive(&mut rt, &turn(&t.0.join("work"), "acp"), reader);
    judge("acp (acp.rs)", reader, n, 20_000, ELECTRON_ACP, &o)
}

fn opencode(n: usize, reader: Reader) -> Result<String, String> {
    let t = Temp::new("burst-opencode");
    let exe = t.0.join("opencode");
    fs::copy(fixture("soak-opencode-serve-burst.py"), &exe).unwrap();
    fs::set_permissions(&exe, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let store = t.store();
    let folder = t.0.join("work");
    let mut c = crate::runtime::opencode::command_for_version(exe, "1.18.27", &folder, Mode::Ask, &store, false).unwrap();
    c.env("SOAK_RECORDS", n.to_string());
    let mut rt = OpenCode::spawn(&mut c).unwrap();
    let o = drive(&mut rt, &turn(&folder, "opencode"), reader);
    judge(
        "opencode serve (opencode_serve.rs)",
        reader,
        n,
        crate::runtime::opencode_serve::MAX_QUEUED_EVENTS,
        ELECTRON_OPENCODE,
        &o,
    )
}

// ---- short forms: 50,000 records against a held reader, once each ----------------------------------------------

#[test]
fn burst_claude_records_short() {
    claude(N, Reader::Held).unwrap();
}

/// The same burst against a slow reader, which takes records out while they still arrive: after the queue overflows,
/// the reader thread must stop taking records, or what reaches the queue after the overflow follows a gap (held, the
/// agent has finished before anything is read, so a reader that carries on past the overflow leaves no gap to see).
#[test]
fn burst_claude_records_slow_short() {
    claude(N, Reader::Slow).unwrap();
}

#[test]
fn burst_codex_short() {
    codex(N, Reader::Held).unwrap();
}

#[test]
fn burst_acp_short() {
    acp(N, Reader::Held).unwrap();
}

#[test]
fn burst_opencode_serve_short() {
    opencode(N, Reader::Held).unwrap();
}

// ---- long form: every reader, held and slow, past the bound and under it, three times -----------------------------

#[test]
#[ignore = "long form (every reader x held/slow x past/under its bound x 3): cargo test -p locust-core bursts_long -- --ignored --nocapture"]
fn bursts_long() {
    let mut lines = Vec::new();
    let mut faults = Vec::new();
    let readers: [(fn(usize, Reader) -> Result<String, String>, usize); 4] = [
        (claude, crate::runtime::records::MAX_QUEUED_RECORDS),
        (codex, crate::runtime::codex::MAX_QUEUED_RECORDS),
        (acp, 20_000),
        (opencode, crate::runtime::opencode_serve::MAX_QUEUED_EVENTS),
    ];
    for round in 0..3 {
        for (run, bound) in readers {
            for reader in [Reader::Held, Reader::Slow] {
                for n in [N, bound / 2] {
                    match run(n, reader) {
                        Ok(l) => lines.push(format!("round {round}: {l}")),
                        Err(f) => faults.push(format!("round {round}: {f}")),
                    }
                }
            }
        }
    }
    for l in &lines {
        eprintln!("ok    {l}");
    }
    for f in &faults {
        eprintln!("FAULT {f}");
    }
    assert!(faults.is_empty(), "{} of {} runs found a fault", faults.len(), faults.len() + lines.len());
}

/// How the time to read a burst under the bound grows with its size, for each reader: the engine's own milliseconds
/// in `next_event`, held reader. A reader whose time grows with the square of the records takes most of a minute on a
/// long answer just under its bound. The rule: a record read at 95% of the bound costs no more than three times what
/// one costs at a sixteenth of it.
#[test]
#[ignore = "FAULT: ACP and OpenCode 1 read a burst in time that grows with its square (19,000 chunks: ~47 s in the engine); SOAK-NOTES row 2"]
fn bursts_scale_long() {
    let readers: [(&str, fn(usize, Reader) -> Result<String, String>, usize); 4] = [
        ("claude", claude, crate::runtime::records::MAX_QUEUED_RECORDS),
        ("codex", codex, crate::runtime::codex::MAX_QUEUED_RECORDS),
        ("acp", acp, 20_000),
        ("opencode", opencode, crate::runtime::opencode_serve::MAX_QUEUED_EVENTS),
    ];
    let mut faults = Vec::new();
    for (who, run, bound) in readers {
        let mut per_record = Vec::new();
        for n in [bound / 16, bound / 8, bound / 4, bound / 2, bound - bound / 20] {
            match run(n, Reader::Held) {
                Ok(line) => {
                    let ms: f64 = line.split(" ms in the engine").next().unwrap().rsplit('(').next().unwrap().parse().unwrap();
                    per_record.push((n, ms / n as f64));
                }
                Err(f) => faults.push(f),
            }
        }
        if let (Some(first), Some(last)) = (per_record.first(), per_record.last()) {
            let grew = last.1 / first.1;
            eprintln!("{who}: {:.3} ms a record at {} records, {:.3} at {} ({grew:.1}x)", first.1, first.0, last.1, last.0);
            if grew > 3.0 {
                faults.push(format!(
                    "{who}: reading time grows with the burst: {:.3} ms a record at {} records, {:.3} at {} ({grew:.1}x)",
                    first.1, first.0, last.1, last.0
                ));
            }
        }
    }
    assert!(faults.is_empty(), "{faults:#?}");
}

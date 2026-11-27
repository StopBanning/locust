//! ROW 1, MANY TURNS: hundreds of turns in one engine with a fake agent. After the last turn, compared with after an
//! early one (turn 50 of 500 in the long form): resident memory, open file descriptors, threads, and child processes
//! still alive. None may keep growing.
//!
//! "Keeps growing", in numbers (the same rule in both forms):
//! * descriptors and threads: more than 3 above the early reading. One leaked per turn would be +450 in the long form;
//!   3 leaves room for a lazily started helper (a cache's thread, a log file) that opens once and stays.
//! * children: any still running after a turn has ended, or more unreaped (zombie) than at the early reading. A turn's
//!   agent is the only child the engine starts here, and it must be gone once the turn is.
//! * memory: more than 16 KiB a turn on average between the early reading and the last, and more than 4 MiB in all.
//!   A turn's whole ledger here is about 3 KiB on disk, so 16 KiB a turn is a turn's records kept several times over;
//!   the 4 MiB floor is the allocator's own drift, which is not a leak and does not scale with turns.
//!
//! The engine runs in a child process of its own (this binary again, `engine_child`), so the numbers are the engine's
//! and not those of whatever other tests share the suite's process.
use super::*;

const CHILD_TEST: &str = "soak_tests::many_turns::engine_child";
/// A conversation is a start and this many follow-ups, then a new one: a day is many conversations, most short.
const TURNS_PER_CONVERSATION: usize = 5;

/// The child: SOAK_TURNS turns in the engine, a reading after every tenth, printed as `SOAK turn=.. rss_kib=..` lines.
#[test]
#[ignore = "the engine half of many_turns_*; runs only as their child (SOAK_CHILD)"]
fn engine_child() {
    if !is_child() {
        return;
    }
    let turns: usize = std::env::var("SOAK_TURNS").unwrap().parse().unwrap();
    let t = Temp::new("many-turns");
    let _g = native::hooks::install(t.store(), fake("quick"));
    let before = reading();
    println!(
        "SOAK turn=0 rss_kib={} fds={} threads={} children={} zombies={} ms=0",
        before.rss_kib, before.fds, before.threads, before.children, before.zombies
    );
    let mut conversation: Option<String> = None;
    let start = Instant::now();
    for turn in 1..=turns {
        if (turn - 1) % TURNS_PER_CONVERSATION == 0 {
            conversation = None;
        }
        let started = native::send(job(&t, conversation.as_deref(), &format!("Soak turn {turn}"))).unwrap();
        finished(&started.conversation_id);
        conversation = Some(started.conversation_id);
        // The agent has been told the turn is over; give it the moment it takes to exit and be reaped, no more.
        let settle = Instant::now() + Duration::from_secs(3);
        while children().0 > 0 && Instant::now() < settle {
            std::thread::sleep(Duration::from_millis(5));
        }
        if turn % 10 == 0 {
            let r = reading();
            println!(
                "SOAK turn={turn} rss_kib={} fds={} threads={} children={} zombies={} ms={:.0}",
                r.rss_kib,
                r.fds,
                r.threads,
                r.children,
                r.zombies,
                since(start)
            );
        }
    }
    // Every turn ended as it should: the last ledger of each conversation reads as completed.
    let mut files = 0;
    let mut completed = 0;
    for e in fs::read_dir(t.ledger_dir()).unwrap().flatten() {
        files += 1;
        completed += usize::from(fs::read_to_string(e.path()).unwrap().contains("\"type\":\"run.completed\""));
    }
    println!("SOAK ledgers={files} completed={completed}");
}

struct Run {
    rows: Vec<Vec<(String, String)>>,
    ledgers: usize,
    completed: usize,
}
impl Run {
    fn at(&self, turn: usize) -> Reading {
        let row = self
            .rows
            .iter()
            .find(|r| r.iter().any(|(k, v)| k == "turn" && *v == turn.to_string()))
            .unwrap_or_else(|| panic!("no reading at turn {turn}"));
        Reading {
            rss_kib: field(row, "rss_kib"),
            fds: field(row, "fds"),
            threads: field(row, "threads"),
            children: field(row, "children"),
            zombies: field(row, "zombies"),
        }
    }
    fn ms(&self, turn: usize) -> f64 {
        let row = self.rows.iter().find(|r| r.iter().any(|(k, v)| k == "turn" && *v == turn.to_string())).unwrap();
        field(row, "ms")
    }
}

fn soak(turns: usize, early: usize) -> Run {
    let out = run_child(CHILD_TEST, &[("SOAK_TURNS", turns.to_string())]);
    let lines = soak_lines(&out);
    let end = lines.iter().find(|l| l.iter().any(|(k, _)| k == "ledgers")).expect("the child's last line").clone();
    let run = Run {
        rows: lines.into_iter().filter(|l| l.iter().any(|(k, _)| k == "turn")).collect(),
        ledgers: field(&end, "ledgers"),
        completed: field(&end, "completed"),
    };
    // The table, for the notes.
    for row in &run.rows {
        eprintln!("{}", row.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" "));
    }
    let (a, z) = (run.at(early), run.at(turns));
    eprintln!(
        "many turns: {turns} turns in {:.1} s ({:.0} ms a turn after turn {early}); turn {early} -> {turns}: rss {} -> {} KiB ({:+} KiB, {:+.1} KiB a turn), fds {} -> {}, threads {} -> {}, children {} -> {}, zombies {} -> {}",
        run.ms(turns) / 1000.0,
        (run.ms(turns) - run.ms(early)) / (turns - early) as f64,
        a.rss_kib,
        z.rss_kib,
        z.rss_kib as i64 - a.rss_kib as i64,
        (z.rss_kib as f64 - a.rss_kib as f64) / (turns - early) as f64,
        a.fds,
        z.fds,
        a.threads,
        z.threads,
        a.children,
        z.children,
        a.zombies,
        z.zombies
    );
    run
}

fn assert_nothing_keeps_growing(run: &Run, turns: usize, early: usize) {
    let (a, z) = (run.at(early), run.at(turns));
    assert_eq!(run.ledgers, turns, "every turn wrote its own ledger");
    assert_eq!(run.completed, turns, "every turn's ledger ends completed");
    assert!(z.fds <= a.fds + 3, "open descriptors keep growing: {} after turn {early}, {} after turn {turns}", a.fds, z.fds);
    assert!(z.threads <= a.threads + 3, "threads keep growing: {} after turn {early}, {} after turn {turns}", a.threads, z.threads);
    assert_eq!(z.children, 0, "a turn's agent is still running after its turn ended ({} children)", z.children);
    assert!(z.zombies <= a.zombies, "unreaped agents keep growing: {} after turn {early}, {} after turn {turns}", a.zombies, z.zombies);
    let grew = z.rss_kib.saturating_sub(a.rss_kib);
    let per_turn = grew as f64 / (turns - early) as f64;
    assert!(
        grew <= 4 * 1024 || per_turn <= 16.0,
        "resident memory keeps growing: {} KiB after turn {early}, {} KiB after turn {turns} ({per_turn:.1} KiB a turn)",
        a.rss_kib,
        z.rss_kib
    );
}

#[test]
fn many_turns_short() {
    let run = soak(60, 10);
    assert_nothing_keeps_growing(&run, 60, 10);
}

#[test]
#[ignore = "long form (500 turns, a few minutes): cargo test -p locust-core many_turns_long -- --ignored --nocapture"]
fn many_turns_long() {
    let run = soak(500, 50);
    assert_nothing_keeps_growing(&run, 500, 50);
}

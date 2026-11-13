//! Electron's tests: runtime-adapters connectors.test.ts (the parser), main/connector-reader.test.ts,
//! main/a-connector-check-says-how-it-went.test.ts and renderer a-connector-says-what-to-do.test.ts.
use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicI64, AtomicUsize, Ordering},
};

const REAL_OUTPUT: &str = "claude.ai Anthropic Economic Index: https://econ-index.mcp.claude.com/mcp - ✔ Connected
claude.ai Google Calendar: https://calendarmcp.googleapis.com/mcp/v1 - ✔ Connected
claude.ai Notion: https://mcp.notion.com/mcp - ! Needs authentication
claude.ai Zapier: https://mcp.zapier.com/api/v1/connect - ✔ Connected
claude.ai Clay: https://api.clay.com/v3/mcp - ✔ Connected
claude.ai Canva: https://mcp.canva.com/mcp - ! Needs authentication
claude.ai Robinhood: https://agent.robinhood.com/mcp/trading - ✔ Connected
claude.ai Gmail: https://gmailmcp.googleapis.com/mcp/v1 - ✔ Connected
claude.ai Figma: https://mcp.figma.com/mcp - ✔ Connected
claude.ai Google Drive: https://drivemcp.googleapis.com/mcp/v1 - ✔ Connected";

const OUTPUT: &str = "Checking MCP server health…

claude.ai Robinhood: https://agent.robinhood.com/mcp/trading - ✔ Connected
claude.ai Notion: https://mcp.notion.com/mcp - ! Needs authentication";

fn statuses(c: &[Connector]) -> Vec<String> {
    c.iter().map(|c| format!("{}:{:?}", c.name, c.status)).collect()
}

// ---- the parser ----

#[test]
fn are_read_off_claude_mcp_list_names_and_health_both() {
    let found = parse_claude_connectors(REAL_OUTPUT);
    assert_eq!(found.len(), 10);
    assert_eq!((found[6].name.as_str(), found[6].location.as_str(), found[6].status), ("claude.ai Robinhood", "https://agent.robinhood.com/mcp/trading", Status::Connected));
    assert_eq!(found.iter().filter(|c| c.status == Status::NeedsAuth).map(|c| c.name.as_str()).collect::<Vec<_>>(), ["claude.ai Notion", "claude.ai Canva"]);
}

#[test]
fn are_read_into_every_class_the_health_check_can_say() {
    let found = parse_claude_connectors(
        "alpha: https://alpha.example/mcp - ✔ Connected\nbeta: https://beta.example/mcp - ! Needs authentication\ngamma: node gamma.js - ✗ Failed to connect\ndelta: https://delta.example/mcp - ✗ Disconnected\nepsilon: https://epsilon.example/mcp - ⏳ Pending approval",
    );
    assert_eq!(statuses(&found), ["alpha:Connected", "beta:NeedsAuth", "gamma:Failed", "delta:Failed", "epsilon:Unreadable"]);
    // Words it does not know are kept, as printed, never guessed into failed or connected.
    assert_eq!(found[4].said.as_deref(), Some("⏳ Pending approval"));
    assert_eq!(found[0].said, None);
}

#[test]
fn survive_the_header_blanks_and_anything_else_the_cli_prints() {
    assert!(parse_claude_connectors("").is_empty());
    assert!(parse_claude_connectors("No MCP servers configured. Use `claude mcp add`").is_empty());
    assert!(parse_claude_connectors("Checking MCP server health…").is_empty());
    let local = parse_claude_connectors("robinhood-trading: node C:\\srv\\rh.js --port 1 - ✔ Connected");
    assert_eq!(local.len(), 1);
    assert_eq!((local[0].name.as_str(), local[0].location.as_str(), local[0].status), ("robinhood-trading", "node C:\\srv\\rh.js --port 1", Status::Connected));
}

// ---- the reader ----

fn reader(answers: Arc<Mutex<Vec<Option<Read>>>>, reads: Arc<AtomicUsize>, clock: Arc<AtomicI64>, ttl_ms: i64) -> Reader {
    let mut r = Reader::new(
        Box::new(move |_| {
            reads.fetch_add(1, Ordering::SeqCst);
            let mut a = answers.lock().unwrap();
            if a.len() > 1 { a.remove(0) } else { a[0].clone() }
        }),
        Box::new(move || clock.load(Ordering::SeqCst)),
    );
    r.ttl_ms = ttl_ms;
    r
}

fn text(s: &str) -> Option<Read> {
    Some(Read::Text(s.into()))
}

#[test]
fn are_empty_until_the_first_reading_rather_than_blocking_for_one() {
    let r = reader(Arc::new(Mutex::new(vec![text(OUTPUT)])), Arc::default(), Arc::default(), TTL_MS);
    assert!(r.names().is_empty() && r.current().is_empty());
}

#[test]
fn are_there_once_a_reading_lands() {
    let r = reader(Arc::new(Mutex::new(vec![text(OUTPUT)])), Arc::default(), Arc::default(), TTL_MS);
    r.refresh();
    assert_eq!(r.names(), ["claude.ai Robinhood"]);
    // Present in the full reading, absent from the rules: a server the person has not finished signing into.
    assert_eq!(r.current().iter().map(|c| c.status).collect::<Vec<_>>(), [Status::Connected, Status::NeedsAuth]);
}

#[test]
fn are_read_once_and_held_not_re_read_on_every_start() {
    let (reads, clock) = (Arc::new(AtomicUsize::new(0)), Arc::new(AtomicI64::new(1_000)));
    let r = reader(Arc::new(Mutex::new(vec![text(OUTPUT)])), reads.clone(), clock.clone(), 60_000);
    r.refresh();
    r.refresh();
    r.refresh();
    assert_eq!(reads.load(Ordering::SeqCst), 1);
    clock.fetch_add(60_001, Ordering::SeqCst);
    r.refresh();
    assert_eq!(reads.load(Ordering::SeqCst), 2);
}

#[test]
fn keep_the_last_good_reading_when_a_read_fails_or_cannot_run() {
    let clock = Arc::new(AtomicI64::new(1_000));
    let answers = Arc::new(Mutex::new(vec![text(OUTPUT), None, text("garbage with no connectors")]));
    let r = reader(answers, Arc::default(), clock.clone(), 10);
    r.refresh();
    assert_eq!(r.names(), ["claude.ai Robinhood"]);
    clock.fetch_add(1_000, Ordering::SeqCst);
    r.refresh();
    assert_eq!(r.names(), ["claude.ai Robinhood"], "a read that could not run emptied the list");
}

#[test]
fn take_one_reading_at_a_time_however_many_runs_start_at_once() {
    let reads = Arc::new(AtomicUsize::new(0));
    let counted = reads.clone();
    let r = Arc::new(Reader::new(
        Box::new(move |_| {
            counted.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(50));
            text(OUTPUT)
        }),
        Box::new(|| 1_000),
    ));
    let all: Vec<_> = (0..3).map(|_| { let r = r.clone(); std::thread::spawn(move || r.refresh()) }).collect();
    for t in all {
        t.join().unwrap();
    }
    assert_eq!(reads.load(Ordering::SeqCst), 1);
}

#[test]
fn read_again_after_a_connector_is_added_even_when_a_reading_was_already_under_way() {
    let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
    let gate = Arc::new(Mutex::new(0usize));
    let gate_in = gate.clone();
    let release = Arc::new(std::sync::Barrier::new(2));
    let release_in = release.clone();
    let r = Arc::new(Reader::new(
        Box::new(move |_| {
            let n = { let mut g = gate_in.lock().unwrap(); *g += 1; *g };
            if n == 1 {
                started_tx.send(()).unwrap();
                release_in.wait();
                return text(OUTPUT);
            }
            text(&format!("{OUTPUT}\nrea: node server.mjs - ✔ Connected"))
        }),
        Box::new(|| 1_000),
    ));
    let before = { let r = r.clone(); std::thread::spawn(move || r.refresh()) };
    started_rx.recv().unwrap();
    // Add a connector added `rea` while that reading ran.
    r.forget();
    release.wait();
    before.join().unwrap();
    // The older answer is not taken as the new list...
    assert!(r.current().is_empty());
    r.refresh();
    assert_eq!(r.names(), ["claude.ai Robinhood", "rea"]);
    // ...and the new one is held as usual.
    r.refresh();
    assert_eq!(*gate.lock().unwrap(), 2);
}

#[test]
fn keeps_how_long_the_check_took_and_when_each_was_last_seen_connected() {
    let clock = Arc::new(AtomicI64::new(1_000_000));
    let ticking = clock.clone();
    let r = Reader::new(
        Box::new(move |_| {
            ticking.fetch_add(1_500, Ordering::SeqCst);
            text("alpha: https://alpha.example/mcp - ✔ Connected\nbeta: https://beta.example/mcp - ! Needs authentication")
        }),
        Box::new({ let c = clock.clone(); move || c.load(Ordering::SeqCst) }),
    );
    r.refresh();
    let all = r.current();
    assert_eq!((all[0].status, all[0].checked_in_ms, all[0].last_connected_ms), (Status::Connected, Some(1_500), Some(1_001_500)));
    assert_eq!((all[1].status, all[1].checked_in_ms, all[1].last_connected_ms), (Status::NeedsAuth, Some(1_500), None));
}

#[test]
fn a_check_that_timed_out_says_so_for_every_connector_and_the_rules_keep_the_last_good_reading() {
    let clock = Arc::new(AtomicI64::new(1_000_000));
    let good = text("alpha: https://alpha.example/mcp - ✔ Connected\nbeta: https://beta.example/mcp - ! Needs authentication");
    let answers = Arc::new(Mutex::new(vec![good.clone(), Some(Read::TimedOut), good]));
    let r = reader(answers, Arc::default(), clock.clone(), 10);
    r.refresh();
    clock.fetch_add(60_000, Ordering::SeqCst);
    r.refresh();
    assert_eq!(statuses(&r.current()), ["alpha:TimedOut", "beta:TimedOut"]);
    // When it last worked survives the timeout: it is the person's best clue.
    assert_eq!(r.current()[0].last_connected_ms, Some(1_000_000));
    assert_eq!(r.names(), ["alpha"]);
    clock.fetch_add(60_000, Ordering::SeqCst);
    r.refresh();
    assert_eq!(r.current().iter().map(|c| c.status).collect::<Vec<_>>(), [Status::Connected, Status::NeedsAuth]);
}

// ---- the words ----

fn shown() -> Vec<Connector> {
    let c = |name: &str, location: &str, status, checked: Option<i64>, last: Option<i64>, said: Option<&str>| Connector {
        name: name.into(), location: location.into(), status, said: said.map(str::to_owned), checked_in_ms: checked, last_connected_ms: last,
    };
    // now = 2026-10-03T12:00:00Z
    let now = 1_791_028_800_000_i64;
    vec![
        c("alpha", "https://alpha.example/mcp/v1?token=SECRET", Status::Connected, Some(2_340), Some(now - 5 * 60_000), None),
        c("beta", "https://beta.example/mcp", Status::NeedsAuth, Some(2_340), None, None),
        c("gamma", "node C:\\srv\\gamma.js --api-key SECRET2 API_TOKEN=SECRET3", Status::Failed, Some(2_340), None, None),
        c("delta", "https://delta.example/mcp", Status::TimedOut, None, Some(now - 3 * 3_600_000), None),
        c("epsilon", "https://epsilon.example/mcp", Status::Unreadable, None, None, Some("⏳ Pending approval")),
    ]
}
const NOW: i64 = 1_791_028_800_000;

#[test]
fn says_each_state_in_words_with_what_to_do() {
    let all = shown();
    assert_eq!(all.iter().map(|c| state_words(c).0).collect::<Vec<_>>(), ["Connected", "Needs sign-in", "Not working", "Did not answer", "Unknown"]);
    assert_eq!(state_words(&all[1]).1.as_deref(), Some("Run /mcp in Claude Code and sign in to beta."));
    assert_eq!(state_words(&all[2]).1.as_deref(), Some("Claude Code could not start or reach it."));
    assert!(state_words(&all[3]).1.unwrap().contains("did not answer in 20 seconds"));
    assert_eq!(state_words(&all[4]).1.as_deref(), Some("Claude Code said: \u{201c}⏳ Pending approval\u{201d}"));
}

#[test]
fn says_when_it_last_worked() {
    let all = shown();
    assert_eq!(last_worked(&all[0], NOW), "Last connected 5 min ago.");
    assert_eq!(last_worked(&all[3], NOW), "Last connected 3 h ago.");
    assert_eq!(last_worked(&all[1], NOW), "Not seen connected since Locust started.");
}

#[test]
fn says_where_a_connector_lives_only_as_a_host_or_as_a_program_on_this_computer() {
    assert_eq!(where_it_lives("https://alpha.example/mcp/v1?token=SECRET"), "alpha.example");
    assert_eq!(where_it_lives("node C:\\srv\\gamma.js --api-key SECRET2"), "a program on this computer");
}

#[test]
fn the_report_has_names_states_timings_and_hosts_and_nothing_else() {
    let all = shown();
    let text = report(&all, "2026-10-03T12:00:00.000Z", NOW);
    for c in &all {
        assert!(text.contains(&c.name), "{text}");
    }
    assert!(text.contains("Last check took 2.3 s.") && text.contains("alpha.example"), "{text}");
    for secret in ["SECRET", "token=", "/mcp/v1", "node ", "gamma.js", "--api-key", "API_TOKEN", "C:\\srv"] {
        assert!(!text.contains(secret), "{secret} in {text}");
    }
}

#[test]
fn a_connected_row_checked_a_moment_ago_has_nothing_to_add() {
    let mut fresh = shown()[0].clone();
    fresh.last_connected_ms = Some(NOW - 10_000);
    assert_eq!(detail(&fresh, NOW), None);
    assert_eq!(detail(&shown()[1], NOW).as_deref(), Some("Run /mcp in Claude Code and sign in to beta. Not seen connected since Locust started."));
}

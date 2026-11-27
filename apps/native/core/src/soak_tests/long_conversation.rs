//! ROW 5, A LONG CONVERSATION: hundreds of follow-ups in one conversation (300 in the long form). Listing recent
//! conversations and reading this one take no worse than three times as long for the last third of the turns as for the
//! first third (turns 200-300 against 0-100).
//!
//! After every turn the engine (this binary again, `engine_child`, so other tests do not share its CPU and caches) times
//! three things: `list_recent(50)` as the window's list asks it; opening the conversation, a fresh reader's first
//! `poll` (every turn's ledger, followed down the chain); and the window's own reader, kept open across turns, polling
//! once more after the turn, and once more again with nothing new. The medians of each third are compared, as medians are steady against a stray slow sample.
use super::*;

const CHILD_TEST: &str = "soak_tests::long_conversation::engine_child";

#[test]
#[ignore = "the engine half of long_conversation_*; runs only as their child (SOAK_CHILD)"]
fn engine_child() {
    if !is_child() {
        return;
    }
    let turns: usize = std::env::var("SOAK_TURNS").unwrap().parse().unwrap();
    let t = Temp::new("long-conversation");
    let _g = native::hooks::install(t.store(), fake("quick"));
    let first = native::send(job(&t, None, "Soak turn 0")).unwrap();
    let id = first.conversation_id.clone();
    finished(&id);
    let mut window = native::session(t.electron().join("mission-ledger"), id.clone());
    window.poll().unwrap();
    for turn in 1..=turns {
        let sent = Instant::now();
        let started = native::send(job(&t, Some(&id), &format!("Soak turn {turn}"))).unwrap();
        assert_eq!(started.conversation_id, id);
        finished(&id);
        let turn_ms = since(sent);

        let a = Instant::now();
        let listed = native::list_recent(50, true).unwrap();
        let list_ms = since(a);
        assert!(listed.summaries.iter().any(|s| s.mission_id == id || s.parent.is_some()), "the conversation is listed");

        let a = Instant::now();
        let snap = native::session(t.electron().join("mission-ledger"), id.clone()).poll().unwrap();
        let open_ms = since(a);
        let users = snap.items.iter().filter(|i| matches!(i, crate::ledger::Item::User(_))).count();
        assert_eq!(users, turn + 1, "every prompt of the conversation is read back");
        assert!(!snap.running && !snap.failed);

        let a = Instant::now();
        window.poll().unwrap();
        let tail_ms = since(a);
        // And once more with nothing new: what each of the window's polls costs while the conversation sits open.
        let a = Instant::now();
        window.poll().unwrap();
        let idle_ms = since(a);
        println!(
            "SOAK turn={turn} turn_ms={turn_ms:.2} list_ms={list_ms:.2} open_ms={open_ms:.2} tail_ms={tail_ms:.2} idle_ms={idle_ms:.2}"
        );
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// Runs `turns` follow-ups; returns, for each measure, (median of the first third, median of the last third).
fn soak(turns: usize) -> Vec<(&'static str, f64, f64, f64)> {
    let out = run_child(CHILD_TEST, &[("SOAK_TURNS", turns.to_string())]);
    let rows = soak_lines(&out);
    assert_eq!(rows.len(), turns);
    let third = turns / 3;
    let mut result = Vec::new();
    for key in ["list_ms", "open_ms", "tail_ms", "idle_ms", "turn_ms"] {
        let band = |from: usize, to: usize| median(rows[from..to].iter().map(|r| field::<f64>(r, key)).collect());
        let (first, middle, last) = (band(0, third), band(third, 2 * third), band(2 * third, turns));
        eprintln!(
            "long conversation, {key}: median {first:.2} ms for turns 1-{third}, {middle:.2} for {}-{}, {last:.2} for {}-{turns} ({:.2}x)",
            third + 1,
            2 * third,
            2 * third + 1,
            last / first
        );
        result.push((key, first, middle, last));
    }
    result
}

/// The measures named, each no more than three times as long in the last third as in the first.
fn assert_no_worse_than_three_times(result: &[(&str, f64, f64, f64)], keys: &[&str]) {
    let mut worse = Vec::new();
    for (key, first, _, last) in result.iter().filter(|r| keys.contains(&r.0)) {
        if *last > 3.0 * first.max(0.05) {
            worse.push(format!("{key}: the last third takes {last:.2} ms against {first:.2} for the first ({:.1}x, more than 3x)", last / first));
        }
    }
    assert!(worse.is_empty(), "{worse:#?}");
}

#[test]
fn long_conversation_list_short() {
    let result = soak(60);
    assert_no_worse_than_three_times(&result, &["list_ms"]);
}

/// FAULT (SOAK-NOTES row 5): reading a long conversation grows faster than its turns. The window's reader now reads
/// only what is new (idle 1.0x, tail 2.1x here); opening it fresh still reads every turn's ledger, so it grows with them.
#[test]
#[ignore = "FAULT: opening a conversation fresh slows down more than 3x over its turns, as it reads every turn's ledger (SOAK-NOTES row 5)"]
fn long_conversation_read_short() {
    let result = soak(60);
    assert_no_worse_than_three_times(&result, &["open_ms", "tail_ms", "idle_ms"]);
}

#[test]
#[ignore = "long form (300 follow-ups); fails on the row 5 fault (open, and at 300 turns the tail rebuild and the idle snapshot's copy): cargo test -p locust-core long_conversation_long -- --ignored --nocapture"]
fn long_conversation_long() {
    let result = soak(300);
    assert_no_worse_than_three_times(&result, &["list_ms", "open_ms", "tail_ms", "idle_ms"]);
}

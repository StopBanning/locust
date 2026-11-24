//! the-context-menu-shows-what-fills-it.test.tsx (0.806), through the whole path: a Claude conversation whose turn has
//! ended, the launch the engine kept for it, the ring's menu opened, `/context` answered by a stand-in with the fixture
//! Electron's host test reads, and the rows and advice drawn above Compact now. Expected words are Electron's, written
//! out.
use super::*;
use locust_core::native;
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FIXTURE: &str = include_str!("../../../desktop/src/main/fixtures/claude-context.json");

struct Dirs(std::path::PathBuf);
impl Drop for Dirs {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A store holding one finished turn on `runtime` with session `sess-1`, and the Claude launch kept for it.
fn store_with_a_finished_turn(runtime: &str) -> (Dirs, locust_core::store::Store) {
    let root = std::env::temp_dir().join(format!("locust-context-menu-{}-{}", std::process::id(), locust_core::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    let dirs = Dirs(root.clone());
    let store = locust_core::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let lines = [
        json!({"schemaVersion": 27, "recordType": "mission.created", "metadata": {"missionId": "m1", "prompt": "Synthetic prompt", "runtime": runtime, "model": "sample", "createdAt": "2026-01-01T00:00:00Z"}}),
        json!({"schemaVersion": 27, "recordType": "mission.event", "event": {"sequence": 1, "occurredAt": "2026-01-01T00:00:01Z", "type": "run.started", "payload": {"runtimeThreadId": "sess-1"}}}),
        json!({"schemaVersion": 27, "recordType": "mission.event", "event": {"sequence": 2, "occurredAt": "2026-01-01T00:00:02Z", "type": "run.completed", "payload": {"runtimeThreadId": "sess-1"}}}),
    ];
    std::fs::create_dir_all(store.ledger_dir().unwrap()).unwrap();
    std::fs::write(store.ledger_dir().unwrap().join("m1.jsonl"), lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
    // The launch as the engine keeps it when the turn starts: paths and flags only.
    let kept = json!({"m1": {"cwd": root.to_string_lossy(), "executablePath": "/usr/local/bin/claude", "sandbox": "read-only", "plugins": [],
        "args": ["--print", "--output-format", "stream-json", "--verbose", "--model", "opus", "--resume", "sess-1"]}});
    std::fs::create_dir_all(store.root()).unwrap();
    std::fs::write(store.root().join("claude-context-launches.json"), kept.to_string()).unwrap();
    (dirs, store)
}

/// `/context` answered by `answer`, each run's arguments noted.
fn stand_in(answer: Result<&'static str, &'static str>) -> Arc<Mutex<Vec<Vec<String>>>> {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let noted = asked.clone();
    *locust_core::invariants::lock(&native::hooks::CLAUDE_CONTEXT_RUN) =
        Some(Arc::new(move |command: &locust_core::claude_context::ContextCommand, _: &locust_core::claude_context::Stop| {
            noted.lock().unwrap().push(command.args.clone());
            answer.map(str::to_owned).map_err(str::to_owned)
        }));
    asked
}

/// The ring on that conversation, with the context reading and Claude Code's `/compact` listed.
fn ring_on(runtime: &str) -> rune::Runtime<State> {
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: Teammate { id: "tm_wren".into(), name: "Wren".into(), role: String::new(), runtime: Some(runtime.into()), model: None,
            account: None, busy: false, runs_in: "ask".into(), folder: None },
        conversation: Conversation { conversation_id: Some("m1".into()), turns: vec![] },
        items: vec![], dir: std::path::PathBuf::new(), worker: None,
        snapshot: ledger::Snapshot {
            context: Some(locust_core::context_usage::Reading { used: 45_200, window: 200_000, percent: 23 }),
            ..Default::default()
        },
        due: 0.0, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
    });
    let compact = locust_core::runtime_commands::CommandInfo { name: "compact".into(), description: String::new(), argument_hint: String::new() };
    s.runtime_commands.by_runtime.insert(runtime.into(), vec![compact]);
    let mut rt = rune::Runtime::new(s, |s, _| control(s, &Palette::new(true)).unwrap(), (500.0, 900.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn settle(rt: &mut rune::Runtime<State>) {
    for _ in 0..20 {
        rt.frame(0.016);
    }
}

fn open_menu(rt: &mut rune::Runtime<State>) {
    let (x, y) = rt.find("context-ring").expect("the ring");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

/// Poll until the read the open menu started has landed.
fn read_lands(rt: &mut rune::Runtime<State>) {
    let until = Instant::now() + Duration::from_secs(10);
    loop {
        rt.update(|s, _| {
            crate::runtime_commands_ui::poll(s);
        });
        let c = &rt.state.runtime_commands;
        if c.breakdown_for.is_none() || !c.breakdown_job.is_pending() {
            rt.update(|s, _| {
                crate::runtime_commands_ui::poll(s);
            });
            break;
        }
        assert!(Instant::now() < until, "the read never landed");
        std::thread::sleep(Duration::from_millis(5));
    }
    settle(rt);
}

/// The menu's words in drawing order.
fn menu_words(rt: &rune::Runtime<State>) -> String {
    let all = rt.elements();
    let Some(menu) = all.iter().position(|e| e.key.as_deref() == Some("context-ring-menu")) else { return String::new() };
    let inside = |mut i: usize| loop {
        if i == menu {
            return true;
        }
        match all[i].parent {
            Some(p) => i = p,
            None => return false,
        }
    };
    (0..all.len()).filter(|&i| !all[i].exiting && inside(i)).filter_map(|i| all[i].text.clone()).collect::<Vec<_>>().join(" | ")
}

// case: lists Claude Code's estimate by category, said to be its estimate, above Compact now
#[test]
fn lists_claude_codes_estimate_by_category_said_to_be_its_estimate_above_compact_now() {
    let (_dirs, store) = store_with_a_finished_turn("claude");
    let _hook = native::hooks::install(store, None);
    let asked = stand_in(Ok(FIXTURE));
    let mut rt = ring_on("claude");
    assert!(asked.lock().unwrap().is_empty(), "nothing is read until the menu opens");
    open_menu(&mut rt);
    read_lands(&mut rt);

    let html = menu_words(&rt);
    assert!(html.contains("Estimated by Claude Code"), "{html}");
    assert!(html.contains("Messages"), "{html}");
    assert!(html.contains("24.5k"), "{html}");
    assert!(html.contains("Free space"), "{html}");
    assert!(!html.contains("deferred"), "{html}");
    assert!(html.contains("Messages take 24.5k"), "{html}");
    assert!(html.find("Estimated by Claude Code") < html.find("Compact now"), "{html}");
    // Run with the kept launch's own flags, on the turn's session, the transport swapped for one JSON answer.
    assert_eq!(
        *asked.lock().unwrap(),
        vec![["--print", "--model", "opus", "--output-format", "json", "--no-session-persistence", "--resume", "sess-1", "--", "/context"].map(String::from).to_vec()]
    );

    // Shut, the rows go with the menu; opened again, they are drawn from the same finished turn without a second run.
    open_menu(&mut rt);
    read_lands(&mut rt);
    assert!(rt.state.runtime_commands.breakdown.is_none());
    open_menu(&mut rt);
    read_lands(&mut rt);
    assert!(menu_words(&rt).contains("Estimated by Claude Code"), "{}", menu_words(&rt));
    assert_eq!(asked.lock().unwrap().len(), 1, "one /context run per finished turn");
}

// case: is unchanged without one
#[test]
fn is_unchanged_without_one() {
    // `/context` failed: the menu is what it was.
    let (_dirs, store) = store_with_a_finished_turn("claude");
    let _hook = native::hooks::install(store, None);
    let asked = stand_in(Err("Context was not available."));
    let mut rt = ring_on("claude");
    open_menu(&mut rt);
    read_lands(&mut rt);
    let html = menu_words(&rt);
    assert_eq!(asked.lock().unwrap().len(), 1, "it was asked");
    assert!(!html.contains("Estimated by Claude Code"), "{html}");
    assert!(rt.find("context-breakdown").is_none());
    assert!(html.contains("Compact now"), "{html}");
    drop(_hook);

    // Another agent's conversation: nothing is asked, and the menu is what it was.
    let (_dirs, store) = store_with_a_finished_turn("codex");
    let _hook = native::hooks::install(store, None);
    let asked = stand_in(Ok(FIXTURE));
    let mut rt = ring_on("codex");
    open_menu(&mut rt);
    read_lands(&mut rt);
    let html = menu_words(&rt);
    assert!(asked.lock().unwrap().is_empty(), "never asked for another agent");
    assert!(!html.contains("Estimated by Claude Code"), "{html}");
    assert!(html.contains("Compact now"), "{html}");
}

//! a-full-context-is-compacted-from-the-ring.test.tsx, the seven cases the engine's own tests left: the ring drawn
//! beside the box (`control`), pressed, and its menu read as a screen reader hears it; and after a compaction, the ring
//! drawn from the conversation the window reads off disk (`ledger::Session::poll`). Expected words are Electron's,
//! written out.
use super::*;
use serde_json::{Value, json};

fn node<'a>(tree: &'a Value, key: &str) -> Option<&'a Value> {
    if tree["key"] == key {
        return Some(tree);
    }
    tree["children"].as_array()?.iter().find_map(|c| node(c, key))
}

fn states(n: &Value) -> Vec<String> {
    n["states"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect()).unwrap_or_default()
}

/// A Claude Code conversation at 164k of 200k, `compact` listed for `listed`.
fn conversation(listed: &str) -> State {
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: Teammate { id: "tm_wren".into(), name: "Wren".into(), role: String::new(), runtime: Some("claude".into()), model: Some("haiku".into()),
            account: None, busy: false, runs_in: "ask".into(), folder: None },
        conversation: Conversation { conversation_id: Some("m".into()), turns: vec![] },
        items: vec![], dir: std::path::PathBuf::new(), worker: None,
        snapshot: ledger::Snapshot {
            tip: "m".into(),
            context: Some(locust_core::context_usage::Reading { used: 164_000, window: 200_000, percent: 82 }),
            ..Default::default()
        },
        due: 0.0, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
    });
    let compact = locust_core::runtime_commands::CommandInfo { name: "compact".into(), description: "Clear conversation history but keep a summary".into(), argument_hint: String::new() };
    s.runtime_commands.by_runtime.insert(listed.into(), vec![compact]);
    s
}

fn ring(s: State) -> rune::Runtime<State> {
    let mut rt = rune::Runtime::new(s, |s, _| col().children(crate::context_ring::control(s, &Palette::new(true))), (500.0, 900.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn press_ring(rt: &mut rune::Runtime<State>) {
    let (x, y) = rt.find("context-ring").expect("the ring is drawn");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
}

/// The words inside the open menu, top to bottom.
fn menu_words(rt: &rune::Runtime<State>) -> Vec<String> {
    let all = rt.elements();
    let menu = all.iter().find(|e| e.key.as_deref() == Some("context-ring-menu")).expect("the menu is open").rect;
    let inside = |r: [f32; 4]| r[0] >= menu[0] - 0.5 && r[1] >= menu[1] - 0.5 && r[0] + r[2] <= menu[0] + menu[2] + 0.5 && r[1] + r[3] <= menu[1] + menu[3] + 0.5;
    all.iter().filter(|e| !e.exiting && inside(e.rect)).filter_map(|e| e.text.clone()).collect()
}

fn element(rt: &rune::Runtime<State>, key: &str) -> rune::ElementInfo {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("{key} is drawn"))
}

// case: opens under the reading, the way the hover says it, with what it does
#[test]
fn opens_under_the_reading_the_way_the_hover_says_it_with_what_it_does() {
    let mut s = conversation("claude");
    s.ledger.as_mut().unwrap().snapshot.cost = Some(locust_core::benchmark::RunCost { usd: Some(1.2), ..Default::default() });
    let mut rt = ring(s);
    press_ring(&mut rt);
    assert_eq!(element(&rt, "context-ring-menu").role, "menu");
    let words = menu_words(&rt);
    assert!(words.iter().any(|w| w == "Context: 164k of 200k used, 82%"), "{words:?}");
    assert!(words.iter().any(|w| w == "This conversation: $1.20"), "{words:?}");
    assert!(words.iter().any(|w| w == "Compact now"), "{words:?}");
    assert!(words.iter().any(|w| w == "Summarize the conversation so far, so the turns ahead have room. Claude Code also does this on its own when the context is full."), "{words:?}");
    let tree = rt.accessibility_json();
    assert!(!states(node(&tree, "compact-now").expect("in the tree")).iter().any(|s| s == "disabled"));
}

// case: shows the reason in place of what it does when it waits
#[test]
fn shows_the_reason_in_place_of_what_it_does_when_it_waits() {
    let mut s = conversation("claude");
    s.ledger.as_mut().unwrap().snapshot.running = true;
    // A message already waits behind the running turn, as the box queues one.
    s.queue_rows.push(composer::QueuedRow {
        id: "q1".into(), key: "m".into(), text: "Then run the tests".into(), origin: composer::Origin::Person, attachments: vec![],
        issue: None, restored: false, held: None, retry_at: None, tries: None,
    });
    let mut rt = ring(s);
    press_ring(&mut rt);
    let words = menu_words(&rt);
    assert!(words.iter().any(|w| w == "A message is already waiting for this turn to end. Compact after it goes."), "{words:?}");
    assert!(!words.iter().any(|w| w.starts_with("This conversation:")), "{words:?}");
    let tree = rt.accessibility_json();
    assert!(states(node(&tree, "compact-now").expect("in the tree")).iter().any(|s| s == "disabled"));
    // Pressed anyway, it sends nothing and the menu stays.
    let (x, y) = rt.find("compact-now").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert!(rt.state.runtime_commands.ring_open);
    assert_eq!(rt.state.queue_rows.len(), 1);
}

// case: is a button that opens a menu when Compact now can be offered
#[test]
fn is_a_button_that_opens_a_menu_when_compact_now_can_be_offered() {
    let mut rt = ring(conversation("claude"));
    let button = element(&rt, "context-ring");
    assert_eq!(button.role, "button");
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("Context: 164k of 200k used, 82%")));
    assert!(rt.find("context-ring-menu").is_none(), "shut until pressed");
    press_ring(&mut rt);
    assert!(rt.find("context-ring-menu").is_some(), "pressed, it opens its menu");
}

// case: is a button while a turn is still starting, before it has a route of its own
#[test]
fn is_a_button_while_a_turn_is_still_starting_before_it_has_a_route_of_its_own() {
    // The window has sent the message and the engine has not yet answered with the turn: no turn of its own yet, so the
    // ring reads the conversation's runtime from its teammate.
    let mut s = conversation("claude");
    s.opening_teammate = Some("tm_wren".into());
    assert!(crate::composer_wiring::turn_running(&s));
    let mut rt = ring(s);
    assert_eq!(element(&rt, "context-ring").role, "button");
    press_ring(&mut rt);
    assert!(menu_words(&rt).iter().any(|w| w == "Compact when this turn ends"), "{:?}", menu_words(&rt));
}

// case: stays a picture where it cannot
#[test]
fn stays_a_picture_where_it_cannot() {
    let mut rt = ring(conversation("codex"));
    let picture = element(&rt, "context-ring");
    assert_ne!(picture.role, "button");
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("Context: 164k of 200k used, 82%")));
    press_ring(&mut rt);
    assert!(rt.find("context-ring-menu").is_none(), "pressing a picture opens nothing");
}

// ---- after a compaction: the conversation as the window reads it off disk.

struct Dir(std::path::PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn turn_file(dir: &std::path::Path, id: &str, parent: Option<&str>, events: &[(&str, Value)]) {
    let mut metadata = json!({"missionId": id, "prompt": "Go on", "runtime": "claude", "model": "haiku", "createdAt": "2026-10-19T06:00:00Z"});
    if let Some(parent) = parent {
        metadata["continuesFrom"] = json!({"missionId": parent});
    }
    let mut lines = vec![json!({"schemaVersion": 27, "recordType": "mission.created", "metadata": metadata})];
    for (n, (kind, payload)) in events.iter().enumerate() {
        lines.push(json!({"schemaVersion": 27, "recordType": "mission.event", "event": {"id": format!("{id}-{n}"), "sequence": n + 1, "occurredAt": "2026-10-19T06:00:00.000Z", "sourceAdapter": "claude", "type": kind, "payload": payload}}));
    }
    std::fs::write(dir.join(format!("{id}.jsonl")), lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
}

fn reported(used: u64) -> (&'static str, Value) {
    ("run.completed", json!({"usage": {"contextTokens": used, "contextWindow": 200_000}, "process": {}}))
}

fn compacted() -> Vec<(&'static str, Value)> {
    vec![
        ("adapter.diagnostic", json!({"code": "claude.context_compacted", "level": "info", "terminal": false, "message": "Claude Code summarized the conversation so far, as asked, and carries on from the summary."})),
        ("run.completed", json!({"usage": {"inputTokens": 10, "outputTokens": 400}, "process": {}})),
    ]
}

/// The conversation up to `tip`, read as the window reads it, and the ring's reading drawn from it (none when no ring);
/// what the conversation cost, which the ring says after a dash, left off.
fn ring_after(dir: &Dir, tip: &str) -> Option<String> {
    let mut worker = ledger::Session::new(dir.0.clone(), tip.into());
    let snapshot = worker.poll().unwrap();
    let mut s = conversation("claude");
    let pane = s.ledger.as_mut().unwrap();
    pane.conversation.conversation_id = Some(tip.into());
    pane.snapshot = snapshot;
    let rt = ring(s);
    rt.find("context-ring")?;
    rt.elements().into_iter().filter_map(|e| e.text).find(|t| t.starts_with("Context:")).map(|t| t.split(" — ").next().unwrap_or_default().to_owned())
}

fn dir() -> Dir {
    let dir = Dir(std::env::temp_dir().join(format!("locust-compact-ring-{}-{}", std::process::id(), locust_core::invariants::unique_nanos())));
    std::fs::create_dir_all(&dir.0).unwrap();
    dir
}

// case: the ring no longer reads the conversation as it was before
#[test]
fn the_ring_no_longer_reads_the_conversation_as_it_was_before() {
    let dir = dir();
    turn_file(&dir.0, "m1", None, &[reported(164_000)]);
    assert_eq!(ring_after(&dir, "m1").as_deref(), Some("Context: 164k of 200k used, 82%"));
    // Compacted in the turn on screen.
    turn_file(&dir.0, "m2", Some("m1"), &compacted());
    assert_eq!(ring_after(&dir, "m2"), None);
    // Compacted in an earlier turn, and the turn on screen has reported nothing yet.
    turn_file(&dir.0, "m3", Some("m2"), &[]);
    assert_eq!(ring_after(&dir, "m3"), None);
}

// case: and reads again from the next turn that reports
#[test]
fn and_reads_again_from_the_next_turn_that_reports() {
    let dir = dir();
    turn_file(&dir.0, "m1", None, &[reported(164_000)]);
    turn_file(&dir.0, "m2", Some("m1"), &compacted());
    turn_file(&dir.0, "m3", Some("m2"), &[reported(18_000)]);
    assert_eq!(ring_after(&dir, "m3").as_deref(), Some("Context: 18k of 200k used, 9%"));
}

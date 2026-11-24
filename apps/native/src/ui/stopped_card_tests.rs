//! The stopped run's card through the thread that draws it (`message_column`): Electron's events through
//! `Transcript::apply`, the turn's runtime from the conversation's teammate, the person's own press from
//! `stopped_by_you`, and the time from the turn's last event. Two Electron files: a-run-a-teammate-stopped-does-not-say-you
//! (who the card says stopped it) and a-stopped-card-counts-the-plan-and-says-what-is-unknown (what it counts and warns
//! of). Expected words are Electron's, written out.
use super::*;
use chrono::TimeZone;
use locust_core::time_marker::TurnTiming;
use serde_json::{Value, json};

fn turn_items(rows: &[(&str, Value)]) -> Vec<ledger::Item> {
    let mut all: Vec<(&str, Value)> = vec![("run.started", json!({}))];
    all.extend(rows.iter().cloned());
    all.push(("run.cancelled", json!({})));
    let mut t = ledger::Transcript::default();
    t.apply(
        all.iter()
            .enumerate()
            .map(|(i, (kind, payload))| json!({"recordType": "mission.event", "event": {"type": kind, "sequence": i + 1, "occurredAt": "2026-10-03T20:00:00Z", "payload": payload}}))
            .collect(),
    );
    let mut items = vec![ledger::Item::User("Fix the build".into())];
    items.extend(t.items);
    items
}

fn tool_ran(id: &str, command: &str) -> Vec<(&'static str, Value)> {
    vec![
        ("tool.started", json!({"itemId": id, "name": "read", "toolKind": "read", "command": command, "phase": "started"})),
        ("tool.completed", json!({"itemId": id, "name": "read", "phase": "completed"})),
    ]
}

fn plan_of(steps: Value) -> (&'static str, Value) {
    ("plan.updated", json!({"plan": steps}))
}

struct Drawn {
    words: Vec<String>,
}

impl Drawn {
    fn all(&self) -> String {
        self.words.join("\n")
    }
    fn count(&self, needle: &str) -> usize {
        self.words.iter().map(|w| w.matches(needle).count()).sum()
    }
    /// The words drawn on the row a label heads ("Cut off", "Plan"), the label itself first.
    fn row_of(&self, label: &str) -> Option<String> {
        let at = self.words.iter().position(|w| w == label)?;
        let next = self.words[at + 1..].iter().position(|w| ["Plan", "Finished", "Cut off", "Never started"].contains(&w.as_str())).map_or(self.words.len(), |n| at + 1 + n);
        Some(self.words[at..next].join("\n"))
    }
}

/// The thread of one stopped turn on `runtime`, in the folder `root`; `pressed` when the person pressed its Stop here;
/// `last_at` the turn's last event. Only the card's own words, top to bottom.
fn stopped_thread(items: Vec<ledger::Item>, runtime: &str, root: Option<&str>, pressed: bool, last_at: Option<String>) -> Drawn {
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: Teammate { id: "tm_wren".into(), name: "Wren".into(), role: String::new(), runtime: Some(runtime.into()), model: None,
            account: None, busy: false, runs_in: "ask".into(), folder: root.map(str::to_owned) },
        conversation: Conversation { conversation_id: Some("m1".into()), turns: vec![] },
        items: vec![], dir: std::path::PathBuf::new(), worker: None,
        snapshot: ledger::Snapshot {
            tip: "m1".into(),
            timings: vec![TurnTiming { before_item: 0, first_at: last_at.clone(), last_at }],
            ..Default::default()
        },
        due: 0.0, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
    });
    if pressed {
        s.stopped_by_you = Some("m1".into());
    }
    let mut rt = rune::Runtime::new((s, items), |(s, items), _| message_column(items, "Wren", &Palette::new(true), s), (800.0, 3000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    // The thread keys the card by its item; it is the item that holds the card's title.
    let title = all.iter().find(|e| e.text.as_deref().is_some_and(|t| t.starts_with("You stopped this run") || t.starts_with("This run was stopped"))).expect("the stopped card's title").rect;
    let holds = |r: [f32; 4], inner: [f32; 4]| r[0] <= inner[0] + 0.5 && r[1] <= inner[1] + 0.5 && r[0] + r[2] >= inner[0] + inner[2] - 0.5 && r[1] + r[3] >= inner[1] + inner[3] - 0.5;
    let card = all.iter().filter(|e| e.key.as_deref().is_some_and(|k| k.contains("/ledger-item-")) && holds(e.rect, title)).map(|e| e.rect).min_by(|a, b| (a[2] * a[3]).total_cmp(&(b[2] * b[3]))).expect("the card's item");
    let inside = |r: [f32; 4]| r[0] >= card[0] - 0.5 && r[1] >= card[1] - 0.5 && r[0] + r[2] <= card[0] + card[2] + 0.5 && r[1] + r[3] <= card[1] + card[3] + 0.5;
    let mut words: Vec<_> = all.iter().filter(|e| !e.exiting && inside(e.rect)).filter_map(|e| e.text.clone().map(|t| (e.rect[1], e.rect[0], t))).collect();
    words.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    Drawn { words: words.into_iter().map(|(_, _, t)| t).collect() }
}

/// 17:08 on this computer's clock, as the record writes it, so the expected clock is a literal in any zone.
fn five_past_eight_pm() -> String {
    chrono::Local.with_ymd_and_hms(2026, 10, 3, 17, 8, 0).single().expect("a plain local time").to_rfc3339()
}

// --- a-run-a-teammate-stopped-does-not-say-you.test.tsx

// case: says "You" for a Stop the person pressed
#[test]
fn says_you_for_a_stop_the_person_pressed() {
    let card = stopped_thread(turn_items(&tool_ran("r", "README.md")), "codex", None, true, Some(five_past_eight_pm()));
    assert!(card.words.iter().any(|w| w == "You stopped this run at 5:08 PM"), "{:?}", card.words);
}

// case: does not, for a run stopped by anything else
#[test]
fn does_not_for_a_run_stopped_by_anything_else() {
    let card = stopped_thread(turn_items(&tool_ran("r", "README.md")), "codex", None, false, Some(five_past_eight_pm()));
    assert!(card.words.iter().any(|w| w == "This run was stopped at 5:08 PM"), "{:?}", card.words);
    assert!(!card.all().contains("You stopped"), "{:?}", card.words);
}

#[test]
fn a_turn_with_no_recorded_time_says_no_time() {
    let card = stopped_thread(turn_items(&tool_ran("r", "README.md")), "codex", None, true, None);
    assert!(card.words.iter().any(|w| w == "You stopped this run"), "{:?}", card.words);
}

// --- a-stopped-card-counts-the-plan-and-says-what-is-unknown.test.tsx

// case: uses the latest plan and never treats extra tool calls as completed plan steps
#[test]
fn uses_the_latest_plan_and_never_treats_extra_tool_calls_as_completed_plan_steps() {
    let mut rows = vec![plan_of(json!(["Old"])), plan_of(json!([{"step": "One", "status": "in_progress"}, {"step": "Two", "status": "pending"}]))];
    for (id, name) in [("a", "a.txt"), ("b", "b.txt"), ("c", "c.txt")] {
        rows.extend(tool_ran(id, name));
    }
    let card = stopped_thread(turn_items(&rows), "opencode", Some("C:/work/project"), true, None);
    assert!(card.words.iter().any(|w| w == "0 finished · 1 cut off · 1 never started (2 steps)"), "{:?}", card.words);
}

// case: shows a fully finished plan even without tool calls
#[test]
fn shows_a_fully_finished_plan_even_without_tool_calls() {
    let card = stopped_thread(turn_items(&[plan_of(json!([{"step": "Answer", "status": "completed"}]))]), "codex", Some("C:/work/project"), true, None);
    assert!(card.words.iter().any(|w| w == "1 finished · 0 cut off · 0 never started (1 step)"), "{:?}", card.words);
    let all = card.all();
    assert!(!all.contains("Stopped before it used any tools"), "{all}");
    assert!(!all.contains("may still finish on its own"), "{all}");
    assert!(!all.contains("undoing a change is yours to do"), "{all}");
}

// case: does not invent a plan for a stopped command without one
#[test]
fn does_not_invent_a_plan_for_a_stopped_command_without_one() {
    let card = stopped_thread(turn_items(&tool_ran("command", "pnpm test")), "opencode", Some("C:/work/project"), true, None);
    assert!(card.row_of("Plan").is_none(), "{:?}", card.words);
    assert!(card.row_of("Finished").is_some_and(|r| r.contains("pnpm test")), "{:?}", card.words);
}

// case: offers an unreported OpenCode command under Cut off with its warning once
#[test]
fn offers_an_unreported_opencode_command_under_cut_off_with_its_warning_once() {
    let card = stopped_thread(turn_items(&tool_ran("disk", "started.txt")), "opencode", Some("C:/work/project"), true, None);
    let cut = card.row_of("Cut off").expect("a Cut off row");
    assert!(cut.contains("An unreported OpenCode command may have been running when stopped"), "{cut}");
    assert_eq!(card.count("may have been running when stopped"), 1, "{:?}", card.words);
    assert!(!card.all().contains("may still finish on its own"), "{:?}", card.words);
    assert!(!card.all().contains("None reported."), "{:?}", card.words);
}

// case: shows the same uncertainty when OpenCode reported no completed tools
#[test]
fn shows_the_same_uncertainty_when_opencode_reported_no_completed_tools() {
    let card = stopped_thread(turn_items(&[]), "opencode", Some("C:/work/project"), true, None);
    assert!(card.row_of("Cut off").is_some(), "{:?}", card.words);
    assert!(card.all().contains("An unreported OpenCode command may have been running when stopped"), "{:?}", card.words);
    assert!(!card.all().contains("nothing is half-done"), "{:?}", card.words);
}

// case: keeps reported interrupted commands and states OpenCode uncertainty once
#[test]
fn keeps_reported_interrupted_commands_and_states_opencode_uncertainty_once() {
    let rows = [("tool.started", json!({"itemId": "running", "name": "bash", "toolKind": "shell", "command": "pnpm test", "phase": "started"}))];
    let card = stopped_thread(turn_items(&rows), "opencode", Some("C:/work/project"), true, None);
    assert!(card.row_of("Cut off").is_some_and(|r| r.contains("pnpm test")), "{:?}", card.words);
    assert_eq!(card.count("may have been running when stopped"), 1, "{:?}", card.words);
}

// case: keeps the ordinary runtime warning and never invents OpenCode uncertainty for it
#[test]
fn keeps_the_ordinary_runtime_warning_and_never_invents_opencode_uncertainty_for_it() {
    let rows = [("tool.started", json!({"itemId": "running", "name": "shell", "toolKind": "shell", "command": "pnpm test", "phase": "started"}))];
    let card = stopped_thread(turn_items(&rows), "codex", Some("C:/work/project"), true, None);
    assert!(card.all().contains("may still finish on its own"), "{:?}", card.words);
    assert!(!card.all().contains("unreported OpenCode command"), "{:?}", card.words);
}

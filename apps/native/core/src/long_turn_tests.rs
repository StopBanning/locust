//! Electron's `renderer/src/a-long-turn-keeps-what-it-said.test.ts`, one Rust test per case, with its shapes and numbers.
//! Colin's run, 2026-10-05: a message early, a long run of calls, a second message, more calls.
use crate::ledger::{EVENT_WINDOW, Item, OPENING_EVENTS, TRIMMED_TURN_LINE, Transcript};
use serde_json::{Value, json};

struct Run {
    sequence: u64,
    events: Vec<Value>,
}

impl Run {
    fn new() -> Self {
        Self { sequence: 0, events: Vec::new() }
    }
    fn event(&mut self, kind: &str, payload: Value) -> Value {
        self.sequence += 1;
        let mut payload = payload;
        payload["evidence"] = json!({ "redacted": true });
        json!({
            "id": format!("e{}", self.sequence),
            "runId": "r",
            "missionId": "m",
            "sequence": self.sequence,
            "occurredAt": format!("2026-10-05T07:40:00.000Z"),
            "sourceAdapter": "claude",
            "type": kind,
            "payload": payload,
        })
    }
    fn push(&mut self, kind: &str, payload: Value) {
        let e = self.event(kind, payload);
        self.events.push(e);
    }
    fn said(&mut self, item_id: &str, text: &str) {
        self.push("message.delta", json!({ "itemId": item_id, "operation": "append", "text": text, "final": true }));
    }
    /// One tool call as Claude Code reports it: a step that opens and closes around the call's start and end.
    fn call(&mut self, n: usize) {
        let id = format!("toolu_{n}");
        let command = format!("echo {n}");
        self.push("step.started", json!({ "stepKind": "item", "itemId": format!("{id}_step"), "itemType": "tool" }));
        self.push("tool.started", json!({ "itemId": id, "toolKind": "tool_use", "name": "Bash", "phase": "started" }));
        self.push("tool.started", json!({ "itemId": id, "toolKind": "tool_use", "name": "Bash", "command": command, "phase": "started" }));
        self.push("step.completed", json!({ "stepKind": "item", "itemId": format!("{id}_step"), "itemType": "tool" }));
        self.push("tool.completed", json!({ "itemId": id, "toolKind": "tool_use", "name": "Bash", "command": command, "phase": "completed", "exitCode": 0 }));
        self.push("step.started", json!({ "stepKind": "item", "itemId": format!("{id}_after"), "itemType": "tool" }));
        self.push("step.completed", json!({ "stepKind": "item", "itemId": format!("{id}_after"), "itemType": "tool" }));
        self.push("step.completed", json!({ "stepKind": "item", "itemId": format!("{id}_done"), "itemType": "tool" }));
    }
    /// The test's `run()`.
    fn colins() -> Self {
        let mut r = Self::new();
        r.push("run.started", json!({}));
        r.push("plan.updated", json!({ "plan": [{ "step": "Read the sample files", "status": "in_progress" }] }));
        r.call(1);
        r.said("m1", "Setting up the worktree and reading how the check is wired.");
        for at in 0..40 {
            r.call(2 + at);
        }
        r.said("m2", "Install finished. Building the app next.");
        for at in 0..50 {
            r.call(42 + at);
        }
        r
    }
    /// `run()` followed by 500 more calls: longer than the window.
    fn too_long() -> Self {
        let mut r = Self::colins();
        for at in 0..500 {
            r.call(100 + at);
        }
        r
    }
}

fn records(events: &[Value]) -> Vec<Value> {
    events.iter().map(|e| json!({ "recordType": "mission.event", "event": e })).collect()
}

fn held(events: &[Value]) -> Transcript {
    let mut t = Transcript::default();
    t.apply(records(events));
    t
}

/// Arriving as a live run does, in many small batches (eviction runs after each event either way).
fn held_live(events: &[Value]) -> Transcript {
    let mut t = Transcript::default();
    for batch in events.chunks(100) {
        t.apply(records(batch));
    }
    t
}

fn messages(t: &Transcript) -> Vec<String> {
    t.items
        .iter()
        .filter_map(|i| match i {
            Item::Assistant { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn notices(t: &Transcript) -> usize {
    t.items.iter().filter(|i| matches!(i, Item::Note { text, .. } if text == TRIMMED_TURN_LINE)).count()
}

fn sequences(t: &Transcript) -> Vec<u64> {
    t.events_in_view().map(|e| e["sequence"].as_u64().unwrap()).collect()
}

fn trace_text(t: &Transcript) -> String {
    t.items
        .iter()
        .find_map(|i| match i {
            Item::Trace(trace) => Some(trace.text()),
            _ => None,
        })
        .unwrap_or_default()
}

#[test]
fn is_longer_than_the_old_window_held_as_colins_run_was() {
    assert!(Run::colins().events.len() > 500);
}

#[test]
fn keeps_both_messages_in_a_running_turn_of_that_length() {
    let run = Run::colins();
    let t = held(&run.events);
    assert_eq!(t.events_in_view().count(), run.events.len());
    assert_eq!(
        messages(&t),
        vec!["Setting up the worktree and reading how the check is wired.", "Install finished. Building the app next."]
    );
}

#[test]
fn draws_the_live_run_and_the_record_through_one_window() {
    // Rust reads the live run and the record through the same Transcript, so there is one number by construction.
    assert_eq!(EVENT_WINDOW, 3_000);
    assert_eq!(OPENING_EVENTS, 750);
}

#[test]
fn says_so_at_its_top_when_even_the_window_is_too_small_and_only_then() {
    let trimmed = held(&Run::too_long().events);
    assert!(trimmed.trimmed);
    assert!(matches!(&trimmed.items[0], Item::Note { level, text } if level == "info" && text == TRIMMED_TURN_LINE));
    assert_eq!(notices(&trimmed), 1);
    let whole = held(&Run::colins().events);
    assert!(!whole.trimmed);
    assert_eq!(notices(&whole), 0);
}

#[test]
fn counts_at_least_when_part_of_the_turn_is_out_of_view_and_not_otherwise() {
    let trimmed = trace_text(&held(&Run::too_long().events));
    assert!(trimmed.contains("at least"), "{trimmed}");
    // The control: a turn drawn whole says its counts plainly.
    let whole = trace_text(&held(&Run::colins().events));
    assert!(!whole.is_empty());
    assert!(!whole.contains("at least"), "{whole}");
}

#[test]
fn keeps_its_opening_messages_and_newest_live_step_after_repeated_evictions() {
    let mut run = Run::too_long();
    run.push("tool.started", json!({ "itemId": "newest", "toolKind": "tool_use", "name": "Read", "phase": "started" }));
    let all = run.events;
    let t = held_live(&all);
    assert_eq!(t.events_in_view().count(), 3000);
    assert!(t.trimmed);
    assert_eq!(messages(&t), messages(&held(&Run::colins().events)));
    assert_eq!(t.events_in_view().last().unwrap()["payload"]["itemId"], "newest");
    let plan = t.items.iter().find_map(|i| match i {
        Item::Plan { steps, .. } => Some(steps.iter().map(|s| (s.text.clone(), s.state.clone())).collect::<Vec<_>>()),
        _ => None,
    });
    assert_eq!(plan, Some(vec![("Read the sample files".to_owned(), "running".to_owned())]));
    // windowEvents(all): the first quarter and the newest three quarters.
    let expected: Vec<u64> = all[..OPENING_EVENTS]
        .iter()
        .chain(&all[all.len() - (EVENT_WINDOW - OPENING_EVENTS)..])
        .map(|e| e["sequence"].as_u64().unwrap())
        .collect();
    assert_eq!(sequences(&t), expected);
}

#[test]
fn joins_continued_opening_prose_and_preserves_its_omission_flag_across_batches() {
    let mut run = Run::too_long();
    let mut t = held(&run.events);
    let before = sequences(&t);
    let from = run.events.len();
    run.said("m1", " Still checking.");
    run.said("new", "Newest update.");
    t.apply(records(&run.events[from..]));
    assert_eq!(t.events_in_view().count(), 3000);
    assert!(t.trimmed);
    let said = messages(&t);
    assert_eq!(said[0], "Setting up the worktree and reading how the check is wired. Still checking.");
    assert_eq!(said.last().unwrap(), "Newest update.");
    assert_ne!(before, sequences(&t));
}

#[test]
fn does_not_call_a_full_or_fragment_joined_window_trimmed_until_an_event_is_dropped() {
    let mut run = Run::new();
    for n in 0..3000 {
        run.said(&format!("m{n}"), "word");
    }
    let mut t = held(&run.events);
    assert!(!t.trimmed);
    let more = |t: &mut Transcript, run: &mut Run, id: &str, text: &str| {
        let from = run.events.len();
        run.said(id, text);
        t.apply(records(&run.events[from..]));
    };
    more(&mut t, &mut run, "m0", " more");
    assert!(!t.trimmed);
    more(&mut t, &mut run, "new", "new");
    assert!(t.trimmed);
    more(&mut t, &mut run, "new", " more");
    assert!(t.trimmed);
}

// a-trimmed-live-turn-says-so-on-screen.test.tsx's counts, at the transcript (the window's drawing is in
// src/ui/messages.rs's tests).
fn steps(count: u64) -> Transcript {
    let events: Vec<Value> = (1..=count)
        .map(|sequence| {
            json!({ "id": format!("e{sequence}"), "runId": "run", "missionId": "mission", "sequence": sequence,
                "occurredAt": "2026-10-05T12:00:00.000Z", "sourceAdapter": "claude", "type": "step.completed",
                "payload": { "stepKind": "turn", "evidence": { "redacted": true } } })
        })
        .collect();
    held_live(&events)
}

#[test]
fn shows_the_omission_notice_once_when_a_running_turn_exceeds_3000_events() {
    assert_eq!(notices(&steps(3001)), 1);
}

#[test]
fn shows_no_omission_notice_below_or_exactly_at_3000_events() {
    assert_eq!(notices(&steps(739)), 0);
    assert_eq!(notices(&steps(3000)), 0);
}

/// Replayed records (the watcher reading back what the live channel already gave) fold in once.
#[test]
fn a_fragment_read_twice_or_late_folds_in_once_and_in_order() {
    let mut run = Run::new();
    run.push("message.delta", json!({ "itemId": "m", "operation": "append", "text": "One", "final": false }));
    run.push("message.delta", json!({ "itemId": "m", "operation": "append", "text": " two", "final": false }));
    run.push("message.delta", json!({ "itemId": "m", "operation": "append", "text": " three.", "final": true }));
    let e = run.events;
    let mut t = Transcript::default();
    t.apply(records(&[e[0].clone(), e[2].clone()]));
    t.apply(records(&[e[1].clone(), e[0].clone(), e[2].clone()]));
    assert_eq!(messages(&t), vec!["One two three."]);
    assert_eq!(t.events_in_view().count(), 1);
}

// missionView's activityTrace with `trimmed`, one rule at a time (each was hidden behind the commands line above).
fn trace_of(run: &Run, trimmed: bool) -> String {
    let t = held(&run.events);
    crate::activity_trace::activity_trace_of(&t.items, &run.events, "completed", Some(true), None, trimmed).text()
}

#[test]
fn a_trimmed_turn_counts_its_other_calls_as_at_least() {
    let mut run = Run::new();
    run.push("tool.started", json!({ "itemId": "read_1", "toolKind": "tool_use", "name": "Read", "phase": "started" }));
    run.push("tool.completed", json!({ "itemId": "read_1", "toolKind": "tool_use", "name": "Read", "phase": "completed" }));
    assert!(trace_of(&run, true).contains("at least 1 tool call"), "{}", trace_of(&run, true));
    assert!(!trace_of(&run, false).contains("at least"), "{}", trace_of(&run, false));
}

#[test]
fn a_trimmed_turn_never_says_no_files_changed() {
    let mut run = Run::new();
    run.push("tool.started", json!({ "itemId": "read_1", "toolKind": "tool_use", "name": "Read", "phase": "started" }));
    run.push("tool.completed", json!({ "itemId": "read_1", "toolKind": "tool_use", "name": "Read", "phase": "completed" }));
    assert!(trace_of(&run, false).contains("no files changed"), "{}", trace_of(&run, false));
    assert!(!trace_of(&run, true).contains("no files changed"), "{}", trace_of(&run, true));
}

#[test]
fn a_trimmed_turn_counts_its_one_command_instead_of_naming_it() {
    let mut run = Run::new();
    run.call(7);
    let whole = trace_of(&run, false);
    assert!(whole.contains("echo 7"), "{whole}");
    let trimmed = trace_of(&run, true);
    assert!(trimmed.contains("ran at least 1 command"), "{trimmed}");
    assert!(!trimmed.contains("echo 7"), "{trimmed}");
}

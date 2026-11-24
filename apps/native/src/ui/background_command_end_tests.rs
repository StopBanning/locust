//! Electron's `a-background-command-says-what-became-of-it.test.tsx`: Claude's background command, read by the ledger
//! reader (`Transcript::apply`, the window's buildThread) and drawn by the thread's activity row (`activity_item`). The
//! events are Electron's capture, reduced as Electron reduces it; expected words are Electron's, written out.
use super::*;
use serde_json::{Value, json};

const CALL: &str = "toolu_016MCbGpcf7wkMy2eHrDb6LC";
const COMMAND: &str = "sleep 8 && echo finished > out.txt";
const SAID: &str = "Background task: sleep 8 seconds then write to out.txt";

fn opened() -> (&'static str, Value) {
    ("tool.started", json!({"itemId": CALL, "toolKind": "tool_use", "name": "Bash", "phase": "started"}))
}
fn backgrounded() -> (&'static str, Value) {
    ("step.started", json!({"stepKind": "item", "itemId": CALL, "itemType": "background", "status": "running", "message": SAID}))
}
fn returned() -> (&'static str, Value) {
    ("tool.completed", json!({"itemId": CALL, "toolKind": "tool_use", "name": "Bash", "command": COMMAND, "title": SAID, "background": true, "phase": "completed"}))
}
fn ended_as(status: &str, kind: &'static str) -> (&'static str, Value) {
    (kind, json!({"stepKind": "item", "itemId": CALL, "itemType": "background", "status": status}))
}

fn read_turn(events: &[(&'static str, Value)], running: bool) -> ledger::Transcript {
    let mut all = vec![("run.started", json!({}))];
    all.extend(events.iter().cloned());
    if !running {
        all.push(("run.completed", json!({})));
    }
    let mut t = ledger::Transcript::default();
    t.apply(
        all.iter()
            .enumerate()
            .map(|(i, (kind, payload))| {
                json!({"recordType": "mission.event", "event": {"type": kind, "sequence": i + 1, "sourceAdapter": "claude",
                    "occurredAt": format!("2026-09-22T22:06:{:02}.000Z", 10 + i), "payload": payload}})
            })
            .collect(),
    );
    t
}

/// The command row the reader made of `events` in a run that is over.
fn command_row(events: &[(&'static str, Value)]) -> (String, ledger::Status, ledger::ToolDetail) {
    let t = read_turn(events, false);
    t.items
        .into_iter()
        .find_map(|item| match item {
            ledger::Item::Tool { name, summary, detail, status } if name == "Bash" => Some((summary, status, *detail)),
            _ => None,
        })
        .expect("a command row")
}

/// One command row drawn on its own; `finished` as the reader marks a call once its run is over.
fn drawn_row(summary: &str, background: bool, ended: &str, finished: bool) -> (Vec<String>, Vec<rune::ElementInfo>) {
    let item = ledger::Item::Tool {
        name: "Bash".into(),
        summary: summary.into(),
        detail: Box::new(ledger::ToolDetail {
            id: "c1".into(),
            arguments: COMMAND.into(),
            background,
            background_ended: ended.into(),
            finished,
            ..Default::default()
        }),
        status: ledger::Status::Done,
    };
    let mut rt = rune::Runtime::new((State::default(), item), |(s, item), _| activity_item(item, &Palette::new(true), s, "conv"), (900.0, 300.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let els = rt.elements();
    (els.iter().filter_map(|e| e.text.clone()).collect(), els)
}

// case: a Claude command row leads with what the model said it was doing
#[test]
fn a_claude_command_row_leads_with_its_description() {
    let (summary, _, detail) = command_row(&[opened(), returned()]);
    assert_eq!(summary, SAID);
    assert_eq!(detail.arguments, COMMAND);
    assert!(detail.background);
    // Drawn, the row's title is the sentence, not the pipeline.
    let (words, _) = drawn_row(&summary, detail.background, &detail.background_ended, false);
    assert!(words.iter().any(|w| w == SAID), "{words:?}");
    assert!(!words.iter().any(|w| w == COMMAND), "{words:?}");
}

// case: learns the work was stopped when the run ended, after the call had closed
#[test]
fn learns_the_work_stopped_with_the_run_after_the_call_closed() {
    let (_, status, detail) = command_row(&[opened(), backgrounded(), returned(), ended_as("stopped-with-run", "step.completed")]);
    assert_eq!(status, ledger::Status::Done, "settled");
    assert_eq!(detail.background_ended, "stopped-with-run");
}

// case: work that finished, or failed, in the background says that
#[test]
fn background_work_says_it_finished_or_failed() {
    let ending = |status: &str, kind: &'static str| command_row(&[opened(), backgrounded(), returned(), ended_as(status, kind)]).2.background_ended;
    assert_eq!(ending("completed", "step.completed"), "completed");
    assert_eq!(ending("failed", "step.failed"), "failed");
    // A word from a later build is still an ending, not silence.
    assert_eq!(ending("evaporated", "step.completed"), "ended");
}

// case: background work does not take the live line, or end it
#[test]
fn background_work_neither_takes_nor_ends_the_live_line() {
    let reading = ("step.started", json!({"stepKind": "item", "itemId": "r1", "itemType": "reasoning-free", "message": "Reading the test output"}));
    let live = |events: &[(&'static str, Value)]| read_turn(events, true).live.map(|line| line.label);
    assert_eq!(live(&[opened(), backgrounded(), returned(), reading.clone()]).as_deref(), Some("Reading the test output"));
    assert_eq!(live(&[opened(), backgrounded(), returned(), reading, ended_as("completed", "step.completed")]).as_deref(), Some("Reading the test output"));
}

// case: stopped with the run: amber, and says why
#[test]
fn stopped_with_the_run_is_amber_and_says_why() {
    let (words, els) = drawn_row(SAID, true, "stopped-with-run", true);
    assert!(words.iter().any(|w| w == "stopped"), "{words:?}");
    assert!(words.iter().any(|w| w.contains("when the run ended")), "{words:?}");
    let l = crate::look::controls::Look::new(true);
    let badge = els.iter().find(|e| e.key.as_deref() == Some("conv/c1/badge")).map(|e| e.fill);
    assert_eq!(badge, Some(l.c("amber-tint")), "is-stalled");
    // Not the green `done` that read as finished work.
    assert!(!words.iter().any(|w| w == "done"), "{words:?}");
}

// case: still running while the run is live, and not claimed afterwards
#[test]
fn running_while_live_and_not_claimed_once_over() {
    let (live, _) = drawn_row(SAID, true, "", false);
    assert!(live.iter().any(|w| w == "running"), "{live:?}");
    assert!(live.iter().any(|w| w.contains("in the background")), "{live:?}");
    // Once the run is over with no word on it, nothing is claimed either way.
    let (over, _) = drawn_row(SAID, true, "", true);
    assert!(over.iter().any(|w| w.contains("did not report")), "{over:?}");
    assert!(!over.iter().any(|w| w == "running"), "{over:?}");
}

// case: a described row keeps the command one press away
#[test]
fn a_described_row_keeps_its_command_one_press_away() {
    let (words, els) = drawn_row(SAID, false, "", true);
    assert!(words.iter().any(|w| w == SAID), "{words:?}");
    let opener = els.iter().find(|e| e.key.as_deref() == Some("conv/c1")).expect("the row");
    assert_eq!(opener.role, "button", "the row opens onto the command");
    // Opened, the command is there.
    let item = ledger::Item::Tool {
        name: "Bash".into(),
        summary: SAID.into(),
        detail: Box::new(ledger::ToolDetail { id: "c1".into(), arguments: COMMAND.into(), finished: true, ..Default::default() }),
        status: ledger::Status::Done,
    };
    let mut s = State::default();
    s.activity_open.insert("conv/c1".into(), true);
    let mut rt = rune::Runtime::new((s, item), |(s, item), _| activity_item(item, &Palette::new(true), s, "conv"), (900.0, 300.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some(COMMAND)), "the command, one press away");
    // No description: nothing to open onto, so it stays a plain row showing the command itself.
    let (plain, els) = drawn_row("", false, "", true);
    assert!(plain.iter().any(|w| w == COMMAND), "{plain:?}");
    let row = els.iter().find(|e| e.key.as_deref() == Some("conv/c1")).expect("the row");
    assert_ne!(row.role, "button", "nothing to open");
}

//! a-long-fold-keeps-the-answer-near.test.tsx (3 cases). The fold it draws (ActivityCard's default card, 10 rows) is
//! one Electron's app never shows: Thread draws only the steps and files variants. The same rule caps the files
//! variant, the turn's changed files, at five (FILE_ROWS_SHOWN, 0.835), so the cases are carried there: a finished
//! turn shows its first five and "Show N more", a live turn has no foot to fold, and a fold only a row or two over is
//! left alone.
use super::*;
use serde_json::{Value, json};

fn turn(count: usize, finished: bool) -> Vec<ledger::Item> {
    let mut rows: Vec<(String, Value)> = vec![("run.started".into(), json!({}))];
    for i in 1..=count {
        let path = format!("step-{i}.md");
        let patch = json!({"text": format!("diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1 @@\n+x\n"), "truncated": false, "added": 1, "removed": 0});
        let started = json!({"itemId": format!("w{i}"), "toolKind": "file_change", "name": "Write", "command": path, "phase": "started"});
        let mut done = started.clone();
        done["phase"] = json!("completed");
        done["patch"] = patch;
        rows.push(("tool.started".into(), started));
        rows.push(("tool.completed".into(), done));
    }
    if finished {
        rows.push(("message.delta".into(), json!({"itemId": "m1", "operation": "append", "text": "Done.", "final": true})));
        rows.push(("run.completed".into(), json!({"process": {"exitCode": 0}})));
    }
    let mut t = locust_core::ledger::Transcript::default();
    t.apply(rows.iter().enumerate().map(|(i, (kind, payload))| json!({"recordType": "mission.event", "event": {"type": kind, "sequence": i + 1, "occurredAt": "2026-09-30T05:00:00.000Z", "payload": payload}})).collect());
    t.items
}

fn render(items: Vec<ledger::Item>, running: bool, open_all: Option<String>) -> Vec<rune::ElementInfo> {
    let mut s = State::default();
    s.teammates.push(crate::Teammate { id: "tm".into(), name: "Codec".into(), role: "Custom".into(), model: None, runtime: Some("claude".into()), folder: Some("C:/work".into()), runs_in: "ask".into(), account: None, busy: running });
    s.conversations.insert("tm".into(), Default::default());
    if running {
        s.opening_teammate = Some("tm".into());
    }
    if let Some(key) = open_all {
        s.activity_open.insert(key, true);
    }
    let mut rt = rune::Runtime::new((s, items), |(s, items), _| message_column(items, "Codec", &Palette::new(true), s), (800.0, 3000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements()
}

/// The thread's words; with `press_more`, after "Show N more" was pressed (its press sets `<scope>/files/all`).
fn drawn(items: Vec<ledger::Item>, running: bool, press_more: bool) -> Vec<String> {
    let mut els = render(items.clone(), running, None);
    if press_more {
        let more = els.iter().find_map(|e| e.key.clone().filter(|k| k.ends_with("/files/more"))).expect("Show N more");
        els = render(items, running, Some(more.replace("/files/more", "/files/all")));
    }
    els.into_iter().filter_map(|e| e.text).collect()
}

fn names(words: &[String], file: &str) -> bool {
    words.iter().any(|w| w.contains(file))
}

// case: shows its first rows once finished, and says how many more
#[test]
fn a_finished_turns_files_show_the_first_five_and_say_how_many_more() {
    let words = drawn(turn(25, true), false, false);
    assert!(names(&words, "step-5.md"), "{words:?}");
    assert!(!names(&words, "step-6.md"), "{words:?}");
    assert!(words.iter().any(|w| w == "Show 20 more"), "{words:?}");
    // And the press shows the rest.
    let all = drawn(turn(25, true), false, true);
    assert!(names(&all, "step-25.md") && !all.iter().any(|w| w == "Show 20 more"), "{all:?}");
}

// case: shows every row while the run is going
#[test]
fn a_turn_still_going_has_no_files_to_fold() {
    let words = drawn(turn(25, false), true, false);
    assert!(!words.iter().any(|w| w.starts_with("Show ") && w.ends_with(" more")), "{words:?}");
}

// case: leaves a fold that is only a row or two over alone
#[test]
fn a_list_only_a_row_or_two_over_is_left_alone() {
    let words = drawn(turn(super::activity_drawers::FILE_ROWS_SHOWN + 2, true), false, false);
    assert!(names(&words, "step-7.md"), "{words:?}");
    assert!(!words.iter().any(|w| w.starts_with("Show ") && w.ends_with(" more")), "{words:?}");
}

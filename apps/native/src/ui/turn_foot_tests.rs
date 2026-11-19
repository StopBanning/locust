//! The window's half of the turn foot (Thread.tsx `TurnFoot`): the files card drawn above the totals with Electron's
//! heading, open, its rows the folder's; and no foot while the turn is still going. Electron's events through
//! `Transcript::apply`, drawn by `message_column`.
use super::*;
use serde_json::{Value, json};

fn events(rows: &[(&str, Value)]) -> Vec<ledger::Item> {
    let mut t = locust_core::ledger::Transcript::default();
    t.apply(
        rows.iter()
            .enumerate()
            .map(|(i, (kind, payload))| json!({"recordType": "mission.event", "event": {"type": kind, "sequence": i + 1, "occurredAt": "2026-09-30T05:00:00.000Z", "payload": payload}}))
            .collect(),
    );
    t.items
}

fn tool(id: &str, kind: &str, name: &str, path: &str, status: Option<&str>, patch: Option<Value>) -> Vec<(&'static str, Value)> {
    let mut p = json!({"itemId": id, "toolKind": kind, "name": name, "command": path, "phase": "started"});
    if let Some(status) = status { p["status"] = json!(status); }
    let mut done = p.clone();
    done["phase"] = json!("completed");
    if let Some(patch) = patch { done["patch"] = patch; }
    vec![("tool.started", p), ("tool.completed", done)]
}

fn new_file(path: &str) -> Value {
    json!({"text": format!("diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1 @@\n+x\n"), "truncated": false, "added": 1, "removed": 0})
}

fn report_turn(finished: bool) -> Vec<ledger::Item> {
    let mut rows: Vec<(&str, Value)> = vec![
        ("run.started", json!({})),
        ("message.delta", json!({"itemId": "m1", "operation": "append", "text": "Writing the report.", "final": true})),
    ];
    rows.extend(tool("w1", "file_change", "Write", "report.md", None, Some(new_file("report.md"))));
    if finished {
        rows.push(("message.delta", json!({"itemId": "m2", "operation": "append", "text": "Done.", "final": true})));
        rows.push(("run.completed", json!({"process": {"exitCode": 0}})));
        for (i, path) in ["file-history/abc@v2", "backups/.claude.json.backup.1"].iter().enumerate() {
            let id: &'static str = if i == 0 { "obs0" } else { "obs1" };
            rows.extend(tool(id, "observed_edit", "edit", path, Some("observed on disk"), Some(new_file(path))));
        }
    }
    events(&rows)
}

/// The thread drawn for a conversation in `folder`, its texts and keys.
fn drawn(items: Vec<ledger::Item>, folder: &str, running: bool) -> (String, Vec<String>) {
    let mut s = State::default();
    s.teammates.push(crate::Teammate { id: "tm".into(), name: "Codec".into(), role: "Custom".into(), model: None, runtime: Some("claude".into()), folder: Some(folder.into()), runs_in: "ask".into(), account: None, busy: running });
    s.conversations.insert("tm".into(), Default::default());
    if running {
        // turn_running: a teammate being opened counts as the turn going.
        s.opening_teammate = Some("tm".into());
    }
    let mut rt = rune::Runtime::new((s, items), |(s, items), _| message_column(items, "Codec", &Palette::new(true), s), (800.0, 2000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let all = rt.elements();
    (all.iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n"), all.iter().filter_map(|e| e.key.clone()).collect())
}

#[test]
fn draws_the_files_card_above_the_totals_with_electrons_heading() {
    let (said, keys) = drawn(report_turn(true), "C:/work", false);
    assert!(said.contains("Edited 1 file \u{b7} 2 more changed in the folder"), "{said}");
    assert!(!said.contains("Edited 3 files"), "{said}");
    assert!(keys.iter().any(|k| k.ends_with("/turn-foot")), "{keys:?}");
    // Open by default: its rows are drawn.
    assert!(said.contains("report.md"), "{said}");
    assert!(said.contains("backups/.claude.json.backup.1"), "{said}");
    // The totals under it do not say how many files again (TurnFoot drops the trace's files segment).
    assert!(!said.lines().any(|l| l == "3 files" || l == "1 file"), "{said}");
}

#[test]
fn draws_no_foot_while_the_turn_is_still_going() {
    let (said, keys) = drawn(report_turn(false), "C:/work", true);
    assert!(!keys.iter().any(|k| k.ends_with("/turn-foot")), "{said}");
    assert!(!said.contains("Edited 1 file"), "{said}");
    // The control: the same turn, over, wears it.
    let (_, keys) = drawn(report_turn(false), "C:/work", false);
    assert!(keys.iter().any(|k| k.ends_with("/turn-foot")), "{keys:?}");
}

#[test]
fn counts_the_folders_change_in_a_resumed_compare_column_not_the_writes_outside_it() {
    let scratch = "C:/Users/me/scratch/ember";
    let mut rows: Vec<(&str, Value)> = vec![("run.started", json!({}))];
    for (i, name) in ["cdp.mjs", "play.mjs"].iter().enumerate() {
        let id: &'static str = if i == 0 { "w0" } else { "w1" };
        rows.extend(tool(id, "tool_use", "Write", &format!("{scratch}/{name}"), None, None));
    }
    rows.push(("run.completed", json!({"process": {"exitCode": 0}})));
    rows.extend(tool("obs", "observed_edit", "edit", "index.html", Some("observed on disk"), Some(new_file("index.html"))));
    let (said, _) = drawn(events(&rows), "C:/Users/me/.locust/compare/cmp_demo-a", false);
    assert!(said.contains("1 file changed in the folder while it ran"), "{said}");
    assert!(!said.contains("did not report"), "{said}");
    assert!(!said.contains("cdp.mjs \u{b7} change not reported"), "the scratch writes are not this turn's files: {said}");
}

#[test]
fn keeps_a_normal_finished_turns_card_one_write_with_a_diff_is_still_one_edited_file() {
    let mut rows: Vec<(&str, Value)> = vec![("run.started", json!({}))];
    rows.extend(tool("w", "file_change", "Write", "report.md", None, Some(new_file("report.md"))));
    rows.push(("run.completed", json!({"process": {"exitCode": 0}})));
    let (said, _) = drawn(events(&rows), "C:/work", false);
    assert!(said.contains("Edited 1 file"), "{said}");
    assert!(said.contains("report.md"), "{said}");
    assert!(!said.contains("did not report"), "{said}");
    assert!(!said.contains("not confirmed"), "{said}");
}

#[test]
fn says_a_write_the_turn_stopped_before_settling_was_stopped_not_that_claude_code_failed_to_report() {
    let rows: Vec<(&str, Value)> = vec![
        ("run.started", json!({})),
        ("tool.started", json!({"itemId": "w", "toolKind": "tool_use", "name": "Write", "command": "notes.md", "phase": "started"})),
        ("run.cancelled", json!({})),
    ];
    let (said, _) = drawn(events(&rows), "C:/work", false);
    assert!(said.contains("not confirmed"), "{said}");
    assert!(!said.contains("Claude Code did not report"), "{said}");
    assert!(!said.lines().any(|l| l.trim() == "did not report"), "{said}");
}

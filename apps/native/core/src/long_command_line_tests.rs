//! An Antigravity run_command whose command is a long heredoc (`python3 - <<'PY'` and many lines; the overnight ledger's
//! tool_14) reads as Electron's steps line reads it: "ran" and the command's first line, the command never capitalised.
//! Follows missionView.ts:2484-2500 (`stepsLine`, "command": a title only when the runtime sent one, else
//! `ran ${clip(firstLine(name), 40)}`) and missionView.ts:3393 (a detail's `title` is only ever `payload.title`).
//! Before, the ledger's 240-character one-line summary of the command no longer equalled the whole command, was taken
//! for a model's title, and the row said "Python3 - <<'PY' import pathlib, ..." on one line past its column.
use crate::ledger::Transcript;
use crate::steps_line::steps_line;
use serde_json::{Value, json};

const HEREDOC: &str = "python3 - <<'PY'\nimport pathlib, subprocess\n\nroot = pathlib.Path(\"/Users/someone/work/lane/apps/native\")\nentries = pathlib.Path(\"/Users/someone/work/scratch/evidence/lane/mine.entries\").read_text().splitlines()\nprint(f\"count={len(entries)}\")\nfor i, line in enumerate(entries, 1):\n    assert line.strip() and not line.lstrip().startswith(\"#\"), f\"bad line {i}: {line!r}\"\n    path, rest = line.split(\"|||\", 1)\n    old, new = rest.split(\" ==> \", 1)\n    target = root / path\nPY";

fn event(seq: u64, kind: &str, payload: Value) -> Value {
    json!({"recordType": "mission.event", "event": {"id": format!("run:antigravity:{seq}"), "runId": "run", "sequence": seq,
        "occurredAt": format!("2026-11-28T03:34:2{seq}.000Z"), "sourceAdapter": "antigravity", "type": kind, "payload": payload}})
}

fn heredoc_turn() -> Vec<crate::ledger::Item> {
    let mut t = Transcript::default();
    t.apply(vec![
        event(1, "tool.started", json!({"itemId": "tool_14", "toolKind": "run_command", "name": "run_command", "phase": "started", "command": HEREDOC})),
        event(2, "tool.completed", json!({"itemId": "tool_14", "toolKind": "run_command", "name": "run_command", "phase": "completed", "command": HEREDOC, "output": "count=10\n", "durationMs": 1038})),
    ]);
    // The turn's own trace line is not a step.
    t.items.into_iter().filter(|i| matches!(i, crate::ledger::Item::Tool { .. })).collect()
}

#[test]
fn a_long_heredoc_command_reads_ran_and_its_first_line_never_capitalised() {
    let items = heredoc_turn();
    assert_eq!(items.len(), 1, "{items:?}");
    let line = steps_line(&items, true, None, false);
    assert_eq!(line.segments[0].text, "Ran python3 - <<'PY'");
    assert!(!line.title.contains("import pathlib"), "the hover is the first line too, not the script: {}", line.title);
}

#[test]
fn a_title_the_runtime_sent_is_still_the_line() {
    let mut t = Transcript::default();
    t.apply(vec![event(1, "tool.completed", json!({"itemId": "tool_1", "toolKind": "run_command", "name": "run_command",
        "phase": "completed", "command": HEREDOC, "title": "Check the break entries"}))]);
    let items: Vec<_> = t.items.into_iter().filter(|i| matches!(i, crate::ledger::Item::Tool { .. })).collect();
    assert_eq!(steps_line(&items, true, None, false).segments[0].text, "Check the break entries");
}

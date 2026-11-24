//! Electron's `a-command-says-how-long-it-took.test.ts` through the thread: Electron's events read by the ledger reader
//! (`Transcript::apply`), each command row drawn by `activity_item`, and the time it took read off the row as drawn.
//! Expected words are Electron's, written out.
use super::*;
use serde_json::{Value, json};

fn bash(id: &str, command: &str, extra: Value) -> Value {
    let mut payload = json!({"itemId": id, "toolKind": "tool_use", "name": "Bash", "command": command});
    if let (Some(into), Some(more)) = (payload.as_object_mut(), extra.as_object()) {
        into.extend(more.clone());
    }
    payload
}

fn with(mut payload: Value, more: Value) -> Value {
    if let (Some(into), Some(more)) = (payload.as_object_mut(), more.as_object()) {
        into.extend(more.clone());
    }
    payload
}

/// Electron's run: (type, payload, seconds in, sequence).
fn electrons_run() -> Vec<(&'static str, Value, u32, u64)> {
    vec![
        ("run.started", json!({}), 0, 1),
        // A test run: 12 seconds.
        ("tool.started", with(bash("a", "pnpm test", json!({})), json!({"phase": "started"})), 1, 2),
        ("tool.completed", with(bash("a", "pnpm test", json!({})), json!({"phase": "completed", "exitCode": 0})), 13, 3),
        // Reported start and end together: no time worth claiming.
        ("tool.started", with(bash("b", "ls", json!({})), json!({"phase": "started"})), 14, 4),
        ("tool.completed", with(bash("b", "ls", json!({})), json!({"phase": "completed", "exitCode": 0})), 14, 5),
        // Sent to the background: its "end" is when it was handed off, not when it finished.
        ("tool.started", with(bash("c", "npm run dev", json!({"background": true})), json!({"phase": "started"})), 15, 6),
        ("tool.completed", with(bash("c", "npm run dev", json!({"background": true})), json!({"phase": "completed"})), 18, 7),
        // Refused before it ran.
        ("tool.started", with(bash("d", "rm -rf build", json!({})), json!({"phase": "started"})), 19, 8),
        ("tool.failed", with(bash("d", "rm -rf build", json!({})), json!({"phase": "completed", "status": "refused", "output": "Not allowed"})), 21, 9),
        // Timed by the runtime itself.
        ("tool.started", with(bash("f", "npm test", json!({})), json!({"phase": "started"})), 20, 12),
        ("tool.completed", with(bash("f", "npm test", json!({})), json!({"phase": "completed", "exitCode": 0, "durationMs": 4200})), 20, 13),
        // A long one, in minutes.
        ("tool.started", with(bash("e", "cargo build", json!({})), json!({"phase": "started"})), 22, 10),
        ("tool.completed", with(bash("e", "cargo build", json!({})), json!({"phase": "completed", "exitCode": 0})), 22 + 95, 11),
        ("run.completed", json!({}), 120, 14),
    ]
}

/// The time a drawn command row says it took: its row's last part, when that part is a time.
fn time_on(label: &str) -> Option<String> {
    let last = label.rsplit(" · ").next()?;
    regex::Regex::new(r"^\d+(s|m \d{2}s|h \d{2}m)$").unwrap().is_match(last).then(|| last.to_owned())
}

/// Each command row of Electron's run, drawn: (the command, the time its row says).
fn commands_took() -> Vec<(String, Option<String>)> {
    let mut t = ledger::Transcript::default();
    t.apply(
        electrons_run()
            .into_iter()
            .map(|(kind, payload, seconds, sequence)| {
                let at = chrono::Utc.with_ymd_and_hms(2026, 9, 29, 12, 0, 0).unwrap() + chrono::Duration::seconds(i64::from(seconds));
                json!({"recordType": "mission.event", "event": {"type": kind, "sequence": sequence, "sourceAdapter": "claude",
                    "occurredAt": at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true), "payload": payload}})
            })
            .collect(),
    );
    t.items
        .into_iter()
        .filter_map(|item| {
            let ledger::Item::Tool { detail, .. } = &item else { return None };
            let (command, key) = (detail.arguments.clone(), format!("conv/{}", detail.id));
            let mut rt = rune::Runtime::new((State::default(), item), |(s, item), _| activity_item(item, &Palette::new(true), s, "conv"), (900.0, 300.0), rune::Env::new(true, 1.0));
            rt.frame(0.0);
            let label = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key.as_str()))?.label?;
            Some((command, time_on(&label)))
        })
        .collect()
}

use chrono::TimeZone;

// case: says how long it ran, from its start to its end
#[test]
fn a_finished_command_says_how_long_it_ran() {
    let took = commands_took();
    assert!(took.contains(&("pnpm test".into(), Some("12s".into()))), "{took:?}");
    assert!(took.contains(&("cargo build".into(), Some("1m 35s".into()))), "{took:?}");
    // The runtime's own clock wins over when the events arrived.
    assert!(took.contains(&("npm test".into(), Some("4s".into()))), "{took:?}");
}

// case: says nothing when there is nothing true to say: instant, backgrounded, or refused
#[test]
fn an_instant_backgrounded_or_refused_command_says_no_time() {
    let took = commands_took();
    for command in ["ls", "npm run dev", "rm -rf build"] {
        assert!(took.contains(&(command.into(), None)), "{command}: {took:?}");
    }
}

// case: says nothing while it is still running
#[test]
fn a_command_still_running_says_no_time() {
    let item = ledger::Item::Tool {
        name: "Bash".into(),
        summary: String::new(),
        detail: Box::new(ledger::ToolDetail { id: "c1".into(), arguments: "pnpm test".into(), duration_ms: Some(5000), ..Default::default() }),
        status: ledger::Status::Running,
    };
    let mut rt = rune::Runtime::new((State::default(), item), |(s, item), _| activity_item(item, &Palette::new(true), s, "conv"), (900.0, 300.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let label = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("conv/c1")).and_then(|e| e.label).expect("the row");
    assert!(label.contains("running"), "{label}");
    assert_eq!(time_on(&label), None, "{label}");
}

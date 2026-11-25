//! Electron's `a-provider-error-is-readable-and-said-once.test.tsx`, through the window: a failed turn recorded the way
//! a runtime records it (`ledger::Session` reading the mission file, Electron's `buildThread`), drawn by the thread.
use super::*;
use crate::tests::TempDir;
use locust_core::failure_words::failure_message;
use locust_core::ledger::Item;
use locust_core::provider_error::provider_error_sentence;
use locust_core::thread_view::error_already_shown;
use rune::{Env, Runtime};
use serde_json::{Value, json};

const MODEL_ERROR: &str = r#"{"type":"error","error":{"message":"model 'gpt-6.1-sol' is not enabled in rustponsesapi","type":"invalid_request_error","param":null,"code":null},"status":400}"#;
const EXAMPLES: [(&str, &str); 5] = [
    (MODEL_ERROR, "OpenAI refused the model gpt-6.1-sol: it is not enabled here right now. Pick another model and send again."),
    (r#"{"error":{"message":"Too many requests"},"status":429}"#, "OpenAI rate limited this request. Wait a while, or pick another model and send again."),
    (r#"{"message":"Service unavailable","status":503}"#, "OpenAI is overloaded or unavailable right now. Try again later, or pick another model."),
    (r#"{"error":{"message":"Unauthorized"},"status":401}"#, "OpenAI refused this request because of sign-in or access permissions. Check the agent's sign-in and model access, then send again."),
    ("The connection closed before the answer finished.", "The connection closed before the answer finished."),
];
const AT: &str = "2026-10-05T12:00:00.000Z";

/// Electron's `recorded`: the runtime's error diagnostic, then the run failing on the same message.
fn recorded(message: &str, runtime: &str) -> Vec<Value> {
    vec![
        json!({"id": "error", "runId": "run-error", "sequence": 1, "occurredAt": AT, "sourceAdapter": runtime, "type": "adapter.diagnostic",
            "payload": {"level": "error", "code": format!("{runtime}.runtime_error"), "message": message, "terminal": false, "evidence": {"raw": message, "redacted": false}}}),
        json!({"id": "failed", "runId": "run-error", "sequence": 2, "occurredAt": "2026-10-05T12:00:01.000Z", "sourceAdapter": runtime, "type": "run.failed",
            "payload": {"kind": "process-failed", "message": message, "runtimeTerminal": "failed", "process": {"exitCode": 1, "signal": null, "stderr": "", "stderrTruncated": false}}}),
    ]
}

/// The mission file holding `events`, read as the window reads a conversation.
fn read(events: &[Value], runtime: &str) -> (TempDir, ledger::Session, ledger::Snapshot) {
    let dir = TempDir::new();
    let created = json!({"schemaVersion": 24, "recordType": "mission.created", "occurredAt": AT,
        "metadata": {"missionId": "m_err", "prompt": "Summarize the turns", "runtime": runtime, "model": "gpt-6.1-sol", "createdAt": AT}});
    let mut lines = vec![created.to_string()];
    for event in events {
        lines.push(json!({"schemaVersion": 24, "recordType": "mission.event", "occurredAt": event["occurredAt"], "event": event}).to_string());
    }
    std::fs::write(dir.0.join("m_err.jsonl"), lines.join("\n") + "\n").unwrap();
    let mut worker = ledger::Session::new(dir.0.clone(), "m_err".into());
    let snapshot = worker.poll().unwrap();
    (dir, worker, snapshot)
}

fn errors(items: &[Item]) -> Vec<String> {
    items.iter().filter_map(|i| match i { Item::Note { level, text } if level == "error" => Some(text.clone()), _ => None }).collect()
}

/// Every text the thread draws for the failed turn.
fn drawn(events: &[Value]) -> Vec<String> {
    let (dir, worker, snapshot) = read(events, "codex");
    let mut s = State::default();
    team_scene(&mut s);
    let header = s.teammates[0].clone();
    s.ledger = Some(LedgerPane {
        header, conversation: Conversation { conversation_id: Some("m_err".into()), turns: vec![] },
        items: snapshot.items.clone(), dir: dir.0.clone(), worker: Some(worker), snapshot, cold_tip: None,
        due: f64::INFINITY, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
    });
    fn view(s: &State, _cx: &mut Cx<State>) -> El {
        crate::thread_view(s, &Palette::new(true))
    }
    let mut rt = Runtime::new(s, view, (1000.0, 1400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let words = rt.elements().into_iter().filter_map(|e| e.text).collect();
    drop(dir);
    words
}

fn said_once(words: &[String], sentence: &str) {
    assert_eq!(words.iter().map(|w| w.matches(sentence).count()).sum::<usize>(), 1, "{sentence} in {words:?}");
    assert!(!words.iter().any(|w| w.contains("rustponsesapi") || w.contains("The run could not continue")), "{words:?}");
}

#[test]
fn shows_the_recorded_error_once_in_a_failed_turn() {
    // case: shows the recorded error %s once in a failed turn
    for (raw, expected) in EXAMPLES {
        let events = recorded(raw, "codex");
        let before = serde_json::to_string(&events).unwrap();
        let (_dir, _worker, snapshot) = read(&events, "codex");
        assert_eq!(errors(&snapshot.items), [expected]);
        said_once(&drawn(&events), expected);
        assert_eq!(serde_json::to_string(&events).unwrap(), before);
    }
}

#[test]
fn keeps_a_failed_turn_readable_when_its_diagnostic_was_never_recorded() {
    let events = recorded(MODEL_ERROR, "codex")[1..].to_vec();
    said_once(&drawn(&events), EXAMPLES[0].1);
}

#[test]
fn does_not_repeat_a_final_rate_limit_failure_as_a_temporary_warning() {
    let raw = EXAMPLES[1].0;
    let mut events = recorded(raw, "codex");
    events.insert(0, json!({"id": "limit", "sequence": 0, "runId": "run-error", "occurredAt": AT, "sourceAdapter": "codex",
        "type": "route.limit_detected", "payload": {"kind": "temporary-rate-limit", "message": raw, "evidence": {"raw": raw, "redacted": false}}}));
    said_once(&drawn(&events), EXAMPLES[1].1);
    let (_dir, _worker, snapshot) = read(&events, "codex");
    assert!(!snapshot.items.iter().any(|i| matches!(i, Item::Limit { .. })), "{:?}", snapshot.items);
}

#[test]
fn formats_restored_errors_for_each_runtime_without_inventing_its_upstream_provider() {
    for runtime in ["claude", "cursor", "antigravity", "copilot", "opencode"] {
        let raw = EXAMPLES[1].0;
        let expected = provider_error_sentence(raw, Some(runtime));
        let (_dir, _worker, snapshot) = read(&recorded(raw, runtime), runtime);
        assert_eq!(errors(&snapshot.items), [expected.clone()], "{runtime}");
        assert!(error_already_shown(&snapshot.items, &expected));
        assert_eq!(failure_message(raw, None, Some(runtime)), expected);
    }
}

#[test]
fn keeps_a_distinct_failure_card_and_does_not_let_a_warning_hide_the_failure() {
    assert!(!error_already_shown(&[Item::Note { level: "warning".into(), text: "The process died".into() }], "The process died"));
    assert!(!error_already_shown(&[Item::Note { level: "error".into(), text: "Another failure".into() }], "The process died"));
}

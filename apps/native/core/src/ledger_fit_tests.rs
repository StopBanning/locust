//! packages/mission-store/test/a-long-field-is-kept-shortened.test.ts (4 cases) and
//! apps/desktop/src/main/an-app-reads-a-long-answer-whole.test.ts (2 cases). Fixed 20 Nov: Rust wrote every field
//! as it came, so a runtime field past 512 KiB could not be written at all, and nothing kept a record bounded.
use super::*;
use crate::ledger_write::LedgerWriter;
use serde_json::json;

const LONG_LEN: usize = 20_000;

fn long() -> String {
    "w".repeat(LONG_LEN)
}

struct Scratch(std::path::PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn writer(id: &str) -> (Scratch, LedgerWriter) {
    let root = std::env::temp_dir().join(format!("locust-fit-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    let store = crate::store::Store::with_codex(root.join("native"), root.join("electron"), vec![]).unwrap();
    let w = LedgerWriter::create_runtime(&store, id, "Say something long.", "account-default", "codex", "codex", &[], None).unwrap();
    (Scratch(root), w)
}

/// The events as a fresh reader finds them on disk.
fn events_on_disk(w: &LedgerWriter) -> Vec<Value> {
    std::fs::read_to_string(&w.path)
        .unwrap()
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .filter(|r| r["recordType"] == "mission.event")
        .map(|r| r["event"].clone())
        .collect()
}

fn evidence(raw: &Value) -> Value {
    json!({"transportSequence": 1, "runtimeEventType": long(), "redacted": false, "raw": raw})
}

fn long_payloads() -> Vec<(&'static str, Value)> {
    let l = long();
    let mut raw = json!({"text": l, "list": (0..150).map(|_| "w".repeat(10)).collect::<Vec<_>>(),
        "deep": {"a": {"b": {"c": {"d": {"e": {"f": {"g": {"h": {"i": "x"}}}}}}}}}});
    raw[l.as_str()] = json!(1);
    let process = json!({"exitCode": 0, "signal": null, "stderr": "", "stderrTruncated": false, "recordCount": 1});
    let mut stderr_heavy = process.clone();
    stderr_heavy["stderr"] = json!("e".repeat(600_000));
    let mut usage = json!({"note": l});
    usage[l.as_str()] = json!(1);
    vec![
        ("run.started", json!({"runtimeThreadId": l, "evidence": evidence(&raw)})),
        ("plan.updated", json!({"itemId": l, "plan": [{"content": l, "status": "pending"}], "final": false, "evidence": evidence(&raw)})),
        ("message.delta", json!({"itemId": l, "operation": "append", "text": l, "final": false, "evidence": evidence(&raw)})),
        ("step.started", json!({"stepKind": "reasoning", "itemId": l, "itemType": l, "status": l, "message": l, "evidence": evidence(&raw)})),
        ("step.completed", json!({"stepKind": "item", "itemId": l, "message": l, "evidence": evidence(&raw)})),
        ("tool.started", json!({"itemId": l, "toolKind": l, "name": l, "command": l, "title": l, "parentItemId": l, "phase": "started", "evidence": evidence(&raw)})),
        ("tool.completed", json!({"itemId": l, "toolKind": "shell", "name": "Bash", "output": {"stdout": l, "rows": (0..30).map(|_| l.clone()).collect::<Vec<_>>()}, "status": l, "exitCode": 0, "phase": "completed", "evidence": evidence(&raw)})),
        ("route.limit_detected", json!({"kind": "quota-exhausted", "message": l, "evidence": evidence(&raw)})),
        ("adapter.diagnostic", json!({"level": "warning", "code": l, "message": l, "terminal": false, "evidence": evidence(&raw)})),
        ("run.completed", json!({"runtimeThreadId": l, "usage": usage, "process": stderr_heavy})),
        ("run.failed", json!({"kind": "unknown", "message": l, "runtimeThreadId": l, "runtimeTerminal": "failed", "process": process})),
    ]
}

fn len16(s: &str) -> usize {
    s.encode_utf16().count()
}

// case: is kept shortened in every event type, and read back
#[test]
fn a_long_field_is_kept_shortened_in_every_event_type_and_read_back() {
    let (_s, mut w) = writer("mission_fit");
    let payloads = long_payloads();
    assert_eq!(payloads.len(), 11, "a walk over nothing would pass");
    for (kind, payload) in &payloads {
        w.append(kind, payload.clone(), Some(&long())).unwrap();
    }
    let read = events_on_disk(&w);
    assert_eq!(read.iter().map(|e| e["type"].as_str().unwrap()).collect::<Vec<_>>(), payloads.iter().map(|(k, _)| *k).collect::<Vec<_>>());
    let message = read.iter().find(|e| e["type"] == "message.delta").unwrap();
    assert_eq!(message["payload"]["text"].as_str().map(str::len), Some(LONG_LEN), "an answer this long is kept whole");
    let said = read.iter().find(|e| e["type"] == "adapter.diagnostic").unwrap()["payload"]["message"].as_str().unwrap().to_owned();
    assert!(len16(&said) <= LEDGER_TEXT_LIMIT && said.starts_with("www") && said.ends_with("www") && said.contains("[shortened]"));
    let started = read.iter().find(|e| e["type"] == "run.started").unwrap();
    assert!(len16(started["runtimeThreadId"].as_str().unwrap()) <= LEDGER_THREAD_LIMIT);
    assert!(len16(started["payload"]["evidence"]["runtimeEventType"].as_str().unwrap()) <= LEDGER_NAME_LIMIT);
    assert_eq!(started["payload"]["evidence"]["redacted"], true, "raw evidence that was fitted says so");
    let tool = read.iter().find(|e| e["type"] == "tool.started").unwrap();
    assert!(len16(tool["payload"]["itemId"].as_str().unwrap()) <= LEDGER_NAME_LIMIT);
    let completed = read.iter().find(|e| e["type"] == "run.completed").unwrap();
    assert!(serde_json::to_string(completed).unwrap().len() <= 480 * 1024, "the 600 KB stderr no longer stops the write");
}

// case: keeps a long answer whole up to its own limit, and shortens one past it (0.869)
#[test]
fn a_long_answer_is_whole_up_to_its_limit_and_shortened_past_it() {
    let (_s, mut w) = writer("mission_answer");
    let report = (0..1_000).map(|i| format!("## Part {i}\n{}", "word ".repeat(16))).collect::<Vec<_>>().join("\n");
    assert!(report.len() > 80_000);
    let too_long = format!("START{}END", "x".repeat(LEDGER_MESSAGE_TEXT_LIMIT));
    let wide = "\u{0001}".repeat(LEDGER_MESSAGE_TEXT_LIMIT - 10);
    for (item, text) in [("a1", &report), ("a2", &too_long), ("a3", &wide)] {
        w.append("message.delta", json!({"itemId": item, "operation": "replace", "text": text, "final": true, "evidence": {"redacted": false}}), None).unwrap();
    }
    let texts: Vec<String> = events_on_disk(&w).iter().map(|e| e["payload"]["text"].as_str().unwrap().to_owned()).collect();
    assert_eq!(texts[0], report);
    assert!(len16(&texts[1]) <= LEDGER_MESSAGE_TEXT_LIMIT && texts[1].starts_with("START") && texts[1].ends_with("END"));
    assert!(texts[1].contains("[shortened]"));
    assert!(len16(&texts[2]) < len16(&wide) && texts[2].contains("[shortened]"), "past a record's bytes, it is shortened too");
}

// case: costs nothing when it already fits
#[test]
fn an_event_that_fits_comes_back_unchanged() {
    let event = json!({"id": "r:acp:1", "runId": "r", "missionId": "m", "sequence": 1, "occurredAt": "2026-10-15T20:00:00.000Z",
        "sourceAdapter": "acp", "type": "message.delta",
        "payload": {"itemId": "m1", "operation": "append", "text": "hello", "final": false, "evidence": {"redacted": false, "raw": {"a": [1, "two"]}}}});
    assert_eq!(fit_event(event.clone()), event);
}

#[test]
fn json_is_bounded_in_entries_and_depth_and_a_nul_is_never_written() {
    let many = Value::Array((0..150).map(|i| json!(i)).collect());
    assert_eq!(fit_json(&many, 0).as_array().unwrap().len(), 100);
    let deep = json!({"a": {"b": {"c": {"d": {"e": {"f": {"g": {"h": {"i": "x"}}}}}}}}});
    assert!(serde_json::to_string(&fit_json(&deep, 0)).unwrap().contains("[nested deeper than the record keeps]"));
    assert_eq!(fit_text("a\0b", 10), "a\u{FFFD}b");
    assert_eq!(fit_text("abcdef", 4), "abc…", "a small limit keeps the head and an ellipsis");
}

// case: gets a 60,000 character report back whole, every section in order
#[test]
fn an_app_gets_a_60000_character_report_back_whole() {
    let sections = ["README and GitHub", "Move the exhaustive key table into a manual", "Content", "Promo and growth", "Workflow", "Wild ideas worth one try"];
    let report = sections.iter().map(|t| format!("## {t}\n{}", "A sentence about this part of the plan. ".repeat(250))).collect::<Vec<_>>().join("\n\n");
    let report = report.trim().to_owned();
    assert!(report.len() > 60_000);
    let (_s, mut w) = writer("mission_long");
    w.append("message.delta", json!({"itemId": "item_1", "operation": "replace", "text": report, "final": true, "evidence": {"redacted": false}}), None).unwrap();
    assert_eq!(events_on_disk(&w)[0]["payload"]["text"].as_str(), Some(report.as_str()));
}

// case: still keeps both ends of an answer past the limit, and says where it was shortened
#[test]
fn an_app_still_gets_both_ends_of_an_answer_past_the_limit() {
    let huge = format!("START {} END", "x".repeat(300_000));
    let (_s, mut w) = writer("mission_huge");
    w.append("message.delta", json!({"itemId": "item_1", "operation": "replace", "text": huge, "final": true, "evidence": {"redacted": false}}), None).unwrap();
    let back = events_on_disk(&w)[0]["payload"]["text"].as_str().unwrap().to_owned();
    assert!(back.len() < huge.len() && back.starts_with("START ") && back.ends_with(" END") && back.contains("[shortened]"));
}

#[test]
fn raw_evidence_still_too_heavy_after_fitting_is_dropped_and_said_to_be() {
    // 100 entries of 16 KiB each fit every per-field limit and still weigh 1.6 MB.
    let raw = Value::Array((0..100).map(|_| json!("r".repeat(LEDGER_TEXT_LIMIT))).collect());
    let (_s, mut w) = writer("mission_heavy");
    w.append("tool.completed", json!({"itemId": "t1", "toolKind": "shell", "name": "Bash", "status": "completed", "evidence": {"redacted": false, "raw": raw}}), None).unwrap();
    let back = &events_on_disk(&w)[0];
    assert!(back["payload"]["evidence"].get("raw").is_none(), "the raw evidence is dropped");
    assert_eq!(back["payload"]["evidence"]["redacted"], true);
    assert_eq!(back["payload"]["name"], "Bash", "the rest of the event is kept");
    assert!(serde_json::to_string(back).unwrap().len() <= 480 * 1024);
}

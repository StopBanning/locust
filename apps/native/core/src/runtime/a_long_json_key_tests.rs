//! Electron's `a-long-json-key-does-not-stop-a-run.test.ts`. Electron's ledger refuses a JSON key over 512 characters,
//! and its Codex adapter kept the record's keys of any length as evidence, so a tool whose input carried one long key
//! made the write throw and stopped the run.
//!
//! Electron feeds `codex exec` JSONL (`item.started`, `mcp_tool_call`); Rust's Codex runtime reads the app-server
//! protocol, so the same call is written here as app-server's `item/started` with an `mcpToolCall` item. Rust's
//! writer neither refuses a long key nor cuts one (its reader cuts keys over 512 when it reads), so "append succeeds"
//! alone could never fail: the test checks that no key over 512 UTF-16 units reaches the file. On this path the Codex
//! normalizer keeps no part of the record as evidence, so the long key never enters the ledger at all.
use super::*;
use crate::{ledger_write::LedgerWriter, store::Store};

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn writer() -> (Temp, LedgerWriter) {
    let root = std::env::temp_dir().join(format!("locust-longkey-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("native")).unwrap();
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    let w = LedgerWriter::create_runtime(&store, "mission_k", "p", "account-default", "codex", "codex", &[], None).unwrap();
    (Temp(root), w)
}

/// The longest key anywhere in a JSON value, in UTF-16 units.
fn longest_key(value: &Value) -> usize {
    match value {
        Value::Object(map) => map.iter().map(|(k, v)| k.encode_utf16().count().max(longest_key(v))).max().unwrap_or(0),
        Value::Array(items) => items.iter().map(longest_key).max().unwrap_or(0),
        _ => 0,
    }
}

fn written(w: &LedgerWriter) -> Vec<Value> {
    std::fs::read_to_string(&w.path).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect()
}

// case: is recorded, not a reason to stop the run
#[test]
fn a_tool_input_with_a_very_long_key_is_recorded_not_a_reason_to_stop_the_run() {
    let (_t, mut w) = writer();
    let long_key = "k".repeat(600);
    let mut codex = Normalizer::default();
    let mut events = Vec::new();
    events.extend(codex.accept("thread/started", &json!({"thread": {"id": "t"}})));
    events.extend(codex.accept(
        "item/started",
        &json!({"threadId": "t", "turnId": "turn_1", "item": {"id": "mcp_1", "type": "mcpToolCall", "server": "notes", "tool": "save", "arguments": {long_key.clone(): "v"}, "status": "inProgress"}}),
    ));
    // The control: the call really came out as a tool start.
    assert!(events.iter().any(|e| e.kind == "tool.started" && e.payload["name"] == "notes.save"), "{:?}", events.iter().map(|e| &e.kind).collect::<Vec<_>>());
    for event in &events {
        w.append(&event.kind, event.payload.clone(), event.thread.as_deref()).unwrap();
    }
    let records = written(&w);
    assert_eq!(records.iter().filter(|r| r["recordType"] == "mission.event").count(), events.len());
    for record in &records {
        assert!(longest_key(record) <= 512, "a key of {} units reached the ledger", longest_key(record));
    }
    let back = crate::ledger::read_transcript(&w.path);
    assert_eq!(back.error, None);
    assert!(!back.failed, "the run is not stopped");
}

/// The readers that do keep a record as evidence (Claude Code, Cursor, OpenCode, and others) keep it through
/// `evidence_raw`, which cuts a key over 512 to fit and says it was redacted (codex-events.ts sanitizeJson).
#[test]
fn a_record_kept_as_evidence_has_its_long_key_cut_to_fit_the_ledger() {
    let (_t, mut w) = writer();
    let long_key = "k".repeat(600);
    let (raw, redacted) = evidence_raw(&json!({"type": "item.started", "item": {"id": "mcp_1", "arguments": {long_key.clone(): "v"}}}));
    assert!(redacted);
    let args = raw["item"]["arguments"].as_object().unwrap();
    let (key, value) = args.iter().next().unwrap();
    assert!(key.encode_utf16().count() <= 512 && key.ends_with("[truncated]") && key.starts_with("kkkk"), "{key}");
    assert_eq!(value, "v");
    w.append("tool.started", json!({"itemId": "mcp_1", "toolKind": "mcpToolCall", "name": "notes.save", "phase": "started", "evidence": {"runtimeEventType": "item.started", "raw": raw, "redacted": redacted}}), None).unwrap();
    assert!(written(&w).iter().all(|r| longest_key(r) <= 512));
}

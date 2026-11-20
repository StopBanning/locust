//! reasoning-is-kept.test.ts (8 cases): reasoning in the fold keeps the runtime's text and how long it thought, draws
//! nothing for a blink or bare whitespace, reads Codex's reasoning item the same way; and it is not a tool call: it is
//! not counted among them and keeps its own row (`tool_fold::fold_plain_tool_runs`).
use crate::activity_summary::activity_summary;
use crate::ledger::{Item, Transcript};
use crate::tool_fold::{fold_plain_tool_runs, Row};
use serde_json::{json, Value};

fn record(sequence: usize, kind: &str, payload: Value, at: &str) -> Value {
    json!({"recordType": "mission.event", "event": {"type": kind, "sequence": sequence, "occurredAt": at, "payload": payload}})
}

const AT: &str = "2026-09-16T10:00:00.000Z";

fn thought(started: Value, completed: Value, ended: &str) -> Vec<Item> {
    let mut t = Transcript::default();
    t.apply(vec![record(1, "run.started", json!({}), AT), record(2, "step.started", started, AT), record(3, "step.completed", completed, ended)]);
    t.items.into_iter().filter(|i| matches!(i, Item::Thought { .. })).collect()
}

fn said(items: &[Item]) -> Option<(String, Option<u64>)> {
    items.iter().find_map(|i| match i {
        Item::Thought { text, duration_ms, .. } => Some((text.clone(), *duration_ms)),
        _ => None,
    })
}

// case: keeps the text the runtime sent
#[test]
fn keeps_the_text_the_runtime_sent() {
    let items = thought(json!({"stepKind": "reasoning"}), json!({"stepKind": "reasoning", "message": "Reading README.md to answer in one sentence."}), "2026-09-16T10:00:20.000Z");
    assert_eq!(said(&items).unwrap().0, "Reading README.md to answer in one sentence.");
}

// case: draws "thought for" with no text when the runtime sent none
#[test]
fn a_thought_with_no_text_keeps_its_length() {
    let items = thought(json!({"stepKind": "reasoning"}), json!({"stepKind": "reasoning"}), "2026-09-16T10:00:20.000Z");
    assert_eq!(said(&items), Some((String::new(), Some(20_000))));
}

// case: and none for a thought under a second, or whitespace that never had a length
#[test]
fn nothing_for_a_blink_or_bare_whitespace() {
    let items = thought(json!({"stepKind": "reasoning"}), json!({"stepKind": "reasoning", "message": "   "}), "2026-09-16T10:00:00.400Z");
    assert!(items.is_empty(), "{items:?}");
}

// case: draws Codex's reasoning item the same way
#[test]
fn codex_reasoning_item_reads_the_same() {
    let items = thought(
        json!({"stepKind": "item", "itemType": "reasoning", "itemId": "rs_1"}),
        json!({"stepKind": "item", "itemType": "reasoning", "itemId": "rs_1"}),
        "2026-09-16T10:00:07.000Z",
    );
    assert_eq!(said(&items).map(|s| s.1), Some(Some(7_000)));
}

// case: still reports how long it thought
#[test]
fn still_reports_how_long_it_thought() {
    let items = thought(json!({"stepKind": "reasoning"}), json!({"stepKind": "reasoning", "message": "x"}), "2026-09-16T10:00:52.000Z");
    assert_eq!(said(&items), Some(("x".into(), Some(52_000))));
}

fn mcp() -> Item {
    let mut t = Transcript::default();
    t.apply(vec![
        record(1, "tool.started", json!({"itemId": "c1", "name": "mcp", "phase": "started"}), AT),
        record(2, "tool.completed", json!({"itemId": "c1", "name": "mcp", "phase": "completed"}), AT),
    ]);
    t.items.into_iter().find(|i| matches!(i, Item::Tool { .. })).expect("a tool row")
}

fn a_thought() -> Item {
    Item::Thought { id: "r:1".into(), text: String::new(), duration_ms: Some(3_000) }
}

// case: does not count thinking among the tool calls
#[test]
fn thinking_is_not_counted_among_tool_calls() {
    let said = activity_summary(&[a_thought(), mcp(), mcp()]);
    assert!(said.contains("2 tool calls"), "{said}");
    assert!(!said.contains("3 tool calls"), "{said}");
}

// case: says nothing about tool calls when a turn only thought
#[test]
fn a_turn_that_only_thought_has_no_tool_activity() {
    assert_eq!(activity_summary(&[a_thought()]), "No tool activity");
}

// case: gives thinking its own row rather than folding it in with tools
#[test]
fn thinking_keeps_its_own_row() {
    let items = [mcp(), a_thought(), mcp()];
    let rows = fold_plain_tool_runs(&items);
    let own: Vec<&Row> = rows.iter().filter(|r| matches!(r, Row::One(i) if matches!(items[*i], Item::Thought { .. }))).collect();
    assert_eq!(own.len(), 1, "{rows:?}");
    assert!(!rows.iter().any(|r| matches!(r, Row::Folded { items: f, .. } if f.contains(&1))), "never folded in with the tools");
}

//! Electron's renderer/src/a-helpers-calls-fold-under-its-row.test.tsx, on the recorded run it uses
//! (packages/runtime-adapters/test/fixtures/claude/helper-calls.jsonl: a real Haiku 4.5 run asked to use a helper to
//! count five files; the Explore helper went to the background, made one Glob, and reported 5). The recording is read
//! from `parity/events.json`, through Rust's own Claude reader, into the thread's items.
use crate::ledger::{Item, Status, Transcript};
use serde_json::{Value, json};

fn recorded(fixture: &str) -> Vec<String> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../parity/events.json");
    let goldens: Value = serde_json::from_str(&std::fs::read_to_string(root).unwrap()).unwrap();
    let streams = goldens.as_array().cloned().or_else(|| goldens["streams"].as_array().cloned()).unwrap_or_else(|| goldens.as_object().unwrap().values().cloned().collect());
    let stream = streams.iter().find(|s| s["fixture"] == fixture).unwrap_or_else(|| panic!("{fixture} in events.json"));
    stream["lines"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_owned()).collect()
}

/// The thread's items, as the window reads them, from a Claude Code recording; `keep` picks which events go in.
fn thread(fixture: &str, keep: impl Fn(&Value) -> bool) -> Vec<Item> {
    let mut n = crate::runtime::claude_events::ClaudeNormalizer::default();
    let mut events = Vec::new();
    for line in recorded(fixture) {
        if let Ok(v) = serde_json::from_str::<Value>(&line) {
            events.extend(n.accept(&v));
        }
    }
    let records: Vec<Value> = events
        .into_iter()
        .filter(|e| keep(&e.payload))
        .enumerate()
        .map(|(i, e)| json!({"recordType": "mission.event", "event": {"sequence": i + 1, "type": e.kind, "payload": e.payload}}))
        .collect();
    let mut t = Transcript::default();
    t.apply(records);
    t.items
}

fn tools(items: &[Item]) -> Vec<&Item> {
    items.iter().filter(|i| matches!(i, Item::Tool { .. })).collect()
}

fn helper(items: &[Item]) -> &Item {
    tools(items).into_iter().find(|i| matches!(i, Item::Tool { detail, .. } if detail.helper)).expect("a helper row")
}

#[test]
fn a_helpers_calls_are_kept_under_its_row_as_the_rows_the_teammates_calls_use() {
    let items = thread("claude/helper-calls.jsonl", |_| true);
    let Item::Tool { detail, .. } = helper(&items) else { unreachable!() };
    assert_eq!(detail.helper_type, "Explore");
    let children: Vec<(String, String, Status)> = detail
        .children
        .iter()
        .filter_map(|c| match c {
            Item::Tool { name, summary, status, detail } => Some((name.clone(), crate::tool_fold::shown_name(name, summary, &detail.arguments), *status)),
            _ => None,
        })
        .collect();
    assert_eq!(children, vec![("Glob".to_owned(), "**/*".to_owned(), Status::Done)]);
}

#[test]
fn a_helpers_calls_leave_the_teammates_own_rows_as_they_read_without_them() {
    let all = thread("claude/helper-calls.jsonl", |_| true);
    let own = thread("claude/helper-calls.jsonl", |p| p.get("parentItemId").is_none());
    let without_children = |items: &[Item]| -> Vec<Item> {
        items
            .iter()
            .cloned()
            .map(|mut i| {
                if let Item::Tool { detail, .. } = &mut i {
                    detail.children.clear();
                }
                i
            })
            .collect()
    };
    assert_eq!(without_children(&all), without_children(&own));
    assert_eq!(tools(&all).len(), tools(&own).len(), "no row of the helper's appears among the teammate's own");
}

#[test]
fn are_nothing_at_all_where_no_helper_ran_no_row_gains_children() {
    let items = thread("claude/edit-and-write.jsonl", |_| true);
    assert!(!tools(&items).is_empty());
    assert!(tools(&items).iter().all(|i| matches!(i, Item::Tool { detail, .. } if detail.children.is_empty())));
}

// ---- a card a helper raised ----

const AGENT: &str = "toolu_01MrF76nyTfdvLaECwMtcA7V";
const GLOB: &str = "toolu_0134dcHNk1GH8kL57hFJ7THL";

#[test]
fn a_card_a_helper_raised_names_the_helper_and_whose_it_is() {
    let items = thread("claude/helper-calls.jsonl", |_| true);
    assert_eq!(crate::approval::helper_asked_by(Some(GLOB), &items, Some("Wren")).as_deref(), Some("the Explore helper of Wren"));
    assert_eq!(crate::approval::helper_asked_by(Some(GLOB), &items, None).as_deref(), Some("the Explore helper of this teammate"));
}

#[test]
fn says_nothing_of_a_helper_when_the_teammate_asked_or_the_call_is_not_known() {
    let items = thread("claude/helper-calls.jsonl", |_| true);
    for id in [Some(AGENT), None, Some("toolu_unknown")] {
        assert_eq!(crate::approval::helper_asked_by(id, &items, Some("Wren")), None, "{id:?}");
    }
}

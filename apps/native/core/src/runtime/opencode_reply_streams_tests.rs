//! Electron's `packages/runtime-adapters/test/an-opencode-reply-streams.test.ts`, case by case, through the caller: the
//! OpenCode server driver (opencode_serve.rs `push`) hands each record it builds -- `text_partial` while a reply grows,
//! `text` once it is finished, `tool_use`, `step_start` -- to `Normalizer::accept` with the session stamped on, as here.
use super::*;
use serde_json::{Value, json};

fn record(kind: &str, part: Value) -> Value {
    json!({"type":kind,"sessionID":"ses_1","part":part})
}

fn deltas(records: &[Value]) -> Vec<(String, String, String, bool)> {
    let mut normal = Normalizer::new(None);
    records
        .iter()
        .flat_map(|r| normal.accept(r))
        .filter(|e| e.kind == "message.delta")
        .map(|e| {
            let p = &e.payload;
            (
                p["itemId"].as_str().unwrap().to_owned(),
                p["operation"].as_str().unwrap().to_owned(),
                p["text"].as_str().unwrap().to_owned(),
                p["final"].as_bool().unwrap(),
            )
        })
        .collect()
}

fn d(item: &str, operation: &str, text: &str, last: bool) -> (String, String, String, bool) {
    (item.into(), operation.into(), text.into(), last)
}

#[test]
fn grows_one_item_as_it_is_written_then_is_replaced_by_the_finished_text_exactly() {
    // case: grows one item as it is written, then is replaced by the finished text, exactly
    let said = deltas(&[
        record("step_start", json!({"id":"s1","type":"step-start"})),
        record("text_partial", json!({"id":"p1","type":"text","text":"The keeper","time":{"start":1}})),
        record("text_partial", json!({"id":"p1","type":"text","text":"The keeper found a","time":{"start":1}})),
        record("text_partial", json!({"id":"p1","type":"text","text":"The keeper found a bottle","time":{"start":1}})),
        record("text", json!({"id":"p1","type":"text","text":"The keeper found a bottle.","time":{"start":1,"end":2}})),
    ]);
    assert_eq!(
        said,
        [
            d("msg_0", "append", "The keeper", false),
            d("msg_0", "append", " found a", false),
            d("msg_0", "append", " bottle", false),
            d("msg_0", "replace", "The keeper found a bottle.", true),
        ]
    );
}

#[test]
fn a_part_rewritten_as_it_is_written_is_replaced_not_appended_to() {
    // case: a part rewritten as it is written is replaced, not appended to
    let said = deltas(&[
        record("text_partial", json!({"id":"p1","type":"text","text":"Helo","time":{"start":1}})),
        record("text_partial", json!({"id":"p1","type":"text","text":"Hello there","time":{"start":1}})),
    ]);
    assert_eq!(said, [d("msg_0", "append", "Helo", false), d("msg_0", "replace", "Hello there", false)]);
}

#[test]
fn two_parts_are_two_items_and_a_finished_part_with_no_partials_before_it_is_one_as_through_run() {
    // case: two parts are two items, and a finished part with no partials before it is one, as through run
    let said = deltas(&[
        record("text_partial", json!({"id":"p1","type":"text","text":"First","time":{"start":1}})),
        record("text", json!({"id":"p1","type":"text","text":"First.","time":{"start":1,"end":2}})),
        record("text", json!({"id":"p2","type":"text","text":"Second.","time":{"start":3,"end":4}})),
    ]);
    assert_eq!(
        said,
        [d("msg_0", "append", "First", false), d("msg_0", "replace", "First.", true), d("msg_1", "replace", "Second.", true)]
    );
}

#[test]
fn opencodes_own_notes_never_stream() {
    // case: OpenCode's own notes never stream
    let said = deltas(&[record("text_partial", json!({"id":"p9","type":"text","synthetic":true,"text":"Continue","time":{"start":1}}))]);
    assert!(said.is_empty());
}

#[test]
fn is_closed_when_the_tool_comes_and_its_finished_form_still_replaces_it_exactly() {
    // case: is closed when the tool comes, and its finished form still replaces it exactly
    let said = deltas(&[
        record("text_partial", json!({"id":"p1","type":"text","text":"Reading README.md","time":{"start":1}})),
        record("tool_use", json!({"id":"t1","type":"tool","tool":"read","callID":"c1","state":{"status":"completed","input":{"filePath":"README.md"},"output":"x","time":{"start":2,"end":3}}})),
        record("text_partial", json!({"id":"p1","type":"text","text":"Reading README.md now","time":{"start":1}})),
        record("text", json!({"id":"p1","type":"text","text":"Reading README.md.","time":{"start":1,"end":2}})),
    ]);
    assert_eq!(
        said,
        [
            d("msg_0", "append", "Reading README.md", false),
            d("msg_0", "replace", "Reading README.md", true),
            d("msg_0", "replace", "Reading README.md.", true),
        ]
    );
}

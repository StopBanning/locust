//! Electron's `shared/messageFragments.test.ts`: the reader joins a reply's fragments into the one event that carries
//! it (`Transcript::hold`, messageFragments.ts `withMessageDeltas` and `joinMessageFragments` in one), and repairs an
//! old Cursor record whose closing message restated the whole reply and had the next answer welded onto it.
//!
//! Rust reads a record and a live turn through the same fold, so Electron's two forms (the whole-record join and the
//! live window's delta) are one here; the cases of both are held against it.
use super::*;
use serde_json::json;

fn fragment(sequence: u64, text: &str, final_: bool, item_id: &str) -> Value {
    json!({ "recordType": "mission.event", "event": { "id": format!("event_{sequence}"), "runId": "run_1",
        "missionId": "mission_1", "sequence": sequence, "type": "message.delta", "occurredAt": "2026-09-11T22:00:00.000Z",
        "sourceAdapter": "cursor", "payload": { "itemId": item_id, "operation": "append", "text": text, "final": final_,
            "evidence": { "redacted": true } } } })
}

fn piece(sequence: u64, text: &str) -> Value {
    fragment(sequence, text, false, "msg_0")
}

fn replace(sequence: u64, text: &str, final_: bool) -> Value {
    let mut out = piece(sequence, text);
    out["event"]["payload"]["operation"] = json!("replace");
    out["event"]["payload"]["final"] = json!(final_);
    out
}

fn tool(sequence: u64) -> Value {
    json!({ "recordType": "mission.event", "event": { "id": format!("event_{sequence}"), "runId": "run_1",
        "missionId": "mission_1", "sequence": sequence, "type": "tool.started", "occurredAt": "2026-09-11T22:00:00.000Z",
        "sourceAdapter": "cursor", "payload": { "itemId": format!("tool_{sequence}"), "name": "Read", "evidence": { "redacted": true } } } })
}

fn read(records: Vec<Value>) -> Transcript {
    let mut t = Transcript::default();
    t.apply(records);
    t
}

/// The held reply events, in order: (itemId, text, final).
fn held(t: &Transcript) -> Vec<(String, String, bool)> {
    t.events
        .values()
        .filter(|e| e["type"] == "message.delta")
        .map(|e| {
            let p = &e["payload"];
            (p["itemId"].as_str().unwrap().to_owned(), p["text"].as_str().unwrap().to_owned(), p["final"] == true)
        })
        .collect()
}

/// The replies the thread draws: (itemId, text).
fn replies(t: &Transcript) -> Vec<(String, String)> {
    t.items
        .iter()
        .filter_map(|i| match i {
            Item::Assistant { item_id, text, .. } => Some((item_id.clone(), text.clone())),
            _ => None,
        })
        .collect()
}

const PRE: [&str; 6] = ["Looking", " up", " NVIDIA", " forward", " earnings", "."];
const ANSWER: [&str; 7] = ["NVIDIA", " forward", " earnings", ",", " pulled", " fresh", "."];

/// Colin's mission `fd0adba1`, reduced: a preamble in token fragments, ONE fragment restating all of it (the closing
/// message an old Cursor failed to mark), then the real answer, appended under the same id.
fn recorded() -> Vec<Value> {
    let mut out = Vec::new();
    let mut n = 0;
    for word in PRE {
        n += 1;
        out.push(piece(n, word));
    }
    n += 1;
    out.push(piece(n, &PRE.concat()));
    for word in ANSWER {
        n += 1;
        out.push(piece(n, word));
    }
    out
}

// case: stops the reply saying itself twice
#[test]
fn stops_the_reply_saying_itself_twice() {
    let t = read(recorded());
    let first = &held(&t)[0].1;
    assert_eq!(first, &PRE.concat());
    assert_ne!(first, &format!("{}{}", PRE.concat(), PRE.concat()));
    assert_eq!(replies(&t)[0].1, PRE.concat());
}

// case: gives the answer back its own message instead of welding it on
#[test]
fn gives_the_answer_back_its_own_message_instead_of_welding_it_on() {
    let t = read(recorded());
    assert_eq!(held(&t).len(), 2);
    assert_eq!(held(&t)[1].1, ANSWER.concat());
    assert_eq!(replies(&t).len(), 2, "the thread draws two replies");
    assert_eq!(replies(&t)[1].1, ANSWER.concat());
}

// case: keeps the first message under the id every other surface knows
#[test]
fn keeps_the_first_message_under_the_id_every_other_surface_knows() {
    let ids: Vec<String> = held(&read(recorded())).into_iter().map(|(id, _, _)| id).collect();
    assert_eq!(ids, ["msg_0", "msg_0#2"]);
    // Read again, the same keys.
    let again: Vec<String> = held(&read(recorded())).into_iter().map(|(id, _, _)| id).collect();
    assert_eq!(again, ids);
    // A third restatement numbers on.
    let mut more = recorded();
    more.push(piece(100, &ANSWER.concat()));
    more.push(piece(101, "Third"));
    let ids: Vec<String> = held(&read(more)).into_iter().map(|(id, _, _)| id).collect();
    assert_eq!(ids, ["msg_0", "msg_0#2", "msg_0#3"]);
}

// case: closes the recovered message, which is what the rest of the host reads
#[test]
fn closes_the_recovered_message_which_is_what_the_rest_of_the_host_reads() {
    let t = read(recorded());
    assert!(held(&t)[0].2, "the restated message is closed");
    assert!(matches!(&t.items.iter().find(|i| matches!(i, Item::Assistant { .. })), Some(Item::Assistant { r#final: true, .. })));
}

// case: leaves a conversation the fixed adapter recorded exactly as it is
#[test]
fn leaves_a_conversation_the_fixed_adapter_recorded_exactly_as_it_is() {
    let t = read(vec![piece(1, "a"), piece(2, "b"), replace(3, "ab", true)]);
    assert_eq!(held(&t), [("msg_0".to_owned(), "ab".to_owned(), true)]);
    // A reply that arrived whole is held as it came, down to its `append`.
    let whole = read(vec![fragment(1, "Whole answer.", true, "msg_0")]);
    assert_eq!(whole.events.values().next().unwrap()["payload"]["operation"], "append");
}

// case: does not eat a short repeat that is genuinely the text
#[test]
fn does_not_eat_a_short_repeat_that_is_genuinely_the_text() {
    assert_eq!(held(&read(vec![piece(1, "a"), piece(2, "a")])), [("msg_0".to_owned(), "aa".to_owned(), false)]);
    assert_eq!(held(&read(vec![piece(1, "xx"), piece(2, "xx")])), [("msg_0".to_owned(), "xxxx".to_owned(), false)]);
    // Several pieces then a one-character repeat of all of them ("a", "a" said so far is "aa", not "a").
    assert_eq!(held(&read(vec![piece(1, "a"), piece(2, "b"), piece(3, "b")])).len(), 1);
    // A one-character fragment equal to all said so far, over several pieces, is still text, never an ending.
    assert_eq!(held(&read(vec![piece(1, ""), piece(2, "a"), piece(3, "a")])), [("msg_0".to_owned(), "aa".to_owned(), false)]);
}

// case: keeps the front of a reply longer than the live window, as it arrives
#[test]
fn keeps_the_front_of_a_reply_longer_than_the_live_window_as_it_arrives() {
    let count = EVENT_WINDOW + 1_200;
    let mut t = Transcript::default();
    let mut whole = String::new();
    for index in 0..count {
        let word = format!("w{index} ");
        whole.push_str(&word);
        t.hold(index as u64 + 1, &piece(index as u64 + 1, &word)["event"]);
    }
    let texts = held(&t);
    // One reply is one event, which is what makes the window safe rather than merely generous.
    assert_eq!(texts.len(), 1);
    assert!(texts[0].1.starts_with("w0 w1 w2 "));
    assert_eq!(texts[0].1, whole);
    assert!(!t.trimmed, "nothing was dropped from the window");
}

// case: folds a fragment into its own message even when a tool ran between them
#[test]
fn folds_a_fragment_into_its_own_message_even_when_a_tool_ran_between_them() {
    let t = read(vec![piece(1, "before "), tool(2), piece(3, "after")]);
    assert_eq!(t.events.len(), 2);
    assert_eq!(held(&t), [("msg_0".to_owned(), "before after".to_owned(), false)]);
}

// case: starts a new event for a new message, and keeps them in order
#[test]
fn starts_a_new_event_for_a_new_message_and_keeps_them_in_order() {
    let t = read(vec![piece(1, "first "), piece(2, "one"), fragment(3, "second ", false, "msg_1"), fragment(4, "two", true, "msg_1")]);
    let texts: Vec<String> = held(&t).into_iter().map(|(_, text, _)| text).collect();
    assert_eq!(texts, ["first one", "second two"]);
}

// case: honours a replace, and carries the latest final
#[test]
fn honours_a_replace_and_carries_the_latest_final() {
    let t = read(vec![piece(1, "draft"), replace(2, "final answer", true)]);
    assert_eq!(held(&t), [("msg_0".to_owned(), "final answer".to_owned(), true)]);
}

// case: passes anything that is not a fragment straight through
#[test]
fn passes_anything_that_is_not_a_fragment_straight_through() {
    let t = read(vec![tool(1)]);
    assert_eq!(t.events.len(), 1);
    assert_eq!(t.events[&1]["type"], "tool.started");
}

// case: agrees with the whole-record form, which is the point of them sharing a file
#[test]
fn agrees_with_the_whole_record_form_which_is_the_point_of_them_sharing_a_file() {
    let arriving = vec![piece(1, "a"), tool(2), piece(3, "b"), fragment(4, "c", true, "msg_0")];
    let mut live = Transcript::default();
    for record in arriving.clone() {
        live.apply(vec![record]);
    }
    assert_eq!(held(&live), held(&read(arriving)));
    assert_eq!(replies(&live), [("msg_0".to_owned(), "abc".to_owned())]);
    // And an old record's repair reads the same arriving one by one.
    let mut live = Transcript::default();
    for record in recorded() {
        live.apply(vec![record]);
    }
    assert_eq!(held(&live), held(&read(recorded())));
}

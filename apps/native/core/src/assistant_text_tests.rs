//! Electron assistant-text cases, through the real ledger fold.
use crate::ledger::{Item, Transcript};
use serde_json::json;
fn messages(deltas: &[(&str, &str, &str, bool)]) -> Vec<(String, String, bool)> {
    let mut transcript = Transcript::default();
    transcript.apply(deltas.iter().enumerate().map(|(i, (id, text, operation, final_))| {
        json!({"recordType":"mission.event","event":{
            "sequence":i + 1,"occurredAt":"2026-08-31T16:00:00.000Z",
            "type":"message.delta","payload":{
                "itemId":id,"text":text,"operation":operation,"final":final_
            }
        }})
    }).collect());
    transcript.items.into_iter().filter_map(|item| match item {
        Item::Assistant { item_id, text, r#final } => Some((item_id, text, r#final)),
        _ => None,
    }).collect()
}
#[test]
fn rebuilds_an_appended_stream_instead_of_keeping_the_last_fragment() {
    assert_eq!(messages(&[
        ("a", "Adapter parity ", "append", false),
        ("a", "holds for ", "append", false),
        ("a", "invoice.paid.", "append", true),
    ]), vec![("a".into(), "Adapter parity holds for invoice.paid.".into(), true)]);
}
#[test]
fn honors_a_replace_as_a_replace() {
    assert_eq!(messages(&[
        ("a", "partial", "append", false),
        ("a", "the whole answer", "replace", true),
    ])[0].1, "the whole answer");
}
#[test]
fn keeps_separate_messages_separate_and_in_order() {
    let messages = messages(&[
        ("a", "first", "append", true),
        ("b", "second", "append", true),
    ]);
    assert_eq!(messages.iter().map(|m| m.1.as_str()).collect::<Vec<_>>(), ["first", "second"]);
}

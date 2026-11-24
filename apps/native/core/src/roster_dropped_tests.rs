//! LIVE2 #16: a teammate with a hue that is not one of the eleven vanished without a word.

use super::SAID;
use serde_json::json;

fn said() -> Vec<String> {
    SAID.with(|s| s.borrow_mut().drain(..).collect())
}

fn record(id: &str, name: &str, hue: &str) -> serde_json::Value {
    json!({"teammateId": id, "name": name, "role": "Code & Migrations", "hue": hue, "createdAt": "2026-09-05T05:00:00.000Z"})
}

#[test]
fn a_teammate_with_an_unknown_hue_is_dropped_and_said_in_electrons_words() {
    let text = json!({"schemaVersion": 1, "teammates": [record("tm_codec", "Codec", "clay"), record("tm_oran", "Oran", "orange")]}).to_string();
    let team = crate::native::parse_roster(&text).unwrap();
    // The rest of the roster stays usable (teammate-store.ts: a malformed record is dropped, not repaired).
    assert_eq!(team.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["Codec"]);
    assert_eq!(said(), ["Roster file: dropped 1 teammate record(s) that did not parse."]);
}

#[test]
fn every_dropped_record_is_counted_and_a_clean_roster_says_nothing() {
    let text = json!({"schemaVersion": 1, "teammates": [
        record("tm_a", "Ada", "orange"), record("tm_b", "Bo", "lime"), record("tm_b", "Bo again", "blue"), json!("not a record")
    ]})
    .to_string();
    assert_eq!(crate::native::parse_roster(&text).unwrap().len(), 1);
    assert_eq!(said(), ["Roster file: dropped 3 teammate record(s) that did not parse."]);
    let clean = json!({"schemaVersion": 1, "teammates": [record("tm_b", "Bo", "lime")]}).to_string();
    crate::native::parse_roster(&clean).unwrap();
    assert!(said().is_empty());
}

#[test]
fn the_words_are_electrons() {
    assert_eq!(super::words(2).as_deref(), Some("Roster file: dropped 2 teammate record(s) that did not parse."));
    assert_eq!(super::words(0), None);
}

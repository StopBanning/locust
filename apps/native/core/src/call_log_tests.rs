use super::*;
use std::{fs, path::PathBuf};

struct Profile(PathBuf, Store);

impl Profile {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "locust-call-log-{}",
            crate::invariants::unique_nanos()
        ));
        let store =
            Store::with_codex(root.join("native"), root.join("electron"), Vec::new()).unwrap();
        fs::create_dir_all(store.root()).unwrap();
        Self(root, store)
    }

    fn file(&self) -> PathBuf {
        self.1.root().join(FILE)
    }
}

impl Drop for Profile {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn call() -> Value {
    json!({
        "callId": "call_1", "teammateId": "tm_ada", "name": "Ada",
        "afterMissionId": "m_typed", "startedAt": 1_000, "endedAt": 104_000,
        "lines": [
            { "role": "you", "text": "Ada, check the readme for the secret ingredient.", "at": 10_000 },
            { "role": "voice", "text": "Let me check.", "at": 12_000 },
            { "role": "you", "text": "So what did you find?", "at": 60_000 },
            { "role": "voice", "text": "It is cardamom.", "at": 62_000 }
        ],
        "turns": [{ "missionId": "m_asked", "at": 20_000 }]
    })
}

fn words<'a>(piece: &Piece<'a>) -> Vec<&'a str> {
    piece
        .lines
        .iter()
        .map(|line| line["text"].as_str().unwrap())
        .collect()
}

// case: puts what asked for a turn above it, and what came after under it
#[test]
fn asks_above_and_replies_below() {
    let fixture = [call()];
    let places = placements(&fixture, &["m_typed", "m_asked"]);
    let before = &places[&Place::Before("m_asked")][0];
    let after = &places[&Place::After("m_asked")][0];
    assert_eq!(
        words(before),
        [
            "Ada, check the readme for the secret ingredient.",
            "Let me check."
        ]
    );
    assert_eq!(words(after), ["So what did you find?", "It is cardamom."]);
    assert!(before.first && !before.last);
    assert!(after.last && !after.first);
}

// case: puts a call that started nothing under the turn that was on screen, else at the end
#[test]
fn calls_without_turns_follow_the_anchor_or_end() {
    let mut fixture = [call()];
    fixture[0]["turns"] = json!([]);
    let places = placements(&fixture, &["m_typed"]);
    let piece = &places[&Place::After("m_typed")][0];
    assert_eq!(piece.lines.len(), 4);
    assert!(piece.first && piece.last);

    fixture[0]["afterMissionId"] = json!("elsewhere");
    assert_eq!(
        placements(&fixture, &["m_typed"])[&Place::End][0]
            .lines
            .len(),
        4
    );
}

// case: splits the words between two turns at the second one
#[test]
fn second_turn_cuts_the_transcript() {
    let mut fixture = [call()];
    fixture[0]["turns"]
        .as_array_mut()
        .unwrap()
        .insert(0, json!({"missionId": "m_again", "at": 61_000}));
    let places = placements(&fixture, &["m_typed", "m_asked", "m_again"]);
    assert_eq!(
        words(&places[&Place::After("m_asked")][0]),
        ["So what did you find?"]
    );
    assert_eq!(
        words(&places[&Place::After("m_again")][0]),
        ["It is cardamom."]
    );
}

// case: is kept only when it reads as one
#[test]
fn only_well_formed_calls_are_kept() {
    assert!(is_saved_call(&call()));
    assert!(!is_saved_call(&Value::Null));
    assert!(!is_saved_call(&json!({})));
    for (field, bad) in [
        ("callId", json!("no spaces allowed")),
        ("lines", json!([{ "role": "them", "text": "x", "at": 1 }])),
        ("name", json!("")),
        ("startedAt", json!(-1)),
    ] {
        let mut invalid = call();
        invalid[field] = bad;
        assert!(!is_saved_call(&invalid), "{field}");
    }
}

// case: keeps a call, gives it back for any of its turns, and replaces it when it is sent again
#[test]
fn saving_an_ended_call_replaces_the_earlier_record() {
    let profile = Profile::new();
    let mut ongoing = call();
    ongoing.as_object_mut().unwrap().remove("endedAt");
    save(&profile.1, &ongoing).unwrap();
    save(&profile.1, &call()).unwrap();
    assert_eq!(list(&profile.1, &["m_asked"]), vec![call()]);
    assert_eq!(list(&profile.1, &["m_typed"]), vec![call()]);
    assert!(list(&profile.1, &["m_other"]).is_empty());
    assert_eq!(read(&profile.1).unwrap(), vec![call()]);
}

// case: refuses what is not a call, and keeps only the newest
#[test]
fn refuses_invalid_records_and_keeps_the_newest_calls() {
    let profile = Profile::new();
    assert!(save(&profile.1, &json!({ "callId": "x" })).is_err());
    assert!(list(&profile.1, &["m_asked"]).is_empty());
    let calls: Vec<_> = (0..500)
        .map(|index| {
            let mut saved = call();
            saved["callId"] = json!(format!("call_{index}"));
            saved["startedAt"] = json!(1_000 + index);
            saved
        })
        .collect();
    fs::write(profile.file(), json!({ "calls": calls }).to_string()).unwrap();
    let mut newest = call();
    newest["callId"] = json!("call_new");
    newest["startedAt"] = json!(999_999);
    save(&profile.1, &newest).unwrap();
    let kept = read(&profile.1).unwrap();
    assert_eq!(kept.len(), 500);
    assert_eq!(kept.last(), Some(&newest));
    assert!(kept.iter().all(|call| call["callId"] != "call_0"));
}

// case: writes nothing over a calls.json that does not read, and lists none from it
#[test]
fn unreadable_calls_survive_a_save_attempt() {
    let profile = Profile::new();
    save(&profile.1, &call()).unwrap();
    let body = fs::read_to_string(profile.file()).unwrap();
    let cut_short = &body[..40];
    for unreadable in [cut_short, "null"] {
        fs::write(profile.file(), unreadable).unwrap();
        assert!(save(&profile.1, &call()).is_err());
        assert_eq!(fs::read_to_string(profile.file()).unwrap(), unreadable);
        assert!(list(&profile.1, &["m_asked"]).is_empty());
    }
    let fresh = Profile::new();
    save(&fresh.1, &call()).unwrap();
    assert_eq!(list(&fresh.1, &["m_asked"]), vec![call()]);
}

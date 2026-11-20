//! Electron's `a-team-travels-as-a-picture.test.ts` (8 cases): a team shared as an image with the team written into it
//! as data, and only the part that is safe and useful to hand to someone else.
use super::*;
use crate::team_card::{TEAM_CARD_KEYWORD, free_name, read_team_card, team_card_of};
use serde_json::json;
use std::collections::HashSet;

/// A real 1x1 PNG.
const PNG: [u8; 70] = [137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 218, 99, 100, 96, 248, 95, 15, 0, 2, 135, 1, 128, 235, 71, 186, 146, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130];

/// Every chunk's CRC checks, and IEND is last: the image is still an image.
fn well_formed(png: &[u8]) -> bool {
    let mut at = 8;
    let mut last: &[u8] = b"";
    while at + 8 <= png.len() {
        let length = u32::from_be_bytes(png[at..at + 4].try_into().unwrap()) as usize;
        let Some(type_and_data) = png.get(at + 4..at + 8 + length) else { return false };
        let Some(crc) = png.get(at + 8 + length..at + 12 + length) else { return false };
        if crc32(type_and_data) != u32::from_be_bytes(crc.try_into().unwrap()) {
            return false;
        }
        last = &png[at + 4..at + 8];
        at += 12 + length;
    }
    at == png.len() && last == b"IEND"
}

// ---- a text chunk in a PNG ----

#[test]
fn goes_in_and_comes_back_names_that_are_not_latin_1_included_and_the_image_stays_well_formed() {
    assert!(well_formed(&PNG), "the control: the image itself is well formed");
    let written = with_text_chunk(&PNG, TEAM_CARD_KEYWORD, "{\"name\":\"Zoë 🦗\"}").unwrap();
    assert!(is_png(&written));
    assert!(well_formed(&written));
    assert_eq!(read_text_chunk(&written, TEAM_CARD_KEYWORD).as_deref(), Some("{\"name\":\"Zoë 🦗\"}"));
}

#[test]
fn replaces_the_one_already_there_rather_than_stacking_a_second() {
    let twice = with_text_chunk(&with_text_chunk(&PNG, TEAM_CARD_KEYWORD, "first").unwrap(), TEAM_CARD_KEYWORD, "second").unwrap();
    assert_eq!(read_text_chunk(&twice, TEAM_CARD_KEYWORD).as_deref(), Some("second"));
    assert_eq!(twice.windows(4).filter(|w| *w == b"tEXt").count(), 1);
}

#[test]
fn reads_nothing_from_an_image_without_one_or_from_something_that_is_not_a_png() {
    assert_eq!(read_text_chunk(&PNG, TEAM_CARD_KEYWORD), None);
    assert_eq!(read_text_chunk(b"not an image", TEAM_CARD_KEYWORD), None);
    assert_eq!(with_text_chunk(b"not an image", TEAM_CARD_KEYWORD, "x").unwrap_err(), "That is not a PNG image.");
}

// ---- the card ----

#[test]
fn carries_name_role_look_and_model_and_no_id_folder_limit_or_model_of_your_own() {
    let card = team_card_of(&[
        json!({"teammateId": "tm_wren", "name": "Wren", "role": "Code & Migrations", "hue": "lime", "avatar": {"seed": 4321}, "createdAt": "2026-09-27T05:00:00.000Z",
            "route": {"runtime": "claude", "model": "opus", "mode": "accept-edits", "effort": "high"}, "monthlyLimitUsd": 20, "worktree": true}),
        json!({"teammateId": "tm_juno", "name": "Juno", "role": "Code & Migrations", "hue": "lime", "avatar": {"seed": 8765}, "createdAt": "2026-09-27T05:00:00.000Z",
            "route": {"runtime": "opencode", "model": "own-1a2b3c4d/acme-70b", "mode": "ask"}}),
    ]);
    assert_eq!(card["schema"], 1);
    assert_eq!(card["teammates"][0]["name"], "Wren");
    assert_eq!(card["teammates"][0]["role"], "Code & Migrations");
    assert_eq!(card["teammates"][0]["hue"], "lime");
    assert_eq!(card["teammates"][0]["avatar"], json!({"seed": 4321}));
    assert_eq!(card["teammates"][0]["route"], json!({"runtime": "claude", "model": "opus", "mode": "accept-edits", "effort": "high"}));
    assert!(card["teammates"][1].get("route").is_none(), "a model of the person's own does not travel");
    let said = card.to_string();
    for kept_home in ["tm_", "monthlyLimitUsd", "worktree", "own-1a2b"] {
        assert!(!said.contains(kept_home), "{kept_home}: {said}");
    }
}

#[test]
fn never_brings_a_teammate_in_on_auto_a_picture_from_someone_else_does_not_widen_what_runs_unasked() {
    let read = read_team_card(&json!({"schema": 1, "teammates": [{"name": "Rook", "role": "Custom", "hue": "blue", "route": {"runtime": "codex", "model": "gpt", "mode": "auto"}}]})).unwrap();
    assert_eq!(read[0]["route"]["mode"], "accept-edits");
}

#[test]
fn is_refused_when_it_is_not_a_locust_team_card() {
    assert!(read_team_card(&json!({"schema": 2, "teammates": []})).is_none());
    assert!(read_team_card(&json!({"teammates": []})).is_none());
    assert!(read_team_card(&json!("hello")).is_none());
}

#[test]
fn never_takes_more_than_twelve_teammates_from_one_card() {
    let many: Vec<_> = (0..40).map(|i| json!({"name": format!("T{i}")})).collect();
    assert_eq!(read_team_card(&json!({"schema": 1, "teammates": many})).unwrap().len(), 12);
}

#[test]
fn gives_a_teammate_whose_name_is_taken_the_next_free_one() {
    assert_eq!(free_name("Wren", &HashSet::from(["wren".to_owned(), "Wren 2".to_owned()])), "Wren 3");
    assert_eq!(free_name("Sable", &HashSet::from(["Wren".to_owned()])), "Sable");
}

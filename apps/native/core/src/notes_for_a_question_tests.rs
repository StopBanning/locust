// Carries apps/desktop/src/shared/which-notes-bear-on-a-question.test.ts (Electron 4ce2f37a): which kept notes a brief
// carries, for the question asked.
use crate::people::{briefed_memories, memories_for_brief};
use serde_json::{Value, json};

fn line(text: &str, day: u32) -> Value {
    json!({ "text": text, "scope": "workspace", "by": "Wren", "at": format!("2026-09-{day:02}T05:00:00.000Z") })
}

const PLACES: [&str; 6] = ["sidebar", "composer", "thread", "settings", "roster", "memory screen"];

fn twenty() -> Vec<Value> {
    let mut rows = vec![line("Locust keeps its orb animation in HomeCover.tsx.", 1), line("Deploys of Locust wait now, but builds go on.", 2)];
    rows.extend((0..18).map(|i| line(&format!("Locust note {i} about the {}.", PLACES[i % PLACES.len()]), 3 + i as u32)));
    rows
}
fn texts(rows: &[&Value]) -> Vec<String> {
    rows.iter().map(|m| m["text"].as_str().unwrap().to_owned()).collect()
}

#[test]
fn a_word_few_notes_carry_lifts_a_note_however_old() {
    let rows = twenty();
    let chosen = memories_for_brief(&rows, Some("Where does Locust keep the orb?"), 8);
    assert!(chosen[0]["text"].as_str().unwrap().contains("orb animation"));
}

#[test]
fn the_rarer_of_two_shared_words_counts_for_more_than_the_newer_note() {
    let rows = twenty();
    let chosen = memories_for_brief(&rows, Some("the orb and the sidebar"), 8);
    assert!(chosen[0]["text"].as_str().unwrap().contains("orb animation"));
    assert!(chosen[1..4].iter().all(|m| m["text"].as_str().unwrap().contains("sidebar")), "{:?}", texts(&chosen));
}

#[test]
fn plain_english_words_do_not_count_toward_a_note() {
    let rows = twenty();
    let newest_eight: Vec<String> = rows[12..].iter().rev().map(|m| m["text"].as_str().unwrap().to_owned()).collect();
    assert_eq!(texts(&memories_for_brief(&rows, Some("but now?"), 8)), newest_eight);
}

#[test]
fn the_brief_keeps_the_newest_notes_when_the_allowance_runs_out() {
    let long: Vec<Value> = (0..8).map(|i| line(&format!("Note from day {}: {}", i + 1, "x".repeat(300)), i + 1)).collect();
    let briefed = texts(&briefed_memories(&long, None, Some(".locust/memory.md")));
    assert!(briefed.len() < 8, "{}", briefed.len());
    assert!(briefed[0].contains("day 8"));
    assert!(!briefed.iter().any(|t| t.contains("day 1:")));
}

// Rust's own (memory.ts sorts by score, then the newer note first; no Electron case has a tie): notes the question
// lifts alike come newest first.
#[test]
fn notes_the_question_lifts_alike_come_newest_first() {
    let rows = twenty();
    let chosen = memories_for_brief(&rows, Some("the sidebar"), 8);
    assert_eq!(texts(&chosen[..3]), ["Locust note 12 about the sidebar.", "Locust note 6 about the sidebar.", "Locust note 0 about the sidebar."]);
}

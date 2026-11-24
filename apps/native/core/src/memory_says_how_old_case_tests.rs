//! Electron's `main/memory-says-how-old.test.ts` (5 cases): a briefed memory says when it was written, on
//! `memory_section`, which a turn's brief calls (`workspace_sections`), and `memory_age`, which it calls for each line.
use super::*;
use serde_json::json;

fn now() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-09-14T12:00:00.000Z").unwrap().with_timezone(&chrono::Utc)
}
fn ago(days: i64) -> String {
    (now() - chrono::Duration::days(days)).to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
fn section(memories: &[Value]) -> String {
    memory_section(Some("Wren"), Some("shop"), memories, false, false, now(), None, None)
}

// case: says it the way a person would
#[test]
fn a_memorys_age_is_said_the_way_a_person_would() {
    let said: Vec<Option<String>> = [0, 1, 5, 21, 90, 500].iter().map(|d| memory_age(&ago(*d), now())).collect();
    assert_eq!(
        said,
        ["today", "yesterday", "5 days ago", "3 weeks ago", "3 months ago", "over a year ago"].map(|s| Some(s.to_owned())).to_vec()
    );
}

// case: says nothing rather than something wrong
#[test]
fn a_memorys_age_is_nothing_rather_than_wrong() {
    assert_eq!(memory_age("", now()), None);
    assert_eq!(memory_age("not a date", now()), None);
    // A clock that moved backwards must not produce "in -2 days".
    assert_eq!(memory_age(&ago(-2), now()), None);
}

// case: puts the age beside who wrote it, in the brief itself
#[test]
fn a_briefed_memory_has_its_age_beside_who_wrote_it() {
    let got = section(&[
        json!({"text": "The API is on port 3000", "scope": "workspace", "by": "Yurt", "createdAt": ago(30)}),
        json!({"text": "The API is on port 8080", "scope": "workspace", "by": "Wren", "createdAt": ago(0)}),
    ]);
    assert!(got.contains("The API is on port 3000 (this folder, by Yurt, 4 weeks ago)"), "{got}");
    assert!(got.contains("The API is on port 8080 (this folder, by Wren, today)"), "{got}");
}

// case: tells the reader what to do when two of them disagree
#[test]
fn the_memory_brief_says_the_newer_of_two_is_usually_the_correction() {
    let got = section(&[json!({"text": "Tests run with pnpm", "scope": "workspace", "by": "Wren", "createdAt": ago(2)})]);
    assert!(got.contains("the newer one is usually the correction"), "{got}");
}

// case: still reads correctly for a memory with no date at all
#[test]
fn a_memory_with_no_date_reads_without_one() {
    let got = section(&[json!({"text": "Tests run with pnpm", "scope": "workspace", "by": "Wren"})]);
    // No trailing comma, no empty parenthetical -- the line is just shorter.
    assert!(got.contains("Tests run with pnpm (this folder, by Wren)"), "{got}");
}

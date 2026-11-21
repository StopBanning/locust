//! The sidebar's half of `main/torn-ledger.test.ts`: the records' damage is said under the conversations in
//! Screens.tsx's words (`native::ListDamage`), and nothing is said when the records are whole. Rust has no
//! "N conversations, M in this folder" header for these words to follow (`missions-lists-conversations.test.tsx`).
use super::*;

fn drawn(rows: usize, damage: native::ListDamage) -> Option<String> {
    let mut s = State::default();
    s.recent = (1..=rows)
        .map(|i| ConversationRow {
            id: format!("m_{i}"),
            teammate: "Wren".into(),
            title: format!("Task {i}"),
            started_by: "native".into(),
            state: "completed".into(),
            updated: format!("2026-11-18T10:0{i}:00Z"),
        })
        .collect();
    s.recent_damage = damage;
    let mut rt = rune::Runtime::new(s, |s, _| sidebar(s, &Palette::new(true)), (280.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some("sidebar-records-damage")).and_then(|e| e.text)
}

#[test]
fn damaged_records_are_said_under_the_list() {
    let mixed = native::ListDamage { incomplete: 2, unreadable: 1, folder_unreadable: false };
    assert_eq!(drawn(2, mixed).as_deref(), Some("2 with an incomplete receipt · 1 file could not be read"));
    let unreadable = native::ListDamage { incomplete: 0, unreadable: 1, folder_unreadable: false };
    assert_eq!(drawn(0, unreadable).as_deref(), Some("1 file could not be read"), "said with nothing listed too");
}

#[test]
fn a_folder_that_could_not_be_listed_is_said_with_nothing_listed() {
    let folder = native::ListDamage { incomplete: 0, unreadable: 0, folder_unreadable: true };
    assert_eq!(drawn(0, folder).as_deref(), Some("the records could not be read"));
}

#[test]
fn whole_records_say_nothing_about_the_records() {
    assert_eq!(drawn(1, native::ListDamage::default()), None);
}

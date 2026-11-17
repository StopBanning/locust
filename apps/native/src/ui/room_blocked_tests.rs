//! Electron's `a-disabled-create-says-why.test.ts`: a disabled Create room says why.

use super::{room_blocked_reason, room_full_note};

#[test]
fn says_nothing_when_the_room_can_be_made() {
    assert_eq!(room_blocked_reason("standup", 2), None);
    assert_eq!(room_blocked_reason("  standup  ", 1), None);
}

#[test]
fn names_the_missing_name() {
    assert_eq!(room_blocked_reason("", 2).as_deref(), Some("Name the room to create it."));
    // Whitespace is not a name, and the button agrees.
    assert_eq!(room_blocked_reason("   ", 2).as_deref(), Some("Name the room to create it."));
}

#[test]
fn names_the_missing_teammates() {
    assert_eq!(room_blocked_reason("standup", 0).as_deref(), Some("Pick at least one teammate."));
}

#[test]
fn keeps_the_cap_first_and_keeps_its_own_words() {
    let full = room_blocked_reason("", 9);
    assert_eq!(full, room_full_note(9));
    assert!(full.unwrap().contains("Untick 1"));
}

#[test]
fn agrees_with_the_condition_that_disables_the_button() {
    let disabled = |name: &str, ticked: usize| name.trim().is_empty() || ticked == 0 || room_full_note(ticked).is_some();
    for name in ["", "   ", "standup"] {
        for ticked in [0, 1, 8, 9, 20] {
            assert_eq!(room_blocked_reason(name, ticked).is_some(), disabled(name, ticked), "{name:?} / {ticked}");
        }
    }
}

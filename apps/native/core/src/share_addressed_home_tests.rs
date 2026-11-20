//! Electron's `share-addressed-home.test.ts` (6 cases): who a share block meant, by name or role, the sender itself when
//! it used its own, and nobody when a role is shared or nothing on the roster answers.
use super::*;

fn entry(id: &str, name: &str, role: &str) -> PeerEntry {
    PeerEntry { teammate_id: id.into(), name: name.into(), role: role.into() }
}
fn jimothy() -> PeerEntry {
    entry("tm_self", "Jimothy", "Finance Bro")
}
fn team() -> Vec<PeerEntry> {
    vec![entry("tm_0", "Wren", "Code & Migrations"), entry("tm_1", "Gem", "Custom")]
}
fn named<'a>(r: Recipient<'a>) -> Option<&'a str> {
    match r {
        Recipient::Teammate(t) => Some(t.name.as_str()),
        _ => None,
    }
}

#[test]
fn is_the_sender_itself_when_it_used_its_own_role() {
    let (me, others) = (jimothy(), team());
    for said in ["Finance Bro", "finance bro", "Jimothy"] {
        assert_eq!(recipient_of(said, &me, &others), Recipient::Itself, "{said}");
    }
}

#[test]
fn is_a_teammate_named_however_it_was_cased() {
    let (me, others) = (jimothy(), team());
    assert_eq!(named(recipient_of("Wren", &me, &others)), Some("Wren"));
    assert_eq!(named(recipient_of("  gem  ", &me, &others)), Some("Gem"));
}

#[test]
fn is_a_teammate_found_by_their_role_when_exactly_one_answers_to_it() {
    let (me, others) = (jimothy(), team());
    assert_eq!(named(recipient_of("Code & Migrations", &me, &others)), Some("Wren"));
    assert_eq!(named(recipient_of("Custom", &me, &others)), Some("Gem"));
}

#[test]
fn is_nobody_when_a_role_is_shared_rather_than_whoever_came_first() {
    let crowded = vec![entry("tm_0", "Gem", "Custom"), entry("tm_1", "Fen", "Custom")];
    assert_eq!(recipient_of("Custom", &jimothy(), &crowded), Recipient::Nobody);
}

#[test]
fn is_nobody_for_a_name_nothing_on_the_roster_answers_to() {
    let (me, others) = (jimothy(), team());
    for said in ["Mallory", "", "   "] {
        assert_eq!(recipient_of(said, &me, &others), Recipient::Nobody, "{said:?}");
    }
}

#[test]
fn prefers_a_real_teammate_over_the_sender_when_a_name_is_worn_twice() {
    let clash = vec![entry("tm_0", "Finance Bro", "Docs & QA")];
    assert_eq!(named(recipient_of("Finance Bro", &jimothy(), &clash)), Some("Finance Bro"));
}

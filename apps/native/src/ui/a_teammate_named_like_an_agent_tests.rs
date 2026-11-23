//! a-teammate-named-like-an-agent-is-said-as-a-teammate.test.ts: a name an AI agent also has is matched exactly (case
//! and spacing aside), the roster tells the other teammates it is a teammate, and the editor says so without stopping
//! the save.
use super::*;
use locust_core::briefing::{PeerEntry, agent_named, assemble, roster_section};

#[test]
fn matches_exact_name() {
    // case: matches exact name
    assert_eq!(agent_named("Codex").as_deref(), Some("Codex"));
    assert_eq!(agent_named("Claude Code").as_deref(), Some("Claude Code"));
}

#[test]
fn matches_different_case() {
    // case: matches different case
    assert_eq!(agent_named("codex").as_deref(), Some("Codex"));
    assert_eq!(agent_named("CODEX").as_deref(), Some("Codex"));
}

#[test]
fn matches_claude_code_vs_claude_code() {
    // case: matches "claude code" vs "Claude Code"
    assert_eq!(agent_named("claude code").as_deref(), Some("Claude Code"));
    assert_eq!(agent_named("Claude Code").as_deref(), Some("Claude Code"));
}

#[test]
fn returns_undefined_for_non_matches_like_codexa_and_cody() {
    // case: returns undefined for non-matches like "Codexa" and "Cody"
    for name in ["Codexa", "Cody", "Casper"] {
        assert_eq!(agent_named(name), None, "{name}");
    }
}

fn peer(id: &str, name: &str) -> PeerEntry {
    PeerEntry { teammate_id: id.into(), name: name.into(), role: "Code & Migrations".into() }
}

fn roster_prompt(others: &[PeerEntry]) -> String {
    assemble(&[roster_section(&peer("tm_casper", "Casper"), others)], &[], 0, &[], None, "Check the roster.").0
}

#[test]
fn lists_a_peer_matching_an_agent_name_with_words_that_settle_it() {
    // case: lists a peer matching an agent name with words that settle it
    let prompt = roster_prompt(&[peer("tm_codex", "Codex")]);
    assert!(
        prompt.contains("Teammates in this workspace besides you (Casper, Code & Migrations): Codex (Code & Migrations; a teammate here, not the Codex program)."),
        "{prompt}"
    );
    assert!(!prompt.contains("Codex (Code & Migrations)"));
}

#[test]
fn leaves_a_non_matching_peer_unchanged_as_the_control() {
    // case: leaves a non-matching peer unchanged as the control
    let prompt = roster_prompt(&[peer("tm_wren", "Wren")]);
    assert!(prompt.contains("Teammates in this workspace besides you (Casper, Code & Migrations): Wren (Code & Migrations)."), "{prompt}");
}

fn editing(name: &str) -> rune::Runtime<State> {
    let mut s = State::default();
    let teammate = json!({"teammateId":"tm_test", "name":"Wren", "hue":"lime", "role":"Code & Migrations", "createdAt":"2026-09-23T05:00:00.000Z", "avatar":{"headwear":0,"accessory":0,"mouth":0}});
    s.people_panel.team = json!({"teammates": [teammate]});
    edit(&mut s, Some("tm_test"));
    s.people_panel.fields[0] = field(name);
    let mut rt = rune::Runtime::new(s, |s, _| edit_form(s, &Palette::new(true)), (1100.0, 1800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

fn said(rt: &rune::Runtime<State>) -> String {
    rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

#[test]
fn shows_the_note_for_codex_and_does_not_block_saving() {
    // case: shows the note for "codex" and does not block saving
    let rt = editing("codex");
    assert!(said(&rt).contains("Codex is also an AI agent's name. Your other teammates will be told this Codex is a teammate."), "{}", said(&rt));
    let save = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("team-save")).unwrap();
    assert!(save.opacity > 0.9, "the save button is not disabled (opacity={})", save.opacity);
    assert!(form_name_clash(&rt.state).is_none());
}

#[test]
fn does_not_show_the_note_for_casper() {
    // case: does not show the note for "Casper"
    assert!(!said(&editing("Casper")).contains("is also an AI agent's name"));
}

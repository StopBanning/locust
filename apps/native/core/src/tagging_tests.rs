//! apps/desktop/src/main/a-teammate-can-be-tagged-from-any-conversation.test.ts
use super::*;
use crate::conversation_turns::tagged_words;

#[test]
fn is_the_message_first_so_their_conversation_is_titled_by_it_then_where_it_came_from_and_the_latest_answer_there() {
    let prompt = tagged_prompt("Can you double-check the tax rate?", Some("Wren"), Some("I changed TAX_RATE to 0.07 in prices.py."));
    assert!(prompt.starts_with("Can you double-check the tax rate?\n\n"));
    assert!(prompt.contains("(You were tagged in Wren's conversation. Wren's latest answer there, for context:)"));
    assert!(prompt.ends_with("I changed TAX_RATE to 0.07 in prices.py."));
}

#[test]
fn says_only_where_it_came_from_when_that_conversation_has_no_answer_yet() {
    assert_eq!(tagged_prompt("Take a look.", Some("Wren"), None), "Take a look.\n\n(You were tagged in Wren's conversation.)");
    assert_eq!(tagged_prompt("Take a look.", Some("Wren"), Some("  ")), "Take a look.\n\n(You were tagged in Wren's conversation.)");
}

#[test]
fn is_the_message_alone_when_it_was_sent_from_nobodys_conversation() {
    assert_eq!(tagged_prompt("Take a look.", None, Some("an answer")), "Take a look.");
}

#[test]
fn fits_the_mission_limit_cutting_the_answer_rather_than_the_message() {
    let prompt = tagged_prompt("Check this.", Some("Wren"), Some(&"x".repeat(20_000)));
    assert!(prompt.encode_utf16().count() <= 8_000);
    assert!(prompt.starts_with("Check this."));
    assert!(prompt.contains("(cut short here)"));
    // Counted as the ledger counts, in UTF-16 units, and cut on a character: an answer of emoji still fits.
    let wide = tagged_prompt("Check this.", Some("Wren"), Some(&"😀".repeat(10_000)));
    assert!(wide.encode_utf16().count() <= 8_000, "{}", wide.encode_utf16().count());
}

#[test]
fn reaches_at_most_a_handful_of_teammates_at_once() {
    assert_eq!(MAX_TAGGED, 5);
    let mate = |id: &str| Teammate { id: id.into(), name: format!("Mate {id}"), role: String::new(), runtime: None, model: None, account: None, busy: false, runs_in: String::new(), folder: None };
    let team: Vec<Teammate> = ["a", "b", "a", "c", "d", "e", "f", "g"].into_iter().map(mate).collect();
    let sent = std::sync::Mutex::new(Vec::new());
    let out = send_tagged(team, "Look.", Some("Wren"), None, |job, origin| {
        assert_eq!(origin.started_by, Some(serde_json::json!({ "kind": "tag" })));
        assert_eq!(job.prompt, "Look.\n\n(You were tagged in Wren's conversation.)");
        sent.lock().unwrap().push(job.teammate.id.clone());
        if job.teammate.id == "c" { return Err("Mate c is already active in another run.".into()); }
        Ok(crate::engine::Started { conversation_id: format!("m_{}", job.teammate.id), mode: "ask".into() })
    });
    assert_eq!(*sent.lock().unwrap(), ["a", "b", "c", "d", "e"]);
    assert_eq!(out.started, ["Mate a", "Mate b", "Mate d", "Mate e"]);
    assert_eq!(out.refused, [("Mate c".to_owned(), "Mate c is busy. Tag them again when they are free.".to_owned())]);
    assert_eq!(out.said().as_deref(), Some("Also sent to Mate a, Mate b, Mate d, Mate e. Not sent to Mate c: Mate c is busy. Tag them again when they are free."));
}

#[test]
fn is_the_message_after_a_restart_too_when_the_ledger_has_recorded_the_run_as_an_ordinary_one() {
    let prompt = tagged_prompt("Can you double-check the tax rate?", Some("Wren"), Some("I changed TAX_RATE."));
    assert_eq!(tagged_words(&prompt), "Can you double-check the tax rate?");
    let m = crate::ledger::Metadata { prompt: prompt.clone(), ..Default::default() };
    assert_eq!(crate::conversation_turns::turn_line(&m).as_deref(), Some("Can you double-check the tax rate?"));
    let tagged = crate::ledger::Metadata { prompt, started_by_kind: Some("tag".into()), ..Default::default() };
    assert_eq!(crate::conversation_turns::turn_line(&tagged).as_deref(), Some("Can you double-check the tax rate?"));
}

#[test]
fn leaves_a_message_nobody_was_tagged_in_alone() {
    assert_eq!(tagged_words("Just a message.\n\n(an aside)"), "Just a message.\n\n(an aside)");
}

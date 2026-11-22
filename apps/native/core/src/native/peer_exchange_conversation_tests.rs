//! peer-exchange-conversation.test.ts: the conversation's prior turn selects the group briefing at real send.
use super::*;

// case: passes the previous turn through to memory.section, and its words into the prompt
#[test]
fn previous_turn_selects_group_words_in_runtime_prompt() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), codex_commands_hook(&t));
    let group = crate::groups::create_group(&t.store(), "Trading").unwrap();
    crate::groups::set_group_instructions(&t.store(), &group.group_id, "Check trading risk.")
        .unwrap();
    let first = send(job(&t, None, "Start here.")).unwrap();
    finished(&first.conversation_id);
    assert!(
        !codex_requests(&t)
            .iter()
            .filter(|r| r["method"] == "turn/start")
            .last()
            .unwrap()["params"]["input"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Standing instructions")
    );
    crate::groups::assign_to_group(&t.store(), &first.conversation_id, Some(&group.group_id))
        .unwrap();
    // Force a full brief; a resumed runtime otherwise receives only the changed paragraphs it lacks.
    let _ = std::fs::remove_file(t.store().root().join("brief-sessions.json"));
    let next = send(job(&t, Some(&first.conversation_id), "Carry on.")).unwrap();
    finished(&next.conversation_id);
    let sent = codex_requests(&t);
    let prompt = sent
        .iter()
        .filter(|r| r["method"] == "turn/start")
        .last()
        .unwrap()["params"]["input"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        prompt.contains("Standing instructions for the group \"Trading\""),
        "{prompt}"
    );
    assert!(prompt.contains("Check trading risk."), "{prompt}");
    assert!(prompt.contains("Carry on."));
}

// case: never refuses a run over a brief it could do without
#[test]
fn unreadable_group_brief_never_refuses_run() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), codex_commands_hook(&t));
    let first = send(job(&t, None, "Start here.")).unwrap();
    finished(&first.conversation_id);
    std::fs::create_dir(t.store().root().join("groups.json")).unwrap();
    let next = send(job(&t, Some(&first.conversation_id), "Carry on.")).unwrap();
    finished(&next.conversation_id);
    let sent = codex_requests(&t);
    let prompt = sent
        .iter()
        .filter(|r| r["method"] == "turn/start")
        .last()
        .unwrap()["params"]["input"][0]["text"]
        .as_str()
        .unwrap();
    assert!(prompt.contains("Carry on."));
    assert!(!prompt.contains("Standing instructions"));
    let rows = ledgers(&t);
    assert_eq!(rows.len(), 2);
}

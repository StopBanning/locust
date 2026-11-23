//! `what-waits-on-you-is-in-one-place.test.ts` (8 cases): paused runs, questions a teammate stopped on,
//! urgent Scout cards, waiting routine copies, and memory suggestions in one ordered list.
use serde_json::{Value, json};

use crate::needs_you::{
    ApprovalInput, ConversationInput, NEEDS_YOU_TEXT, NeedsYouItem, NeedsYouPlace, RoutineInput, ScoutCardInput,
    needs_you, needs_you_chip, needs_you_label, needs_you_place, open_question,
};

const ASK: &str =
    "<locust-ask>\nWhich port should the API use?\n- 3000 :: as documented\n- 3001 :: as deployed\n</locust-ask>";

fn final_msg(text: &str, item_id: &str) -> Value {
    json!({
        "type": "message.delta",
        "payload": { "itemId": item_id, "operation": "replace", "text": text, "final": true }
    })
}

fn completed() -> Value {
    json!({ "type": "run.completed", "payload": {} })
}

fn owner_of_run(run_id: &str) -> Option<&'static str> {
    (run_id == "run_w").then_some("tm_wren")
}

fn owner_of_mission(mission_id: &str) -> Option<&'static str> {
    (mission_id == "mission_p").then_some("tm_pip")
}

fn name_of(teammate_id: &str) -> Option<&'static str> {
    match teammate_id {
        "tm_wren" => Some("Wren"),
        "tm_pip" => Some("Pip"),
        _ => None,
    }
}

// case: is its last final message’s question, once the run has completed
#[test]
fn is_its_last_final_messages_question_once_the_run_has_completed() {
    assert_eq!(
        open_question(&[final_msg(&format!("Two ways to go.\n{ASK}"), "m1"), completed()]).as_deref(),
        Some("Which port should the API use?")
    );
    // Still running: the block may not be finished.
    assert_eq!(open_question(&[final_msg(&format!("Two ways to go.\n{ASK}"), "m1")]), None);
    // An earlier message asked; the last one did not.
    assert_eq!(open_question(&[final_msg(ASK, "m1"), final_msg("Done, I went with 3001.", "m2"), completed()]), None);
}

// case: is read the same the second time
#[test]
fn is_read_the_same_the_second_time() {
    let events = vec![final_msg(ASK, "m1"), completed()];
    assert_eq!(open_question(&events), open_question(&events));
}

// case: is nothing when nothing does
#[test]
fn is_nothing_when_nothing_does() {
    let items = needs_you(&[], owner_of_run, &[], owner_of_mission, |_| None, name_of, 0, &[], None, &[]);
    assert!(items.is_empty());
}

// case: is a paused run first, then a question, then memory -- each named by its teammate
#[test]
fn is_a_paused_run_first_then_a_question_then_memory_each_named_by_its_teammate() {
    let ask_events = vec![final_msg(ASK, "m1"), completed()];
    let approvals = [
        ApprovalInput { approval_id: "a1", run_id: "run_w", mission_id: "mission_w", kind: "command", summary: "Run a command", detail: "npm test" },
        ApprovalInput { approval_id: "a2", run_id: "run_x", mission_id: "mission_x", kind: "question", summary: "Which branch?", detail: "" },
        ApprovalInput { approval_id: "a3", run_id: "run_w", mission_id: "mission_w", kind: "file-change", summary: "Change src/app.ts", detail: "@@ -1 +1 @@" },
    ];
    let conversations = [
        ConversationInput { mission_id: "mission_p", phase: "completed", waiting_question: None },
        ConversationInput { mission_id: "mission_running", phase: "running", waiting_question: None },
        ConversationInput { mission_id: "mission_failed", phase: "failed", waiting_question: None },
        ConversationInput { mission_id: "mission_unread", phase: "completed", waiting_question: None },
    ];
    let items = needs_you(
        &approvals,
        owner_of_run,
        &conversations,
        owner_of_mission,
        |id| matches!(id, "mission_p" | "mission_running" | "mission_failed").then_some(ask_events.as_slice()),
        name_of,
        3,
        &[],
        None,
        &[],
    );
    let keys: Vec<&str> = items.iter().map(NeedsYouItem::key).collect();
    assert_eq!(keys, ["approval:a1", "approval:a2", "approval:a3", "decision:mission_p", "memory"]);
    let labels: Vec<String> = items.iter().map(needs_you_label).collect();
    assert_eq!(
        labels,
        [
            "Wren wants to run: npm test",
            "A teammate is asking: Which branch?",
            "Wren needs your approval: Change src/app.ts",
            "Pip asked: Which port should the API use?",
            "3 memory suggestions waiting",
        ]
    );
}

// case: says each thing in one line, short enough to be a menu row
#[test]
fn says_each_thing_in_one_line_short_enough_to_be_a_menu_row() {
    let long = format!("echo\n{}", "x".repeat(300));
    let approvals = [ApprovalInput {
        approval_id: "a1",
        run_id: "run_w",
        mission_id: "m",
        kind: "command",
        summary: "Run a command",
        detail: &long,
    }];
    let items = needs_you(&approvals, owner_of_run, &[], owner_of_mission, |_| None, name_of, 0, &[], None, &[]);
    let what = match &items[0] {
        NeedsYouItem::Approval { what, .. } => what.as_str(),
        other => panic!("expected approval, got {other:?}"),
    };
    assert!(what.chars().count() <= NEEDS_YOU_TEXT, "len={}", what.chars().count());
    assert!(!what.contains('\n'));
    assert!(what.ends_with('…'));
}

// case: counts a card that needs you now, and a copy waiting for Keep or Discard, after a question and before memory
#[test]
fn counts_a_card_that_needs_you_now_and_a_copy_waiting_for_keep_or_discard() {
    let ask_events = vec![final_msg(ASK, "m1"), completed()];
    let approvals = [ApprovalInput {
        approval_id: "a1",
        run_id: "run_w",
        mission_id: "mission_w",
        kind: "command",
        summary: "Run a command",
        detail: "npm test",
    }];
    let conversations = [ConversationInput { mission_id: "mission_p", phase: "completed", waiting_question: None }];
    let scout_cards = [
        ScoutCardInput { card_id: "card_soon", urgency: "soon", said: "A review can wait." },
        ScoutCardInput { card_id: "card_now", urgency: "now", said: "Atlas has waited 25 minutes for your OK." },
        ScoutCardInput { card_id: "card_later", urgency: "later", said: "This can wait." },
        ScoutCardInput { card_id: "  ", urgency: "now", said: "No card." },
    ];
    let routines = [
        RoutineInput { routine_id: "rt_idle", name: "Idle sweep", files: None },
        RoutineInput { routine_id: "rt_inbox", name: "Morning digest", files: Some(2) },
        RoutineInput { routine_id: "rt_empty", name: "Quiet pass", files: Some(0) },
    ];
    let items = needs_you(
        &approvals,
        owner_of_run,
        &conversations,
        owner_of_mission,
        |_| Some(ask_events.as_slice()),
        name_of,
        1,
        &scout_cards,
        Some("Scout"),
        &routines,
    );
    let keys: Vec<&str> = items.iter().map(NeedsYouItem::key).collect();
    assert_eq!(keys, ["approval:a1", "decision:mission_p", "scout:card_now", "routine:rt_inbox", "routine:rt_empty", "memory"]);
    let labels: Vec<String> = items.iter().map(needs_you_label).collect();
    assert_eq!(
        labels,
        [
            "Wren wants to run: npm test",
            "Pip asked: Which port should the API use?",
            "Scout noticed: Atlas has waited 25 minutes for your OK.",
            "Morning digest: Waiting for you · 2 files in its copy",
            "Quiet pass: Waiting for you",
            "1 memory suggestion waiting",
        ]
    );
    let places: Vec<NeedsYouPlace> = items.iter().map(needs_you_place).collect();
    assert_eq!(
        places,
        [
            NeedsYouPlace::Conversation { mission_id: "mission_w".into() },
            NeedsYouPlace::Conversation { mission_id: "mission_p".into() },
            NeedsYouPlace::Home { card_id: "card_now".into() },
            NeedsYouPlace::Routines { routine_id: "rt_inbox".into() },
            NeedsYouPlace::Routines { routine_id: "rt_empty".into() },
            NeedsYouPlace::Memory,
        ]
    );
}

// case: says Scout when the card has no name of its own, and leaves soon and later off the list
#[test]
fn says_scout_when_the_card_has_no_name_of_its_own_and_leaves_soon_and_later_off_the_list() {
    let scout_cards = [
        ScoutCardInput { card_id: "card_soon", urgency: "soon", said: "Later is fine." },
        ScoutCardInput { card_id: "card_now", urgency: "now", said: "Checks failed." },
    ];
    let items = needs_you(&[], owner_of_run, &[], owner_of_mission, |_| None, name_of, 0, &scout_cards, None, &[]);
    let labels: Vec<String> = items.iter().map(needs_you_label).collect();
    assert_eq!(labels, ["Scout noticed: Checks failed."]);
}

// case: reads as a sentence
#[test]
fn reads_as_a_sentence() {
    assert_eq!(needs_you_chip(1), "1 needs you");
    assert_eq!(needs_you_chip(3), "3 need you");
    assert_eq!(
        needs_you_label(&NeedsYouItem::Memory { key: "memory".into(), count: 1 }),
        "1 memory suggestion waiting"
    );
}

//! Electron's `main/an-app-picks-up-a-turn-locust-closed-under.test.ts` (0.882), the host half:
//! `carry_with` picks up a turn Locust closed under (on its own agent too, told to reconcile what it cannot confirm),
//! leaves the window's `carry` card press refusing it, moves a finished conversation to another agent with a message,
//! refuses a finished move without a message or onto its own agent, and appends a message after a ran-out carry-on's words.
use super::provider_carry_on::{carried_from, stopped_in, team};
use super::*;
use crate::carry_on::{CarryOnAlso, cut_off_prompt};

fn started_ev() -> Value {
    json!({"type": "run.started", "payload": {}})
}

fn completed_ev() -> Value {
    json!({"type": "run.completed", "payload": {}})
}

fn quota_ev() -> Value {
    json!({"type": "route.limit_detected", "payload": {"kind": "quota-exhausted"}})
}

// case: picks it up on its own agent too, told to reconcile what it cannot confirm
#[test]
fn picks_it_up_on_its_own_agent_too_told_to_reconcile_what_it_cannot_confirm() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, Value::Null);
    stopped_in(&t, "mission_cutoff_own", "codex", "ask", &[started_ev()]);
    let (id, _, _) = carry_with(
        "mission_cutoff_own",
        "codex",
        None,
        &CarryOnAlso { cut_off: true, message: None },
    )
    .unwrap();
    finished(&id);
    let started = carried_from(&t, "mission_cutoff_own");
    assert_eq!(started.len(), 1);
    assert_eq!(started[0]["runtime"], "codex");
    assert_eq!(started[0]["continuesFrom"]["missionId"], "mission_cutoff_own");
    assert_eq!(started[0]["prompt"], cut_off_prompt("codex"));
    let words = cut_off_prompt("codex");
    assert!(words.contains("Locust closed while it was working"), "{words}");
    assert!(words.contains("never retry an irreversible action"), "{words}");
}

// case: leaves the window’s card as it was: without the app’s word a cut-off turn is not carried on
#[test]
fn leaves_the_windows_card_as_it_was_without_the_apps_word_a_cut_off_turn_is_not_carried_on() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, Value::Null);
    stopped_in(&t, "mission_cutoff_card", "codex", "ask", &[started_ev()]);
    assert!(carry("mission_cutoff_card", "claude", None).is_err());
    assert!(carried_from(&t, "mission_cutoff_card").is_empty());
}

// case: sends the message as the next turn on the other agent, with what was done before it
#[test]
fn sends_the_message_as_the_next_turn_on_the_other_agent_with_what_was_done_before_it() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, Value::Null);
    stopped_in(&t, "mission_finished_move", "codex", "ask", &[started_ev(), completed_ev()]);
    let (id, _, _) = carry_with(
        "mission_finished_move",
        "claude",
        None,
        &CarryOnAlso { cut_off: false, message: Some("Review what Codex wrote.".into()) },
    )
    .unwrap();
    finished(&id);
    let started = carried_from(&t, "mission_finished_move");
    assert_eq!(started.len(), 1);
    assert_eq!(started[0]["runtime"], "claude");
    let prompt = started[0]["prompt"].as_str().unwrap();
    assert!(prompt.contains("taking over this conversation from another agent (Codex CLI), which finished its last turn"), "{prompt}");
    assert!(prompt.ends_with("\n\nThe person now asks:\n\nReview what Codex wrote."), "{prompt}");
    assert_eq!(
        crate::carry_on::carry_decision(
            Some("completed"),
            &[started_ev(), completed_ev()],
            "codex",
            "claude",
            "ask",
            &CarryOnAlso { cut_off: false, message: Some("Review what Codex wrote.".into()) },
        )
        .unwrap()
        .prompt,
        "Review what Codex wrote."
    );
    assert_eq!(started[0]["continuesFrom"]["missionId"], "mission_finished_move");
    assert_eq!(started[0]["continuesFrom"]["reason"], "route-switch");
    assert_eq!(started[0]["startedBy"], json!({"kind": "carry-on", "from": "codex"}));
}

// case: needs the message, and sends one on its own agent the plain way
#[test]
fn needs_the_message_and_sends_one_on_its_own_agent_the_plain_way() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, Value::Null);
    stopped_in(&t, "mission_finished_no_msg", "codex", "ask", &[started_ev(), completed_ev()]);
    let without = carry("mission_finished_no_msg", "claude", None).unwrap_err();
    assert!(without.contains("finished") && without.contains("give a message"), "{without}");

    stopped_in(&t, "mission_finished_same", "codex", "ask", &[started_ev(), completed_ev()]);
    let same = carry_with(
        "mission_finished_same",
        "codex",
        None,
        &CarryOnAlso { cut_off: false, message: Some("More.".into()) },
    )
    .unwrap_err();
    assert!(same.contains("already runs this conversation"), "{same}");
    assert!(carried_from(&t, "mission_finished_no_msg").is_empty());
    assert!(carried_from(&t, "mission_finished_same").is_empty());
}

// case: passes the message through the door, and puts it after the carry-on words of a turn that ran out
#[test]
fn puts_the_message_after_the_carry_on_words_of_a_turn_that_ran_out() {
    let t = Temp::new();
    let _g = hooks::install(t.store(), fake("success"));
    team(&t, Value::Null);
    stopped_in(&t, "mission_ran_out_msg", "codex", "ask", &[started_ev(), quota_ev()]);
    let (id, _, _) = carry_with(
        "mission_ran_out_msg",
        "claude",
        None,
        &CarryOnAlso { cut_off: false, message: Some("Then push it.".into()) },
    )
    .unwrap();
    finished(&id);
    let started = carried_from(&t, "mission_ran_out_msg");
    assert_eq!(started.len(), 1);
    let prompt = started[0]["prompt"].as_str().unwrap();
    assert!(prompt.contains("ran out of its plan's usage"), "{prompt}");
    assert!(prompt.ends_with("\n\nThen push it."), "{prompt}");
}

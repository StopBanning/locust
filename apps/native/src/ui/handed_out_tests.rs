//! Electron's `renderer/src/a-lead-shows-what-it-handed-out.test.tsx`, drawn in the whole window: under a lead's turn,
//! a card says who has each ask it handed out and how it is going, beside the exchanges it sums up.
use super::*;
use rune::{Env, Runtime};

const LEAD: &str = "mis_lead";

fn posted(message_id: &str, to: (&str, &str), words: &str) -> (ledger::PeerLink, locust_core::workroom::Message) {
    let link = ledger::PeerLink { direction: "posted".into(), message_id: message_id.into(), peer_teammate_id: to.0.into(), occurred_at: "2026-10-25T12:00:00.000Z".into() };
    let message = locust_core::workroom::Message {
        message_id: message_id.into(),
        sequence: 0,
        from: locust_core::workroom::Party { teammate_id: "tm_bro".into(), name: "Bro".into() },
        from_mission: LEAD.into(),
        to: locust_core::workroom::Party { teammate_id: to.0.into(), name: to.1.into() },
        text: words.into(),
        posted_at: "2026-10-25T12:00:00.000Z".into(),
        observed: None,
    };
    (link, message)
}

const CASPER: (&str, &str) = ("tm_casper", "Casper");
const TOASTER: (&str, &str) = ("tm_toaster", "Toaster");
const FIX: &str = "Fix the login test.\nKeep the existing sign-in behavior.";

/// The turn `mission` that received `message_id`, as the window's summaries hold it.
fn delivered(s: &mut State, message_id: &str, mission: &str, running: bool) {
    let sum = s.ledger_summaries.entry(mission.into()).or_insert_with(|| ledger::Summary { mission_id: mission.into(), ..Default::default() });
    sum.running = running;
    sum.peer_links.push(ledger::PeerLink { direction: "received".into(), message_id: message_id.into(), peer_teammate_id: "tm_bro".into(), occurred_at: String::new() });
}

/// Bro's conversation, its turns' shares posted from `LEAD`; with `earlier`, a second turn asked after it.
fn window(shares: &[(&str, (&str, &str), &str)], earlier: bool, prepare: impl FnOnce(&mut State)) -> Runtime<State> {
    let mut s = State::default();
    let header = Teammate { id: "tm_bro".into(), name: "Bro".into(), role: "Lead".into(), model: None, runtime: Some("claude".into()), folder: None, runs_in: "ask".into(), account: None, busy: false };
    s.teammates.push(header.clone());
    s.conversations.insert("tm_bro".into(), Default::default());
    let mut items = vec![
        ledger::Item::User("Please hand out the work".into()),
        ledger::Item::Assistant { item_id: "a1".into(), text: "Handed out.".into(), r#final: true },
    ];
    let mut metas = vec![ledger::Metadata { mission_id: LEAD.into(), ..Default::default() }];
    if earlier {
        items.push(ledger::Item::User("What comes next?".into()));
        items.push(ledger::Item::Assistant { item_id: "a2".into(), text: "Waiting on them.".into(), r#final: true });
        metas.push(ledger::Metadata { mission_id: "mis_next".into(), parent: Some(LEAD.into()), ..Default::default() });
    }
    let mut peer_links = Vec::new();
    for (id, to, words) in shares {
        let (link, message) = posted(id, *to, words);
        peer_links.push((LEAD.to_owned(), link));
        s.workroom_messages.insert((*id).into(), message);
    }
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header,
        conversation: Conversation { conversation_id: Some(LEAD.into()), turns: Vec::new() },
        items,
        dir: std::path::PathBuf::new(),
        worker: None,
        snapshot: ledger::Snapshot { turn_metas: metas, peer_links, ..Default::default() },
        due: 0.0,
        loading: false,
        after_tip: None,
        after_tip_deadline: None,
        backoff: 0.0,
    });
    prepare(&mut s);
    let mut rt = Runtime::new(s, crate::view, (1400.0, 900.0), Env::new(true, 1.0));
    for _ in 0..10 {
        rt.frame(0.05);
    }
    rt
}

fn keyed(rt: &Runtime<State>, prefix: &str) -> Vec<String> {
    rt.elements().into_iter().filter_map(|e| e.key).filter(|k| k.starts_with(prefix)).collect()
}

fn text_of(rt: &Runtime<State>, key: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).and_then(|e| e.text.or(e.label))
}

/// The chip's word for the to-do from `message_id`.
fn state(rt: &Runtime<State>, message_id: &str) -> Option<String> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(&format!("handed-out-state-{message_id}"))).and_then(|e| e.label)
}

#[test]
fn lists_two_delegations_with_their_live_states_and_open_retaining_the_full_exchange() {
    // case: lists two delegations with their live states and Open, retaining the full exchange
    let rt = window(&[("msg_fix", CASPER, FIX), ("msg_review", TOASTER, "Review the migration")], false, |s| {
        delivered(s, "msg_fix", "mis_fix", true);
        delivered(s, "msg_review", "mis_review", true);
        s.home.waiting_for.insert("mis_review".into(), "waiting on you".into());
    });
    assert_eq!(text_of(&rt, &format!("handed-out-label-{LEAD}")).as_deref(), Some("Handed out \u{b7} 2 to-dos"));
    assert_eq!(text_of(&rt, "handed-out-who-msg_fix").as_deref(), Some("Casper"));
    assert_eq!(text_of(&rt, "handed-out-who-msg_review").as_deref(), Some("Toaster"));
    // The whole ask is its hover, however it is cut on the line.
    let task = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("handed-out-task-msg_fix")).expect("the ask");
    assert_eq!(task.help.as_deref(), Some(FIX));
    assert_eq!(state(&rt, "msg_fix").as_deref(), Some("Working"));
    assert_eq!(state(&rt, "msg_review").as_deref(), Some("Needs you"));
    assert_eq!(keyed(&rt, "handed-out-open-"), ["handed-out-open-msg_fix", "handed-out-open-msg_review"]);
    // The exchanges it sums up are still there, one per teammate.
    assert_eq!(keyed(&rt, "peer-toggle-").len(), 2, "{:?}", keyed(&rt, "peer-"));
}

#[test]
fn a_to_dos_open_opens_the_turn_that_received_it() {
    let mut rt = window(&[("msg_fix", CASPER, FIX)], false, |s| delivered(s, "msg_fix", "mis_fix", false));
    let (x, y) = rt.find("handed-out-open-msg_fix").expect("Open");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..5 {
        rt.frame(0.05);
    }
    assert_eq!(rt.state.ledger.as_ref().and_then(|pane| pane.conversation.conversation_id.clone()).as_deref(), Some("mis_fix"));
}

#[test]
fn adds_no_card_when_the_turn_handed_nothing_out_including_an_incoming_message() {
    // case: adds no card when the turn handed nothing out, including an incoming message
    let rt = window(&[], false, |_| {});
    assert!(keyed(&rt, "handed-out-").is_empty());
    let rt = window(&[("msg_fix", CASPER, FIX)], false, |s| {
        if let Some(pane) = s.ledger.as_mut() {
            pane.snapshot.peer_links[0].1.direction = "received".into();
        }
    });
    assert!(keyed(&rt, "handed-out-").is_empty(), "{:?}", keyed(&rt, "handed-out-"));
    assert!(!keyed(&rt, "peer-toggle-").is_empty(), "the message itself is still shown");
}

#[test]
fn reads_done_when_the_receiving_turn_is_completed_from_the_same_state_as_a_call() {
    // case: reads Done when the receiving turn is completed, from the same state as a call
    let rt = window(&[("msg_fix", CASPER, FIX)], false, |s| delivered(s, "msg_fix", "mis_fix", false));
    assert_eq!(text_of(&rt, &format!("handed-out-label-{LEAD}")).as_deref(), Some("Handed out \u{b7} 1 to-do"));
    assert_eq!(state(&rt, "msg_fix").as_deref(), Some("Done"));
    // Green, as Done is everywhere a call's to-do says it.
    let l = crate::look::controls::Look::new(true);
    let chip = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("handed-out-state-msg_fix")).unwrap();
    assert_eq!(chip.fill, l.c("green-tint"));
    let failed = window(&[("msg_fix", CASPER, FIX)], false, |s| {
        delivered(s, "msg_fix", "mis_fix", false);
        s.ledger_summaries.get_mut("mis_fix").unwrap().failed = true;
    });
    assert_eq!(state(&failed, "msg_fix").as_deref(), Some("Failed"));
    let stopped = window(&[("msg_fix", CASPER, FIX)], false, |s| {
        delivered(s, "msg_fix", "mis_fix", false);
        s.ledger_summaries.get_mut("mis_fix").unwrap().cancelled = true;
    });
    assert_eq!(state(&stopped, "msg_fix").as_deref(), Some("Stopped"));
}

#[test]
fn keeps_an_earlier_turns_card_under_that_turn_without_repeating_it_under_the_newest() {
    // case: keeps an earlier turn’s card under that turn, without repeating it under the newest
    let rt = window(&[("msg_fix", CASPER, FIX)], true, |s| delivered(s, "msg_fix", "mis_fix", false));
    assert_eq!(keyed(&rt, "handed-out-label-"), [format!("handed-out-label-{LEAD}")]);
    let card = rt.presented(&format!("handed-out-{LEAD}")).expect("the card").0;
    let next = rt.elements().into_iter().find(|e| e.text.as_deref() == Some("What comes next?")).expect("the next ask").rect;
    let reply = rt.elements().into_iter().find(|e| e.text.as_deref() == Some("Handed out.")).expect("the lead reply").rect;
    assert!(card[1] >= reply[1] + reply[3], "card {card:?} must follow reply {reply:?}");
    assert!(card[1] < next[1], "card at {card:?}, next ask at {next:?}");
}

#[test]
fn shows_an_undelivered_share_as_starting_without_an_open_button() {
    // case: shows an undelivered share as Starting without an Open button
    let rt = window(&[("msg_fix", CASPER, FIX)], false, |_| {});
    assert_eq!(text_of(&rt, &format!("handed-out-label-{LEAD}")).as_deref(), Some("Handed out \u{b7} 1 to-do"));
    assert_eq!(state(&rt, "msg_fix").as_deref(), Some("Starting"));
    assert!(keyed(&rt, "handed-out-open-").is_empty());
}

#[test]
fn keeps_separate_asks_delivered_to_the_same_turn_rather_than_losing_one_row() {
    // case: keeps separate asks delivered to the same turn, rather than losing one row
    let rt = window(&[("msg_fix", CASPER, FIX), ("msg_more", CASPER, "Also check the signed-out case")], false, |s| {
        delivered(s, "msg_fix", "mis_fix", true);
        delivered(s, "msg_more", "mis_fix", true);
    });
    assert_eq!(text_of(&rt, &format!("handed-out-label-{LEAD}")).as_deref(), Some("Handed out \u{b7} 2 to-dos"));
    assert_eq!(keyed(&rt, "handed-out-todo-"), ["handed-out-todo-msg_fix", "handed-out-todo-msg_more"]);
    assert_eq!(keyed(&rt, "handed-out-open-").len(), 2);
    assert_eq!(text_of(&rt, "handed-out-task-msg_more").as_deref(), Some("Also check the signed-out case"));
}

#[test]
fn a_share_received_by_the_turn_on_screen_is_found_before_the_history_has_it() {
    // The live turn's own record names the delivery before any summary does (peerRunFor over the live missions).
    let rt = window(&[("msg_fix", CASPER, FIX)], false, |s| {
        let pane = s.ledger.as_mut().unwrap();
        pane.snapshot.peer_links.push(("mis_live".into(), ledger::PeerLink { direction: "received".into(), message_id: "msg_fix".into(), peer_teammate_id: "tm_bro".into(), occurred_at: String::new() }));
    });
    assert_eq!(keyed(&rt, "handed-out-open-"), ["handed-out-open-msg_fix"]);
}

#[test]
fn a_live_receiving_turn_outranks_a_stale_completed_summary() {
    let rt = window(&[("msg_fix", CASPER, FIX)], false, |s| {
        delivered(s, "msg_fix", "mis_fix", false);
        s.seen_live.push("mis_fix".into());
    });
    assert_eq!(state(&rt, "msg_fix").as_deref(), Some("Working"));
}

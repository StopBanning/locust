//! Electron `a-tables-status-is-a-chip.test.tsx`, case 4: where the thread has a box, a status is a chip you can
//! change, naming its row (StatusChip.tsx, 0.733).
use super::*;
use rune::{Env, Runtime};

const TABLE: &str = "| Task | Status |\n| --- | --- |\n| **Billing** rename | Blocked |\n";

fn in_thread(s: &State, _: &mut Cx<State>) -> El {
    reply_in_thread(s, TABLE, false, false, &Palette::new(true))
}
fn in_preview(s: &State, _: &mut Cx<State>) -> El {
    reply_with_copy(s, TABLE, false, false, &Palette::new(true))
}
fn drawn(s: State, can_draft: bool) -> Runtime<State> {
    let mut rt = Runtime::new(s, if can_draft { in_thread } else { in_preview }, (800.0, 600.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

/// The chip's address, as the reply's Markdown carries it.
fn chip_link() -> String {
    let prepared = crate::markdown_prep::prepare_reply(TABLE, true).markdown;
    let at = prepared.find(crate::markdown_prep::STATUS_MARK).expect("a status chip");
    prepared[at..].split(')').next().unwrap().to_owned()
}

#[test]
fn is_a_chip_you_can_change_where_the_thread_has_a_box_naming_its_row_0_733() {
    // The chip names its row by its first other cell, as Electron's plainCell reads it.
    let link = chip_link();
    assert_eq!(crate::markdown_prep::status_of_link(&link), Some(("red".to_owned(), "Billing rename".to_owned())));
    // A press where the thread has the box opens the other statuses, named for the row.
    let mut s = State::default();
    status_pressed(&mut s, &link, true, body_key(TABLE));
    let mut rt = drawn(s, true);
    let said: Vec<String> = rt.elements().into_iter().filter_map(|e| e.text).collect();
    assert!(said.iter().any(|t| t == "Change the status of Billing rename"), "{said:?}");
    for other in ["Not started", "In progress", "Waiting", "Done"] {
        assert!(said.iter().any(|t| t == other), "{other}: {said:?}");
    }
    assert!(rt.find("status-change-red").is_none(), "its own status is not offered");
    // Picking one writes Electron's words into the box, for the person to send.
    let (x, y) = rt.find("status-change-green").expect("Done");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.0);
    assert_eq!(rt.state.draft.text, "Mark \"Billing rename\" as Done in the tracker, and say what that changes.");
    assert_eq!(crate::markdown_prep::status_ask("Billing rename", "Done"), "Mark \"Billing rename\" as Done in the tracker, and say what that changes.");
    assert!(rt.state.status_change.is_none(), "the strip closes once one is picked");
}

#[test]
fn where_there_is_no_box_the_chip_is_the_plain_chip_it_was() {
    let mut s = State::default();
    status_pressed(&mut s, &chip_link(), false, body_key(TABLE));
    assert!(s.status_change.is_none());
    s.status_change = Some(StatusChange { reply: body_key(TABLE), tone: "red".into(), item: "Billing rename".into() });
    let rt = drawn(s, false);
    assert!(rt.find("status-change").is_none());
}

#[test]
fn a_second_press_on_the_chip_closes_what_the_first_opened() {
    let mut s = State::default();
    status_pressed(&mut s, &chip_link(), true, body_key(TABLE));
    status_pressed(&mut s, &chip_link(), true, body_key(TABLE));
    assert!(s.status_change.is_none());
}

fn thread(s: &State, _: &mut Cx<State>) -> El {
    let items = [locust_core::ledger::Item::Assistant { item_id: "a".into(), text: TABLE.into(), r#final: true }];
    crate::messages::message_column(&items, "Locust", &Palette::new(true), s)
}

/// The conversation's thread is where the box is: its replies are the changeable kind.
#[test]
fn the_conversations_thread_is_where_a_status_can_be_changed() {
    let mut s = State::default();
    // The thread draws the reply's shown text (protocol blocks taken out), so that is what the chip's press names.
    let shown = locust_core::protocol_tags::shown_text(TABLE);
    s.status_change = Some(StatusChange { reply: body_key(&shown), tone: "red".into(), item: "Billing rename".into() });
    let mut rt = Runtime::new(s, thread, (800.0, 600.0), Env::new(true, 1.0));
    rt.frame(0.0);
    assert!(rt.find("status-change").is_some());
}

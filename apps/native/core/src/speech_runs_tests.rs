//! `Thread.tsx`'s face-per-run and Copy-once rules.

use super::{continues_speech, last_speech};
use crate::ledger::{Item, Status, ToolDetail};

fn say(words: &str) -> Item {
    Item::Assistant { item_id: words.into(), text: words.into(), r#final: true }
}
fn step() -> Item {
    Item::Tool { name: "Read".into(), status: Status::Done, summary: "a.ts".into(), detail: Box::new(ToolDetail::default()) }
}
fn shown(items: &[Item]) -> impl Fn(usize) -> bool + '_ {
    |i| matches!(&items[i], Item::Assistant { text, .. } if !text.trim().is_empty())
}

#[test]
fn a_turn_that_speaks_three_times_wears_its_face_once() {
    let items = [Item::User("Make the file.".into()), say("Making your HELLO file"), say("File created"), say("DONE")];
    let faces: Vec<bool> = (1..4).map(|i| !continues_speech(&items, i, shown(&items))).collect();
    assert_eq!(faces, [true, false, false]);
}

#[test]
fn the_steps_between_two_replies_do_not_start_a_new_run() {
    let thought = Item::Thought { id: "t".into(), text: "**Checking**".into(), duration_ms: None };
    let items = [say("Reading it."), step(), thought, step(), say("Read.")];
    assert!(continues_speech(&items, 4, shown(&items)));
}

#[test]
fn the_persons_words_or_a_note_between_them_start_a_new_run() {
    let items = [say("One."), Item::User("And?".into()), say("Two."), Item::Note { level: "warning".into(), text: "n".into() }, say("Three.")];
    assert!(!continues_speech(&items, 2, shown(&items)));
    assert!(!continues_speech(&items, 4, shown(&items)));
}

#[test]
fn a_plan_is_speech_and_an_empty_reply_is_not() {
    let plan = Item::Plan { id: "p".into(), steps: Vec::new(), finished: true, stopped: false };
    let items = [plan, say("Done.")];
    assert!(continues_speech(&items, 1, shown(&items)));
    let items = [Item::User("Go.".into()), say("  "), say("Done.")];
    assert!(!continues_speech(&items, 2, shown(&items)), "a reply with nothing shown heads nothing");
}

#[test]
fn copy_sits_under_the_last_reply_once_nothing_is_going() {
    let items = [say("One."), step(), say("Two."), step()];
    assert_eq!(last_speech(&items, false, shown(&items)), Some(2));
    assert_eq!(last_speech(&items, true, shown(&items)), None, "not while the turn runs");
    let streaming = [say("One."), Item::Assistant { item_id: "s".into(), text: "Still".into(), r#final: false }];
    assert_eq!(last_speech(&streaming, false, shown(&streaming)), None, "not on a reply still arriving");
    let empty = [say("One."), say(" ")];
    assert_eq!(last_speech(&empty, false, shown(&empty)), Some(0));
}

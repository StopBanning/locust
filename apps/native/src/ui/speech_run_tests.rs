//! `Thread.tsx` drawn: one face per run of speech, and Copy once, under the turn's last reply.

use super::*;

fn drawn(items: &[ledger::Item], s: State) -> (usize, usize) {
    let mut rt = rune::Runtime::new(
        (s, items.to_vec()),
        |(s, items), _| message_column(items, "Codec", &Palette::new(true), s),
        (800.0, 1200.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    let elements = rt.elements();
    let names = elements.iter().filter(|e| e.text.as_deref() == Some("Codec")).count();
    // In sight at rest: an older turn's last reply keeps a Copy too, hidden until the pointer is on it
    // (REVIEW-ui-changes-2026-11-26.md change 8, reply_copy_tests.rs).
    let copies = elements.iter().filter(|e| e.label.as_deref() == Some("Copy reply") && e.opacity > 0.5).count();
    (names, copies)
}

fn say(id: &str, words: &str) -> ledger::Item {
    ledger::Item::Assistant { item_id: id.into(), text: words.into(), r#final: true }
}

fn step(id: &str) -> ledger::Item {
    ledger::Item::Tool {
        name: "Read".into(),
        status: ledger::Status::Done,
        summary: "a.ts".into(),
        detail: Box::new(ledger::ToolDetail { id: id.into(), finished: true, ..Default::default() }),
    }
}

#[test]
fn a_turn_that_speaks_between_its_steps_names_its_teammate_once_and_copies_once() {
    let items = [
        ledger::Item::User("Make the file.".into()),
        say("a", "Making your HELLO file."),
        step("t1"),
        say("b", "File created."),
        step("t2"),
        say("c", "DONE"),
    ];
    assert_eq!(drawn(&items, State::default()), (1, 1));
}

#[test]
fn each_turn_names_its_teammate_and_has_its_own_copy_only_at_the_end() {
    let items = [
        ledger::Item::User("One?".into()),
        say("a", "One."),
        ledger::Item::User("Two?".into()),
        say("b", "Two."),
    ];
    let (names, copies) = drawn(&items, State::default());
    assert_eq!(names, 2, "the person's words start a new run");
    assert_eq!(copies, 1, "Copy sits under the last reply only");
}

//! Copy under the replies, found by name in the drawn window (REVIEW-ui-changes-2026-11-26.md change 8, Colin's yes
//! 26 Nov): in sight at rest under the newest reply; under an older turn's last reply only while the pointer is on it,
//! as Electron's `.lc-agentline__copy` (Thread.tsx:324-345, shell.css:23083-23105); none under a reply that is not its
//! turn's last.

use super::*;

fn two_turns(s: &mut State) {
    crate::scenes::ledger_scene(s, false, false);
    if let Some(pane) = s.ledger.as_mut() {
        pane.items = vec![
            ledger::Item::User("What does total() do?".into()),
            ledger::Item::Assistant { item_id: "first".into(), text: "Reading the cart first.".into(), r#final: true },
            ledger::Item::Tool {
                name: "Read".into(),
                status: ledger::Status::Done,
                summary: "cart.py".into(),
                detail: Box::new(ledger::ToolDetail { id: "t1".into(), finished: true, ..Default::default() }),
            },
            ledger::Item::Assistant { item_id: "older".into(), text: "It adds up the cart.".into(), r#final: true },
            ledger::Item::User("And tax?".into()),
            ledger::Item::Assistant { item_id: "newest".into(), text: "Tax is added after the discount.".into(), r#final: true },
        ];
        pane.snapshot.items = pane.items.clone();
        pane.snapshot.running = false;
        pane.snapshot.live = None;
    }
    s.flight = None;
}

fn window() -> rune::Runtime<State> {
    let mut s = State::default();
    two_turns(&mut s);
    let mut rt = rune::Runtime::new(s, |s, cx| crate::view(s, cx), (1215.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.016);
    rt
}

fn starting(rt: &rune::Runtime<State>, prefix: &str) -> Vec<rune::ElementInfo> {
    rt.elements().into_iter().filter(|e| !e.exiting && e.key.as_deref().is_some_and(|k| k.starts_with(prefix))).collect()
}

fn said(rt: &rune::Runtime<State>, words: &str) -> rune::ElementInfo {
    rt.elements().into_iter().find(|e| e.text.as_deref().is_some_and(|t| t.contains(words))).unwrap_or_else(|| panic!("no {words:?}"))
}

#[test]
fn the_newest_reply_shows_copy_at_rest() {
    let rt = window();
    let newest = starting(&rt, "reply-copy/");
    assert_eq!(newest.len(), 1, "one Copy in sight at rest");
    assert!(newest[0].rect[1] > said(&rt, "Tax is added").rect[1], "under the newest reply");
    assert!(newest[0].opacity > 0.99, "{}", newest[0].opacity);
}

#[test]
fn an_older_turns_last_reply_has_copy_and_a_reply_before_it_in_its_turn_does_not() {
    let rt = window();
    let older = starting(&rt, "reply-copy-hover/");
    assert_eq!(older.len(), 1, "one hidden Copy, for the older turn's last reply only");
    let last = said(&rt, "It adds up the cart.");
    let first = said(&rt, "Reading the cart first.");
    let next = said(&rt, "And tax?");
    assert!(older[0].rect[1] >= last.rect[1] + last.rect[3], "under the turn's last reply");
    assert!(older[0].rect[1] > first.rect[1] + first.rect[3] + 1.0, "not under the reply before it");
    assert!(older[0].rect[1] + older[0].rect[3] <= next.rect[1] + next.rect[3], "in the gap, not pushing the next turn down");
    assert!(older[0].opacity < 0.01, "hidden at rest: {}", older[0].opacity);
}

#[test]
fn an_older_turns_copy_shows_while_the_pointer_is_on_its_reply() {
    let mut rt = window();
    let reply = said(&rt, "It adds up the cart.");
    rt.pointer_move(reply.rect[0] + 4.0, reply.rect[1] + reply.rect[3] / 2.0);
    for n in 0..20 {
        rt.frame(0.016 * (n + 2) as f64);
    }
    let copy = starting(&rt, "reply-copy-hover/").remove(0);
    assert!(copy.opacity > 0.99, "shown under the pointer: {}", copy.opacity);
}

use super::*;
use rune::{Env, Runtime};

// Native part of the cards case: Start all is not supported (one active turn).
#[test]
fn a_finished_reply_draws_each_suggested_task_and_puts_away_only_the_pressed_one() {
    let mut s = State::default();
    crate::scenes::ledger_scene(&mut s, false, false);
    let pane = s.ledger.as_mut().unwrap();
    pane.items = vec![
        ledger::Item::User("Clean up the update images.".into()),
        ledger::Item::Assistant {
            item_id: "parity-next".into(),
            r#final: true,
            text: "Done.\n\n<locust-next>\nClean the Linux cache too :: Keep the waiting update.\nTell people in What's new :: Add a changelog line.\n</locust-next>".into(),
        },
    ];
    pane.snapshot.running = false;
    pane.after_tip = None;
    s.flight = None;
    let mut rt = Runtime::new(s, crate::view, (1200.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.05);
    let all = rt.elements();
    for (word, count) in [
        ("2 suggested tasks", 1),
        ("Clean the Linux cache too", 1),
        ("Keep the waiting update.", 1),
        ("Tell people in What's new", 1),
        ("Add a changelog line.", 1),
        ("Do it here", 2),
        ("Start", 2),
    ] {
        let seen = all
            .iter()
            .filter(|e| !e.exiting && e.text.as_deref() == Some(word))
            .count();
        assert_eq!(seen, count, "{word}");
    }
    assert_eq!(
        all.iter()
            .filter(|e| !e.exiting
                && e.label
                    .as_deref()
                    .is_some_and(|l| l.starts_with("Put away: ")))
            .count(),
        2
    );
    let [x, y, w, h] = all
        .iter()
        .find(|el| el.label.as_deref() == Some("Put away: Clean the Linux cache too"))
        .expect("named dismiss button")
        .rect;
    rt.pointer_down(x + w / 2.0, y + h / 2.0);
    rt.pointer_up(x + w / 2.0, y + h / 2.0);
    rt.frame(0.1);
    let after = rt.elements();
    let shows = |words| {
        after
            .iter()
            .any(|e| !e.exiting && e.text.as_deref() == Some(words))
    };
    assert!(shows("1 suggested task") && shows("Tell people in What's new"));
    assert!(!shows("Clean the Linux cache too"));
}

//! a-turn-sent-back-to-fix-is-said-as-locusts.test.tsx, the line and card cases, drawn headless.
use super::*;
use locust_core::edit_check::EditCheckOutcome;
use rune::{Env, Runtime};

type Draw = Box<dyn Fn(&State, &Palette) -> El>;
thread_local! {
    static DRAW: std::cell::RefCell<Option<Draw>> = const { std::cell::RefCell::new(None) };
}
fn view(s: &State, _: &mut Cx<State>) -> El {
    DRAW.with(|d| col().w_full().child(d.borrow().as_ref().map_or_else(col, |f| f(s, &Palette::new(true)))))
}
fn words(el: impl Fn(&State, &Palette) -> El + 'static) -> String {
    DRAW.with(|d| *d.borrow_mut() = Some(Box::new(el)));
    let mut rt = Runtime::new(State::default(), view, (900.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n")
}

#[test]
fn says_what_failed_and_which_fix_it_is_with_what_was_sent_a_click_away() {
    let first = words(|s, p| check_fix_line("npm test", 1, 2, "FAIL adds two numbers", None, p, s, "fix-1"));
    assert!(first.contains("npm test failed, so Locust sent it back"), "{first}");
    assert!(first.contains("fix 1 of 2"), "{first}");
    assert!(first.contains("What Locust sent"), "{first}");
    // What was sent waits behind the click.
    assert!(!first.contains("FAIL adds two numbers"), "{first}");
    let second = words(|s, p| check_fix_line("npm test", 2, 2, "FAIL adds three numbers", None, p, s, "fix-2"));
    assert!(second.contains("npm test still failed, so Locust sent it back again"), "{second}");
    assert!(second.contains("fix 2 of 2"), "{second}");
}

#[test]
fn names_whom_it_went_back_to_when_the_conversation_has_a_teammate() {
    let first = words(|s, p| check_fix_line("npm test", 1, 2, "x", Some("Wren"), p, s, "fix-1"));
    assert!(first.contains("failed, so Locust sent it back to Wren"), "{first}");
    let second = words(|s, p| check_fix_line("npm test", 2, 2, "x", Some("Wren"), p, s, "fix-2"));
    assert!(second.contains("still failed, so Locust sent it back to Wren again"), "{second}");
}

#[test]
fn a_pass_after_a_fix_is_the_fixs_after_the_persons_own_turn_it_is_the_turns() {
    let after_fix = words(|s, p| card("m_fix2", "npm test", EditCheckOutcome::Passed, &[], false, false, Some(2), true, Some("Wren"), p, s));
    assert!(after_fix.contains("passed after fix 2."), "{after_fix}");
    let after_turn = words(|s, p| card("m_1", "npm test", EditCheckOutcome::Passed, &[], false, false, None, true, Some("Wren"), p, s));
    assert!(after_turn.contains("passed after this turn."), "{after_turn}");
}

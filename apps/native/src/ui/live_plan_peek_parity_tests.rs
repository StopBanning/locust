use super::*;
use rune::{Env, Runtime};
use serde_json::json;

fn thread(has_plan: bool) -> Runtime<State> {
    let mut s = State::default();
    crate::scenes::ledger_scene(&mut s, false, false);
    let pane = s.ledger.as_mut().unwrap();
    pane.items = vec![ledger::Item::User("Read the ledger.".into())];
    pane.snapshot.running = true;
    pane.after_tip = None;
    let events = [
        json!({"type": "run.started", "occurredAt": "2026-09-04T21:47:00.000Z", "payload": {}}),
        json!({"type": "plan.updated", "payload": {"plan": {"plan": [{"step": "Read", "status": "completed"}, {"step": "Fix", "status": "in_progress"}, {"step": "Check", "status": "pending"}]}}}),
    ];
    let mut line =
        locust_core::thread_view::live_line(&events, true, "2026-09-04T21:47:00.000Z", false)
            .unwrap();
    assert_eq!(line.detail.as_deref(), Some("step 2 of 3"));
    if !has_plan {
        line.plan = None;
    }
    pane.snapshot.live = Some(line);
    let mut rt = Runtime::new(s, crate::view, (1200.0, 900.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.05);
    rt
}

fn press_peek(rt: &mut Runtime<State>) {
    let (x, y) = rt.find("live-step-peek").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.05);
}

fn by_key<'a>(node: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    if node["key"] == key {
        return Some(node);
    }
    node["children"]
        .as_array()?
        .iter()
        .find_map(|child| by_key(child, key))
}

fn has_state(node: &serde_json::Value, state: &str) -> bool {
    node["states"]
        .as_array()
        .is_some_and(|states| states.iter().any(|value| value.as_str() == Some(state)))
}

// case: the control is a button with its label and aria-expanded
#[test]
fn a_live_plan_button_says_which_step_and_whether_it_is_open() {
    let mut rt = thread(true);
    let closed = rt.accessibility_json();
    let button = by_key(&closed, "live-step-peek").unwrap();
    assert_eq!(button["role"], "button");
    assert_eq!(button["name"], "Show the plan, step 2 of 3");
    assert!(has_state(button, "collapsed"));
    press_peek(&mut rt);
    assert!(has_state(
        by_key(&rt.accessibility_json(), "live-step-peek").unwrap(),
        "expanded"
    ));
}

// case: the card lists the steps in order with done/current/next marked
#[test]
fn pressing_the_live_plan_lists_done_current_and_next_in_order() {
    let mut rt = thread(true);
    assert!(rt.find("live-plan-peek").is_none());
    press_peek(&mut rt);
    let words: Vec<_> = rt
        .elements()
        .into_iter()
        .filter(|el| {
            el.key
                .as_deref()
                .is_some_and(|k| k.starts_with("live-plan-step-"))
        })
        .filter_map(|el| el.text)
        .collect();
    assert_eq!(words, ["Done: Read", "Current: Fix", "Next: Check"]);
}

// Native focus scopes the key handler to the button instead of registering a document listener.
#[test]
fn escape_closes_the_live_plan_after_pressing_its_button() {
    let mut rt = thread(true);
    press_peek(&mut rt);
    assert!(rt.find("live-plan-peek").is_some());
    rt.key(KeyEvent {
        key: Key::Escape,
        mods: Default::default(),
    });
    rt.frame(0.1);
    assert!(rt.find("live-plan-peek").is_none());
}

// case: no card without a plan: plain step words with no button or popover
#[test]
fn live_step_words_without_the_plan_are_not_a_button() {
    let mut rt = thread(false);
    let tree = rt.accessibility_json();
    assert!(
        by_key(&tree, "live-step-peek").is_none_or(|node| node["role"] != "button"
            && !has_state(node, "expanded")
            && !has_state(node, "collapsed"))
    );
    assert!(
        rt.elements()
            .iter()
            .any(|el| el.key.as_deref() == Some("live-step-peek")
                && el.text.as_deref() == Some(" · step 2 of 3"))
    );
    press_peek(&mut rt);
    assert!(rt.find("live-plan-peek").is_none());
}

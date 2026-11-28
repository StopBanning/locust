use super::*;
use rune::{Env, Key, Runtime};
use serde_json::Value;

fn window(with_steps: bool) -> Runtime<State> {
    let mut s = tests_live_state();
    let live = s.ledger.as_mut().unwrap().snapshot.live.as_mut().unwrap();
    live.detail = Some("step 2 of 3".into());
    live.plan = Some(if with_steps {
        [
            ("Inspect code", "done"),
            ("Apply fix", "running"),
            ("Run tests", "pending"),
        ]
        .into_iter()
        .map(|(text, state)| locust_core::thread_view::PlanStepView {
            text: text.into(),
            state,
        })
        .collect()
    } else {
        vec![]
    });
    let mut rt = Runtime::new(
        s,
        |s, _| {
            message_column(
                &[ledger::Item::User("Fix it".into())],
                "Codec",
                &Palette::new(true),
                s,
            )
        },
        (800.0, 800.0),
        Env::new(true, 1.0),
    );
    settle(&mut rt);
    rt
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..8 {
        rt.frame(0.05);
    }
}

fn press(rt: &mut Runtime<State>) {
    let (x, y) = rt.find("live-step-peek").expect("real live plan button");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn node<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    if value["key"] == key {
        return Some(value);
    }
    value["children"]
        .as_array()?
        .iter()
        .find_map(|child| node(child, key))
}

fn expanded(rt: &Runtime<State>) -> bool {
    let tree = rt.accessibility_json();
    let peek = node(&tree, "live-step-peek").unwrap();
    peek["states"]
        .as_array()
        .is_some_and(|states| states.iter().any(|s| s == "expanded"))
}

// case: the control is a button with its label and aria-expanded
#[test]
fn messages_live_plan_button_reports_its_label_and_open_state() {
    let mut rt = window(true);
    let tree = rt.accessibility_json();
    let peek = node(&tree, "live-step-peek").unwrap();
    assert_eq!(peek["role"], "button");
    assert_eq!(peek["name"], "Show the plan, step 2 of 3");
    assert!(!expanded(&rt));
    press(&mut rt);
    assert!(expanded(&rt));
}

// case: the card lists the steps in order with done/current/next marked
#[test]
fn messages_live_plan_lists_the_actual_steps_and_their_status_in_order() {
    let mut rt = window(true);
    press(&mut rt);
    assert!(rt.find("live-plan-card").is_some());
    let tree = rt.accessibility_json();
    let expected = [
        ("Inspect code", "Done"),
        ("Apply fix", "Current step"),
        ("Run tests", "Next"),
    ];
    let ys: Vec<_> = expected
        .iter()
        .enumerate()
        .map(|(index, (words, status))| {
            let key = format!("live-plan-step-{index}");
            let row = node(&tree, &key).unwrap();
            assert_eq!(row["name"], *words);
            assert_eq!(row["description"], *status);
            rt.find(&key).unwrap().1
        })
        .collect();
    assert!(ys.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(!rt.elements().iter().any(|e| {
        e.key
            .as_deref()
            .is_some_and(|k| k.starts_with("live-plan-step-orb"))
    }));
}

// case: closes on Escape and cleans up listeners
#[test]
fn messages_live_plan_escape_dismisses_the_card_and_releases_the_handler() {
    let mut rt = window(true);
    rt.on_key = Some(|s, event, _| {
        if event.key == Key::Escape {
            s.status = "window received Escape".into();
        }
        false
    });
    press(&mut rt);
    rt.key(Key::Escape.into());
    settle(&mut rt);
    assert!(rt.find("live-plan-card").is_none());
    assert_ne!(rt.state.status, "window received Escape");
    rt.key(Key::Escape.into());
    settle(&mut rt);
    assert_eq!(rt.state.status, "window received Escape");

    press(&mut rt);
    let returned_plan = rt
        .state
        .ledger
        .as_ref()
        .unwrap()
        .snapshot
        .live
        .as_ref()
        .unwrap()
        .plan
        .clone();
    rt.update(|s, _| {
        s.status.clear();
        s.ledger
            .as_mut()
            .unwrap()
            .snapshot
            .live
            .as_mut()
            .unwrap()
            .plan = None;
    });
    settle(&mut rt);
    assert!(rt.find("live-plan-card").is_none());
    assert!(
        !rt.state
            .activity_open
            .keys()
            .any(|key| key.starts_with("live-plan/"))
    );
    rt.key(Key::Escape.into());
    settle(&mut rt);
    assert_eq!(rt.state.status, "window received Escape");
    rt.update(move |s, _| {
        s.ledger
            .as_mut()
            .unwrap()
            .snapshot
            .live
            .as_mut()
            .unwrap()
            .plan = returned_plan;
    });
    settle(&mut rt);
    assert!(
        rt.find("live-plan-card").is_none(),
        "the same plan must return closed"
    );
    assert!(!expanded(&rt));
}

// case: no card without a plan: plain step words with no button or popover
#[test]
fn messages_live_plan_with_no_steps_is_plain_words() {
    let mut rt = window(false);
    let peek = rt
        .elements()
        .into_iter()
        .find(|e| e.key.as_deref() == Some("live-step-peek"))
        .unwrap();
    assert_eq!(peek.text.as_deref(), Some(" · step 2 of 3"));
    assert_ne!(peek.role, "button");
    press(&mut rt); // Plain text has no behavior.
    assert!(rt.find("live-plan-card").is_none());
}

use super::*;
use rune::{Env, Runtime};

#[test]
fn next_steps_buttons_render_disabled_when_a_turn_cannot_start() {
    let mut s = State::default();
    crate::scenes::ledger_scene(&mut s, false, false);
    s.demo = false;
    s.route_can_run = false;
    let mut rt = Runtime::new(
        s,
        |s, _| {
            let steps = [NextStep {
                title: "Clean the Linux cache too".into(),
                brief: "Do the same for the AppImage cache.".into(),
            }];
            view(s, &steps, "scope-1", true, &Palette::new(true)).unwrap()
        },
        (700., 300.),
        Env::new(true, 1.),
    );
    for _ in 0..4 {
        rt.frame(0.05);
    }

    let find = |rt: &Runtime<State>, key: &str| {
        rt.elements()
            .into_iter()
            .find(|el| !el.exiting && el.key.as_deref() == Some(key))
            .unwrap_or_else(|| panic!("missing {key}"))
    };
    let here = find(&rt, "next-step-here-scope-1-0");
    let start = find(&rt, "next-step-start-scope-1-0");
    assert!(!here.focusable, "Do it here renders disabled while a turn cannot start");
    assert!(!start.focusable, "Start renders disabled while a turn cannot start");

    rt.update(|s, _| s.route_can_run = true);
    for _ in 0..4 {
        rt.frame(0.05);
    }
    assert!(find(&rt, "next-step-here-scope-1-0").focusable, "Do it here is focusable once a turn may start");
    assert!(find(&rt, "next-step-start-scope-1-0").focusable, "Start is focusable once a turn may start");
}

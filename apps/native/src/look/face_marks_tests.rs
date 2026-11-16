//! Electron's `the-runtimes-wear-their-marks.test.tsx`, "a teammate's face": the badge with the runtime's own mark.

use super::*;

thread_local! {
    static DRAWING: std::cell::RefCell<(&'static str, f32, Option<&'static str>)> = const { std::cell::RefCell::new(("", 26.0, None)) };
}

fn draw(slot: &'static str, size: f32, runtime: Option<&'static str>) {
    DRAWING.with(|d| *d.borrow_mut() = (slot, size, runtime));
    let s = crate::State::default();
    let mut rt = rune::Runtime::new(
        s,
        |s, _| {
            let (slot, size, runtime) = DRAWING.with(|d| *d.borrow());
            bot_face(s, slot, "tm_wren", "Wren", size, BotFaceOpts { runtime, ..Default::default() }, &super::super::controls::Look::new(true))
        },
        (100.0, 100.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
}

#[test]
fn wears_the_mark_of_the_runtime_it_is_given_on_a_badge() {
    draw("mark-a", 26.0, Some("claude"));
    assert_eq!(drawn_mark("mark-a"), Some(Some("claude".to_owned())));
    draw("mark-b", 26.0, Some("codex"));
    assert_eq!(drawn_mark("mark-b"), Some(Some("codex".to_owned())));
}

#[test]
fn wears_none_when_it_is_too_small_for_a_mark_to_be_a_mark_or_is_given_no_runtime() {
    draw("mark-c", MARKED_FACE_MIN - 1.0, Some("claude"));
    assert_eq!(drawn_mark("mark-c"), Some(None));
    draw("mark-d", 26.0, None);
    assert_eq!(drawn_mark("mark-d"), Some(None));
    assert_eq!(MARKED_FACE_MIN, 24.0);
}

#[test]
fn in_the_sidebars_row_of_faces_says_which_model_is_who_and_nothing_on_a_model_of_their_own() {
    let mate = |runtime: Option<&str>, model: Option<&str>| locust_core::engine::Teammate {
        id: "tm".into(),
        name: "T".into(),
        role: "Code & Migrations".into(),
        runtime: runtime.map(str::to_owned),
        model: model.map(str::to_owned),
        account: None,
        busy: false,
        runs_in: String::new(),
        folder: None,
    };
    assert_eq!(crate::sidebar::roster_runtime(&mate(Some("claude"), Some("sonnet"))), Some("claude"));
    assert_eq!(crate::sidebar::roster_runtime(&mate(Some("codex"), Some("account-default"))), Some("codex"));
    assert_eq!(crate::sidebar::roster_runtime(&mate(Some("opencode"), Some("own-1a2b3c4d/acme-70b"))), None);
    assert_eq!(crate::sidebar::roster_runtime(&mate(None, None)), None);
    // And the sidebar's own face wears it.
    ROSTER.with(|r| *r.borrow_mut() = Some(mate(Some("claude"), Some("sonnet"))));
    let mut rt = rune::Runtime::new(
        crate::State::default(),
        |s, _| {
            let teammate = ROSTER.with(|r| r.borrow().clone()).unwrap();
            crate::sidebar::roster_face(s, "roster", &teammate, None, None, 26.0, &super::super::controls::Look::new(true))
        },
        (100.0, 100.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    assert_eq!(drawn_mark("roster-tm"), Some(Some("claude".to_owned())));
}

thread_local! {
    static ROSTER: std::cell::RefCell<Option<locust_core::engine::Teammate>> = const { std::cell::RefCell::new(None) };
}

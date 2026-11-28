use super::*;
use crate::Flight;
use rune::{Env, Runtime};

fn closing_window(working: bool, keep_running: bool, quitting: bool) -> Runtime<State> {
    let mut s = State::default();
    crate::team_scene(&mut s);
    s.teammates[0].name = "Wren".into();
    s.selected = 0;
    s.flight = working.then(|| Flight {
        teammate_id: s.teammates[0].id.clone(),
        conversation_id: None,
        header: None,
        after_tip: None,
        message: None,
    });
    s.settings_panel.keep_running = keep_running;
    s.quit_guard.quit_requested = quitting;
    let mut rt = Runtime::new(s, crate::view, (1400.0, 900.0), Env::new(true, 1.0));
    rt.update(request_close);
    rt.frame(0.0);
    rt
}

// case: asks only when someone is working and the person is the one closing
#[test]
fn window_close_asks_only_during_work() {
    let _engine = locust_core::native::hooks::no_saved_rules();
    assert!(closing_window(true, false, false).state.quit_guard.open);
    let idle = closing_window(false, false, false);
    assert!(!idle.state.quit_guard.open);
    assert!(idle.state.quit_guard.quit_requested);
}

// case: names who is working, and says what each answer does
#[test]
fn quit_question_draws_the_teammate_and_explains_the_choices() {
    let _engine = locust_core::native::hooks::no_saved_rules();
    let rt = closing_window(true, false, false);
    let words = rt
        .elements()
        .into_iter()
        .filter_map(|el| el.text)
        .collect::<Vec<_>>();
    assert!(
        words.iter().any(|line| line == "Wren is still working."),
        "{words:?}"
    );
    assert!(
        words.iter().any(
            |line| line.contains("closes to the tray until you open it again")
                && line.contains("If you quit now, the run stops")
        ),
        "{words:?}"
    );
}

#[test]
fn keep_running_closes_to_the_tray_without_asking_during_work() {
    let _engine = locust_core::native::hooks::no_saved_rules();
    for work in [true, false] {
        let rt = closing_window(work, true, false);
        assert!(rt.state.quit_guard.kept_in_background);
        assert!(!rt.state.quit_guard.open);
        assert!(!rt.state.quit_guard.quit_requested);
    }
    let rt = closing_window(true, true, true);
    assert!(
        !rt.state.quit_guard.kept_in_background,
        "explicit app quit still quits"
    );
    assert!(!rt.state.quit_guard.open);
    assert!(rt.state.quit_guard.quit_requested);
}

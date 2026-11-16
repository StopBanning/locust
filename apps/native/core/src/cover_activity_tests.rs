//! the-home-cover-rests-after-inactivity.test.ts, case by case: the hook's timers are the caller's seconds here.

use super::*;

fn mounted() -> CoverActivity {
    CoverActivity::new(0.0, true, false, false)
}

#[test]
fn pauses_at_45_seconds_with_no_polling_or_timers_left_while_resting() {
    let cover = mounted();
    assert_eq!(COVER_REST_AFTER_MS, 45_000);
    assert!(!cover.away(44.999));
    assert!(cover.away(45.0));
    assert_eq!(cover.rests_in(45.0), None);
}

#[test]
fn pointermove_pointerdown_keydown_wheel_and_focus_resume_synchronously_and_reset_the_deadline() {
    // pointermove, pointerdown, keydown and wheel are all `touch`; focus is `set_focused(true)`.
    for focus in [false, true] {
        let mut cover = mounted();
        assert!(cover.away(45.0));
        if focus { cover.set_focused(true, 45.0) } else { cover.touch(45.0) }
        assert!(!cover.away(45.0));
        assert!(!cover.away(45.0 + 44.999));
        assert_eq!(cover.rests_in(45.0), Some(45.0));
    }
}

#[test]
fn stream_arrival_resumes_and_extends_the_deadline_without_a_change_to_the_status_words() {
    let mut cover = mounted();
    cover.touch(45.0);
    assert!(!cover.away(45.0));
    cover.touch(89.0);
    assert!(!cover.away(91.0));
    assert!(cover.away(134.0));
}

#[test]
fn blur_and_hiding_pause_immediately_work_cannot_wake_a_window_nobody_sees() {
    let mut cover = mounted();
    cover.set_focused(false, 1.0);
    cover.touch(1.0);
    assert!(cover.away(1.0));
    cover.set_focused(true, 2.0);
    assert!(!cover.away(2.0));
    cover.set_hidden(true, 3.0);
    cover.touch(3.0);
    assert!(cover.away(3.0));
    cover.set_hidden(false, 4.0);
    assert!(!cover.away(4.0));
}

#[test]
fn reduced_motion_never_animates_despite_input_focus_or_a_stream_and_follows_preference_changes() {
    let mut cover = CoverActivity::new(0.0, true, false, true);
    cover.touch(1.0);
    cover.set_focused(true, 1.0);
    assert!(cover.away(1.0));
    assert_eq!(cover.rests_in(1.0), None);
    cover.set_reduced(false, 2.0);
    assert!(!cover.away(2.0));
    cover.set_reduced(true, 3.0);
    assert!(cover.away(3.0));
}

// The sixth case (canvas clocks told on pause and resume, listeners removed on unmount) is React's and the DOM's. Here
// the one wake the window needs is `rests_in`, and nothing is left once rested or held.
#[test]
fn leaves_one_deadline_while_running_and_none_once_rested_or_held() {
    let mut cover = mounted();
    assert_eq!(cover.rests_in(10.0), Some(35.0));
    assert_eq!(cover.rests_in(50.0), None);
    cover.set_focused(false, 51.0);
    assert_eq!(cover.rests_in(51.0), None);
}

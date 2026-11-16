//! `the-face-legend-shows-every-face.test.tsx`: one Electron case, one test, named after it, with Electron's values.
//! Its fifth case (the button and the real face under it) draws the window: `settings_panel::face_legend_tests`.

use super::*;
use crate::bot_rig::{eye_glyphs_for, flash_for, mood_for};
use crate::face_status::{face_presence_for, TeammateStatus};

const ALL: [FaceActivity; 9] = [
    FaceActivity::Thinking,
    FaceActivity::Working,
    FaceActivity::Delegating,
    FaceActivity::Responding,
    FaceActivity::Waiting,
    FaceActivity::Receiving,
    FaceActivity::Blocked,
    FaceActivity::Done,
    FaceActivity::Idle,
];

#[test]
fn shows_every_face_a_teammate_has_each_once_a_subagent_at_work_wears_the_work_face_so_it_is_shown_as_that() {
    let shown: Vec<FaceActivity> = FACE_LEGEND.iter().map(|step| step.activity).collect();
    for (i, a) in shown.iter().enumerate() {
        assert!(!shown[i + 1..].contains(a), "{a:?} shown twice");
    }
    let faces = |a: FaceActivity| format!("{:?}", (eye_glyphs_for(a), mood_for(a), flash_for(a), legend_presence(a)));
    for activity in ALL {
        assert!(shown.iter().any(|one| faces(*one) == faces(activity)), "{activity:?}");
    }
    assert!(!shown.contains(&FaceActivity::Delegating));
}

#[test]
fn names_each_in_the_words_the_app_says_beside_a_face_in_sentence_case() {
    for step in FACE_LEGEND {
        let name = legend_name(step.activity);
        assert_eq!(name.to_lowercase(), face_label(step.activity));
        let first = name.chars().next().unwrap();
        assert_eq!(first.to_uppercase().next(), Some(first));
    }
    let names: Vec<String> = FACE_LEGEND.iter().map(|step| legend_name(step.activity)).collect();
    assert_eq!(names, ["Idle", "Thinking", "Working", "Replying", "Done", "Waiting on you", "Listening", "Blocked"]);
}

#[test]
fn gives_each_face_the_dot_a_teammates_has_for_it() {
    let status = |a: FaceActivity| match a {
        FaceActivity::Thinking | FaceActivity::Working | FaceActivity::Responding => TeammateStatus::Working,
        FaceActivity::Waiting => TeammateStatus::ApprovalNeeded,
        FaceActivity::Blocked => TeammateStatus::Blocked,
        FaceActivity::Done | FaceActivity::Receiving | FaceActivity::Idle => TeammateStatus::Idle,
        FaceActivity::Delegating => unreachable!("not in the legend"),
    };
    for step in FACE_LEGEND {
        assert_eq!(legend_presence(step.activity), face_presence_for(status(step.activity)), "{:?}", step.activity);
    }
}

#[test]
fn holds_each_long_enough_to_be_read_the_whole_round_under_half_a_minute() {
    for step in FACE_LEGEND {
        assert!(step.seconds >= 2.0 && step.seconds <= 4.0, "{:?}", step.activity);
    }
    assert!(FACE_LEGEND.iter().map(|step| step.seconds).sum::<f64>() < 30.0);
    // And Electron's own times, in its order.
    let times: Vec<f64> = FACE_LEGEND.iter().map(|step| step.seconds).collect();
    assert_eq!(times, [2.6, 3.4, 3.0, 2.8, 2.4, 3.0, 2.6, 2.8]);
}

// The legend's timer (FaceLegend.tsx 71-76): not an Electron case of its own, Electron's behaviour.

#[test]
fn the_next_face_comes_once_this_one_has_had_its_time_and_a_press_shows_the_next_at_once() {
    let mut clock = LegendClock::default();
    assert_eq!(clock.shown().activity, FaceActivity::Idle);
    assert!(!clock.tick(10.0, false));
    assert!(!clock.tick(12.5, false), "Idle has 2.6 s");
    assert!(clock.tick(12.6, false));
    assert_eq!(clock.shown().activity, FaceActivity::Thinking);
    assert_eq!(clock.due_in(12.6), Some(3.4));
    clock.press();
    assert_eq!(clock.shown().activity, FaceActivity::Working);
    assert!(!clock.tick(13.0, false), "a press starts the next face's full time");
    assert!(!clock.tick(15.9, false));
    assert!(clock.tick(16.0, false));
    assert_eq!(clock.shown().activity, FaceActivity::Responding);
}

#[test]
fn while_it_rests_it_stops_where_it_is_and_goes_on_with_the_full_time_after() {
    let mut clock = LegendClock::default();
    clock.tick(0.0, false);
    assert!(!clock.tick(100.0, true), "resting never moves it on");
    assert_eq!(clock.shown().activity, FaceActivity::Idle);
    assert_eq!(clock.due_in(100.0), None);
    assert!(!clock.tick(101.0, false));
    assert!(!clock.tick(103.5, false));
    assert!(clock.tick(103.6, false));
    // A press still shows the next one while it rests.
    clock.press();
    assert!(!clock.tick(200.0, true));
    assert_eq!(clock.shown().activity, FaceActivity::Working);
}

#[test]
fn goes_round_again_after_the_eighth() {
    let mut clock = LegendClock::default();
    for _ in 0..8 {
        clock.press();
    }
    assert_eq!(clock.shown().activity, FaceActivity::Idle);
    assert_eq!(legend_label(clock.shown().activity), "A teammate's face: Idle. Show the next one.");
}

#[test]
fn says_its_eyes_say_it_too_only_while_terminal_faces_is_on() {
    assert_eq!(
        legend_lede(true),
        "A teammate's face says what it is doing, and its eyes say it too. This one shows each in turn; click it for the next."
    );
    assert_eq!(
        legend_lede(false),
        "A teammate's face says what it is doing. This one shows each in turn; click it for the next. With Terminal faces on, its eyes say it too."
    );
}

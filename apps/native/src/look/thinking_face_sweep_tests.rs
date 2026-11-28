//! Electron's thinking face cases through the native head projection and painter.
use super::*;
use crate::look::bot_stage::BotStage;
use locust_core::bot_rig::{BotAvatarState, BotMotionLevel, FaceActivity, HeadGlance, rest_pose, screen_pose, thinking_glance};

#[derive(Debug)]
struct Ink {
    points: Vec<f32>,
    width: f32,
    round: bool,
}

// Rune keeps Shape's stroke list private. Inspect the emitted strokes the same way as
// glyph_eyes_tests, not a second copy of the eye geometry.
fn ink(shape: Shape) -> Vec<Ink> {
    format!("{shape:?}").split("Stroke { points: [").skip(1).map(|s| {
        let (points, fields) = s.split_once("], closed: ").expect("Rune stroke points");
        let width = fields.split_once(", width: ").unwrap().1.split(',').next().unwrap().parse().unwrap();
        Ink {
            points: points.split([',', '[', ']']).map(str::trim).filter(|p| !p.is_empty())
                .map(|p| p.parse().unwrap()).collect(),
            width,
            round: fields.split_once(", cap: ").unwrap().1.starts_with("Round,"),
        }
    }).collect()
}

fn lights(pair: [&str; 2], at: f64, open: f64) -> Vec<Ink> {
    ink(draw_screen_light(
        Shape::new(), &[], Phosphor::Cyan, None, 1.0, None, &pair.map(str::to_owned),
        [-11.5, 0.0], [11.5, 0.0], 1.0, open, open,
        &|p| [p[0] as f32, p[1] as f32], 1.0, at, 0.0,
    ))
}

fn draw(snap: &BotSnapshot) -> Shape {
    draw_bot_shape(&BotDrawSpec {
        shape: "prompt", face: BotFace::Eyes, hue: "blue", size: 64.0, light_theme: false,
        presence: None, ring: false, snap,
    })
}

// case: is at rest on a still bot, drawn at second 0 (reduced motion holds still): the head where the rig has it, the dots where they hold
#[test]
fn sweep_thinking_face_in_reduced_motion_draws_and_keeps_its_rest() {
    assert_eq!(thinking_glance(0.0).mix, 0.0);
    assert_eq!(thinking_glance(-0.1).mix, 0.0);
    let stage = BotStage::default();
    let snap = || stage.snapshot_bot("test-sweep-thinking", "prompt", FaceActivity::Thinking, None, BotMotionLevel::Full, true, false, true);
    let first = snap();
    assert_eq!(first.s_seconds, 0.0);
    assert_eq!(first.eyes.as_ref().map(|pair| pair.concat()).as_deref(), Some("••"));
    stage.poll(10.0, true, true, &mut rune::Tx::default());
    let later = snap();
    assert_eq!(later.s_seconds, 0.0);
    assert_eq!(draw(&first), draw(&later));
}

// case: replaces the rig's heading as its mix rises, rather than adding a second turn to it
#[test]
fn sweep_screen_head_takes_the_glance_instead_of_adding_it() {
    let mut rig = rest_pose(BotAvatarState::Default);
    rig.yaw = 0.5;
    rig.pitch = -0.2;
    for (mix, yaw, pitch) in [(0.0, 0.5, -0.2), (0.5, 0.1, -0.04), (1.0, -0.3, 0.12)] {
        let pose = screen_pose(rig, Some(0.5), Some(HeadGlance { yaw: -0.3, pitch: 0.12, mix, wander: 1.0 }));
        assert!((pose.yaw - yaw).abs() < 1e-9);
        assert!((pose.pitch - pitch).abs() < 1e-9);
    }
    for n in 0..420 {
        assert!(thinking_glance(n as f64 / 100.0).yaw.abs() < 0.75);
    }
}

// case: share one gaze: at any moment the two are the same distance apart, and level
#[test]
fn sweep_thinking_dots_are_level_and_stay_one_fixed_distance_apart() {
    // The first half of the cycle is the shared gaze, not the later typing bounce.
    for n in 0..63 {
        let strokes = lights(["•", "•"], 8.4 + n as f64 / 30.0, 1.0);
        assert_eq!(strokes.len(), 4, "two lit dots and their two glows");
        let (left, right) = (&strokes[1].points, &strokes[3].points);
        assert!((right[0] - left[0] - 23.0).abs() < 1e-5);
        assert!((right[1] - left[1]).abs() < 1e-5);
    }
}

// case: never thin in a blink: every stroke keeps its weight, open, blinking or shut
#[test]
fn sweep_nondot_eye_strokes_keep_their_weight_during_blinks() {
    for pair in [[">", "▮"], ["^", "^"], [">", "<"], ["x", "x"], ["|", "|"]] {
        let weights = |open| lights(pair, 1.0, open).into_iter().map(|stroke| stroke.width).collect::<Vec<_>>();
        let open = weights(1.0);
        assert!(!open.is_empty(), "{pair:?}");
        for blink in [0.6, 0.3, 0.05] {
            assert_eq!(weights(blink), open, "{pair:?} at {blink}");
        }
    }
}

// case: never thin as the head turns: a round dot is drawn round, the same size, however far it is turned
#[test]
fn sweep_the_painted_dots_stay_round_and_the_same_width_through_head_turns() {
    let stage = BotStage::default();
    let mut snap = stage.snapshot_bot("test-sweep-turn", "prompt", FaceActivity::Thinking, None, BotMotionLevel::Full, true, false, true);
    snap.pose = rest_pose(BotAvatarState::Default);
    snap.pose.eye_open = 1.0;
    let widths = |snap: &BotSnapshot| {
        let strokes = ink(draw(snap));
        let dots = &strokes[strokes.len() - 4..];
        assert!(dots.iter().all(|stroke| stroke.round));
        assert!(dots.iter().all(|stroke| stroke.points.len() == 4));
        dots.iter().map(|stroke| stroke.width).collect::<Vec<_>>()
    };
    let front = widths(&snap);
    assert!(front[1] > 0.6 && front[3] > 0.6);
    for yaw in [0.3, 0.6, 0.75] {
        snap.pose.yaw = yaw;
        assert_eq!(widths(&snap), front);
    }
}

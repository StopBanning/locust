//! Electron `main/only-the-face-you-talk-to-hops.test.ts`, `describe('the slight bounce')` (2 cases):
//! the subtle bounce is a small 7% lift with a longer rest (down again by 46% and still from there),
//! and stops completely when reduced motion is on.

use super::*;
use crate::look::bots::{draw_bot_shape, BotDrawSpec};

fn min_y_of(shape: &rune::Shape) -> f32 {
    let text = format!("{shape:?}");
    let mut best = f32::INFINITY;
    let mut rest = text.as_str();
    while let Some(open) = rest.find('[') {
        rest = &rest[open + 1..];
        if rest.starts_with('[') {
            continue;
        }
        let Some(close) = rest.find(']') else { break };
        if let Some((_, b)) = rest[..close].split_once(',')
            && let Ok(y) = b.trim().parse::<f32>()
        {
            best = best.min(y);
        }
        rest = &rest[close + 1..];
    }
    best
}

fn step_subtle(stage: &BotStage, now_s: f64, reduced_motion: bool) -> BotSnapshot {
    let mut tx = rune::Tx::default();
    stage.begin_frame();
    stage.snapshot_bot(
        "b",
        "tm_wren",
        FaceActivity::Working,
        None,
        BotMotionLevel::Subtle,
        false,
        false,
        reduced_motion,
    );
    stage.poll(now_s, reduced_motion, true, &mut tx);
    stage.inner.borrow().bots["b"].cached.clone()
}

// case: is a small lift and a longer rest
#[test]
fn is_a_small_lift_and_a_longer_rest() {
    // Electron @keyframes lcBotBounce: 1.5s cycle, peak -7% at 23%, back to 0 at 46%, rest for 54%.
    assert_eq!(subtle_bounce_lift(0.0), 0.0);
    let peak = subtle_bounce_lift(0.23 * BOUNCE_CYCLE_S);
    assert!((peak - 7.0).abs() < 1e-6, "peak lift={peak}");
    // Intermediate keyframe 11.5% is -3.5%, 46% is 0.
    assert!((subtle_bounce_lift(0.115 * BOUNCE_CYCLE_S) - 3.5).abs() < 1e-6);
    assert_eq!(subtle_bounce_lift(0.46 * BOUNCE_CYCLE_S), 0.0);
    // Rest is the longer part (46%..100% of the cycle stays at 0).
    for pct in [0.46, 0.50, 0.65, 0.80, 0.99] {
        assert_eq!(subtle_bounce_lift(pct * BOUNCE_CYCLE_S), 0.0, "still at {pct}");
    }

    // Verified through the production BotStage and draw_bot_shape: at 23% the drawn bot sits higher
    // than at 65% (rest), by roughly 7% of the 100-point box.
    let stage = BotStage::default();
    step_subtle(&stage, 0.0, false);
    let snap_peak = step_subtle(&stage, 0.23 * BOUNCE_CYCLE_S, false);
    let lift_peak = stage.inner.borrow().bots["b"].bounce_lift;
    assert!((lift_peak - 7.0).abs() < 1e-6);
    let snap_rest = step_subtle(&stage, 0.65 * BOUNCE_CYCLE_S, false);
    assert_eq!(stage.inner.borrow().bots["b"].bounce_lift, 0.0);

    let drawn_peak = draw_bot_shape(&BotDrawSpec {
        shape: "circle",
        face: BotFace::Eyes,
        hue: "blue",
        size: 100.0,
        light_theme: false,
        presence: None,
        ring: false,
        snap: &snap_peak,
    });
    let drawn_rest = draw_bot_shape(&BotDrawSpec {
        shape: "circle",
        face: BotFace::Eyes,
        hue: "blue",
        size: 100.0,
        light_theme: false,
        presence: None,
        ring: false,
        snap: &snap_rest,
    });
    let rise = min_y_of(&drawn_rest) - min_y_of(&drawn_peak);
    assert!(rise > 4.0 && rise < 10.0, "drawn bounce rise={rise}px on 100px box");
}

// case: stops for a person who asked for less motion
#[test]
fn stops_for_a_person_who_asked_for_less_motion() {
    let stage = BotStage::default();
    let base = step_subtle(&stage, 0.0, true);
    for t in [0.115, 0.23, 0.345, 0.65] {
        let snap = step_subtle(&stage, t * BOUNCE_CYCLE_S, true);
        let inner = stage.inner.borrow();
        let slot = &inner.bots["b"];
        assert!(!slot.bouncing, "no bounce under reduced motion at {t}");
        assert_eq!(slot.bounce_lift, 0.0);
        assert_eq!(snap.pose.y, base.pose.y, "pose.y held still under reduced motion at {t}");
    }
}

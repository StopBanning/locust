//! A SCREEN'S EYES HAVE WEIGHT (Electron 0.574, `a-screens-eyes-have-weight.test.ts`): the eyes on a screen trail
//! the body's hops and turns by a unit or two and spring back, and settle the same however often they are drawn.

use crate::bot_rig::{step_lag, Lag, LAG_REST};

const STEP: f64 = 1.0 / 60.0;

/// A hop as the rig makes it: up 12 units and down again over 0.4 s, then still. The lag at each frame.
fn through_a_hop(seconds: f64, step: f64) -> Vec<Lag> {
    let mut frames = Vec::new();
    let mut lag = LAG_REST;
    let (mut last_y, mut last_v) = (0.0, 0.0);
    let mut t = step;
    while t <= seconds {
        let y = if t < 0.4 { -12.0 * (t / 0.4 * std::f64::consts::PI).sin() } else { 0.0 };
        let v = (y - last_y) / step;
        lag = step_lag(lag, step, 0.0, v - last_v);
        last_y = y;
        last_v = v;
        frames.push(lag);
        t += step;
    }
    frames
}

fn most_y(frames: &[Lag]) -> f64 {
    frames.iter().map(|l| l.y).fold(f64::MIN, f64::max)
}

fn least_y(frames: &[Lag]) -> f64 {
    frames.iter().map(|l| l.y).fold(f64::MAX, f64::min)
}

#[test]
fn trail_a_hop_sinking_as_the_body_takes_off_floating_in_the_air_and_sinking_again_as_it_lands() {
    let frames = through_a_hop(1.2, STEP);
    assert!(most_y(&frames[0..6]) > 0.5);
    assert!(least_y(&frames[8..22]) < -0.3);
    assert!(most_y(&frames[24..34]) > 0.5);
}

#[test]
fn settle_within_half_a_second_of_the_body_coming_to_rest() {
    let frames = through_a_hop(1.2, STEP);
    for lag in &frames[(0.9 / STEP).round() as usize..] {
        assert!(lag.y.abs() < 0.05, "{lag:?}");
    }
}

#[test]
fn never_trail_by_more_than_three_units_however_hard_the_body_moves() {
    let mut lag = LAG_REST;
    for i in 0..120 {
        lag = step_lag(lag, STEP, if i % 2 == 0 { 900.0 } else { -900.0 }, if i % 3 == 0 { -900.0 } else { 900.0 });
        assert!(lag.x.abs() <= 3.0 && lag.y.abs() <= 3.0, "{lag:?}");
    }
}

#[test]
fn at_the_side_drawn_15_times_a_second_settle_just_the_same_and_never_swing_frame_to_frame() {
    let frames = through_a_hop(1.2, 1.0 / 15.0);
    for lag in &frames[(0.9_f64 * 15.0).round() as usize..] {
        assert!(lag.y.abs() < 0.05, "{lag:?}");
    }
    for i in 2..frames.len() {
        let before = frames[i - 1].y - frames[i - 2].y;
        let now = frames[i].y - frames[i - 1].y;
        assert!(!(before * now < 0.0 && now.abs() > 1.0 && before.abs() > 1.0), "frame {i} swings");
    }
}

#[test]
fn at_the_side_never_pinned_at_the_end_of_their_room_after_the_body_is_still() {
    let mut lag = step_lag(LAG_REST, 1.0 / 15.0, 0.0, 40.0);
    for _ in 0..30 {
        lag = step_lag(lag, 1.0 / 15.0, 0.0, 0.0);
    }
    assert!(lag.y.abs() < 0.05, "{lag:?}");
}

#[test]
fn drawn_30_or_60_times_a_second_take_the_same_path_as_before_one_step_each() {
    let at30 = step_lag(Lag { x: 1.0, y: -1.0, vx: 2.0, vy: -2.0 }, 1.0 / 30.0, 5.0, -5.0);
    let vy = -2.0 + (-260.0 * -1.0 - 22.0 * -2.0) * (1.0 / 30.0) - -5.0 * 0.35;
    assert!((at30.vy - vy).abs() < 1e-10, "{} vs {vy}", at30.vy);
    assert!((at30.y - (-1.0 + vy / 30.0)).abs() < 1e-10, "{}", at30.y);
}

#[test]
fn sit_where_they_are_put_while_the_body_moves_at_a_steady_speed() {
    let mut lag = LAG_REST;
    for _ in 0..60 {
        lag = step_lag(lag, STEP, 0.0, 0.0);
    }
    assert_eq!(lag, LAG_REST);
}

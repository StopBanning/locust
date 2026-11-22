//! The math cases from a-screen-switches-on-and-its-dots-blink.test.ts.
use super::*;
use crate::bot_spec::{blinking_dot, glyph_shape};

// case: is the dot open and the screen's own dash shut
#[test]
fn is_the_dot_open_and_the_screens_own_dash_shut() {
    let (open, weight) = blinking_dot(0.0);
    assert_eq!(weight, glyph_shape("•").unwrap().weight);
    assert!((open[2] - open[0]).hypot(open[3] - open[1]) < 0.05);
    let (shut, weight) = blinking_dot(1.0);
    assert_eq!(shut, [-4.8, 0.4, 4.8, 0.4]);
    assert_eq!(shut.as_slice(), glyph_shape("-").unwrap().lines[0]);
    assert_eq!(weight, glyph_shape("-").unwrap().weight);
}

// case: draws out and lightens steadily as it shuts, never thinner than the dash
#[test]
fn draws_out_and_lightens_steadily_as_it_shuts_never_thinner_than_the_dash() {
    let (mut last_weight, mut last_length) = (f64::INFINITY, -1.0);
    for i in 0..=20 {
        let (line, weight) = blinking_dot(i as f64 / 20.0);
        assert!(weight <= last_weight && weight >= glyph_shape("-").unwrap().weight);
        assert!(line[2] - line[0] >= last_length);
        (last_weight, last_length) = (weight, line[2] - line[0]);
    }
}

// case: draws a line across the glass first, then opens it into a flash that settles to dark
#[test]
fn draws_a_line_across_the_glass_first_then_opens_it_into_a_flash_that_settles_to_dark() {
    let first = power_on_light(0.0);
    assert_eq!((first.width, first.height, first.alpha), (0.0, 0.0, 1.0));
    assert!((power_on_light(0.33).width - 1.0).abs() < 1e-9);
    assert_eq!(power_on_light(0.3).height, 0.0);
    assert!(power_on_light(0.5).height > 0.5 && power_on_light(0.5).alpha < 0.85);
    assert!(power_on_light(0.67).alpha.abs() < 0.001);
    let mut last = 2.0;
    for i in 34..=100 {
        let a = power_on_light(i as f64 / 100.0).alpha;
        assert!(a <= last + 1e-12);
        last = a;
    }
}

// case: keeps the eyes shut until the flash has settled, then blinks them open
#[test]
fn keeps_the_eyes_shut_until_the_flash_has_settled_then_blinks_them_open() {
    for p in [0.0, 0.2, 0.5, 0.66] {
        assert_eq!(power_on_eyes(p), 0.0);
    }
    assert!(power_on_eyes(0.8) > 0.0);
    assert_eq!(power_on_eyes(1.0), 1.0);
    let mut last = 0.0;
    for i in 0..=100 {
        let p = i as f64 / 100.0;
        let a = power_on_eyes(p);
        assert!(a >= last);
        assert_eq!(power_on_light(p).eyes, a);
        last = a;
    }
    assert!(POWER_ON_S < 1.0);
}

// case: winces with its shudder as `> <`, and stays within the reach a screen leaves it
#[test]
fn winces_with_its_shudder_and_stays_within_the_reach_a_screen_leaves_it() {
    let mut pinched = 1.0_f64;
    for i in 1..600 {
        let t = i as f64 / 100.0;
        let m = glyph_motion("><", 0, t);
        pinched = pinched.min(m.sx);
        assert!(m.dx.abs() <= 2.0 && m.sy <= 1.16);
        assert_eq!(glyph_motion("xx", 0, t).sx, 1.0);
    }
    assert!(pinched < 0.9);
}

//! `the-thinking-dots-never-jump.test.ts`: the two dots' 4.2 s loop, read at a face's 30 frames a second.

use crate::bot_rig::glyph_motion;

const CYCLE: f64 = 4.2;
const FRAME: f64 = 1.0 / 30.0;

fn step(eye: usize, from: f64, to: f64) -> f64 {
    let (a, b) = (glyph_motion("••", eye, from), glyph_motion("••", eye, to));
    (b.dx - a.dx).hypot(b.dy - a.dy)
}

#[test]
fn end_each_loop_where_the_next_begins_with_no_wrap_to_jump_across() {
    for eye in [0, 1] {
        let before = glyph_motion("••", eye, 2.0 * CYCLE - 0.001);
        let after = glyph_motion("••", eye, 2.0 * CYCLE + 0.001);
        assert!((after.dx - before.dx).hypot(after.dy - before.dy) < 0.05, "eye {eye}");
    }
}

#[test]
fn never_move_farther_in_one_frame_than_the_bounce_does() {
    let mut bounce: f64 = 0.0;
    let mut t = 2.1;
    while t < CYCLE - FRAME {
        bounce = bounce.max(step(0, t, t + FRAME)).max(step(1, t, t + FRAME));
        t += 0.0001;
    }
    assert!(bounce > 0.3, "{bounce}");
    for eye in [0, 1] {
        let mut t = CYCLE * 0.5;
        while t < CYCLE * 2.5 {
            assert!(step(eye, t, t + FRAME) <= bounce + 1e-9, "eye {eye} at {t:.3} s");
            t += FRAME;
        }
    }
}

#[test]
fn hold_still_while_the_face_looks_about_then_bounce_in_turn() {
    let mut t = 0.001;
    while t < 2.1 {
        let left = glyph_motion("••", 0, CYCLE * 3.0 + t);
        assert_eq!(left, glyph_motion("••", 1, CYCLE * 3.0 + t));
        assert_eq!(left, glyph_motion("••", 0, CYCLE * 3.0 + 0.5));
        t += 0.01;
    }
    let apart = [2.3, 2.5, 2.7, 3.1, 3.3, 3.5]
        .map(|t| (glyph_motion("••", 0, CYCLE * 3.0 + t).dy - glyph_motion("••", 1, CYCLE * 3.0 + t).dy).abs());
    assert!(apart.iter().cloned().fold(0.0, f64::max) > 0.5, "{apart:?}");
}

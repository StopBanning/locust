//! Exact pure cases from a-teammates-eyes-are-drawn-not-typed.test.ts.
//! Drawing cases remain uncarried; these exercise the geometry used by the native painter.
use crate::bot_rig::{
    FaceActivity::*, GlyphMotion, SCREEN_RESTING_EYES, eye_glyphs_for, eye_openness, glyph_motion,
    thinking_glance,
};
use crate::bot_spec::*;
const ACTIVITIES: [crate::bot_rig::FaceActivity; 9] = [
    Thinking, Working, Delegating, Responding, Waiting, Receiving, Blocked, Done, Idle,
];
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 0.005, "{a} != {b}");
}
fn over(pair: &str, eye: usize) -> Vec<GlyphMotion> {
    (0..240)
        .map(|i| glyph_motion(pair, eye, 1.0 + i as f64 / 30.0))
        .collect()
}
#[test]
fn is_drawn_for_every_glyph_a_state_asks_for_and_a_screen_rests_on_one_too() {
    for glyph in ACTIVITIES
        .into_iter()
        .flat_map(|a| eye_glyphs_for(a).into_iter().flatten())
        .chain(SCREEN_RESTING_EYES)
    {
        assert!(glyph_shape(glyph).is_some(), "{glyph}");
    }
}
#[test]
fn is_as_heavy_as_the_eye_it_stands_for() {
    for glyph in [">", "<", "_", "-", "^", "x", "|", "o", "c", "•", "▮"] {
        assert!(glyph_shape(glyph).unwrap().weight >= 3.8, "{glyph}");
    }
}
#[test]
fn blinks_with_the_eye_open_at_its_full_weight_shut_at_its_blink_weight() {
    assert_eq!(eye_openness(12.6), 1.0);
    assert_eq!(eye_openness(2.8), 0.0);
    close(eye_openness(7.7), 0.5);
}
#[test]
fn is_the_two_mascots_prompts_and_sparks_and_no_other_shapes() {
    assert!(outline_of("prompt").screen);
    assert!(outline_of("spark").screen);
    for shape in ["hopper", "swarm", "critter", "droid"] {
        assert!(!outline_of(shape).screen, "{shape}");
    }
}
#[test]
fn keeps_the_hue_it_is_given() {
    assert_eq!(same_hue("#ff0000", 1.0, 0.5, 1.0), "hsla(0, 100%, 50%, 1)");
    let blue = same_hue("#6fb7d6", 0.95, 0.8, 1.0);
    assert!(blue.starts_with("hsla(19"), "{blue}");
    assert!(blue.ends_with(", 95%, 80%, 1)"), "{blue}");
}
#[test]
fn is_set_flat_in_the_bodys_front_and_turns_with_it_all_the_way() {
    let at = FaceAt {
        x: 50.0,
        y: 50.0,
        scale: 1.0,
    };
    let ahead = front_plane(0.0, 0.0, at);
    close(ahead[0], 0.95);
    close(ahead[3], 0.95);
    close(ahead[4], 0.0);
    close(ahead[5], 0.0);
    let turned = front_plane(0.5, 0.0, at);
    close(turned[0], 0.5_f64.cos() * 0.95);
    close(turned[4], 0.5_f64.sin() * 9.75);
    let low = front_plane(
        0.0,
        0.3,
        FaceAt {
            x: 50.0,
            y: 60.0,
            scale: 0.95,
        },
    );
    close(
        low[5],
        (0.3_f64.cos() * 0.95 * 10.0 - 0.3_f64.sin() * 9.75 - 10.0) / 0.95,
    );
}
#[test]
fn move_every_pair_a_state_asks_for_over_a_few_seconds() {
    for pair in [">▮", "••", "^^", "xx"] {
        let seen: std::collections::HashSet<_> = over(pair, 1)
            .iter()
            .map(|m| format!("{:.2},{:.2},{:.2},{}", m.dx, m.dy, m.sx, m.shown))
            .collect();
        assert!(seen.len() > 4, "{pair}");
    }
}
#[test]
fn at_work_the_cursor_blinks_and_the_prompt_stays_lit() {
    assert!(over(">▮", 1).iter().any(|m| !m.shown));
    assert!(over(">▮", 1).iter().any(|m| m.shown));
    assert!(over(">▮", 0).iter().all(|m| m.shown));
}
#[test]
fn in_thought_look_up_and_about_then_bounce_in_turn_like_a_reply_being_typed() {
    let left: Vec<_> = (0..126)
        .map(|i| glyph_motion("••", 0, 4.2 * 3.0 + i as f64 / 30.0))
        .collect();
    let right: Vec<_> = (0..126)
        .map(|i| glyph_motion("••", 1, 4.2 * 3.0 + i as f64 / 30.0))
        .collect();
    let looks: Vec<_> = (0..126)
        .map(|i| thinking_glance(4.2 * 3.0 + i as f64 / 30.0))
        .filter(|l| l.mix > 0.99)
        .collect();
    assert!(looks.iter().any(|l| l.yaw < -0.2));
    assert!(looks.iter().any(|l| l.yaw > 0.2));
    assert!(looks.iter().all(|l| l.pitch > 0.0));
    assert!(!looks.is_empty());
    assert!(
        left.iter()
            .zip(&right)
            .any(|(l, r)| (l.dy - r.dy).abs() > 0.6)
    );
    assert!(left.iter().all(|m| m.sy > 0.5));
}
#[test]
fn are_never_flat_lines_for_work_or_thought() {
    for activity in [Working, Delegating, Thinking] {
        for glyph in eye_glyphs_for(activity).into_iter().flatten() {
            assert!(!["-", "_"].contains(&glyph));
        }
    }
}
#[test]
fn set_off_from_where_a_still_bot_rests_second_zero_of_their_state() {
    for pair in [">▮", "••", "^^", "xx", "><", "cc", "||"] {
        for eye in [0, 1] {
            let rest = glyph_motion(pair, eye, 0.0);
            let away = glyph_motion(pair, eye, 0.0001);
            assert!(
                (away.dx - rest.dx).hypot(away.dy - rest.dy) < 0.002,
                "{pair} {eye}"
            );
            assert_eq!(away.shown, rest.shown);
            assert_eq!(glyph_motion(pair, eye, -0.2), rest);
        }
    }
    for pair in ["••", "^^", "xx", "><", "cc", "||"] {
        let m = glyph_motion(pair, 1, 0.0);
        assert_eq!((m.dx, m.sx, m.sy, m.shown), (0.0, 1.0, 1.0, true));
    }
    let m = glyph_motion(">▮", 1, 0.0);
    assert_eq!((m.dy, m.sx, m.sy, m.shown), (0.0, 1.0, 1.0, true));
}
#[test]
fn stay_small_a_glyph_never_wanders_off_its_eye() {
    for pair in [">▮", "••", "^^", "xx"] {
        for m in over(pair, 0).into_iter().chain(over(pair, 1)) {
            assert!(m.dx.abs() <= 2.0 && m.dy.abs() <= 2.0, "{pair}");
        }
    }
}
#[test]
fn keeps_its_full_size_where_the_body_has_room() {
    assert_eq!(
        fit_visor(
            |x, y| x > 0.0 && x < 100.0 && y > 0.0 && y < 100.0,
            50.0,
            50.0,
            1.0,
            EYES_VISOR,
            3.0
        ),
        EYES_VISOR
    );
}
#[test]
fn shrinks_until_it_sits_inside_the_body_with_a_margin() {
    let circle = |x: f64, y: f64| (x - 50.0).hypot(y - 50.0) <= 30.0;
    let fitted = fit_visor(circle, 50.0, 50.0, 1.0, EYES_VISOR, 3.0);
    assert!(fitted.half_width < EYES_VISOR.half_width);
    assert!(
        visor_outline(fitted)
            .iter()
            .all(|[x, y]| circle(50.0 + x, 50.0 + y))
    );
    let bigger = VisorBox {
        half_width: fitted.half_width * 1.1 + 3.0,
        half_height: fitted.half_height * 1.1 + 3.0,
        corner: fitted.corner * 1.1 + 3.0,
        ..fitted
    };
    assert!(
        !visor_outline(bigger)
            .iter()
            .all(|[x, y]| circle(50.0 + x, 50.0 + y))
    );
}
#[test]
fn reads_the_hue_of_whatever_colour_the_plastic_hands_it() {
    close(hue_of("hsl(219.7 89.7% 64.7%)"), 219.7);
    assert_eq!(hue_of("#ff0000"), 0.0);
    assert_eq!(hue_of("rgb(0, 0, 255)"), 240.0);
}

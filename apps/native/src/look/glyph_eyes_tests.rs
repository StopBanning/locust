//! Electron's a-teammates-eyes-are-drawn-not-typed: one case per Rust test.
use super::*;
use locust_core::bot_rig::{FaceActivity, GlyphMotion, eye_glyphs_for, thinking_glance};
use locust_core::bot_spec::{
    EYES_VISOR, FaceAt, VisorBox, fit_visor, front_plane, hue_of, same_hue,
};
fn over(pair: &str, eye: usize) -> Vec<GlyphMotion> {
    (0..240)
        .map(|i| glyph_motion(pair, eye, 1.0 + i as f64 / 30.0))
        .collect()
}
fn glyph(g: &str) -> String {
    format!(
        "{:?}",
        paint_glyph_at(
            Shape::new(),
            g,
            0.0,
            0.0,
            1.0,
            1.0,
            &|p| [p[0] as f32, p[1] as f32],
            1.0,
            color_from_css("#1a1a2e", 1.0),
            color_from_css("#1a1a2e", 0.0)
        )
    )
}
fn light(pair: [&str; 2], seconds: f64) -> String {
    format!(
        "{:?}",
        draw_screen_light(
            Shape::new(),
            &[],
            Phosphor::Cyan,
            None,
            1.0,
            None,
            &pair.map(str::to_owned),
            [-11.5, 0.0],
            [11.5, 0.0],
            1.0,
            1.0,
            1.0,
            &|p| [p[0] as f32, p[1] as f32],
            1.0,
            seconds,
            0.0
        )
    )
}
#[test]
fn is_round_strokes_never_a_letter() {
    for g in [">", "▮"] {
        let s = glyph(g);
        assert_eq!(
            s.matches("cap: Round").count(),
            2,
            "one ink stroke plus its glow: {s}"
        );
        assert!(!s.contains("Text"));
        assert!(
            s.contains(&format!("{:?}", color_from_css("#1a1a2e", 1.0))),
            "{s}"
        );
    }
}
#[test]
fn is_drawn_for_every_glyph_a_state_asks_for_and_a_screen_rests_on_one_too() {
    use FaceActivity::*;
    for a in [
        Thinking, Working, Delegating, Responding, Waiting, Receiving, Blocked, Done, Idle,
    ] {
        for g in eye_glyphs_for(a)
            .into_iter()
            .flatten()
            .chain(SCREEN_RESTING_EYES)
        {
            assert!(glyph_shape(g).is_some(), "{a:?} {g}");
            assert_ne!(glyph(g), format!("{:?}", Shape::new()));
        }
    }
}
#[test]
fn is_as_heavy_as_the_eye_it_stands_for() {
    for g in [">", "<", "_", "-", "^", "x", "|", "o", "c", "•", "▮"] {
        assert!(glyph_shape(g).unwrap().weight >= 3.8, "{g}");
    }
}
#[test]
fn blinks_with_the_eye_open_at_its_full_weight_shut_at_its_blink_weight() {
    assert_eq!(eye_openness(12.6), 1.0);
    assert_eq!(eye_openness(2.8), 0.0);
    assert!((eye_openness(7.7) - 0.5).abs() < 0.005);
}
#[test]
fn is_the_two_mascots_prompt_and_spark_and_no_other_shapes() {
    for shape in BOT_SHAPES {
        assert_eq!(
            outline_of(shape).screen,
            matches!(shape, "prompt" | "spark"),
            "{shape}"
        );
    }
    assert!(!outline_of("droid").screen);
}
#[test]
fn is_drawn_once_a_frame_under_the_eyes_and_the_eyes_glow_terminal_cyan() {
    let cyan = color_from_rgb_f64(phosphor_rgb(Phosphor::Cyan).map(f64::from), 0.96);
    assert!(light([">", "▮"], 0.0).contains(&format!("{cyan:?}")));
    let poly = [[-20.0, -10.0], [20.0, -10.0], [20.0, 10.0], [-20.0, 10.0]];
    let panel = format!(
        "{:?}",
        draw_screen_panel(
            Shape::new(),
            &poly,
            Phosphor::Cyan,
            None,
            1.0,
            None,
            &[">".into(), "▮".into()],
            [-11.5, 0.0],
            [11.5, 0.0],
            1.0,
            1.0,
            1.0,
            &|p| [p[0] as f32, p[1] as f32],
            1.0,
            0.0,
            0.0
        )
    );
    let glass = format!("{:?}", rgb8(12, 16, 22));
    assert_eq!(panel.matches(&glass).count(), 1);
    assert!(panel.find(&glass).unwrap() < panel.find(&format!("{cyan:?}")).unwrap());
}
#[test]
fn keeps_the_hue_it_is_given() {
    assert_eq!(same_hue("#ff0000", 1.0, 0.5, 1.0), "hsla(0, 100%, 50%, 1)");
    assert!((190.0..200.0).contains(&hue_of("#6fb7d6")));
    assert!(same_hue("#6fb7d6", 0.95, 0.8, 1.0).ends_with(", 95%, 80%, 1)"));
}
#[test]
fn is_set_flat_in_the_bodys_front_and_turns_with_it_all_the_way() {
    let at = FaceAt {
        x: 50.0,
        y: 50.0,
        scale: 1.0,
    };
    let a = front_plane(0.0, 0.0, at);
    assert_eq!([a[0], a[3], a[4], a[5]], [0.95, 0.95, 0.0, 0.0]);
    let t = front_plane(0.5, 0.0, at);
    assert!((t[0] - 0.5_f64.cos() * 0.95).abs() < 0.005);
    assert!((t[4] - 0.5_f64.sin() * 9.75).abs() < 0.005);
    let low = front_plane(
        0.0,
        0.3,
        FaceAt {
            x: 50.0,
            y: 60.0,
            scale: 0.95,
        },
    );
    assert!(
        (low[5] - (0.3_f64.cos() * 0.95 * 10.0 - 0.3_f64.sin() * 9.75 - 10.0) / 0.95).abs() < 0.005
    );
}
#[test]
fn is_drawn_in_the_transform_the_body_uses() {
    // Exercise draw_bot_shape's actual body/visor/eye mapper at a turn and tilt.
    // Native has one geometry space, so it needs no captured Canvas transform.
    let mut pose = locust_core::bot_rig::rest_pose(locust_core::bot_rig::BotAvatarState::Default);
    pose.yaw = 0.3;
    pose.pitch = 0.2;
    let snap = BotSnapshot {
        pose,
        eyes: Some([">".into(), "▮".into()]),
        tone: Phosphor::Cyan,
        flash: None,
        flash_progress: 1.0,
        power_on: None,
        shut: 0.0,
        lag_x: 0.0,
        lag_y: 0.0,
        hop: false,
        blinking: false,
        screen: true,
        plush: false,
        s_seconds: 0.0,
        hovered: false,
    };
    let drawn = format!(
        "{:?}",
        draw_bot_shape(&BotDrawSpec {
            shape: "prompt",
            face: BotFace::Eyes,
            hue: "blue",
            size: 40.0,
            light_theme: false,
            presence: None,
            ring: false,
            snap: &snap
        })
    );
    let body = ViewTransform {
        draw_size: 40.0,
        scale: 0.4,
        pose,
        y_shift_100: 0.0,
    };
    let outline = outline_of("prompt");
    let visor = visor_of(&outline, BotFace::Eyes);
    let eye = [
        outline.face_x - 11.5 * outline.face_scale,
        outline.face_y + visor.y * outline.face_scale - 1.0,
    ];
    let scale = outline.face_scale * SCREEN_GLYPH_SCALE * 0.9;
    for p in glyph_shape(">").unwrap().lines[0].chunks_exact(2) {
        let point = body.map([eye[0] + p[0] * scale, eye[1] + p[1] * scale], 0.0);
        assert!(drawn.contains(&format!("{point:?}")), "{drawn}");
    }
}
#[test]
fn wears_no_mouth_on_a_screen_and_does_on_a_face_of_plastic() {
    let mut snap = BotSnapshot {
        pose: locust_core::bot_rig::rest_pose(locust_core::bot_rig::BotAvatarState::Default),
        eyes: Some(["•".into(), "•".into()]),
        tone: Phosphor::Cyan,
        flash: None,
        flash_progress: 1.0,
        power_on: None,
        shut: 0.0,
        lag_x: 0.0,
        lag_y: 0.0,
        hop: false,
        blinking: false,
        screen: true,
        plush: false,
        s_seconds: 0.0,
        hovered: false,
    };
    let draw = |snap: &BotSnapshot, face| {
        format!(
            "{:?}",
            draw_bot_shape(&BotDrawSpec {
                shape: "prompt",
                face,
                hue: "blue",
                size: 40.0,
                light_theme: false,
                presence: None,
                ring: false,
                snap
            })
        )
    };
    assert_eq!(
        draw(&snap, BotFace::Eyes).matches("Round").count(),
        draw(&snap, BotFace::Mouth).matches("Round").count()
    );
    snap.screen = false;
    assert_ne!(draw(&snap, BotFace::Eyes), draw(&snap, BotFace::Mouth));
}
#[test]
fn move_every_pair_a_state_asks_for_over_a_few_seconds() {
    for pair in [">▮", "••", "^^", "xx"] {
        let seen: std::collections::BTreeSet<_> = over(pair, 1)
            .iter()
            .map(|m| format!("{:.2},{:.2},{:.2},{}", m.dx, m.dy, m.sx, m.shown))
            .collect();
        assert!(seen.len() > 4, "{pair}");
    }
}
#[test]
fn at_work_the_cursor_blinks_and_the_prompt_stays_lit() {
    let m = over(">▮", 1);
    assert!(m.iter().any(|m| !m.shown));
    assert!(m.iter().any(|m| m.shown));
    assert!(over(">▮", 0).iter().all(|m| m.shown));
    let dark = (0..240)
        .map(|i| 1.0 + i as f64 / 30.0)
        .find(|s| !glyph_motion(">▮", 1, *s).shown)
        .unwrap();
    assert_eq!(light([">", "▮"], dark), light([">", "unknown"], dark));
}
#[test]
fn in_thought_look_up_and_about_then_bounce_in_turn_like_a_reply_being_typed() {
    let t: Vec<_> = (0..126).map(|i| 4.2 * 3.0 + i as f64 / 30.0).collect();
    let looks: Vec<_> = t
        .iter()
        .map(|s| thinking_glance(*s))
        .filter(|l| l.mix > 0.99)
        .collect();
    assert!(looks.iter().any(|l| l.yaw < -0.2));
    assert!(looks.iter().any(|l| l.yaw > 0.2));
    assert!(looks.iter().all(|l| l.pitch > 0.0));
    assert!(
        t.iter()
            .any(|s| (glyph_motion("••", 0, *s).dy - glyph_motion("••", 1, *s).dy).abs() > 0.6)
    );
    assert!(t.iter().all(|s| glyph_motion("••", 0, *s).sy > 0.5));
}
#[test]
fn are_never_flat_lines_for_work_or_thought() {
    for a in [
        FaceActivity::Working,
        FaceActivity::Delegating,
        FaceActivity::Thinking,
    ] {
        for g in eye_glyphs_for(a).into_iter().flatten() {
            assert!(!["-", "_"].contains(&g));
        }
    }
}
#[test]
fn set_off_from_where_a_still_bot_rests_second_zero_of_their_state() {
    for pair in [">▮", "••", "^^", "xx", "><", "cc", "||"] {
        for eye in [0, 1] {
            let r = glyph_motion(pair, eye, 0.0);
            let a = glyph_motion(pair, eye, 0.0001);
            assert!((a.dx - r.dx).hypot(a.dy - r.dy) < 0.002, "{pair} {eye}");
            assert_eq!(a.shown, r.shown);
            assert_eq!(glyph_motion(pair, eye, -0.2), r);
        }
        let r = glyph_motion(pair, 1, 0.0);
        assert_eq!((r.sx, r.sy, r.shown), (1.0, 1.0, true));
        if pair == ">▮" {
            assert_eq!(r.dy, 0.0);
        } else {
            assert_eq!(r.dx, 0.0);
        }
    }
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
    let f = fit_visor(circle, 50.0, 50.0, 1.0, EYES_VISOR, 3.0);
    assert!(f.half_width < EYES_VISOR.half_width);
    assert!(
        visor_outline(f)
            .iter()
            .all(|[x, y]| circle(50.0 + x, 50.0 + y))
    );
    let b = VisorBox {
        half_width: f.half_width * 1.1 + 3.0,
        half_height: f.half_height * 1.1 + 3.0,
        corner: f.corner * 1.1 + 3.0,
        ..f
    };
    assert!(
        !visor_outline(b)
            .iter()
            .all(|[x, y]| circle(50.0 + x, 50.0 + y))
    );
}
#[test]
fn reads_the_hue_of_whatever_colour_the_plastic_hands_it() {
    assert!((hue_of("hsl(219.7 89.7% 64.7%)") - 219.7).abs() < 0.005);
    assert_eq!(hue_of("#ff0000"), 0.0);
    assert_eq!(hue_of("rgb(0, 0, 255)"), 240.0);
}

// Additional painter regression: the carets stretch around each eye, independently
// of their bob. A change that only translates them fails this geometric check.
#[test]
fn the_painter_stretches_the_glyph_as_well_as_moving_it() {
    let bob = (0.2_f64 * 4.2).sin();
    let point = [
        (-11.5 - 5.2 * (1.0 + 0.06 * bob)) as f32,
        (-1.5 * bob + 2.8 * (1.0 - 0.08 * bob)) as f32,
    ];
    assert!(light(["^", "^"], 0.2).contains(&format!("{point:?}")));
}

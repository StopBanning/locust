//! Electron `a-thinking-face-looks-with-its-head.test.ts`, `describe("a screen's eyes")` (5 cases):
//! drawn directly through `draw_bot_shape` on a screen-faced bot, verifying actual stroke points
//! and stroke weights.

use super::*;
use locust_core::bot_rig::{rest_pose, BotAvatarState};

const CYCLE: f64 = 4.2;

#[derive(Debug, Clone)]
struct DrawnStroke {
    points: Vec<[f32; 2]>,
    width: f32,
}

fn parse_points(src: &str) -> Vec<[f32; 2]> {
    let mut out = Vec::new();
    let mut rest = src;
    while let Some(open) = rest.find('[') {
        rest = &rest[open + 1..];
        if rest.starts_with('[') {
            continue;
        }
        let Some(close) = rest.find(']') else { break };
        let inner = &rest[..close];
        if let Some((a, b)) = inner.split_once(',')
            && let (Ok(x), Ok(y)) = (a.trim().parse::<f32>(), b.trim().parse::<f32>())
        {
            out.push([x, y]);
        }
        rest = &rest[close + 1..];
    }
    out
}

fn lit_strokes(shape: &Shape) -> Vec<DrawnStroke> {
    let lit_col = color_from_rgb_f64(phosphor_rgb(Phosphor::Cyan).map(f64::from), 0.96);
    let want_color = format!("{lit_col:?}");
    let text = format!("{shape:?}");
    let mut out = Vec::new();
    let mut rest = text.as_str();
    while let Some(idx) = rest.find("Stroke { points: ") {
        rest = &rest[idx + "Stroke { points: ".len()..];
        let Some(end_pts) = rest.find(", closed:") else { break };
        let points = parse_points(&rest[..end_pts]);
        let after_pts = &rest[end_pts..];
        let Some(w_idx) = after_pts.find("width: ") else { break };
        let after_w = &after_pts[w_idx + "width: ".len()..];
        let Some(w_end) = after_w.find(',') else { break };
        let width: f32 = after_w[..w_end].trim().parse().unwrap();
        let Some(c_idx) = after_w.find("color: Color { ") else { break };
        let after_c = &after_w[c_idx + "color: ".len()..];
        let Some(c_end) = after_c.find('}') else { break };
        let color = &after_c[..=c_end];
        if color == want_color {
            out.push(DrawnStroke { points, width });
        }
    }
    out
}

fn draw_frame(eyes: [&str; 2], look_x: f64, look_y: f64, seconds: f64, yaw: f64, open: f64) -> Vec<DrawnStroke> {
    let mut pose = rest_pose(BotAvatarState::Default);
    pose.yaw = yaw;
    pose.pitch = 0.0;
    pose.look_x = look_x;
    pose.look_y = look_y;
    pose.eye_open = open;
    pose.blink_l = 0.0;
    pose.blink_r = 0.0;
    let snap = BotSnapshot {
        pose,
        eyes: Some([eyes[0].to_owned(), eyes[1].to_owned()]),
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
        s_seconds: seconds,
        hovered: false,
    };
    let shape = draw_bot_shape(&BotDrawSpec {
        shape: "prompt",
        face: BotFace::Eyes,
        hue: "blue",
        size: 80.0,
        light_theme: false,
        presence: None,
        ring: false,
        snap: &snap,
    });
    lit_strokes(&shape)
}

// case: keep their place on the glass while the rig looks about: its look turns the head, not the dots
#[test]
fn keep_their_place_on_the_glass_while_the_rig_looks_about() {
    let points_of = |lx: f64, ly: f64, s: f64| -> Vec<Vec<[String; 2]>> {
        draw_frame(["•", "•"], lx, ly, s, 0.0, 1.0)
            .into_iter()
            .map(|st| st.points.into_iter().map(|[x, y]| [format!("{x:.5}"), format!("{y:.5}")]).collect())
            .collect()
    };
    for seconds in [0.3, 0.9, 1.6] {
        let ahead = points_of(0.0, 0.0, seconds);
        assert_eq!(ahead.len(), 2, "two dot strokes at {seconds}");
        for (x, y) in [(4.5, 0.0), (-4.5, 3.0), (3.6, -2.4), (-9.0, 7.0)] {
            assert_eq!(points_of(x, y, seconds), ahead, "look {x},{y} at {seconds}");
        }
    }
}

// case: share one gaze: at any moment the two are the same distance apart, and level
#[test]
fn share_one_gaze_at_any_moment_the_two_are_the_same_distance_apart_and_level() {
    let mut spread: Option<f32> = None;
    for i in 0..126 {
        let seconds = CYCLE * 2.0 + f64::from(i) / 30.0;
        let strokes = draw_frame(["•", "•"], 0.0, 0.0, seconds, 0.0, 1.0);
        assert_eq!(strokes.len(), 2, "frame {i}");
        let left = strokes[0].points[0];
        let right = strokes[1].points[0];
        let m0 = glyph_motion("••", 0, seconds);
        let m1 = glyph_motion("••", 1, seconds);
        if seconds % CYCLE < 2.1 {
            assert_eq!(m0, m1);
            assert!((right[1] - left[1]).abs() < 1e-4, "level at {seconds}: {} vs {}", left[1], right[1]);
            let d = right[0] - left[0];
            let s = *spread.get_or_insert(d);
            assert!((d - s).abs() < 1e-4, "spread at {seconds}: {d} vs {s}");
        }
    }
    assert!(spread.unwrap_or(0.0) > 0.0);
}

// case: never thin in a blink: every stroke keeps its weight, open, blinking or shut
#[test]
fn never_thin_in_a_blink_every_stroke_keeps_its_weight_open_blinking_or_shut() {
    for eyes in [[">", "▮"], ["^", "^"], [">", "<"], ["x", "x"], ["|", "|"]] {
        let open: Vec<f32> = draw_frame(eyes, 0.0, 0.0, 1.0, 0.0, 1.0)
            .into_iter()
            .map(|st| st.width)
            .collect();
        assert!(!open.is_empty(), "{eyes:?}");
        for openness in [0.6, 0.3, 0.05] {
            let blinking: Vec<f32> = draw_frame(eyes, 0.0, 0.0, 1.0, 0.0, openness)
                .into_iter()
                .map(|st| st.width)
                .collect();
            assert_eq!(blinking, open, "{eyes:?} at {openness}");
        }
    }
}

// case: blink, dots too: a dot, with no length to squash, closes into the screen's dash and never thinner (2026-10-05)
#[test]
fn blink_dots_too_a_dot_closes_into_the_screens_dash_and_never_thinner() {
    let outline = outline_of("prompt");
    let scale = 80.0_f64 / 100.0;
    let eye_scale = outline.face_scale * SCREEN_GLYPH_SCALE * 0.9;
    let factor = (eye_scale * scale * 0.9 * 0.65) as f32;
    let dot_w = glyph_shape("•").unwrap().weight as f32 * factor;
    let dash_w = glyph_shape("-").unwrap().weight as f32 * factor;
    let mut last = f32::INFINITY;
    for openness in [1.0, 0.8, 0.6, 0.4, 0.2, 0.05] {
        let strokes = draw_frame(["•", "•"], 0.0, 0.0, 1.0, 0.0, openness);
        assert_eq!(strokes.len(), 2);
        for st in &strokes {
            assert!(st.width >= dash_w - 1e-4, "width {} < dash {dash_w} at {openness}", st.width);
            assert!(st.width <= dot_w + 1e-4, "width {} > dot {dot_w} at {openness}", st.width);
            assert!(st.width <= last + 1e-4, "grew at {openness}: {} > {last}", st.width);
        }
        last = strokes.iter().map(|s| s.width).fold(f32::INFINITY, f32::min);
    }
    // Shut, it is a dash: level, and 9.6 glyph units across.
    let shut = draw_frame(["•", "•"], 0.0, 0.0, 1.0, 0.0, 0.0);
    let [a, b] = [shut[0].points[0], shut[0].points[1]];
    assert!((b[1] - a[1]).abs() < 1e-4, "dash not level: {a:?} -> {b:?}");
    let want_span = (9.6 * eye_scale * scale) as f32;
    assert!(((b[0] - a[0]).abs() - want_span).abs() < 0.05, "span {} vs {want_span}", (b[0] - a[0]).abs());
}

// case: never thin as the head turns: a round dot is drawn round, the same size, however far it is turned
#[test]
fn never_thin_as_the_head_turns_a_round_dot_is_drawn_round_the_same_size_however_far_it_is_turned() {
    let facing = draw_frame(["•", "•"], 0.0, 0.0, 1.0, 0.0, 1.0);
    assert_eq!(facing.len(), 2);
    let facing_widths: Vec<f32> = facing.iter().map(|s| s.width).collect();
    for yaw in [0.3, 0.6, 0.75] {
        let turned = draw_frame(["•", "•"], 0.0, 0.0, 1.0, yaw, 1.0);
        let widths: Vec<f32> = turned.iter().map(|s| s.width).collect();
        assert_eq!(widths, facing_widths, "turned {yaw}");
        // Each dot's endpoints coincide horizontally (a round cap circle, not an oval narrowed by yaw).
        for st in &turned {
            assert!((st.points[1][0] - st.points[0][0]).abs() < 1e-4, "dot stretched at yaw {yaw}: {:?}", st.points);
        }
    }
}

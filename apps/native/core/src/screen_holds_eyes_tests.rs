//! A SCREEN HOLDS ITS EYES (Electron 0.562, `a-screen-holds-its-eyes.test.ts`): every pair a teammate can wear, on
//! screens from Prompt's own down to half its size, glancing to every extreme, at every moment of eight seconds of
//! motion, stays inside the screen's rounded rectangle, and the two eyes never touch. And what a screen glows.

use crate::bot_rig::{flash_for, glyph_motion, phosphor_for, FaceActivity};
use crate::bot_spec::{glyph_shape, screen_eye_layout, GlyphShape, Phosphor, VisorBox, EYES_VISOR, MOUTH_VISOR};

const PAIRS: [[&str; 2]; 9] = [
    [">", "▮"], ["•", "•"], ["^", "^"], ["x", "x"], [">", "<"], ["o", "o"], ["|", "|"], [">", "_"], ["-", "-"],
];
const LOOKS: [[f64; 2]; 5] = [[0.0, 0.0], [4.5, 0.0], [-4.5, 7.0], [4.5, -7.0], [-4.5, -7.0]];

fn scaled(b: VisorBox, k: f64) -> VisorBox {
    VisorBox { half_width: b.half_width * k, half_height: b.half_height * k, corner: b.corner * k, y: b.y }
}

fn screens() -> Vec<(VisorBox, f64, String)> {
    [1.0, 0.9, 0.8, 0.7, 0.6, 0.5]
        .iter()
        .flat_map(|&k| {
            [
                (scaled(EYES_VISOR, k), 1.0, format!("eyes screen x{k}")),
                (scaled(MOUTH_VISOR, k), -3.5, format!("mouth screen x{k}")),
            ]
        })
        .collect()
}

/// A disc of radius r centred at (x, y), in face units, lies inside the screen's rounded rectangle.
fn inside(b: &VisorBox, x: f64, y: f64, r: f64) -> bool {
    let hw = b.half_width - r;
    let hh = b.half_height - r;
    let corner = (b.corner - r).max(0.0);
    let dx = x.abs();
    let dy = (y - b.y).abs();
    if dx > hw || dy > hh {
        return false;
    }
    let ex = dx - (hw - corner);
    let ey = dy - (hh - corner);
    ex <= 0.0 || ey <= 0.0 || ex.hypot(ey) <= corner + 1e-9
}

fn shape(token: &str) -> GlyphShape {
    glyph_shape(token).unwrap_or_else(|| panic!("no shape for {token}"))
}

#[test]
fn keeps_every_stroke_of_every_pair_inside_the_screen_whatever_the_glance_or_the_moment() {
    let mut outside: Vec<String> = Vec::new();
    for (b, eye_y, name) in screens() {
        for pair in PAIRS {
            for look in LOOKS {
                let layout = screen_eye_layout(b, pair, look, eye_y);
                for tick in 1..=120 {
                    let seconds = tick as f64 / 15.0;
                    for eye in 0..2 {
                        let s = shape(pair[eye]);
                        let motion = glyph_motion(&pair.concat(), eye, seconds);
                        let [cx, cy] = layout.centres[eye];
                        let r = s.weight * layout.scale / 2.0;
                        let mut points: Vec<[f64; 2]> =
                            s.lines.iter().flat_map(|line| line.chunks(2).map(|p| [p[0], p[1]])).collect();
                        if let Some(ring) = s.ring {
                            for k in 0..24 {
                                let a = k as f64 / 24.0 * std::f64::consts::TAU;
                                points.push([ring * a.cos(), ring * a.sin()]);
                            }
                        }
                        for [px, py] in points {
                            let x = cx + (px * motion.sx + motion.dx) * layout.scale;
                            let y = cy + (py * motion.sy + motion.dy) * layout.scale;
                            if !inside(&b, x, y, r) && outside.len() < 5 {
                                outside.push(format!(
                                    "{name} {} look {:?} t={seconds:.2} eye {eye}: ({x:.2}, {y:.2}) r {r:.2}",
                                    pair.concat(),
                                    look
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(outside, Vec::<String>::new());
}

#[test]
fn never_lets_the_two_eyes_touch() {
    let widest = |s: &GlyphShape| {
        let xs = s.lines.iter().flat_map(|line| line.iter().step_by(2).map(|v| v.abs()));
        xs.fold(s.ring.unwrap_or(0.0), f64::max) * 1.16 + 2.0 + s.weight / 2.0
    };
    for (b, eye_y, _) in screens() {
        for pair in PAIRS {
            let layout = screen_eye_layout(b, pair, [0.0, 0.0], eye_y);
            let gap = layout.centres[1][0] - layout.centres[0][0];
            assert!(gap > (widest(&shape(pair[0])) + widest(&shape(pair[1]))) * layout.scale, "{}", pair.concat());
        }
    }
}

#[test]
fn draws_prompts_screen_at_the_mascots_size_the_room_is_there_so_the_glyphs_are_not_shrunk() {
    let layout = screen_eye_layout(EYES_VISOR, [">", "▮"], [0.0, 0.0], 1.0);
    assert!((layout.scale - 1.3).abs() < 0.05, "{}", layout.scale);
}

#[test]
fn glows_cyan_at_work_in_thought_at_rest_and_done_amber_waiting_red_stuck_green_only_as_a_flash_when_done() {
    use FaceActivity::*;
    let all = [Thinking, Working, Delegating, Responding, Waiting, Receiving, Blocked, Done, Idle];
    let glows: Vec<Phosphor> = all.iter().map(|&a| phosphor_for(a)).collect();
    use Phosphor::{Amber, Cyan, Green, Red};
    assert_eq!(glows, vec![Cyan, Cyan, Cyan, Cyan, Amber, Cyan, Red, Cyan, Cyan]);
    let flashes: Vec<Option<Phosphor>> = all.iter().map(|&a| flash_for(a)).collect();
    assert_eq!(flashes, vec![None, None, None, None, None, None, None, Some(Green), None]);
}

#[test]
fn has_a_lit_colour_and_a_glow_for_each_none_of_them_the_teammates_own() {
    for glow in [Phosphor::Cyan, Phosphor::Green, Phosphor::Amber, Phosphor::Red] {
        assert!(glow.lit_css().starts_with("hsl("), "{glow:?}");
        assert!(glow.glow_css().starts_with("hsla("), "{glow:?}");
    }
}

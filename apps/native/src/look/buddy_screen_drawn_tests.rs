//! Electron `codex-buddys-face-changes-as-a-screen-does.test.ts`, `describe('his screen, drawn')`
//! (the 3 drawn-screen cases): eyes fit strictly inside the glass across sizes, looks, glyphs and
//! motion times; eyes are sized in bot-screen units (`glass_units * w == 54`); and his ink border
//! is drawn around the glass first before the glass inside it.

use super::*;
use locust_core::buddy_moves::{buddy_moments, screen_at, ScreenRectF};
use locust_core::buddy_rig::{BUDDY_RIG_PLATE_COL, BUDDY_RIG_PLATE_ROW};

/// One parsed `Op::Fill` or `Op::Stroke` from `format!("{shape:?}")`.
#[derive(Debug)]
struct DrawnStroke {
    points: Vec<[f32; 2]>,
    width: f32,
    color: String,
}

#[derive(Debug)]
struct DrawnFill {
    points: Vec<[f32; 2]>,
    color: String,
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

fn parse_strokes(shape: &Shape) -> Vec<DrawnStroke> {
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
        let color = after_c[..=c_end].to_owned();
        out.push(DrawnStroke { points, width, color });
    }
    out
}

fn parse_fills(shape: &Shape) -> Vec<DrawnFill> {
    let text = format!("{shape:?}");
    let mut out = Vec::new();
    let mut rest = text.as_str();
    while let Some(idx) = rest.find("Fill { points: ") {
        rest = &rest[idx + "Fill { points: ".len()..];
        let Some(end_pts) = rest.find(", color: ") else { break };
        let points = parse_points(&rest[..end_pts]);
        let after_c = &rest[end_pts + ", color: ".len()..];
        let Some(c_end) = after_c.find('}') else { break };
        let color = after_c[..=c_end].to_owned();
        out.push(DrawnFill { points, color });
    }
    out
}

fn glass_at(size: f64) -> ScreenRectF {
    let rect = screen_at(&CODEX_BUDDY, BUDDY_RIG_PLATE_ROW, BUDDY_RIG_PLATE_COL).expect("plate cell");
    let k = size / 208.0;
    ScreenRectF {
        x: (size - (size * 192.0) / 208.0) / 2.0 + f64::from(rect.x) * k,
        y: f64::from(rect.y) * k,
        w: f64::from(rect.w) * k,
        h: f64::from(rect.h) * k,
    }
}

fn cyan_lit() -> Color {
    color_from_rgb_f64(phosphor_rgb(Phosphor::Cyan).map(f64::from), 0.96)
}

fn cyan_glow() -> Color {
    color_from_rgb_f64(phosphor_rgb(Phosphor::Cyan).map(f64::from), 0.28)
}

// case: keeps every eye inside its glass, however it moves and wherever he looks
#[test]
fn keeps_every_eye_inside_its_glass_however_it_moves_and_wherever_he_looks() {
    let mut pairs: Vec<[String; 2]> = vec![
        ["|".into(), "|".into()],
        ["•".into(), "•".into()],
        [">".into(), "▮".into()],
        ["^".into(), "^".into()],
        ["o".into(), "o".into()],
        [">".into(), "<".into()],
        ["c".into(), "c".into()],
    ];
    for moment in buddy_moments() {
        pairs.push([moment.eyes[0].to_owned(), moment.eyes[1].to_owned()]);
    }
    let lit = cyan_lit();
    let glow = cyan_glow();
    let lit_str = format!("{lit:?}");
    let mut drawn = 0usize;
    let mut outside = Vec::new();
    for size in [34.0, 64.0, 96.0] {
        for look in [-1.0, -0.6, -0.2, 0.0, 0.3, 0.8, 1.0] {
            let glass = glass_at(size);
            for pair in &pairs {
                let joined = format!("{}{}", pair[0], pair[1]);
                for t in [0.0, 0.4, 1.1, 2.3, 2.6, 3.1, 3.9] {
                    let motions = [glyph_motion(&joined, 0, t), glyph_motion(&joined, 1, t)];
                    let shape = paint_buddy_screen_eyes(
                        Shape::new(),
                        glass,
                        pair,
                        motions,
                        1.0,
                        1.0,
                        lit,
                        glow,
                        look,
                    );
                    for stroke in parse_strokes(&shape).into_iter().filter(|st| st.color == lit_str) {
                        drawn += 1;
                        let half = f64::from(stroke.width) / 2.0;
                        for [x, y] in stroke.points {
                            let (x, y) = (f64::from(x), f64::from(y));
                            if x - half < glass.x - 0.01
                                || x + half > glass.x + glass.w + 0.01
                                || y - half < glass.y - 0.01
                                || y + half > glass.y + glass.h + 0.01
                            {
                                outside.push(format!("{size} px, look {look}: {joined} at {t}"));
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(drawn > 2000, "drawn={drawn}");
    assert!(outside.is_empty(), "outside: {:?}", &outside[..outside.len().min(5)]);
}

// case: draws his eyes as a bot screen’s, sized to the glass in the units a bot’s screen has
#[test]
fn draws_his_eyes_as_a_bot_screens_sized_to_the_glass_in_the_units_a_bots_screen_has() {
    let glass = glass_at(96.0);
    let lit = cyan_lit();
    let glow = cyan_glow();
    let lit_str = format!("{lit:?}");
    let pair = ["|".to_owned(), "|".to_owned()];
    let motions = [glyph_motion("||", 0, 0.0), glyph_motion("||", 1, 0.0)];
    let shape = paint_buddy_screen_eyes(Shape::new(), glass, &pair, motions, 1.0, 1.0, lit, glow, 0.0);
    let lit_strokes: Vec<DrawnStroke> = parse_strokes(&shape)
        .into_iter()
        .filter(|st| st.color == lit_str)
        .collect();
    assert_eq!(lit_strokes.len(), 2, "two lit eye bars");
    let middle = (glass.x + glass.w / 2.0) as f32;
    let left_max_x = lit_strokes[0].points.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max);
    let right_min_x = lit_strokes[1].points.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    assert!(left_max_x < middle, "left={left_max_x} middle={middle}");
    assert!(right_min_x > middle, "right={right_min_x} middle={middle}");
    // Its glass is a bot screen's width in face units (EYES_VISOR's 54).
    assert!((glass_units(glass.w) * glass.w - 54.0).abs() < 1e-6);

    // And the production draw_buddy_shape path calls paint_buddy_glass + paint_buddy_screen_eyes.
    let full_buddy = static_buddy_shape("blue", 96.0, true, false, false);
    let full_lit: Vec<DrawnStroke> = parse_strokes(&full_buddy)
        .into_iter()
        .filter(|st| st.color == lit_str)
        .collect();
    assert_eq!(full_lit.len(), 2, "draw_buddy_shape draws two lit screen eyes");
}

// case: puts his ink round the glass first, then the glass inside it
#[test]
fn puts_his_ink_round_the_glass_first_then_the_glass_inside_it() {
    let glass = glass_at(96.0);
    let shape = paint_buddy_glass(
        Shape::new(),
        glass,
        CODEX_BUDDY.ink,
        CODEX_BUDDY.glass,
        1.0,
        1.0 / glass_units(glass.w),
    );
    let fills = parse_fills(&shape);
    assert!(fills.len() >= 2, "ink fill + glass fill");
    let want_ink = format!("{:?}", color_from_css(CODEX_BUDDY.ink, 1.0));
    assert_eq!(fills[0].color, want_ink, "first fill must be CODEX_BUDDY.ink");
    let min_x = |pts: &[[f32; 2]]| pts.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    let max_x = |pts: &[[f32; 2]]| pts.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max);
    assert!(min_x(&fills[0].points) < min_x(&fills[1].points));
    assert!(max_x(&fills[0].points) > max_x(&fills[1].points));

    // And draw_buddy_shape draws CODEX_BUDDY.ink fill before the glass fill on the real buddy.
    let buddy_shape = static_buddy_shape("blue", 96.0, true, false, false);
    let buddy_fills = parse_fills(&buddy_shape);
    let [r, g, b, _] = same_hue_rgba(CODEX_BUDDY.glass, 0.46, 0.125, 1.0);
    let want_glass = format!("{:?}", rgb8(r, g, b));
    let glass_idx = buddy_fills.iter().position(|f| f.color == want_glass).expect("glass fill in draw_buddy_shape");
    assert!(glass_idx > 0, "ink fill precedes glass fill");
    assert_eq!(buddy_fills[glass_idx - 1].color, want_ink, "CODEX_BUDDY.ink fill directly surrounds glass");
    assert!(min_x(&buddy_fills[glass_idx - 1].points) < min_x(&buddy_fills[glass_idx].points));
    assert!(max_x(&buddy_fills[glass_idx - 1].points) > max_x(&buddy_fills[glass_idx].points));
}

//! Vector rig-drawn bots and Codex Buddy on Rune's `Shape` canvas.
//!
//! Draws all 23 bot shapes (`BOT_SHAPES`), 11 hues (`HUES`), `eyes` / `mouth` faces,
//! CRT screen phosphor expressions (`terminal_faces`), plush vs. plastic finishes (`plush`),
//! presence badges, and Codex Buddy (`codex-buddy`) directly from the pure geometry and
//! rigs in `locust_core::bot_spec`, `locust_core::bot_rig`, `locust_core::buddy_rig`,
//! and `locust_core::buddy_moves`.
//!
//! Replaces the 440 static PNG stills previously bundled under `assets/bots/`.

use crate::look::bot_stage::{BotSnapshot, BuddySnapshot, PresenceDot};
use locust_core::bot_rig::{
    eye_openness, glyph_motion, BotAvatarPose, PowerOnLight, SCREEN_RESTING_EYES,
};
use locust_core::bot_spec::{
    anchors_of, auto_ink, body_color_of, draw_size_for, glyph_shape, hue_hex, is_locust_bot,
    mix_phosphor_rgba, outline_of, parse_css_rgb, part_pieces, parse_svg_subpaths, same_hue_rgba,
    shade_css, visor_of, visor_outline, BodyBox, BotFace, ChestMarkKind, Phosphor, BOT_SHAPES,
    CLAUDE_MARK_RAYS, DERIVED_SHAPES, HUES as CORE_HUES, SCREEN_GLYPH_SCALE,
    SUPERSAMPLE_AT_OR_BELOW,
};
use locust_core::buddy_moves::{glass_units, pet_window, screen_face_for, CODEX_BUDDY};
use locust_core::buddy_rig::{
    arm_joints, arm_rings, body_y, dumbbell_reach, ArmJoints, Vec3, head_drop, hem_of, limb, nearness,
    shorts_poly, shoulder_of, sleeve_pieces, star_points, torso_poly, ArmPose, BuddyPiece,
    BUDDY_COLOR_BAR, BUDDY_COLOR_CAP, BUDDY_COLOR_HAIR, BUDDY_COLOR_INK, BUDDY_COLOR_MARK,
    BUDDY_COLOR_PLATE, BUDDY_COLOR_PLATE_LIGHT, BUDDY_COLOR_SHADE, BUDDY_COLOR_SHIRT,
    BUDDY_COLOR_SHIRT_SHADE, BUDDY_COLOR_SHORTS, BUDDY_COLOR_SKIN, BUDDY_COLOR_STAR, BUDDY_FIST,
    BUDDY_GEAR, BUDDY_INK, BUDDY_RIG_MIDDLE, BUDDY_RIG_PLATE_COL, BUDDY_RIG_PLATE_ROW,
    BUDDY_SHIRT_MARK, DUMBBELL_HALF_HEIGHT, DUMBBELL_HALF_LENGTH, SHORTS_STARS,
};
use rune::{Color, LineCap, Shape};

pub const SHAPES: &[&str] = &DERIVED_SHAPES;
pub const ALL_SHAPES: &[&str] = &BOT_SHAPES;
pub const HUES: &[&str] = &CORE_HUES;
pub const CELL: u32 = 112;
pub const PLUSH_SHORT_BELOW: f32 = 40.0;
const PET_FRAME_HEIGHT: f64 = 200.0;

#[derive(Debug, Clone)]
pub struct BotDrawSpec<'a> {
    pub shape: &'a str,
    pub face: BotFace,
    pub hue: &'a str,
    pub size: f32,
    pub light_theme: bool,
    pub presence: Option<PresenceDot>,
    /// TeammateBot.tsx 445: the amber ring is a waiting teammate's (`activity === 'waiting'`), not its dot's.
    pub ring: bool,
    pub snap: &'a BotSnapshot,
}

#[derive(Debug, Clone)]
pub struct BuddyDrawSpec<'a> {
    pub hue: &'a str,
    pub size: f32,
    pub light_theme: bool,
    pub presence: Option<PresenceDot>,
    /// As `BotDrawSpec::ring`: TeammateBot draws the ring over a worn pet too.
    pub ring: bool,
    pub snap: &'a BuddySnapshot,
}

/// An sRGB byte colour as Rune holds it (linear light, straight alpha).
fn rgba8(r: u8, g: u8, b: u8, a: u8) -> Color {
    let channel = |v: u8| rune::color::srgb_decode(v as f32 / 255.0);
    Color { r: channel(r), g: channel(g), b: channel(b), a: a as f32 / 255.0 }
}

fn rgb8(r: u8, g: u8, b: u8) -> Color {
    rgba8(r, g, b, 255)
}

pub fn color_from_rgb_f64(rgb: [f64; 3], a: f32) -> Color {
    rgba8(
        rgb[0].clamp(0.0, 255.0).round() as u8,
        rgb[1].clamp(0.0, 255.0).round() as u8,
        rgb[2].clamp(0.0, 255.0).round() as u8,
        (a.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

pub fn color_from_css(css: &str, a: f32) -> Color {
    if let Some(rgb) = parse_css_rgb(css) {
        return color_from_rgb_f64(rgb, a);
    }
    let trimmed = css.trim().trim_start_matches('#');
    if trimmed.len() >= 6
        && let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&trimmed[0..2], 16),
            u8::from_str_radix(&trimmed[2..4], 16),
            u8::from_str_radix(&trimmed[4..6], 16),
        )
    {
        return rgba8(r, g, b, (a.clamp(0.0, 1.0) * 255.0).round() as u8);
    }
    rgba8(147, 164, 189, (a.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    a.mix(b, t.clamp(0.0, 1.0))
}

fn phosphor_rgb(tone: Phosphor) -> [u8; 3] {
    match tone {
        Phosphor::Cyan => [0x78, 0xe8, 0xff],
        Phosphor::Green => [0x74, 0xf2, 0x9b],
        Phosphor::Amber => [0xf7, 0xbe, 0x4b],
        Phosphor::Red => [0xff, 0x6b, 0x6b],
    }
}

/// The presence dot's fill (PixelFace.tsx `PRESENCE_TONE`, shell.css `.lc-presence--*`): working is monochrome
/// since 0.430 (`--lc-text-primary`), waiting amber, blocked red.
/// A pet's marks, as a bot wears them: the amber waiting ring round the whole face and the presence dot at its foot
/// (`.lc-bot__ring`, `.lc-presence` around a pet in TeammateBot.tsx).
pub fn pet_marks_shape(size: f32, ring: bool, presence: Option<PresenceDot>, light_theme: bool) -> Shape {
    let mut s = Shape::new().view_box(size, size);
    if ring {
        let r = size * 0.47;
        s = s.stroke_with(disc_points(size * 0.5, size * 0.5, r, r, 32), true, (size * 0.045).max(0.9), LineCap::Round, rgba8(245, 166, 35, 220));
    }
    if let Some(presence) = presence {
        let dot_r = (size * 0.1).max(2.2);
        let (cx, cy) = (size * 0.84, size * 0.84);
        s = s.fill(disc_points(cx, cy, dot_r * 1.32, dot_r * 1.32, 14), rgba8(14, 18, 26, 230));
        s = s.fill(disc_points(cx, cy, dot_r, dot_r, 14), presence_color(presence, light_theme));
    }
    s
}

fn presence_color(kind: PresenceDot, light_theme: bool) -> Color {
    let look = crate::look::controls::Look::new(!light_theme);
    look.c(match kind {
        PresenceDot::Working => "text-primary",
        PresenceDot::Waiting => "amber",
        PresenceDot::Blocked => "red",
    })
}

fn disc_points(cx: f32, cy: f32, rx: f32, ry: f32, steps: usize) -> Vec<[f32; 2]> {
    let n = steps.max(8);
    (0..n)
        .map(|i| {
            let a = (i as f32) * (std::f32::consts::TAU) / (n as f32);
            [cx + a.cos() * rx, cy + a.sin() * ry]
        })
        .collect()
}

fn rounded_rect_points(x: f32, y: f32, w: f32, h: f32, r: f32, steps: usize) -> Vec<[f32; 2]> {
    let r = r.min(w * 0.5).min(h * 0.5).max(0.0);
    if r <= 0.05 {
        return vec![[x, y], [x + w, y], [x + w, y + h], [x, y + h]];
    }
    let s = steps.max(3);
    let mut pts = Vec::with_capacity(s * 4);
    let corners = [
        (x + w - r, y + r, -std::f32::consts::FRAC_PI_2, 0.0),
        (x + w - r, y + h - r, 0.0, std::f32::consts::FRAC_PI_2),
        (x + r, y + h - r, std::f32::consts::FRAC_PI_2, std::f32::consts::PI),
        (x + r, y + r, std::f32::consts::PI, std::f32::consts::PI * 1.5),
    ];
    for (cx, cy, a0, a1) in corners {
        for i in 0..=s {
            let a = a0 + (a1 - a0) * (i as f32) / (s as f32);
            pts.push([cx + a.cos() * r, cy + a.sin() * r]);
        }
    }
    pts
}

#[derive(Clone, Copy)]
struct ViewTransform {
    draw_size: f32,
    /// Converts 0..100 SVG space into 0..draw_size canvas space.
    scale: f32,
    pose: BotAvatarPose,
    y_shift_100: f64,
}

impl ViewTransform {
    fn map(&self, p: [f64; 2], depth_z: f64) -> [f32; 2] {
        let cx = 50.0;
        let cy = 50.0;
        let dx = (p[0] - cx) * self.pose.sx;
        let dy = (p[1] - cy) * self.pose.sy;
        let yaw_scale = self.pose.yaw.cos().clamp(0.45, 1.0);
        let pitch_scale = self.pose.pitch.cos().clamp(0.55, 1.0);
        let (sin_r, cos_r) = (self.pose.roll + self.pose.whirl_angle).sin_cos();
        let x0 = dx * yaw_scale;
        let y0 = dy * pitch_scale;
        let rx = x0 * cos_r - y0 * sin_r;
        let ry = x0 * sin_r + y0 * cos_r;
        let ext_x = self.pose.yaw.sin() * depth_z;
        let ext_y = self.pose.pitch.sin() * depth_z;
        let nx = cx + rx + self.pose.x * 1.1 + ext_x;
        let ny = cy + ry + self.pose.y * 1.1 + self.y_shift_100 + ext_y;
        [(nx as f32) * self.scale, (ny as f32) * self.scale]
    }
}

fn transform_poly(pts: &[[f64; 2]], vt: &ViewTransform, depth_z: f64) -> Vec<[f32; 2]> {
    pts.iter().map(|&p| vt.map(p, depth_z)).collect()
}

fn bounds_of_subpaths(subpaths: &[Vec<[f64; 2]>]) -> BodyBox {
    let mut left = f64::MAX;
    let mut top = f64::MAX;
    let mut right = f64::MIN;
    let mut bottom = f64::MIN;
    for poly in subpaths {
        for &[x, y] in poly {
            left = left.min(x);
            top = top.min(y);
            right = right.max(x);
            bottom = bottom.max(y);
        }
    }
    if left >= right || top >= bottom {
        return BodyBox {
            left: 0.2,
            top: 0.2,
            right: 0.8,
            bottom: 0.8,
        };
    }
    BodyBox {
        left: (left / 100.0).clamp(0.05, 0.45),
        top: (top / 100.0).clamp(0.05, 0.45),
        right: (right / 100.0).clamp(0.55, 0.95),
        bottom: (bottom / 100.0).clamp(0.55, 0.95),
    }
}

fn draw_extruded_poly(
    mut s: Shape,
    poly: &[[f64; 2]],
    vt: &ViewTransform,
    base_color: Color,
    shade_color: Color,
    max_depth: f64,
    plush: bool,
) -> Shape {
    if poly.len() < 3 {
        return s;
    }
    let layers = 7usize;
    for idx in (1..=layers).rev() {
        let frac = (idx as f64) / (layers as f64);
        let depth = -frac * max_depth;
        let c = lerp_color(base_color, shade_color, (0.35 + 0.55 * frac) as f32);
        let mapped = transform_poly(poly, vt, depth);
        s = s.fill(mapped, c);
    }
    let front = transform_poly(poly, vt, 0.0);
    s = s.fill(front.clone(), base_color);

    if plush {
        let tuft_len = if vt.draw_size < PLUSH_SHORT_BELOW * 2.0 {
            1.4_f32
        } else {
            2.3_f32
        };
        let stroke_w = if vt.draw_size < PLUSH_SHORT_BELOW * 2.0 {
            (1.6 * vt.scale).max(0.6)
        } else {
            (2.4 * vt.scale).max(0.8)
        };
        let wool_edge = lerp_color(base_color, Color::WHITE, 0.16).alpha(0.72);
        s = s.stroke_with(front.clone(), true, stroke_w, LineCap::Round, wool_edge);
        let step = (front.len() / 16).max(1);
        for i in (0..front.len()).step_by(step) {
            let a = front[i];
            let b = front[(i + 1) % front.len()];
            let dx = b[0] - a[0];
            let dy = b[1] - a[1];
            let len = dx.hypot(dy).max(1e-4);
            let nx = dy / len;
            let ny = -dx / len;
            let tip = [
                a[0] + nx * tuft_len * vt.scale,
                a[1] + ny * tuft_len * vt.scale,
            ];
            s = s.line(a, tip, stroke_w * 0.72, LineCap::Round, wool_edge);
        }
    } else {
        let rim = lerp_color(base_color, Color::WHITE, 0.28).alpha(0.42);
        s = s.stroke_with(
            front,
            true,
            (1.5 * vt.scale).max(0.55),
            LineCap::Round,
            rim,
        );
    }
    s
}

fn draw_plastic_sheen(mut s: Shape, bounds: BodyBox, vt: &ViewTransform) -> Shape {
    let cx = ((bounds.left + bounds.right) * 0.5) * 100.0;
    let cy = ((bounds.top + bounds.bottom) * 0.5) * 100.0;
    let rx_100 = ((bounds.right - bounds.left) * 50.0).max(15.0);
    let ry_100 = ((bounds.bottom - bounds.top) * 50.0).max(15.0);
    let spec_pos = vt.map([cx - rx_100 * 0.28, cy - ry_100 * 0.34], 0.0);
    let rx = (rx_100 as f32) * 0.30 * vt.scale;
    let ry = (ry_100 as f32) * 0.19 * vt.scale;
    s = s.fill(
        disc_points(spec_pos[0], spec_pos[1], rx.max(0.8), ry.max(0.5), 14),
        rgba8(255, 255, 255, 50),
    );
    let glint_pos = vt.map([cx - rx_100 * 0.32, cy - ry_100 * 0.38], 0.0);
    s = s.fill(
        disc_points(
            glint_pos[0],
            glint_pos[1],
            (rx * 0.45).max(0.45),
            (ry * 0.45).max(0.35),
            10,
        ),
        rgba8(255, 255, 255, 86),
    );
    s
}

/// The colours of a mascot's chest mark (Bot.tsx `chest`): a shade of the body's own colour, deeper, under it; then
/// the mark, pale where the body's light falls (top left) and toned toward the body's colour where it does not.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ChestInk {
    pub shade: [u8; 4],
    pub pale: [u8; 4],
    pub dark: [u8; 4],
}

pub(crate) fn chest_ink(mark: ChestMarkKind, body_css: &str) -> ChestInk {
    let shade = same_hue_rgba(body_css, 0.6, 0.26, 0.6);
    match mark {
        ChestMarkKind::Prompt => ChestInk {
            shade,
            pale: same_hue_rgba(body_css, 0.9, 0.92, 1.0),
            dark: same_hue_rgba(body_css, 0.75, 0.78, 1.0),
        },
        ChestMarkKind::Spark => ChestInk {
            shade,
            pale: [255, 243, 230, 255],
            dark: same_hue_rgba(body_css, 0.6, 0.83, 1.0),
        },
    }
}

/// The lit mark's gradient (pale from 0 to 0.45 of the way from top left to bottom right, then toward dark), as the
/// canvas blends it: per channel, in sRGB.
pub(crate) fn chest_gradient(ink: ChestInk, t: f64) -> [u8; 4] {
    let k = ((t.clamp(0.0, 1.0) - 0.45) / 0.55).clamp(0.0, 1.0);
    let mix = |a: u8, b: u8| (f64::from(a) + (f64::from(b) - f64::from(a)) * k).round() as u8;
    [mix(ink.pale[0], ink.dark[0]), mix(ink.pale[1], ink.dark[1]), mix(ink.pale[2], ink.dark[2]), 255]
}

/// Alpha the lit mark is painted at (Bot.tsx `globalAlpha = 0.94`).
const CHEST_LIT_ALPHA: f64 = 0.94;

fn draw_chest_mark(
    mut s: Shape,
    outline: &locust_core::bot_spec::Outline,
    vt: &ViewTransform,
    body_css: &str,
) -> Shape {
    let Some(chest) = outline.chest else {
        return s;
    };
    let (cx, cy, h) = (chest.x, chest.y, chest.size);
    let ink = chest_ink(chest.mark, body_css);
    // The shade is the mark again, a touch down and right of it (h * 0.04, h * 0.06).
    let (shade_dx, shade_dy) = (h * 0.04, h * 0.06);
    let rgba = |c: [u8; 4], alpha: f64| rgba8(c[0], c[1], c[2], (alpha.clamp(0.0, 1.0) * 255.0).round() as u8);
    // Where along the light's way (from (x - h/2, y - h/2) to (x + h/2, y + h/2)) a point lies, 0 to 1.
    let light_at = |px: f64, py: f64| ((px - (cx - h * 0.5)) + (py - (cy - h * 0.5))) / (2.0 * h);
    match chest.mark {
        ChestMarkKind::Prompt => {
            // The Codex mascot's `>_`: h * 0.6 / 0.4 / 0.2 / 0.1 / 0.62, drawn h * 0.27 thick.
            let w = ((h * 0.27) as f32 * vt.scale).max(0.75);
            let chevron = [[cx - h * 0.6, cy - h * 0.4], [cx - h * 0.2, cy], [cx - h * 0.6, cy + h * 0.4]];
            let underscore = [[cx + h * 0.1, cy + h * 0.4], [cx + h * 0.62, cy + h * 0.4]];
            let shade = rgba(ink.shade, f64::from(ink.shade[3]) / 255.0);
            let map_by = |p: [f64; 2], dx: f64, dy: f64| vt.map([p[0] + dx, p[1] + dy], 0.0);
            s = s.stroke_with(chevron.map(|p| map_by(p, shade_dx, shade_dy)).to_vec(), false, w, LineCap::Round, shade);
            s = s.line(map_by(underscore[0], shade_dx, shade_dy), map_by(underscore[1], shade_dx, shade_dy), w, LineCap::Round, shade);
            // The mark, lit as the body is: each stretch takes the light where it lies.
            let stretch = |s: Shape, a: [f64; 2], b: [f64; 2]| -> Shape {
                const PIECES: usize = 6;
                let mut s = s;
                for i in 0..PIECES {
                    let (k0, k1) = (i as f64 / PIECES as f64, (i + 1) as f64 / PIECES as f64);
                    let at = |k: f64| [a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k];
                    let mid = at((k0 + k1) * 0.5);
                    let colour = rgba(chest_gradient(ink, light_at(mid[0], mid[1])), CHEST_LIT_ALPHA);
                    s = s.line(map_by(at(k0), 0.0, 0.0), map_by(at(k1), 0.0, 0.0), w, LineCap::Round, colour);
                }
                s
            };
            s = stretch(s, chevron[0], chevron[1]);
            s = stretch(s, chevron[1], chevron[2]);
            s = stretch(s, underscore[0], underscore[1]);
        }
        ChestMarkKind::Spark => {
            // Bot.tsx chest(): a ray per entry, narrow at its root, its tip cut square with the corners off, round a solid middle.
            let radius = h / 2.0;
            let shade = rgba(ink.shade, f64::from(ink.shade[3]) / 255.0);
            let mut painted: Vec<(Vec<[f64; 2]>, f64)> = Vec::new();
            for [degrees, reach, width] in CLAUDE_MARK_RAYS {
                let angle = (degrees - 90.0).to_radians();
                let (uy, ux) = angle.sin_cos();
                let point = |along: f64, across: f64| [cx + ux * along - uy * across, cy + uy * along + ux * across];
                let tip = radius * reach;
                let wide = radius * 0.11 * width;
                let root = radius * 0.085 * width;
                let corner = wide * 0.5;
                let points = vec![point(0.0, -root), point(tip - corner, -wide), point(tip, -wide + corner), point(tip, wide - corner), point(tip - corner, wide), point(0.0, root)];
                let centre = points.iter().fold([0.0, 0.0], |c, p| [c[0] + p[0] / 6.0, c[1] + p[1] / 6.0]);
                painted.push((points, light_at(centre[0], centre[1])));
            }
            let middle: Vec<[f64; 2]> = (0..16)
                .map(|i| {
                    let a = i as f64 / 16.0 * std::f64::consts::TAU;
                    [cx + a.cos() * radius * 0.25, cy + a.sin() * radius * 0.25]
                })
                .collect();
            painted.push((middle, light_at(cx, cy)));
            for (points, _) in &painted {
                s = s.fill(points.iter().map(|p| vt.map([p[0] + shade_dx, p[1] + shade_dy], 0.0)).collect::<Vec<_>>(), shade);
            }
            for (points, t) in &painted {
                let colour = rgba(chest_gradient(ink, *t), CHEST_LIT_ALPHA);
                s = s.fill(points.iter().map(|p| vt.map(*p, 0.0)).collect::<Vec<_>>(), colour);
            }
        }
    }
    s
}

fn paint_glyph_at(
    mut s: Shape,
    glyph: &str,
    cx: f64,
    cy: f64,
    eye_scale: f64,
    open_y: f64,
    map_pt: &dyn Fn([f64; 2]) -> [f32; 2],
    stroke_scale: f32,
    lit: Color,
    glow: Color,
) -> Shape {
    let Some(gs) = glyph_shape(glyph) else {
        return s;
    };
    if glyph == "•" {
        let (line, weight) = locust_core::bot_spec::blinking_dot(1.0 - open_y);
        let points = vec![map_pt([cx + line[0] * eye_scale, cy + line[1] * eye_scale]),
            map_pt([cx + line[2] * eye_scale, cy + line[3] * eye_scale])];
        let w = (weight as f32 * eye_scale as f32 * stroke_scale * 0.65).max(0.6);
        return s.stroke_with(points.clone(), false, w * 1.85, LineCap::Round, glow)
            .stroke_with(points, false, w, LineCap::Round, lit);
    }
    let w = ((gs.weight as f32) * (eye_scale as f32) * stroke_scale * 0.65).max(0.6);
    for seg in gs.lines {
        let mut pts = Vec::with_capacity(seg.len() / 2);
        for chunk in seg.chunks_exact(2) {
            pts.push(map_pt([
                cx + chunk[0] * eye_scale,
                cy + chunk[1] * eye_scale * open_y,
            ]));
        }
        if pts.len() >= 2 {
            s = s.stroke_with(pts.clone(), gs.closed, w * 1.85, LineCap::Round, glow);
            s = s.stroke_with(pts, gs.closed, w, LineCap::Round, lit);
        }
    }
    if let Some(r) = gs.ring {
        let c = map_pt([cx, cy]);
        let rx = (r * eye_scale) as f32 * stroke_scale;
        let ry = (r * eye_scale * open_y.abs().max(0.18)) as f32 * stroke_scale;
        let pts = disc_points(c[0], c[1], rx.max(0.5), ry.max(0.4), 14);
        s = s.stroke_with(pts.clone(), true, w * 1.8, LineCap::Round, glow);
        s = s.stroke_with(pts, true, w, LineCap::Round, lit);
    }
    s
}

#[allow(clippy::too_many_arguments)]
fn draw_screen_panel(
    mut s: Shape,
    poly: &[[f32; 2]],
    tone: Phosphor,
    flash: Option<(Phosphor, f64)>,
    flash_progress: f64,
    power_on: Option<PowerOnLight>,
    pair: &[String; 2],
    left_eye: [f64; 2],
    right_eye: [f64; 2],
    eye_scale: f64,
    open_l: f64,
    open_r: f64,
    map_pt: &dyn Fn([f64; 2]) -> [f32; 2],
    stroke_scale: f32,
    s_seconds: f64,
    shut: f64,
) -> Shape {
    s = draw_screen_ground(s, poly, tone);
    draw_screen_light(
        s, poly, tone, flash, flash_progress, power_on, pair, left_eye, right_eye, eye_scale, open_l, open_r, map_pt,
        stroke_scale, s_seconds, shut,
    )
}

/// A screen's glass: dark, a hint of its phosphor, and its sheen.
fn draw_screen_ground(mut s: Shape, poly: &[[f32; 2]], tone: Phosphor) -> Shape {
    s = s.fill(poly.to_vec(), rgb8(12, 16, 22));
    let [r, g, b] = phosphor_rgb(tone);
    s = s.fill(poly.to_vec(), color_from_rgb_f64([r as f64, g as f64, b as f64], 0.14));

    if poly.len() >= 4 {
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        for &[px, py] in poly {
            min_x = min_x.min(px);
            min_y = min_y.min(py);
            max_x = max_x.max(px);
            max_y = max_y.max(py);
        }
        let gw = (max_x - min_x) * 0.55;
        let gh = (max_y - min_y) * 0.22;
        if gw > 1.0 && gh > 0.5 {
            s = s.fill(
                rounded_rect_points(
                    min_x + (max_x - min_x) * 0.12,
                    min_y + (max_y - min_y) * 0.14,
                    gw,
                    gh,
                    gh * 0.5,
                    4,
                ),
                rgba8(255, 255, 255, 26),
            );
        }
    }

    s
}

/// What a screen shows on its glass: a finish's flash, the eyes in their light, or its switching on.
#[allow(clippy::too_many_arguments)]
fn draw_screen_light(
    mut s: Shape,
    poly: &[[f32; 2]],
    tone: Phosphor,
    flash: Option<(Phosphor, f64)>,
    flash_progress: f64,
    power_on: Option<PowerOnLight>,
    pair: &[String; 2],
    left_eye: [f64; 2],
    right_eye: [f64; 2],
    eye_scale: f64,
    open_l: f64,
    open_r: f64,
    map_pt: &dyn Fn([f64; 2]) -> [f32; 2],
    stroke_scale: f32,
    s_seconds: f64,
    shut: f64,
) -> Shape {
    let own = phosphor_rgb(tone).map(f64::from);
    let rgb = if let Some((colour, _)) = flash.filter(|_| s_seconds > 0.0) {
        let from = phosphor_rgb(colour).map(f64::from);
        let k = flash_progress.clamp(0.0, 1.0);
        std::array::from_fn(|i| from[i] + (own[i] - from[i]) * k)
    } else {
        own
    };
    let lit_col = color_from_rgb_f64(rgb, 0.96);
    let glow_col = color_from_rgb_f64(rgb, 0.28);

    if let Some(p_light) = power_on.filter(|_| s_seconds > 0.0) {
        let line_k = p_light.width.clamp(0.0, 1.0);
        let phase_eyes = p_light.eyes;
        if p_light.height > 0.0 && p_light.alpha > 0.0 && !poly.is_empty() {
            let mid = poly.iter().map(|p| p[1]).sum::<f32>() / poly.len() as f32;
            let glass: Vec<[f32; 2]> = poly.iter().map(|p| [p[0], mid + (p[1] - mid) * p_light.height as f32]).collect();
            s = s.fill(glass, lit_col.alpha(p_light.alpha as f32));
        }
        if line_k > 0.02 && p_light.alpha > 0.0 {
            let mid_y = (left_eye[1] + right_eye[1]) * 0.5;
            let span = ((right_eye[0] - left_eye[0]).abs() * 0.72).max(eye_scale * 8.0) * line_k;
            let cx = (left_eye[0] + right_eye[0]) * 0.5;
            s = s.line(
                map_pt([cx - span, mid_y]),
                map_pt([cx + span, mid_y]),
                (1.1 * stroke_scale).max(0.8),
                LineCap::Round,
                rgba8(235, 255, 255, (p_light.alpha.clamp(0.0, 1.0) * 230.0) as u8),
            );
        }
        if phase_eyes > 0.05 {
            let pair_str = format!("{}{}", pair[0], pair[1]);
            for (idx, (glyph, pos, open)) in
                [(pair[0].as_str(), left_eye, open_l), (pair[1].as_str(), right_eye, open_r)]
                    .into_iter()
                    .enumerate()
            {
                let gm = glyph_motion(&pair_str, idx, s_seconds);
                if !gm.shown {
                    continue;
                }
                let gpos = [pos[0] + gm.dx * eye_scale, pos[1] + gm.dy * eye_scale];
                s = paint_glyph_at(
                    s,
                    glyph,
                    gpos[0],
                    gpos[1],
                    eye_scale,
                    (open * (1.0 - shut) * phase_eyes).max(0.08),
                    &|pt| map_pt([
                        gpos[0] + (pt[0] - gpos[0]) * gm.sx,
                        gpos[1] + (pt[1] - gpos[1]) * gm.sy,
                    ]),
                    stroke_scale,
                    lit_col,
                    glow_col,
                );
            }
        }
        return s;
    }

    let pair_str = format!("{}{}", pair[0], pair[1]);
    for (idx, (glyph, pos, open)) in
        [(pair[0].as_str(), left_eye, open_l), (pair[1].as_str(), right_eye, open_r)]
            .into_iter()
            .enumerate()
    {
        let gm = glyph_motion(&pair_str, idx, s_seconds);
        if !gm.shown {
            continue;
        }
        let gpos = [pos[0] + gm.dx * eye_scale, pos[1] + gm.dy * eye_scale];
        let eff_open = (open * (1.0 - shut * 0.9)).max(0.08);
        s = paint_glyph_at(
            s,
            glyph,
            gpos[0],
            gpos[1],
            eye_scale,
            eff_open,
            &|pt| map_pt([
                gpos[0] + (pt[0] - gpos[0]) * gm.sx,
                gpos[1] + (pt[1] - gpos[1]) * gm.sy,
            ]),
            stroke_scale,
            lit_col,
            glow_col,
        );
    }
    s
}

pub fn draw_bot_shape(spec: &BotDrawSpec<'_>) -> Shape {
    let draw_size = draw_size_for(spec.size);
    let scale = draw_size / 100.0;
    let pose = spec.snap.pose;
    let hop_lift = if spec.snap.hop { 3.5 } else { 0.0 };
    let y_shift_100 = -(pose.breath * 0.8 + hop_lift);
    let vt = ViewTransform {
        draw_size,
        scale,
        pose,
        y_shift_100,
    };

    let shape_key = if BOT_SHAPES.contains(&spec.shape) {
        spec.shape
    } else {
        "circle"
    };
    let outline = outline_of(shape_key);
    let hex_opt = hue_hex(spec.hue, spec.light_theme);
    let body_css = body_color_of(shape_key, hex_opt);
    let shade_css_str = shade_css(&body_css, -0.22, -0.05);
    let ink_css = auto_ink(&body_css);
    let body_col = color_from_css(&body_css, 1.0);
    let shade_col = color_from_css(&shade_css_str, 1.0);
    let eye_col = color_from_css(ink_css, 0.96);

    let subpaths = parse_svg_subpaths(&outline.body);
    let bounds = bounds_of_subpaths(&subpaths);
    let anchor = anchors_of(bounds);

    let mut s = Shape::new().view_box(draw_size, draw_size);

    // Amber approval/waiting ring around body anchor.
    if spec.ring {
        let rc = [
            ((anchor.ring.left + anchor.ring.side * 0.5) as f32) * draw_size,
            ((anchor.ring.top + anchor.ring.side * 0.5) as f32) * draw_size,
        ];
        let rr = (anchor.ring.side as f32 * 0.5) * draw_size;
        s = s.stroke_with(
            disc_points(rc[0], rc[1], rr, rr, 32),
            true,
            (draw_size * 0.045).max(0.9),
            LineCap::Round,
            rgba8(245, 166, 35, 220),
        );
    }

    // Separate parts (e.g. hopper antennae/wings, critter/cat ears, swarm orbiters) extruded cleanly.
    if let Some(parts_str) = &outline.parts {
        let pieces = part_pieces(parts_str);
        let depth = outline.parts_depth.unwrap_or(2.8);
        let part_col = if is_locust_bot(shape_key) {
            shade_col
        } else {
            body_col
        };
        for piece in pieces {
            for poly in parse_svg_subpaths(piece) {
                if poly.len() >= 3 {
                    s = draw_extruded_poly(
                        s,
                        &poly,
                        &vt,
                        part_col,
                        shade_col,
                        depth,
                        spec.snap.plush,
                    );
                }
            }
        }
    }

    // Main body contour(s) extruded in depth layers.
    for poly in &subpaths {
        s = draw_extruded_poly(s, poly, &vt, body_col, shade_col, 4.2, spec.snap.plush);
    }
    if !spec.snap.plush {
        s = draw_plastic_sheen(s, bounds, &vt);
    }
    s = draw_chest_mark(s, &outline, &vt, &body_css);

    // Visor & Face expressions in 0..100 face space.
    let gaze_x = (pose.look_x + spec.snap.lag_x * 0.25).clamp(-1.3, 1.3) * 3.2;
    let gaze_y = (pose.look_y + spec.snap.lag_y * 0.25).clamp(-1.1, 1.1) * 2.4;
    let visor = visor_of(&outline, spec.face);
    let visor_pts_100: Vec<[f64; 2]> = visor_outline(visor)
        .into_iter()
        .map(|[vx, vy]| [outline.face_x + vx * outline.face_scale + gaze_x * 0.35, outline.face_y + vy * outline.face_scale + gaze_y * 0.35])
        .collect();
    let visor_poly = transform_poly(&visor_pts_100, &vt, 0.0);

    let eye_sep = 11.5 * outline.face_scale;
    let left_eye = [
        outline.face_x + gaze_x - eye_sep,
        outline.face_y + visor.y * outline.face_scale - 1.0 + gaze_y,
    ];
    let right_eye = [
        outline.face_x + gaze_x + eye_sep,
        outline.face_y + visor.y * outline.face_scale - 1.0 + gaze_y,
    ];
    let open_l = (pose.eye_open * (1.0 - pose.blink_l) * (1.0 - spec.snap.shut)).clamp(0.05, 1.0);
    let open_r = (pose.eye_open * (1.0 - pose.blink_r) * (1.0 - spec.snap.shut)).clamp(0.05, 1.0);

    if spec.snap.screen {
        let pair = spec
            .snap
            .eyes
            .clone()
            .unwrap_or_else(|| [SCREEN_RESTING_EYES[0].to_owned(), SCREEN_RESTING_EYES[1].to_owned()]);
        let eye_scale = outline.face_scale * SCREEN_GLYPH_SCALE * 0.9;
        s = draw_screen_panel(
            s,
            &visor_poly,
            spec.snap.tone,
            spec.snap.flash,
            spec.snap.flash_progress,
            spec.snap.power_on,
            &pair,
            left_eye,
            right_eye,
            eye_scale,
            open_l,
            open_r,
            &|pt| vt.map(pt, 0.0),
            scale * 0.9,
            spec.snap.s_seconds,
            spec.snap.shut,
        );
    } else {
        s = s.fill(visor_poly, rgba8(14, 18, 26, 232));
        let eye_rx = (3.4 * outline.face_scale as f32) * scale;
        let eye_ry_base = (4.8 * outline.face_scale as f32) * scale;
        let lp = vt.map(left_eye, 0.0);
        let rp = vt.map(right_eye, 0.0);
        for (p, open) in [(lp, open_l), (rp, open_r)] {
            if open < 0.22 {
                s = s.line(
                    [p[0] - eye_rx, p[1]],
                    [p[0] + eye_rx, p[1]],
                    (2.2 * scale).max(0.8),
                    LineCap::Round,
                    Color::WHITE,
                );
            } else {
                let ry = (eye_ry_base * (open as f32)).max(0.6);
                s = s.fill(disc_points(p[0], p[1], eye_rx, ry, 14), Color::WHITE);
                if ry > 1.3 {
                    s = s.fill(
                        disc_points(
                            p[0] - eye_rx * 0.28,
                            p[1] - ry * 0.28,
                            eye_rx * 0.3,
                            ry * 0.25,
                            8,
                        ),
                        rgba8(255, 255, 255, 200),
                    );
                }
            }
        }
        if spec.face == BotFace::Mouth {
            let mw = 9.0 * outline.face_scale;
            let my = outline.face_y + (visor.y + 8.5) * outline.face_scale + gaze_y;
            let smile_curve: Vec<[f32; 2]> = (-4..=4)
                .map(|i| {
                    let t = (i as f64) / 4.0;
                    let sag = (1.0 - t * t) * 3.2 * (1.0 + pose.laugh * 0.5);
                    vt.map([outline.face_x + gaze_x + t * mw, my + sag], 0.0)
                })
                .collect();
            let sw = ((eye_openness(spec.size as f64) as f32) * scale * 1.4).max(0.8);
            s = s.stroke_with(smile_curve, false, sw, LineCap::Round, Color::WHITE);
        }
    }

    // Presence dot using anchored `(dot.x, dot.y)`.
    if let Some(presence) = spec.presence {
        let dot_r = (draw_size * 0.135).max(2.2);
        let cx = (anchor.dot.x as f32) * draw_size;
        let cy = (anchor.dot.y as f32) * draw_size;
        s = s.fill(
            disc_points(cx, cy, dot_r * 1.32, dot_r * 1.32, 14),
            rgba8(14, 18, 26, 230),
        );
        s = s.fill(disc_points(cx, cy, dot_r, dot_r, 14), presence_color(presence, spec.light_theme));
    }

    let _ = (SUPERSAMPLE_AT_OR_BELOW, same_hue_rgba, mix_phosphor_rgba);
    s
}

fn draw_buddy_pieces(
    mut s: Shape,
    pieces: &[BuddyPiece],
    fill: Color,
    stroke: Option<(f32, Color)>,
    map: &dyn Fn([f64; 2]) -> [f32; 2],
    scale: f32,
) -> Shape {
    for piece in pieces {
        match piece {
            BuddyPiece::Disc { x, y, r } => {
                let p = map([*x, *y]);
                let pr = (*r as f32) * scale;
                let pts = disc_points(p[0], p[1], pr, pr, 16);
                s = s.fill(pts.clone(), fill);
                if let Some((w, sc)) = stroke {
                    s = s.stroke_with(pts, true, w, LineCap::Round, sc);
                }
            }
            BuddyPiece::Poly(pts) => {
                let mapped: Vec<[f32; 2]> = pts.iter().map(|&p| map(p)).collect();
                s = s.fill(mapped.clone(), fill);
                if let Some((w, sc)) = stroke {
                    s = s.stroke_with(mapped, true, w, LineCap::Round, sc);
                }
            }
        }
    }
    s
}

#[allow(clippy::too_many_arguments)]
fn draw_dumbbell(
    mut s: Shape,
    hand: [f64; 2],
    arm: &ArmPose,
    hand_z: f64,
    ink_w: f32,
    ink_col: Color,
    plate_col: Color,
    plate_light: Color,
    bar_col: Color,
    map: &dyn Fn([f64; 2]) -> [f32; 2],
    scale: f32,
) -> Shape {
    let near = nearness(hand_z);
    let hand3 = Vec3 { x: hand[0], y: hand[1], z: hand_z };
    let reach = dumbbell_reach(arm, &ArmJoints { shoulder: hand3, elbow: hand3, hand: hand3 });
    let dir = [arm.grip.cos(), arm.grip.sin() * 0.4];
    let ax = [
        hand[0] - dir[0] * (DUMBBELL_HALF_LENGTH * 0.72 * near),
        hand[1] - dir[1] * (DUMBBELL_HALF_LENGTH * 0.72 * near),
    ];
    let bx = [
        hand[0] + dir[0] * (DUMBBELL_HALF_LENGTH * 0.72 * near),
        hand[1] + dir[1] * (DUMBBELL_HALF_LENGTH * 0.72 * near),
    ];
    s = s.line(
        map(ax),
        map(bx),
        ((3.4 * near) as f32) * scale,
        LineCap::Round,
        bar_col,
    );
    let rx = ((reach.x.abs().max(5.5)) as f32) * scale * 0.42;
    let ry = ((DUMBBELL_HALF_HEIGHT * near) as f32) * scale * 0.85;
    for end in [ax, bx] {
        let p = map(end);
        let pts = disc_points(p[0], p[1], rx.max(1.5), ry.max(2.0), 14);
        s = s.fill(pts.clone(), plate_col);
        s = s.fill(
            disc_points(
                p[0] - rx * 0.25,
                p[1] - ry * 0.2,
                (rx * 0.55).max(0.8),
                (ry * 0.65).max(1.0),
                10,
            ),
            plate_light,
        );
        s = s.stroke_with(pts, true, ink_w * 0.8, LineCap::Round, ink_col);
    }
    s
}

pub fn draw_buddy_shape(spec: &BuddyDrawSpec<'_>) -> Shape {
    let draw_size = draw_size_for(spec.size);
    let win = pet_window(PET_FRAME_HEIGHT, BUDDY_RIG_MIDDLE);
    let scale = draw_size / (win.side as f32);
    let map = |pt: [f64; 2]| -> [f32; 2] {
        [
            ((pt[0] - win.left) as f32) * scale,
            ((pt[1] - win.top) as f32) * scale,
        ]
    };

    let ink = color_from_css(BUDDY_COLOR_INK, 1.0);
    let ink_w = (BUDDY_INK as f32 * scale * 0.75).max(0.75);
    let skin = color_from_css(BUDDY_COLOR_SKIN, 1.0);
    let skin_shade = color_from_css(BUDDY_COLOR_SHADE, 1.0);
    let shirt = color_from_css(BUDDY_COLOR_SHIRT, 1.0);
    let shirt_shade = color_from_css(BUDDY_COLOR_SHIRT_SHADE, 1.0);
    let shorts = color_from_css(BUDDY_COLOR_SHORTS, 1.0);
    let star_col = color_from_css(BUDDY_COLOR_STAR, 1.0);
    let cap_blue = hue_hex(spec.hue, spec.light_theme)
        .filter(|_| spec.hue != "pearl")
        .map(|h| color_from_css(h, 1.0))
        .unwrap_or_else(|| color_from_css(BUDDY_COLOR_CAP, 1.0));
    let cap_dark = lerp_color(cap_blue, Color::BLACK, 0.32);
    let hair = color_from_css(BUDDY_COLOR_HAIR, 1.0);
    let plate = color_from_css(BUDDY_COLOR_PLATE, 1.0);
    let plate_light = color_from_css(BUDDY_COLOR_PLATE_LIGHT, 1.0);
    let bar = color_from_css(BUDDY_COLOR_BAR, 1.0);

    let dip = spec.snap.pose.dip;
    let left_joints = arm_joints(&spec.snap.pose.left, -1, dip);
    let right_joints = arm_joints(&spec.snap.pose.right, 1, dip);
    let mut s = Shape::new().view_box(draw_size, draw_size);

    if spec.ring {
        let r = draw_size * 0.47;
        s = s.stroke_with(
            disc_points(draw_size * 0.5, draw_size * 0.5, r, r, 32),
            true,
            (draw_size * 0.035).max(1.0),
            LineCap::Round,
            rgba8(245, 166, 35, 220),
        );
    }

    let draw_arm = |mut shape: Shape,
                    arm_pose: &ArmPose,
                    joints: &locust_core::buddy_rig::ArmJoints|
     -> Shape {
        let (upper, fore) = arm_rings(joints, arm_pose);
        let skin_pieces = [limb(&upper), limb(&fore)].concat();
        shape = draw_buddy_pieces(shape, &skin_pieces, skin, Some((ink_w * 0.7, ink)), &map, scale);
        let sleeves = sleeve_pieces(joints);
        shape = draw_buddy_pieces(shape, &sleeves, shirt, Some((ink_w * 0.7, ink)), &map, scale);
        let (ha, hb) = hem_of(joints);
        shape = shape.line(map(ha), map(hb), ink_w * 0.6, LineCap::Round, ink);
        let hand_xy = [joints.hand.x, joints.hand.y];
        shape = draw_dumbbell(
            shape,
            hand_xy,
            arm_pose,
            joints.hand.z,
            ink_w,
            ink,
            plate,
            plate_light,
            bar,
            &map,
            scale,
        );
        let fist_r = ((8.2 * nearness(joints.hand.z) * BUDDY_FIST * 0.62) as f32) * scale;
        let hp = map(hand_xy);
        let fist_pts = disc_points(hp[0], hp[1], fist_r, fist_r * 0.9, 12);
        let _ = BUDDY_GEAR;
        shape = shape.fill(fist_pts.clone(), skin_shade);
        shape.stroke_with(fist_pts, true, ink_w * 0.75, LineCap::Round, ink)
    };

    if left_joints.hand.z < -4.0 {
        s = draw_arm(s, &spec.snap.pose.left, &left_joints);
    }
    if right_joints.hand.z < -4.0 {
        s = draw_arm(s, &spec.snap.pose.right, &right_joints);
    }

    let sp: Vec<[f32; 2]> = shorts_poly(dip).into_iter().map(&map).collect();
    s = s.fill(sp.clone(), shorts);
    for &[sx, sy, sr, sturn] in &SHORTS_STARS {
        let y = body_y(sy, dip);
        if y < win.top + win.side {
            let star: Vec<[f32; 2]> = star_points(sx, y, sr, sturn).into_iter().map(&map).collect();
            s = s.fill(star, star_col);
        }
    }
    s = s.stroke_with(sp, true, ink_w * 0.85, LineCap::Round, ink);

    let tp: Vec<[f32; 2]> = torso_poly(dip).into_iter().map(&map).collect();
    s = s.fill(tp.clone(), shirt);
    s = s.stroke_with(tp, true, ink_w * 0.85, LineCap::Round, ink);
    for side in [-1i8, 1i8] {
        let shp: Vec<[f32; 2]> = shoulder_of(side, dip).into_iter().map(&map).collect();
        s = s.fill(shp, shirt_shade);
    }
    let mark_xy = map([BUDDY_SHIRT_MARK.0, body_y(BUDDY_SHIRT_MARK.1, dip)]);
    let mark_r = (BUDDY_SHIRT_MARK.2 as f32) * scale * 0.82;
    let mark_col = color_from_css(BUDDY_COLOR_MARK, 0.9);
    s = s.stroke_with(
        disc_points(mark_xy[0], mark_xy[1], mark_r, mark_r, 20),
        true,
        ink_w * 0.75,
        LineCap::Round,
        mark_col,
    );
    s = s.line(
        [mark_xy[0] - mark_r * 0.4, mark_xy[1] - mark_r * 0.25],
        [mark_xy[0] - mark_r * 0.05, mark_xy[1]],
        ink_w * 0.75,
        LineCap::Round,
        mark_col,
    );
    s = s.line(
        [mark_xy[0] - mark_r * 0.4, mark_xy[1] + mark_r * 0.25],
        [mark_xy[0] - mark_r * 0.05, mark_xy[1]],
        ink_w * 0.75,
        LineCap::Round,
        mark_col,
    );
    s = s.line(
        [mark_xy[0] + mark_r * 0.05, mark_xy[1] + mark_r * 0.25],
        [mark_xy[0] + mark_r * 0.42, mark_xy[1] + mark_r * 0.25],
        ink_w * 0.75,
        LineCap::Round,
        mark_col,
    );

    if left_joints.hand.z >= -4.0 {
        s = draw_arm(s, &spec.snap.pose.left, &left_joints);
    }
    if right_joints.hand.z >= -4.0 {
        s = draw_arm(s, &spec.snap.pose.right, &right_joints);
    }

    let drop = head_drop(dip);
    let head_cx = BUDDY_RIG_MIDDLE + spec.snap.look * 3.8;
    let head_cy = 51.0 + drop;

    for side in [-1.0, 1.0] {
        let ep = map([head_cx + side * 24.5 - spec.snap.look * 1.4, head_cy + 4.0]);
        let ear_pts = disc_points(ep[0], ep[1], 5.4 * scale, 6.8 * scale, 14);
        s = s.fill(ear_pts.clone(), skin_shade);
        s = s.stroke_with(ear_pts, true, ink_w * 0.75, LineCap::Round, ink);
    }

    let neck_poly = [
        map([BUDDY_RIG_MIDDLE - 9.0, 72.0 + drop]),
        map([BUDDY_RIG_MIDDLE + 9.0, 72.0 + drop]),
        map([BUDDY_RIG_MIDDLE + 10.0, 82.0 + drop]),
        map([BUDDY_RIG_MIDDLE - 10.0, 82.0 + drop]),
    ];
    s = s.fill(neck_poly, skin_shade);

    let hp = map([head_cx, head_cy]);
    let head_rx = 23.5 * scale;
    let head_ry = 24.5 * scale;
    let head_pts = disc_points(hp[0], hp[1], head_rx, head_ry, 28);
    s = s.fill(head_pts.clone(), skin);
    s = s.stroke_with(head_pts, true, ink_w, LineCap::Round, ink);

    let hair_pts = vec![
        map([head_cx - 21.0, head_cy - 6.0]),
        map([head_cx - 14.0, head_cy - 18.0]),
        map([head_cx + 14.0, head_cy - 18.0]),
        map([head_cx + 21.0, head_cy - 6.0]),
        map([head_cx + 10.0, head_cy - 9.0]),
        map([head_cx, head_cy - 6.5]),
        map([head_cx - 10.0, head_cy - 9.5]),
    ];
    s = s.fill(hair_pts, hair);

    let crown_center = map([head_cx, head_cy - 11.5]);
    let crown_pts = disc_points(crown_center[0], crown_center[1], 24.5 * scale, 15.0 * scale, 22);
    s = s.fill(crown_pts.clone(), cap_blue);
    s = s.stroke_with(crown_pts, true, ink_w, LineCap::Round, ink);

    let brim_pts: Vec<[f32; 2]> = (-6..=6)
        .map(|i| {
            let t = (i as f64) / 6.0;
            map([
                head_cx + spec.snap.look * 3.0 + t * 23.0,
                head_cy - 11.0 + (1.0 - t * t) * 5.2,
            ])
        })
        .chain((-6..=6).rev().map(|i| {
            let t = (i as f64) / 6.0;
            map([
                head_cx + t * 22.0,
                head_cy - 15.5 + (1.0 - t * t) * 2.0,
            ])
        }))
        .collect();
    s = s.fill(brim_pts.clone(), cap_dark);
    s = s.stroke_with(brim_pts, true, ink_w * 0.85, LineCap::Round, ink);

    let gaze_offset = spec.snap.look * 4.5;
    if spec.snap.screen {
        let glass = spec.snap.glass.unwrap_or_else(|| {
            let cell = screen_face_for(CODEX_BUDDY.id)
                .and_then(|f| {
                    f.cells
                        .get(BUDDY_RIG_PLATE_ROW)
                        .and_then(|r| r.get(BUDDY_RIG_PLATE_COL))
                        .copied()
                })
                .unwrap_or([82, 44, 36, 22]);
            locust_core::buddy_moves::ScreenRectF {
                x: cell[0] as f64,
                y: cell[1] as f64,
                w: cell[2] as f64,
                h: cell[3] as f64,
            }
        });
        let tl = map([head_cx - glass.w * 0.52 + gaze_offset * 0.4, head_cy - 3.5]);
        let br = map([head_cx + glass.w * 0.52 + gaze_offset * 0.4, head_cy + 14.5]);
        let gw = (br[0] - tl[0]).max(4.0);
        let gh = (br[1] - tl[1]).max(3.0);
        let glass_poly = rounded_rect_points(tl[0], tl[1], gw, gh, gh * 0.38, 5);
        s = s.stroke_with(glass_poly.clone(), true, ink_w * 0.9, LineCap::Round, ink);
        let unit = glass_units(glass.w);
        let le = [head_cx + gaze_offset * 0.5 - glass.w * 0.22, head_cy + 5.5];
        let re = [head_cx + gaze_offset * 0.5 + glass.w * 0.22, head_cy + 5.5];
        s = draw_screen_panel(
            s,
            &glass_poly,
            spec.snap.tone,
            spec.snap.flash,
            spec.snap.flash_progress,
            spec.snap.power_on,
            &spec.snap.eyes,
            le,
            re,
            unit,
            1.0,
            1.0,
            &map,
            scale * 0.5,
            spec.snap.s_seconds,
            spec.snap.shut,
        );
    } else {
        let le = map([head_cx - 7.5 + gaze_offset, head_cy + 3.5]);
        let re = map([head_cx + 7.5 + gaze_offset, head_cy + 3.5]);
        let eye_r = (2.35 * scale).max(0.8);
        let open = (1.0 - spec.snap.shut).max(0.1) as f32;
        for ep in [le, re] {
            s = s.fill(disc_points(ep[0], ep[1], eye_r, eye_r * open, 12), ink);
        }
        let smile: Vec<[f32; 2]> = (-4..=4)
            .map(|i| {
                let t = (i as f64) / 4.0;
                map([
                    head_cx + gaze_offset * 0.8 + t * 5.8,
                    head_cy + 11.2 + (1.0 - t * t) * 2.4,
                ])
            })
            .collect();
        s = s.stroke_with(smile, false, ink_w * 0.85, LineCap::Round, ink);
    }

    if let Some(presence) = spec.presence {
        let dot_r = (draw_size * 0.135).max(2.2);
        let cx = draw_size - dot_r * 1.25;
        let cy = draw_size - dot_r * 1.25;
        s = s.fill(
            disc_points(cx, cy, dot_r * 1.32, dot_r * 1.32, 14),
            rgba8(14, 18, 26, 230),
        );
        s = s.fill(disc_points(cx, cy, dot_r, dot_r, 14), presence_color(presence, spec.light_theme));
    }

    s
}

/// A worn puppet's screen (`PetPuppet.tsx` paint, `PetSprite.tsx` paintGlass and paintScreenEyes): its glass laid on
/// its face where the drawing's own was, and a bot screen's eyes on it, moved with the drawing by the rig's pose.
pub struct PuppetDrawSpec<'a> {
    pub puppet: &'static locust_core::pet_puppets::PetPuppet,
    /// The rig's pose as the puppet wears it (`puppet_pose`).
    pub pose: BotAvatarPose,
    /// Box units to the face's points (`crate::look::puppets::face_matrix`).
    pub matrix: [f64; 6],
    pub size: f32,
    pub snap: &'a BotSnapshot,
}

/// Its glass and its eyes. Electron's glass is a gradient from `sameHue(tint, 0.42, 0.17 dark)` at its top to
/// `sameHue(tint, 0.5, 0.08 dark)` at its foot; a `Shape` fills one colour, so it is filled with the colour halfway.
pub fn draw_puppet_shape(spec: &PuppetDrawSpec<'_>) -> Shape {
    use locust_core::bot_spec::{screen_eye_layout, VisorBox, EYES_VISOR, SCREEN_PAD};
    use locust_core::pet_puppets::puppet_glass;
    let p = spec.puppet;
    let m = spec.matrix;
    let map = move |q: [f64; 2]| -> [f32; 2] {
        [(m[0] * q[0] + m[2] * q[1] + m[4]) as f32, (m[1] * q[0] + m[3] * q[1] + m[5]) as f32]
    };
    let px_per_unit = (m[0].hypot(m[1]) * m[2].hypot(m[3])).sqrt();
    let glass = puppet_glass(p);
    let corner = glass.w.min(glass.h) * p.corner;
    let outline = |x: f64, y: f64, w: f64, h: f64, r: f64| -> Vec<[f32; 2]> {
        rounded_rect_points(x as f32, y as f32, w as f32, h as f32, r as f32, 5)
            .into_iter()
            .map(|[a, b]| map([f64::from(a), f64::from(b)]))
            .collect()
    };
    let mut s = Shape::new().view_box(spec.size, spec.size);
    if let Some(ink) = p.ink {
        // Its ink round the glass, as wide as a pet's own lines (GLASS_INK, 2 of the drawing's pixels).
        let w = 2.0 * 100.0 / p.window[2];
        s = s.fill(outline(glass.x - w, glass.y - w, glass.w + 2.0 * w, glass.h + 2.0 * w, corner + w), color_from_css(ink, 1.0));
    }
    let dark = p.dark.unwrap_or(1.0);
    let [r, g, b, _] = same_hue_rgba(p.tint, 0.46, 0.125 * dark, 1.0);
    let poly = outline(glass.x, glass.y, glass.w, glass.h, corner);
    s = s.fill(poly.clone(), rgb8(r, g, b));
    // The sheen over its top half, and its rim of light.
    s = s.fill(outline(glass.x, glass.y, glass.w, glass.h / 2.0, corner.min(glass.h / 4.0)), rgba8(255, 255, 255, 17));
    let unit = 1.0 / glass_units(glass.w);
    s = s.stroke_with(poly.clone(), true, (1.6 * unit * px_per_unit) as f32, LineCap::Round, rgba8(255, 255, 255, 41));

    // The eyes: placed and sized as a bot screen's are, with the glass as its visor (paintScreenEyes).
    let snap = spec.snap;
    let pair: [String; 2] = snap.eyes.clone().unwrap_or_else(|| SCREEN_RESTING_EYES.map(str::to_owned));
    let u = glass_units(glass.w);
    let box_ = VisorBox { half_width: EYES_VISOR.half_width, half_height: (glass.h / 2.0) * u, corner: corner * u, y: 0.0 };
    let laid = screen_eye_layout(box_, [&pair[0], &pair[1]], [0.0, snap.lag_y / 0.5], p.eye_y.unwrap_or(0.0) * box_.half_height);
    let larger = p.eye_scale.unwrap_or(1.0).max(1.0);
    let reach = |g: &str| glyph_shape(g).or_else(|| glyph_shape("\u{2022}")).map(|sh| locust_core::bot_spec::glyph_reach(&sh));
    let (reach_x, reach_y) = [&pair[0], &pair[1]]
        .iter()
        .filter_map(|g| reach(g))
        .fold((0.0f64, 0.0f64), |(x, y), r| (x.max(r[0]), y.max(r[1])));
    let room_x = box_.half_width - SCREEN_PAD;
    let room_y = box_.half_height - SCREEN_PAD;
    let scale = (laid.scale * larger).min(room_y / reach_y).min((room_x - 0.5) / (2.0 * reach_x));
    let centre = |[x, y]: [f64; 2]| -> [f64; 2] {
        let x = x.signum() * (x.abs() * larger).min(room_x - reach_x * scale);
        [glass.x + glass.w / 2.0 + x / u, glass.y + glass.h / 2.0 + y / u]
    };
    let open = 1.0 - 0.92 * spec.pose.blink_l.max(spec.pose.blink_r);
    draw_screen_light(
        s,
        &poly,
        snap.tone,
        snap.flash,
        snap.flash_progress,
        snap.power_on,
        &pair,
        centre(laid.centres[0]),
        centre(laid.centres[1]),
        scale / u,
        open,
        open,
        &map,
        px_per_unit as f32,
        snap.s_seconds,
        snap.shut,
    )
}

/// A puppet's marks, on what is drawn of it (`puppetBody` through `anchorsOf`, as its host's CSS variables): the amber
/// waiting ring round its paint and the presence dot at its foot. They stay in the face's box as the drawing moves.
pub fn puppet_marks_shape(size: f32, body: BodyBox, ring: bool, presence: Option<PresenceDot>, light_theme: bool) -> Shape {
    let anchors = anchors_of(body);
    let mut s = Shape::new().view_box(size, size);
    if ring {
        let r = (anchors.ring.side as f32) * size * 0.5;
        let c = [((anchors.ring.left as f32) * size) + r, ((anchors.ring.top as f32) * size) + r];
        s = s.stroke_with(disc_points(c[0], c[1], r, r, 32), true, (size * 0.045).max(0.9), LineCap::Round, rgba8(245, 166, 35, 220));
    }
    if let Some(presence) = presence {
        let dot_r = (size * 0.1).max(2.2);
        let (cx, cy) = ((anchors.dot.x as f32) * size, (anchors.dot.y as f32) * size);
        s = s.fill(disc_points(cx, cy, dot_r * 1.32, dot_r * 1.32, 14), rgba8(14, 18, 26, 230));
        s = s.fill(disc_points(cx, cy, dot_r, dot_r, 14), presence_color(presence, light_theme));
    }
    s
}

/// Static shape helper used when rendering outside a `State` frame or in vector bounds tests.
pub fn static_bot_shape(
    shape: &str,
    face: BotFace,
    hue: &str,
    size: f32,
    screen: bool,
    plush: bool,
    light_theme: bool,
) -> Shape {
    let snap = BotSnapshot {
        pose: locust_core::bot_rig::rest_pose(locust_core::bot_rig::BotAvatarState::Default),
        eyes: if screen {
            Some(["|".to_owned(), "|".to_owned()])
        } else {
            None
        },
        tone: Phosphor::Cyan,
        flash: None,
        flash_progress: 1.0,
        power_on: None,
        shut: 0.0,
        lag_x: 0.0,
        lag_y: 0.0,
        hop: false,
        blinking: false,
        screen,
        plush,
        s_seconds: 0.0,
        hovered: false,
    };
    draw_bot_shape(&BotDrawSpec {
        shape,
        face,
        hue,
        size,
        light_theme,
        presence: None,
        ring: false,
        snap: &snap,
    })
}

/// Static shape helper for Codex Buddy outside a `State` frame or in tests.
pub fn static_buddy_shape(
    hue: &str,
    size: f32,
    screen: bool,
    plush: bool,
    light_theme: bool,
) -> Shape {
    let snap = BuddySnapshot {
        pose: locust_core::buddy_rig::REST_POSE,
        look: 0.0,
        eyes: ["|".to_owned(), "|".to_owned()],
        tone: Phosphor::Cyan,
        flash: None,
        flash_progress: 1.0,
        power_on: None,
        shut: 0.0,
        glass: None,
        screen,
        plush,
        s_seconds: 0.0,
        hovered: false,
    };
    draw_buddy_shape(&BuddyDrawSpec {
        hue,
        size,
        light_theme,
        presence: None,
        ring: false,
        snap: &snap,
    })
}

#[cfg(test)]
mod presence_tests {
    use super::*;

    // PixelFace.tsx `PRESENCE_TONE`: working 'live' (monochrome since 0.430), approval 'amber', blocked 'red', idle none. The only
    // part of PixelFace.tsx Electron still uses: TeammateBot imports it and nothing draws the PixelFace chip itself.
    #[test]
    fn the_presence_dot_is_amber_for_approval_red_for_blocked_and_the_text_colour_for_work_in_both_themes() {
        for dark in [true, false] {
            let look = crate::look::controls::Look::new(dark);
            assert_eq!(presence_color(PresenceDot::Waiting, !dark), look.c("amber"), "dark {dark}");
            assert_eq!(presence_color(PresenceDot::Blocked, !dark), look.c("red"), "dark {dark}");
            assert_eq!(presence_color(PresenceDot::Working, !dark), look.c("text-primary"), "dark {dark}");
        }
        assert_ne!(presence_color(PresenceDot::Waiting, false), presence_color(PresenceDot::Blocked, false));
    }
}

#[cfg(test)]
#[path = "glyph_eyes_tests.rs"]
mod glyph_eyes_tests;

#[cfg(test)]
#[path = "screen_switches_tests.rs"]
mod screen_switches_tests;

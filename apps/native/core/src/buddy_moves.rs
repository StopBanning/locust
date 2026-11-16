//! Codex Buddy's screen measurements, moves, sets, moments, and screen conductor.
//! Pure math & data ported from `petScreens.ts`, `petRoutines.ts`, and `PetSprite.tsx`.

use std::collections::HashMap;
use std::f64::consts::PI;

use crate::bot_rig::{
    eye_glyphs_for, glyph_motion, power_on_eyes, screen_light, seeded, BotAvatarState,
    BotMood, BotMotionLevel, CrtKind, FaceActivity, FaceChanges, Glance, GlyphMotion, IdleMoment,
    ShownFace, BLINK_GAP_S, CRT_FLICK_S, LOOK_ABOUT_S, POWER_ON_S, SCREEN_RESTING_EYES,
    THINKING_CYCLE_S,
};
use crate::bot_spec::{Phosphor, EYES_VISOR};
use crate::buddy_rig::{
    head_drop, BuddyRigSim, ArmPose, RigTarget, PET_COLUMNS, REST_ARM, REST_TARGET,
};

// =========================================================================
// Screen measurement table (`petScreens.ts`)
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScreenRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenRectF {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl From<ScreenRect> for ScreenRectF {
    fn from(r: ScreenRect) -> Self {
        Self {
            x: r.x as f64,
            y: r.y as f64,
            w: r.w as f64,
            h: r.h as f64,
        }
    }
}

pub struct PetScreenFace {
    pub id: &'static str,
    pub frame_width: u32,
    pub frame_height: u32,
    pub glass: &'static str,
    pub ink: &'static str,
    pub counts: &'static [usize],
    pub cells: &'static [&'static [[u32; 4]]],
    pub edges: &'static [&'static [[u32; 2]]],
}

pub const CODEX_BUDDY_COUNTS: [usize; 9] = [6, 8, 8, 4, 5, 8, 6, 6, 6];

static ROW0_CELLS: [[u32; 4]; 6] = [
    [71, 44, 37, 27],
    [72, 44, 37, 27],
    [72, 44, 37, 27],
    [72, 44, 37, 27],
    [73, 44, 37, 27],
    [72, 44, 37, 27],
];
static ROW1_CELLS: [[u32; 4]; 8] = [
    [98, 52, 37, 30],
    [98, 52, 37, 30],
    [101, 54, 37, 30],
    [92, 50, 37, 30],
    [98, 52, 37, 30],
    [100, 54, 37, 30],
    [91, 51, 37, 30],
    [98, 52, 37, 30],
];
static ROW2_CELLS: [[u32; 4]; 8] = [
    [58, 52, 37, 30],
    [56, 54, 37, 30],
    [52, 55, 37, 30],
    [56, 52, 37, 30],
    [56, 53, 37, 30],
    [56, 54, 37, 30],
    [56, 52, 37, 30],
    [51, 54, 37, 30],
];
static ROW3_CELLS: [[u32; 4]; 4] = [
    [76, 44, 37, 27],
    [76, 44, 37, 27],
    [76, 44, 37, 27],
    [69, 44, 37, 27],
];
static ROW4_CELLS: [[u32; 4]; 5] = [
    [74, 70, 32, 23],
    [72, 44, 37, 27],
    [73, 44, 37, 27],
    [72, 44, 37, 27],
    [74, 70, 31, 23],
];
static ROW5_CELLS: [[u32; 4]; 8] = [
    [77, 46, 39, 31],
    [80, 48, 39, 31],
    [86, 46, 39, 31],
    [85, 46, 39, 31],
    [80, 46, 39, 31],
    [83, 47, 39, 31],
    [86, 47, 39, 31],
    [80, 48, 39, 31],
];
static ROW6_CELLS: [[u32; 4]; 6] = [
    [68, 47, 37, 27],
    [70, 48, 37, 27],
    [64, 55, 44, 32],
    [76, 46, 37, 27],
    [75, 48, 37, 27],
    [75, 47, 37, 27],
];
static ROW7_CELLS: [[u32; 4]; 6] = [
    [72, 53, 37, 27],
    [75, 50, 37, 27],
    [74, 50, 37, 27],
    [78, 69, 31, 25],
    [70, 52, 37, 27],
    [72, 53, 37, 27],
];
static ROW8_CELLS: [[u32; 4]; 6] = [
    [69, 48, 37, 27],
    [79, 49, 37, 27],
    [76, 48, 37, 27],
    [80, 46, 37, 27],
    [77, 49, 37, 27],
    [70, 48, 37, 27],
];

static CODEX_CELLS: [&[[u32; 4]]; 9] = [
    &ROW0_CELLS,
    &ROW1_CELLS,
    &ROW2_CELLS,
    &ROW3_CELLS,
    &ROW4_CELLS,
    &ROW5_CELLS,
    &ROW6_CELLS,
    &ROW7_CELLS,
    &ROW8_CELLS,
];

static ROW0_EDGES: [[u32; 2]; 6] = [[33, 158], [33, 159], [34, 158], [34, 157], [35, 157], [34, 158]];
static ROW1_EDGES: [[u32; 2]; 8] = [
    [33, 159],
    [36, 155],
    [10, 182],
    [40, 151],
    [36, 156],
    [17, 175],
    [42, 150],
    [34, 157],
];
static ROW2_EDGES: [[u32; 2]; 8] = [
    [39, 152],
    [34, 157],
    [10, 181],
    [42, 149],
    [31, 160],
    [38, 153],
    [40, 151],
    [41, 151],
];
static ROW3_EDGES: [[u32; 2]; 4] = [[32, 160], [33, 158], [33, 159], [40, 152]];
static ROW4_EDGES: [[u32; 2]; 5] = [[42, 149], [18, 173], [19, 172], [19, 173], [42, 149]];
static ROW5_EDGES: [[u32; 2]; 8] = [
    [13, 178],
    [6, 186],
    [9, 182],
    [11, 181],
    [13, 179],
    [12, 179],
    [10, 181],
    [13, 179],
];
static ROW6_EDGES: [[u32; 2]; 6] = [[33, 159], [36, 155], [17, 174], [28, 164], [28, 164], [29, 163]];
static ROW7_EDGES: [[u32; 2]; 6] = [[35, 156], [21, 170], [24, 168], [43, 148], [21, 170], [34, 158]];
static ROW8_EDGES: [[u32; 2]; 6] = [[34, 158], [30, 161], [31, 161], [30, 162], [29, 162], [32, 159]];

static CODEX_EDGES: [&[[u32; 2]]; 9] = [
    &ROW0_EDGES,
    &ROW1_EDGES,
    &ROW2_EDGES,
    &ROW3_EDGES,
    &ROW4_EDGES,
    &ROW5_EDGES,
    &ROW6_EDGES,
    &ROW7_EDGES,
    &ROW8_EDGES,
];

/// A `static`, not a `const`: callers and tests hold `&CODEX_BUDDY` and compare it by address, and only a static has
/// one address (a const is copied at each use; release builds happened to merge the copies, debug builds do not).
pub static CODEX_BUDDY: PetScreenFace = PetScreenFace {
    id: "codex-buddy",
    frame_width: 192,
    frame_height: 208,
    glass: "#126dc7",
    ink: "#0c080d",
    counts: &CODEX_BUDDY_COUNTS,
    cells: &CODEX_CELLS,
    edges: &CODEX_EDGES,
};

pub fn screen_face_for(id: &str) -> Option<&'static PetScreenFace> {
    if id == CODEX_BUDDY.id {
        Some(&CODEX_BUDDY)
    } else {
        None
    }
}

pub const EDGE_SLACK: u32 = 2;

pub fn painted_edges(
    data: &[u8],
    width: usize,
    left: usize,
    top: usize,
    frame_width: usize,
    frame_height: usize,
) -> Option<[u32; 2]> {
    let painted = |x: usize| -> bool {
        for y in top..(top + frame_height) {
            let idx = (y * width + left + x) * 4 + 3;
            if data.get(idx).copied().unwrap_or(0) > 128 {
                return true;
            }
        }
        false
    };
    let mut first = 0usize;
    while first < frame_width && !painted(first) {
        first += 1;
    }
    if first == frame_width {
        return None;
    }
    let mut last = frame_width - 1;
    while last > first && !painted(last) {
        last -= 1;
    }
    Some([first as u32, (last + 1) as u32])
}

pub fn painted_box(
    data: &[u8],
    width: usize,
    left: usize,
    top: usize,
    frame_width: usize,
    frame_height: usize,
) -> Option<[u32; 4]> {
    let sides = painted_edges(data, width, left, top, frame_width, frame_height)?;
    let painted = |y: usize| -> bool {
        for x in (sides[0] as usize)..(sides[1] as usize) {
            let idx = ((top + y) * width + left + x) * 4 + 3;
            if data.get(idx).copied().unwrap_or(0) > 128 {
                return true;
            }
        }
        false
    };
    let mut first = 0usize;
    while first < frame_height && !painted(first) {
        first += 1;
    }
    let mut last = frame_height - 1;
    while last > first && !painted(last) {
        last -= 1;
    }
    Some([sides[0], first as u32, sides[1], (last + 1) as u32])
}

pub fn screen_fits(face: &PetScreenFace, data: &[u8], width: usize, height: usize) -> bool {
    if width != (face.frame_width as usize) * PET_COLUMNS
        || height != (face.frame_height as usize) * 9
    {
        return false;
    }
    for (row, &count) in face.counts.iter().enumerate() {
        for column in 0..PET_COLUMNS {
            let want = if column < count {
                face.edges.get(row).and_then(|r| r.get(column)).copied()
            } else {
                None
            };
            let left = column * (face.frame_width as usize);
            let top = row * (face.frame_height as usize);
            let found = painted_edges(
                data,
                width,
                left,
                top,
                face.frame_width as usize,
                face.frame_height as usize,
            );
            match (want, found) {
                (None, None) => {}
                (Some(w), Some(f)) => {
                    if f[0].abs_diff(w[0]) > EDGE_SLACK || f[1].abs_diff(w[1]) > EDGE_SLACK {
                        return false;
                    }
                }
                _ => return false,
            }
        }
    }
    true
}

pub fn screen_at(face: &PetScreenFace, row: usize, column: usize) -> Option<ScreenRect> {
    let cell = face.cells.get(row)?.get(column)?;
    Some(ScreenRect {
        x: cell[0],
        y: cell[1],
        w: cell[2],
        h: cell[3],
    })
}

// =========================================================================
// Routines & Poses (`petRoutines.ts`)
// =========================================================================

pub fn ease(t: f64) -> f64 {
    let k = t.clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

pub fn roll_at(from: f64, to: f64, k: f64) -> f64 {
    let t = k.clamp(0.0, 1.0);
    let share = if to.abs() >= from.abs() {
        t * t
    } else {
        1.0 - (1.0 - t) * (1.0 - t)
    };
    from + (to - from) * share
}

fn mix_arm(a: ArmPose, b: ArmPose, k: f64) -> ArmPose {
    ArmPose {
        abduct: a.abduct + (b.abduct - a.abduct) * k,
        flex: a.flex + (b.flex - a.flex) * k,
        bend: a.bend + (b.bend - a.bend) * k,
        bend_up: a.bend_up + (b.bend_up - a.bend_up) * k,
        grip: roll_at(a.grip, b.grip, k),
    }
}

fn joint_gap(a: &ArmPose, b: &ArmPose) -> f64 {
    let d0 = a.abduct - b.abduct;
    let d1 = a.flex - b.flex;
    let d2 = a.bend - b.bend;
    let d3 = a.bend_up - b.bend_up;
    (d0 * d0 + d1 * d1 + d2 * d2 + d3 * d3).sqrt()
}

fn curve_at(points: &[ArmPose], t: f64) -> ArmPose {
    let n = points.len();
    let first = points[0];
    if n <= 2 {
        return mix_arm(first, *points.last().unwrap_or(&first), t);
    }
    let spans = (n - 1) as f64;
    let u = (t.max(0.0) * spans).min(spans - 1e-9);
    let i = u.floor() as usize;
    let k = u - (i as f64);
    let p1 = points[i];
    let p2 = points[i + 1];
    let p0 = if i > 0 {
        points[i - 1]
    } else {
        mix_arm(p2, p1, 2.0)
    };
    let p3 = points.get(i + 2).copied().unwrap_or_else(|| mix_arm(p1, p2, 2.0));
    let k2 = k * k;
    let k3 = k2 * k;
    let at = |a: f64, b: f64, c: f64, d: f64| -> f64 {
        0.5 * (2.0 * b
            + (c - a) * k
            + (2.0 * a - 5.0 * b + 4.0 * c - d) * k2
            + (3.0 * b - a - 3.0 * c + d) * k3)
    };
    ArmPose {
        abduct: at(p0.abduct, p1.abduct, p2.abduct, p3.abduct),
        flex: at(p0.flex, p1.flex, p2.flex, p3.flex),
        bend: at(p0.bend, p1.bend, p2.bend, p3.bend),
        bend_up: at(p0.bend_up, p1.bend_up, p2.bend_up, p3.bend_up),
        grip: at(p0.grip, p1.grip, p2.grip, p3.grip),
    }
}

const WAY_SAMPLES: usize = 24;

fn smooth_way(points: &[ArmPose], k: f64) -> ArmPose {
    if k <= 0.0 {
        return points[0];
    }
    if k >= 1.0 {
        return *points.last().unwrap_or(&points[0]);
    }
    let mut along = Vec::with_capacity(WAY_SAMPLES + 1);
    along.push(0.0);
    let mut last = curve_at(points, 0.0);
    for i in 1..=WAY_SAMPLES {
        let next = curve_at(points, (i as f64) / (WAY_SAMPLES as f64));
        along.push(along[i - 1] + joint_gap(&last, &next));
        last = next;
    }
    let total = along[WAY_SAMPLES];
    if total <= 1e-9 {
        return points[0];
    }
    let want = k * total;
    let mut i = 1usize;
    while i < WAY_SAMPLES && along[i] < want {
        i += 1;
    }
    let a = along[i - 1];
    let b = along[i];
    let frac = if b > a { (want - a) / (b - a) } else { 0.0 };
    curve_at(points, ((i - 1) as f64 + frac) / (WAY_SAMPLES as f64))
}

pub fn mix_target(a: RigTarget, b: RigTarget, k: f64) -> RigTarget {
    if k <= 0.0 {
        return a;
    }
    if k >= 1.0 {
        return b;
    }
    RigTarget {
        pose: crate::buddy_rig::BuddyPose {
            left: mix_arm(a.pose.left, b.pose.left, k),
            right: mix_arm(a.pose.right, b.pose.right, k),
            dip: a.pose.dip + (b.pose.dip - a.pose.dip) * k,
        },
        look: a.look + (b.look - a.look) * k,
        tilt: None,
    }
}

pub const POSES_REST: ArmPose = REST_ARM;
pub const POSES_CURL: ArmPose = ArmPose {
    abduct: 0.03,
    flex: 0.18,
    bend: 2.55,
    bend_up: 0.85,
    grip: 0.0,
};
pub const POSES_HAMMER: ArmPose = ArmPose {
    abduct: 0.03,
    flex: 0.18,
    bend: 2.55,
    bend_up: 0.85,
    grip: PI / 2.0,
};
pub const POSES_HAMMER_HANG: ArmPose = ArmPose {
    abduct: REST_ARM.abduct,
    flex: REST_ARM.flex,
    bend: REST_ARM.bend,
    bend_up: REST_ARM.bend_up,
    grip: PI / 2.0,
};
pub const POSES_CURL_LOW: ArmPose = ArmPose {
    abduct: -0.04,
    flex: 0.2,
    bend: 1.82,
    bend_up: 0.48,
    grip: 0.0,
};
pub const POSES_HAMMER_LOW: ArmPose = ArmPose {
    abduct: -0.04,
    flex: 0.2,
    bend: 1.82,
    bend_up: 0.48,
    grip: PI / 2.0,
};
pub const POSES_RACK: ArmPose = ArmPose {
    abduct: 1.45,
    flex: 0.5,
    bend: 2.6,
    bend_up: 0.9,
    grip: PI / 2.0,
};
pub const POSES_PRESS: ArmPose = ArmPose {
    abduct: 2.15,
    flex: 0.3,
    bend: 1.75,
    bend_up: 1.0,
    grip: PI / 2.0,
};
pub const POSES_LOCKOUT: ArmPose = ArmPose {
    abduct: 2.4,
    flex: 0.15,
    bend: 1.1,
    bend_up: 1.0,
    grip: PI / 2.0,
};
pub const POSES_FLEX: ArmPose = ArmPose {
    abduct: 1.42,
    flex: 0.1,
    bend: 2.5,
    bend_up: 1.0,
    grip: PI / 2.0,
};
pub const POSES_FLEX_SQUEEZE: ArmPose = ArmPose {
    abduct: 1.48,
    flex: 0.1,
    bend: 2.62,
    bend_up: 1.0,
    grip: PI / 2.0,
};
pub const POSES_WAVE_IN: ArmPose = ArmPose {
    abduct: 1.95,
    flex: 0.1,
    bend: 2.4,
    bend_up: 1.0,
    grip: PI / 2.0,
};
pub const POSES_WAVE_OUT: ArmPose = ArmPose {
    abduct: 1.95,
    flex: 0.1,
    bend: 1.95,
    bend_up: 1.0,
    grip: PI / 2.0,
};
pub const POSES_THINK: ArmPose = ArmPose {
    abduct: 0.3,
    flex: 0.6,
    bend: 2.3,
    bend_up: 0.25,
    grip: PI / 2.0,
};
pub const POSES_TUCK: ArmPose = ArmPose {
    abduct: -0.08,
    flex: 0.65,
    bend: 2.4,
    bend_up: 0.7,
    grip: 0.0,
};
pub const POSES_TUCK_UPRIGHT: ArmPose = ArmPose {
    abduct: -0.08,
    flex: 0.65,
    bend: 2.3,
    bend_up: 0.6,
    grip: PI / 2.0,
};
pub const POSES_HALF_CURL: ArmPose = ArmPose {
    abduct: 0.05,
    flex: 0.2,
    bend: 1.4,
    bend_up: 0.0,
    grip: 0.0,
};
pub const POSES_SAG: ArmPose = ArmPose {
    abduct: 0.04,
    flex: 0.02,
    bend: 0.04,
    bend_up: 0.0,
    grip: 0.0,
};

pub const REST: RigTarget = REST_TARGET;

pub const LADDER: [ArmPose; 5] = [
    POSES_REST,
    POSES_TUCK,
    POSES_RACK,
    POSES_PRESS,
    POSES_LOCKOUT,
];

pub const HANDOFF_MS_BASE: f64 = 280.0;
pub const HANDOFF_MS_PER_JOINT: f64 = 210.0;
pub const HANDOFF_MS_MOST: f64 = 1150.0;

fn nearest_rung(arm: &ArmPose) -> usize {
    let mut best = 0usize;
    let mut best_d = joint_gap(arm, &LADDER[0]);
    for (i, rung) in LADDER.iter().enumerate().skip(1) {
        let d = joint_gap(arm, rung);
        if d < best_d {
            best = i;
            best_d = d;
        }
    }
    best
}

fn arm_way(from: ArmPose, to: ArmPose) -> Vec<ArmPose> {
    let a = nearest_rung(&from);
    let b = nearest_rung(&to);
    if a == b {
        return vec![from, to];
    }
    let mut way = vec![from];
    if b > a {
        for i in a..=b {
            way.push(LADDER[i]);
        }
    } else {
        for i in (b..=a).rev() {
            way.push(LADDER[i]);
        }
    }
    way.push(to);
    way
}

fn along_way(way: &[ArmPose], k: f64) -> ArmPose {
    let first = way[0];
    let last = *way.last().unwrap_or(&first);
    let chest = way.iter().position(|p| *p == POSES_TUCK);
    let mut share = ease(k);
    if let Some(c) = chest {
        if c > 0 && c < way.len() - 1 {
            let lengths: Vec<f64> = way.windows(2).map(|w| joint_gap(&w[0], &w[1])).collect();
            let total: f64 = lengths.iter().sum();
            let total = if total == 0.0 { 1.0 } else { total };
            let at: f64 = lengths[..c].iter().sum::<f64>() / total;
            share = ease((k - (at - 0.18)) / 0.36);
        }
    }
    let mut posed = smooth_way(way, k);
    posed.grip = first.grip + (last.grip - first.grip) * share;
    posed
}

fn way_length(way: &[ArmPose]) -> f64 {
    way.windows(2).map(|w| joint_gap(&w[0], &w[1])).sum()
}

pub fn handoff_ms(from: &RigTarget, start: &RigTarget) -> f64 {
    let far = way_length(&arm_way(from.pose.left, start.pose.left))
        .max(way_length(&arm_way(from.pose.right, start.pose.right)));
    let moved =
        far + (from.pose.dip - start.pose.dip).abs() / 10.0 + (from.look - start.look).abs() / 4.0;
    if moved < 0.02 {
        return 0.0;
    }
    HANDOFF_MS_MOST.min(HANDOFF_MS_BASE + HANDOFF_MS_PER_JOINT * far)
}

pub fn handoff(from: RigTarget, start: RigTarget, k: f64) -> RigTarget {
    if k <= 0.0 {
        return from;
    }
    if k >= 1.0 {
        return start;
    }
    let t = ease(k);
    RigTarget {
        pose: crate::buddy_rig::BuddyPose {
            left: along_way(&arm_way(from.pose.left, start.pose.left), t),
            right: along_way(&arm_way(from.pose.right, start.pose.right), t),
            dip: from.pose.dip + (start.pose.dip - from.pose.dip) * t,
        },
        look: from.look + (start.look - from.look) * t,
        tilt: None,
    }
}

const fn both(arm: ArmPose, dip: f64, look: f64) -> RigTarget {
    RigTarget::new(
        crate::buddy_rig::BuddyPose {
            left: arm,
            right: arm,
            dip,
        },
        look,
    )
}

const fn each(left: ArmPose, right: ArmPose, dip: f64, look: f64) -> RigTarget {
    RigTarget::new(
        crate::buddy_rig::BuddyPose {
            left,
            right,
            dip,
        },
        look,
    )
}

const fn with_look(mut target: RigTarget, look: f64) -> RigTarget {
    target.look = look;
    target
}

#[derive(Debug, Clone, Copy)]
enum PieceKind {
    Hold(RigTarget),
    Glide(RigTarget, RigTarget),
    Through(RigTarget, RigTarget, RigTarget),
    Breath,
    SwingWave,
    Strain,
}

#[derive(Debug, Clone, Copy)]
struct MovePiece {
    ms: f64,
    kind: PieceKind,
}

impl MovePiece {
    fn at(&self, k: f64) -> RigTarget {
        match self.kind {
            PieceKind::Hold(t) => t,
            PieceKind::Glide(from, to) => mix_target(from, to, ease(k)),
            PieceKind::Through(from, via, to) => {
                if k <= 0.0 {
                    return from;
                }
                if k >= 1.0 {
                    return to;
                }
                let t = ease(k);
                let near = |a: f64, b: f64, c: f64| {
                    (1.0 - t) * (1.0 - t) * a + 2.0 * (1.0 - t) * t * b + t * t * c
                };
                let roll = |g0: f64, g1: f64, g2: f64| -> f64 {
                    if t < 0.5 {
                        roll_at(g0, g1, t * 2.0)
                    } else {
                        roll_at(g1, g2, (t - 0.5) * 2.0)
                    }
                };
                let mut left = smooth_way(&[from.pose.left, via.pose.left, to.pose.left], t);
                left.grip = roll(from.pose.left.grip, via.pose.left.grip, to.pose.left.grip);
                let mut right = smooth_way(&[from.pose.right, via.pose.right, to.pose.right], t);
                right.grip = roll(
                    from.pose.right.grip,
                    via.pose.right.grip,
                    to.pose.right.grip,
                );
                RigTarget::new(
                    crate::buddy_rig::BuddyPose {
                        left,
                        right,
                        dip: near(from.pose.dip, via.pose.dip, to.pose.dip),
                    },
                    near(from.look, via.look, to.look),
                )
            }
            PieceKind::Breath => both(REST_ARM, 0.7 * (PI * k).sin(), 0.0),
            PieceKind::SwingWave => {
                let win = each(REST_ARM, POSES_WAVE_IN, 0.0, 0.0);
                let wout = each(REST_ARM, POSES_WAVE_OUT, 0.0, 0.0);
                mix_target(win, wout, 0.5 - 0.5 * (k * 3.0 * 2.0 * PI).cos())
            }
            PieceKind::Strain => {
                let shake = 0.035 * (k * PI * 2.0 * 6.0).sin() * (PI * k).sin();
                let rack = both(POSES_RACK, 2.5, 0.0);
                let part = mix_target(rack, both(POSES_PRESS, 2.5, 0.0), 0.4);
                let mut target = mix_target(rack, part, ease((k * 1.6).min(1.0)));
                target.pose.left.abduct += shake;
                target.pose.right.abduct += shake;
                target
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct Move {
    pub name: &'static str,
    pub ms: f64,
    pub loops: bool,
    pub holds: bool,
    pub sets: &'static [f64],
    pub lifts: &'static [&'static str],
    pub still: RigTarget,
    pieces: Vec<MovePiece>,
}

impl Move {
    pub fn at(&self, into: f64) -> RigTarget {
        if self.pieces.is_empty() {
            return REST;
        }
        let mut left = into.max(0.0);
        for piece in &self.pieces {
            if left < piece.ms {
                return piece.at(left / piece.ms);
            }
            left -= piece.ms;
        }
        self.pieces.last().map_or(REST, |p| p.at(1.0))
    }
}

fn hold_p(t: RigTarget, ms: f64) -> MovePiece {
    MovePiece {
        ms,
        kind: PieceKind::Hold(t),
    }
}
fn glide_p(a: RigTarget, b: RigTarget, ms: f64) -> MovePiece {
    MovePiece {
        ms,
        kind: PieceKind::Glide(a, b),
    }
}
fn through_p(a: RigTarget, v: RigTarget, b: RigTarget, ms: f64) -> MovePiece {
    MovePiece {
        ms,
        kind: PieceKind::Through(a, v, b),
    }
}

fn curls_pieces(reps: usize) -> Vec<MovePiece> {
    let low = both(POSES_CURL_LOW, 0.6, 0.0);
    let top = both(POSES_CURL, 1.2, 0.0);
    let mut out = vec![glide_p(REST, low, 420.0)];
    for _ in 0..reps {
        out.push(glide_p(low, top, 460.0));
        out.push(hold_p(top, 160.0));
        out.push(glide_p(top, low, 560.0));
        out.push(hold_p(low, 140.0));
    }
    out.push(glide_p(low, REST, 480.0));
    out
}

fn alternating_pieces(reps: usize) -> Vec<MovePiece> {
    let low = both(POSES_CURL_LOW, 0.6, 0.0);
    let left = each(POSES_CURL, POSES_CURL_LOW, 0.8, 0.0);
    let right = each(POSES_CURL_LOW, POSES_CURL, 0.8, 0.0);
    let mut out = vec![glide_p(REST, low, 420.0)];
    for _ in 0..reps {
        out.push(glide_p(low, left, 440.0));
        out.push(hold_p(left, 120.0));
        out.push(glide_p(left, low, 500.0));
        out.push(glide_p(low, right, 440.0));
        out.push(hold_p(right, 120.0));
        out.push(glide_p(right, low, 500.0));
    }
    out.push(glide_p(low, REST, 480.0));
    out
}

fn hammer_pieces(reps: usize) -> Vec<MovePiece> {
    let hang = both(POSES_HAMMER_LOW, 0.6, 0.0);
    let top = both(POSES_HAMMER, 1.0, 0.0);
    let mut out = vec![glide_p(REST, hang, 520.0)];
    for _ in 0..reps {
        out.push(glide_p(hang, top, 580.0));
        out.push(hold_p(top, 160.0));
        out.push(glide_p(top, hang, 700.0));
        out.push(hold_p(hang, 160.0));
    }
    out.push(glide_p(hang, REST, 520.0));
    out
}

fn press_pieces(reps: usize) -> Vec<MovePiece> {
    let rack = both(POSES_RACK, 2.0, 0.0);
    let mid = both(POSES_PRESS, 1.2, 0.0);
    let lockout = both(POSES_LOCKOUT, 0.5, 0.0);
    let tuck = both(POSES_TUCK_UPRIGHT, 1.0, 0.0);
    let mut out = vec![through_p(REST, tuck, rack, 780.0), hold_p(rack, 140.0)];
    for _ in 0..reps {
        out.push(through_p(rack, mid, lockout, 620.0));
        out.push(hold_p(lockout, 220.0));
        out.push(through_p(lockout, mid, rack, 700.0));
        out.push(hold_p(rack, 160.0));
    }
    out.push(through_p(rack, tuck, REST, 800.0));
    out
}

fn look_about_pieces() -> Vec<MovePiece> {
    let look_ms = LOOK_ABOUT_S * 1000.0;
    let l = with_look(REST, -0.8);
    let r = with_look(REST, 0.8);
    vec![
        glide_p(REST, l, look_ms * 0.2),
        hold_p(l, look_ms * 0.2),
        glide_p(l, r, look_ms * 0.25),
        hold_p(r, look_ms * 0.15),
        glide_p(r, REST, look_ms * 0.2),
    ]
}

fn build_move(
    name: &'static str,
    pieces: Vec<MovePiece>,
    loops: bool,
    holds: bool,
    sets: &'static [f64],
    lifts: &'static [&'static str],
    still: RigTarget,
) -> Move {
    let ms = pieces.iter().map(|p| p.ms).sum();
    Move {
        name,
        ms,
        loops,
        holds,
        sets,
        lifts,
        still,
        pieces,
    }
}

static WORKOUT_LIFTS: [&str; 4] = ["press", "curls", "alternating", "hammer"];
static WORKOUT_SETS: [f64; 4] = [0.0, 7720.0, 13480.0, 19520.0];

pub fn workout_move() -> Move {
    let breather = hold_p(REST, 900.0);
    let mut pieces = Vec::new();
    let lifts = [
        press_pieces(3),
        curls_pieces(3),
        alternating_pieces(2),
        hammer_pieces(3),
    ];
    for lift in lifts {
        pieces.extend(lift);
        pieces.push(breather);
    }
    build_move(
        "workout",
        pieces,
        true,
        false,
        &WORKOUT_SETS,
        &WORKOUT_LIFTS,
        both(POSES_RACK, 2.0, 0.0),
    )
}

pub fn workout_standing_move() -> Move {
    workout_move()
}

pub fn think_move() -> Move {
    let dots_ms = THINKING_CYCLE_S * 1000.0;
    let look_ms = LOOK_ABOUT_S * 1000.0;
    let busy_ms = dots_ms - look_ms;
    let chin = each(REST_ARM, POSES_THINK, 0.4, 0.0);
    let chin_way = each(REST_ARM, POSES_TUCK_UPRIGHT, 0.3, 0.0);
    let slow_curl = each(POSES_CURL, REST_ARM, 0.6, 0.0);
    let mut pieces = look_about_pieces();
    pieces.push(through_p(REST, chin_way, chin, busy_ms * 0.3));
    pieces.push(hold_p(chin, busy_ms * 0.4));
    pieces.push(through_p(chin, chin_way, REST, busy_ms * 0.3));
    pieces.extend(look_about_pieces());
    pieces.push(glide_p(REST, slow_curl, busy_ms * 0.4));
    pieces.push(hold_p(slow_curl, busy_ms * 0.2));
    pieces.push(glide_p(slow_curl, REST, busy_ms * 0.4));
    build_move("think", pieces, true, false, &[], &[], chin)
}

pub fn wait_move() -> Move {
    let fidget = each(REST_ARM, POSES_HALF_CURL, 0.3, 0.0);
    let look_left = with_look(REST, -0.8);
    let look_right = with_look(REST, 0.8);
    build_move(
        "wait",
        vec![
            MovePiece {
                ms: 1800.0,
                kind: PieceKind::Breath,
            },
            glide_p(REST, look_left, 300.0),
            hold_p(look_left, 700.0),
            glide_p(look_left, REST, 300.0),
            MovePiece {
                ms: 1400.0,
                kind: PieceKind::Breath,
            },
            glide_p(REST, fidget, 450.0),
            hold_p(fidget, 300.0),
            glide_p(fidget, REST, 550.0),
            glide_p(REST, look_right, 300.0),
            hold_p(look_right, 700.0),
            glide_p(look_right, REST, 300.0),
        ],
        true,
        false,
        &[],
        &[],
        REST,
    )
}

pub fn wave_move() -> Move {
    let win = each(REST_ARM, POSES_WAVE_IN, 0.0, 0.0);
    let wtuck = each(REST_ARM, POSES_TUCK_UPRIGHT, 0.0, 0.0);
    build_move(
        "wave",
        vec![
            through_p(REST, wtuck, win, 680.0),
            MovePiece {
                ms: 640.0 * 3.0,
                kind: PieceKind::SwingWave,
            },
            through_p(win, wtuck, REST, 700.0),
        ],
        false,
        false,
        &[],
        &[],
        win,
    )
}

pub fn flex_move() -> Move {
    let flexed = both(POSES_FLEX, 1.0, 0.0);
    let squeezed = both(POSES_FLEX_SQUEEZE, 1.6, 0.0);
    let flex_tuck = both(POSES_TUCK_UPRIGHT, 0.5, 0.0);
    build_move(
        "flex",
        vec![
            through_p(REST, flex_tuck, flexed, 720.0),
            glide_p(flexed, squeezed, 320.0),
            hold_p(squeezed, 340.0),
            glide_p(squeezed, flexed, 320.0),
            hold_p(flexed, 200.0),
            through_p(flexed, flex_tuck, REST, 780.0),
        ],
        false,
        false,
        &[],
        &[],
        flexed,
    )
}

pub fn stuck_move() -> Move {
    let rack = both(POSES_RACK, 2.5, 0.0);
    let part = mix_target(rack, both(POSES_PRESS, 2.5, 0.0), 0.4);
    let heavy = both(POSES_SAG, 3.0, 0.0);
    let tuck = both(POSES_TUCK_UPRIGHT, 1.5, 0.0);
    build_move(
        "stuck",
        vec![
            through_p(REST, tuck, rack, 780.0),
            MovePiece {
                ms: 900.0,
                kind: PieceKind::Strain,
            },
            glide_p(part, rack, 520.0),
            through_p(rack, tuck, heavy, 880.0),
        ],
        false,
        true,
        &[],
        &[],
        heavy,
    )
}

pub fn resting_move() -> Move {
    build_move("rest", Vec::new(), false, false, &[], &[], REST)
}

fn once_move(name: &'static str, pieces: Vec<MovePiece>) -> Move {
    let still = pieces.first().map_or(REST, |p| p.at(1.0));
    build_move(name, pieces, false, false, &[], &[], still)
}

fn held_move(name: &'static str, target: RigTarget, ms: f64) -> Move {
    build_move(
        name,
        vec![
            glide_p(REST, target, 520.0),
            hold_p(target, ms - 1040.0),
            glide_p(target, REST, 520.0),
        ],
        false,
        false,
        &[],
        &[],
        target,
    )
}

pub const LOOKING_LEFT: f64 = -1.0;
pub const LOOKING_RIGHT: f64 = 1.0;

pub fn look_left_move() -> Move {
    held_move("look-left", with_look(REST, LOOKING_LEFT), 1800.0)
}

pub fn look_right_move() -> Move {
    held_move("look-right", with_look(REST, LOOKING_RIGHT), 1800.0)
}

pub fn moment_move(name: &str) -> Option<Move> {
    let chin = each(REST_ARM, POSES_THINK, 0.4, 0.0);
    let chin_way = each(REST_ARM, POSES_TUCK_UPRIGHT, 0.3, 0.0);
    match name {
        "curls" => Some(once_move("curls", curls_pieces(2))),
        "press" => Some(once_move("press", press_pieces(2))),
        "alternating" => Some(once_move("alternating", alternating_pieces(1))),
        "hammer" => Some(once_move("hammer", hammer_pieces(2))),
        "curious" => Some(build_move(
            "curious",
            vec![
                through_p(REST, with_look(chin_way, 0.5), with_look(chin, 0.5), 560.0),
                hold_p(with_look(chin, 0.5), 880.0),
                through_p(with_look(chin, 0.5), chin_way, REST, 560.0),
            ],
            false,
            false,
            &[],
            &[],
            with_look(chin, 0.5),
        )),
        "doze" => Some(held_move("doze", both(REST_ARM, 0.8, 0.0), 2600.0)),
        _ => None,
    }
}

pub fn buddy_moments() -> Vec<IdleMoment> {
    let defs: [(
        &str,
        [&str; 2],
        Option<BotMood>,
        Option<Glance>,
        Option<BotAvatarState>,
        f64,
    ); 7] = [
        ("curls", ["^", "^"], Some(BotMood::Glad), None, None, 0.0),
        ("press", ["o", "o"], None, None, None, 0.0),
        (
            "look",
            ["o", "o"],
            None,
            Some(Glance { x: 1.0, y: -0.35 }),
            None,
            1.8,
        ),
        (
            "alternating",
            ["^", "^"],
            Some(BotMood::Glad),
            None,
            None,
            0.0,
        ),
        (
            "curious",
            ["o", "o"],
            Some(BotMood::Curious),
            None,
            None,
            0.0,
        ),
        ("hammer", ["^", "^"], Some(BotMood::Glad), None, None, 0.0),
        (
            "doze",
            ["c", "c"],
            None,
            None,
            Some(BotAvatarState::Sleeping),
            0.0,
        ),
    ];
    defs.into_iter()
        .map(|(name, eyes, mood, glance, state, secs)| {
            let seconds = moment_move(name).map_or(secs, |m| m.ms / 1000.0 + 0.4);
            IdleMoment {
                name,
                eyes,
                seconds,
                mood,
                glance,
                state,
            }
        })
        .collect()
}

pub fn buddy_moment(seed: f64, count: u32) -> IdleMoment {
    let list = buddy_moments();
    let start = (seeded(seed, 0, 3) * (list.len() as f64)).floor() as usize;
    let mut chosen = list[(start + (count as usize)) % list.len()];
    if let Some(mut g) = chosen.glance {
        g.x = if seeded(seed, count, 2) < 0.5 {
            -1.0
        } else {
            1.0
        };
        chosen.glance = Some(g);
    }
    chosen
}

pub fn buddy_move_for(key: &str, look: Option<f64>, level: BotMotionLevel) -> Move {
    match key {
        "working" | "delegating" | "responding" => match level {
            BotMotionLevel::Subtle => workout_standing_move(),
            BotMotionLevel::Full => workout_move(),
        },
        "thinking" => think_move(),
        "waiting" => wait_move(),
        "receiving" | "idle:noticed" => wave_move(),
        "done" => flex_move(),
        "blocked" => stuck_move(),
        "idle:look" => {
            if look.unwrap_or(1.0) < 0.0 {
                look_left_move()
            } else {
                look_right_move()
            }
        }
        "working:think" => think_move(),
        other if other.starts_with("working:") => match level {
            BotMotionLevel::Subtle => workout_standing_move(),
            BotMotionLevel::Full => workout_move(),
        },
        other if other.starts_with("idle:") => {
            moment_move(&other["idle:".len()..]).unwrap_or_else(resting_move)
        }
        _ => resting_move(),
    }
}

pub fn has_routine(id: &str) -> bool {
    id == "codex-buddy"
}

pub fn glance_look(side: crate::bot_rig::GlanceSide) -> Option<f64> {
    match side {
        crate::bot_rig::GlanceSide::Left => Some(LOOKING_LEFT),
        crate::bot_rig::GlanceSide::Right => Some(LOOKING_RIGHT),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveAt {
    pub target: RigTarget,
    pub settled: bool,
}

pub fn move_pose_at(m: &Move, elapsed: f64, from: f64, still: bool) -> MoveAt {
    if still {
        return MoveAt {
            target: m.still,
            settled: true,
        };
    }
    if m.ms <= 0.0 {
        return MoveAt {
            target: if m.holds { m.still } else { REST },
            settled: true,
        };
    }
    let into = from.max(0.0) + elapsed.max(0.0);
    if m.loops {
        return MoveAt {
            target: m.at(into % m.ms),
            settled: false,
        };
    }
    if into < m.ms {
        return MoveAt {
            target: m.at(into),
            settled: false,
        };
    }
    MoveAt {
        target: if m.holds { m.at(m.ms) } else { REST },
        settled: true,
    }
}

// =========================================================================
// Framing, blinks & PetScreenConductor (`PetSprite.tsx`)
// =========================================================================

pub const CLOSEST_FRAMING: f64 = 0.55;
pub const FRAMING_TOP: f64 = 6.0;
pub const GLASS_CORNER: f64 = 0.32;
pub const GLASS_INK: f64 = 2.0;
pub const PET_BLINK_S: f64 = 0.17;
pub const EYES_REST_S: f64 = 0.4;
pub const PET_BLINK_EVERY_S: f64 = 3.6;
pub const PET_BLINK_JITTER_S: f64 = 1.4;
pub const OLD_TWEENS_DATABASE: &str = "locust-pet-tweens";

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PetWindow {
    pub left: f64,
    pub top: f64,
    pub side: f64,
}

pub fn pet_window(frame_height: f64, middle: f64) -> PetWindow {
    let side = frame_height * CLOSEST_FRAMING;
    PetWindow {
        left: middle - side / 2.0,
        top: FRAMING_TOP,
        side,
    }
}

pub fn glass_units(rect_w: f64) -> f64 {
    EYES_VISOR.half_width / (rect_w / 2.0)
}

pub fn pet_blink_shut(s: f64, since: f64, seed: f64) -> f64 {
    if s == 0.0 {
        return 0.0;
    }
    let into = s - since - BLINK_GAP_S;
    if into < 0.0 {
        return 0.0;
    }
    let k = (into / PET_BLINK_EVERY_S).floor() as isize;
    for n in [k, k - 1] {
        if n < 0 {
            continue;
        }
        let nu = n as u32;
        let at = since
            + BLINK_GAP_S
            + (n as f64) * PET_BLINK_EVERY_S
            + seeded(seed, nu, 9) * PET_BLINK_JITTER_S;
        let p = (s - at) / PET_BLINK_S;
        if (0.0..=1.0).contains(&p) {
            return if p < 0.5 { p * 2.0 } else { (1.0 - p) * 2.0 };
        }
    }
    0.0
}

#[derive(Debug, Clone)]
pub struct PetScreenAsk {
    pub eyes: Option<[String; 2]>,
    pub phosphor: Phosphor,
    pub flash: Option<Phosphor>,
    pub key: String,
    pub r#move: Move,
    pub rests: bool,
    pub crt: Option<CrtKind>,
}

impl PetScreenAsk {
    pub fn from_activity(key: &str, activity: FaceActivity, look: Option<f64>, level: BotMotionLevel) -> Self {
        let eyes = if key == "idle" {
            None
        } else {
            eye_glyphs_for(activity).map(|[l, r]| [l.to_string(), r.to_string()])
        };
        let phosphor = match activity {
            FaceActivity::Waiting => Phosphor::Amber,
            FaceActivity::Blocked => Phosphor::Red,
            _ => Phosphor::Cyan,
        };
        let flash = if activity == FaceActivity::Done {
            Some(Phosphor::Green)
        } else {
            None
        };
        Self {
            eyes,
            phosphor,
            flash,
            key: key.to_string(),
            r#move: buddy_move_for(key, look, level),
            rests: matches!(activity, FaceActivity::Idle | FaceActivity::Blocked),
            crt: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PetScreenFrame {
    pub rig: RigTarget,
    pub glass: Option<ScreenRectF>,
    pub pair: [String; 2],
    pub motions: [GlyphMotion; 2],
    pub squash: f64,
    pub closed_squash: f64,
    pub light: (String, String),
    pub booting: Option<f64>,
    pub opened: f64,
    pub move_name: &'static str,
    pub lift: Option<&'static str>,
    pub settled: bool,
}

#[derive(Debug, Clone)]
struct BodyMoveState {
    key: Option<String>,
    r#move: Move,
    from: f64,
    start: f64,
    hand_from: Option<RigTarget>,
    hand_start: RigTarget,
    hand_ms: f64,
}

pub struct PetScreenConductor {
    seed: f64,
    boot_at: Option<f64>,
    boot_span: f64,
    changes_seen: u32,
    changes: FaceChanges,
    rig: BuddyRigSim,
    last_s: Option<f64>,
    body: Option<BodyMoveState>,
    plate_glass: Option<ScreenRect>,
    resting_since: Option<f64>,
    turns: HashMap<&'static str, usize>,
}

impl PetScreenConductor {
    pub fn new(face: &PetScreenFace, seed: f64, initial: &PetScreenAsk) -> Self {
        let shown = ShownFace {
            pair: initial.eyes.clone(),
            tone: initial.phosphor,
            key: Some(initial.key.clone()),
        };
        Self {
            seed,
            boot_at: None,
            boot_span: POWER_ON_S,
            changes_seen: 0,
            changes: FaceChanges::new(shown),
            rig: BuddyRigSim::default(),
            last_s: None,
            body: None,
            plate_glass: screen_at(
                face,
                crate::buddy_rig::BUDDY_RIG_PLATE_ROW,
                crate::buddy_rig::BUDDY_RIG_PLATE_COL,
            ),
            resting_since: None,
            turns: HashMap::new(),
        }
    }

    pub fn boot(&mut self, at: f64) {
        self.boot_at = Some(at);
        self.boot_span = POWER_ON_S;
    }

    fn begin(&mut self, key: Option<String>, m: Move, from: f64) {
        let mut start = 0.0;
        if !m.sets.is_empty() {
            let turn = self.turns.get(m.name).copied().unwrap_or_else(|| {
                (seeded(self.seed, 0, 5) * (m.sets.len() as f64)).floor() as usize
            });
            start = m.sets[turn % m.sets.len()];
            self.turns.insert(m.name, turn + 1);
        }
        let hand_from = self.body.as_ref().map(|_| self.rig.now());
        let hand_start = move_pose_at(&m, 0.0, start, false).target;
        let hand_ms = hand_from
            .map(|hf| handoff_ms(&hf, &hand_start))
            .unwrap_or(0.0);
        self.body = Some(BodyMoveState {
            key,
            r#move: m,
            from,
            start,
            hand_from,
            hand_start,
            hand_ms,
        });
    }

    pub fn frame(
        &mut self,
        s: f64,
        ask: &PetScreenAsk,
        turned_to: Option<crate::bot_rig::GlanceSide>,
        crt_on_change: Option<&dyn Fn(Option<&str>, &str, u32) -> Option<CrtKind>>,
    ) -> PetScreenFrame {
        let wanted = ShownFace {
            pair: ask.eyes.clone(),
            tone: ask.phosphor,
            key: Some(ask.key.clone()),
        };
        let change = self.changes.at(s, wanted, ask.flash, self.boot_at, self.boot_span);
        let showing_key = self.changes.shown().key.clone();
        let showing_pair = self.changes.shown().pair.clone();
        let showing_tone = self.changes.shown().tone;

        if change.count != self.changes_seen {
            self.changes_seen = change.count;
            let kind = if s == 0.0 {
                None
            } else {
                ask.crt.or_else(|| {
                    crt_on_change.and_then(|f| f(showing_key.as_deref(), &ask.key, change.count))
                })
            };
            if let Some(k) = kind {
                self.boot_at = Some(s);
                self.boot_span = match k {
                    CrtKind::On => POWER_ON_S,
                    CrtKind::Flick => CRT_FLICK_S,
                };
            }
        }

        let answering = showing_key.as_deref() == Some(ask.key.as_str());
        if self.body.is_none() {
            let m = if answering {
                ask.r#move.clone()
            } else {
                resting_move()
            };
            self.begin(showing_key.clone(), m, s);
        } else if change.open && answering {
            let need_switch = self
                .body
                .as_ref()
                .map_or(true, |b| b.key != showing_key || b.r#move.name != ask.r#move.name);
            if need_switch {
                let start_s = if self.body.as_ref().map_or(true, |b| b.key != showing_key) && s != 0.0
                {
                    change.since
                } else {
                    s
                };
                self.begin(showing_key.clone(), ask.r#move.clone(), start_s);
            }
        } else if self.body.as_ref().map_or(false, |b| b.from == 0.0) && s != 0.0 {
            if let Some(ref mut b) = self.body {
                b.from = s;
            }
        }

        let moving = self.body.as_ref().expect("body initialized");
        let elapsed = (s - moving.from) * 1000.0;
        let handing = s != 0.0 && moving.hand_from.is_some() && elapsed < moving.hand_ms;
        let own = move_pose_at(
            &moving.r#move,
            if s == 0.0 {
                elapsed
            } else {
                (elapsed - moving.hand_ms).max(0.0)
            },
            moving.start,
            s == 0.0,
        );
        let wanted_rig = if handing {
            handoff(
                moving.hand_from.unwrap(),
                moving.hand_start,
                elapsed / moving.hand_ms,
            )
        } else {
            own.target
        };
        let turned = turned_to.and_then(glance_look);
        let target = match turned {
            Some(l) => RigTarget {
                look: l,
                ..wanted_rig
            },
            None => wanted_rig,
        };
        let sprung = if s == 0.0 || self.last_s.is_none() {
            self.rig.place(target)
        } else {
            self.rig.step(target, s - self.last_s.unwrap())
        };
        self.last_s = if s == 0.0 { None } else { Some(s) };
        let still = s == 0.0 || self.rig.rests_at(&target);

        if ask.rests && (own.settled || s == 0.0) {
            if self.resting_since.is_none() {
                self.resting_since = Some(s);
            }
        } else {
            self.resting_since = None;
        }
        let rest_k = if s == 0.0 {
            1.0
        } else {
            self.resting_since
                .map_or(0.0, |rs| ((s - rs) / EYES_REST_S).min(1.0))
        };

        let pair = showing_pair.unwrap_or_else(|| {
            [
                SCREEN_RESTING_EYES[0].to_string(),
                SCREEN_RESTING_EYES[1].to_string(),
            ]
        });
        let pair_joined = format!("{}{}", pair[0], pair[1]);
        let loop_at = |eye: usize| -> GlyphMotion {
            let moved = glyph_motion(&pair_joined, eye, s - change.since);
            if rest_k <= 0.0 {
                return moved;
            }
            let rest = glyph_motion(&pair_joined, eye, 0.0);
            let mix = |a: f64, b: f64| a + (b - a) * rest_k;
            GlyphMotion {
                dx: mix(moved.dx, rest.dx),
                dy: mix(moved.dy, rest.dy),
                sx: mix(moved.sx, rest.sx),
                sy: mix(moved.sy, rest.sy),
                shown: if rest_k >= 0.5 { rest.shown } else { moved.shown },
            }
        };

        let blink = if ask.rests {
            0.0
        } else {
            pet_blink_shut(s, change.since, self.seed)
        };
        let phase = match self.boot_at {
            Some(ba) if s != 0.0 => Some((s - ba) / self.boot_span),
            _ => None,
        };
        let booting = match phase {
            Some(p) if p < 1.0 => Some(p.max(0.0)),
            _ => None,
        };
        let opened = booting.map_or(1.0, power_on_eyes);
        let lift = if moving.r#move.sets.is_empty() {
            None
        } else {
            moving
                .r#move
                .sets
                .iter()
                .position(|&st| (st - moving.start).abs() < 1e-9)
                .and_then(|idx| moving.r#move.lifts.get(idx).copied())
        };

        PetScreenFrame {
            rig: sprung,
            glass: self.plate_glass.map(|pg| {
                let mut rf = ScreenRectF::from(pg);
                rf.y += head_drop(sprung.pose.dip);
                rf
            }),
            pair,
            motions: [loop_at(0), loop_at(1)],
            squash: 1.0 - 0.92 * change.shut.max(blink).max(1.0 - opened),
            closed_squash: 1.0 - 0.92 * change.shut,
            light: screen_light(showing_tone, self.changes.flash(), s),
            booting,
            opened,
            move_name: moving.r#move.name,
            lift,
            settled: s == 0.0
                || (change.quiet
                    && own.settled
                    && !handing
                    && still
                    && blink == 0.0
                    && rest_k >= 1.0
                    && booting.is_none()
                    && turned.is_none()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot_rig::{flash_fade, GlanceSide, IDLE_MOMENTS, SWAP_S, FLASH_S};
    use crate::bot_spec::{bot_size, mix_phosphor_css, screen_eye_layout, VisorBox};
    use crate::buddy_rig::{
        arm_joints, dumbbell_reach, neck_of, BUDDY_RIG_MIDDLE, BUDDY_RIG_PLATE_COL,
        BUDDY_RIG_PLATE_ROW,
    };

    fn all_moves() -> Vec<Move> {
        let keys = [
            "idle",
            "thinking",
            "working",
            "delegating",
            "responding",
            "receiving",
            "waiting",
            "done",
            "blocked",
            "idle:noticed",
            "idle:listening",
            "idle:look",
        ];
        let mut out = vec![
            workout_move(),
            workout_standing_move(),
            think_move(),
            wait_move(),
            wave_move(),
            flex_move(),
            stuck_move(),
            resting_move(),
        ];
        for k in keys {
            out.push(buddy_move_for(k, Some(-1.0), BotMotionLevel::Full));
        }
        for m in buddy_moments() {
            out.push(buddy_move_for(
                &format!("idle:{}", m.name),
                Some(-1.0),
                BotMotionLevel::Full,
            ));
        }
        out.push(buddy_move_for("idle:look", Some(1.0), BotMotionLevel::Full));
        out
    }

    fn hand(target: RigTarget, side: i8) -> [f64; 3] {
        let arm = if side < 0 {
            &target.pose.left
        } else {
            &target.pose.right
        };
        let h = arm_joints(arm, side, target.pose.dip).hand;
        [h.x, h.y, h.z]
    }

    // -------------------------------------------------------------------------
    // codex-buddy-lifts-in-sets.test.ts (16 cases)
    // -------------------------------------------------------------------------

    #[test]
    fn ask_only_for_poses_his_rig_can_take_every_joint_a_number_every_pass_a_length() {
        for m in all_moves() {
            let mut t = 0.0;
            while t <= m.ms {
                let target = m.at(t);
                for v in [
                    target.pose.left.abduct,
                    target.pose.left.flex,
                    target.pose.left.bend,
                    target.pose.left.bend_up,
                    target.pose.left.grip,
                    target.pose.right.abduct,
                    target.pose.right.flex,
                    target.pose.right.bend,
                    target.pose.right.bend_up,
                    target.pose.right.grip,
                    target.pose.dip,
                    target.look,
                ] {
                    assert!(v.is_finite(), "{} at {}", m.name, t);
                }
                t += 50.0;
            }
            assert!(m.ms >= 0.0, "{}", m.name);
        }
    }

    #[test]
    fn working_go_round_every_lift_he_has_a_breather_after_each() {
        let w = workout_move();
        assert_eq!(w.lifts, ["press", "curls", "alternating", "hammer"]);
        assert!(w.loops);
        assert_eq!(w.sets.len(), 4);
        // Verify sets start timestamps match computed piece boundaries!
        let mut cursor = 0.0;
        let press_ms: f64 = press_pieces(3).iter().map(|p| p.ms).sum::<f64>() + 900.0;
        let curls_ms: f64 = curls_pieces(3).iter().map(|p| p.ms).sum::<f64>() + 900.0;
        let alt_ms: f64 = alternating_pieces(2).iter().map(|p| p.ms).sum::<f64>() + 900.0;
        let expected_sets = [
            cursor,
            {
                cursor += press_ms;
                cursor
            },
            {
                cursor += curls_ms;
                cursor
            },
            {
                cursor += alt_ms;
                cursor
            },
        ];
        assert_eq!(w.sets, expected_sets);

        for &start in w.sets {
            let prev = w.at((start - 1.0 + w.ms) % w.ms);
            assert_eq!(prev, REST, "before set at {start} ms");
        }
        assert!(w.ms > 18_000.0);
        assert!(w.ms < 50_000.0);
    }

    #[test]
    fn all_standing_beside_a_name_or_not_no_squat_his_body_never_dipping_more_than_a_reps_effort() {
        assert_eq!(
            buddy_move_for("working", None, BotMotionLevel::Subtle).name,
            workout_standing_move().name
        );
        assert_eq!(
            buddy_move_for("working", None, BotMotionLevel::Full).name,
            workout_move().name
        );
        for m in all_moves() {
            let mut t = 0.0;
            while t < m.ms {
                assert!(m.at(t).pose.dip < 3.5, "{}", m.name);
                t += 20.0;
            }
        }
    }

    #[test]
    fn lift_in_reps_a_press_goes_up_over_his_head_and_back_to_his_shoulders_three_times() {
        let w = workout_move();
        let from = w.sets[0];
        let to = w.sets[1];
        let mut lockouts = 0;
        let mut up = false;
        let mut t = from;
        while t < to {
            let y = hand(w.at(t), -1)[1];
            if !up && y < 60.0 {
                lockouts += 1;
                up = true;
            }
            if y > 78.0 {
                up = false;
            }
            t += 10.0;
        }
        assert_eq!(lockouts, 3);
    }

    #[test]
    fn curl_toward_you_at_the_top_his_weight_is_nearer_you_than_his_shoulder_and_higher_than_it_hangs() {
        let hanging = arm_joints(&REST_ARM, -1, 0.0).hand;
        let curled = arm_joints(&POSES_CURL, -1, 0.0).hand;
        assert!(curled.z > 10.0);
        assert!(curled.y < hanging.y - 30.0);
    }

    #[test]
    fn think_in_time_with_the_dots_his_eyes_one_way_and_the_other_while_they_look_about() {
        let th = think_move();
        assert!((th.ms - 2.0 * THINKING_CYCLE_S * 1000.0).abs() < 1e-6);
        for lp in [0.0, 1.0] {
            let look_at = |s: f64| move_pose_at(&th, (lp * THINKING_CYCLE_S + s) * 1000.0, 0.0, false).target.look;
            assert!(look_at(LOOK_ABOUT_S * 0.3) < -0.5);
            assert!(look_at(LOOK_ABOUT_S * 0.75) > 0.5);
            let busy = move_pose_at(
                &th,
                (lp * THINKING_CYCLE_S + LOOK_ABOUT_S + (THINKING_CYCLE_S - LOOK_ABOUT_S) / 2.0) * 1000.0,
                0.0,
                false,
            )
            .target;
            assert_eq!(busy.look, 0.0);
            assert!(hand(busy, -1)[1].min(hand(busy, 1)[1]) < 115.0);
        }
    }

    #[test]
    fn finish_with_a_flex_the_weights_up_by_his_head_then_rest() {
        let fl = flex_move();
        assert_eq!(buddy_move_for("done", None, BotMotionLevel::Full).name, fl.name);
        assert!(!fl.loops);
        let top = move_pose_at(&fl, fl.ms / 2.0, 0.0, false).target;
        assert!(hand(top, -1)[1] < 95.0);
        assert!(hand(top, 1)[1] < 95.0);
        assert_eq!(
            move_pose_at(&fl, fl.ms + 1.0, 0.0, false),
            MoveAt {
                target: REST,
                settled: true
            }
        );
    }

    #[test]
    fn get_stuck_under_a_press_that_will_not_go_up_and_stay_there_until_someone_helps() {
        let st = stuck_move();
        assert_eq!(buddy_move_for("blocked", None, BotMotionLevel::Full).name, st.name);
        assert!(st.holds);
        let mut t = 0.0;
        while t < st.ms {
            assert!(hand(st.at(t), -1)[1] > 70.0, "at {t}");
            t += 10.0;
        }
        let held = move_pose_at(&st, 60_000.0, 0.0, false);
        assert!(held.settled);
        assert_eq!(held.target, st.at(st.ms));
        assert!(held.target.pose.dip > 2.0);
        assert_eq!(st.still, held.target);
    }

    #[test]
    fn wave_at_a_message_or_at_your_pointer_with_one_arm_then_rest() {
        let wv = wave_move();
        assert_eq!(buddy_move_for("receiving", None, BotMotionLevel::Full).name, wv.name);
        assert_eq!(buddy_move_for("idle:noticed", None, BotMotionLevel::Full).name, wv.name);
        let mut swings = 0;
        let mut last = 0.0;
        let mut last_sign = 0.0;
        let mut t = 0.0;
        while t < wv.ms {
            let target = wv.at(t);
            assert_eq!(target.pose.left, REST_ARM);
            let x = hand(target, 1)[0];
            let diff = x - last;
            let sign = if diff.abs() < 1e-12 { 0.0 } else { diff.signum() };
            if t > 0.0 && sign != 0.0 && last_sign != 0.0 && sign != last_sign {
                swings += 1;
            }
            if sign != 0.0 {
                last_sign = sign;
            }
            last = x;
            t += 10.0;
        }
        assert!(swings >= 5);
        assert_eq!(
            move_pose_at(&wv, wv.ms + 1.0, 0.0, false),
            MoveAt {
                target: REST,
                settled: true
            }
        );
    }

    #[test]
    fn answer_each_face() {
        assert_eq!(buddy_move_for("working", None, BotMotionLevel::Full).name, "workout");
        assert_eq!(buddy_move_for("delegating", None, BotMotionLevel::Full).name, "workout");
        assert_eq!(buddy_move_for("responding", None, BotMotionLevel::Full).name, "workout");
        assert_eq!(buddy_move_for("thinking", None, BotMotionLevel::Full).name, "think");
        assert_eq!(buddy_move_for("waiting", None, BotMotionLevel::Full).name, "wait");
        assert_eq!(buddy_move_for("idle", None, BotMotionLevel::Full).name, "rest");
        assert_eq!(buddy_move_for("idle:listening", None, BotMotionLevel::Full).name, "rest");
        assert_eq!(
            move_pose_at(&buddy_move_for("idle:look", Some(-1.0), BotMotionLevel::Full), 900.0, 0.0, false)
                .target
                .look,
            LOOKING_LEFT
        );
        assert_eq!(
            move_pose_at(&buddy_move_for("idle:look", Some(1.0), BotMotionLevel::Full), 900.0, 0.0, false)
                .target
                .look,
            LOOKING_RIGHT
        );
        assert_eq!(glance_look(GlanceSide::Left), Some(LOOKING_LEFT));
        assert_eq!(glance_look(GlanceSide::Right), Some(LOOKING_RIGHT));
        assert_eq!(glance_look(GlanceSide::Up), None);
    }

    #[test]
    fn goes_round_when_it_loops_from_the_set_it_was_begun_at() {
        let w = workout_move();
        let start = w.sets[3];
        assert_eq!(
            move_pose_at(&w, 0.0, start, false),
            MoveAt {
                target: w.at(start),
                settled: false
            }
        );
        assert_eq!(
            move_pose_at(&w, w.ms + 250.0, start, false).target,
            w.at(start + 250.0)
        );
    }

    #[test]
    fn is_its_still_pose_on_a_still_face() {
        for m in all_moves() {
            assert_eq!(
                move_pose_at(&m, 12_345.0, 0.0, true),
                MoveAt {
                    target: m.still,
                    settled: true
                }
            );
        }
    }

    #[test]
    fn is_rest_settled_once_a_move_played_once_is_done() {
        assert_eq!(
            move_pose_at(&resting_move(), 0.0, 0.0, false),
            MoveAt {
                target: REST,
                settled: true
            }
        );
        assert_eq!(
            move_pose_at(&buddy_move_for("idle:curls", None, BotMotionLevel::Full), 60_000.0, 0.0, false),
            MoveAt {
                target: REST,
                settled: true
            }
        );
    }

    #[test]
    fn moments_go_round_every_lift_he_has_in_turn_within_one_round_of_moments() {
        let bm = buddy_moments();
        let w = workout_move();
        for seed in [0.0, 0.31, 0.77] {
            let names: Vec<&str> = (0..bm.len() as u32)
                .map(|c| buddy_moment(seed, c).name)
                .collect();
            let uniq: std::collections::HashSet<&str> = names.iter().copied().collect();
            assert_eq!(uniq.len(), bm.len());
            for lift in w.lifts {
                assert!(names.contains(&lift), "missing {lift}");
            }
        }
    }

    #[test]
    fn moments_show_only_faces_at_rest_never_busy_never_stuck_never_green() {
        let mut shown: std::collections::HashSet<String> = IDLE_MOMENTS
            .iter()
            .map(|m| format!("{}{}", m.eyes[0], m.eyes[1]))
            .collect();
        shown.insert("oo".into());
        shown.insert("^^".into());
        shown.insert("cc".into());
        for m in buddy_moments() {
            let joined = format!("{}{}", m.eyes[0], m.eyes[1]);
            assert!(shown.contains(&joined), "{}", m.name);
            assert!(!["••", ">▮", "><"].contains(&joined.as_str()));
        }
    }

    #[test]
    fn moments_last_as_long_as_the_set_they_show() {
        for m in buddy_moments() {
            if let Some(mv) = moment_move(m.name) {
                assert!(m.seconds * 1000.0 >= mv.ms, "{}", m.name);
            }
        }
    }

    // -------------------------------------------------------------------------
    // codex-buddy-moves-on-his-own-rig.test.ts (rig & framing cases)
    // -------------------------------------------------------------------------

    #[derive(Clone, Copy)]
    struct HandOut {
        x: f64,
        y: f64,
        out: f64,
    }

    fn hands_of(target: RigTarget) -> [HandOut; 2] {
        let window = pet_window(CODEX_BUDDY.frame_height as f64, BUDDY_RIG_MIDDLE);
        let one = |side: i8| -> HandOut {
            let arm = if side < 0 {
                &target.pose.left
            } else {
                &target.pose.right
            };
            let joints = arm_joints(arm, side, target.pose.dip);
            let reach = dumbbell_reach(arm, &joints);
            let in_view = joints.hand.y - reach.y < window.top + window.side;
            let out = if in_view {
                0.0f64
                    .max(window.left - (joints.hand.x - reach.x))
                    .max(joints.hand.x + reach.x - (window.left + window.side))
                    .max(window.top - (joints.hand.y - reach.y))
            } else {
                0.0
            };
            HandOut {
                x: joints.hand.x,
                y: joints.hand.y,
                out,
            }
        };
        [one(-1), one(1)]
    }

    fn base_moves() -> Vec<Move> {
        let mut v = vec![
            workout_move(),
            workout_standing_move(),
            think_move(),
            wait_move(),
            wave_move(),
            flex_move(),
            stuck_move(),
            resting_move(),
        ];
        for m in buddy_moments() {
            if let Some(mv) = moment_move(m.name) {
                v.push(mv);
            }
        }
        v
    }

    fn every_change_moves() -> Vec<Move> {
        let ms = base_moves();
        let mut out = Vec::new();
        for a in &ms {
            for b in &ms {
                out.push(a.clone());
                out.push(b.clone());
            }
        }
        out
    }

    fn run_sim(fps: f64, sprung: bool, moves: &[Move]) -> Vec<[HandOut; 2]> {
        let mut sim = BuddyRigSim::default();
        let mut frames = Vec::new();
        let cuts = [1.0, 0.37, 0.61, 1.0, 0.23, 0.5, 0.81];
        for (i, m) in moves.iter().enumerate() {
            let start = if m.sets.is_empty() {
                0.0
            } else {
                m.sets[i % m.sets.len()]
            };
            let span = (if m.loops { m.ms } else { m.ms + 1500.0 }) * cuts[i % cuts.len()];
            let from = if i == 0 { None } else { Some(sim.now()) };
            let begun = move_pose_at(m, 0.0, start, false).target;
            let ms = from.map(|f| handoff_ms(&f, &begun)).unwrap_or(0.0);
            let mut t = 0.0;
            while t < span + ms {
                let own = move_pose_at(m, (t - ms).max(0.0), start, false).target;
                let target = match from {
                    Some(f) if t < ms => handoff(f, begun, t / ms),
                    _ => own,
                };
                let drawn = if sprung {
                    sim.step(target, 1.0 / fps)
                } else {
                    own
                };
                frames.push(hands_of(drawn));
                t += 1000.0 / fps;
            }
        }
        frames
    }

    fn motion_of(frames: &[[HandOut; 2]]) -> (f64, f64) {
        let mut step = 0.0f64;
        let mut change = 0.0f64;
        for i in 2..frames.len() {
            for h in [0, 1] {
                let a = frames[i - 2][h];
                let b = frames[i - 1][h];
                let c = frames[i][h];
                step = step.max((c.x - b.x).hypot(c.y - b.y));
                change = change.max((c.x - 2.0 * b.x + a.x).hypot(c.y - 2.0 * b.y + a.y));
            }
        }
        (step, change)
    }

    // The cases of codex-buddy-moves-on-his-own-rig.test.ts, one test each, named after the case.

    #[test]
    fn stay_inside_his_window_wherever_they_are_in_view_through_every_move_and_every_change_between_them() {
        let changes = every_change_moves();
        for fps in [30.0, 60.0] {
            let worst = run_sim(fps, true, &changes)
                .iter()
                .flat_map(|f| f.iter())
                .fold(0.0f64, |m, h| m.max(h.out));
            assert_eq!(worst, 0.0, "{fps} fps");
        }
    }

    #[test]
    fn would_be_caught_leaving_it_a_lateral_raise_arms_straight_out_does() {
        let raise = RigTarget::new(
            crate::buddy_rig::BuddyPose {
                left: ArmPose { abduct: 1.45, flex: 0.05, bend: 0.2, bend_up: 0.0, grip: 0.0 },
                right: REST_TARGET.pose.right,
                dip: 0.0,
            },
            0.0,
        );
        assert!(hands_of(raise)[0].out > 10.0);
    }

    /// How far his screen's corners, tilted and dropped with his head, go past his window; 0 inside.
    fn face_out(target: RigTarget) -> f64 {
        let window = pet_window(CODEX_BUDDY.frame_height as f64, BUDDY_RIG_MIDDLE);
        let cell = CODEX_BUDDY.cells[BUDDY_RIG_PLATE_ROW][BUDDY_RIG_PLATE_COL];
        let [x, y, w, h] = [cell[0] as f64, cell[1] as f64, cell[2] as f64, cell[3] as f64];
        let neck = neck_of(target.pose.dip);
        let tilt = target.tilt.unwrap_or(0.0);
        let drop = head_drop(target.pose.dip);
        let mut out = 0.0f64;
        for [cx, cy] in [[x, y], [x + w, y], [x, y + h], [x + w, y + h]] {
            let dx = cx - neck[0];
            let dy = cy + drop - neck[1];
            let px = neck[0] + dx * tilt.cos() - dy * tilt.sin();
            let py = neck[1] + dx * tilt.sin() + dy * tilt.cos();
            out = out
                .max(window.left - px)
                .max(px - (window.left + window.side))
                .max(window.top - py)
                .max(py - (window.top + window.side));
        }
        out
    }

    #[test]
    fn stays_in_his_window_whole_however_his_head_tilts_and_his_body_dips_through_every_move_and_every_change_between_them() {
        let mut sim = BuddyRigSim::default();
        let mut worst = 0.0f64;
        for m in every_change_moves() {
            let mut t = 0.0;
            while t < m.ms + 500.0 {
                worst = worst.max(face_out(sim.step(move_pose_at(&m, t, 0.0, false).target, 1.0 / 30.0)));
                t += 1000.0 / 30.0;
            }
        }
        assert_eq!(worst, 0.0);
    }

    #[test]
    fn would_be_caught_leaving_it_his_body_dropped_far_down_takes_it_out() {
        let dropped = RigTarget::new(crate::buddy_rig::BuddyPose { dip: 60.0, ..REST_TARGET.pose }, 0.0);
        assert!(face_out(dropped) > 5.0);
    }

    #[test]
    fn never_jumps_a_hand_moves_at_most_11_px_of_his_drawing_a_frame_and_its_movement_changes_by_at_most_5() {
        let changes = every_change_moves();
        for fps in [30.0, 60.0] {
            let (step, change) = motion_of(&run_sim(fps, true, &changes));
            assert!(step < (11.0 * 30.0) / fps, "{fps} fps step={step}");
            assert!(change < (5.0 * 30.0 * 30.0) / (fps * fps), "{fps} fps change={change}");
        }
    }

    #[test]
    fn would_be_caught_jumping_the_same_moves_drawn_as_asked_with_no_springs_snap_where_one_cuts_into_the_next() {
        let (_, change) = motion_of(&run_sim(30.0, false, &base_moves()));
        assert!(change > 20.0, "change={change}");
    }

    #[test]
    fn is_the_same_at_30_frames_a_second_as_at_60_and_as_at_144() {
        let w = workout_move();
        let at = |fps: f64| -> RigTarget {
            let mut sim = BuddyRigSim::default();
            let mut now = sim.now();
            let mut t = 0.0;
            while t <= 2000.0 {
                now = sim.step(move_pose_at(&w, t, 0.0, false).target, 1.0 / fps);
                t += 1000.0 / fps;
            }
            now
        };
        let a = hands_of(at(30.0));
        for fps in [60.0, 144.0] {
            let b = hands_of(at(fps));
            for h in [0, 1] {
                assert!((a[h].x - b[h].x).hypot(a[h].y - b[h].y) < 1.5, "{fps} fps");
            }
        }
    }

    #[test]
    fn picks_up_after_his_clock_stopped_from_where_he_was_no_further_than_one_long_frame_would_take_him() {
        let mut sim = BuddyRigSim::default();
        let before = hands_of(sim.now());
        let after = hands_of(sim.step(move_pose_at(&flex_move(), 800.0, 0.0, false).target, 30.0));
        assert!((after[0].x - before[0].x).hypot(after[0].y - before[0].y) < 25.0);
    }

    #[test]
    fn comes_to_rest_within_a_second_and_a_half_of_a_move_ending_so_his_clock_can_stop() {
        for mv in [flex_move(), wave_move(), stuck_move(), moment_move("press").unwrap()] {
            let mut sim = BuddyRigSim::default();
            let mut t = 0.0;
            while t < mv.ms {
                sim.step(move_pose_at(&mv, t, 0.0, false).target, 1.0 / 30.0);
                t += 1000.0 / 30.0;
            }
            let end = move_pose_at(&mv, mv.ms + 1.0, 0.0, false).target;
            let mut rested = -1.0;
            let mut s = 0.0;
            while s < 3.0 {
                sim.step(end, 1.0 / 30.0);
                if sim.rests_at(&end) {
                    rested = s;
                    break;
                }
                s += 1.0 / 30.0;
            }
            assert!(rested >= 0.0, "{} never rested", mv.name);
            assert!(rested < 1.5, "{} rested={rested}", mv.name);
        }
    }

    #[test]
    fn is_placed_where_he_is_asked_at_once_on_a_still_face_and_is_at_rest_there() {
        let mut sim = BuddyRigSim::default();
        let st = stuck_move();
        let placed = sim.place(st.still);
        assert_eq!(placed.pose, st.still.pose);
        assert_eq!(placed.look, st.still.look);
        assert_eq!(placed.tilt, Some(0.0));
        assert!(sim.rests_at(&st.still));
        assert!(!sim.rests_at(&REST_TARGET));
    }

    // -------------------------------------------------------------------------
    // codex-buddys-face-changes-as-a-screen-does.test.ts, one test each. Its three drawn-screen cases (every eye
    // inside the glass, eyes in a bot screen's units, the ink round the glass) have no Rust test yet.
    // -------------------------------------------------------------------------

    fn ask_for(key: &str) -> PetScreenAsk {
        let act = match key {
            "thinking" => FaceActivity::Thinking,
            "working" | "delegating" | "responding" => FaceActivity::Working,
            "waiting" => FaceActivity::Waiting,
            "done" => FaceActivity::Done,
            "blocked" => FaceActivity::Blocked,
            "receiving" => FaceActivity::Receiving,
            _ => FaceActivity::Idle,
        };
        PetScreenAsk::from_activity(key, act, None, BotMotionLevel::Full)
    }

    fn off_by(a: RigTarget, b: RigTarget) -> f64 {
        let vals = |t: RigTarget| {
            [
                t.pose.left.abduct, t.pose.left.flex, t.pose.left.bend, t.pose.left.bend_up, t.pose.left.grip,
                t.pose.right.abduct, t.pose.right.flex, t.pose.right.bend, t.pose.right.bend_up, t.pose.right.grip,
                t.pose.dip / 10.0, t.look,
            ]
        };
        vals(a).iter().zip(vals(b).iter()).map(|(x, y)| (x - y).abs()).fold(0.0f64, f64::max)
    }

    /// Two of a glyph's moments, the same to within rounding.
    fn same_motion(a: GlyphMotion, b: GlyphMotion) {
        for (x, y) in [(a.dx, b.dx), (a.dy, b.dy), (a.sx, b.sx), (a.sy, b.sy)] {
            assert!((x - y).abs() < 1e-9, "{a:?} vs {b:?}");
        }
        assert_eq!(a.shown, b.shown);
    }

    const FRAME: f64 = 1.0 / 30.0;

    #[test]
    fn shuts_his_eyes_brings_the_new_ones_in_under_the_lids_opens_them_and_only_then_moves_his_body() {
        let idle = ask_for("idle");
        let mut cond = PetScreenConductor::new(&CODEX_BUDDY, 0.3, &idle);
        let s = 100.0;
        assert_eq!(off_by(cond.frame(s, &idle, None, None).rig, REST_TARGET), 0.0);

        let working = ask_for("working");
        let began = s + FRAME;
        let mut opened: Option<f64> = None;
        let mut s = began;
        while s < began + 1.0 {
            let now = cond.frame(s, &working, None, None);
            let p = (s - began) / SWAP_S;
            if p < 0.5 {
                // Shutting on the old eyes.
                assert_eq!(now.pair.join(""), "||", "p {p:.2}");
                if p > 0.0 {
                    assert!(now.squash < 1.0);
                }
            } else if p < 1.0 {
                // The new eyes, under lids that open.
                assert_eq!(now.pair.join(""), ">▮");
            }
            if p < 1.0 {
                // His body holds where it was until his eyes are open.
                assert_eq!(off_by(now.rig, REST_TARGET), 0.0, "p {p:.2}");
            } else if off_by(now.rig, REST_TARGET) > 0.0 && opened.is_none() {
                opened = Some(s);
            }
            s += FRAME;
        }
        // Then his workout, from the lift his seed picks.
        let opened = opened.expect("his body moved once his eyes were open");
        assert!(opened - began >= SWAP_S - 1e-9);
        assert!(opened - began < SWAP_S + 2.0 * FRAME);
        assert_eq!(cond.frame(began + 1.0, &working, None, None).move_name, "workout");
    }

    #[test]
    fn flashes_a_finish_green_as_his_new_eyes_come_in_never_on_the_old_ones_and_fades_back() {
        let working = ask_for("working");
        let mut cond = PetScreenConductor::new(&CODEX_BUDDY, 0.3, &working);
        cond.frame(100.0, &working, None, None);
        let done = ask_for("done");
        let began = 100.0 + FRAME;
        let mut t = began;
        while t < began + SWAP_S / 2.0 - 1e-6 {
            let now = cond.frame(t, &done, None, None);
            assert_eq!(now.pair.join(""), ">▮");
            assert_eq!(now.light.0, Phosphor::Cyan.lit_css());
            t += FRAME / 4.0;
        }
        let arriving = cond.frame(began + SWAP_S / 2.0 + 0.01, &done, None, None);
        assert_eq!(arriving.pair.join(""), "^^");
        assert_eq!(arriving.light.0, mix_phosphor_css(Phosphor::Green.lit_css(), Phosphor::Cyan.lit_css(), 0.0));
        assert_eq!(cond.frame(began + SWAP_S / 2.0 + FLASH_S + 0.05, &done, None, None).light.0, Phosphor::Cyan.lit_css());
        // And he flexes once his eyes are open.
        assert_eq!(cond.frame(began + SWAP_S + 0.05, &done, None, None).move_name, flex_move().name);
    }

    #[test]
    fn is_his_at_once_on_a_still_face_the_moves_still_pose_its_eyes_at_rest_settled() {
        for key in ["idle", "thinking", "working", "waiting", "done", "blocked", "receiving"] {
            let idle = ask_for("idle");
            let mut cond = PetScreenConductor::new(&CODEX_BUDDY, 0.3, &idle);
            cond.frame(0.0, &idle, None, None);
            let asked = ask_for(key);
            let now = cond.frame(0.0, &asked, None, None);
            let still = buddy_move_for(key, None, BotMotionLevel::Full).still;
            assert_eq!(now.rig.pose, still.pose, "{key}");
            assert_eq!(now.rig.look, still.look, "{key}");
            assert_eq!(now.squash, 1.0, "{key}");
            assert!(now.settled, "{key}");
            same_motion(now.motions[0], glyph_motion(&now.pair.join(""), 0, 0.0));
        }
    }

    #[test]
    fn he_finishes_his_move_his_springs_and_his_eyes_come_to_rest_and_he_is_settled_stuck_under_the_weights() {
        let working = ask_for("working");
        let blocked = ask_for("blocked");
        let mut cond = PetScreenConductor::new(&CODEX_BUDDY, 0.3, &working);
        cond.frame(100.0, &working, None, None);
        let mut settled_at: Option<f64> = None;
        let mut s = 100.0 + FRAME;
        while s < 106.0 {
            let now = cond.frame(s, &blocked, None, None);
            if now.settled {
                settled_at.get_or_insert(s);
                assert!(off_by(now.rig, stuck_move().still) < 0.01);
            } else {
                settled_at = None;
            }
            s += FRAME;
        }
        let playing = SWAP_S + stuck_move().ms / 1000.0;
        let settled_at = settled_at.expect("he settled");
        assert!(settled_at - 100.0 >= playing + EYES_REST_S - 2.0 * FRAME);
        // Settled, nothing on him changes: his eyes at their loop's rest.
        let last = cond.frame(106.0, &blocked, None, None);
        same_motion(last.motions[0], glyph_motion("><", 0, 0.0));
        assert_eq!(last.squash, 1.0);
    }

    #[test]
    fn at_rest_he_does_not_blink_on_his_own_moving_he_does() {
        let idle = ask_for("idle");
        let working = ask_for("working");
        let mut resting = PetScreenConductor::new(&CODEX_BUDDY, 0.3, &idle);
        let mut moving = PetScreenConductor::new(&CODEX_BUDDY, 0.3, &working);
        let (mut resting_blinks, mut moving_blinks) = (0, 0);
        let mut s = 100.0;
        while s < 130.0 {
            if resting.frame(s, &idle, None, None).squash < 1.0 {
                resting_blinks += 1;
            }
            if moving.frame(s, &working, None, None).squash < 1.0 {
                moving_blinks += 1;
            }
            s += FRAME;
        }
        assert_eq!(resting_blinks, 0);
        assert!(moving_blinks > 0);
    }

    #[test]
    fn starts_each_time_from_the_next_lift() {
        let idle = ask_for("idle");
        let working = ask_for("working");
        let mut face = PetScreenConductor::new(&CODEX_BUDDY, 0.62, &idle);
        let lifts = workout_move().lifts;
        let mut begun = Vec::new();
        let mut s = 100.0;
        face.frame(s, &idle, None, None);
        for _ in 0..3 {
            // The set his work begins from, once his eyes are open.
            let mut first: Option<&str> = None;
            let until = s + 1.0;
            while s < until {
                let now = face.frame(s, &working, None, None);
                if now.move_name == "workout" && first.is_none() {
                    first = now.lift;
                }
                s += FRAME;
            }
            begun.push(lifts.iter().position(|l| Some(*l) == first));
            let until = s + 2.0;
            while s < until {
                face.frame(s, &idle, None, None);
                s += FRAME;
            }
        }
        // Three sets in turn, each the one after the last.
        assert!(begun.iter().all(|set| set.is_some()));
        let begun: Vec<usize> = begun.into_iter().flatten().collect();
        assert_eq!(begun[1], (begun[0] + 1) % lifts.len());
        assert_eq!(begun[2], (begun[0] + 2) % lifts.len());
    }

    #[test]
    fn turns_his_eyes_that_way_while_it_lasts_easing_there_and_back_and_he_is_not_settled_while_it_does() {
        let idle = ask_for("idle");
        let mut face = PetScreenConductor::new(&CODEX_BUDDY, 0.3, &idle);
        let mut s = 100.0;
        let mut run = |face: &mut PetScreenConductor, turned: Option<GlanceSide>, seconds: f64, s: &mut f64| {
            let mut now = face.frame(*s, &idle, turned, None);
            let until = *s + seconds;
            while *s < until {
                now = face.frame(*s, &idle, turned, None);
                *s += FRAME;
            }
            now
        };
        run(&mut face, None, 1.0, &mut s);
        let turning = face.frame(s, &idle, Some(GlanceSide::Left), None);
        s += FRAME;
        // Eased, not snapped.
        assert!(turning.rig.look > -0.5);
        assert!(!turning.settled);
        assert!(run(&mut face, Some(GlanceSide::Left), 0.6, &mut s).rig.look < -0.9);
        assert!(run(&mut face, Some(GlanceSide::Right), 0.6, &mut s).rig.look > 0.9);
        assert!(run(&mut face, None, 1.0, &mut s).rig.look.abs() < 0.02);
    }

    #[test]
    fn come_no_sooner_after_a_change_than_a_bots_and_no_more_than_5_s_apart() {
        for seed in [0.0, 0.4, 0.93] {
            let mut shut = Vec::new();
            let mut s = 50.0;
            while s < 120.0 {
                if pet_blink_shut(s, 50.0, seed) > 0.5 {
                    shut.push(s);
                }
                s += 0.01;
            }
            let blinks: Vec<f64> = shut
                .iter()
                .copied()
                .enumerate()
                .filter(|&(i, s)| i == 0 || s - shut[i - 1] > PET_BLINK_S)
                .map(|(_, s)| s)
                .collect();
            assert!(blinks.len() > 10, "seed {seed}");
            assert!(blinks[0] - 50.0 >= BLINK_GAP_S, "seed {seed}");
            for w in blinks.windows(2) {
                let gap = w[1] - w[0];
                assert!(gap >= BLINK_GAP_S - 0.02, "seed {seed} gap={gap}");
                assert!(gap <= 5.02, "seed {seed} gap={gap}");
            }
        }
    }

    #[test]
    fn are_none_on_a_still_face_and_the_same_every_time() {
        assert_eq!(pet_blink_shut(0.0, 0.0, 0.5), 0.0);
        assert_eq!(pet_blink_shut(4321.234, 12.0, 0.5), pet_blink_shut(4321.234, 12.0, 0.5));
    }

    fn shown(pair: Option<[&str; 2]>, key: &str) -> ShownFace {
        ShownFace { pair: pair.map(|[a, b]| [a.to_string(), b.to_string()]), tone: Phosphor::Cyan, key: Some(key.to_string()) }
    }

    #[test]
    fn is_the_bots_and_his_face_changes_read_frame_by_frame() {
        let mut changes = FaceChanges::new(shown(Some(["|", "|"]), "idle"));
        assert!(changes.at(10.0, shown(Some(["|", "|"]), "idle"), None, None, POWER_ON_S).open);
        let wanted = shown(Some([">", "▮"]), "working");
        let shutting = changes.at(10.0 + FRAME, wanted.clone(), None, None, POWER_ON_S);
        assert!(!shutting.open);
        assert_eq!(changes.coming().unwrap().key.as_deref(), Some("working"));
        assert_eq!(changes.shown().key.as_deref(), Some("idle"));
        let open = changes.at(10.0 + FRAME + SWAP_S + 0.001, wanted, None, None, POWER_ON_S);
        assert!(open.open);
        assert!((open.since - (10.0 + FRAME + SWAP_S)).abs() < 1e-9);
        assert_eq!(changes.shown().key.as_deref(), Some("working"));
        assert_eq!(open.count, 1);
    }

    #[test]
    fn rests_his_eyes_as_it_rests_a_still_bots_a_still_face_changes_at_once() {
        let mut changes = FaceChanges::new(shown(None, "idle"));
        let still = changes.at(0.0, shown(Some(["^", "^"]), "done"), Some(Phosphor::Green), None, POWER_ON_S);
        assert!(still.open);
        assert_eq!(still.shut, 0.0);
        assert!(still.quiet);
        assert_eq!(still.since, 0.0);
        assert_eq!(still.eyes, Some(["^".to_string(), "^".to_string()]));
        assert!(changes.flash().is_none());
        assert_eq!(resting_move().ms, 0.0);
    }

    // -------------------------------------------------------------------------
    // codex-buddy-wears-a-screen.test.tsx: his table, framing, sheet checks, and the choice kept (the cases that
    // draw him are in `src/look/faces.rs`)
    // -------------------------------------------------------------------------

    fn sheet_like(
        face: &PetScreenFace,
        shift_cell: Option<(usize, usize, isize)>,
    ) -> (Vec<u8>, usize, usize) {
        let width = (face.frame_width as usize) * PET_COLUMNS;
        let height = (face.frame_height as usize) * 9;
        let mut data = vec![0u8; width * height * 4];
        for (r, row) in face.edges.iter().enumerate() {
            for (c, &[left, right]) in row.iter().enumerate() {
                let shift = match shift_cell {
                    Some((sr, sc, by)) if sr == r && sc == c => by,
                    _ => 0,
                };
                for y in (r * (face.frame_height as usize) + 5)..(r * (face.frame_height as usize) + 203) {
                    let xl = ((c * (face.frame_width as usize)) as isize + left as isize + shift) as usize;
                    let xr = ((c * (face.frame_width as usize)) as isize + right as isize + shift) as usize;
                    for x in xl..xr {
                        data[(y * width + x) * 4 + 3] = 255;
                    }
                }
            }
        }
        (data, width, height)
    }

    #[test]
    fn is_his_alone_no_other_pet_has_a_screen_measured_for_it_or_moves_of_its_own() {
        assert!(std::ptr::eq(screen_face_for("codex-buddy").unwrap(), &CODEX_BUDDY));
        assert!(screen_face_for("hoodie-cat").is_none());
        assert!(screen_face_for("astro-bot").is_none());
        assert!(has_routine("codex-buddy"));
        assert!(!has_routine("hoodie-cat"));
    }

    #[test]
    fn has_a_screen_on_every_one_of_his_drawings_inside_the_drawing_he_is_painted_in() {
        for (row, &count) in CODEX_BUDDY.counts.iter().enumerate() {
            assert_eq!(CODEX_BUDDY.cells[row].len(), count);
            assert_eq!(CODEX_BUDDY.edges[row].len(), count);
            for column in 0..count {
                let rect = screen_at(&CODEX_BUDDY, row, column).unwrap_or_else(|| panic!("{row},{column}"));
                let [left, right] = CODEX_BUDDY.edges[row][column];
                assert!(rect.x >= left);
                assert!(rect.x + rect.w <= right);
                assert!(rect.y >= 5);
                assert!(rect.y + rect.h <= 203);
                // A face's screen, not a speck: big enough for a bot's two eyes.
                assert!(rect.w >= 30);
                assert!(rect.h >= 22);
            }
            assert!(screen_at(&CODEX_BUDDY, row, count).is_none());
        }
    }

    #[test]
    fn holds_its_size_as_he_moves_one_size_for_each_way_he_faces() {
        use std::collections::BTreeSet;
        let size = |row: usize, column: usize| {
            let cell = CODEX_BUDDY.cells[row][column];
            format!("{}x{}", cell[2], cell[3])
        };
        let sizes = |cells: &[(usize, usize)]| -> BTreeSet<String> { cells.iter().map(|&(r, c)| size(r, c)).collect() };
        let row = |r: usize, columns: &[usize]| -> Vec<(usize, usize)> { columns.iter().map(|&c| (r, c)).collect() };
        let one = |s: &str| -> BTreeSet<String> { BTreeSet::from([s.to_string()]) };
        // Facing you: standing, waving, at the bar, curling, pressing.
        let facing = [
            row(0, &[0, 1, 2, 3, 4, 5]),
            row(3, &[0, 1, 2, 3]),
            row(4, &[1, 2, 3]),
            row(6, &[0, 1, 3, 4, 5]),
            row(7, &[0, 1, 2, 4, 5]),
            row(8, &[0, 1, 2, 3, 4, 5]),
        ]
        .concat();
        assert_eq!(sizes(&facing), one("37x27"));
        // Turned, walking right or left: one size for both.
        let turned = [row(1, &[0, 1, 2, 3, 4, 5, 6, 7]), row(2, &[0, 1, 2, 3, 4, 5, 6, 7])].concat();
        assert_eq!(sizes(&turned), one("37x30"));
        // On the bench, his head nearer and over his brows.
        assert_eq!(sizes(&row(5, &[0, 1, 2, 3, 4, 5, 6, 7])), one("39x31"));
        // Squatting, drawn larger, and his head tucked between his arms, hanging and locked out: smaller, as measured.
        assert_eq!(size(6, 2), "44x32");
        for (r, c) in [(4, 0), (4, 4), (7, 3)] {
            assert!(CODEX_BUDDY.cells[r][c][2] < 37, "{r},{c}");
        }
    }

    #[test]
    fn at_every_size_so_his_face_is_half_as_big_again_as_fitted_whole_where_he_is_usually_seen_and_larger() {
        let win = pet_window(208.0, BUDDY_RIG_MIDDLE);
        // His screen's width in a box `size` across, as the box frames him.
        let screen_width = |size: f64| (37.0 * size) / win.side;
        for size in [
            bot_size::THREAD_LIVE as f64,
            bot_size::SIDEBAR_FACES as f64,
            bot_size::WORKROOM_HEADER as f64,
            bot_size::ROSTER_CARD as f64,
            64.0,
            120.0,
        ] {
            assert!(screen_width(size) / ((37.0 * size) / 208.0) > 1.6, "{size}");
        }
        assert!(screen_width(bot_size::SIDEBAR_FACES as f64) > 9.0);
        assert_eq!(
            win,
            PetWindow {
                left: BUDDY_RIG_MIDDLE - 208.0 * CLOSEST_FRAMING / 2.0,
                top: FRAMING_TOP,
                side: 208.0 * CLOSEST_FRAMING
            }
        );
    }

    #[test]
    fn never_cuts_his_face_on_every_drawing_his_screen_is_inside_his_window() {
        let shown = pet_window(208.0, BUDDY_RIG_MIDDLE);
        for (r, row) in CODEX_BUDDY.cells.iter().enumerate() {
            for (c, &[x, y, w, h]) in row.iter().enumerate() {
                assert!((x as f64) - 2.0 >= shown.left, "{r},{c}");
                assert!((x + w) as f64 + 2.0 <= shown.left + shown.side, "{r},{c}");
                assert!((y as f64) - 2.0 >= shown.top, "{r},{c}");
                assert!((y + h) as f64 + 2.0 <= shown.top + shown.side, "{r},{c}");
            }
        }
    }

    #[test]
    fn takes_the_sheet_that_was_measured() {
        let (data, width, height) = sheet_like(&CODEX_BUDDY, None);
        assert!(screen_fits(&CODEX_BUDDY, &data, width, height));
    }

    #[test]
    fn takes_it_re_compressed_an_edge_a_pixel_or_two_off_is_the_same_drawing() {
        let (data, width, height) = sheet_like(&CODEX_BUDDY, Some((5, 2, 2)));
        assert!(screen_fits(&CODEX_BUDDY, &data, width, height));
    }

    #[test]
    fn refuses_one_redrawn_resized_or_with_a_drawing_where_his_has_none() {
        let (moved, mw, mh) = sheet_like(&CODEX_BUDDY, Some((7, 3, (EDGE_SLACK + 2) as isize)));
        assert!(!screen_fits(&CODEX_BUDDY, &moved, mw, mh));
        let (mut data, width, height) = sheet_like(&CODEX_BUDDY, None);
        assert!(!screen_fits(&CODEX_BUDDY, &data, width, height - 208));
        // A drawing in row 0's empty seventh cell.
        for y in 5..203 {
            data[(y * width + 6 * 192 + 90) * 4 + 3] = 255;
        }
        assert!(!screen_fits(&CODEX_BUDDY, &data, width, height));
    }

    #[test]
    fn reads_a_drawings_painted_sides() {
        let (data, width, _) = sheet_like(&CODEX_BUDDY, None);
        assert_eq!(painted_edges(&data, width, 0, 0, 192, 208), Some([33, 158]));
        assert_eq!(painted_edges(&data, width, 7 * 192, 0, 192, 208), None);
    }

    #[test]
    fn gives_him_his_screen_when_it_is_the_sheet_measured_and_not_when_it_is_another() {
        // The window's decision (pets.ts `ensurePet`): the table for the pet's id, kept only if the sheet fits it.
        let read = |id: &str, sheet: &(Vec<u8>, usize, usize)| {
            screen_face_for(id).filter(|face| screen_fits(face, &sheet.0, sheet.1, sheet.2))
        };
        let measured = sheet_like(&CODEX_BUDDY, None);
        assert!(std::ptr::eq(read("codex-buddy", &measured).unwrap(), &CODEX_BUDDY));
        let another = sheet_like(&CODEX_BUDDY, Some((2, 4, (EDGE_SLACK + 3) as isize)));
        assert!(read("codex-buddy", &another).is_none());
        // Another pet drawn the same is still not him.
        assert!(read("hoodie-cat", &measured).is_none());
    }
}

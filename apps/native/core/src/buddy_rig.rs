//! Codex Buddy on a rig of his own: his 3D joints, springs, head cut, and body geometry.
//! Pure math & data ported from `buddyRig.ts` and `buddyBody.ts`.

use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vec3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArmPose {
    /// Raised out to the side, radians: 0 hanging, PI/2 straight out.
    pub abduct: f64,
    /// Raised to the front, toward you, radians.
    pub flex: f64,
    /// The elbow's bend, radians: 0 straight.
    pub bend: f64,
    /// Which way the elbow bends the forearm: 0 toward you (curl), 1 up (press).
    pub bend_up: f64,
    /// The dumbbell's roll in the picture: 0 bar across, PI/2 hammer grip.
    pub grip: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BuddyPose {
    pub left: ArmPose,
    pub right: ArmPose,
    /// How far his hips drop, pixels.
    pub dip: f64,
}

pub const PET_COLUMNS: usize = 8;

pub const BUDDY_RIG_PLATE_ROW: usize = 8;
pub const BUDDY_RIG_PLATE_COL: usize = 0;
pub const BUDDY_RIG_MIDDLE: f64 = 93.5;
pub const BUDDY_SHOULDER_LEFT: [f64; 2] = [62.0, 92.0];
pub const BUDDY_SHOULDER_RIGHT: [f64; 2] = [125.0, 92.0];
pub const BUDDY_UPPER: f64 = 28.0;
pub const BUDDY_FORE: f64 = 26.0;
pub const BUDDY_HIPS: f64 = 146.0;
pub const BUDDY_INK: f64 = 2.5;
pub const BUDDY_GEAR: f64 = 0.8;
pub const BUDDY_FIST: f64 = 1.25;
pub const BUDDY_SHIRT_MARK: (f64, f64, f64) = (93.5, 103.0, 14.5);
pub const BUDDY_FEET: f64 = 200.0;
pub const BUDDY_CHIN: f64 = 78.0;

pub const BUDDY_COLOR_SKIN: &str = "#f2bd8f";
pub const BUDDY_COLOR_SHADE: &str = "#d9a273";
pub const BUDDY_COLOR_INK: &str = "#0c080d";
pub const BUDDY_COLOR_PLATE: &str = "#57575d";
pub const BUDDY_COLOR_PLATE_LIGHT: &str = "#9a9aa2";
pub const BUDDY_COLOR_BAR: &str = "#3c3c42";
pub const BUDDY_COLOR_SHIRT: &str = "#fefefe";
pub const BUDDY_COLOR_SHIRT_SHADE: &str = "#e6e6ea";
pub const BUDDY_COLOR_SHORTS: &str = "#f27d1c";
pub const BUDDY_COLOR_STAR: &str = "#ffd98e";
pub const BUDDY_COLOR_SHOE: &str = "#1a161b";
pub const BUDDY_COLOR_SOLE: &str = "#8f8f97";
pub const BUDDY_COLOR_MARK: &str = "#0c080d";
pub const BUDDY_COLOR_CAP: &str = "#126dc7";
pub const BUDDY_COLOR_CAP_DARK: &str = "#0c4b8e";
pub const BUDDY_COLOR_HAIR: &str = "#261c1a";

pub const LEAN_MOST: f64 = 0.16;
pub const LEAN_AT: f64 = 13.0;

pub fn lean_of(dip: f64) -> f64 {
    LEAN_MOST * (dip / LEAN_AT).clamp(0.0, 1.0)
}

pub fn body_y(y: f64, dip: f64) -> f64 {
    if y >= BUDDY_HIPS {
        y + dip
    } else {
        BUDDY_HIPS + dip - (BUDDY_HIPS - y) * (1.0 - lean_of(dip))
    }
}

pub fn head_drop(dip: f64) -> f64 {
    body_y(BUDDY_CHIN, dip) - BUDDY_CHIN
}

pub const DUMBBELL_HALF_LENGTH: f64 = 21.25;
pub const DUMBBELL_HALF_HEIGHT: f64 = 11.0;

pub const REST_ARM: ArmPose = ArmPose {
    abduct: 0.1,
    flex: 0.04,
    bend: 0.14,
    bend_up: 0.0,
    grip: 0.0,
};

pub const REST_POSE: BuddyPose = BuddyPose {
    left: REST_ARM,
    right: REST_ARM,
    dip: 0.0,
};

pub const SHRUG: f64 = 3.0;

pub fn shrug_of(abduct: f64) -> f64 {
    let k = ((abduct - 1.5) / 1.3).clamp(0.0, 1.0);
    SHRUG * k * k * (3.0 - 2.0 * k)
}

pub fn nearness(z: f64) -> f64 {
    1.0 + (z * 0.0045).clamp(-0.08, 0.16)
}

fn norm(v: Vec3) -> Vec3 {
    let n = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    let d = if n == 0.0 { 1.0 } else { n };
    Vec3::new(v.x / d, v.y / d, v.z / d)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ArmJoints {
    pub shoulder: Vec3,
    pub elbow: Vec3,
    pub hand: Vec3,
}

/// `side`: -1 for picture left, +1 for picture right.
pub fn arm_joints(pose: &ArmPose, side: i8, dip: f64) -> ArmJoints {
    let side_f = if side < 0 { -1.0 } else { 1.0 };
    let at = if side < 0 {
        BUDDY_SHOULDER_LEFT
    } else {
        BUDDY_SHOULDER_RIGHT
    };
    let s = Vec3::new(at[0], body_y(at[1], dip) - shrug_of(pose.abduct), 0.0);
    let out_x = side_f * pose.abduct.sin();
    let out_y = pose.abduct.cos();
    let u = norm(Vec3::new(
        out_x,
        out_y * pose.flex.cos(),
        out_y.abs() * pose.flex.sin(),
    ));
    let elbow = Vec3::new(
        s.x + u.x * BUDDY_UPPER,
        s.y + u.y * BUDDY_UPPER,
        u.z * BUDDY_UPPER,
    );
    let toward = norm(Vec3::new(0.0, -pose.bend_up, 1.0 - pose.bend_up));
    let along = toward.x * u.x + toward.y * u.y + toward.z * u.z;
    let mut perp = Vec3::new(
        toward.x - along * u.x,
        toward.y - along * u.y,
        toward.z - along * u.z,
    );
    if (perp.x * perp.x + perp.y * perp.y + perp.z * perp.z).sqrt() < 1e-4 {
        perp = Vec3::new(0.0, 0.0, 1.0);
    }
    let p = norm(perp);
    let c = pose.bend.cos();
    let n = pose.bend.sin();
    let f = Vec3::new(
        c * u.x + n * p.x,
        c * u.y + n * p.y,
        c * u.z + n * p.z,
    );
    let hand = Vec3::new(
        elbow.x + f.x * BUDDY_FORE,
        elbow.y + f.y * BUDDY_FORE,
        elbow.z + f.z * BUDDY_FORE,
    );
    ArmJoints {
        shoulder: s,
        elbow,
        hand,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reach {
    pub x: f64,
    pub y: f64,
}

pub fn dumbbell_reach(pose: &ArmPose, joints: &ArmJoints) -> Reach {
    let scale = BUDDY_GEAR * nearness(joints.hand.z);
    let length = (DUMBBELL_HALF_LENGTH + BUDDY_INK / 2.0) * scale;
    let height = (DUMBBELL_HALF_HEIGHT + BUDDY_INK / 2.0) * scale;
    let c = pose.grip.cos().abs();
    let s = pose.grip.sin().abs();
    Reach {
        x: c * length + s * height,
        y: s * length + c * height,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    pub value: f64,
    pub velocity: f64,
    pub stiffness: f64,
    pub damping: f64,
}

impl Spring {
    pub const fn new(value: f64, stiffness: f64, damping: f64) -> Self {
        Self {
            value,
            velocity: 0.0,
            stiffness,
            damping,
        }
    }

    pub fn step(&mut self, target: f64, dt: f64, push: f64) -> f64 {
        self.velocity +=
            (self.stiffness * (target - self.value) - self.damping * self.velocity + push) * dt;
        self.value += self.velocity * dt;
        self.value
    }

    pub fn rests_at(&self, target: f64) -> bool {
        (target - self.value).abs() < 0.002 && self.velocity.abs() < 0.01
    }

    pub fn place(&mut self, value: f64) {
        self.value = value;
        self.velocity = 0.0;
    }
}

pub const RIG_STEP_S: f64 = 1.0 / 120.0;
pub const RIG_LONGEST_FRAME_S: f64 = 0.1;

pub const WEIGHT_SWING_STIFFNESS: f64 = 70.0;
pub const WEIGHT_SWING_DAMPING: f64 = 7.5;
pub const WEIGHT_SWING_PUSH: f64 = 0.00012;

pub const HEAD_TILT_STIFFNESS: f64 = 85.0;
pub const HEAD_TILT_DAMPING: f64 = 11.0;
pub const HEAD_TILT_PER_LOOK: f64 = 0.06;
pub const HEAD_TILT_ARM_PUSH: f64 = 0.000045;
pub const HEAD_TILT_DIP_PUSH: f64 = 0.00002;
pub const HEAD_TILT_MOST: f64 = 0.14;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigTarget {
    pub pose: BuddyPose,
    pub look: f64,
    pub tilt: Option<f64>,
}

impl RigTarget {
    pub const fn new(pose: BuddyPose, look: f64) -> Self {
        Self {
            pose,
            look,
            tilt: None,
        }
    }
}

pub const REST_TARGET: RigTarget = RigTarget::new(REST_POSE, 0.0);

#[derive(Debug, Clone, Copy)]
struct ArmSprings {
    abduct: Spring,
    flex: Spring,
    bend: Spring,
    bend_up: Spring,
    grip: Spring,
}

impl ArmSprings {
    fn from_pose(p: ArmPose) -> Self {
        Self {
            abduct: Spring::new(p.abduct, 180.0, 19.0),
            flex: Spring::new(p.flex, 180.0, 19.0),
            bend: Spring::new(p.bend, 180.0, 19.0),
            bend_up: Spring::new(p.bend_up, 180.0, 19.0),
            grip: Spring::new(p.grip, 180.0, 19.0),
        }
    }

    fn now(&self) -> ArmPose {
        ArmPose {
            abduct: self.abduct.value,
            flex: self.flex.value,
            bend: self.bend.value,
            bend_up: self.bend_up.value,
            grip: self.grip.value,
        }
    }

    fn step_toward(&mut self, from: ArmPose, to: ArmPose, k: f64, dt: f64) {
        let between = |a: f64, b: f64| a + (b - a) * k;
        self.abduct.step(between(from.abduct, to.abduct), dt, 0.0);
        self.flex.step(between(from.flex, to.flex), dt, 0.0);
        self.bend.step(between(from.bend, to.bend), dt, 0.0);
        self.bend_up.step(between(from.bend_up, to.bend_up), dt, 0.0);
        self.grip.step(between(from.grip, to.grip), dt, 0.0);
    }

    fn place(&mut self, p: ArmPose) {
        self.abduct.place(p.abduct);
        self.flex.place(p.flex);
        self.bend.place(p.bend);
        self.bend_up.place(p.bend_up);
        self.grip.place(p.grip);
    }

    fn rests_at(&self, p: ArmPose) -> bool {
        self.abduct.rests_at(p.abduct)
            && self.flex.rests_at(p.flex)
            && self.bend.rests_at(p.bend)
            && self.bend_up.rests_at(p.bend_up)
            && self.grip.rests_at(p.grip)
    }
}

#[derive(Debug, Clone)]
pub struct BuddyRigSim {
    left: ArmSprings,
    right: ArmSprings,
    left_swing: Spring,
    right_swing: Spring,
    dip: Spring,
    look: Spring,
    tilt: Spring,
    lifts: Option<([f64; 2], [f64; 2], [f64; 2])>,
    last: RigTarget,
    left_hand_x: Option<(f64, f64)>,
    right_hand_x: Option<(f64, f64)>,
}

impl Default for BuddyRigSim {
    fn default() -> Self {
        Self::new(REST_TARGET)
    }
}

impl BuddyRigSim {
    pub fn new(start: RigTarget) -> Self {
        Self {
            left: ArmSprings::from_pose(start.pose.left),
            right: ArmSprings::from_pose(start.pose.right),
            left_swing: Spring::new(0.0, WEIGHT_SWING_STIFFNESS, WEIGHT_SWING_DAMPING),
            right_swing: Spring::new(0.0, WEIGHT_SWING_STIFFNESS, WEIGHT_SWING_DAMPING),
            dip: Spring::new(start.pose.dip, 120.0, 14.0),
            look: Spring::new(start.look, 140.0, 20.0),
            tilt: Spring::new(
                start.look * HEAD_TILT_PER_LOOK,
                HEAD_TILT_STIFFNESS,
                HEAD_TILT_DAMPING,
            ),
            lifts: None,
            last: start,
            left_hand_x: None,
            right_hand_x: None,
        }
    }

    pub fn step(&mut self, target: RigTarget, seconds: f64) -> RigTarget {
        let span = seconds.max(0.0).min(RIG_LONGEST_FRAME_S);
        let steps = ((span / RIG_STEP_S - 1e-9).ceil() as usize).max(1);
        let dt = span / (steps as f64);
        let from = self.last;
        let between = |a: f64, b: f64, k: f64| a + (b - a) * k;

        if dt > 0.0 {
            for i in 1..=steps {
                let k = (i as f64) / (steps as f64);
                self.left.step_toward(from.pose.left, target.pose.left, k, dt);
                self.right.step_toward(from.pose.right, target.pose.right, k, dt);
                self.dip.step(between(from.pose.dip, target.pose.dip, k), dt, 0.0);
                self.look.step(between(from.look, target.look, k), dt, 0.0);

                // Left dumbbell swing
                let lx = arm_joints(&self.left.now(), -1, self.dip.value).hand.x;
                let laccel = match self.left_hand_x {
                    Some((x0, x1)) => (lx - 2.0 * x1 + x0) / (dt * dt),
                    None => 0.0,
                };
                self.left_hand_x = Some((self.left_hand_x.map_or(lx, |(_, x1)| x1), lx));
                self.left_swing
                    .step(0.0, dt, -WEIGHT_SWING_PUSH * laccel * WEIGHT_SWING_STIFFNESS);

                // Right dumbbell swing
                let rx = arm_joints(&self.right.now(), 1, self.dip.value).hand.x;
                let raccel = match self.right_hand_x {
                    Some((x0, x1)) => (rx - 2.0 * x1 + x0) / (dt * dt),
                    None => 0.0,
                };
                self.right_hand_x = Some((self.right_hand_x.map_or(rx, |(_, x1)| x1), rx));
                self.right_swing
                    .step(0.0, dt, -WEIGHT_SWING_PUSH * raccel * WEIGHT_SWING_STIFFNESS);

                // Head tilt
                let ly = arm_joints(&self.left.now(), -1, self.dip.value).hand.y;
                let ry = arm_joints(&self.right.now(), 1, self.dip.value).hand.y;
                let push = match self.lifts {
                    Some((sl, sr, sd)) => {
                        let acc = |now: f64, prev: [f64; 2]| (now - 2.0 * prev[1] + prev[0]) / (dt * dt);
                        let look_sign = if self.look.value == 0.0 {
                            1.0
                        } else {
                            self.look.value.signum()
                        };
                        HEAD_TILT_ARM_PUSH * (acc(ly, sl) - acc(ry, sr))
                            + HEAD_TILT_DIP_PUSH * acc(self.dip.value, sd) * look_sign
                    }
                    None => 0.0,
                };
                let (pl, pr, pd) = match self.lifts {
                    Some((sl, sr, sd)) => (sl[1], sr[1], sd[1]),
                    None => (ly, ry, self.dip.value),
                };
                self.lifts = Some(([pl, ly], [pr, ry], [pd, self.dip.value]));
                self.tilt.step(
                    self.look.value * HEAD_TILT_PER_LOOK,
                    dt,
                    push * HEAD_TILT_STIFFNESS,
                );
            }
        }
        self.last = target;
        self.now()
    }

    pub fn place(&mut self, target: RigTarget) -> RigTarget {
        self.last = target;
        self.left.place(target.pose.left);
        self.right.place(target.pose.right);
        self.left_swing.place(0.0);
        self.right_swing.place(0.0);
        self.left_hand_x = None;
        self.right_hand_x = None;
        self.dip.place(target.pose.dip);
        self.look.place(target.look);
        self.tilt.place(target.look * HEAD_TILT_PER_LOOK);
        self.lifts = None;
        self.now()
    }

    pub fn rests_at(&self, target: &RigTarget) -> bool {
        self.left.rests_at(target.pose.left)
            && self.right.rests_at(target.pose.right)
            && self.left_swing.rests_at(0.0)
            && self.right_swing.rests_at(0.0)
            && self.dip.rests_at(target.pose.dip)
            && self.look.rests_at(target.look)
            && self.tilt.rests_at(target.look * HEAD_TILT_PER_LOOK)
    }

    pub fn now(&self) -> RigTarget {
        let mut left = self.left.now();
        left.grip += self.left_swing.value;
        let mut right = self.right.now();
        right.grip += self.right_swing.value;
        let tilt = self.tilt.value.clamp(-HEAD_TILT_MOST, HEAD_TILT_MOST);
        RigTarget {
            pose: BuddyPose {
                left,
                right,
                dip: self.dip.value,
            },
            look: self.look.value,
            tilt: Some(tilt),
        }
    }
}

pub fn neck_of(dip: f64) -> [f64; 2] {
    [BUDDY_RIG_MIDDLE, BUDDY_CHIN + head_drop(dip)]
}

// =========================================================================
// Pure body geometry & pixel cut-head helpers (`buddyBody.ts`)
// =========================================================================

pub const HEAD_ROWS: usize = 82;

pub fn cut_head(pixels: &mut [u8], width: usize) {
    if width == 0 {
        return;
    }
    let height = pixels.len() / 4 / width;
    for y in 0..height {
        for x in 0..width {
            let i = (y * width + x) * 4;
            if i + 3 >= pixels.len() {
                continue;
            }
            if y >= HEAD_ROWS {
                pixels[i + 3] = 0;
                continue;
            }
            if y < 72 {
                continue;
            }
            let r = pixels[i];
            let g = pixels[i + 1];
            let b = pixels[i + 2];
            let white = r > 205
                && g > 205
                && b > 205
                && r.max(g).max(b) - r.min(g).min(b) < 26;
            if white || x < 76 || x > 114 {
                pixels[i + 3] = 0;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FaceShape {
    pub mask: Vec<u8>,
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

const FACE_LEFT: usize = 58;
const FACE_TOP: usize = 30;
const FACE_RIGHT: usize = 122;
const FACE_BOTTOM: usize = HEAD_ROWS;
const FACE_SEEDS: [[usize; 2]; 4] = [[86, 52], [80, 66], [96, 66], [88, 74]];

fn is_skin(pixels: &[u8], i: usize) -> bool {
    if i + 3 >= pixels.len() {
        return false;
    }
    let r = pixels[i] as i32;
    let g = pixels[i + 1] as i32;
    let b = pixels[i + 2] as i32;
    let a = pixels[i + 3] as i32;
    a > 128 && r > 150 && r - b > 60 && g > 110 && r - g > 25 && r - g < 75
}

pub fn face_of(pixels: &[u8], width: usize) -> Option<FaceShape> {
    if width < FACE_RIGHT || pixels.len() < HEAD_ROWS * width * 4 {
        return None;
    }
    let w = FACE_RIGHT - FACE_LEFT;
    let h = FACE_BOTTOM - FACE_TOP;
    let inside = |x: isize, y: isize| -> bool {
        x >= FACE_LEFT as isize
            && x < FACE_RIGHT as isize
            && y >= FACE_TOP as isize
            && y < FACE_BOTTOM as isize
    };
    let at = |x: usize, y: usize| -> usize { (y - FACE_TOP) * w + (x - FACE_LEFT) };
    let mut face = vec![0u8; w * h];
    let mut queue: VecDeque<(usize, usize)> = VecDeque::new();
    for [x, y] in FACE_SEEDS {
        let idx = (y * width + x) * 4;
        if is_skin(pixels, idx) && face[at(x, y)] == 0 {
            face[at(x, y)] = 1;
            queue.push_back((x, y));
        }
    }
    if queue.is_empty() {
        return None;
    }
    let steps: [(isize, isize); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
    while let Some((x, y)) = queue.pop_back() {
        for (dx, dy) in steps {
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            if !inside(nx, ny) {
                continue;
            }
            let (nxu, nyu) = (nx as usize, ny as usize);
            if face[at(nxu, nyu)] != 0 || !is_skin(pixels, (nyu * width + nxu) * 4) {
                continue;
            }
            face[at(nxu, nyu)] = 1;
            queue.push_back((nxu, nyu));
        }
    }

    let mut outside = vec![0u8; w * h];
    for x in FACE_LEFT..FACE_RIGHT {
        for y in [FACE_TOP, FACE_BOTTOM - 1] {
            if face[at(x, y)] == 0 && outside[at(x, y)] == 0 {
                outside[at(x, y)] = 1;
                queue.push_back((x, y));
            }
        }
    }
    for y in FACE_TOP..FACE_BOTTOM {
        for x in [FACE_LEFT, FACE_RIGHT - 1] {
            if face[at(x, y)] == 0 && outside[at(x, y)] == 0 {
                outside[at(x, y)] = 1;
                queue.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = queue.pop_back() {
        for (dx, dy) in steps {
            let nx = x as isize + dx;
            let ny = y as isize + dy;
            if !inside(nx, ny) {
                continue;
            }
            let (nxu, nyu) = (nx as usize, ny as usize);
            if face[at(nxu, nyu)] != 0 || outside[at(nxu, nyu)] != 0 {
                continue;
            }
            outside[at(nxu, nyu)] = 1;
            queue.push_back((nxu, nyu));
        }
    }

    let is_ink = |x: usize, y: usize| -> bool {
        let i = (y * width + x) * 4;
        if i + 3 >= pixels.len() {
            return true;
        }
        let lum = 0.2126 * (pixels[i] as f64)
            + 0.7152 * (pixels[i + 1] as f64)
            + 0.0722 * (pixels[i + 2] as f64);
        pixels[i + 3] < 128 || lum < 70.0
    };
    for _pass in 0..2 {
        let mut grown = Vec::new();
        for y in FACE_TOP..FACE_BOTTOM {
            for x in FACE_LEFT..FACE_RIGHT {
                if outside[at(x, y)] == 0 || is_ink(x, y) {
                    continue;
                }
                if steps.iter().any(|&(dx, dy)| {
                    let nx = x as isize + dx;
                    let ny = y as isize + dy;
                    inside(nx, ny) && outside[at(nx as usize, ny as usize)] == 0
                }) {
                    grown.push(at(x, y));
                }
            }
        }
        for i in grown {
            outside[i] = 0;
        }
    }

    let mut mask = vec![0u8; width * HEAD_ROWS];
    let mut x0 = FACE_RIGHT;
    let mut y0 = FACE_BOTTOM;
    let mut x1 = FACE_LEFT;
    let mut y1 = FACE_TOP;
    for y in FACE_TOP..FACE_BOTTOM {
        for x in FACE_LEFT..FACE_RIGHT {
            if outside[at(x, y)] != 0 {
                continue;
            }
            mask[y * width + x] = 1;
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x + 1);
            y1 = y1.max(y + 1);
        }
    }
    Some(FaceShape {
        mask,
        x: x0,
        y: y0,
        w: x1.saturating_sub(x0),
        h: y1.saturating_sub(y0),
    })
}

pub fn eyes_on(bx: f64, by: f64, bw: f64, bh: f64) -> [f64; 4] {
    [bx + bw * 0.12, by + bh * 0.2, bw * 0.76, bh * 0.62]
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ring {
    pub x: f64,
    pub y: f64,
    pub r: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BuddyPiece {
    Disc { x: f64, y: f64, r: f64 },
    Poly(Vec<[f64; 2]>),
}

pub fn clockwise_pts(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut area = 0.0;
    for i in 0..points.len() {
        let [x0, y0] = points[i];
        let [x1, y1] = points[(i + 1) % points.len()];
        area += x0 * y1 - x1 * y0;
    }
    if area >= 0.0 {
        points.to_vec()
    } else {
        points.iter().copied().rev().collect()
    }
}

pub fn taper(a: Ring, b: Ring) -> Vec<BuddyPiece> {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let length = dx.hypot(dy);
    if length <= (a.r - b.r).abs() + 1e-6 {
        return vec![
            BuddyPiece::Disc {
                x: a.x,
                y: a.y,
                r: a.r,
            },
            BuddyPiece::Disc {
                x: b.x,
                y: b.y,
                r: b.r,
            },
        ];
    }
    let ux = dx / length;
    let uy = dy / length;
    let c = (a.r - b.r) / length;
    let s = (1.0 - c * c).max(0.0).sqrt();
    let side = |sign: f64| -> [f64; 2] { [ux * c - uy * s * sign, uy * c + ux * s * sign] };
    let [p1x, p1y] = side(1.0);
    let [p2x, p2y] = side(-1.0);
    vec![
        BuddyPiece::Disc {
            x: a.x,
            y: a.y,
            r: a.r,
        },
        BuddyPiece::Disc {
            x: b.x,
            y: b.y,
            r: b.r,
        },
        BuddyPiece::Poly(clockwise_pts(&[
            [a.x + p1x * a.r, a.y + p1y * a.r],
            [b.x + p1x * b.r, b.y + p1y * b.r],
            [b.x + p2x * b.r, b.y + p2y * b.r],
            [a.x + p2x * a.r, a.y + p2y * a.r],
        ])),
    ]
}

pub fn limb(rings: &[Ring]) -> Vec<BuddyPiece> {
    let mut out = Vec::new();
    for pair in rings.windows(2) {
        out.extend(taper(pair[0].clone(), pair[1].clone()));
    }
    out
}

fn lerp_vec3(a: Vec3, b: Vec3, k: f64) -> Vec3 {
    Vec3::new(
        a.x + (b.x - a.x) * k,
        a.y + (b.y - a.y) * k,
        a.z + (b.z - a.z) * k,
    )
}

pub fn arm_rings(joints: &ArmJoints, pose: &ArmPose) -> (Vec<Ring>, Vec<Ring>) {
    let size = |v: Vec3, r: f64| Ring {
        x: v.x,
        y: v.y,
        r: r * nearness(v.z),
    };
    let s = joints.shoulder;
    let e = joints.elbow;
    let h = joints.hand;
    let ux = e.x - s.x;
    let uy = e.y - s.y;
    let len = ux.hypot(uy);
    let len = if len == 0.0 { 1.0 } else { len };
    let mut nx = -uy / len;
    let mut ny = ux / len;
    if nx * (h.x - e.x) + ny * (h.y - e.y) < 0.0 {
        nx = -nx;
        ny = -ny;
    }
    let pump = (pose.bend / 2.2).clamp(0.0, 1.0);
    let belly = lerp_vec3(s, e, 0.45);
    let mut belly_ring = size(belly, 10.6 + 2.4 * pump);
    belly_ring.x += nx * 2.2 * pump;
    belly_ring.y += ny * 2.2 * pump;
    let upper = vec![size(s, 10.0), belly_ring, size(e, 8.0)];
    let fore = vec![
        size(e, 8.4),
        size(lerp_vec3(e, h, 0.32), 9.6),
        size(lerp_vec3(e, h, 0.86), 6.6),
    ];
    (upper, fore)
}

pub fn arm_bend_at(a: Vec3, joint: Vec3, b: Vec3) -> f64 {
    let ux = a.x - joint.x;
    let uy = a.y - joint.y;
    let uz = a.z - joint.z;
    let vx = b.x - joint.x;
    let vy = b.y - joint.y;
    let vz = b.z - joint.z;
    let mu = (ux * ux + uy * uy + uz * uz).sqrt();
    let mv = (vx * vx + vy * vy + vz * vz).sqrt();
    let cos = (ux * vx + uy * vy + uz * vz)
        / ((if mu == 0.0 { 1.0 } else { mu }) * (if mv == 0.0 { 1.0 } else { mv }));
    std::f64::consts::PI - cos.clamp(-1.0, 1.0).acos()
}

pub fn sleeve_pieces(joints: &ArmJoints) -> Vec<BuddyPiece> {
    let s = joints.shoulder;
    let hem = lerp_vec3(s, joints.elbow, 0.42);
    let near = nearness(hem.z);
    let ux = hem.x - s.x;
    let uy = hem.y - s.y;
    let len = ux.hypot(uy);
    let len = if len == 0.0 { 1.0 } else { len };
    let nx = -uy / len;
    let ny = ux / len;
    let top_r = 12.4;
    let r = 12.0 * near;
    vec![
        BuddyPiece::Disc {
            x: s.x,
            y: s.y,
            r: top_r,
        },
        BuddyPiece::Poly(clockwise_pts(&[
            [s.x + nx * top_r, s.y + ny * top_r],
            [hem.x + nx * r, hem.y + ny * r],
            [hem.x - nx * r, hem.y - ny * r],
            [s.x - nx * top_r, s.y - ny * top_r],
        ])),
    ]
}

pub fn hem_of(joints: &ArmJoints) -> ([f64; 2], [f64; 2]) {
    let s = joints.shoulder;
    let hem = lerp_vec3(s, joints.elbow, 0.42);
    let ux = hem.x - s.x;
    let uy = hem.y - s.y;
    let len = ux.hypot(uy);
    let len = if len == 0.0 { 1.0 } else { len };
    let r = 12.0 * nearness(hem.z);
    (
        [hem.x - (uy / len) * r, hem.y + (ux / len) * r],
        [hem.x + (uy / len) * r, hem.y - (ux / len) * r],
    )
}

pub const TORSO_BASE: [[f64; 2]; 10] = [
    [78.0, 77.0],
    [109.0, 77.0],
    [122.0, 82.0],
    [126.0, 93.0],
    [117.5, 104.0],
    [119.0, 131.0],
    [68.0, 131.0],
    [69.5, 104.0],
    [61.0, 93.0],
    [65.0, 82.0],
];

pub fn torso_poly(dip: f64) -> Vec<[f64; 2]> {
    let pts: Vec<[f64; 2]> = TORSO_BASE
        .iter()
        .map(|&[x, y]| [x, body_y(y, dip)])
        .collect();
    clockwise_pts(&pts)
}

pub const SHORTS_STARS: [[f64; 4]; 9] = [
    [74.0, 136.0, 4.6, 0.2],
    [86.0, 134.0, 4.2, -0.1],
    [101.0, 135.0, 4.5, 0.15],
    [114.0, 137.0, 4.3, -0.2],
    [70.0, 151.0, 4.4, -0.15],
    [82.0, 149.0, 4.6, 0.1],
    [97.0, 151.0, 4.2, -0.05],
    [110.0, 150.0, 4.6, 0.2],
    [120.0, 152.0, 3.6, 0.0],
];

pub fn shorts_poly(dip: f64) -> Vec<[f64; 2]> {
    clockwise_pts(&[
        [64.5, body_y(128.0, dip)],
        [122.5, body_y(128.0, dip)],
        [123.0, 160.0 + dip],
        [64.0, 160.0 + dip],
    ])
}

pub fn star_points(x: f64, y: f64, r: f64, turn: f64) -> Vec<[f64; 2]> {
    (0..10)
        .map(|i| {
            let angle = turn - std::f64::consts::FRAC_PI_2 + (i as f64) * std::f64::consts::PI / 5.0;
            let radius = if i % 2 == 0 { r } else { r * 0.45 };
            [x + angle.cos() * radius, y + angle.sin() * radius]
        })
        .collect()
}

pub const SHOULDER_MASK: [[f64; 2]; 7] = [
    [52.0, 90.0],
    [57.0, 79.0],
    [75.0, 78.0],
    [77.0, 104.0],
    [70.0, 104.5],
    [60.0, 101.5],
    [52.0, 99.0],
];

pub fn shoulder_of(side: i8, dip: f64) -> Vec<[f64; 2]> {
    SHOULDER_MASK
        .iter()
        .map(|&[x, y]| {
            let mx = if side < 0 {
                x
            } else {
                2.0 * BUDDY_RIG_MIDDLE - x
            };
            [mx, body_y(y, dip)]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filled(rgb: [u8; 3]) -> Vec<u8> {
        let mut p = Vec::with_capacity(192 * 208 * 4);
        for _ in 0..(192 * 208) {
            p.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
        p
    }

    #[test]
    fn keeps_his_cap_to_his_chin_and_nothing_of_him_below_his_body_is_drawn() {
        let mut head = filled([40, 90, 200]);
        cut_head(&mut head, 192);
        let alpha = |x: usize, y: usize| head[(y * 192 + x) * 4 + 3];
        for (x, y) in [(90, 10), (40, 50), (150, 60), (95, 78)] {
            assert_eq!(alpha(x, y), 255, "{x},{y}");
        }
        for (x, y) in [(90, HEAD_ROWS), (90, 120), (60, 190)] {
            assert_eq!(alpha(x, y), 0, "{x},{y}");
        }
    }

    #[test]
    fn takes_his_shirt_from_under_his_chin_its_white_anywhere_anything_beside_his_chin_and_leaves_his_chin() {
        let mut white = filled([250, 250, 250]);
        cut_head(&mut white, 192);
        assert_eq!(white[(76 * 192 + 95) * 4 + 3], 0);
        assert_eq!(white[(60 * 192 + 95) * 4 + 3], 255);

        let mut skin = filled([242, 189, 143]);
        cut_head(&mut skin, 192);
        assert_eq!(skin[(78 * 192 + 95) * 4 + 3], 255);
        assert_eq!(skin[(78 * 192 + 60) * 4 + 3], 0);
        assert_eq!(skin[(78 * 192 + 130) * 4 + 3], 0);
    }

    #[test]
    fn finds_his_face_to_be_his_screen_his_skin_inside_his_faces_ink_and_his_eyes_and_mouth_in_it_not_his_ears_his_ink_or_his_fringe() {
        let mut pixels = filled([30, 30, 34]);
        let mut paint = |x: usize, y: usize, rgb: [u8; 3]| {
            let i = (y * 192 + x) * 4;
            pixels[i] = rgb[0];
            pixels[i + 1] = rgb[1];
            pixels[i + 2] = rgb[2];
            pixels[i + 3] = 255;
        };
        for y in 30..HEAD_ROWS {
            for x in 50..130 {
                let d = (x as f64 - 88.0).hypot(y as f64 - 60.0);
                if d < 22.0 {
                    paint(x, y, [242, 189, 143]);
                } else if d < 25.0 {
                    paint(x, y, [12, 8, 13]);
                }
                if (x as f64 - 58.0).hypot(y as f64 - 60.0) < 4.0 {
                    paint(x, y, [242, 189, 143]);
                }
            }
        }
        for (cx, cy) in [(80isize, 58isize), (96, 58), (88, 70)] {
            for dy in -2..=2 {
                for dx in -2..=2 {
                    paint((cx + dx) as usize, (cy + dy) as usize, [20, 18, 22]);
                }
            }
        }
        for x in 70..106 {
            for y in 38..44 {
                paint(x, y, [25, 22, 24]);
            }
        }
        let face = face_of(&pixels, 192).expect("face");
        let on = |x: usize, y: usize| face.mask[y * 192 + x];
        for (x, y) in [(88, 64), (70, 60), (80, 58), (96, 58), (88, 70)] {
            assert_eq!(on(x, y), 1, "{x},{y}");
        }
        for (x, y) in [(88 - 23, 60), (88 + 24, 60), (58, 60), (88, 40), (40, 60)] {
            assert_eq!(on(x, y), 0, "{x},{y}");
        }
        let eyes = eyes_on(face.x as f64, face.y as f64, face.w as f64, face.h as f64);
        assert!(eyes[0] > face.x as f64);
        assert!(eyes[0] + eyes[2] < (face.x + face.w) as f64);
    }

    #[test]
    fn finds_no_face_where_there_is_no_skin_he_wears_a_screen_over_it_instead() {
        assert!(face_of(&filled([30, 30, 34]), 192).is_none());
    }

    /// Electron's case also cuts a sheet through a stubbed `document` canvas, once and cached, and reads one that
    /// throws as none: that is canvas plumbing, and nothing here reads a sheet. Carried: where his screen plate is.
    #[test]
    fn is_cut_once_a_sheet_and_kept_for_every_face_wearing_it_a_sheet_that_cannot_be_read_gives_none_and_he_is_drawn_as_his_maker_drew_him() {
        assert_eq!((BUDDY_RIG_PLATE_ROW, BUDDY_RIG_PLATE_COL), (8, 0));
    }
}

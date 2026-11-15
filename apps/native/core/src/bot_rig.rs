//! Pure bot animation rig, state choreography, eye/screen transitions, and frame pacing.
//!
//! Ported 1:1 from `bot-avatars@0.2.2` (`BotAvatarSim`, `restPose`), `Bot.tsx`,
//! `TeammateBot.tsx`, `faceState.ts`, `faceLife.ts`, `glances.ts`, and `frameBeat.ts`.
//! Kept free of any UI toolkit or rendering backend dependencies.

use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;

use crate::bot_spec::{mix_phosphor_css, BotFace, Phosphor};

const DEG: f64 = PI / 180.0;
const TAU: f64 = PI * 2.0;

pub const BOT_AVATAR_OVERSCAN: f64 = 1.5;
pub const BOT_AVATAR_RISE: f64 = 0.1;
pub const BOT_FRAMES_PER_SECOND: f64 = 30.0;
pub const SETTLED_FRAMES: u32 = 10;
pub const PLUSH_SHORT_BELOW: f64 = 40.0;

pub const GLANCE_HOLD_MS: u64 = 1_500;
pub const HOP_MS: u64 = 1_500;
pub const DONE_HOP_MS: u64 = 1_800;
pub const RECEIVED_GLANCE_MS: u64 = 2_800;

pub const THINKING_HUSH: f64 = 0.7;
pub const THINKING_CYCLE_S: f64 = 4.2;
pub const LOOK_ABOUT_S: f64 = 2.1;
pub const DOTS_REST_Y: f64 = -0.5;
pub const HEAD_GLANCE_S: f64 = 0.6;

pub const SCREEN_SQUASH: f64 = 0.45;
pub const SCREEN_YAW_LIMIT: f64 = 0.75;
pub const CURIOUS_TILT: f64 = 0.21;
pub const PERK_LIFT: f64 = 0.12;
pub const BLINK_GAP_S: f64 = 2.2;

pub const LAG_MOST_STEP: f64 = 1.0 / 30.0;
const LAG_SPRING: f64 = 260.0;
const LAG_DAMPING: f64 = 22.0;
const LAG_GAIN: f64 = 0.35;
const LAG_MOST: f64 = 3.0;

pub const FLASH_S: f64 = 1.2;
const FLASH_HOLD_S: f64 = 0.3;
pub const SWAP_S: f64 = 0.26;
pub const POWER_ON_S: f64 = 0.9;
pub const CRT_FLICK_S: f64 = 0.4;
const POWER_LINE: f64 = 0.33;
const POWER_FLASH: f64 = 0.67;

pub const MARKED_FACE_MIN: f32 = 24.0;
pub const PRESENCE_MIN: f32 = 28.0;
pub const NOTICE_DWELL_MS: u64 = 220;

pub const LISTEN_HOLD_MS: u64 = 4_000;
pub const CRT_ON_KEYS: [&str; 3] = ["done", "blocked", "waiting"];
pub const CRT_FLICK_SHARE: f64 = 1.0 / 3.0;

pub const SCREEN_RESTING_EYES: [&str; 2] = ["|", "|"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotAvatarState {
    Default,
    Working,
    Sleeping,
}

impl BotAvatarState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Working => "working",
            Self::Sleeping => "sleeping",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "working" => Self::Working,
            "sleeping" => Self::Sleeping,
            _ => Self::Default,
        }
    }

    fn index(self) -> usize {
        match self {
            Self::Default => 0,
            Self::Working => 1,
            Self::Sleeping => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BotAvatarPose {
    pub yaw: f64,
    pub pitch: f64,
    pub roll: f64,
    pub x: f64,
    pub y: f64,
    pub sx: f64,
    pub sy: f64,
    pub eye_open: f64,
    pub blink_l: f64,
    pub blink_r: f64,
    pub look_x: f64,
    pub look_y: f64,
    pub breath: f64,
    pub laugh: f64,
    pub whirl: f64,
    pub whirl_angle: f64,
    pub w: [f64; 3],
}

pub fn rest_pose(state: BotAvatarState) -> BotAvatarPose {
    let (pitch, roll, y, look_y, w) = match state {
        BotAvatarState::Default => (0.0, 0.0, 0.0, 0.0, [1.0, 0.0, 0.0]),
        BotAvatarState::Working => (5.0 * DEG, 0.0, 0.0, 0.0, [0.0, 1.0, 0.0]),
        BotAvatarState::Sleeping => (-16.0 * DEG, 6.0 * DEG, 3.0, 1.0, [0.0, 0.0, 1.0]),
    };
    BotAvatarPose {
        yaw: 0.0,
        pitch,
        roll,
        x: 0.0,
        y,
        sx: 1.0,
        sy: 1.0,
        eye_open: 1.0,
        blink_l: 0.0,
        blink_r: 0.0,
        look_x: 0.0,
        look_y,
        breath: 0.0,
        laugh: 0.0,
        whirl: 0.0,
        whirl_angle: 0.0,
        w,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EaseKind {
    Sharp,
    Pulse,
    Soft,
    Bouncy,
}

impl EaseKind {
    fn peak_t(self) -> f64 {
        match self {
            Self::Sharp => 0.0,
            Self::Pulse => 2.0 / 7.0,
            Self::Soft => 0.5,
            Self::Bouncy => 0.144,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JumpSpec {
    pub height: f64,
    pub time: f64,
    pub stretch: f64,
    pub spin: i32,
    pub lean: f64,
    pub every: f64,
    pub land: f64,
    pub squash: f64,
    pub squash_time: f64,
    pub squash_ease: EaseKind,
    pub ground_time: f64,
    pub ground_ease: EaseKind,
    pub rise_time: f64,
    pub rise_ease: EaseKind,
    pub click_squash_time: f64,
}

impl Default for JumpSpec {
    fn default() -> Self {
        Self {
            height: 26.0,
            time: 0.68,
            stretch: 1.0,
            spin: 1,
            lean: 6.0,
            every: 8.0,
            land: 0.0,
            squash: 1.15,
            squash_time: 0.37,
            squash_ease: EaseKind::Pulse,
            ground_time: 0.11,
            ground_ease: EaseKind::Pulse,
            rise_time: 0.33,
            rise_ease: EaseKind::Pulse,
            click_squash_time: 0.24,
        }
    }
}

pub const GLAD_HOP: JumpSpec = JumpSpec {
    height: 26.0,
    time: 0.68,
    stretch: 1.0,
    spin: 0,
    lean: 6.0,
    every: 8.0,
    land: 0.0,
    squash: 1.15,
    squash_time: 0.37,
    squash_ease: EaseKind::Bouncy,
    ground_time: 0.11,
    ground_ease: EaseKind::Pulse,
    rise_time: 0.33,
    rise_ease: EaseKind::Bouncy,
    click_squash_time: 0.24,
};

#[derive(Debug, Clone)]
struct RigRng {
    state: u32,
}

impl RigRng {
    fn new(seed: f64) -> Self {
        let base = ((seed * 1_000_000.0).floor() as i64 as u32).wrapping_add(1);
        let init = base.wrapping_mul(2654435761);
        Self {
            state: if init == 0 { 1 } else { init },
        }
    }

    fn next(&mut self) -> f64 {
        self.state = self.state.wrapping_add(1831565813);
        let mut o = self.state;
        o = (o ^ (o >> 15)).wrapping_mul(o | 1);
        o ^= o.wrapping_add((o ^ (o >> 7)).wrapping_mul(o | 61));
        ((o ^ (o >> 14)) as f64) / 4294967296.0
    }
}

fn damp(a: f64, b: f64, rate: f64, dt: f64) -> f64 {
    a + (b - a) * (1.0 - (-rate * dt).exp())
}

fn smoothstep3(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

fn cosine_ease(t: f64) -> f64 {
    0.5 - 0.5 * (PI * t).cos()
}

fn rise_curve(t: f64, kind: EaseKind) -> f64 {
    if t <= 0.0 {
        return 1.0;
    }
    if t >= 1.0 {
        return 0.0;
    }
    match kind {
        EaseKind::Sharp => (1.0 - t) * (1.0 - t),
        EaseKind::Soft => 0.5 + 0.5 * (PI * t).cos(),
        EaseKind::Bouncy => (-3.2 * t).exp() * (5.4 * t).cos() - t * t * t * 0.026,
        EaseKind::Pulse => {
            let o = 4.2 * t;
            (1.0 + o) * (-o).exp() - t * t * t * 0.078
        }
    }
}

fn squash_curve(t: f64, kind: EaseKind) -> f64 {
    if t <= 0.0 || t >= 1.0 {
        return 0.0;
    }
    let o = match kind {
        EaseKind::Sharp => (1.0 - t) * (1.0 - t),
        EaseKind::Soft => (PI * t).sin().powi(2),
        EaseKind::Bouncy => (-3.15 * t).exp() * (8.43 * t).sin() / 0.596,
        EaseKind::Pulse => {
            let a = 7.0 * t;
            a * a * (2.0 - a).exp() / 4.0
        }
    };
    let s = if t > 0.85 {
        1.0 - (t - 0.85) / 0.15
    } else {
        1.0
    };
    o * s * s * (3.0 - 2.0 * s)
}

fn ground_curve(t: f64, kind: EaseKind) -> f64 {
    1.0 + 0.25 * squash_curve(t, kind)
}

fn edge_bump(t: f64) -> f64 {
    let m = t.abs().min((t - 1.0).abs()) / 0.11;
    (-m * m).exp()
}

#[derive(Debug, Clone)]
struct DriftChannel {
    amp: f64,
    hold_min: f64,
    hold_max: f64,
    rate: f64,
    spring: bool,
    value: f64,
    vel: f64,
    target: f64,
    next: f64,
}

impl DriftChannel {
    fn new(amp: f64, hold_min: f64, hold_max: f64, rate: f64, spring: bool) -> Self {
        Self {
            amp,
            hold_min,
            hold_max,
            rate,
            spring,
            value: 0.0,
            vel: 0.0,
            target: 0.0,
            next: 0.0,
        }
    }

    fn update(&mut self, t: f64, dt: f64, rng: &mut RigRng) {
        if t >= self.next {
            self.target = (rng.next() * 2.0 - 1.0) * self.amp;
            self.next = t + self.hold_min + rng.next() * (self.hold_max - self.hold_min);
        }
        if self.spring {
            let s = self.rate * 1.6;
            let a = 0.9;
            self.vel += (s * s * (self.target - self.value) - 2.0 * a * s * self.vel) * dt;
            self.value += self.vel * dt;
        } else {
            self.value = damp(self.value, self.target, self.rate, dt);
        }
    }

    fn aim(&mut self, target: f64) {
        self.target = target;
        self.next = f64::INFINITY;
    }

    fn set(&mut self, amp: f64, hold_min: f64, hold_max: f64, rate: f64) {
        self.amp = amp;
        self.hold_min = hold_min;
        self.hold_max = hold_max;
        self.rate = rate;
        self.next = 0.0;
    }
}

#[derive(Debug, Clone)]
struct Pulse {
    duration: f64,
    p: f64,
}

impl Pulse {
    fn new(duration: f64) -> Self {
        Self { duration, p: -1.0 }
    }

    fn fire(&mut self) {
        self.p = 0.0;
    }

    fn active(&self) -> bool {
        self.p >= 0.0
    }

    fn update(&mut self, dt: f64) {
        if self.p < 0.0 {
            return;
        }
        self.p += dt / self.duration;
        if self.p >= 1.0 {
            self.p = -1.0;
        }
    }
}

const IA: f64 = 35.0 * DEG;
const LA: f64 = 14.0 * DEG;
const HA: f64 = 3.2 * DEG;
const FA: f64 = 2.6;
const XC: f64 = 4.4;
const HOP_PERIOD_S: f64 = 0.68;
const FLIP_HEIGHT: f64 = 26.0;
const GC_RATIO: f64 = 0.2;

fn crouch_s(j: &JumpSpec, poked: bool) -> f64 {
    if poked {
        j.click_squash_time
    } else {
        GC_RATIO * j.time
    }
}

fn flip_duration(j: &JumpSpec, poked: bool) -> f64 {
    crouch_s(j, poked)
        + j.time
        + j.squash_ease.peak_t() * if poked { j.click_squash_time } else { j.squash_time }
        + j.ground_time.max(0.0)
        + j.rise_time
        + j.land.max(0.0)
        + 0.05
}

#[derive(Debug, Clone)]
pub struct BotAvatarSim {
    pub pose: BotAvatarPose,
    pub state: BotAvatarState,
    pub t: f64,
    w_from: [f64; 3],
    tr: f64,
    tr_duration: f64,
    blink: Pulse,
    blink_again: bool,
    dart: Pulse,
    dart_x: f64,
    dart_y: f64,
    flip: Pulse,
    flip_poked: bool,
    jump: JumpSpec,
    flip_side: f64,
    nod: Pulse,
    hop_phase: f64,
    hop_count: u32,
    hop_gain: f64,
    laugh_ev: Pulse,
    prev_yaw: f64,
    jelly: f64,
    jelly_v: f64,
    gaze_lead: f64,
    gaze_dir: [i32; 2],
    gaze_at: f64,
    turn_k: f64,
    breath_phase: f64,
    ptr_x: f64,
    ptr_y: f64,
    ptr_s: f64,
    ptr_target_x: f64,
    ptr_target_y: f64,
    ptr_target_s: f64,
    pub base_yaw: f64,
    rng: RigRng,
    yaw_w: DriftChannel,
    pitch_w: DriftChannel,
    roll_w: DriftChannel,
    look_xw: DriftChannel,
    look_yw: DriftChannel,
    pub blink_at: f64,
    flip_at: f64,
    nod_at: f64,
    dart_at: f64,
    laugh_at: f64,
}

impl BotAvatarSim {
    pub fn new(seed: f64, state: BotAvatarState) -> Self {
        let mut rng = RigRng::new(seed);
        let t = rng.next() * 10.0;
        let hop_phase = rng.next();
        let breath_phase = rng.next();
        let blink_at = t + 1.0 + rng.next() * 3.0;
        let jump = JumpSpec::default();
        let flip_at = next_flip_at(&jump, t, 1.0, &mut rng);
        let nod_at = t + 3.0 + rng.next() * 4.0;
        let dart_at = t + 1.0 + rng.next() * 2.0;
        let laugh_at = t + 0.6 + rng.next() * 1.5;

        let mut sim = Self {
            pose: BotAvatarPose {
                yaw: 0.0,
                pitch: 0.0,
                roll: 0.0,
                x: 0.0,
                y: 0.0,
                sx: 1.0,
                sy: 1.0,
                eye_open: 1.0,
                blink_l: 0.0,
                blink_r: 0.0,
                look_x: 0.0,
                look_y: 0.0,
                breath: 0.0,
                laugh: 0.0,
                whirl: 0.0,
                whirl_angle: 0.0,
                w: [1.0, 0.0, 0.0],
            },
            state: BotAvatarState::Default,
            t,
            w_from: [1.0, 0.0, 0.0],
            tr: 1.0,
            tr_duration: 1.2,
            blink: Pulse::new(0.17),
            blink_again: false,
            dart: Pulse::new(0.12),
            dart_x: 0.0,
            dart_y: 0.0,
            flip: Pulse::new(flip_duration(&jump, false)),
            flip_poked: false,
            jump,
            flip_side: 1.0,
            nod: Pulse::new(1.7),
            hop_phase,
            hop_count: 0,
            hop_gain: 0.0,
            laugh_ev: Pulse::new(0.8),
            prev_yaw: 0.0,
            jelly: 0.0,
            jelly_v: 0.0,
            gaze_lead: 0.0,
            gaze_dir: [0, 0],
            gaze_at: 0.0,
            turn_k: 1.0,
            breath_phase,
            ptr_x: 0.0,
            ptr_y: 0.0,
            ptr_s: 0.0,
            ptr_target_x: 0.0,
            ptr_target_y: 0.0,
            ptr_target_s: 0.0,
            base_yaw: 0.0,
            rng,
            yaw_w: DriftChannel::new(36.0 * DEG, 1.1, 2.6, 3.0, true),
            pitch_w: DriftChannel::new(10.0 * DEG, 1.1, 2.6, 2.6, true),
            roll_w: DriftChannel::new(5.0 * DEG, 1.6, 3.2, 2.0, true),
            look_xw: DriftChannel::new(3.6, 0.5, 2.0, 14.0, false),
            look_yw: DriftChannel::new(2.4, 0.5, 2.0, 14.0, false),
            blink_at,
            flip_at,
            nod_at,
            dart_at,
            laugh_at,
        };
        sim.apply_state(state, true);
        sim
    }

    pub fn set_state(&mut self, state: BotAvatarState) {
        self.apply_state(state, false);
    }

    fn apply_state(&mut self, state: BotAvatarState, immediate: bool) {
        if state == self.state && !immediate {
            return;
        }
        let prev = self.state;
        self.state = state;
        if immediate {
            for i in 0..3 {
                self.pose.w[i] = if state.index() == i { 1.0 } else { 0.0 };
            }
            self.tr = 1.0;
        } else {
            self.w_from = self.pose.w;
            self.tr = 0.0;
            self.tr_duration = match prev {
                BotAvatarState::Sleeping => 1.0,
                _ => match state {
                    BotAvatarState::Default => 1.2,
                    BotAvatarState::Working => 0.7,
                    BotAvatarState::Sleeping => 1.4,
                },
            };
        }
        match state {
            BotAvatarState::Default => {
                self.yaw_w.set(IA, 2.6, 5.4, 2.0);
                self.pitch_w.set(LA, 2.8, 5.8, 1.8);
                self.roll_w.set(HA, 3.4, 6.6, 1.5);
                self.gaze_at = 0.0;
                self.gaze_dir = [0, 0];
                self.look_xw.set(3.6, 0.6, 2.2, 13.0);
                self.look_yw.set(2.4, 0.6, 2.2, 13.0);
                self.flip_at = next_flip_at(&self.jump, self.t, 0.6, &mut self.rng);
            }
            BotAvatarState::Working => {
                self.yaw_w.set(16.0 * DEG, 0.9, 1.8, 4.0);
                self.pitch_w.set(3.0 * DEG, 1.2, 2.4, 3.0);
                self.roll_w.set(0.0, 1.0, 2.0, 3.0);
                self.look_xw.set(2.0, 0.5, 1.2, 12.0);
                self.look_yw.set(1.0, 0.5, 1.2, 12.0);
                self.hop_phase = 0.0;
                self.hop_count = 0;
                self.laugh_at = self.t + 0.5 + self.rng.next() * 1.2;
            }
            BotAvatarState::Sleeping => {
                self.yaw_w.set(7.0 * DEG, 3.0, 6.0, 0.7);
                self.pitch_w.set(3.0 * DEG, 3.0, 6.0, 0.7);
                self.roll_w.set(2.0 * DEG, 3.0, 6.0, 0.6);
                self.look_xw.set(0.0, 2.0, 4.0, 2.0);
                self.look_yw.set(0.0, 2.0, 4.0, 2.0);
                self.nod_at = self.t + 2.5 + self.rng.next() * 4.0;
            }
        }
    }

    pub fn set_pointer(&mut self, x: f64, y: f64, strength: f64) {
        self.ptr_target_x = x.clamp(-1.2, 1.2);
        self.ptr_target_y = y.clamp(-1.2, 1.2);
        self.ptr_target_s = strength.clamp(0.0, 1.0);
    }

    pub fn poke(&mut self) {
        if self.flip.active() && self.flip.p < 0.6 {
            return;
        }
        self.flip_poked = true;
        self.flip.duration = flip_duration(&self.jump, true);
        self.flip_side = if self.rng.next() < 0.5 { -1.0 } else { 1.0 };
        self.flip.fire();
        self.flip_at = next_flip_at(&self.jump, self.t, 1.1, &mut self.rng);
    }

    /// How much the head swings when it looks about (`setTurn`'s value).
    pub fn turn(&self) -> f64 {
        self.turn_k
    }

    /// Where the gaze is being pulled: (x, y, strength).
    pub fn pointer_target(&self) -> (f64, f64, f64) {
        (self.ptr_target_x, self.ptr_target_y, self.ptr_target_s)
    }

    pub fn set_turn(&mut self, turn: f64) {
        let k = turn.max(0.0);
        if (k - self.turn_k).abs() > f64::EPSILON {
            self.turn_k = k;
            if self.state == BotAvatarState::Default {
                self.gaze_at = 0.0;
            }
        }
    }

    pub fn set_jump(&mut self, jump: JumpSpec) {
        let prev_every = self.jump.every;
        self.jump = jump;
        if (jump.every - prev_every).abs() > f64::EPSILON {
            self.flip_at = next_flip_at(&self.jump, self.t, 1.0, &mut self.rng);
        }
    }

    /// Blink the rig's eyes now if `fire`, and push back its own next blink by `BLINK_GAP_S`.
    pub fn blink_now(&mut self, fire: bool) {
        if fire && !self.blink.active() {
            self.blink.fire();
        }
        self.blink_at = self.blink_at.max(self.t + BLINK_GAP_S);
    }

    pub fn heading(&self) -> f64 {
        self.base_yaw
    }

    fn next_gaze(&mut self) -> [i32; 2] {
        let [ox, oy] = self.gaze_dir;
        if ox != 0 || oy != 0 {
            let e = self.rng.next();
            return if e < 0.66 {
                [-ox, -oy]
            } else if e < 0.85 {
                [-ox, oy]
            } else {
                [0, 0]
            };
        }
        let corners = [[1, -1], [-1, 1], [-1, -1], [1, 1]];
        let idx = ((self.rng.next() * corners.len() as f64).floor() as usize).min(3);
        corners[idx]
    }

    pub fn update(&mut self, dt_raw: f64) {
        let dt = dt_raw.min(0.05);
        self.t += dt;
        let o = self.t;

        if self.tr < 1.0 {
            self.tr = (self.tr + dt / self.tr_duration).min(1.0);
            let p = cosine_ease(self.tr);
            for q in 0..3 {
                let target = if q == self.state.index() { 1.0 } else { 0.0 };
                self.pose.w[q] = self.w_from[q] + (target - self.w_from[q]) * p;
            }
        }

        let [e, r, i] = self.pose.w;
        let state_poses = [
            (0.0, 0.0, 0.0, 0.0, 0.0),
            (5.0 * DEG, 0.0, 0.0, 0.0, 0.0),
            (-16.0 * DEG, 6.0 * DEG, 3.0, 0.0, 1.0),
        ];
        let (mut l_pitch, mut l_roll, mut l_y, mut l_lx, mut l_ly) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (q, weight) in self.pose.w.iter().enumerate() {
            let sp = state_poses[q];
            l_pitch += sp.0 * weight;
            l_roll += sp.1 * weight;
            l_y += sp.2 * weight;
            l_lx += sp.3 * weight;
            l_ly += sp.4 * weight;
        }

        if self.state == BotAvatarState::Default && o >= self.gaze_at {
            let [px, qy] = self.next_gaze();
            self.gaze_dir = [px, qy];
            let v = 0.84 + self.rng.next() * 0.16;
            self.yaw_w.aim((px as f64) * IA * v * self.turn_k);
            self.pitch_w.aim((qy as f64) * LA * v);
            self.roll_w.aim((px as f64) * HA * v * self.turn_k);
            self.gaze_at = o + FA + self.rng.next() * (XC - FA);
        }

        self.yaw_w.update(o, dt, &mut self.rng);
        self.pitch_w.update(o, dt, &mut self.rng);
        self.roll_w.update(o, dt, &mut self.rng);
        self.look_xw.update(o, dt, &mut self.rng);
        self.look_yw.update(o, dt, &mut self.rng);

        self.ptr_s = damp(self.ptr_s, self.ptr_target_s, 8.0, dt);
        self.ptr_x = damp(self.ptr_x, self.ptr_target_x, 14.0, dt);
        self.ptr_y = damp(self.ptr_y, self.ptr_target_y, 14.0, dt);

        let h = self.ptr_s;
        let d = 1.0 - 0.75 * h;
        self.base_yaw = damp(self.base_yaw, self.yaw_w.value * d + 22.0 * DEG * self.ptr_x * h, 5.0, dt);
        let u = l_pitch + self.pitch_w.value * d - 12.0 * DEG * self.ptr_y * h;
        let c = l_roll + self.roll_w.value * d;
        let f = l_y;
        let b = l_lx + self.look_xw.value * d + 4.5 * self.ptr_x * h;
        let m_look_y = l_ly + self.look_yw.value * d + 3.0 * self.ptr_y * h;

        let mut y_yaw = 0.0;
        let mut m_y = 0.0;
        let mut a_sx = 1.0;
        let mut g_sy = 1.0;
        let mut nod_pitch = 0.0;
        let mut x_roll = 0.0;
        let mut blink_p = 0.0;
        let mut dart_kx = 0.0;
        let mut dart_ly = 0.0;
        let mut laugh_s: f64 = 0.0;
        let mut whirl_o: f64 = 0.0;
        let mut whirl_ang = 0.0;

        let smooth_range = |lo: f64, hi: f64, v: f64| -> f64 {
            let t = ((v - lo) / (hi - lo)).clamp(0.0, 1.0);
            t * t * (3.0 - 2.0 * t)
        };
        let whirl_env = |p: f64| -> f64 { smooth_range(0.1, 0.26, p) * (1.0 - smooth_range(0.66, 0.9, p)) };
        let whirl_spin = |p: f64| -> f64 { TAU * (1.5 * p + 0.9 * smoothstep3(p)) };

        if o >= self.blink_at && !self.blink.active() && e + r > 0.5 {
            self.blink.fire();
            self.blink_again = !self.blink_again && self.rng.next() < 0.22;
            self.blink_at = o + if self.blink_again { 0.28 } else { 2.2 + self.rng.next() * 2.6 };
        }
        self.blink.update(dt);
        if self.blink.active() {
            blink_p = (PI * self.blink.p).sin();
        }

        if o >= self.dart_at && !self.dart.active() && e + r > 0.5 {
            self.dart.fire();
            self.dart_x = (self.rng.next() * 2.0 - 1.0) * 4.0;
            self.dart_y = (self.rng.next() * 2.0 - 1.0) * 2.0;
            self.dart.duration = 0.25 + self.rng.next() * 0.45;
            self.dart_at = o + 1.2 + self.rng.next() * 2.6;
        }
        self.dart.update(dt);
        if self.dart.active() {
            let p = self.dart.p;
            let q = if p < 0.15 {
                p / 0.15
            } else if p > 0.8 {
                (1.0 - p) / 0.2
            } else {
                1.0
            };
            dart_kx += self.dart_x * q * (e + r);
            dart_ly += self.dart_y * q * (e + r);
        }

        if self.state == BotAvatarState::Default && o >= self.flip_at && !self.flip.active() {
            self.flip_poked = false;
            self.flip.duration = flip_duration(&self.jump, false);
            self.flip_side = if self.rng.next() < 0.5 { -1.0 } else { 1.0 };
            self.flip.fire();
            self.flip_at = next_flip_at(&self.jump, o, 1.0, &mut self.rng);
        }
        self.flip.update(dt);
        if self.flip.active() {
            let j = self.jump;
            let q = crouch_s(&j, self.flip_poked);
            let v = (self.flip.p * self.flip.duration - q) / j.time;
            let bt = v.clamp(0.0, 1.0);
            let g = (PI * bt).sin();
            y_yaw += TAU * (j.spin as f64) * smoothstep3(bt);
            m_y -= j.height * g;
            let n_after = (v - 1.0) * j.time - j.land;
            let dt_sq = if self.flip_poked { j.click_squash_time } else { j.squash_time };
            let pre_sq = |wt: f64| -> f64 {
                if self.flip_poked {
                    wt * wt * (3.0 - 2.0 * wt)
                } else {
                    edge_bump(wt - 1.0)
                }
            };
            let gt = j.ground_time.max(0.0);
            let peak_d = j.squash_ease.peak_t() * dt_sq;
            let ft = n_after - peak_d - gt;
            let yt = if n_after <= peak_d {
                squash_curve(n_after / dt_sq, j.squash_ease)
            } else if ft <= 0.0 {
                ground_curve((n_after - peak_d) / gt, j.ground_ease)
            } else {
                rise_curve(ft / j.rise_time, j.rise_ease)
            };
            let amp = if v < 0.0 {
                pre_sq((1.0 + v * j.time / q).max(0.0))
            } else if n_after > 0.0 {
                yt
            } else if v < 0.2 {
                edge_bump(v)
            } else {
                0.0
            } * j.squash;
            a_sx += 0.16 * amp - 0.06 * g * j.stretch;
            g_sy += -0.18 * amp + 0.09 * g * j.stretch;
            x_roll += self.flip_side * j.lean * DEG * g;
            if j.spin > 0 {
                laugh_s = laugh_s.max(g);
                whirl_o = whirl_o.max(whirl_env(bt));
                whirl_ang = whirl_spin(bt);
            }
        }

        let j = self.jump;
        let gt = j.ground_time.max(0.0);
        let z = gt + j.rise_time;
        if self.state == BotAvatarState::Working {
            self.hop_gain = r;
        }
        if self.hop_gain > 0.02 && (self.state == BotAvatarState::Working || self.hop_phase > 0.0) {
            self.hop_phase += dt / HOP_PERIOD_S;
            if self.hop_phase >= 1.0 {
                if self.state == BotAvatarState::Working {
                    self.hop_phase -= 1.0;
                    self.hop_count += 1;
                } else if (self.hop_phase - 1.0) * HOP_PERIOD_S >= z {
                    self.hop_phase = 0.0;
                    self.hop_gain = 0.0;
                }
            }
            let p = self.hop_gain;
            let q = self.hop_phase.min(1.0);
            let v = (PI * q).sin();
            let third = self.hop_count % 3 == 2;
            m_y -= if third { FLIP_HEIGHT } else { 18.0 } * v * p;
            let n_after = if self.state != BotAvatarState::Working && self.hop_phase > 1.0 {
                (self.hop_phase - 1.0) * HOP_PERIOD_S
            } else {
                -1.0
            };
            let dt_hop = if n_after < 0.0 {
                edge_bump(self.hop_phase)
            } else if n_after < gt {
                ground_curve(n_after / gt, j.ground_ease)
            } else {
                rise_curve((n_after - gt) / j.rise_time, j.rise_ease)
            };
            a_sx += (0.16 * dt_hop - 0.06 * v) * p;
            g_sy += (-0.18 * dt_hop + 0.09 * v) * p;
            if third {
                y_yaw += TAU * smoothstep3(q) * p;
                laugh_s = laugh_s.max(v * p);
                if whirl_env(q) * p > whirl_o {
                    whirl_o = whirl_env(q) * p;
                    whirl_ang = whirl_spin(q);
                }
            }
            x_roll += if self.hop_count % 2 == 0 { 1.0 } else { -1.0 } * 6.0 * DEG * v * p;
        }

        if self.state == BotAvatarState::Working && o >= self.laugh_at && !self.laugh_ev.active() {
            self.laugh_ev.fire();
            self.laugh_ev.duration = 0.6 + self.rng.next() * 0.5;
            self.laugh_at = o + 1.6 + self.rng.next() * 2.2;
        }
        self.laugh_ev.update(dt);
        if self.laugh_ev.active() {
            let p = self.laugh_ev.p;
            let env = if p < 0.18 {
                p / 0.18
            } else if p > 0.78 {
                (1.0 - p) / 0.22
            } else {
                1.0
            };
            laugh_s = laugh_s.max(env);
        }

        if self.state == BotAvatarState::Sleeping && o >= self.nod_at && !self.nod.active() {
            self.nod.fire();
            self.nod_at = o + 4.0 + self.rng.next() * 4.0;
        }
        self.nod.update(dt);
        if self.nod.active() {
            let p = self.nod.p;
            let q = if p < 0.72 {
                smoothstep3(p / 0.72)
            } else {
                1.0 - smoothstep3((p - 0.72) / 0.28)
            };
            nod_pitch -= 13.0 * DEG * q * i;
        }

        self.breath_phase += dt / (3.6 + 1.2 * i);
        let breath = (self.breath_phase * TAU).sin();
        self.pose.breath = breath;
        a_sx += breath * (0.008 + 0.014 * i);
        g_sy += breath * (0.012 + 0.02 * i);
        let idle_bob = (o * TAU / 3.4).sin() * 2.0 * (1.0 - i);

        self.pose.yaw = self.base_yaw + y_yaw;
        let mut dyaw = self.base_yaw - self.prev_yaw;
        dyaw = ((dyaw + PI) % TAU + TAU) % TAU - PI;
        self.prev_yaw = self.base_yaw;
        let yaw_speed = if dt > 0.0 { dyaw.abs() / dt } else { 0.0 };
        let lead_target = if dt > 0.0 {
            (dyaw / dt * 2.4).clamp(-2.2, 2.2)
        } else {
            0.0
        };
        self.gaze_lead = damp(self.gaze_lead, lead_target, 9.0, dt);

        let jelly_target = (0.055 * yaw_speed).min(0.22);
        let omega = 16.0;
        let zeta = 0.45;
        self.jelly_v += (omega * omega * (jelly_target - self.jelly) - 2.0 * zeta * omega * self.jelly_v) * dt;
        self.jelly += self.jelly_v * dt;
        let gt = self.jelly.clamp(-0.08, 0.28) * 0.6;
        a_sx *= 1.0 + gt;
        g_sy *= 1.0 - 0.55 * gt;

        self.pose.pitch = u + nod_pitch;
        self.pose.roll = c + x_roll;
        self.pose.x = 0.0;
        self.pose.y = f + m_y + idle_bob;
        self.pose.sx = a_sx;
        self.pose.sy = g_sy;
        self.pose.eye_open = 1.0;
        self.pose.laugh = damp(self.pose.laugh, laugh_s, 30.0, dt);
        self.pose.blink_l = blink_p;
        self.pose.blink_r = blink_p;
        self.pose.look_x = b + dart_kx + self.gaze_lead;
        self.pose.look_y = m_look_y + dart_ly;
        self.pose.whirl = whirl_o;
        self.pose.whirl_angle = whirl_ang;
    }
}

fn next_flip_at(j: &JumpSpec, now: f64, scale: f64, rng: &mut RigRng) -> f64 {
    if j.every > 0.0 {
        now + j.every * scale * (0.625 + rng.next() * 0.75)
    } else {
        f64::INFINITY
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PixelRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pull {
    pub x: f64,
    pub y: f64,
    pub strength: f64,
}

pub fn pointer_pull(box_rect: PixelRect, x: f64, y: f64) -> Pull {
    let head = {
        let h = box_rect.width / BOT_AVATAR_OVERSCAN;
        if h == 0.0 || !h.is_finite() { 1.0 } else { h }
    };
    let dx = (x - (box_rect.left + box_rect.width / 2.0)) / head;
    let dy = (y - (box_rect.top + box_rect.height / 2.0 + BOT_AVATAR_RISE * head)) / head;
    let distance = dx.hypot(dy);
    let strength = if distance < 1.0 {
        1.0
    } else if distance > 3.0 {
        0.0
    } else {
        1.0 - (distance - 1.0) / 2.0
    };
    let norm = distance.max(1.0);
    Pull {
        x: dx / norm,
        y: dy / norm,
        strength,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FaceActivity {
    Thinking,
    Working,
    Delegating,
    Responding,
    Waiting,
    Receiving,
    Blocked,
    Done,
    Idle,
}

impl FaceActivity {
    pub const ALL: [Self; 9] = [
        Self::Thinking,
        Self::Working,
        Self::Delegating,
        Self::Responding,
        Self::Waiting,
        Self::Receiving,
        Self::Blocked,
        Self::Done,
        Self::Idle,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Thinking => "thinking",
            Self::Working => "working",
            Self::Delegating => "delegating",
            Self::Responding => "responding",
            Self::Waiting => "waiting",
            Self::Receiving => "receiving",
            Self::Blocked => "blocked",
            Self::Done => "done",
            Self::Idle => "idle",
        }
    }
}

pub fn is_motionless(activity: FaceActivity) -> bool {
    matches!(activity, FaceActivity::Idle | FaceActivity::Blocked)
}

pub fn face_label(activity: FaceActivity) -> &'static str {
    match activity {
        FaceActivity::Thinking => "thinking",
        FaceActivity::Working => "working",
        FaceActivity::Delegating => "subagent working",
        FaceActivity::Responding => "replying",
        FaceActivity::Waiting => "waiting on you",
        FaceActivity::Receiving => "listening",
        FaceActivity::Blocked => "blocked",
        FaceActivity::Done => "done",
        FaceActivity::Idle => "idle",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveActivity {
    Thinking,
    Working,
    Delegating,
    Responding,
    Idle,
}

pub fn is_subagent_tool(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    matches!(lower.as_str(), "task" | "agent" | "subagent" | "spawn_agent")
        || lower
            .strip_prefix("subagent:")
            .map(|rest| !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
            .unwrap_or(false)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveRuntimeEvent {
    MessageDelta { final_chunk: bool },
    StepStarted { step_kind: String },
    StepCompleted { step_kind: String },
    StepFailed { step_kind: String },
    ToolStarted {
        item_id: String,
        name: String,
        by_helper: bool,
    },
    ToolCompleted { item_id: String },
    ToolFailed { item_id: String },
    Other,
}

pub fn live_activity_of(events: &[LiveRuntimeEvent], running: bool) -> LiveActivity {
    if !running {
        return LiveActivity::Idle;
    }
    let mut streaming = false;
    let mut reasoning = false;
    let mut open_tools = HashSet::<&str>::new();
    let mut open_subagents = HashSet::<&str>::new();
    let mut turn_open = false;

    for ev in events {
        match ev {
            LiveRuntimeEvent::MessageDelta { final_chunk } => streaming = !final_chunk,
            LiveRuntimeEvent::StepStarted { step_kind } => {
                if step_kind == "reasoning" {
                    reasoning = true;
                } else if step_kind == "turn" {
                    turn_open = true;
                }
            }
            LiveRuntimeEvent::StepCompleted { step_kind } | LiveRuntimeEvent::StepFailed { step_kind } => {
                if step_kind == "reasoning" {
                    reasoning = false;
                } else if step_kind == "turn" {
                    turn_open = false;
                }
            }
            LiveRuntimeEvent::ToolStarted {
                item_id,
                name,
                by_helper,
            } => {
                if *by_helper {
                    continue;
                }
                open_tools.insert(item_id.as_str());
                if is_subagent_tool(name) {
                    open_subagents.insert(item_id.as_str());
                }
                reasoning = false;
            }
            LiveRuntimeEvent::ToolCompleted { item_id } | LiveRuntimeEvent::ToolFailed { item_id } => {
                open_tools.remove(item_id.as_str());
                open_subagents.remove(item_id.as_str());
                streaming = false;
            }
            LiveRuntimeEvent::Other => {}
        }
    }

    if streaming && open_tools.is_empty() {
        return LiveActivity::Responding;
    }
    if reasoning {
        return LiveActivity::Thinking;
    }
    if !open_subagents.is_empty() {
        return LiveActivity::Delegating;
    }
    if !open_tools.is_empty() {
        return LiveActivity::Working;
    }
    if turn_open {
        return LiveActivity::Working;
    }
    LiveActivity::Thinking
}

pub fn teammate_activity(
    blocked: bool,
    waiting_on_you: bool,
    live: LiveActivity,
    recently_done: bool,
    recently_received: bool,
) -> FaceActivity {
    if blocked {
        return FaceActivity::Blocked;
    }
    if waiting_on_you {
        return FaceActivity::Waiting;
    }
    match live {
        LiveActivity::Thinking => return FaceActivity::Thinking,
        LiveActivity::Working => return FaceActivity::Working,
        LiveActivity::Delegating => return FaceActivity::Delegating,
        LiveActivity::Responding => return FaceActivity::Responding,
        LiveActivity::Idle => {}
    }
    if recently_done {
        return FaceActivity::Done;
    }
    if recently_received {
        return FaceActivity::Receiving;
    }
    FaceActivity::Idle
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlanceSide {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glance {
    pub x: f64,
    pub y: f64,
}

pub fn glance_toward(side: GlanceSide) -> Glance {
    match side {
        GlanceSide::Left => Glance { x: -1.0, y: 0.0 },
        GlanceSide::Right => Glance { x: 1.0, y: 0.0 },
        GlanceSide::Up => Glance { x: 0.0, y: -1.0 },
        GlanceSide::Down => Glance { x: 0.0, y: 1.0 },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlanceAxis {
    Row,
    Column,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handoff<'a> {
    pub key: &'a str,
    pub from: &'a str,
    pub to: &'a str,
}

pub fn glances_among(
    line: &[String],
    handoffs: &[Handoff<'_>],
    axis: GlanceAxis,
) -> HashMap<String, GlanceSide> {
    let mut sides = HashMap::new();
    let (onward, back) = match axis {
        GlanceAxis::Row => (GlanceSide::Right, GlanceSide::Left),
        GlanceAxis::Column => (GlanceSide::Down, GlanceSide::Up),
    };
    for h in handoffs {
        let from = line.iter().position(|id| id == h.from);
        let to = line.iter().position(|id| id == h.to);
        if let (Some(f), Some(t)) = (from, to) {
            if f != t {
                sides.insert(h.from.to_string(), if t > f { onward } else { back });
                sides.insert(h.to.to_string(), if t > f { back } else { onward });
            }
        }
    }
    sides
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotMood {
    Curious,
    Perked,
    Glad,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IdleMoment {
    pub name: &'static str,
    pub eyes: [&'static str; 2],
    pub mood: Option<BotMood>,
    pub state: Option<BotAvatarState>,
    pub glance: Option<Glance>,
    pub seconds: f64,
}

pub const IDLE_MOMENTS: [IdleMoment; 4] = [
    IdleMoment {
        name: "look",
        eyes: ["o", "o"],
        mood: None,
        state: None,
        glance: Some(Glance { x: 1.0, y: -0.35 }),
        seconds: 1.8,
    },
    IdleMoment {
        name: "content",
        eyes: ["^", "^"],
        mood: Some(BotMood::Glad),
        state: None,
        glance: None,
        seconds: 1.6,
    },
    IdleMoment {
        name: "curious",
        eyes: ["o", "o"],
        mood: Some(BotMood::Curious),
        state: None,
        glance: None,
        seconds: 2.0,
    },
    IdleMoment {
        name: "doze",
        eyes: ["c", "c"],
        mood: None,
        state: Some(BotAvatarState::Sleeping),
        glance: None,
        seconds: 2.6,
    },
];

pub const WORKING_BEATS: [IdleMoment; 4] = [
    IdleMoment {
        name: "think",
        eyes: ["•", "•"],
        mood: None,
        state: None,
        glance: None,
        seconds: 3.4,
    },
    IdleMoment {
        name: "read",
        eyes: ["o", "o"],
        mood: None,
        state: None,
        glance: Some(Glance { x: 1.0, y: 0.3 }),
        seconds: 2.4,
    },
    IdleMoment {
        name: "focus",
        eyes: ["-", "-"],
        mood: None,
        state: None,
        glance: None,
        seconds: 2.2,
    },
    IdleMoment {
        name: "idea",
        eyes: ["o", "o"],
        mood: Some(BotMood::Perked),
        state: None,
        glance: None,
        seconds: 2.0,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotMotionLevel {
    Full,
    Subtle,
}

pub fn moment_every(level: BotMotionLevel) -> (f64, f64) {
    match level {
        BotMotionLevel::Full => (7.0, 14.0),
        BotMotionLevel::Subtle => (18.0, 36.0),
    }
}

pub fn working_lead(level: BotMotionLevel) -> (f64, f64) {
    match level {
        BotMotionLevel::Full => (5.0, 8.0),
        BotMotionLevel::Subtle => (8.0, 13.0),
    }
}

pub fn seeded(seed: f64, count: u32, salt: u32) -> f64 {
    let mut hash = ((seed * 4294967296.0).floor() as i64 as u32)
        ^ (count.wrapping_add(1)).wrapping_mul(2654435761)
        ^ (salt.wrapping_add(7)).wrapping_mul(40503);
    hash = (hash ^ (hash >> 16)).wrapping_mul(2246822507);
    hash = (hash ^ (hash >> 13)).wrapping_mul(3266489909);
    (hash ^ (hash >> 16)) as f64 / 4294967296.0
}

pub fn moment_for(seed: f64, count: u32, last: Option<&str>) -> IdleMoment {
    let choices: Vec<IdleMoment> = IDLE_MOMENTS
        .iter()
        .copied()
        .filter(|m| Some(m.name) != last)
        .collect();
    let idx = ((seeded(seed, count, 1) * choices.len() as f64).floor() as usize).min(choices.len() - 1);
    let mut chosen = choices[idx];
    if let Some(ref mut g) = chosen.glance {
        g.x = if seeded(seed, count, 2) < 0.5 { -1.0 } else { 1.0 };
    }
    chosen
}

pub fn working_beat_for(seed: f64, count: u32) -> IdleMoment {
    let start = (seeded(seed, 0, 3) * WORKING_BEATS.len() as f64).floor() as usize;
    let mut chosen = WORKING_BEATS[(start + count as usize) % WORKING_BEATS.len()];
    if let Some(ref mut g) = chosen.glance {
        g.x = if seeded(seed, count, 4) < 0.5 { -1.0 } else { 1.0 };
    }
    chosen
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrtKind {
    On,
    Flick,
}

pub fn crt_rule(seed: f64, from: Option<&str>, to: Option<&str>, count: u32) -> Option<CrtKind> {
    let to = to?;
    if CRT_ON_KEYS.contains(&to) {
        return Some(CrtKind::On);
    }
    if to == "working" {
        return if from.map(|f| f.starts_with("working")).unwrap_or(false) {
            None
        } else {
            Some(CrtKind::On)
        };
    }
    if to.starts_with("working:") && seeded(seed, count, 5) < CRT_FLICK_SHARE {
        return Some(CrtKind::Flick);
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BotMotion {
    pub state: BotAvatarState,
    pub paused: bool,
    pub jump_every: Option<f64>,
    pub bounces: bool,
    pub hops: bool,
}

pub fn at_work(activity: FaceActivity) -> bool {
    matches!(
        activity,
        FaceActivity::Thinking
            | FaceActivity::Working
            | FaceActivity::Delegating
            | FaceActivity::Responding
    )
}

pub fn worn_activity(activity: FaceActivity) -> FaceActivity {
    if at_work(activity) {
        FaceActivity::Working
    } else {
        activity
    }
}

pub fn bot_motion(activity: FaceActivity, level: BotMotionLevel) -> BotMotion {
    match activity {
        FaceActivity::Done => BotMotion {
            state: BotAvatarState::Default,
            paused: false,
            jump_every: Some(0.0),
            bounces: false,
            hops: true,
        },
        FaceActivity::Working | FaceActivity::Delegating => match level {
            BotMotionLevel::Full => BotMotion {
                state: BotAvatarState::Working,
                paused: false,
                jump_every: None,
                bounces: false,
                hops: false,
            },
            BotMotionLevel::Subtle => BotMotion {
                state: BotAvatarState::Default,
                paused: false,
                jump_every: Some(0.0),
                bounces: true,
                hops: false,
            },
        },
        FaceActivity::Thinking
        | FaceActivity::Responding
        | FaceActivity::Receiving
        | FaceActivity::Waiting => BotMotion {
            state: BotAvatarState::Default,
            paused: false,
            jump_every: match level {
                BotMotionLevel::Full => None,
                BotMotionLevel::Subtle => Some(0.0),
            },
            bounces: false,
            hops: false,
        },
        FaceActivity::Blocked | FaceActivity::Idle => BotMotion {
            state: BotAvatarState::Default,
            paused: true,
            jump_every: Some(0.0),
            bounces: false,
            hops: false,
        },
    }
}

pub fn eye_glyphs_for(activity: FaceActivity) -> Option<[&'static str; 2]> {
    match activity {
        FaceActivity::Working | FaceActivity::Delegating => Some([">", "▮"]),
        FaceActivity::Thinking => Some(["•", "•"]),
        FaceActivity::Done => Some(["^", "^"]),
        FaceActivity::Blocked => Some([">", "<"]),
        FaceActivity::Waiting | FaceActivity::Receiving | FaceActivity::Responding => {
            Some(["o", "o"])
        }
        FaceActivity::Idle => None,
    }
}

pub fn phosphor_for(activity: FaceActivity) -> Phosphor {
    match activity {
        FaceActivity::Waiting => Phosphor::Amber,
        FaceActivity::Blocked => Phosphor::Red,
        _ => Phosphor::Cyan,
    }
}

pub fn flash_for(activity: FaceActivity) -> Option<Phosphor> {
    if activity == FaceActivity::Done {
        Some(Phosphor::Green)
    } else {
        None
    }
}

pub fn mood_for(activity: FaceActivity) -> Option<BotMood> {
    match activity {
        FaceActivity::Waiting => Some(BotMood::Curious),
        FaceActivity::Receiving => Some(BotMood::Perked),
        FaceActivity::Done => Some(BotMood::Glad),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EverydayFace {
    pub eyes: Option<[&'static str; 2]>,
    pub mood: Option<BotMood>,
    pub key: String,
    pub state: Option<BotAvatarState>,
    pub glance: Option<Glance>,
    pub lively: bool,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct EverydayFaceLife {
    pub noticed: bool,
    pub listening: bool,
    pub moment: Option<IdleMoment>,
    pub beat: Option<IdleMoment>,
}

pub fn everyday_face(activity: FaceActivity, life: EverydayFaceLife) -> EverydayFace {
    let own = EverydayFace {
        eyes: eye_glyphs_for(activity),
        mood: mood_for(activity),
        key: activity.as_str().to_string(),
        state: None,
        glance: None,
        lively: false,
    };
    if activity == FaceActivity::Working && !life.noticed {
        if let Some(beat) = life.beat {
            return EverydayFace {
                eyes: Some(beat.eyes),
                mood: beat.mood,
                key: format!("working:{}", beat.name),
                state: Some(BotAvatarState::Default),
                glance: beat.glance,
                lively: false,
            };
        }
    }
    if activity != FaceActivity::Idle {
        return EverydayFace {
            lively: life.noticed,
            ..own
        };
    }
    if life.noticed {
        return EverydayFace {
            eyes: Some(["^", "^"]),
            mood: Some(BotMood::Glad),
            key: "idle:noticed".to_string(),
            state: None,
            glance: None,
            lively: true,
        };
    }
    if life.listening {
        return EverydayFace {
            eyes: Some(["o", "o"]),
            mood: Some(BotMood::Perked),
            key: "idle:listening".to_string(),
            state: None,
            glance: None,
            lively: true,
        };
    }
    if let Some(moment) = life.moment {
        return EverydayFace {
            eyes: Some(moment.eyes),
            mood: moment.mood,
            key: format!("idle:{}", moment.name),
            state: moment.state,
            glance: moment.glance,
            lively: true,
        };
    }
    own
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlyphMotion {
    pub dx: f64,
    pub dy: f64,
    pub sx: f64,
    pub sy: f64,
    pub shown: bool,
}

const GLYPH_AT_REST: GlyphMotion = GlyphMotion {
    dx: 0.0,
    dy: 0.0,
    sx: 1.0,
    sy: 1.0,
    shown: true,
};

fn frac(n: f64) -> f64 {
    n - n.floor()
}

fn hump(phase: f64, from: f64, length: f64) -> f64 {
    let p = (phase - from) / length;
    if p <= 0.0 || p >= 1.0 {
        0.0
    } else {
        (PI * p).sin().powi(2)
    }
}

pub fn glyph_motion(pair: &str, eye: usize, seconds: f64) -> GlyphMotion {
    let seconds = seconds.max(0.0);
    match pair {
        ">▮" => {
            let phase = frac(seconds / 2.4);
            let typing = phase < 0.34;
            let keystroke = if typing { frac((phase * 2.4) / 0.15) } else { 1.0 };
            let pop = if typing { hump(keystroke, 0.0, 0.5) } else { 0.0 };
            if eye == 0 {
                return GlyphMotion {
                    dx: 0.9 * hump(phase, 0.0, 0.34),
                    sx: 1.0 + 0.08 * pop,
                    ..GLYPH_AT_REST
                };
            }
            let steps = if typing {
                ((phase * 2.4) / 0.15).floor() as i32 % 4
            } else {
                3
            };
            let blink_on = typing || frac((phase * 2.4 - 0.82) / 0.8) < 0.56;
            GlyphMotion {
                dx: steps as f64 * 0.85 - 1.3,
                dy: 0.0,
                sx: 1.0 + 0.16 * pop,
                sy: 1.0 - 0.1 * pop,
                shown: blink_on,
            }
        }
        "••" => {
            let t = frac(seconds / THINKING_CYCLE_S) * THINKING_CYCLE_S;
            if t < LOOK_ABOUT_S {
                return GlyphMotion {
                    dy: DOTS_REST_Y,
                    ..GLYPH_AT_REST
                };
            }
            let beat = ((t - LOOK_ABOUT_S) * PI * 2.4 - (eye as f64) * PI * 0.6)
                .sin()
                .max(0.0);
            let p = ((t - LOOK_ABOUT_S) / 0.25).min(1.0);
            let q = ((t - (THINKING_CYCLE_S - 0.35)) / 0.35).clamp(0.0, 1.0);
            let swing = beat * p * p * (3.0 - 2.0 * p) * (1.0 - q * q * (3.0 - 2.0 * q));
            GlyphMotion {
                dx: 0.0,
                dy: DOTS_REST_Y - 1.3 * swing,
                sx: 1.0 - 0.12 * swing,
                sy: 1.0 + 0.12 * swing,
                shown: true,
            }
        }
        "^^" => {
            let bob = (seconds * 4.2 - (eye as f64) * 0.35).sin().max(0.0);
            GlyphMotion {
                dx: 0.0,
                dy: -1.5 * bob,
                sx: 1.0 + 0.06 * bob,
                sy: 1.0 - 0.08 * bob,
                shown: true,
            }
        }
        "xx" | "><" => {
            let phase = frac(seconds / 3.0);
            let shudder = hump(phase, 0.0, 0.14);
            let wince = if pair == "><" { hump(phase, 0.0, 0.4) } else { 0.0 };
            GlyphMotion {
                dx: 1.1 * shudder * (seconds * 70.0).sin(),
                dy: 0.0,
                sx: 1.0 - 0.18 * wince,
                sy: 1.0 + 0.06 * wince,
                shown: true,
            }
        }
        "cc" => {
            let breath = (seconds * 1.3).sin();
            GlyphMotion {
                dy: 0.5 * breath,
                sx: 1.0 + 0.03 * breath,
                ..GLYPH_AT_REST
            }
        }
        _ => GLYPH_AT_REST,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadGlance {
    pub yaw: f64,
    pub pitch: f64,
    pub mix: f64,
    pub wander: f64,
}

const THINKING_GLANCES: [(f64, f64); 2] = [(-0.22, 0.12), (0.22, 0.1)];

pub fn ease_out_quint(t: f64) -> f64 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powi(5)
}

pub fn glance_ease(t: f64) -> f64 {
    ease_out_quint(t.clamp(0.0, 1.0).powf(1.6))
}

pub fn thinking_glance(seconds: f64) -> HeadGlance {
    if seconds <= 0.0 {
        return HeadGlance {
            yaw: 0.0,
            pitch: 0.0,
            mix: 0.0,
            wander: 1.0,
        };
    }
    let t = frac(seconds / THINKING_CYCLE_S) * THINKING_CYCLE_S;
    let every = LOOK_ABOUT_S / THINKING_GLANCES.len() as f64;
    let at = ((t / every).floor() as usize).min(THINKING_GLANCES.len() - 1);
    let to = THINKING_GLANCES[at];
    let from = THINKING_GLANCES[at.saturating_sub(1)];
    let k = if at == 0 {
        1.0
    } else {
        glance_ease((t - (at as f64) * every) / HEAD_GLANCE_S)
    };
    let mix = if t < LOOK_ABOUT_S {
        glance_ease(t / HEAD_GLANCE_S)
    } else {
        1.0 - glance_ease((t - LOOK_ABOUT_S) / HEAD_GLANCE_S)
    };
    HeadGlance {
        yaw: from.0 + (to.0 - from.0) * k,
        pitch: from.1 + (to.1 - from.1) * k,
        mix,
        wander: 1.0,
    }
}

#[derive(Debug, Clone)]
pub struct EasedValue {
    from: f64,
    to: f64,
    since: f64,
    seconds: f64,
}

impl EasedValue {
    pub fn new(initial: f64, seconds: f64) -> Self {
        Self {
            from: initial,
            to: initial,
            since: f64::NEG_INFINITY,
            seconds,
        }
    }

    pub fn set(&mut self, target: f64, now: f64) {
        if (target - self.to).abs() <= f64::EPSILON {
            return;
        }
        self.from = self.at(now);
        self.to = target;
        self.since = now;
    }

    pub fn at(&self, now: f64) -> f64 {
        self.from + (self.to - self.from) * glance_ease((now - self.since) / self.seconds)
    }

    pub fn settled(&self, now: f64) -> bool {
        now - self.since >= self.seconds
    }
}

pub fn eye_openness(line_width: f64) -> f64 {
    ((line_width - 2.8) / 9.8).clamp(0.0, 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lag {
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
}

pub const LAG_REST: Lag = Lag {
    x: 0.0,
    y: 0.0,
    vx: 0.0,
    vy: 0.0,
};

pub fn step_lag(lag: Lag, dt: f64, kick_x: f64, kick_y: f64) -> Lag {
    let steps = ((dt / LAG_MOST_STEP - 1e-9).ceil() as usize).max(1);
    let each = dt / steps as f64;
    let mut x = lag.x;
    let mut y = lag.y;
    let mut vx = lag.vx;
    let mut vy = lag.vy;
    for step in 0..steps {
        let kx = if step == 0 { kick_x * LAG_GAIN } else { 0.0 };
        let ky = if step == 0 { kick_y * LAG_GAIN } else { 0.0 };
        vx = vx + (-LAG_SPRING * x - LAG_DAMPING * vx) * each - kx;
        vy = vy + (-LAG_SPRING * y - LAG_DAMPING * vy) * each - ky;
        x = (x + vx * each).clamp(-LAG_MOST, LAG_MOST);
        y = (y + vy * each).clamp(-LAG_MOST, LAG_MOST);
    }
    Lag { x, y, vx, vy }
}

pub fn flash_fade(seconds: f64) -> f64 {
    let k = ((seconds - FLASH_HOLD_S) / (FLASH_S - FLASH_HOLD_S)).clamp(0.0, 1.0);
    k * k * (3.0 - 2.0 * k)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerOnLight {
    pub width: f64,
    pub height: f64,
    pub alpha: f64,
}

pub fn power_on_light(phase: f64) -> PowerOnLight {
    let p = phase.clamp(0.0, 1.0);
    let across = ease_out_quint(p / POWER_LINE);
    let open = ease_out_quint((p - POWER_LINE) / (POWER_FLASH - POWER_LINE));
    PowerOnLight {
        width: across,
        height: open,
        alpha: if p < POWER_LINE { 1.0 } else { 0.85 * (1.0 - open) },
    }
}

pub fn power_on_eyes(phase: f64) -> f64 {
    ease_out_quint((phase.clamp(0.0, 1.0) - POWER_FLASH) / (1.0 - POWER_FLASH))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShownFace {
    pub pair: Option<[String; 2]>,
    pub tone: Phosphor,
    pub key: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FaceChange {
    pub since: f64,
    pub open: bool,
    pub shut: f64,
    pub quiet: bool,
    pub count: u32,
    pub eyes: Option<[String; 2]>,
}

#[derive(Debug, Clone)]
struct SwapState {
    start: f64,
    to: ShownFace,
    in_place: bool,
}

#[derive(Debug, Clone)]
pub struct FaceChanges {
    shown: ShownFace,
    flash_shown: Option<Phosphor>,
    flash_at: f64,
    swap: Option<SwapState>,
    swap_shut: f64,
    since: f64,
    changes: u32,
    changed_at: f64,
    change_now: FaceChange,
}

impl FaceChanges {
    pub fn new(initial: ShownFace) -> Self {
        let eyes = initial.pair.clone();
        Self {
            shown: initial,
            flash_shown: None,
            flash_at: f64::NEG_INFINITY,
            swap: None,
            swap_shut: 0.0,
            since: 0.0,
            changes: 0,
            changed_at: f64::NAN,
            change_now: FaceChange {
                since: 0.0,
                open: true,
                shut: 0.0,
                quiet: true,
                count: 0,
                eyes,
            },
        }
    }

    pub fn at(
        &mut self,
        s: f64,
        wanted: ShownFace,
        flash: Option<Phosphor>,
        boot_at: Option<f64>,
        boot_span: f64,
    ) -> FaceChange {
        if s == self.changed_at && s != 0.0 {
            return self.change_now.clone();
        }
        self.changed_at = s;
        if s == 0.0 {
            self.swap = None;
            self.swap_shut = 0.0;
            self.shown = wanted;
            self.flash_shown = None;
            self.since = 0.0;
        } else {
            let target_ref = self.swap.as_ref().map(|sw| &sw.to).unwrap_or(&self.shown);
            if target_ref != &wanted {
                let start = match &self.swap {
                    None => s,
                    Some(sw) => {
                        let p = (s - sw.start) / SWAP_S;
                        if p >= 1.0 {
                            s
                        } else if p < 0.5 {
                            sw.start
                        } else {
                            s - (1.0 - p) * SWAP_S
                        }
                    }
                };
                self.swap = Some(SwapState {
                    start,
                    to: wanted,
                    in_place: false,
                });
                self.changes += 1;
            }
            if let Some(ref mut sw) = self.swap {
                let p = (s - sw.start) / SWAP_S;
                if p >= 0.5 && !sw.in_place {
                    sw.in_place = true;
                    self.shown = sw.to.clone();
                    self.since = sw.start + SWAP_S;
                    if flash != self.flash_shown {
                        self.flash_shown = flash;
                        self.flash_at = sw.start + SWAP_S / 2.0;
                    }
                }
                self.swap_shut = if p >= 1.0 {
                    0.0
                } else if p < 0.5 {
                    p * 2.0
                } else {
                    (1.0 - p) * 2.0
                };
                if p >= 1.0 {
                    self.swap = None;
                }
            }
            if flash != self.flash_shown && self.swap.as_ref().map(|sw| sw.in_place).unwrap_or(true)
            {
                self.flash_shown = flash;
                self.flash_at = s;
            }
        }

        let quiet = self.swap.is_none()
            && (s == 0.0
                || ((self.flash_shown.is_none() || flash_fade(s - self.flash_at) >= 1.0)
                    && boot_at.map(|b| s - b >= boot_span).unwrap_or(true)));
        self.change_now = FaceChange {
            since: self.since,
            open: self.swap.is_none(),
            shut: self.swap_shut,
            quiet,
            count: self.changes,
            eyes: self.shown.pair.clone(),
        };
        self.change_now.clone()
    }

    pub fn shown(&self) -> &ShownFace {
        &self.shown
    }

    pub fn coming(&self) -> Option<&ShownFace> {
        self.swap.as_ref().map(|s| &s.to)
    }

    pub fn flash(&self) -> Option<(Phosphor, f64)> {
        self.flash_shown.map(|c| (c, self.flash_at))
    }
}

pub fn screen_light(tone: Phosphor, flash: Option<(Phosphor, f64)>, s: f64) -> (String, String) {
    let own = (tone.lit_css().to_string(), tone.glow_css().to_string());
    let Some((col, at)) = flash else {
        return own;
    };
    let k = flash_fade(s - at);
    if k >= 1.0 {
        return own;
    }
    (
        mix_phosphor_css(col.lit_css(), tone.lit_css(), k),
        mix_phosphor_css(col.glow_css(), tone.glow_css(), k),
    )
}

pub fn screen_pose(
    pose: BotAvatarPose,
    heading: Option<f64>,
    glance: Option<HeadGlance>,
) -> BotAvatarPose {
    let wrapped = pose.yaw.sin().atan2(pose.yaw.cos());
    let wander = glance.map(|g| g.wander).unwrap_or(1.0);
    let own = heading.unwrap_or(wrapped).clamp(-SCREEN_YAW_LIMIT, SCREEN_YAW_LIMIT) * wander;
    let own_pitch = pose.pitch * wander;
    let mix = glance.map(|g| g.mix).unwrap_or(0.0);
    BotAvatarPose {
        yaw: own + (glance.map(|g| g.yaw).unwrap_or(own) - own) * mix,
        pitch: own_pitch + (glance.map(|g| g.pitch).unwrap_or(own_pitch) - own_pitch) * mix,
        laugh: 0.0,
        whirl: 0.0,
        sx: 1.0 + (pose.sx - 1.0) * SCREEN_SQUASH,
        sy: 1.0 + (pose.sy - 1.0) * SCREEN_SQUASH,
        ..pose
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MoodWeights {
    pub curious: f64,
    pub perked: f64,
    pub glad: f64,
}

pub fn tilt_side(seed: f64) -> f64 {
    if ((seed * 1000.0).floor() as i64) % 2 == 0 {
        1.0
    } else {
        -1.0
    }
}

pub fn mood_pose(
    pose: BotAvatarPose,
    weights: MoodWeights,
    side: f64,
    screen: bool,
) -> BotAvatarPose {
    let glad = if screen {
        0.0
    } else {
        weights.glad.clamp(0.0, 1.0)
    };
    let [idle, working, sleeping] = pose.w;
    BotAvatarPose {
        roll: pose.roll + side * CURIOUS_TILT * weights.curious,
        pitch: pose.pitch + PERK_LIFT * weights.perked,
        laugh: if glad > 0.0 {
            pose.laugh.max(glad)
        } else {
            pose.laugh
        },
        w: if glad > 0.0 {
            [idle * (1.0 - glad), working + idle * glad, sleeping]
        } else {
            pose.w
        },
        ..pose
    }
}

pub fn rested_pose(pose: BotAvatarPose, k: f64) -> BotAvatarPose {
    if k <= 0.0 {
        return pose;
    }
    let keep = 1.0 - k.min(1.0);
    BotAvatarPose {
        yaw: pose.yaw * keep,
        pitch: pose.pitch * keep,
        roll: pose.roll * keep,
        x: pose.x * keep,
        y: pose.y * keep,
        sx: 1.0 + (pose.sx - 1.0) * keep,
        sy: 1.0 + (pose.sy - 1.0) * keep,
        look_x: pose.look_x * keep,
        look_y: pose.look_y * keep,
        breath: pose.breath * keep,
        laugh: pose.laugh * keep,
        whirl: pose.whirl * keep,
        ..pose
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyWants {
    pub mood: Option<BotMood>,
    pub hop: bool,
    pub glancing: bool,
    pub resting: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyFrame {
    pub pose: BotAvatarPose,
    pub changed: bool,
    pub blink: bool,
    pub hop: bool,
    pub rest: f64,
    pub settled: bool,
}

#[derive(Debug, Clone)]
pub struct BodyConductor {
    screen: bool,
    still: bool,
    thinking: EasedValue,
    curious: EasedValue,
    perked: EasedValue,
    glad: EasedValue,
    rest: EasedValue,
    lean: f64,
    answered_mood: Option<BotMood>,
    answered_thinking: bool,
    answered_since: f64,
    answered_resting: bool,
    blinked: u32,
}

impl BodyConductor {
    pub fn new(seed: f64, screen: bool, still: bool, resting_at_start: bool) -> Self {
        Self {
            screen,
            still,
            thinking: EasedValue::new(0.0, HEAD_GLANCE_S),
            curious: EasedValue::new(0.0, HEAD_GLANCE_S),
            perked: EasedValue::new(0.0, HEAD_GLANCE_S),
            glad: EasedValue::new(0.0, HEAD_GLANCE_S),
            rest: EasedValue::new(if resting_at_start { 1.0 } else { 0.0 }, HEAD_GLANCE_S),
            lean: tilt_side(seed),
            answered_mood: None,
            answered_thinking: false,
            answered_since: 0.0,
            answered_resting: resting_at_start,
            blinked: 0,
        }
    }

    pub fn frame(
        &mut self,
        seconds: f64,
        change: &FaceChange,
        rig: BotAvatarPose,
        heading: Option<f64>,
        wants: BodyWants,
    ) -> BodyFrame {
        let changed = change.count != self.blinked;
        let blink = changed && !self.screen;
        self.blinked = change.count;
        let mut hop = false;
        if change.open {
            let dots = change
                .eyes
                .as_ref()
                .map(|[a, b]| a == "•" && b == "•")
                .unwrap_or(false);
            self.answered_mood = wants.mood;
            self.answered_thinking = dots;
            if dots {
                self.answered_since = change.since;
            }
            self.answered_resting = wants.resting;
            hop = wants.hop && !self.still;
        }
        self.thinking.set(
            if self.answered_thinking && !wants.glancing {
                1.0
            } else {
                0.0
            },
            seconds,
        );
        let glance = thinking_glance(seconds - self.answered_since);
        let thought = self.thinking.at(seconds);
        let weight = |ev: &mut EasedValue, key: BotMood| -> f64 {
            if self.still {
                if wants.mood == Some(key) { 1.0 } else { 0.0 }
            } else {
                ev.set(
                    if self.answered_mood == Some(key) { 1.0 } else { 0.0 },
                    seconds,
                );
                ev.at(seconds)
            }
        };
        let worn = MoodWeights {
            curious: weight(&mut self.curious, BotMood::Curious),
            perked: weight(&mut self.perked, BotMood::Perked),
            glad: weight(&mut self.glad, BotMood::Glad),
        };
        self.rest
            .set(if self.answered_resting { 1.0 } else { 0.0 }, seconds);
        let rested = if self.still {
            1.0
        } else {
            self.rest.at(seconds)
        };
        let base_pose = if self.screen {
            screen_pose(
                rig,
                heading,
                Some(HeadGlance {
                    yaw: glance.yaw,
                    pitch: glance.pitch,
                    mix: glance.mix * thought,
                    wander: 1.0 - THINKING_HUSH * thought,
                }),
            )
        } else {
            rig
        };
        let pose = mood_pose(rested_pose(base_pose, rested), worn, self.lean, self.screen);
        let settled = change.quiet
            && self.thinking.settled(seconds)
            && self.rest.settled(seconds)
            && self.curious.settled(seconds)
            && self.perked.settled(seconds)
            && self.glad.settled(seconds)
            && rig.blink_l.max(rig.blink_r) < 0.01;
        BodyFrame {
            pose,
            changed,
            blink,
            hop,
            rest: rested,
            settled,
        }
    }
}

/// Shared drawing beat (`frameBeat.ts`): allows 1 ms of slack so a 60 Hz frame arriving
/// at 33.3 ms (every second frame) is accepted rather than pushed out to ~50 ms.
#[derive(Debug, Clone)]
pub struct FrameBeat {
    every_ms: f64,
    at_ms: f64,
}

impl FrameBeat {
    pub fn new(per_second: f64) -> Self {
        Self {
            every_ms: 1000.0 / per_second - 1.0,
            at_ms: f64::NEG_INFINITY,
        }
    }

    pub fn due(&mut self, now_ms: f64) -> bool {
        if now_ms == self.at_ms {
            return true;
        }
        if now_ms - self.at_ms >= self.every_ms || now_ms < self.at_ms {
            self.at_ms = now_ms;
            return true;
        }
        false
    }
}

/// Pure bot frame clock matching `startBotClock` semantics for testable pacing & idle rest.
#[derive(Debug, Clone)]
pub struct BotClock {
    beat: FrameBeat,
    draw_beat: Option<FrameBeat>,
    last_ms: f64,
    stopped: bool,
    pending_handle: Option<u64>,
    next_handle: u64,
    pub draws: u32,
    pub stepped_seconds: f64,
}

impl BotClock {
    /// Starts the clock and immediately performs the initial draw before any animation frame.
    pub fn start(now_ms: f64, away: bool, at_the_side: bool) -> Self {
        let mut clock = Self {
            beat: FrameBeat::new(BOT_FRAMES_PER_SECOND),
            draw_beat: if at_the_side {
                Some(FrameBeat::new(BOT_FRAMES_PER_SECOND / 2.0))
            } else {
                None
            },
            last_ms: now_ms,
            stopped: false,
            pending_handle: None,
            next_handle: 1,
            draws: 1,
            stepped_seconds: 0.0,
        };
        if !away {
            clock.schedule();
        }
        clock
    }

    fn schedule(&mut self) {
        if self.pending_handle.is_none() && !self.stopped {
            let h = self.next_handle;
            self.next_handle += 1;
            self.pending_handle = Some(h);
        }
    }

    pub fn pending_frame(&self) -> Option<u64> {
        self.pending_handle
    }

    pub fn on_presence(&mut self, now_ms: f64, away: bool) {
        if self.stopped {
            return;
        }
        if away {
            self.pending_handle = None;
        } else if self.pending_handle.is_none() {
            self.last_ms = now_ms;
            self.schedule();
        }
    }

    /// Processes a fired animation frame, returning `(did_step_dt, did_draw)`.
    pub fn tick(&mut self, now_ms: f64, showing: bool, away: bool) -> (Option<f64>, bool) {
        if self.stopped {
            return (None, false);
        }
        self.pending_handle = None;
        let mut stepped = None;
        let mut drew = false;
        if self.beat.due(now_ms) {
            let dt = ((now_ms - self.last_ms) / 1000.0).min(0.05);
            self.last_ms = now_ms;
            if showing {
                self.stepped_seconds += dt;
                stepped = Some(dt);
                if self
                    .draw_beat
                    .as_mut()
                    .map(|b| b.due(now_ms))
                    .unwrap_or(true)
                {
                    self.draws += 1;
                    drew = true;
                }
            }
        }
        if !self.stopped && !away {
            self.schedule();
        }
        (stepped, drew)
    }

    pub fn stop(&mut self) -> Option<u64> {
        self.stopped = true;
        self.pending_handle.take()
    }
}

pub fn default_face() -> BotFace {
    BotFace::Eyes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bot_spec::{
        body_color_of, locust_bot, outline_of, BOT_AVATAR_PRESETS, BOT_SATURATION,
        PALETTE_SATURATION,
    };

    // --- every-bot-is-drawn-by-the-rig.test.ts (6 cases) ---

    #[test]
    fn is_the_librarys_own_for_its_eighteen_its_path_its_face_and_where_it_sits() {
        for preset in BOT_AVATAR_PRESETS {
            let outline = outline_of(preset.id);
            assert_eq!(outline.body, preset.body);
            assert_eq!(outline.parts.as_deref().unwrap_or(""), preset.parts);
            assert_eq!(outline.face, preset.face);
            assert!((outline.face_x - preset.face_x).abs() < 1e-9);
            assert!((outline.face_y - preset.face_y).abs() < 1e-9);
            assert!((outline.face_scale - preset.face_scale).abs() < 1e-9);
        }
    }

    #[test]
    fn is_locusts_own_for_its_two_as_it_always_was() {
        for id in ["hopper", "swarm"] {
            let outline = outline_of(id);
            let spec = locust_bot(id).expect("locust bot");
            assert_eq!(outline.body, spec.body);
            assert_eq!(outline.parts.as_deref().unwrap_or(""), spec.parts);
            assert_eq!(outline.face, BotFace::Eyes);
            assert!((outline.face_x - spec.face_x).abs() < 1e-9);
            assert!((outline.face_y - spec.face_y).abs() < 1e-9);
            assert!((outline.face_scale - spec.face_scale).abs() < 1e-9);
        }
    }

    #[test]
    fn is_botavatars_treatment_for_a_library_shape_a_hue_a_touch_more_vivid_the_palette_more_so() {
        let custom = body_color_of("clover", Some("#4f68ff"));
        let palette = body_color_of("clover", None);
        assert_ne!(custom, "#4f68ff");
        assert!(custom.starts_with("hsl("));
        assert!(palette.starts_with("hsl("));
        assert!(PALETTE_SATURATION > BOT_SATURATION);
    }

    #[test]
    fn is_flat_for_locusts_own_which_never_took_the_treatment() {
        assert_eq!(body_color_of("hopper", Some("#4f68ff")), "#4f68ff");
        assert_eq!(body_color_of("swarm", Some("#52884b")), "#52884b");
    }

    #[test]
    fn looks_straight_at_a_pointer_within_a_head_width() {
        let rect = PixelRect {
            left: 100.0,
            top: 200.0,
            width: 90.0,
            height: 90.0,
        };
        let head = 90.0 / BOT_AVATAR_OVERSCAN; // 60.0
        let cx = rect.left + rect.width / 2.0;
        let cy = rect.top + rect.height / 2.0 + BOT_AVATAR_RISE * head;
        let pull = pointer_pull(rect, cx + head * 0.5, cy);
        assert!((pull.strength - 1.0).abs() < 1e-9);
        assert!((pull.x - 0.5).abs() < 1e-9);
        assert!(pull.y.abs() < 1e-9);
    }

    #[test]
    fn lets_go_by_three_head_widths_and_never_looks_further_than_one_head_across() {
        let rect = PixelRect {
            left: 0.0,
            top: 0.0,
            width: 90.0,
            height: 90.0,
        };
        let head = 90.0 / BOT_AVATAR_OVERSCAN;
        let cx = rect.width / 2.0;
        let cy = rect.height / 2.0 + BOT_AVATAR_RISE * head;

        let mid = pointer_pull(rect, cx + head * 2.0, cy);
        assert!((mid.strength - 0.5).abs() < 1e-9);
        assert!((mid.x - 1.0).abs() < 1e-9);
        assert!(mid.y.abs() < 1e-9);

        let far = pointer_pull(rect, cx + head * 3.5, cy);
        assert_eq!(far.strength, 0.0);
        assert!((far.x - 1.0).abs() < 1e-9);
    }

    // --- a-locust-bot-draws-before-its-first-frame.test.ts (5 cases) ---

    #[test]
    fn is_drawn_once_before_any_animation_frame_so_a_page_that_paints_none_still_shows_it() {
        let clock = BotClock::start(0.0, false, false);
        assert_eq!(clock.draws, 1);
        assert!(clock.pending_frame().is_some());
    }

    #[test]
    fn then_moves_and_draws_while_it_is_showing_and_only_then() {
        let mut clock = BotClock::start(0.0, false, false);
        assert_eq!(clock.draws, 1);

        let (dt, drew) = clock.tick(34.0, false, false);
        assert!(dt.is_none());
        assert!(!drew);
        assert_eq!(clock.draws, 1);
        assert_eq!(clock.stepped_seconds, 0.0);

        let (dt, drew) = clock.tick(68.0, true, false);
        assert!(dt.is_some());
        assert!(drew);
        assert_eq!(clock.draws, 2);
        assert!(clock.stepped_seconds > 0.0);
    }

    #[test]
    fn draws_at_most_30_frames_a_second_every_other_frame_at_60_hz_every_fifth_at_144() {
        // 60 Hz: 1000/60 = 16.6667 ms per frame over 1 second (60 ticks) -> 30 draws
        let mut clock60 = BotClock::start(0.0, false, false);
        for i in 1..=60 {
            clock60.tick(i as f64 * (1000.0 / 60.0), true, false);
        }
        assert!(clock60.draws - 1 <= 30 && clock60.draws - 1 >= 29);

        // 144 Hz: 1000/144 = 6.9444 ms per frame over 1 second (144 ticks) -> <= 30 draws
        let mut clock144 = BotClock::start(0.0, false, false);
        for i in 1..=144 {
            clock144.tick(i as f64 * (1000.0 / 144.0), true, false);
        }
        assert!(clock144.draws - 1 <= 30 && clock144.draws - 1 >= 28);
    }

    #[test]
    fn draws_at_most_15_frames_a_second_at_the_side_beside_a_name_0_670_still_moving_a_whole_second() {
        let mut clock = BotClock::start(0.0, false, true);
        for i in 1..=60 {
            clock.tick(i as f64 * (1000.0 / 60.0), true, false);
        }
        assert!(clock.draws - 1 <= 15 && clock.draws - 1 >= 14);
        assert!((clock.stepped_seconds - 1.0).abs() < 0.08);
    }

    #[test]
    fn stops_when_asked_cancelling_the_frame_it_was_waiting_on() {
        let mut clock = BotClock::start(0.0, false, false);
        let pending = clock.pending_frame();
        assert!(pending.is_some());
        let cancelled = clock.stop();
        assert_eq!(cancelled, pending);
        assert!(clock.pending_frame().is_none());
        let (dt, drew) = clock.tick(100.0, true, false);
        assert!(dt.is_none());
        assert!(!drew);
        assert_eq!(clock.draws, 1);
    }

    #[test]
    fn bot_avatar_sim_updates_smoothly_and_blinks_on_demand() {
        let mut sim = BotAvatarSim::new(0.42, BotAvatarState::Default);
        let initial = sim.pose;
        assert_eq!(initial.w, [1.0, 0.0, 0.0]);
        for _ in 0..20 {
            sim.update(1.0 / 30.0);
        }
        assert!(sim.t > 0.6);
        sim.blink_now(true);
        sim.update(1.0 / 30.0);
        assert!(sim.pose.blink_l > 0.0);
    }

    #[test]
    fn body_conductor_and_face_changes_coordinate_blinks_and_resting() {
        let initial = ShownFace {
            pair: None,
            tone: Phosphor::Cyan,
            key: Some("idle".into()),
        };
        let mut fc = FaceChanges::new(initial);
        let mut body = BodyConductor::new(0.25, true, false, false);
        let sim = BotAvatarSim::new(0.25, BotAvatarState::Working);

        let ch0 = fc.at(
            0.0,
            ShownFace {
                pair: Some(["•".into(), "•".into()]),
                tone: Phosphor::Cyan,
                key: Some("thinking".into()),
            },
            None,
            None,
            POWER_ON_S,
        );
        assert_eq!(ch0.count, 0);

        let ch1 = fc.at(
            0.1,
            ShownFace {
                pair: Some([">".into(), "▮".into()]),
                tone: Phosphor::Cyan,
                key: Some("working".into()),
            },
            Some(Phosphor::Green),
            None,
            POWER_ON_S,
        );
        assert_eq!(ch1.count, 1);
        let f1 = body.frame(
            0.1,
            &ch1,
            sim.pose,
            Some(sim.heading()),
            BodyWants {
                mood: None,
                hop: false,
                glancing: false,
                resting: false,
            },
        );
        assert!(f1.changed);
        assert!(!f1.blink); // screen swaps its own lids
    }
}

#[cfg(test)]
mod renderer_mood_tests {
    use super::*;
    fn rest() -> BotAvatarPose {
        BotAvatarPose { yaw: 0.2, pitch: -0.05, roll: 0.03, ..rest_pose(BotAvatarState::Default) }
    }
    fn close(a: f64, b: f64) { assert!((a-b).abs() < 0.005, "{a} != {b}"); }

    // a-teammate-wears-how-it-feels.test.ts: one test per Electron case, exact inputs.
    #[test]
    fn is_worn_for_what_a_teammate_is_doing_curious_waiting_perked_receiving_glad_done() {
        use FaceActivity::*;
        let all = [Thinking, Working, Delegating, Responding, Waiting, Receiving, Blocked, Done, Idle];
        let found: Vec<_> = all.into_iter().map(mood_for).collect();
        assert_eq!(found, [None, None, None, None, Some(BotMood::Curious), Some(BotMood::Perked), None, Some(BotMood::Glad), None]);
    }
    #[test]
    fn tips_a_curious_head_to_its_own_side_by_the_tilt_in_proportion_to_the_mood() {
        for side in [1.0, -1.0] {
            close(mood_pose(rest(), MoodWeights { curious: 1.0, ..Default::default() }, side, false).roll, 0.03+side*CURIOUS_TILT);
            close(mood_pose(rest(), MoodWeights { curious: 0.5, ..Default::default() }, side, true).roll, 0.03+side*CURIOUS_TILT*0.5);
        }
        assert!(CURIOUS_TILT.to_degrees() > 10.0 && CURIOUS_TILT.to_degrees() < 15.0);
    }
    #[test]
    fn tips_each_teammate_the_same_way_every_time_and_not_every_teammate_the_same_way() {
        let sides: Vec<_> = (0..40).map(|i| tilt_side(i as f64/40.0+0.0123)).collect();
        assert!(sides.contains(&1.0) && sides.contains(&-1.0));
        assert!(sides.iter().all(|s| *s == 1.0 || *s == -1.0));
        for seed in [0.15, 0.37, 0.81] { assert_eq!(tilt_side(seed), tilt_side(seed)); }
    }
    #[test]
    fn lifts_a_perked_head_up_a_little() {
        close(mood_pose(rest(), MoodWeights { perked: 1.0, ..Default::default() }, 1.0, false).pitch, -0.05+PERK_LIFT);
        assert!(PERK_LIFT > 0.0);
    }
    #[test]
    fn smiles_with_the_rigs_own_laugh_on_a_face_of_plastic_lending_working_weight_for_eyes_alone() {
        let glad = mood_pose(rest(), MoodWeights { glad: 1.0, ..Default::default() }, 1.0, false);
        assert_eq!(glad.laugh, 1.0);
        close(glad.w[1], 1.0);
        close(glad.w.iter().sum(), 1.0);
        close(mood_pose(rest(), MoodWeights { glad: 0.5, ..Default::default() }, 1.0, false).w[1], 0.5);
    }
    #[test]
    fn leaves_a_screens_eyes_to_its_glyphs_carets_say_glad_there() {
        let glad = mood_pose(rest(), MoodWeights { glad: 1.0, ..Default::default() }, 1.0, true);
        assert_eq!(glad.laugh, 0.0);
        assert_eq!(glad.w, [1.0, 0.0, 0.0]);
    }
    #[test]
    fn wears_nothing_when_it_feels_nothing_and_never_changes_the_rigs_own_pose() {
        let pose = rest();
        assert_eq!(mood_pose(pose, MoodWeights::default(), 1.0, false), pose);
        mood_pose(pose, MoodWeights { curious: 1.0, perked: 1.0, glad: 1.0 }, -1.0, false);
        assert_eq!(pose, rest());
    }
    #[test]
    fn blinks_the_real_rigs_eyes_at_once() {
        let mut sim = BotAvatarSim::new(0.4, BotAvatarState::Default);
        for _ in 0..4 { sim.update(1.0/30.0); }
        let before = sim.pose.blink_l.max(sim.pose.blink_r);
        sim.blink_now(true);
        let mut deepest: f64 = 0.0;
        for _ in 0..6 { sim.update(1.0/30.0); deepest = deepest.max(sim.pose.blink_l); }
        assert!(deepest > before.max(0.5), "{deepest} <= {before}");
        // Null rig is JS plumbing: Rust calls the method only on an owned rig.
    }
    #[test]
    fn lands_with_the_librarys_own_bounce_and_no_turn_and_can_put_every_part_back() {
        assert_eq!(GLAD_HOP.spin, 0);
        assert_eq!(GLAD_HOP.squash_ease, EaseKind::Bouncy);
        assert_eq!(GLAD_HOP.rise_ease, EaseKind::Bouncy);
        assert_ne!(JumpSpec::default().squash_ease, EaseKind::Bouncy);
        let mut sim = BotAvatarSim::new(0.4, BotAvatarState::Default);
        sim.set_jump(GLAD_HOP);
        assert_eq!(sim.jump, GLAD_HOP);
        sim.set_jump(JumpSpec::default());
        assert_eq!(sim.jump, JumpSpec::default());
    }
}

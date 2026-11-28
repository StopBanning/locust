//! Frame scheduler and animation state for rig-drawn bots and Codex Buddy.
//!
//! Zero UI toolkit rendering logic here: this module advances pure `locust_core::bot_rig`
//! and `locust_core::buddy_moves` rigs for visible bot slots and asks `rune::Tx` for the
//! next frame **only** when a visible bot is actively working or finishing a transition.
//! Resting bots stay settled with zero wakeups.

use locust_core::bot_rig::{
    bot_motion, crt_rule, eye_glyphs_for, face_box, flash_fade, flash_for, is_motionless, mood_for, phosphor_for,
    power_on_light, rest_pose, step_lag, BodyConductor, BodyWants, BotAvatarPose, BotAvatarSim,
    BotAvatarState, BotMood, BotMotionLevel, CrtKind, FaceActivity, FaceChanges, FrameBeat,
    GlanceHold, GlanceSide, Lag, PowerOnLight, Pull, ShownFace, BOT_FRAMES_PER_SECOND, CRT_FLICK_S, LAG_REST,
    NOTICE_DWELL_MS, POWER_ON_S, RECEIVED_GLANCE_MS,
};
use locust_core::bot_rig::pointer_pull;
use locust_core::bot_spec::{outline_of, seed_of, Phosphor};
use locust_core::buddy_moves::{
    glance_look, screen_face_for, OldTweensCleanup, PetScreenAsk, PetScreenConductor, ScreenRectF, CODEX_BUDDY,
};
use locust_core::buddy_rig::{BuddyPose, REST_POSE};
use locust_core::bot_spec::BotFace;
use rune::Tx;
use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

const FULL_STEP_MS: u64 = 33;
const SUBTLE_STEP_MS: u64 = 66;
const HOVER_PEEK_S: f64 = 2.6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresenceDot {
    Working,
    Waiting,
    Blocked,
}

#[derive(Debug, Clone)]
pub struct BotSnapshot {
    pub pose: BotAvatarPose,
    pub eyes: Option<[String; 2]>,
    pub tone: Phosphor,
    pub flash: Option<(Phosphor, f64)>,
    pub flash_progress: f64,
    pub power_on: Option<PowerOnLight>,
    pub shut: f64,
    pub lag_x: f64,
    pub lag_y: f64,
    pub hop: bool,
    pub blinking: bool,
    pub screen: bool,
    pub plush: bool,
    pub s_seconds: f64,
    pub hovered: bool,
}

#[derive(Debug, Clone)]
pub struct BuddySnapshot {
    pub pose: BuddyPose,
    pub look: f64,
    pub eyes: [String; 2],
    pub tone: Phosphor,
    pub flash: Option<(Phosphor, f64)>,
    pub flash_progress: f64,
    pub power_on: Option<PowerOnLight>,
    pub shut: f64,
    pub glass: Option<ScreenRectF>,
    pub screen: bool,
    pub plush: bool,
    pub s_seconds: f64,
    pub hovered: bool,
}

/// The parts of a face's drawing the view chooses: `BotDrawSpec` or `BuddyDrawSpec` without the pose, which the stage
/// steps itself.
#[derive(Debug, Clone, PartialEq)]
pub enum FaceDrawing {
    Bot { shape: String, face: BotFace, hue: String, size: f32, light_theme: bool, presence: Option<PresenceDot>, ring: bool },
    Buddy { hue: String, size: f32, light_theme: bool, presence: Option<PresenceDot>, ring: bool },
}

/// What the view last drew a face with, and the drawing the window shows for it (`rune::LiveShape`). A face the view
/// handed over in its last build moves by poll drawing its next pose into `live`, with no rebuild (speed-2 row 1).
/// `this_build` is cleared by `begin_frame` and set again when the view hands the face over (`live_face`); a slot the
/// view drew some other way (a puppet) keeps asking for a rebuild.
#[derive(Default)]
struct Drawn {
    with: Option<FaceDrawing>,
    live: Option<rune::LiveShape>,
    this_build: bool,
}

impl Drawn {
    fn hand_over(&mut self, with: FaceDrawing, shape: rune::Shape) -> rune::LiveShape {
        self.with = Some(with);
        self.this_build = true;
        match &self.live {
            Some(live) => {
                live.set(shape);
                live.clone()
            }
            None => self.live.insert(rune::LiveShape::new(shape)).clone(),
        }
    }

    /// Draws `snap` into the window's drawing; false when the view must draw it (nothing handed over this build).
    fn publish_bot(&self, _slot_key: &str, snap: &BotSnapshot) -> bool {
        let (true, Some(FaceDrawing::Bot { shape, face, hue, size, light_theme, presence, ring }), Some(live)) =
            (self.this_build, &self.with, &self.live)
        else {
            return false;
        };
        let drawn = super::bots::draw_bot_shape(&super::bots::BotDrawSpec {
            shape,
            face: *face,
            hue,
            size: *size,
            light_theme: *light_theme,
            presence: *presence,
            ring: *ring,
            snap,
        });
        #[cfg(test)]
        super::faces::note_drawn(_slot_key, &drawn);
        live.set(drawn);
        true
    }

    fn publish_buddy(&self, _slot_key: &str, snap: &BuddySnapshot) -> bool {
        let (true, Some(FaceDrawing::Buddy { hue, size, light_theme, presence, ring }), Some(live)) =
            (self.this_build, &self.with, &self.live)
        else {
            return false;
        };
        let drawn = super::bots::draw_buddy_shape(&super::bots::BuddyDrawSpec {
            hue,
            size: *size,
            light_theme: *light_theme,
            presence: *presence,
            ring: *ring,
            snap,
        });
        #[cfg(test)]
        super::faces::note_drawn(_slot_key, &drawn);
        live.set(drawn);
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DesiredState {
    activity: FaceActivity,
    glance: Option<GlanceSide>,
    motion: BotMotionLevel,
    screen: bool,
    plush: bool,
}

struct BotSlot {
    seed: f64,
    desired: DesiredState,
    sim: BotAvatarSim,
    changes: FaceChanges,
    conductor: BodyConductor,
    lag: Lag,
    prev_pose: BotAvatarPose,
    start_s: f64,
    last_step_s: f64,
    last_change_count: u32,
    boot_at: Option<f64>,
    boot_span: f64,
    hovered: bool,
    hover_started_s: Option<f64>,
    hover_noticed_s: Option<f64>,
    hover_until_s: Option<f64>,
    /// The handoff glance and when it was given (Bot.tsx `aim.glance`, `glancedAt`).
    glance: GlanceHold,
    /// Where the pointer pulls the gaze while it is over the face (Bot.tsx `pointerPull`), once it has moved there.
    pull: Option<Pull>,
    seen_this_frame: bool,
    settled: bool,
    cached: BotSnapshot,
    drawn: Drawn,
}

struct BuddySlot {
    seed: f64,
    desired: DesiredState,
    /// The handoff glance and when it was given (PetSprite.tsx `glanced`).
    glance: GlanceHold,
    conductor: PetScreenConductor,
    start_s: f64,
    last_step_s: f64,
    hovered: bool,
    hover_started_s: Option<f64>,
    hover_noticed_s: Option<f64>,
    hover_until_s: Option<f64>,
    seen_this_frame: bool,
    settled: bool,
    cached: BuddySnapshot,
    drawn: Drawn,
}

struct StageInner {
    now_s: f64,
    beat_full: FrameBeat,
    beat_subtle: FrameBeat,
    bots: HashMap<String, BotSlot>,
    buddies: HashMap<String, BuddySlot>,
    reduced_snap_pending: bool,
    /// When the faces last stepped (ms), so every poll asks for the same next step.
    stepped_ms: f64,
    wake_count: u64,
    old_tweens: OldTweensCleanup,
    #[cfg(test)]
    old_tweens_removed: Vec<String>,
}

impl Default for StageInner {
    fn default() -> Self {
        Self {
            now_s: 0.0,
            beat_full: FrameBeat::new(BOT_FRAMES_PER_SECOND),
            beat_subtle: FrameBeat::new(BOT_FRAMES_PER_SECOND / 2.0),
            bots: HashMap::new(),
            buddies: HashMap::new(),
            reduced_snap_pending: false,
            stepped_ms: f64::NEG_INFINITY,
            wake_count: 0,
            old_tweens: OldTweensCleanup::new(),
            #[cfg(test)]
            old_tweens_removed: Vec::new(),
        }
    }
}

pub struct BotStage {
    inner: RefCell<StageInner>,
}

impl Default for BotStage {
    fn default() -> Self {
        Self {
            inner: RefCell::new(StageInner::default()),
        }
    }
}

fn avatar_state_for(activity: FaceActivity, still: bool) -> BotAvatarState {
    if still {
        return BotAvatarState::Default;
    }
    match activity {
        FaceActivity::Thinking
        | FaceActivity::Working
        | FaceActivity::Delegating
        | FaceActivity::Responding => BotAvatarState::Working,
        FaceActivity::Blocked => BotAvatarState::Sleeping,
        FaceActivity::Done | FaceActivity::Waiting | FaceActivity::Receiving | FaceActivity::Idle => {
            BotAvatarState::Default
        }
    }
}

fn shown_face_for(activity: FaceActivity, screen: bool) -> ShownFace {
    let pair = if screen && !matches!(activity, FaceActivity::Idle) {
        eye_glyphs_for(activity).map(|[a, b]| [a.to_owned(), b.to_owned()])
    } else {
        None
    };
    let tone = if screen {
        phosphor_for(activity)
    } else {
        Phosphor::Cyan
    };
    ShownFace {
        pair,
        tone,
        key: Some(activity.as_str().to_owned()),
    }
}

/// TeammateBot's `follows`: the pointer rested on the face long enough to be noticed, and is still on it.
fn follows(slot: &BotSlot) -> bool {
    slot.hovered && slot.hover_noticed_s.is_some()
}

fn build_bot_snapshot(
    slot: &mut BotSlot,
    now_s: f64,
    dt_s: f64,
    reduced_motion: bool,
    hover_notice: bool,
) -> BotSnapshot {
    build_bot_frame(slot, now_s, dt_s, reduced_motion, hover_notice, false)
}

/// A new bot's first picture: Bot.tsx draws once before its clock's first step (`draw()`, 2007), so the face shows the
/// rig as it was made, upright, not a step taken at no time. That step would show a working bot's hop at phase 0, its
/// landing squash (1.16 wide, 0.82 high), which flattened the ghost's hem in the picker's preview.
fn first_bot_snapshot(slot: &mut BotSlot, now_s: f64, reduced_motion: bool) -> BotSnapshot {
    build_bot_frame(slot, now_s, 0.0, reduced_motion, false, true)
}

fn build_bot_frame(
    slot: &mut BotSlot,
    now_s: f64,
    dt_s: f64,
    reduced_motion: bool,
    hover_notice: bool,
    first: bool,
) -> BotSnapshot {
    let desired = slot.desired;
    let still = reduced_motion || is_motionless(desired.activity);
    let target_state = avatar_state_for(desired.activity, still && !hover_notice);
    slot.sim.set_state(target_state);
    // Bot.tsx 2393-2400, in its order: a glance while it holds, then the pointer for a face that has noticed it
    // (TeammateBot `follows`: the pointer rested NOTICE_DWELL_MS on it and has not left), then nothing.
    let held = slot.glance.aim(now_s);
    let follows = follows(slot);
    if !still || hover_notice || held.is_some() || follows {
        if let Some(glance) = held {
            slot.sim.set_pointer(glance.x, glance.y, 1.0);
        } else if follows && let Some(pull) = slot.pull {
            slot.sim.set_pointer(pull.x, pull.y, pull.strength);
        } else {
            slot.sim.set_pointer(0.0, 0.0, 0.0);
        }
        if !first {
            slot.sim.update(dt_s.min(0.1));
        }
    } else {
        slot.sim.set_pointer(0.0, 0.0, 0.0);
        if reduced_motion {
            slot.sim.pose = rest_pose(target_state);
        } else if !first {
            slot.sim.update(dt_s.min(0.1));
        }
    }

    let rel_s = (now_s - slot.start_s).max(0.0);
    let sample_s = if reduced_motion { 0.0 } else { rel_s };
    let wanted = shown_face_for(desired.activity, desired.screen);
    let flash = if desired.screen {
        flash_for(desired.activity)
    } else {
        None
    };
    let prev_key = slot.changes.shown().key.clone();
    let change = slot
        .changes
        .at(sample_s, wanted, flash, slot.boot_at, slot.boot_span);
    if change.count != slot.last_change_count {
        slot.last_change_count = change.count;
        if sample_s > 0.0
            && desired.screen
            && let Some(kind) = crt_rule(
                slot.seed,
                prev_key.as_deref(),
                Some(desired.activity.as_str()),
                change.count,
            )
        {
            slot.boot_at = Some(sample_s);
            slot.boot_span = match kind {
                CrtKind::On => POWER_ON_S,
                CrtKind::Flick => CRT_FLICK_S,
            };
        }
    }

    let mood = mood_for(desired.activity).or(if hover_notice {
        Some(BotMood::Curious)
    } else {
        None
    });
    let motion = bot_motion(desired.activity, desired.motion);
    let body_wants = BodyWants {
        mood,
        hop: !still && (motion.hops || desired.motion == BotMotionLevel::Full),
        resting: still && !hover_notice,
        glancing: held.is_some() || hover_notice,
    };
    let rig_pose = if reduced_motion {
        rest_pose(BotAvatarState::Default)
    } else {
        slot.sim.pose
    };
    let frame = slot.conductor.frame(
        sample_s,
        &change,
        rig_pose,
        Some(slot.sim.heading()),
        body_wants,
    );

    if dt_s > 0.0 && !reduced_motion {
        let step_s = dt_s.max(1e-3);
        let kick_x = (frame.pose.x - slot.prev_pose.x + (frame.pose.yaw - slot.prev_pose.yaw) * 4.0) / step_s;
        let kick_y = (frame.pose.y - slot.prev_pose.y + (frame.pose.pitch - slot.prev_pose.pitch) * 4.0) / step_s;
        slot.lag = step_lag(slot.lag, dt_s.min(0.1), kick_x, kick_y);
    } else if reduced_motion {
        slot.lag = LAG_REST;
    }
    slot.prev_pose = frame.pose;

    let active_flash = slot.changes.flash();
    let flash_progress = active_flash
        .map(|(_, at)| flash_fade((sample_s - at).max(0.0)))
        .unwrap_or(1.0);
    let power_on = slot.boot_at.and_then(|start| {
        if sample_s >= start && sample_s < start + slot.boot_span {
            let phase = (sample_s - start) / slot.boot_span;
            Some(power_on_light(phase))
        } else {
            None
        }
    });

    let active_eyes = change.eyes.or_else(|| slot.changes.shown().pair.clone());

    slot.settled =
        reduced_motion || (still && !hover_notice && held.is_none() && !follows && frame.settled && power_on.is_none());

    BotSnapshot {
        pose: frame.pose,
        eyes: active_eyes,
        tone: slot.changes.shown().tone,
        flash: active_flash,
        flash_progress,
        power_on,
        shut: change.shut,
        lag_x: slot.lag.x,
        lag_y: slot.lag.y,
        hop: frame.hop,
        blinking: frame.blink,
        screen: desired.screen,
        plush: desired.plush,
        s_seconds: sample_s,
        hovered: slot.hovered,
    }
}

fn build_buddy_snapshot(
    slot: &mut BuddySlot,
    now_s: f64,
    reduced_motion: bool,
    hover_notice: bool,
) -> BuddySnapshot {
    let desired = slot.desired;
    let rel_s = if reduced_motion {
        0.0
    } else {
        (now_s - slot.start_s).max(0.0)
    };
    let still = reduced_motion || (is_motionless(desired.activity) && !hover_notice);
    // A glance holds GLANCE_HOLD_MS from when it was given, then his eyes come back (ScreenPet 779-784).
    let held = slot.glance.side_now(now_s);
    let mut ask = PetScreenAsk::from_activity(
        if hover_notice {
            "receiving"
        } else {
            desired.activity.as_str()
        },
        if hover_notice {
            FaceActivity::Receiving
        } else {
            desired.activity
        },
        held.and_then(glance_look),
        desired.motion,
    );
    if !desired.screen {
        ask.eyes = None;
        ask.flash = None;
    }
    let turned = if hover_notice && held.is_none() { Some(GlanceSide::Right) } else { held };
    let seed = slot.seed;
    let crt_cb = move |from: Option<&str>, to: &str, count: u32| crt_rule(seed, from, Some(to), count);
    let frame = slot.conductor.frame(
        rel_s,
        &ask,
        turned,
        if desired.screen { Some(&crt_cb) } else { None },
    );
    let (pose, look) = if reduced_motion {
        (REST_POSE, 0.0)
    } else {
        (frame.rig.pose, frame.rig.look)
    };
    slot.settled = reduced_motion || (still && frame.settled && frame.booting.is_none());

    BuddySnapshot {
        pose,
        look,
        eyes: frame.pair,
        tone: if desired.screen {
            phosphor_for(desired.activity)
        } else {
            Phosphor::Cyan
        },
        flash: slot.conductor.active_flash(),
        flash_progress: slot.conductor.active_flash()
            .map_or(1.0, |(_, at)| flash_fade((rel_s - at).max(0.0))),
        power_on: frame.booting.map(power_on_light),
        shut: 1.0 - frame.squash,
        glass: frame.glass,
        screen: desired.screen,
        plush: desired.plush,
        s_seconds: rel_s,
        hovered: slot.hovered,
    }
}

impl BotStage {
    pub fn begin_frame(&self) {
        let mut inner = self.inner.borrow_mut();
        for slot in inner.bots.values_mut() {
            slot.seen_this_frame = false;
            slot.drawn.this_build = false;
        }
        for slot in inner.buddies.values_mut() {
            slot.seen_this_frame = false;
            slot.drawn.this_build = false;
        }
    }

    /// The view drew the face in `slot_key` with `with`, as `shape`: the drawing the window shows it by
    /// (`rune::canvas_live`), which poll redraws as the face moves, so a moving face rebuilds nothing. None when the
    /// stage has no such face (its snapshot was not asked for this build): draw `shape` as it is.
    pub fn live_face(&self, slot_key: &str, with: FaceDrawing, shape: rune::Shape) -> Option<rune::LiveShape> {
        let mut inner = self.inner.borrow_mut();
        let drawn = match with {
            FaceDrawing::Bot { .. } => &mut inner.bots.get_mut(slot_key)?.drawn,
            FaceDrawing::Buddy { .. } => &mut inner.buddies.get_mut(slot_key)?.drawn,
        };
        Some(drawn.hand_over(with, shape))
    }

    pub fn set_hover(&self, key: &str, hovered: bool) {
        let mut inner = self.inner.borrow_mut();
        let now_s = inner.now_s;
        if let Some(slot) = inner.bots.get_mut(key) {
            slot.hovered = hovered;
            if hovered {
                slot.hover_started_s = Some(now_s);
                slot.hover_until_s = Some(now_s + HOVER_PEEK_S);
                slot.settled = false;
            } else {
                slot.hover_started_s = None;
                slot.hover_noticed_s = None;
                slot.hover_until_s = None;
                slot.pull = None;
                slot.settled = false;
            }
        }
        if let Some(slot) = inner.buddies.get_mut(key) {
            slot.hovered = hovered;
            if hovered {
                slot.hover_started_s = Some(now_s);
                slot.hover_until_s = Some(now_s + HOVER_PEEK_S);
                slot.settled = false;
            } else {
                slot.hover_started_s = None;
                slot.hover_noticed_s = None;
                slot.hover_until_s = None;
                slot.settled = false;
            }
        }
    }

    /// The pointer moved to (`x`, `y`) in the face's own points, on a face drawn `size` square (Bot.tsx's
    /// `pointermove` and `pointerPull`, measured from the face's overscanned box).
    pub fn point_at(&self, key: &str, x: f64, y: f64, size: f64) {
        let mut inner = self.inner.borrow_mut();
        if let Some(slot) = inner.bots.get_mut(key) {
            slot.pull = Some(pointer_pull(face_box(size), x, y));
            if follows(slot) {
                slot.settled = false;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn snapshot_bot(
        &self,
        slot_key: &str,
        id: &str,
        activity: FaceActivity,
        glance: Option<GlanceSide>,
        motion: BotMotionLevel,
        screen: bool,
        plush: bool,
        reduced_motion: bool,
    ) -> BotSnapshot {
        self.snapshot_rig(slot_key, id, activity, glance, motion, screen, plush, reduced_motion, true)
    }

    /// A worn puppet's rig (`PetPuppet.tsx`): a bot's, with a screen (its conductor answers as a screen's), and no
    /// shape's turn, since a drawing has none (PetPuppet makes its `BotAvatarSim` without `setTurn`).
    #[allow(clippy::too_many_arguments)]
    pub fn snapshot_puppet(
        &self,
        slot_key: &str,
        id: &str,
        activity: FaceActivity,
        glance: Option<GlanceSide>,
        motion: BotMotionLevel,
        plush: bool,
        reduced_motion: bool,
    ) -> BotSnapshot {
        self.snapshot_rig(slot_key, id, activity, glance, motion, true, plush, reduced_motion, false)
    }

    #[allow(clippy::too_many_arguments)]
    fn snapshot_rig(
        &self,
        slot_key: &str,
        id: &str,
        activity: FaceActivity,
        glance: Option<GlanceSide>,
        motion: BotMotionLevel,
        screen: bool,
        plush: bool,
        reduced_motion: bool,
        turned: bool,
    ) -> BotSnapshot {
        let mut inner = self.inner.borrow_mut();
        let now_s = inner.now_s;
        let desired = DesiredState {
            activity,
            glance,
            motion,
            screen,
            plush,
        };
        let entry = inner.bots.entry(slot_key.to_owned()).or_insert_with(|| {
            let seed = seed_of(id);
            let shown = shown_face_for(activity, screen);
            let resting_at_start = is_motionless(activity) || reduced_motion;
            let mut sim = BotAvatarSim::new(seed, avatar_state_for(activity, resting_at_start));
            // Bot.tsx: `sim.setTurn(outline.turn)` once, when the rig is made.
            if turned {
                sim.set_turn(outline_of(id).turn);
            }
            let changes = FaceChanges::new(shown);
            let conductor = BodyConductor::new(seed, screen, reduced_motion, resting_at_start);
            let prev_pose = sim.pose;
            let cached = BotSnapshot {
                pose: prev_pose,
                eyes: changes.shown().pair.clone(),
                tone: changes.shown().tone,
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
            let mut slot = BotSlot {
                seed,
                desired,
                sim,
                changes,
                conductor,
                lag: LAG_REST,
                prev_pose,
                start_s: now_s,
                last_step_s: now_s,
                last_change_count: 0,
                boot_at: None,
                boot_span: POWER_ON_S,
                hovered: false,
                hover_started_s: None,
                hover_noticed_s: None,
                hover_until_s: None,
                glance: GlanceHold::default(),
                pull: None,
                seen_this_frame: true,
                settled: resting_at_start,
                cached,
                drawn: Drawn::default(),
            };
            if slot.glance.give(glance, now_s) {
                slot.settled = false;
            }
            slot.cached = first_bot_snapshot(&mut slot, now_s, reduced_motion);
            slot
        });
        entry.seen_this_frame = true;
        // A new glance wakes a face at rest (Bot.tsx 2476-2481: `wake.current?.()`).
        let glanced = entry.glance.give(glance, now_s);
        if glanced {
            entry.settled = false;
        }
        if entry.desired != desired {
            let changed_activity = entry.desired.activity != desired.activity;
            entry.desired = desired;
            if changed_activity || glanced || !is_motionless(desired.activity) {
                entry.settled = false;
            }
            let hover_notice = entry
                .hover_noticed_s
                .zip(entry.hover_until_s)
                .is_some_and(|(_, u)| now_s < u);
            entry.cached = build_bot_snapshot(entry, now_s, 0.0, reduced_motion, hover_notice);
        }
        if reduced_motion && !entry.settled {
            entry.cached = build_bot_snapshot(entry, now_s, 0.0, true, false);
        }
        entry.cached.clone()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn snapshot_buddy(
        &self,
        slot_key: &str,
        id: &str,
        activity: FaceActivity,
        glance: Option<GlanceSide>,
        motion: BotMotionLevel,
        screen: bool,
        plush: bool,
        reduced_motion: bool,
    ) -> BuddySnapshot {
        let mut inner = self.inner.borrow_mut();
        let mut forgotten_name: Option<String> = None;
        let _ = inner.old_tweens.forget_once(|name| {
            forgotten_name = Some(name.to_owned());
        });
        #[cfg(test)]
        if let Some(name) = forgotten_name {
            inner.old_tweens_removed.push(name);
        }
        #[cfg(not(test))]
        let _ = forgotten_name;
        let now_s = inner.now_s;
        let desired = DesiredState {
            activity,
            glance,
            motion,
            screen,
            plush,
        };
        let entry = inner.buddies.entry(slot_key.to_owned()).or_insert_with(|| {
            let seed = seed_of(id);
            let ask = PetScreenAsk::from_activity(
                activity.as_str(),
                activity,
                glance.and_then(glance_look),
                motion,
            );
            let face = screen_face_for(CODEX_BUDDY.id).expect("codex-buddy screen table");
            let conductor = PetScreenConductor::new(face, seed, &ask);
            let resting_at_start = is_motionless(activity) || reduced_motion;
            let cached = BuddySnapshot {
                pose: REST_POSE,
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
            let mut slot = BuddySlot {
                seed,
                desired,
                glance: GlanceHold::default(),
                conductor,
                start_s: now_s,
                last_step_s: now_s,
                hovered: false,
                hover_started_s: None,
                hover_noticed_s: None,
                hover_until_s: None,
                seen_this_frame: true,
                settled: resting_at_start,
                cached,
                drawn: Drawn::default(),
            };
            slot.glance.give(glance, now_s);
            slot.cached = build_buddy_snapshot(&mut slot, now_s, reduced_motion, false);
            slot
        });
        entry.seen_this_frame = true;
        // The hold starts when a glance is given (ScreenPet 738-742). A new glance is also a new `desired`, so the
        // pet is rebuilt and woken below, and its conductor is not settled while he is turned.
        entry.glance.give(glance, now_s);
        if entry.desired != desired {
            let changed_activity = entry.desired.activity != desired.activity;
            entry.desired = desired;
            if changed_activity || !is_motionless(desired.activity) {
                entry.settled = false;
            }
            let hover_notice = entry
                .hover_noticed_s
                .zip(entry.hover_until_s)
                .is_some_and(|(_, u)| now_s < u);
            entry.cached = build_buddy_snapshot(entry, now_s, reduced_motion, hover_notice);
        }
        if reduced_motion && !entry.settled {
            entry.cached = build_buddy_snapshot(entry, now_s, true, false);
        }
        entry.cached.clone()
    }

    pub fn poll(&self, now_s: f64, reduced_motion: bool, focused: bool, tx: &mut Tx) -> bool {
        let mut inner = self.inner.borrow_mut();
        inner.now_s = now_s;
        inner.bots.retain(|_, slot| slot.seen_this_frame);
        inner.buddies.retain(|_, slot| slot.seen_this_frame);

        if reduced_motion {
            if inner.reduced_snap_pending {
                inner.reduced_snap_pending = false;
                for slot in inner.bots.values_mut() {
                    slot.cached = build_bot_snapshot(slot, now_s, 0.0, true, false);
                }
                for slot in inner.buddies.values_mut() {
                    slot.cached = build_buddy_snapshot(slot, now_s, true, false);
                }
                return true;
            }
            return false;
        }
        inner.reduced_snap_pending = true;

        if !focused {
            return false;
        }

        let dwell_s = (NOTICE_DWELL_MS as f64) / 1000.0;
        let received_s = (RECEIVED_GLANCE_MS as f64) / 1000.0;
        let mut any_full = false;
        let mut any_subtle = false;

        for slot in inner.bots.values_mut() {
            if slot.hovered
                && slot.hover_noticed_s.is_none()
                && slot
                    .hover_started_s
                    .is_some_and(|start| now_s - start >= dwell_s)
            {
                slot.hover_noticed_s = Some(now_s);
                slot.hover_until_s = Some(now_s + received_s);
                slot.settled = false;
            }
            let hover_active = slot
                .hover_until_s
                .is_some_and(|until| now_s < until)
                || slot.hovered;
            let active = !is_motionless(slot.desired.activity) || !slot.settled || hover_active;
            if active {
                if slot.desired.motion == BotMotionLevel::Full {
                    any_full = true;
                } else {
                    any_subtle = true;
                }
            }
        }
        for slot in inner.buddies.values_mut() {
            if slot.hovered
                && slot.hover_noticed_s.is_none()
                && slot
                    .hover_started_s
                    .is_some_and(|start| now_s - start >= dwell_s)
            {
                slot.hover_noticed_s = Some(now_s);
                slot.hover_until_s = Some(now_s + received_s);
                slot.settled = false;
            }
            let hover_active = slot
                .hover_until_s
                .is_some_and(|until| now_s < until)
                || slot.hovered;
            let active = !is_motionless(slot.desired.activity) || !slot.settled || hover_active;
            if active {
                if slot.desired.motion == BotMotionLevel::Full {
                    any_full = true;
                } else {
                    any_subtle = true;
                }
            }
        }

        if !any_full && !any_subtle {
            return false;
        }

        let now_ms = now_s * 1000.0;
        let step_due = if any_full {
            inner.beat_full.due(now_ms)
        } else {
            inner.beat_subtle.due(now_ms)
        };
        let step_ms = if any_full {
            FULL_STEP_MS
        } else {
            SUBTLE_STEP_MS
        };
        // The next step comes `step_ms` after the last one, whatever woke this poll. Asked `step_ms` from every poll,
        // each frame something else drew (a hover, a reply) started another chain of wakes beside the steps' own, and
        // the chains piled up to a frame every vsync for a face that steps 30 times a second (speed-2 row 1).
        if step_due {
            inner.stepped_ms = now_ms;
        }
        let next_ms = (inner.stepped_ms + step_ms as f64 - now_ms).clamp(1.0, step_ms as f64);
        tx.wake_after(Duration::from_secs_f64(next_ms / 1000.0));
        inner.wake_count += 1;

        if !step_due {
            return false;
        }

        // A face the view handed over is drawn here, into the drawing the window shows; any other needs the view.
        let mut redrawn = false;
        for (key, slot) in inner.bots.iter_mut() {
            let hover_notice = slot
                .hover_noticed_s
                .zip(slot.hover_until_s)
                .is_some_and(|(_, u)| now_s < u);
            let hover_active = hover_notice || slot.hovered;
            if is_motionless(slot.desired.activity) && slot.settled && !hover_active {
                slot.last_step_s = now_s;
                continue;
            }
            let dt = (now_s - slot.last_step_s).clamp(0.0, 0.1);
            slot.last_step_s = now_s;
            slot.cached = build_bot_snapshot(slot, now_s, dt, false, hover_notice);
            redrawn |= !slot.drawn.publish_bot(key, &slot.cached);
        }
        for (key, slot) in inner.buddies.iter_mut() {
            let hover_notice = slot
                .hover_noticed_s
                .zip(slot.hover_until_s)
                .is_some_and(|(_, u)| now_s < u);
            let hover_active = hover_notice || slot.hovered;
            if is_motionless(slot.desired.activity) && slot.settled && !hover_active {
                slot.last_step_s = now_s;
                continue;
            }
            slot.last_step_s = now_s;
            slot.cached = build_buddy_snapshot(slot, now_s, false, hover_notice);
            redrawn |= !slot.drawn.publish_buddy(key, &slot.cached);
        }
        redrawn
    }

    pub fn wake_count(&self) -> u64 {
        self.inner.borrow().wake_count
    }

    pub fn active_count(&self) -> usize {
        let inner = self.inner.borrow();
        let bots = inner
            .bots
            .values()
            .filter(|s| s.seen_this_frame && (!is_motionless(s.desired.activity) || !s.settled))
            .count();
        let buddies = inner
            .buddies
            .values()
            .filter(|s| s.seen_this_frame && (!is_motionless(s.desired.activity) || !s.settled))
            .count();
        bots + buddies
    }

    /// What a bot was last asked to show, for the window's own tests.
    #[cfg(test)]
    pub fn desired_activity(&self, key: &str) -> Option<FaceActivity> {
        self.inner.borrow().bots.get(key).map(|slot| slot.desired.activity)
    }

    /// Where a bot's gaze is pulled now, (x, y, strength), for the window's own tests.
    /// The seed the face in `key` was made with (`seedOf` of the teammate it was drawn for).
    #[cfg(test)]
    pub fn seed(&self, key: &str) -> Option<f64> {
        self.inner.borrow().bots.get(key).map(|slot| slot.seed)
    }

    #[cfg(test)]
    pub fn pointer_target(&self, key: &str) -> Option<(f64, f64, f64)> {
        self.inner.borrow().bots.get(key).map(|slot| slot.sim.pointer_target())
    }

    #[cfg(test)]
    pub fn last_hop(&self, key: &str) -> Option<bool> {
        self.inner.borrow().bots.get(key).map(|slot| slot.cached.hop)
    }

    #[cfg(test)]
    pub fn buddy_snapshot(&self, key: &str) -> Option<BuddySnapshot> {
        self.inner.borrow().buddies.get(key).map(|slot| slot.cached.clone())
    }

    pub fn slot_count(&self) -> usize {
        let inner = self.inner.borrow();
        inner.bots.len() + inner.buddies.len()
    }
}

#[cfg(test)]
mod renderer_eye_tests {
    use super::*;

    // a-teammates-eyes-say-what-it-is-doing.test.ts, through the actual stage face projection.
    #[test]
    fn is_a_prompt_and_a_block_cursor_at_work_round_eyes_in_thought_carets_when_done_and_eyes_on_you() {
        use FaceActivity::*;
        let all = [Thinking, Working, Delegating, Responding, Waiting, Receiving, Blocked, Done, Idle];
        let shown: Vec<_> = all.into_iter().map(|activity| {
            shown_face_for(activity, true).pair.map(|p| p.concat()).unwrap_or("own".into())
        }).collect();
        assert_eq!(shown, ["••", ">▮", ">▮", "oo", "oo", "oo", "><", "^^", "own"]);
    }

    #[test]
    fn is_drawn_on_a_screen_only_a_face_with_no_screen_keeps_its_own_eyes() {
        assert_eq!(shown_face_for(FaceActivity::Working, true).pair, Some([">".into(), "▮".into()]));
        assert_eq!(shown_face_for(FaceActivity::Working, false).pair, None);
    }
}

#[cfg(test)]
mod first_frame_tests {
    use super::*;

    // Bot.tsx 2007: `draw()` before the clock's first step, so a new bot shows the rig as BotAvatarSim made it (sx = sy =
    // 1). Stepping it at no time gave a working bot its hop's landing squash, and the picker's 64-point ghost lost its hem.
    #[test]
    fn a_new_working_bot_is_first_drawn_upright_as_its_rig_was_made() {
        let stage = BotStage::default();
        let snap = stage.snapshot_bot("k", "ghost", FaceActivity::Working, None, BotMotionLevel::Full, false, false, false);
        assert_eq!((snap.pose.sx, snap.pose.sy), (1.0, 1.0));
    }

    #[test]
    fn its_clock_then_steps_it_into_the_hop() {
        let stage = BotStage::default();
        stage.snapshot_bot("k", "ghost", FaceActivity::Working, None, BotMotionLevel::Full, false, false, false);
        let mut inner = stage.inner.borrow_mut();
        let slot = inner.bots.get_mut("k").unwrap();
        let next = build_bot_snapshot(slot, 0.016, 0.016, false, false);
        assert!(next.pose.sx > 1.1 && next.pose.sy < 0.9, "landing squash on the first step: {:?}", (next.pose.sx, next.pose.sy));
    }
}

#[cfg(test)]
mod head_turn_tests {
    use super::*;

    fn stage_after(id: &str, glance: Option<GlanceSide>, frames: usize) -> BotStage {
        let stage = BotStage::default();
        for _ in 0..frames {
            stage.snapshot_bot("k", id, FaceActivity::Working, glance, BotMotionLevel::Full, false, false, false);
        }
        stage
    }

    // Bot.tsx:2262 sets the turn once from the shape; every library shape has 1 (every-bot-is-drawn-by-the-rig.test.ts:29).
    #[test]
    fn a_bot_keeps_its_shapes_head_turn_through_every_frame() {
        for id in ["clover", "droid", "hopper", "swarm"] {
            let stage = stage_after(id, None, 10);
            let inner = stage.inner.borrow();
            assert_eq!(inner.bots["k"].sim.turn(), outline_of(id).turn, "{id}");
        }
        let stage = stage_after("clover", None, 10);
        assert_eq!(stage.inner.borrow().bots["k"].sim.turn(), 1.0);
    }

    // TeammateBot.tsx:128-133 with Bot.tsx:2393-2395: a side is a pointer at full strength, and it never touches the turn.
    fn glanced(side: GlanceSide) -> (f64, f64, f64, f64) {
        let stage = stage_after("clover", Some(side), 3);
        let inner = stage.inner.borrow();
        let sim = &inner.bots["k"].sim;
        let (x, y, strength) = sim.pointer_target();
        (x, y, strength, sim.turn())
    }

    #[test]
    fn a_glance_to_the_left_points_the_gaze_left_and_leaves_the_turn_alone() {
        assert_eq!(glanced(GlanceSide::Left), (-1.0, 0.0, 1.0, 1.0));
    }

    #[test]
    fn a_glance_to_the_right_points_the_gaze_right_and_leaves_the_turn_alone() {
        assert_eq!(glanced(GlanceSide::Right), (1.0, 0.0, 1.0, 1.0));
    }

    #[test]
    fn a_glance_up_points_the_gaze_up_and_leaves_the_turn_alone() {
        assert_eq!(glanced(GlanceSide::Up), (0.0, -1.0, 1.0, 1.0));
    }

    #[test]
    fn a_glance_down_points_the_gaze_down_and_leaves_the_turn_alone() {
        assert_eq!(glanced(GlanceSide::Down), (0.0, 1.0, 1.0, 1.0));
    }

    /// The stage's own clock, driven as the window drives it: a snapshot each frame, then `poll` at `now_s`.
    fn frame(stage: &BotStage, activity: FaceActivity, glance: Option<GlanceSide>, now_s: f64) {
        let mut tx = rune::Tx::default();
        stage.begin_frame();
        stage.snapshot_bot("k", "clover", activity, glance, BotMotionLevel::Full, false, false, false);
        stage.poll(now_s, false, true, &mut tx);
    }

    fn target(stage: &BotStage) -> (f64, f64, f64) {
        stage.inner.borrow().bots["k"].sim.pointer_target()
    }

    // Bot.tsx:2394 and GLANCE_HOLD_MS (1.5 s): the glance holds from when it is given, then the eyes come back,
    // though the handoff that asked for it is still there.
    #[test]
    fn a_glance_lets_go_one_and_a_half_seconds_after_it_was_given() {
        let stage = BotStage::default();
        frame(&stage, FaceActivity::Working, None, 0.0);
        let mut t = 0.0;
        while t < 2.0 {
            t += 0.05;
            frame(&stage, FaceActivity::Working, Some(GlanceSide::Right), t);
            if t < 0.05 + 1.45 {
                assert_eq!(target(&stage), (1.0, 0.0, 1.0), "still held at {t:.2} s");
            }
        }
        assert_eq!(target(&stage), (0.0, 0.0, 0.0), "let go by 2 s, 1.95 s after it was given");
        // A different glance starts the hold again.
        frame(&stage, FaceActivity::Working, Some(GlanceSide::Left), t + 0.05);
        assert_eq!(target(&stage), (-1.0, 0.0, 1.0));
    }

    // Bot.tsx 2476-2481 wakes the face for a glance: a teammate at rest turns too, and comes to rest again.
    #[test]
    fn a_face_at_rest_wakes_for_a_glance_and_rests_again_once_it_lets_go() {
        let stage = BotStage::default();
        frame(&stage, FaceActivity::Idle, None, 0.0);
        frame(&stage, FaceActivity::Idle, Some(GlanceSide::Up), 0.05);
        frame(&stage, FaceActivity::Idle, Some(GlanceSide::Up), 0.1);
        assert_eq!(target(&stage), (0.0, -1.0, 1.0));
        assert!(!stage.inner.borrow().bots["k"].settled);
        let mut t = 0.1;
        while t < 6.0 {
            t += 0.05;
            frame(&stage, FaceActivity::Idle, Some(GlanceSide::Up), t);
        }
        assert_eq!(target(&stage), (0.0, 0.0, 0.0));
        assert!(stage.inner.borrow().bots["k"].settled, "at rest again");
    }

    // TeammateBot `follows` (NOTICE_DWELL_MS, then until the pointer leaves) and Bot.tsx:2396-2398 `pointerPull`
    // over the face's overscanned box: at 96 points, a pointer 48 points right of the head is looked at straight on,
    // full strength (every-bot-is-drawn-by-the-rig.test.ts:59-64).
    #[test]
    fn a_face_that_has_noticed_the_pointer_looks_toward_it_and_lets_go_when_it_leaves() {
        let stage = BotStage::default();
        frame(&stage, FaceActivity::Working, None, 0.0);
        stage.set_hover("k", true);
        // The head of a 96-point face sits 9.6 points below its middle.
        stage.point_at("k", 48.0 + 48.0, 48.0 + 9.6, 96.0);
        frame(&stage, FaceActivity::Working, None, 0.1);
        assert_eq!(target(&stage), (0.0, 0.0, 0.0), "not noticed before NOTICE_DWELL_MS");
        frame(&stage, FaceActivity::Working, None, 0.4);
        frame(&stage, FaceActivity::Working, None, 0.45);
        let (x, y, strength) = target(&stage);
        assert!((x - 0.5).abs() < 1e-9 && y.abs() < 1e-9 && strength == 1.0, "{x} {y} {strength}");
        stage.point_at("k", 48.0 - 96.0, 48.0 + 9.6, 96.0);
        frame(&stage, FaceActivity::Working, None, 0.5);
        let (x, _, strength) = target(&stage);
        assert!((x + 1.0).abs() < 1e-9 && strength == 1.0, "it follows the pointer to the left: {x} {strength}");
        stage.set_hover("k", false);
        frame(&stage, FaceActivity::Working, None, 0.55);
        assert_eq!(target(&stage), (0.0, 0.0, 0.0));
    }
}

/// A pet's glance (PetSprite.tsx 117-120 and 183-187; ScreenPet 738-742 and 779-784): it holds `GLANCE_HOLD_MS` from
/// when it is given, then his eyes come back, though the handoff that asked for it is still there.
#[cfg(test)]
mod pet_glance_tests {
    use super::*;

    /// The stage's own clock, driven as the window drives it: a snapshot each frame, then `poll` at `now_s`.
    fn frame(stage: &BotStage, activity: FaceActivity, glance: Option<GlanceSide>, now_s: f64) {
        let mut tx = rune::Tx::default();
        stage.begin_frame();
        stage.snapshot_buddy("p", "pip", activity, glance, BotMotionLevel::Full, true, false, false);
        stage.poll(now_s, false, true, &mut tx);
    }

    /// Frames every 50 ms from `from` until `until`, asking for `glance` each time; the time reached.
    fn run(stage: &BotStage, activity: FaceActivity, glance: Option<GlanceSide>, from: f64, until: f64) -> f64 {
        let mut t = from;
        while t < until {
            t += 0.05;
            frame(stage, activity, glance, t);
        }
        t
    }

    fn look(stage: &BotStage) -> f64 {
        stage.inner.borrow().buddies["p"].cached.look
    }

    fn settled(stage: &BotStage) -> bool {
        stage.inner.borrow().buddies["p"].settled
    }

    #[test]
    fn a_pets_glance_holds_one_and_a_half_seconds_then_his_eyes_come_back() {
        let stage = BotStage::default();
        frame(&stage, FaceActivity::Working, None, 0.0);
        let t = run(&stage, FaceActivity::Working, Some(GlanceSide::Left), 0.0, 1.0);
        assert!(look(&stage) < -0.9, "looking left at {t:.2} s: {}", look(&stage));
        let t = run(&stage, FaceActivity::Working, Some(GlanceSide::Left), t, 1.5);
        assert!(look(&stage) < -0.9, "still held at {t:.2} s: {}", look(&stage));
        let t = run(&stage, FaceActivity::Working, Some(GlanceSide::Left), t, 3.0);
        assert!(look(&stage).abs() < 0.02, "back by {t:.2} s though still asked: {}", look(&stage));
    }

    #[test]
    fn a_different_glance_starts_a_pets_hold_again() {
        let stage = BotStage::default();
        frame(&stage, FaceActivity::Working, None, 0.0);
        let t = run(&stage, FaceActivity::Working, Some(GlanceSide::Left), 0.0, 3.0);
        assert!(look(&stage).abs() < 0.02, "let go: {}", look(&stage));
        let t = run(&stage, FaceActivity::Working, Some(GlanceSide::Right), t, t + 0.7);
        assert!(look(&stage) > 0.9, "looking right at {t:.2} s: {}", look(&stage));
    }

    #[test]
    fn a_pet_at_rest_wakes_for_a_glance_and_rests_again_once_it_lets_go() {
        let stage = BotStage::default();
        let t = run(&stage, FaceActivity::Idle, None, -0.05, 8.0);
        assert!(settled(&stage), "at rest before the glance");
        let t = run(&stage, FaceActivity::Idle, Some(GlanceSide::Right), t, t + 0.05);
        assert!(!settled(&stage), "woken by the glance");
        let t = run(&stage, FaceActivity::Idle, Some(GlanceSide::Right), t, t + 0.65);
        assert!(look(&stage) > 0.9, "turned at {t:.2} s: {}", look(&stage));
        let t = run(&stage, FaceActivity::Idle, Some(GlanceSide::Right), t, t + 8.0);
        assert!(look(&stage).abs() < 0.02, "back at {t:.2} s: {}", look(&stage));
        assert!(settled(&stage), "at rest again");
    }

    // case: are removed from this computer, once a launch, the store they were kept in by name
    #[test]
    fn codex_buddy_forgets_old_tweens_once_per_launch_when_drawn() {
        let stage = BotStage::default();
        frame(&stage, FaceActivity::Working, None, 0.0);
        frame(&stage, FaceActivity::Working, None, 0.05);
        assert_eq!(stage.inner.borrow().old_tweens_removed, vec!["locust-pet-tweens"]);
    }
}


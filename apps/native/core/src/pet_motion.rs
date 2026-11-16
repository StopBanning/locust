//! What a pet does for what its teammate is doing (Electron `renderer/src/petMotion.ts`, 0.563-0.564).
//!
//! Colin, 2026-10-03: "just clarity these should be the exact same as teammates". So a pet says everything a bot says,
//! in its own sheet's rows -- OpenPets' own map from an agent's state to a row (thinking reviews, work runs, waiting
//! waits, success jumps, an error fails), and Locust's rules for when a face moves:
//!
//! - idle: still, in its resting pose -- a bot at rest keeps still too;
//! - thinking: the review row, looping;
//! - working, delegating, answering: the running row, which is work in place, looping;
//! - receiving a message: a wave, twice, then rest;
//! - waiting on you: the waiting row, looping, inside the amber ring;
//! - done: the jump, twice -- the done hop -- then rest;
//! - stuck: the failure, twice, then held on its lowest moment, so a stuck teammate looks stuck until someone helps.
//!
//! This is the pure half: which row, which cell at a time, when it next changes. Drawing the sheet is the UI's.
//! A pet whose drawings have been read as moves (Electron `petRoutines.ts`: Codex Buddy, with his screen) does its own
//! instead.

use crate::bot_rig::{FaceActivity, GlanceSide};
use crate::pets::{PET_NEUTRAL, PetState, pet_gaze_cell, pet_row};

/// The row a pet plays for what its teammate is doing (`petStateFor`).
pub fn pet_state_for(activity: FaceActivity) -> PetState {
    match activity {
        FaceActivity::Thinking => PetState::Review,
        FaceActivity::Working | FaceActivity::Delegating | FaceActivity::Responding => PetState::Running,
        FaceActivity::Receiving => PetState::Waving,
        FaceActivity::Waiting => PetState::Waiting,
        FaceActivity::Done => PetState::Jumping,
        FaceActivity::Blocked => PetState::Failed,
        FaceActivity::Idle => PetState::Idle,
    }
}

/// A cell of the sheet, and the state it shows (a finished wave or jump shows rest).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PetCell {
    pub row: u32,
    pub column: u32,
    pub shown: PetState,
    /// Nothing more will change: the clock can stop.
    pub settled: bool,
}

/// The resting pose: version 2's own (row 0, column 6), version 1's first idle frame. `rows` is 11 or 9.
pub fn resting_cell(rows: u8) -> PetCell {
    if rows == 11 {
        PetCell { row: PET_NEUTRAL.0, column: PET_NEUTRAL.1, shown: PetState::Idle, settled: true }
    } else {
        PetCell { row: pet_row(PetState::Idle).row, column: 0, shown: PetState::Idle, settled: true }
    }
}

/// The failure's lowest moment, held while a teammate stays stuck: the middle of its row.
pub fn held_failure_cell() -> PetCell {
    let failed = pet_row(PetState::Failed);
    PetCell { row: failed.row, column: failed.frames / 2, shown: PetState::Failed, settled: true }
}

/// The cell to draw `elapsed` ms after the state began. Still (reduced motion, or a face that keeps still): the resting
/// pose, the held failure, or the state's first frame.
pub fn pet_cell_at(state: PetState, elapsed: f64, rows: u8, still: bool) -> PetCell {
    if state == PetState::Idle {
        return resting_cell(rows);
    }
    let spec = pet_row(state);
    if still {
        return if state == PetState::Failed { held_failure_cell() } else { PetCell { row: spec.row, column: 0, shown: state, settled: true } };
    }
    let frame_ms = spec.ms / f64::from(spec.frames);
    let time = elapsed.max(0.0);
    if let Some(plays) = spec.plays
        && time >= spec.ms * f64::from(plays)
    {
        return if state == PetState::Failed { held_failure_cell() } else { resting_cell(rows) };
    }
    PetCell { row: spec.row, column: ((time / frame_ms).floor() as u32) % spec.frames, shown: state, settled: false }
}

/// How long one drawing melts into the next, at most (0.564): a pet's rows step 6-8 drawings a movement, bots move at 30 a
/// second, so each new drawing appears with the last one fading off it over about half a frame, never longer than this.
pub const PET_FADE_MAX_MS: u32 = 80;

/// How long the drawing before this one fades off it: half a frame, at most `PET_FADE_MAX_MS`. None at rest or when still.
pub fn pet_fade_ms(state: PetState, still: bool) -> u32 {
    if still || state == PetState::Idle {
        return 0;
    }
    let spec = pet_row(state);
    PET_FADE_MAX_MS.min((spec.ms / f64::from(spec.frames) / 2.0).round() as u32)
}

/// How long until the next cell, or None once nothing more will change.
pub fn pet_next_change_in(state: PetState, elapsed: f64, still: bool) -> Option<f64> {
    if state == PetState::Idle || still {
        return None;
    }
    let spec = pet_row(state);
    let frame_ms = spec.ms / f64::from(spec.frames);
    let time = elapsed.max(0.0);
    if let Some(plays) = spec.plays
        && time >= spec.ms * f64::from(plays)
    {
        return None;
    }
    Some(frame_ms - (time % frame_ms))
}

/// A pet looking toward a teammate it just handed a message to: a version 2 sheet's head turned that way -- right 4, left
/// 12, up 0. Not down: some pets' makers drew the back of the head for it. None on a version 1 sheet.
pub fn pet_glance_cell(side: GlanceSide, rows: u8) -> Option<PetCell> {
    if rows != 11 {
        return None;
    }
    let index = match side {
        GlanceSide::Right => 4,
        GlanceSide::Left => 12,
        GlanceSide::Up => 0,
        GlanceSide::Down => return None,
    };
    let (row, column) = pet_gaze_cell(index)?;
    Some(PetCell { row, column, shown: PetState::Idle, settled: true })
}

/// Below this mean brightness (0-1) of its painted pixels a pet is drawn with a rim. Measured 0.564: Reaper 0.08, Cabin
/// 0.12, Dot and the Yeelight bot 0.20; the next darkest, Meowbot, 0.36 reads unaided.
pub const DARK_PET: f64 = 0.25;

/// The mean brightness (0-1) of the pixels that are mostly opaque, from `rgba` bytes; None when none are.
pub fn painted_brightness(rgba: &[u8]) -> Option<f64> {
    let (mut sum, mut count) = (0.0_f64, 0_u32);
    for px in rgba.chunks_exact(4) {
        if px[3] <= 128 {
            continue;
        }
        sum += (0.2126 * f64::from(px[0]) + 0.7152 * f64::from(px[1]) + 0.0722 * f64::from(px[2])) / 255.0;
        count += 1;
    }
    (count > 0).then(|| sum / f64::from(count))
}

#[cfg(test)]
#[path = "pet_motion_tests.rs"]
mod tests;

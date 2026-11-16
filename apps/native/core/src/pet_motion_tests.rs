//! a-pet-plays-what-its-teammate-is-doing.test.ts (0.563, 0.564): every case with its own numbers.

use super::*;
use crate::pets::{PET_NEUTRAL, PetState, pet_row};

fn cell(state: PetState, elapsed: f64, rows: u8, still: bool) -> PetCell {
    pet_cell_at(state, elapsed, rows, still)
}

// "what a pet does for each thing its teammate does: maps every activity to a row of its sheet".
#[test]
fn every_activity_maps_to_a_row_of_the_sheet() {
    let map = [
        (FaceActivity::Idle, PetState::Idle),
        (FaceActivity::Thinking, PetState::Review),
        (FaceActivity::Working, PetState::Running),
        (FaceActivity::Delegating, PetState::Running),
        (FaceActivity::Responding, PetState::Running),
        (FaceActivity::Receiving, PetState::Waving),
        (FaceActivity::Waiting, PetState::Waiting),
        (FaceActivity::Done, PetState::Jumping),
        (FaceActivity::Blocked, PetState::Failed),
    ];
    assert_eq!(map.len(), FaceActivity::ALL.len(), "every activity is in the map");
    for (activity, state) in map {
        assert_eq!(pet_state_for(activity), state, "{activity:?}");
    }
}

// "the frame a pet shows: rests in its resting pose -- version 2's own, version 1's first idle frame".
#[test]
fn a_pet_at_rest_shows_its_resting_pose() {
    let v2 = cell(PetState::Idle, 12_345.0, 11, false);
    assert_eq!((v2.row, v2.column, v2.settled), (PET_NEUTRAL.0, PET_NEUTRAL.1, true));
    assert_eq!((PET_NEUTRAL.0, PET_NEUTRAL.1), (0, 6));
    let v1 = cell(PetState::Idle, 12_345.0, 9, false);
    assert_eq!((v1.row, v1.column, v1.settled), (0, 0, true));
    assert_eq!(pet_next_change_in(PetState::Idle, 0.0, false), None);
}

// "loops work and thought through their rows at OpenPets' pace".
#[test]
fn work_and_thought_loop_through_their_rows_at_openpets_pace() {
    let running = pet_row(PetState::Running);
    let step = running.ms / f64::from(running.frames);
    let first = cell(PetState::Running, 0.0, 11, false);
    assert_eq!((first.row, first.column, first.settled), (running.row, 0, false));
    assert_eq!(cell(PetState::Running, step * 2.0 + 1.0, 11, false).column, 2);
    // Round again after one pass.
    assert_eq!(cell(PetState::Running, running.ms + 1.0, 11, false).column, 0);
    let next = pet_next_change_in(PetState::Running, step / 2.0, false).unwrap();
    assert!((next - step / 2.0).abs() < 0.005, "{next} vs {}", step / 2.0);
    assert_eq!(cell(PetState::Review, 0.0, 11, false).row, pet_row(PetState::Review).row);
    assert_eq!(cell(PetState::Waiting, 0.0, 11, false).row, pet_row(PetState::Waiting).row);
}

// "waves and jumps twice, then rests".
#[test]
fn a_pet_waves_and_jumps_twice_then_rests() {
    for state in [PetState::Waving, PetState::Jumping] {
        let spec = pet_row(state);
        let second = cell(state, spec.ms + 1.0, 11, false);
        assert_eq!((second.row, second.shown, second.settled), (spec.row, state, false), "{state:?} second pass");
        let after = cell(state, spec.ms * 2.0 + 1.0, 11, false);
        assert_eq!((after.row, after.column, after.shown, after.settled), (PET_NEUTRAL.0, PET_NEUTRAL.1, PetState::Idle, true), "{state:?} after");
        assert_eq!(pet_next_change_in(state, spec.ms * 2.0 + 1.0, false), None, "{state:?}");
    }
}

// "fails twice, then holds its lowest moment while the teammate stays stuck".
#[test]
fn a_pet_fails_twice_then_holds_its_lowest_moment() {
    let spec = pet_row(PetState::Failed);
    let held = cell(PetState::Failed, spec.ms * 2.0 + 1.0, 11, false);
    assert_eq!((held.row, held.column, held.shown, held.settled), (spec.row, spec.frames / 2, PetState::Failed, true));
    assert_eq!(held.column, 4);
    assert_eq!(cell(PetState::Failed, 60_000.0, 9, false), held);
}

// "holds still for reduced motion: a pose, never a row half played".
#[test]
fn reduced_motion_holds_a_pose_never_a_row_half_played() {
    let running = cell(PetState::Running, 400.0, 11, true);
    assert_eq!((running.row, running.column, running.settled), (pet_row(PetState::Running).row, 0, true));
    assert_eq!(cell(PetState::Failed, 0.0, 11, true).column, pet_row(PetState::Failed).frames / 2);
    assert_eq!(pet_next_change_in(PetState::Running, 400.0, true), None);
}

// "a pet glancing at a teammate: turns a version 2 head toward it, left, right or up".
#[test]
fn a_version_2_pet_turns_its_head_toward_a_teammate_left_right_or_up() {
    let at = |side| pet_glance_cell(side, 11).map(|c| (c.row, c.column));
    assert_eq!(at(GlanceSide::Right), Some((9, 4)));
    assert_eq!(at(GlanceSide::Left), Some((10, 4)));
    assert_eq!(at(GlanceSide::Up), Some((9, 0)));
    let c = pet_glance_cell(GlanceSide::Right, 11).unwrap();
    assert_eq!((c.shown, c.settled), (PetState::Idle, true));
}

// "keeps its pose where its sheet has no head to turn, or would show the back of it".
#[test]
fn a_pet_keeps_its_pose_where_there_is_no_head_to_turn_or_only_the_back_of_it() {
    assert_eq!(pet_glance_cell(GlanceSide::Right, 9), None);
    assert_eq!(pet_glance_cell(GlanceSide::Down, 11), None);
}

// "melts each drawing into the next over about half a frame, never longer than PET_FADE_MAX_MS".
#[test]
fn each_drawing_melts_into_the_next_over_half_a_frame_at_most_the_maximum() {
    assert_eq!(PET_FADE_MAX_MS, 80);
    let want = [(PetState::Running, 68), (PetState::Review, 80), (PetState::Waving, 80), (PetState::Jumping, 80), (PetState::Failed, 76), (PetState::Waiting, 80)];
    for (state, expected) in want {
        let spec = pet_row(state);
        let frame = spec.ms / f64::from(spec.frames);
        assert_eq!(pet_fade_ms(state, false), PET_FADE_MAX_MS.min((frame / 2.0).round() as u32), "{state:?}");
        assert_eq!(pet_fade_ms(state, false), expected, "{state:?}");
        assert!(pet_fade_ms(state, false) > 0, "{state:?}");
    }
}

// "fades nothing at rest or when still: nothing moves there".
#[test]
fn nothing_fades_at_rest_or_when_still() {
    assert_eq!(pet_fade_ms(PetState::Idle, false), 0);
    assert_eq!(pet_fade_ms(PetState::Running, true), 0);
}

// "measures how dark a pet is from its opaque pixels only".
#[test]
fn how_dark_a_pet_is_comes_from_its_opaque_pixels_only() {
    let pixels = |rgba: &[[u8; 4]]| -> Vec<u8> { rgba.iter().flatten().copied().collect() };
    // A black cloak on a clear ground: dark, whatever the clear pixels hold.
    let reaper = painted_brightness(&pixels(&[[10, 10, 12, 255], [20, 18, 22, 255], [255, 255, 255, 0], [255, 255, 255, 40]])).unwrap();
    assert!(reaper < DARK_PET, "{reaper}");
    let cloud = painted_brightness(&pixels(&[[250, 240, 220, 255], [240, 230, 210, 255]])).unwrap();
    assert!(cloud > DARK_PET, "{cloud}");
    assert_eq!(painted_brightness(&pixels(&[[0, 0, 0, 0]])), None);
    assert_eq!(DARK_PET, 0.25);
}

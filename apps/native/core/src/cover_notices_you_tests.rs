//! the-cover-notices-you.test.ts: the sleeping Hopper's wake/doze rule and replaying the cover glass on click.
use super::*;

#[test]
// case: sleeps through a pointer far away, costing nothing: the same value back
fn sleeps_through_a_pointer_far_away() {
    assert_eq!(hopper_wakes(ASLEEP, Some(WAKE_WITHIN + 2.0), 1_000), ASLEEP);
    assert_eq!(hopper_wakes(ASLEEP, None, 1_000), ASLEEP);
}

#[test]
// case: wakes when a pointer comes near
fn wakes_when_a_pointer_comes_near() {
    assert_eq!(
        hopper_wakes(ASLEEP, Some(WAKE_WITHIN - 0.1), 1_000),
        Wakefulness { awake: true, away_since: None }
    );
}

#[test]
// case: stays awake a while after the pointer goes, then dozes off
fn stays_awake_a_while_after_the_pointer_goes_then_dozes_off() {
    let awake = hopper_wakes(ASLEEP, Some(0.5), 1_000);
    let left = hopper_wakes(awake, Some(WAKE_WITHIN + 1.0), 2_000);
    assert_eq!(left, Wakefulness { awake: true, away_since: Some(2_000) });
    // Moving about far away does not restart the count.
    assert_eq!(hopper_wakes(left, Some(WAKE_WITHIN + 3.0), 3_000), left);
    assert_eq!(hopper_wakes(left, None, 2_000 + DOZE_AFTER_MS - 1), left);
    assert_eq!(hopper_wakes(left, None, 2_000 + DOZE_AFTER_MS), ASLEEP);
}

#[test]
// case: stays awake for a pointer that comes back before it dozes
fn stays_awake_for_a_pointer_that_comes_back_before_it_dozes() {
    let left = Wakefulness { awake: true, away_since: Some(2_000) };
    assert_eq!(
        hopper_wakes(left, Some(0.4), 3_000),
        Wakefulness { awake: true, away_since: None }
    );
}

#[test]
// case: measures from the middle of its box, in its own widths
fn measures_from_the_middle_of_its_box_in_its_own_widths() {
    let b = CoverBox { x: 100.0, y: 100.0, width: 80.0, height: 80.0 };
    assert_eq!(widths_from(b, 140.0, 140.0), 0.0);
    assert_eq!(widths_from(b, 300.0, 140.0), 2.0);
}

#[test]
// case: lights it again once it has lit, as the tube allows
fn lights_it_again_once_it_has_lit_as_the_tube_allows() {
    assert!(replays_on_ask(LightingPlan::Repeat, 1));
    assert!(replays_on_ask(LightingPlan::Once, 1));
}

#[test]
// case: never where the tube never lights -- Off, or reduced motion -- nor before its first lighting
fn never_where_the_tube_never_lights_nor_before_first_lighting() {
    assert!(!replays_on_ask(LightingPlan::None, 3));
    assert!(!replays_on_ask(LightingPlan::Repeat, 0));
    assert_eq!(lighting_plan("off", false), LightingPlan::None);
    assert_eq!(lighting_plan("full", true), LightingPlan::None);
}

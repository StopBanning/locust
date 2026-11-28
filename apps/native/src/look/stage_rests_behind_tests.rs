//! Electron `a-face-rests-behind-other-windows.test.ts` on the production `BotStage`:
//! actual moving bots and Codex Buddy pause completely while the window is behind others,
//! resume without jumping for the paused duration, draw once when started unfocused, and
//! ask for nothing once removed.

use super::*;

fn step_both(stage: &BotStage, now_s: f64, focused: bool) -> (bool, BotSnapshot, BuddySnapshot) {
    let mut tx = rune::Tx::default();
    stage.begin_frame();
    let bot = stage.snapshot_bot(
        "bot-slot",
        "tm_wren",
        FaceActivity::Working,
        None,
        BotMotionLevel::Full,
        true,
        false,
        false,
    );
    let buddy = stage.snapshot_buddy(
        "buddy-slot",
        "tm_buddy",
        FaceActivity::Working,
        None,
        BotMotionLevel::Full,
        true,
        false,
        false,
    );
    let redrawn = stage.poll(now_s, false, focused, &mut tx);
    let bot_after = stage.inner.borrow().bots["bot-slot"].cached.clone();
    let buddy_after = stage.inner.borrow().buddies["buddy-slot"].cached.clone();
    let _ = (bot, buddy);
    (redrawn, bot_after, buddy_after)
}

// case: asks for no frames and draws nothing more, holding where it was
#[test]
fn production_stage_asks_for_no_frames_and_holds_pose_behind_other_windows() {
    let stage = BotStage::default();
    step_both(&stage, 0.0, true);
    let (redrawn, bot1, buddy1) = step_both(&stage, 0.034, true);
    assert!(redrawn, "moved on focused step");
    assert!((bot1.s_seconds - 0.034).abs() < 1e-6);
    assert!((buddy1.s_seconds - 0.034).abs() < 1e-6);
    let wakes_before = stage.wake_count();

    let (r_away1, bot_away1, buddy_away1) = step_both(&stage, 5.0, false);
    let (r_away2, bot_away2, buddy_away2) = step_both(&stage, 9.0, false);
    assert!(!r_away1 && !r_away2, "no redraws while away");
    assert_eq!(stage.wake_count(), wakes_before, "no frames asked for while away");
    assert_eq!(bot_away1.s_seconds, bot1.s_seconds);
    assert_eq!(bot_away2.s_seconds, bot1.s_seconds);
    assert_eq!(buddy_away1.s_seconds, buddy1.s_seconds);
    assert_eq!(buddy_away2.s_seconds, buddy1.s_seconds);
}

// case: picks up when the window comes back, with no jump for the time it was away
#[test]
fn production_stage_picks_up_on_focus_with_no_jump_for_the_time_away() {
    let stage = BotStage::default();
    step_both(&stage, 0.0, true);
    let (_, bot1, buddy1) = step_both(&stage, 0.034, true);
    assert!((bot1.s_seconds - 0.034).abs() < 1e-6);
    assert!((buddy1.s_seconds - 0.034).abs() < 1e-6);

    // Go away at 0.034s, stay away for 5 seconds, then focus returns at 5.034s.
    step_both(&stage, 0.034, false);
    step_both(&stage, 5.034, false);
    let wakes_before = stage.wake_count();
    let (_, bot_back, buddy_back) = step_both(&stage, 5.034, true);
    assert!(stage.wake_count() > wakes_before, "asks for a frame again on focus return");
    assert!(
        (bot_back.s_seconds - 0.034).abs() < 1e-6,
        "bot did not jump 5s on return: {}",
        bot_back.s_seconds
    );
    assert!(
        (buddy_back.s_seconds - 0.034).abs() < 1e-6,
        "buddy did not jump 5s on return: {}",
        buddy_back.s_seconds
    );

    // One frame later (34ms): steps forward by one frame's worth (to 0.068s), not 5 seconds.
    let (redrawn, bot_next, buddy_next) = step_both(&stage, 5.068, true);
    assert!(redrawn);
    assert!((bot_next.s_seconds - 0.068).abs() < 1e-6, "bot_next={}", bot_next.s_seconds);
    assert!((buddy_next.s_seconds - 0.068).abs() < 1e-6, "buddy_next={}", buddy_next.s_seconds);
}

// case: started behind other windows, is drawn once, and moves only once its window is in front
#[test]
fn production_stage_started_behind_other_windows_draws_once_then_moves_on_focus() {
    let stage = BotStage::default();
    let (r0, bot0, buddy0) = step_both(&stage, 0.0, false);
    assert!(!r0);
    assert_eq!(stage.wake_count(), 0);
    assert_eq!(bot0.s_seconds, 0.0);
    assert_eq!(buddy0.s_seconds, 0.0);

    let (r1, bot1, _) = step_both(&stage, 2.0, false);
    assert!(!r1);
    assert_eq!(stage.wake_count(), 0);
    assert_eq!(bot1.s_seconds, 0.0);

    // Focus comes in front at 2.0s; one frame later at 2.034s it moves from 0.0 to 0.034s.
    step_both(&stage, 2.0, true);
    assert!(stage.wake_count() > 0);
    let (r2, bot2, buddy2) = step_both(&stage, 2.034, true);
    assert!(r2);
    assert!((bot2.s_seconds - 0.034).abs() < 1e-6, "{}", bot2.s_seconds);
    assert!((buddy2.s_seconds - 0.034).abs() < 1e-6, "{}", buddy2.s_seconds);
}

// case: lets go of its window when it stops, and asks for nothing after
#[test]
fn production_stage_stops_asking_once_faces_leave_the_frame() {
    let stage = BotStage::default();
    step_both(&stage, 0.0, true);
    step_both(&stage, 0.034, true);
    assert_eq!(stage.slot_count(), 2);

    // Next frame renders no faces; poll drops the slots and asks for no more frames, whatever focus does.
    let mut tx = rune::Tx::default();
    stage.begin_frame();
    let wakes_before = stage.wake_count();
    assert!(!stage.poll(0.068, false, true, &mut tx));
    assert_eq!(stage.slot_count(), 0);
    assert!(!stage.poll(0.102, false, false, &mut tx));
    assert!(!stage.poll(0.136, false, true, &mut tx));
    assert_eq!(stage.wake_count(), wakes_before);
}

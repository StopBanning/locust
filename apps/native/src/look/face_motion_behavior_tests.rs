//! Electron `only-the-face-you-talk-to-hops.test.tsx` on the production `BotStage`:
//! full-motion working bots hop on the rig and do NOT bounce, while subtle working/thinking
//! bots bounce subtly without hopping or flipping, and waiting/idle bots neither bounce nor hop.

use super::*;

fn step(stage: &BotStage, key: &str, activity: FaceActivity, motion: BotMotionLevel, now_s: f64) -> BotSnapshot {
    let mut tx = rune::Tx::default();
    stage.begin_frame();
    stage.snapshot_bot(key, "tm_yurt", activity, None, motion, false, false, false);
    stage.poll(now_s, false, true, &mut tx);
    stage.inner.borrow().bots[key].cached.clone()
}

// case: does not bounce as well -- it hops
#[test]
fn full_motion_bot_does_not_bounce_as_well_it_hops() {
    let stage = BotStage::default();
    step(&stage, "full", FaceActivity::Working, BotMotionLevel::Full, 0.0);
    let snap = step(&stage, "full", FaceActivity::Working, BotMotionLevel::Full, 0.345);
    {
        let inner = stage.inner.borrow();
        let slot = &inner.bots["full"];
        assert!(!slot.bouncing, "full motion does not bounce");
        assert_eq!(slot.bounce_lift, 0.0);
        assert_eq!(slot.sim_state, BotAvatarState::Working, "full motion hops in Working state");
        assert!(slot.jump_every > 0.0, "full motion flips on the rig's schedule");
    }
    // At work in Full motion, the rig's hop squashes/stretches the body.
    assert!((snap.pose.sx - 1.0).abs() > 0.01 || (snap.pose.sy - 1.0).abs() > 0.01);

    // By contrast, a subtle working (or thinking, via worn_activity) bot bounces and does not hop or flip,
    // while a waiting bot does not bounce.
    let subtle = BotStage::default();
    step(&subtle, "work", FaceActivity::Working, BotMotionLevel::Subtle, 0.0);
    step(&subtle, "work", FaceActivity::Working, BotMotionLevel::Subtle, 0.345);
    {
        let inner = subtle.inner.borrow();
        let slot = &inner.bots["work"];
        assert!(slot.bouncing, "subtle working bot bounces");
        assert!(slot.bounce_lift > 6.5, "lifted near 7% peak: {}", slot.bounce_lift);
        assert_eq!(slot.sim_state, BotAvatarState::Default, "looks around instead of hopping");
        assert_eq!(slot.jump_every, 0.0, "never flips");
    }

    let thinking = BotStage::default();
    step(&thinking, "think", FaceActivity::Thinking, BotMotionLevel::Subtle, 0.0);
    step(&thinking, "think", FaceActivity::Thinking, BotMotionLevel::Subtle, 0.345);
    assert!(thinking.inner.borrow().bots["think"].bouncing, "thinking wears the working face and bounces");

    let waiting = BotStage::default();
    step(&waiting, "wait", FaceActivity::Waiting, BotMotionLevel::Subtle, 0.0);
    step(&waiting, "wait", FaceActivity::Waiting, BotMotionLevel::Subtle, 0.345);
    assert!(!waiting.inner.borrow().bots["wait"].bouncing, "waiting does not bounce");
}

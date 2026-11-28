//! A full-motion face uses the working rig, not the side face's small CSS bounce.
use super::*;

// case: does not bounce as well -- it hops
#[test]
fn sweep_full_working_face_draws_the_rig_hop_without_a_second_bounce() {
    let motion = bot_motion(FaceActivity::Working, BotMotionLevel::Full);
    assert!(!motion.bounces, "the full face adds no side-face bounce");
    let stage = BotStage::default();
    let frame = || stage.snapshot_bot(
        "sweep-full-face", "ghost", FaceActivity::Working, None,
        BotMotionLevel::Full, false, false, false,
    );
    frame();
    // The body conductor can still move a face if the stage gives it an idle rig.
    // Check what the live stage selects, as well as the resulting movement below.
    assert_eq!(
        stage.inner.borrow().bots["sweep-full-face"].sim.state,
        BotAvatarState::Working,
        "the full face uses the moving working rig, not the idle rig",
    );
    let (mut highest, mut widest) = (f64::INFINITY, 0.0_f64);
    for step in 1..=420 {
        stage.poll(step as f64 / 30.0, false, true, &mut rune::Tx::default());
        let pose = frame().pose;
        highest = highest.min(pose.y);
        widest = widest.max(pose.yaw.sin().atan2(pose.yaw.cos()).abs());
    }
    assert!(highest < -1.0, "the actual rig hops (height {highest})");
    assert!(widest > 2.5, "the full working rig makes its own turn (yaw {widest})");
}

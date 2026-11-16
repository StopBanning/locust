//! Exact cases from a-teammate-at-work-goes-round-its-faces.test.ts.
use crate::bot_rig::*;
use crate::bot_spec::Phosphor;
use std::collections::HashSet;

#[test]
fn wears_the_working_face_whatever_its_run_is_doing_this_second() {
    for activity in [FaceActivity::Thinking, FaceActivity::Working, FaceActivity::Delegating, FaceActivity::Responding] {
        assert!(at_work(activity));
        assert_eq!(worn_activity(activity), FaceActivity::Working);
        assert_eq!(everyday_face(worn_activity(activity), EverydayFaceLife::default()).eyes, Some([">", "▮"]));
        assert_eq!(phosphor_for(worn_activity(activity)), Phosphor::Cyan);
    }
}
#[test]
fn keeps_the_faces_that_mean_something_to_look_at_waiting_on_you_listening_stuck_done_at_rest() {
    for activity in [FaceActivity::Waiting, FaceActivity::Receiving, FaceActivity::Blocked, FaceActivity::Done, FaceActivity::Idle] {
        assert!(!at_work(activity));
        assert_eq!(worn_activity(activity), activity);
    }
    assert_eq!(eye_glyphs_for(worn_activity(FaceActivity::Blocked)), Some([">", "<"]));
    assert_eq!(eye_glyphs_for(worn_activity(FaceActivity::Done)), Some(["^", "^"]));
    assert_eq!(phosphor_for(worn_activity(FaceActivity::Waiting)), Phosphor::Amber);
}
#[test]
fn has_a_beat_of_each_other_working_face_and_none_of_the_faces_that_mean_something() {
    let names: HashSet<_> = WORKING_BEATS.iter().map(|b| b.name).collect();
    assert_eq!(names.len(), WORKING_BEATS.len());
    assert!(WORKING_BEATS.len() >= 4);
    let faces: Vec<_> = WORKING_BEATS.iter().map(|b| b.eyes.join("")).collect();
    assert!(faces.iter().any(|f| f == "••"));
    for meaning in ["^^", "><", "||", "cc", ">▮"] { assert!(!faces.iter().any(|f| f == meaning)); }
    for beat in WORKING_BEATS { assert_eq!(beat.state, None, "{}", beat.name); }
}
#[test]
fn is_a_beat_the_prompt_leading_every_beat_shorter_than_the_shortest_lead() {
    for level in [BotMotionLevel::Full, BotMotionLevel::Subtle] {
        for beat in WORKING_BEATS {
            assert!(beat.seconds >= 1.5, "{}", beat.name);
            assert!(beat.seconds < working_lead(level).0, "{}", beat.name);
        }
    }
    assert!(working_lead(BotMotionLevel::Full).1 < working_lead(BotMotionLevel::Subtle).1);
}
#[test]
fn goes_round_all_of_them_in_turn_each_face_from_a_place_of_its_own() {
    for seed in [0.11, 0.37, 0.62, 0.9] {
        let round: Vec<_> = (0..4).map(|n| working_beat_for(seed, n).name).collect();
        assert_eq!(round.iter().collect::<HashSet<_>>().len(), WORKING_BEATS.len());
        assert_eq!(working_beat_for(seed, 4).name, round[0]);
    }
    assert!([0.05, 0.2, 0.35, 0.5, 0.65, 0.8, 0.95].into_iter().map(|s| working_beat_for(s, 0).name).collect::<HashSet<_>>().len() > 1);
    let sides: HashSet<_> = (0..40).filter_map(|n| working_beat_for(0.37, n).glance.map(|g| g.x as i32)).collect();
    assert_eq!(sides, HashSet::from([-1, 1]));
}
#[test]
fn wears_a_beat_as_a_change_of_its_own_its_body_easing_off_the_hop_for_it() {
    let beat = WORKING_BEATS[0];
    let life = EverydayFaceLife { beat: Some(beat), ..Default::default() };
    let face = everyday_face(FaceActivity::Working, life);
    assert_eq!(face.eyes, Some(beat.eyes));
    assert_eq!(face.key, format!("working:{}", beat.name));
    assert_eq!(face.state, Some(BotAvatarState::Default));
    assert_eq!(everyday_face(FaceActivity::Working, EverydayFaceLife { noticed: true, ..life }).key, "working");
    assert_eq!(everyday_face(FaceActivity::Blocked, life).eyes, Some([">", "<"]));
}
#[test]
fn keeps_codex_buddy_lifting_through_the_beats_and_thinking_for_the_thinking_one() {
    use crate::buddy_moves::*;
    assert_eq!(buddy_move_for("working:think", None, BotMotionLevel::Full).name, think_move().name);
    for beat in WORKING_BEATS.iter().filter(|b| b.name != "think") {
        let key = format!("working:{}", beat.name);
        assert_eq!(buddy_move_for(&key, None, BotMotionLevel::Full).name, workout_move().name);
        assert_eq!(buddy_move_for(&key, None, BotMotionLevel::Subtle).name, workout_standing_move().name);
    }
}
fn step(n: u32, words: bool) -> Vec<LiveRuntimeEvent> {
    let mut events = vec![
        LiveRuntimeEvent::StepStarted { step_kind: "reasoning".into() },
        LiveRuntimeEvent::StepCompleted { step_kind: "reasoning".into() },
        LiveRuntimeEvent::ToolStarted { item_id: format!("tool_{n}"), name: "run_command".into(), by_helper: false },
    ];
    if words { events.push(LiveRuntimeEvent::MessageDelta { final_chunk: false }); }
    events
}
#[test]
fn works_through_a_tool_loop_whose_steps_carry_words_never_replying_until_its_answer() {
    let mut events = vec![LiveRuntimeEvent::StepStarted { step_kind: "turn".into() }];
    for n in 0..12 {
        events.extend(step(n, n % 3 == 0));
        assert_eq!(live_activity_of(&events, true), LiveActivity::Working, "step {n}, its tool running");
        events.push(LiveRuntimeEvent::ToolCompleted { item_id: format!("tool_{n}") });
        assert_ne!(live_activity_of(&events, true), LiveActivity::Responding, "step {n}, its tool done");
    }
    events.push(LiveRuntimeEvent::MessageDelta { final_chunk: true });
    assert_ne!(live_activity_of(&events, true), LiveActivity::Responding);
}
#[test]
fn still_replies_while_a_reply_streams_with_no_tool_open_as_every_other_runtime_does() {
    assert_eq!(live_activity_of(&[LiveRuntimeEvent::MessageDelta { final_chunk: false }], true), LiveActivity::Responding);
    let mut after_tool = step(1, false);
    after_tool.push(LiveRuntimeEvent::ToolCompleted { item_id: "tool_1".into() });
    after_tool.push(LiveRuntimeEvent::MessageDelta { final_chunk: false });
    assert_eq!(live_activity_of(&after_tool, true), LiveActivity::Responding);
}
#[test]
fn switches_on_for_starting_work_finishing_getting_stuck_and_waiting_on_you() {
    for (from, to, n) in [("idle", "working", 1), ("done", "working", 1), ("working", "done", 2), ("working:read", "blocked", 2), ("idle", "waiting", 3)] {
        assert_eq!(crt_rule(0.37, Some(from), Some(to), n), Some(CrtKind::On));
    }
}
#[test]
fn never_switches_on_for_a_beat_going_back_to_the_lead_a_moment_at_rest_or_you_pointing() {
    for n in 0..30 {
        for (from, to) in [("working:think", "working"), ("idle", "idle:look"), ("idle", "idle:noticed"), ("idle:doze", "idle")] {
            assert_eq!(crt_rule(0.37, Some(from), Some(to), n), None);
        }
    }
}
#[test]
fn flicks_for_about_one_beat_in_three_a_faces_own_beats_never_every_one() {
    for seed in [0.11, 0.37, 0.62, 0.9] {
        let flicks = (0..300).filter(|n| crt_rule(seed, Some("working"), Some("working:read"), *n) == Some(CrtKind::Flick)).count();
        let share = flicks as f64 / 300.0;
        assert!(share > CRT_FLICK_SHARE - 0.1 && share < CRT_FLICK_SHARE + 0.1, "{seed}: {share}");
    }
    let at = |seed| (0..24).map(|n| crt_rule(seed, Some("working"), Some("working:focus"), n) == Some(CrtKind::Flick)).collect::<Vec<_>>();
    assert_ne!(at(0.11), at(0.62));
}

//! Electron's face tests, case by case, with Electron's values (parity/tests.tsv, round 6, job 4). Every test is named for
//! one Electron case; the file it comes from is in the comment above it.
//!
//! - `a-face-rests-behind-other-windows.test.ts`: the clock's four cases (the window-presence cases read a browser
//!   window's focus and visibility; the native window hands its focus to the stage as a flag).
//! - `every-face-draws-on-one-beat.test.ts`
//! - `a-screen-keeps-its-face-to-you.test.ts`
//! - `only-the-face-you-talk-to-hops.test.tsx` (the motion levels)
//! - `a-face-at-rest-answers-you.test.ts`
//! - `a-teammate-at-work-goes-round-its-faces.test.ts`
//! - `one-fact-one-surface.test.ts` (the duration)

use std::collections::BTreeSet;

use crate::bot_rig::*;
use crate::bot_spec::Phosphor;

// ---- a-face-rests-behind-other-windows.test.ts, "a moving face, while its window is behind other windows" ----

// case: asks for no frames and draws nothing more, holding where it was
#[test]
fn a_moving_face_asks_for_no_frames_and_draws_nothing_more_while_its_window_is_behind_others() {
    let mut clock = BotClock::start(0.0, false, false);
    let (step, drew) = clock.tick(34.0, true, false);
    assert!(drew);
    assert_eq!(clock.draws, 2);
    assert!((step.unwrap() - 0.034).abs() < 1e-9);
    clock.on_presence(34.0, true);
    assert_eq!(clock.pending_frame(), None, "no frame is asked for");
    assert_eq!(clock.draws, 2, "and nothing more is drawn");
    assert!((clock.stepped_seconds - 0.034).abs() < 1e-9, "the face holds where it was: {}", clock.stepped_seconds);
}

// case: picks up when the window comes back, with no jump for the time it was away
#[test]
fn a_face_picks_up_when_its_window_comes_back_with_no_jump_for_the_time_it_was_away() {
    let mut clock = BotClock::start(0.0, false, false);
    clock.tick(34.0, true, false);
    clock.on_presence(34.0, true);
    // Five seconds behind other windows: no frame comes. The window returns at 5000 ms.
    clock.on_presence(5_000.0, false);
    assert!(clock.pending_frame().is_some(), "it asks for a frame again");
    let (step, drew) = clock.tick(5_034.0, true, false);
    assert!(drew);
    assert_eq!(clock.draws, 3);
    // The step after coming back is one frame's worth, not the five seconds it was away.
    assert!((step.unwrap() - 0.034).abs() < 1e-9, "{step:?}");
    assert!((clock.stepped_seconds - 0.068).abs() < 1e-9, "{}", clock.stepped_seconds);
}

// case: started behind other windows, is drawn once, and moves only once its window is in front
#[test]
fn a_face_started_behind_other_windows_is_drawn_once_and_moves_only_once_its_window_is_in_front() {
    let mut clock = BotClock::start(0.0, true, false);
    assert_eq!(clock.draws, 1);
    assert_eq!(clock.pending_frame(), None);
    clock.on_presence(0.0, false);
    assert!(clock.pending_frame().is_some());
}

// case: lets go of its window when it stops, and asks for nothing after
#[test]
fn a_face_that_has_stopped_asks_for_nothing_after_whatever_its_window_does() {
    let mut clock = BotClock::start(0.0, false, false);
    clock.stop();
    assert_eq!(clock.pending_frame(), None);
    clock.on_presence(10.0, true);
    clock.on_presence(20.0, false);
    assert_eq!(clock.pending_frame(), None);
    assert_eq!(clock.draws, 1);
}

// ---- every-face-draws-on-one-beat.test.ts ----

// "lets two clocks that started on different frames draw in the same frames, 30 a second in all".
#[test]
fn two_clocks_that_started_on_different_frames_draw_in_the_same_frames_30_a_second_in_all() {
    let mut beat = FrameBeat::new(30.0);
    let mut drawing_frames = BTreeSet::new();
    let (mut drawn_a, mut drawn_b) = (0, 0);
    for frame in 1..=60 {
        let now = f64::from(frame) * 1000.0 / 60.0;
        // Clock A asks first in each frame; clock B, started a frame later, asks after it.
        if beat.due(now) {
            drawn_a += 1;
            drawing_frames.insert(now.to_bits());
        }
        if frame > 1 && beat.due(now) {
            drawn_b += 1;
            drawing_frames.insert(now.to_bits());
        }
    }
    assert!(drawing_frames.len() <= 30, "{}", drawing_frames.len());
    assert!(drawing_frames.len() >= 29, "{}", drawing_frames.len());
    assert_eq!(drawn_a, drawing_frames.len());
    assert!(drawn_b >= 29, "{drawn_b}");
}

// "draws at most 30 frames a second on a 144 Hz screen too".
#[test]
fn it_draws_at_most_30_frames_a_second_on_a_144_hz_screen_too() {
    let mut beat = FrameBeat::new(30.0);
    let drawn = (1..=144).filter(|frame| beat.due(f64::from(*frame) * 1000.0 / 144.0)).count();
    assert!(drawn <= 30, "{drawn}");
    assert!(drawn >= 28, "{drawn}");
}

// "starts again for a clock whose time went backwards".
#[test]
fn a_beat_starts_again_for_a_clock_whose_time_went_backwards() {
    let mut beat = FrameBeat::new(30.0);
    assert!(beat.due(5000.0));
    assert!(beat.due(10.0));
    assert!(!beat.due(20.0));
}

// ---- a-screen-keeps-its-face-to-you.test.ts ----

fn pose(change: impl FnOnce(&mut BotAvatarPose)) -> BotAvatarPose {
    let mut p = rest_pose(BotAvatarState::Default);
    p.sx = 1.0;
    p.sy = 1.0;
    p.eye_open = 1.0;
    change(&mut p);
    p
}

// "a screen-faced pose keeps the heading the rig would have without its spin, its glances and hop included".
#[test]
fn a_screen_faced_pose_keeps_the_heading_the_rig_would_have_without_its_spin() {
    let spinning = pose(|p| {
        p.yaw = 0.3 + std::f64::consts::PI;
        p.look_x = 0.4;
        p.y = -12.0;
        p.roll = 0.1;
    });
    let kept = screen_pose(spinning, Some(0.3), None);
    assert_eq!(kept.yaw, 0.3);
    assert_eq!(kept.look_x, 0.4);
    assert_eq!(kept.y, -12.0);
    assert_eq!(kept.roll, 0.1);
}

// "without a heading to read, turns no further than its screen can show".
#[test]
fn a_screen_pose_without_a_heading_turns_no_further_than_its_screen_can_show() {
    use std::f64::consts::PI;
    assert!(screen_pose(pose(|p| p.yaw = PI * 0.9), None, None).yaw <= 0.75);
    assert!(screen_pose(pose(|p| p.yaw = -PI * 0.9), None, None).yaw >= -0.75);
    assert!((screen_pose(pose(|p| p.yaw = 0.2 + PI * 2.0), None, None).yaw - 0.2).abs() < 1e-9);
}

// "never laughs or whirls, and keeps a little under half of the squash".
#[test]
fn a_screen_pose_never_laughs_or_whirls_and_keeps_a_little_under_half_of_the_squash() {
    let landing = pose(|p| {
        p.laugh = 0.9;
        p.whirl = 0.8;
        p.sx = 1.16;
        p.sy = 0.82;
    });
    let kept = screen_pose(landing, Some(0.0), None);
    assert_eq!(kept.laugh, 0.0);
    assert_eq!(kept.whirl, 0.0);
    assert!((kept.sx - (1.0 + 0.16 * SCREEN_SQUASH)).abs() < 1e-9);
    assert!((kept.sy - (1.0 - 0.18 * SCREEN_SQUASH)).abs() < 1e-9);
    assert!(SCREEN_SQUASH < 0.5);
}

// "reads the real rig: through ten seconds of work, a screen never faces away".
#[test]
fn through_ten_seconds_of_work_a_screen_never_faces_away() {
    let mut sim = BotAvatarSim::new(0.37, BotAvatarState::Working);
    let (mut widest, mut raw_widest) = (0.0_f64, 0.0_f64);
    for _ in 0..600 {
        sim.update(1.0 / 60.0);
        let heading = sim.base_yaw;
        raw_widest = raw_widest.max(sim.pose.yaw.sin().atan2(sim.pose.yaw.cos()).abs());
        widest = widest.max(screen_pose(sim.pose, Some(heading), None).yaw.abs());
    }
    // The rig itself turns its back (a spin passes through pi); the screen stays facing front.
    assert!(raw_widest > 2.5, "{raw_widest}");
    assert!(widest < 0.75, "{widest}");
}

// ---- only-the-face-you-talk-to-hops.test.tsx: "a subtle bot, everywhere but the conversation" ----

fn summary_of(activity: FaceActivity) -> &'static str {
    let motion = bot_motion(activity, BotMotionLevel::Subtle);
    if motion.hops {
        "hop"
    } else if motion.paused {
        "still"
    } else if motion.bounces {
        "bounce"
    } else {
        "look"
    }
}

// case: looks around at work instead of hopping, and bounces a little
#[test]
fn a_subtle_bot_looks_around_at_work_instead_of_hopping_and_bounces_a_little() {
    for activity in [FaceActivity::Working, FaceActivity::Delegating] {
        let motion = bot_motion(activity, BotMotionLevel::Subtle);
        assert_eq!(motion.state, BotAvatarState::Default, "{activity:?}");
        assert!(!motion.paused, "{activity:?}");
        assert_eq!(motion.jump_every, Some(0.0), "{activity:?}");
        assert!(motion.bounces, "{activity:?}");
        assert!(!motion.hops, "{activity:?}");
    }
}

// case: never flips, whatever it is doing
#[test]
fn a_subtle_bot_never_flips_whatever_it_is_doing() {
    for activity in FaceActivity::ALL {
        assert_eq!(bot_motion(activity, BotMotionLevel::Subtle).jump_every, Some(0.0), "{activity:?}");
    }
}

// case: bounces only at work: thinking and answering look around, still keeps still, and a finish hops once
#[test]
fn a_subtle_bot_bounces_only_at_work_and_a_finish_hops_once() {
    let want = [
        (FaceActivity::Thinking, "look"),
        (FaceActivity::Working, "bounce"),
        (FaceActivity::Delegating, "bounce"),
        (FaceActivity::Responding, "look"),
        (FaceActivity::Waiting, "look"),
        (FaceActivity::Receiving, "look"),
        (FaceActivity::Blocked, "still"),
        (FaceActivity::Done, "hop"),
        (FaceActivity::Idle, "still"),
    ];
    assert_eq!(want.len(), FaceActivity::ALL.len());
    for (activity, expected) in want {
        assert_eq!(summary_of(activity), expected, "{activity:?}");
    }
}

// case: keeps the whole performance: it hops at work, and flips on its own schedule
#[test]
fn the_face_you_are_talking_to_keeps_the_whole_performance() {
    let working = bot_motion(FaceActivity::Working, BotMotionLevel::Full);
    assert_eq!((working.state, working.paused, working.bounces, working.hops), (BotAvatarState::Working, false, false, false));
    assert_eq!(working.jump_every, None, "it flips on the rig's own schedule");
    let thinking = bot_motion(FaceActivity::Thinking, BotMotionLevel::Full);
    assert_eq!((thinking.state, thinking.paused, thinking.bounces, thinking.hops), (BotAvatarState::Default, false, false, false));
    assert_eq!(thinking.jump_every, None);
}

// ---- a-face-at-rest-answers-you.test.ts ----

const NONE: EverydayFaceLife = EverydayFaceLife { noticed: false, listening: false, moment: None, beat: None };

fn look() -> IdleMoment {
    *IDLE_MOMENTS.iter().find(|m| m.name == "look").unwrap()
}

// "says what it is doing, whatever else is going on: busy, stuck, waiting or done, a pointer only turns its head".
#[test]
fn a_face_at_rest_says_what_it_is_doing_whatever_else_is_going_on() {
    for activity in FaceActivity::ALL.into_iter().filter(|a| *a != FaceActivity::Idle) {
        let loud = EverydayFaceLife { noticed: true, listening: true, moment: Some(look()), beat: None };
        for life in [NONE, loud] {
            let face = everyday_face(activity, life);
            assert_eq!(face.eyes, eye_glyphs_for(activity), "{activity:?}");
            assert_eq!(face.mood, mood_for(activity), "{activity:?}");
            assert_eq!(face.key, activity.as_str(), "{activity:?}");
        }
        // Pointed at, it wakes to look at you, even where it would keep still (stuck).
        assert!(everyday_face(activity, EverydayFaceLife { noticed: true, ..NONE }).lively, "{activity:?}");
    }
}

// "answers you first: pointed at, it is glad; typed to, it listens; otherwise a moment of its own, or its bars".
#[test]
fn a_face_at_rest_answers_you_first() {
    let noticed = everyday_face(FaceActivity::Idle, EverydayFaceLife { noticed: true, listening: true, moment: Some(look()), beat: None });
    assert_eq!((noticed.eyes, noticed.mood, noticed.key.as_str(), noticed.lively), (Some(["^", "^"]), Some(BotMood::Glad), "idle:noticed", true));
    let listening = everyday_face(FaceActivity::Idle, EverydayFaceLife { noticed: false, listening: true, moment: Some(look()), beat: None });
    assert_eq!((listening.eyes, listening.mood, listening.key.as_str(), listening.lively), (Some(["o", "o"]), Some(BotMood::Perked), "idle:listening", true));
    let moment = everyday_face(FaceActivity::Idle, EverydayFaceLife { moment: Some(look()), ..NONE });
    assert_eq!((moment.eyes, moment.key.as_str(), moment.lively, moment.glance), (Some(["o", "o"]), "idle:look", true, look().glance));
    let bars = everyday_face(FaceActivity::Idle, NONE);
    assert_eq!((bars.eyes, bars.mood, bars.key.as_str(), bars.lively), (None, None, "idle", false));
}

// "never flashes green for any of it: the green is the finish's alone".
#[test]
fn a_face_never_flashes_green_for_anything_but_the_finish() {
    for activity in FaceActivity::ALL {
        assert_eq!(flash_for(activity), (activity == FaceActivity::Done).then_some(Phosphor::Green), "{activity:?}");
    }
}

// "shows, in its moments, only faces that do not say it is busy or stuck".
#[test]
fn its_moments_show_only_faces_that_do_not_say_it_is_busy_or_stuck() {
    let busy = ["••", ">▮", "><"];
    for moment in IDLE_MOMENTS {
        assert!(!busy.contains(&moment.eyes.concat().as_str()), "{}", moment.name);
        assert!(moment.seconds > 0.5 && moment.seconds < 3.0, "{} {}", moment.name, moment.seconds);
    }
    // Between them, every other face it has: eyes wide, glad, curious, asleep.
    let faces: BTreeSet<String> = IDLE_MOMENTS.iter().map(|m| m.eyes.concat()).collect();
    assert_eq!(faces, ["oo", "^^", "cc"].iter().map(|s| s.to_string()).collect());
    assert!(IDLE_MOMENTS.iter().any(|m| m.mood == Some(BotMood::Curious)));
}

// "never has the same moment twice running, looks to either side, and each face makes its own choices".
#[test]
fn a_face_never_has_the_same_moment_twice_running_looks_to_either_side_and_makes_its_own_choices() {
    for seed in [0.05, 0.31, 0.62, 0.97] {
        let mut last: Option<&'static str> = None;
        let (mut sides, mut names) = (BTreeSet::new(), BTreeSet::new());
        for count in 0..60 {
            let moment = moment_for(seed, count, last);
            assert_ne!(Some(moment.name), last, "seed {seed} count {count}");
            // The same face, the same count: the same moment.
            assert_eq!(moment_for(seed, count, last), moment);
            if let Some(g) = moment.glance {
                sides.insert(g.x as i32);
            }
            names.insert(moment.name);
            last = Some(moment.name);
        }
        assert_eq!(sides, BTreeSet::from([-1, 1]), "seed {seed}");
        assert_eq!(names.len(), IDLE_MOMENTS.len(), "seed {seed}");
    }
    // Two faces side by side do not keep time with each other.
    let first = |seed: f64| -> Vec<f64> { (0..8).map(|count| seeded(seed, count, 0)).collect() };
    assert_ne!(first(0.2), first(0.21));
    for value in first(0.2) {
        assert!((0.0..1.0).contains(&value), "{value}");
    }
}

// "has its moments now and then, the face you talk to more often than the rest, never a show".
#[test]
fn a_face_has_its_moments_now_and_then_the_face_you_talk_to_more_often_never_a_show() {
    let (least, most) = moment_every(BotMotionLevel::Full);
    assert!(least >= 5.0);
    assert!(most > least);
    assert!(moment_every(BotMotionLevel::Subtle).0 > most);
    // Longer between them than the longest of them: most of the time a face at rest is at rest.
    let longest = IDLE_MOMENTS.iter().map(|m| m.seconds).fold(0.0, f64::max);
    assert!(least > 2.0 * longest);
}

// "is a presence, not a mark beside a name: the sizes that answer you are the teammate itself (botSizes.ts)".
#[test]
fn the_sizes_that_answer_you_are_the_teammate_itself_and_the_marks_beside_a_name_are_not() {
    use crate::bot_spec::bot_size::*;
    for presence in [THREAD_LIVE, WORKROOM_HEADER, ROSTER_CARD, SIDEBAR_FACES, RAIL_ROW] {
        assert!(presence >= PRESENCE_MIN, "{presence}");
    }
    for mark in [PICKER_MARK, TILE_MARK, CONVERSATION_OWNER, MEMORY_AUTHOR] {
        assert!(mark < PRESENCE_MIN, "{mark}");
    }
    // A pointer passing over a list is not a visit; listening outlasts a pause between words.
    assert!(NOTICE_DWELL_MS >= 150);
    assert!(LISTEN_HOLD_MS >= 2_000);
}

// ---- a-teammate-at-work-goes-round-its-faces.test.ts ----

const LIVE: [FaceActivity; 4] = [FaceActivity::Thinking, FaceActivity::Working, FaceActivity::Delegating, FaceActivity::Responding];
const ATTENTION: [FaceActivity; 5] = [FaceActivity::Waiting, FaceActivity::Receiving, FaceActivity::Blocked, FaceActivity::Done, FaceActivity::Idle];

// "at work, a face leads with the prompt and the hop: wears the working face whatever its run is doing this second".
#[test]
fn at_work_a_face_wears_the_working_face_whatever_its_run_is_doing_this_second() {
    for activity in LIVE {
        assert!(at_work(activity), "{activity:?}");
        assert_eq!(worn_activity(activity), FaceActivity::Working, "{activity:?}");
        assert_eq!(everyday_face(worn_activity(activity), NONE).eyes, Some([">", "▮"]), "{activity:?}");
        assert_eq!(phosphor_for(worn_activity(activity)), Phosphor::Cyan, "{activity:?}");
    }
}

// "keeps the faces that mean something to look at: waiting on you, listening, stuck, done, at rest".
#[test]
fn a_face_keeps_the_faces_that_mean_something_to_look_at() {
    for activity in ATTENTION {
        assert!(!at_work(activity), "{activity:?}");
        assert_eq!(worn_activity(activity), activity, "{activity:?}");
    }
    assert_eq!(eye_glyphs_for(worn_activity(FaceActivity::Blocked)), Some([">", "<"]));
    assert_eq!(eye_glyphs_for(worn_activity(FaceActivity::Done)), Some(["^", "^"]));
    assert_eq!(phosphor_for(worn_activity(FaceActivity::Waiting)), Phosphor::Amber);
}

// "has a beat of each other working face, and none of the faces that mean something".
#[test]
fn it_has_a_beat_of_each_other_working_face_and_none_of_the_faces_that_mean_something() {
    let faces: Vec<String> = WORKING_BEATS.iter().map(|b| b.eyes.concat()).collect();
    let names: BTreeSet<&str> = WORKING_BEATS.iter().map(|b| b.name).collect();
    assert_eq!(names.len(), WORKING_BEATS.len());
    assert!(WORKING_BEATS.len() >= 4);
    assert!(faces.contains(&"••".to_owned()));
    for meaning in ["^^", "><", "||", "cc", ">▮"] {
        assert!(!faces.contains(&meaning.to_owned()), "{meaning}");
    }
    for beat in WORKING_BEATS {
        assert!(beat.state.is_none(), "{}", beat.name);
    }
}

// "is a beat, the prompt leading: every beat shorter than the shortest lead".
#[test]
fn a_beat_is_shorter_than_the_shortest_lead_and_the_face_you_talk_to_goes_round_sooner() {
    for level in [BotMotionLevel::Full, BotMotionLevel::Subtle] {
        let (least, _) = working_lead(level);
        for beat in WORKING_BEATS {
            assert!(beat.seconds >= 1.5, "{}", beat.name);
            assert!(beat.seconds < least, "{} {level:?}", beat.name);
        }
    }
    assert!(working_lead(BotMotionLevel::Full).1 < working_lead(BotMotionLevel::Subtle).1);
}

// "goes round all of them in turn, each face from a place of its own".
#[test]
fn a_face_goes_round_all_its_beats_in_turn_each_from_a_place_of_its_own() {
    for seed in [0.11, 0.37, 0.62, 0.9] {
        let round: Vec<&str> = (0..4).map(|count| working_beat_for(seed, count).name).collect();
        assert_eq!(round.iter().collect::<BTreeSet<_>>().len(), WORKING_BEATS.len(), "{seed}");
        assert_eq!(working_beat_for(seed, 4).name, round[0]);
    }
    let starts: BTreeSet<&str> = [0.05, 0.2, 0.35, 0.5, 0.65, 0.8, 0.95].iter().map(|s| working_beat_for(*s, 0).name).collect();
    assert!(starts.len() > 1);
    // Reading, it looks to one side or the other, not always the same.
    let sides: BTreeSet<i32> = (0..40).filter_map(|count| working_beat_for(0.37, count).glance.map(|g| g.x as i32)).collect();
    assert_eq!(sides, BTreeSet::from([-1, 1]));
}

// "wears a beat as a change of its own, its body easing off the hop for it".
#[test]
fn a_face_wears_a_beat_as_a_change_of_its_own() {
    let beat = WORKING_BEATS[0];
    let face = everyday_face(FaceActivity::Working, EverydayFaceLife { beat: Some(beat), ..NONE });
    assert_eq!(face.eyes, Some(beat.eyes));
    assert_eq!(face.key, format!("working:{}", beat.name));
    assert_eq!(face.state, Some(BotAvatarState::Default));
    // Pointed at, it looks at you instead.
    assert_eq!(everyday_face(FaceActivity::Working, EverydayFaceLife { noticed: true, beat: Some(beat), ..NONE }).key, "working");
    // A beat is for work alone.
    assert_eq!(everyday_face(FaceActivity::Blocked, EverydayFaceLife { beat: Some(beat), ..NONE }).eyes, Some([">", "<"]));
}

// ---- "text beside a tool is not a reply" ----

fn tool_step(n: usize, words: bool) -> Vec<LiveRuntimeEvent> {
    // One Antigravity planner step: its reasoning, a tool call, and words delivered after the call, never final.
    let mut events = vec![
        LiveRuntimeEvent::StepStarted { step_kind: "reasoning".into() },
        LiveRuntimeEvent::StepCompleted { step_kind: "reasoning".into() },
        LiveRuntimeEvent::ToolStarted { item_id: format!("tool_{n}"), name: "run_command".into(), by_helper: false },
    ];
    if words {
        events.push(LiveRuntimeEvent::MessageDelta { final_chunk: false });
    }
    events
}

fn tool_done(n: usize) -> LiveRuntimeEvent {
    LiveRuntimeEvent::ToolCompleted { item_id: format!("tool_{n}") }
}

// "works through a tool loop whose steps carry words, never replying until its answer".
#[test]
fn a_tool_loop_whose_steps_carry_words_never_replies_until_its_answer() {
    let mut events = vec![LiveRuntimeEvent::StepStarted { step_kind: "turn".into() }];
    for n in 0..12 {
        events.extend(tool_step(n, n % 3 == 0));
        assert_eq!(live_activity_of(&events, true), LiveActivity::Working, "step {n}, its tool running");
        events.push(tool_done(n));
        assert_ne!(live_activity_of(&events, true), LiveActivity::Responding, "step {n}, its tool done");
    }
    events.push(LiveRuntimeEvent::MessageDelta { final_chunk: true });
    assert_ne!(live_activity_of(&events, true), LiveActivity::Responding);
}

// "still replies while a reply streams with no tool open, as every other runtime does".
#[test]
fn it_still_replies_while_a_reply_streams_with_no_tool_open() {
    let events = vec![LiveRuntimeEvent::MessageDelta { final_chunk: false }];
    assert_eq!(live_activity_of(&events, true), LiveActivity::Responding);
    let mut after_tool = tool_step(1, false);
    after_tool.push(tool_done(1));
    after_tool.push(LiveRuntimeEvent::MessageDelta { final_chunk: false });
    assert_eq!(live_activity_of(&after_tool, true), LiveActivity::Responding);
}

// ---- "the screen switches on again for the changes worth looking up for" ----

// "for starting work, finishing, getting stuck and waiting on you".
#[test]
fn the_screen_switches_on_again_for_starting_work_finishing_getting_stuck_and_waiting_on_you() {
    let rule = |from, to, count| crt_rule(0.37, Some(from), Some(to), count);
    assert_eq!(rule("idle", "working", 1), Some(CrtKind::On));
    assert_eq!(rule("done", "working", 1), Some(CrtKind::On));
    assert_eq!(rule("working", "done", 2), Some(CrtKind::On));
    assert_eq!(rule("working:read", "blocked", 2), Some(CrtKind::On));
    assert_eq!(rule("idle", "waiting", 3), Some(CrtKind::On));
}

// "never for a beat going back to the lead, a moment at rest, or you pointing".
#[test]
fn the_screen_never_switches_on_for_a_beat_going_back_to_the_lead_a_moment_at_rest_or_you_pointing() {
    for count in 0..30 {
        let rule = |from, to| crt_rule(0.37, Some(from), Some(to), count);
        assert_eq!(rule("working:think", "working"), None, "{count}");
        assert_eq!(rule("idle", "idle:look"), None, "{count}");
        assert_eq!(rule("idle", "idle:noticed"), None, "{count}");
        assert_eq!(rule("idle:doze", "idle"), None, "{count}");
    }
}

// "flicks for about one beat in three, a face's own beats, never every one".
#[test]
fn the_screen_flicks_for_about_one_beat_in_three_never_every_one() {
    for seed in [0.11, 0.37, 0.62, 0.9] {
        let flicks = (0..300).filter(|count| crt_rule(seed, Some("working"), Some("working:read"), *count) == Some(CrtKind::Flick)).count();
        let share = flicks as f64 / 300.0;
        assert!(share > CRT_FLICK_SHARE - 0.1, "{seed}: {share}");
        assert!(share < CRT_FLICK_SHARE + 0.1, "{seed}: {share}");
    }
    // Two faces do not flick on the same beats.
    let at = |seed: f64| -> String { (0..24).map(|count| if crt_rule(seed, Some("working"), Some("working:focus"), count) == Some(CrtKind::Flick) { '1' } else { '0' }).collect() };
    assert_ne!(at(0.11), at(0.62));
}

// ---- one-fact-one-surface.test.ts, "one mission, one duration" ----

// "a 41-second run is 41s, not 0m".
#[test]
fn a_41_second_run_is_41s_not_0m() {
    assert_eq!(crate::ledger::duration_text(41_000), "41s");
    assert!(!crate::ledger::duration_text(41_000).contains("0m"));
}

// "and a long one still reads in minutes".
#[test]
fn a_long_run_still_reads_in_minutes() {
    assert_eq!(crate::ledger::duration_text(150_000), "2m 30s");
}

// ---- a-screen-keeps-its-face-to-you.test.ts, the case a Rust value cannot break ----

// "copies the rig's pose and never changes it, so the rig eases from its own last frame". `screen_pose` takes the pose by
// value (it is `Copy`), so the rig's own pose cannot change; this holds that, and cannot be broken short of changing the
// signature to a reference.
#[test]
fn a_screen_pose_copies_the_rigs_pose_and_never_changes_it() {
    let rig = pose(|p| {
        p.yaw = 4.0;
        p.laugh = 1.0;
        p.sx = 1.2;
    });
    let before = rig;
    let _ = screen_pose(rig, Some(0.0), None);
    assert_eq!(rig, before);
}

// ---- a-thinking-face-looks-with-its-head.test.ts, "a thinking face" ----

const CYCLE: f64 = 4.2;
const MS: f64 = 0.001;

fn sign(x: f64) -> f64 {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
}

// "eases toward each glance: from rest, never past it or back, slowing all the way to its landing, then held".
#[test]
fn a_thinking_face_eases_toward_each_glance_never_past_it_or_back_slowing_to_its_landing_then_held() {
    // The second glance, up and to the right, leaves where the first landed: across the head's turn and tilt.
    for lp in [0.0, 3.0] {
        let at = |t: f64| thinking_glance(lp * CYCLE + 1.05 + t);
        let (start, end) = (at(0.0), at(HEAD_GLANCE_S));
        assert!(end.yaw - start.yaw > 0.3, "{} -> {}", start.yaw, end.yaw);
        let mut last = start;
        let mut speeds: Vec<f64> = Vec::new();
        let steps = ((HEAD_GLANCE_S + 1e-9) / MS).floor() as usize;
        for i in 1..=steps {
            let t = i as f64 * MS;
            let now = at(t);
            for (name, n, l, s, e) in [("yaw", now.yaw, last.yaw, start.yaw, end.yaw), ("pitch", now.pitch, last.pitch, start.pitch, end.pitch)] {
                // Between where it left and where it lands, the whole way: no overshoot, no step back.
                assert!(n >= s.min(e) - 1e-12, "{name} at {t:.3}");
                assert!(n <= s.max(e) + 1e-12, "{name} at {t:.3}");
                assert!(sign(n - l) * sign(e - s) >= 0.0, "{name} turns back at {t:.3}");
            }
            speeds.push((now.yaw - last.yaw).hypot(now.pitch - last.pitch));
            last = now;
        }
        let top = speeds.iter().cloned().fold(0.0, f64::max);
        let quickest = speeds.iter().position(|s| *s == top).unwrap();
        // From rest: its first frame's worth (a 30th of a second) goes a twentieth of the way or less, not the third an
        // unsoftened ease-out goes; at its quickest within the first quarter; then only ever slower, to a standstill.
        let first_frame = at(1.0 / 30.0);
        assert!((first_frame.yaw - start.yaw).hypot(first_frame.pitch - start.pitch) < 0.05 * (end.yaw - start.yaw).hypot(end.pitch - start.pitch));
        assert!(quickest < speeds.len() / 4, "{quickest} of {}", speeds.len());
        for i in quickest + 1..speeds.len() {
            assert!(speeds[i] <= speeds[i - 1] + 1e-12, "speeds up at {:.3}", (i + 1) as f64 * MS);
        }
        assert!(*speeds.last().unwrap() < 1e-6);
        // Landed, and held until the face gives the head back.
        assert!((at(1.0).yaw - end.yaw).abs() < 1e-9);
    }
}

// "takes the head as the loop begins and gives it back before the dots are halfway through their bounce".
#[test]
fn a_thinking_face_takes_the_head_as_the_loop_begins_and_gives_it_back_before_the_dots_are_halfway() {
    let mut last = thinking_glance(CYCLE * 2.0 + MS).mix;
    let mut i = 2;
    while (i as f64) * MS < CYCLE {
        let t = i as f64 * MS;
        let mix = thinking_glance(CYCLE * 2.0 + t).mix;
        assert!((0.0..=1.0).contains(&mix), "{mix}");
        // Rising while the face looks about, falling once the dots bounce: never back the other way.
        if t < 2.1 {
            assert!(mix >= last - 1e-12, "at {t:.3}");
        } else {
            assert!(mix <= last + 1e-12, "at {t:.3}");
        }
        last = mix;
        i += 1;
    }
    assert_eq!(thinking_glance(CYCLE * 2.0 + 1.0).mix, 1.0);
    for t in [2.1 + HEAD_GLANCE_S, 3.0, 3.5, 4.19] {
        assert_eq!(thinking_glance(CYCLE * 2.0 + t).mix, 0.0, "{t}");
    }
}

// "looks up, and to both sides, as the dots used to".
#[test]
fn a_thinking_face_looks_up_and_to_both_sides() {
    let looks: Vec<HeadGlance> = (0..210).map(|i| thinking_glance(CYCLE * 5.0 + i as f64 / 100.0)).collect();
    let looking: Vec<&HeadGlance> = looks.iter().filter(|l| l.mix > 0.99).collect();
    assert!(looking.iter().map(|l| l.yaw).fold(f64::INFINITY, f64::min) < -0.2);
    assert!(looking.iter().map(|l| l.yaw).fold(f64::NEG_INFINITY, f64::max) > 0.2);
    assert!(looking.iter().all(|l| l.pitch > 0.0));
}

// case: is at rest on a still bot, drawn at second 0 (reduced motion holds still): the head where the rig has it, the dots where they hold
#[test]
fn a_thinking_face_is_at_rest_on_a_still_bot_drawn_at_second_0() {
    assert_eq!(thinking_glance(0.0).mix, 0.0);
    // Before its thinking has begun (its dots still opening), the same.
    assert_eq!(thinking_glance(-0.1).mix, 0.0);
    assert_eq!(glyph_motion("••", 0, 0.0), glyph_motion("••", 0, 1.0));
    assert_eq!(glyph_motion("••", 1, 0.0), glyph_motion("••", 0, 0.0));
}

// case: replaces the rig's heading as its mix rises, rather than adding a second turn to it
#[test]
fn a_thinking_glance_replaces_the_rigs_heading_as_its_mix_rises() {
    let rig = pose(|p| {
        p.yaw = 0.5;
        p.pitch = -0.2;
    });
    let look = |mix| Some(HeadGlance { yaw: -0.3, pitch: 0.12, mix, wander: 1.0 });
    assert!((screen_pose(rig, Some(0.5), look(0.0)).yaw - 0.5).abs() < 1e-9);
    assert!((screen_pose(rig, Some(0.5), look(1.0)).yaw + 0.3).abs() < 1e-9);
    assert!((screen_pose(rig, Some(0.5), look(1.0)).pitch - 0.12).abs() < 1e-9);
    assert!((screen_pose(rig, Some(0.5), look(0.5)).yaw - 0.1).abs() < 1e-9);
    // Its glances sit well inside the turn a screen can show.
    let mut t = MS;
    while t < CYCLE {
        assert!(thinking_glance(t).yaw.abs() < 0.75, "{t}");
        t += 0.01;
    }
}

// "holds the rig's own turning back meanwhile, so no turn of the head is faster than its glances (the real rig)".
#[test]
fn a_thinking_face_holds_the_rigs_own_turning_back_so_no_turn_is_faster_than_its_glances() {
    // Ten seconds of a thinking face at 30 frames a second, on the rig itself: its heading wanders up to 0.6 either way.
    let fastest = |hush: f64| -> f64 {
        let mut sim = BotAvatarSim::new(0.26, BotAvatarState::Default);
        let (mut last, mut most): (Option<f64>, f64) = (None, 0.0);
        for frame in 1..=300 {
            sim.update(1.0 / 30.0);
            let glance = thinking_glance(CYCLE * 7.0 + f64::from(frame) / 30.0);
            let yaw = screen_pose(sim.pose, Some(sim.base_yaw), Some(HeadGlance { wander: 1.0 - hush, ..glance })).yaw;
            if let Some(l) = last {
                most = most.max((yaw - l).abs());
            }
            last = Some(yaw);
        }
        most
    };
    // The glances' own quickest frame: two of them 0.44 apart, over HEAD_GLANCE_S.
    let mut glancing = 0.0_f64;
    let mut t = 0.0;
    while t < CYCLE {
        let (a, b) = (thinking_glance(CYCLE + t + 1.0 / 30.0), thinking_glance(CYCLE + t));
        glancing = glancing.max((a.yaw * a.mix - b.yaw * b.mix).abs());
        t += 1.0 / 30.0;
    }
    assert!(fastest(THINKING_HUSH) < glancing * 1.6, "{} vs {}", fastest(THINKING_HUSH), glancing);
    // Kept whole, the rig's heading is handed back from as far as it wandered: faster than any glance.
    assert!(fastest(0.0) > fastest(THINKING_HUSH));
}

// "fades its look in and out from wherever it is, gliding on when changed midway".
#[test]
fn a_thinking_look_fades_in_and_out_from_wherever_it_is_gliding_on_when_changed_midway() {
    let mut thinking = EasedValue::new(0.0, HEAD_GLANCE_S);
    thinking.set(1.0, 10.0);
    let midway = thinking.at(10.1);
    assert!((midway - glance_ease(0.1 / HEAD_GLANCE_S)).abs() < 1e-9);
    assert!(midway > 0.0);
    // Stopped thinking midway: it falls from where it is, never jumping back to 1 or to 0 first.
    thinking.set(0.0, 10.1);
    assert!((thinking.at(10.1) - midway).abs() < 1e-9);
    let mut last = midway;
    let mut i = 1;
    while 10.1 + (i as f64) * MS < 10.1 + HEAD_GLANCE_S {
        let now = thinking.at(10.1 + (i as f64) * MS);
        assert!(now <= last + 1e-12);
        assert!(now >= 0.0);
        last = now;
        i += 1;
    }
    assert_eq!(thinking.at(11.0), 0.0);
    // Between changes, the same moment always reads the same.
    assert_eq!(thinking.at(10.3), thinking.at(10.3));
}

// ---- a-teammate-at-work-goes-round-its-faces.test.ts: Codex Buddy ----

// "keeps Codex Buddy lifting through the beats, and thinking for the thinking one".
#[test]
fn codex_buddy_keeps_lifting_through_the_beats_and_thinks_for_the_thinking_one() {
    use crate::buddy_moves::{buddy_move_for, think_move, workout_move, workout_standing_move, Move};
    // A move is told from another by what it is made of (Electron compares the module's own objects).
    let print = |m: &Move| (m.name, m.ms.to_bits(), m.loops, m.sets.to_vec(), m.lifts.to_vec());
    assert_eq!(print(&buddy_move_for("working:think", None, BotMotionLevel::Full)), print(&think_move()));
    for beat in WORKING_BEATS.iter().filter(|b| b.name != "think") {
        let key = format!("working:{}", beat.name);
        assert_eq!(print(&buddy_move_for(&key, None, BotMotionLevel::Full)), print(&workout_move()), "{}", beat.name);
        assert_eq!(print(&buddy_move_for(&key, None, BotMotionLevel::Subtle)), print(&workout_standing_move()), "{}", beat.name);
    }
    assert_ne!(print(&workout_move()), print(&think_move()));
}

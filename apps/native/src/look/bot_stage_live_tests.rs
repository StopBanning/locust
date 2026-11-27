//! speed-2 row 1: a moving face in a focused window moved by rebuilding and re-measuring the whole view, each step of
//! its rig (`bot_stage.poll` reported every step as a change, and the face was a `canvas(shape)` built in the view).
//! Now the view hands the face's drawing to the stage (`BotStage::live_face`, faces.rs) and shows it by
//! `rune::canvas_live`; poll draws each next pose into it and reports no change, so the window paints the new pose
//! with no commit. Faces still loop as Electron's CSS does: only the rebuild is gone.
use super::*;
use locust_core::bot_rig::FaceActivity;
use rune::{Env, Runtime};

const SLOT: &str = "roster-tm_wren";

/// The real window, focused, the wide sidebar showing Wren, whose face moves while `busy` (Working loops).
fn window(busy: bool) -> Runtime<State> {
    let mut s = State::default();
    s.layout_pref = "wide".into();
    apply_team(
        &mut s,
        vec![Teammate {
            id: "tm_wren".into(),
            name: "Wren".into(),
            role: String::new(),
            runtime: Some("codex".into()),
            model: None,
            account: None,
            busy,
            runs_in: "ask".into(),
            folder: None,
        }],
    );
    s.window_focused = true;
    let mut rt = Runtime::new(s, view, (1100.0, 780.0), Env::new(true, 1.0));
    rt.poll = Some(poll);
    rt.frame(0.0);
    rt
}

fn drawn() -> String {
    format!("{:?}", crate::look::faces::drawn_shape(SLOT).expect("the face is drawn"))
}

#[test]
fn a_moving_face_in_a_focused_window_moves_with_no_rebuild() {
    let mut rt = window(true);
    assert_eq!(rt.state.bot_stage.desired_activity(SLOT), Some(FaceActivity::Working));
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
    let commits = rt.stats.commits;
    let mut poses = std::collections::HashSet::new();
    for _ in 0..120 {
        rt.frame(1.0 / 60.0);
        assert!(rt.state.bot_stage.active_count() > 0, "the face keeps moving");
        poses.insert(drawn());
    }
    assert!(poses.len() >= 10, "the face moved through {} poses in two seconds", poses.len());
    assert!(
        rt.stats.commits - commits <= 2,
        "two seconds of a moving face rebuilt the window {} times",
        rt.stats.commits - commits
    );
}

/// What the window painted inside the face's box this frame (instances starting there), by their rects.
fn painted_face(rt: &Runtime<State>) -> Vec<[f32; 4]> {
    let ([x0, y0, w, h], _) = rt.presented(&format!("bot-{SLOT}")).expect("the face is presented");
    let (x1, y1) = (x0 + w, y0 + h);
    rt.instances
        .iter()
        .map(|i| i.rect)
        .filter(|r| r[0] >= x0 - 2.0 && r[0] <= x1 + 2.0 && r[1] >= y0 - 2.0 && r[1] <= y1 + 2.0)
        .collect()
}

#[test]
fn what_the_window_paints_is_the_pose_the_stage_drew_last() {
    let mut rt = window(true);
    for _ in 0..40 {
        rt.frame(1.0 / 60.0);
    }
    let commits = rt.stats.commits;
    let mut last = painted_face(&rt);
    assert!(!last.is_empty(), "the face paints");
    let mut changed = 0;
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
        let now = painted_face(&rt);
        if now != last {
            changed += 1;
        }
        last = now;
    }
    assert!(changed >= 3, "the window painted the face's new poses ({changed} changes in half a second)");
    assert_eq!(rt.stats.commits, commits, "none of them rebuilt the window");
}

#[test]
fn a_face_at_rest_asks_for_nothing_and_a_new_activity_still_rebuilds() {
    let mut rt = window(false);
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
    assert_eq!(rt.state.bot_stage.desired_activity(SLOT), Some(FaceActivity::Idle));
    let commits = rt.stats.commits;
    rt.update(|s, _| s.teammates[0].busy = true);
    rt.frame(1.0 / 60.0);
    assert!(rt.stats.commits > commits, "a new activity is the view's");
    assert_eq!(rt.state.bot_stage.desired_activity(SLOT), Some(FaceActivity::Working));
}

#[test]
fn a_moving_face_frames_the_window_once_a_step_however_often_something_else_draws() {
    // The window's host: a frame at each wake the window asks for, and every quarter second one more frame between
    // them, as a hover or a landing reply draws one. Two seconds of a face stepping 30 times a second.
    let mut rt = window(true);
    let mut clock = 0.0;
    for _ in 0..6 {
        rt.frame(1.0 / 60.0);
        clock += 1.0 / 60.0;
    }
    let (start, mut frames) = (clock, 0u32);
    let mut next_extra = start + 0.25;
    while clock < start + 2.0 {
        let wake = rt.wake_deadline().expect("a moving face asks for its next step");
        let at = if next_extra < wake { next_extra += 0.25; next_extra - 0.25 + 0.007 } else { wake };
        let dt = (at - clock).max(0.001);
        clock += dt;
        rt.frame(dt);
        frames += 1;
    }
    // 60 steps and 8 extra frames, with room; a wake asked from every poll kept each extra frame's chain going.
    assert!(frames <= 80, "{frames} frames in two seconds");
    assert!(frames >= 50, "the face still steps: {frames} frames");
}

/// A measure, run by hand (speed-2 notes): the real window, focused, one face moving, framed only when it asks (as a
/// window's host frames), for `LOCUST_FACE_SECS` seconds (default 20). Prints the process's CPU over that time (`ps`),
/// the commits and the frames per second.
#[test]
#[ignore = "a measure, run by hand: speed-2 row 1"]
fn measure_a_focused_window_with_one_moving_face() {
    let secs: f64 = std::env::var("LOCUST_FACE_SECS").ok().and_then(|v| v.parse().ok()).unwrap_or(20.0);
    let cpu = || {
        let out = std::process::Command::new("ps").args(["-o", "cputime=", "-p", &std::process::id().to_string()]).output().unwrap();
        // [[hh:]mm:]ss.cc
        String::from_utf8_lossy(&out.stdout).trim().split(':').fold(0.0, |acc, part| acc * 60.0 + part.parse::<f64>().unwrap_or(0.0))
    };
    // LOCUST_FACE_BUSY=0: the same window with the face at rest, for the floor under the face's cost.
    let busy = std::env::var("LOCUST_FACE_BUSY").map_or(true, |v| v != "0");
    let mut rt = window(busy);
    let mut clock = 0.0;
    let step = |rt: &mut Runtime<State>, clock: &mut f64, last: &mut std::time::Instant| -> bool {
        let now = std::time::Instant::now();
        let dt = (now - *last).as_secs_f64();
        *last = now;
        *clock += dt;
        rt.frame(dt)
    };
    let mut last = std::time::Instant::now();
    let warm = std::time::Instant::now() + Duration::from_secs(2);
    while std::time::Instant::now() < warm {
        step(&mut rt, &mut clock, &mut last);
        std::thread::sleep(Duration::from_millis(16));
    }
    let (cpu0, commits0, start) = (cpu(), rt.stats.commits, std::time::Instant::now());
    let mut frames = 0u64;
    while start.elapsed().as_secs_f64() < secs {
        let animating = step(&mut rt, &mut clock, &mut last);
        frames += 1;
        assert!(!busy || rt.state.bot_stage.active_count() > 0, "the face kept moving");
        // A window's host: the next vsync while anything animates, else the next wake asked for.
        let wait = if animating { 1.0 / 60.0 } else { rt.wake_deadline().map_or(0.25, |at| (at - clock).clamp(0.001, 0.25)) };
        std::thread::sleep(Duration::from_secs_f64(wait));
    }
    let wall = start.elapsed().as_secs_f64();
    println!(
        "face-measure cpu_percent={:.1} commits_per_s={:.1} frames_per_s={:.1} secs={wall:.1}",
        (cpu() - cpu0) / wall * 100.0,
        (rt.stats.commits - commits0) as f64 / wall,
        frames as f64 / wall
    );
}

//! The live scenes draw what a person looks for in a running turn, found by name in the rendered window.
//!
//! Expected words and orbs are Electron's, never this crate's: the orb by state is missionView.ts `orbStateFor`
//! (reading `searching`, shell `connecting`, helper `weaving`, thinking `listening`, writing `shaping`); the plan's head
//! orb is ThreadItems.tsx's `<Orb state="solving" box={PLAN_HEAD_ORB} />` (24) and its running step `PLAN_ORB`
//! (`listening`, 20); the live line's orb box is Orb.tsx `ORB_BOX` (26); the headline is `liveActionLine` or else
//! `sweepText(REGISTER_WORD[register])` (ThreadItems.tsx), and "step N of M" the plan's peek.

use super::*;

fn window(build: fn(&mut State)) -> rune::Runtime<State> {
    let mut s = State::default();
    build(&mut s);
    let mut rt = rune::Runtime::new(s, |s, cx| crate::view(s, cx), (1215.0, 800.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.frame(0.016);
    rt
}

fn keyed(rt: &rune::Runtime<State>, key: &str) -> Option<rune::ElementInfo> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

/// The orbs on screen, by key and drawn state.
fn orbs(rt: &rune::Runtime<State>) -> Vec<(String, &'static str, [f32; 2])> {
    rt.elements()
        .into_iter()
        .filter(|e| e.trace_name == Some("thinking-orb") && !e.exiting)
        .map(|e| (e.key.clone().unwrap_or_default(), e.trace_namespace.map_or("", |(_, state)| state), [e.rect[2], e.rect[3]]))
        .collect()
}

/// The live line a person reads: its orb's state, its words, and that its clock is there.
fn live_line(rt: &rune::Runtime<State>) -> (&'static str, String, bool) {
    let all = orbs(rt);
    let orb = all.iter().find(|(key, ..)| key == "live-step-orb").unwrap_or_else(|| panic!("no live orb: {all:?}"));
    // Drawn at the box, 26 (Orb.tsx ORB_BOX), or at a 20-point preset's own size inside it.
    assert!(orb.2 == [26.0, 26.0] || orb.2 == [20.0, 20.0], "the live orb's size {:?}", orb.2);
    let label = keyed(rt, "live-step-label").expect("the live step's words");
    let time = keyed(rt, "live-step-time").is_some_and(|t| t.text.as_deref().is_some_and(|t| t.starts_with(" · ") && t.len() > 3));
    (orb.1, label.text.or(label.label).unwrap_or_default(), time)
}

fn plan_orbs(rt: &rune::Runtime<State>, plan: &str) -> Vec<(String, &'static str, [f32; 2])> {
    orbs(rt).into_iter().filter(|(key, ..)| key.starts_with(&format!("{plan}/"))).collect()
}

/// `--scene native-live --snap` shows three orbs: the live line's, the plan's head and its running step.
fn three_orbs_on_a_streaming_plan(build: fn(&mut State)) {
    let rt = window(build);
    let (orb, words, time) = live_line(&rt);
    // The streaming branch: "Writing" with the plan's step as the action (missionView.ts, `else if (streaming)`).
    assert_eq!(orb, "shaping");
    assert_eq!(words, "Summarize the result");
    assert!(time, "the turn's clock");
    assert_eq!(keyed(&rt, "live-step-peek").and_then(|e| e.text).as_deref(), Some(" · step 2 of 2"));
    let mut plan = plan_orbs(&rt, "preview-plan");
    plan.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        plan,
        vec![
            ("preview-plan/plan-head-orb".to_owned(), "solving", [24.0, 24.0]),
            ("preview-plan/plan-orb/listening".to_owned(), "listening", [20.0, 20.0]),
        ]
    );
    assert_eq!(orbs(&rt).len(), 3, "{:?}", orbs(&rt));
}

#[test]
fn native_live_shows_the_live_line_and_the_plan_with_three_orbs() {
    three_orbs_on_a_streaming_plan(native_scene_live);
}

#[test]
fn ledger_live_shows_the_live_line_and_the_plan_with_three_orbs() {
    three_orbs_on_a_streaming_plan(ledger_live_scene);
}

#[test]
fn ledger_live_light_shows_the_same_three_orbs() {
    three_orbs_on_a_streaming_plan(ledger_live_light_scene);
}

#[test]
fn the_finished_transcript_shows_no_orb_and_no_live_line() {
    let rt = window(ledger_transcript_scene);
    assert!(orbs(&rt).is_empty(), "{:?}", orbs(&rt));
    assert!(keyed(&rt, "live-step-label").is_none());
}

#[test]
fn thinking_with_nothing_open_says_thinking_beside_the_listening_orb() {
    let rt = window(live_thinking_scene);
    let (orb, words, time) = live_line(&rt);
    assert_eq!((orb, words.as_str(), time), ("listening", "Thinking…", true));
    assert!(orbs(&rt).iter().all(|(key, ..)| key == "live-step-orb"), "no plan, so no plan orb: {:?}", orbs(&rt));
}

#[test]
fn a_shell_command_open_says_what_it_runs_beside_the_connecting_orb() {
    let rt = window(live_shell_scene);
    let (orb, words, time) = live_line(&rt);
    assert_eq!((orb, words.as_str(), time), ("connecting", "Running npm test", true));
}

#[test]
fn a_read_open_shows_the_searching_orb() {
    let rt = window(live_read_scene);
    let (orb, words, time) = live_line(&rt);
    assert_eq!(orb, "searching");
    assert!(time && !words.is_empty(), "{words:?}");
}

#[test]
fn a_subagent_out_shows_the_weaving_orb() {
    let rt = window(live_subagent_scene);
    let (orb, words, time) = live_line(&rt);
    assert_eq!(orb, "weaving");
    assert!(time && !words.is_empty(), "{words:?}");
}

/// Electron's `liveActionLine` for a read tool: "Reading cart.py"; for a helper: "Asking a helper: …". The Rust
/// projection (core thread_view.rs `live_action`) has neither, so the line says "Using a tool · Read". Not this lane's
/// file: core/src/thread_view.rs.
#[test]
#[ignore = "core/src/thread_view.rs live_action lacks liveActionLine's read and helper words (not Live 1's file)"]
fn a_read_and_a_helper_say_what_they_do_in_electrons_words() {
    assert_eq!(live_line(&window(live_read_scene)).1, "Reading cart.py");
    assert!(live_line(&window(live_subagent_scene)).1.starts_with("Asking a helper"));
}

/// REVIEW-ui-changes-2026-11-26.md change 7 (Colin's yes, 26 Nov), over Composer.tsx:2315-2333 where only the Stop
/// button dims: once Stop is pressed the live line says "Stopping…", its orb still there, until the stop lands.
#[test]
fn stopping_says_stopping_until_the_stop_lands() {
    let rt = window(live_stopping_scene);
    assert!(rt.state.stopping.is_pending());
    let (orb, words, time) = live_line(&rt);
    assert_eq!((orb, words.as_str(), time), ("connecting", "Stopping\u{2026}", true));
}

#[test]
fn a_turn_that_hit_its_limit_runs_nothing_live() {
    let rt = window(live_limit_scene);
    assert!(orbs(&rt).is_empty(), "{:?}", orbs(&rt));
    assert!(keyed(&rt, "live-step-label").is_none());
    assert!(rt.elements().iter().any(|e| e.text.as_deref().is_some_and(|t| t.contains("usage limit"))));
}

/// Thread.tsx's limit is a red card headed "Usage limit reached" with its kind as a tag; Rust draws the raw sentence
/// (messages.rs, Fix A / Live 2).
#[test]
#[ignore = "messages.rs draws the limit as a grey sentence, not Thread.tsx's card (Live 2)"]
fn a_turn_that_hit_its_limit_shows_electrons_red_card() {
    let rt = window(live_limit_scene);
    assert!(rt.elements().iter().any(|e| e.text.as_deref() == Some("Usage limit reached")));
}

/// missionView.ts draws the live line from the first moment ("Starting…", the floor's `breathing` orb); the older
/// thread draws none until the record exists (REVIEW finding 4; messages.rs, Fix A / Live 2).
#[test]
#[ignore = "the older thread draws no live line just after Send (messages.rs, Fix A / Live 2)"]
fn just_after_send_the_older_thread_shows_the_live_line() {
    let rt = window(live_after_send_scene);
    let (orb, words, _) = live_line(&rt);
    assert_eq!((orb, words.as_str()), ("breathing", "Starting…"));
}


/// Every live scene, the parts a person looks for in a running turn by name: the orb, the step's words, its clock.
#[test]
fn every_live_scene_draws_the_orb_the_words_and_the_clock() {
    let scenes: [(&str, fn(&mut State)); 8] = [
        ("native-live", native_scene_live),
        ("ledger-live", ledger_live_scene),
        ("reviews-live", reviews_live_scene),
        ("live-thinking", live_thinking_scene),
        ("live-shell", live_shell_scene),
        ("live-read", live_read_scene),
        ("live-subagent", live_subagent_scene),
        ("live-stopping", live_stopping_scene),
    ];
    for (name, build) in scenes {
        let rt = window(build);
        let (orb, words, time) = live_line(&rt);
        assert!(!orb.is_empty() && !words.is_empty() && time, "{name}: orb {orb:?}, words {words:?}, clock {time}");
    }
}

/// A plan's done step wears the check, its running step the orb, while the turn runs (ThreadItems.tsx PlanSteps).
#[test]
fn the_plan_marks_its_done_step_with_the_check() {
    let rt = window(native_scene_live);
    assert!(keyed(&rt, "preview-plan/plan-check/0").is_some(), "the done step's check");
    assert!(keyed(&rt, "preview-plan/plan-check/1").is_none(), "the running step has the orb instead");
}

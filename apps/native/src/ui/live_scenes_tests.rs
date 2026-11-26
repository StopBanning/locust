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

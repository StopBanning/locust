//! the-lockup-powers-on.test.tsx (5 cases): when Home's lockup lights, by the boot screen's preference (Full relights,
//! Subtle lights once, Off never) and the system's motion setting, and what it draws. Through the window's own poll on
//! Home headless; the expected values are PoweredLockup.tsx's.
use super::home_lockup_tests::{at_age, home, info, settle};
use super::*;
use crate::look::lockup::{Lighting, next_lighting_delay};
use locust_core::home_cover::{LightingPlan, lighting_plan};

/// Home after its first lighting has ended, in front: what it has planned next.
fn after_the_first(tube: &str) -> rune::Runtime<State> {
    let mut rt = home(tube, false, (1215.0, 1600.0));
    at_age(&mut rt, Lighting::Power, 3.0);
    settle(&mut rt);
    rt
}

// case: follows the boot screen preference: full relights, subtle once, off never
#[test]
fn full_relights_subtle_lights_once_and_off_never() {
    assert_eq!(lighting_plan("full", false), LightingPlan::Repeat);
    assert_eq!(lighting_plan("subtle", false), LightingPlan::Once);
    assert_eq!(lighting_plan("off", false), LightingPlan::None);

    let full = after_the_first("full");
    assert_eq!(full.state.home.cover_lit_count, 1);
    assert!(full.state.home.cover_relight_at.is_some(), "Full: the next lighting is due");
    let subtle = after_the_first("subtle");
    assert_eq!(subtle.state.home.cover_lit_count, 1, "Subtle: lit once");
    assert_eq!(subtle.state.home.cover_relight_at, None, "and nothing more is due");
    let off = home("off", false, (1215.0, 1600.0));
    assert_eq!(off.state.home.cover_lit_count, 0, "Off: never");
    assert!(off.find("home-cover-power-0").is_none());
}

// case: never, when the system asks for reduced motion
#[test]
fn never_lights_when_the_system_asks_for_reduced_motion() {
    assert_eq!(lighting_plan("full", true), LightingPlan::None);
    assert_eq!(lighting_plan("subtle", true), LightingPlan::None);
    for tube in ["full", "subtle"] {
        let mut rt = home(tube, true, (1215.0, 1600.0));
        settle(&mut rt);
        assert_eq!(rt.state.home.cover_lit_count, 0, "{tube}");
        assert_eq!(rt.state.home.cover_relight_at, None, "{tube}");
        assert!(rt.find("home-cover-power-0").is_none() && rt.find("home-cover-lockup-tube").is_none(), "{tube}");
    }
}

// case: comes back 7 to 10 seconds after it last ended
#[test]
fn comes_back_7_to_10_seconds_after_it_last_ended() {
    assert_eq!(next_lighting_delay(0.0), 7_000);
    assert_eq!(next_lighting_delay(1.0), 10_000);
    assert_eq!(next_lighting_delay(0.5), 8_500);
    // A random source that misbehaves still lands inside the window.
    assert_eq!(next_lighting_delay(7.0), 10_000);
    assert_eq!(next_lighting_delay(-1.0), 7_000);

    let rt = after_the_first("full");
    let wait = rt.state.home.cover_relight_at.unwrap() - cover_now();
    assert!((6.5..=10.0).contains(&wait), "due 7 to 10 s after the last one ended: {wait}");
    // Not while the window is behind others: a resting cover starts none.
    let mut away = after_the_first("full");
    away.update(|s, _| s.window_focused = false);
    settle(&mut away);
    assert_eq!(away.state.home.cover_relight_at, None);
}

// case: is the traced mark beside the name in live type, named Locust
#[test]
fn is_the_traced_mark_beside_the_name_in_live_type_named_locust() {
    let mut rt = home("subtle", false, (1215.0, 1600.0));
    at_age(&mut rt, Lighting::Power, 0.8);
    let image = info(&rt, "home-cover-lockup-img");
    assert_eq!((image.role, image.label.as_deref()), ("img", Some("Locust")));
    let all = rt.elements();
    let at = |key: &str| all.iter().position(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap();
    let kids = |i: usize| all.iter().filter(|e| e.parent == Some(i)).cloned().collect::<Vec<_>>();
    // The lockup and, laid over it, the same lockup once more for the lime.
    let lockup = kids(at("home-cover-lockup"));
    let lit = kids(at("home-cover-power-0"));
    assert_eq!(lockup.len(), 2);
    assert_eq!(lockup[0].text, None, "the mark first: the traced drawing, not type");
    assert_eq!(lockup[1].text.as_deref(), Some("LOCUST"), "then the name in live type");
    assert_eq!(lit.len(), 2);
    let size = |r: [f32; 4]| [r[2].round(), r[3].round()];
    assert_eq!(size(lit[0].rect), size(lockup[0].rect), "the second copy is the same mark");
    assert_eq!(lit[1].text, lockup[1].text);
    assert!(lockup[0].rect[0] + lockup[0].rect[2] <= lockup[1].rect[0], "the mark beside the name");
}

// case: is still until the runtimes have answered
#[test]
fn is_still_until_the_runtimes_have_answered() {
    let mut rt = home("full", false, (1215.0, 1600.0));
    rt.update(|s, _| {
        s.settings_panel.agents = None;
        s.home.cover_lit_count = 0;
        s.home.cover_power_frame = None;
    });
    settle(&mut rt);
    assert_eq!(rt.state.home.cover_lit_count, 0, "in front, Full, and still unlit: the agents have not answered");
    assert!(rt.find("home-cover-power-0").is_none() && rt.find("home-cover-lockup-tube").is_none());
    // Once they answer, it lights.
    rt.update(|s, _| s.settings_panel.agents = Some(Vec::new()));
    settle(&mut rt);
    assert_eq!(rt.state.home.cover_lit_count, 1);
}

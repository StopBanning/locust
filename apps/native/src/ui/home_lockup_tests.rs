//! the-lockup-lighting-is-drawn.test.ts (7 cases), drawn: Home's cover headless at 1215 wide, its lockup powered on
//! and relit through the window's own poll (`crate::poll` -> `home_panel::poll_cover`), and read off the frame. The
//! expected numbers are shell.css's and tokens.css's. The keyframes themselves are in look/lockup_tests.rs.
use super::*;
use crate::look::lockup::Lighting;
use rune::{Env, Runtime};

pub(super) fn home(tube: &str, reduced: bool, size: (f32, f32)) -> Runtime<State> {
    home_in(tube, reduced, size, true)
}

pub(super) fn home_in(tube: &str, reduced: bool, size: (f32, f32), dark: bool) -> Runtime<State> {
    let mut state = State::default();
    state.demo = true;
    state.home.home_open = true;
    state.settings_panel.agents = Some(Vec::new());
    state.settings_panel.settings.tube = tube.into();
    state.reduced_motion.set(reduced);
    state.appearance = Some(dark);
    let mut rt = Runtime::new(state, |s, _| home_view(s, &Palette::new(s.appearance.unwrap_or(true))), size, Env::new(dark, 1.0));
    rt.poll = Some(crate::poll);
    settle(&mut rt);
    rt
}

pub(super) fn settle(rt: &mut Runtime<State>) {
    for _ in 0..12 {
        rt.frame(0.05);
    }
}

/// Puts the lighting `age` seconds in on the cover's clock, and draws it.
pub(super) fn at_age(rt: &mut Runtime<State>, kind: Lighting, age: f64) {
    // Freeze the cover clock here, so the frames below draw exactly this age.
    let now = cover_now();
    super::COVER_CLOCK.with(|clock| clock.set(Some(now)));
    rt.update(|s, _| {
        s.home.cover_lit_at = Some(now - age);
        s.home.cover_lighting_kind = Some(kind);
    });
    rt.frame(0.0);
    rt.frame(0.0);
}

pub(super) fn info(rt: &Runtime<State>, key: &str) -> rune::ElementInfo {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no {key}"))
}

fn present(rt: &Runtime<State>, key: &str) -> bool {
    rt.elements().iter().any(|e| e.key.as_deref() == Some(key) && !e.exiting)
}

/// The lockup's opacity as drawn: the layer `lcLockupPower` and `lcLockupRelight` animate.
pub(super) fn lockup_ink(rt: &Runtime<State>) -> f32 {
    let all = rt.elements();
    let lockup = all.iter().position(|e| e.key.as_deref() == Some("home-cover-lockup")).expect("the lockup");
    all[all[lockup].parent.expect("its layer")].opacity
}

fn children_of(rt: &Runtime<State>, key: &str) -> Vec<rune::ElementInfo> {
    let all = rt.elements();
    let at = all.iter().position(|e| e.key.as_deref() == Some(key) && !e.exiting).unwrap_or_else(|| panic!("no {key}"));
    all.iter().filter(|e| e.parent == Some(at)).cloned().collect()
}

fn drawn_lockup() -> LockupDrawn {
    DRAWN_LOCKUP.with(|d| d.get())
}

fn inside(outer: [f32; 4], inner: [f32; 4]) -> bool {
    inner[0] >= outer[0] - 0.5 && inner[1] >= outer[1] - 0.5 && inner[0] + inner[2] <= outer[0] + outer[2] + 0.5 && inner[1] + inner[3] <= outer[1] + outer[3] + 0.5
}

// case: is the cover drawn to scale: its 960x254 numbers times the measured k
#[test]
fn the_cover_is_its_960_by_254_numbers_times_k() {
    let rt = home("off", false, (1215.0, 1600.0));
    // k measured off the drawing: the machine is 560 wide on the cover's 960 grid.
    let k = info(&rt, "home-cover-machine").rect[2] / 560.0;
    let card = info(&rt, "home-cover").rect;
    assert!((card[3] - 254.0 * k).abs() <= 1.0, "height: calc(254px * k): {} for k {k}", card[3]);
    let mark = children_of(&rt, "home-cover-lockup")[0].rect;
    assert!((mark[2] - 100.0 * k).abs() <= 1.0, "the mark: calc(100px * k): {}", mark[2]);
    let name = drawn_lockup().name.expect("the name is drawn");
    assert!((name.size - 48.0 * k).abs() <= 0.01, "the name: calc(48px * k): {}", name.size);
    let plate = rt.elements().into_iter().filter_map(|e| e.key).find(|k| k.starts_with("home-cover") && k.contains("plate"));
    assert_eq!(plate, None, "no plate: the lockup is on the glass");
}

// case: centres the claim under the lockup, in mono capitals, never under the 10.5px floor
#[test]
fn the_claim_is_centred_under_the_lockup_in_mono_capitals_never_under_10_5px() {
    for width in [1215.0, 640.0] {
        let rt = home("off", false, (width, 1600.0));
        let lockup = info(&rt, "home-cover-lockup").rect;
        let claim = info(&rt, "home-cover-line");
        let centre = |r: [f32; 4]| r[0] + r[2] / 2.0;
        assert!((centre(lockup) - centre(claim.rect)).abs() <= 1.5, "at {width}: lockup {lockup:?}, claim {:?}", claim.rect);
        assert!(claim.rect[1] >= lockup[1] + lockup[3] - 0.5, "under it");
        let said = claim.text.unwrap();
        assert_eq!(said, said.to_uppercase(), "text-transform: uppercase");
        let style = drawn_lockup().claim.unwrap();
        assert_eq!(style.family, rune::Family::Mono, "font-family: var(--lc-font-mono)");
        assert!(style.size >= 10.5, "font-size: max(10.5px, ..) at {width}: {}", style.size);
        assert!((style.tracking - 0.16 * style.size).abs() < 1e-3, "letter-spacing: 0.16em");
    }
}

// case: powers on from dark, lit lime, and ends in the app ink
#[test]
fn the_lockup_powers_on_from_dark_lit_lime_and_ends_in_the_glass_ink() {
    // --lc-lime is the ivory in the dark theme (the glass's own ink) and #1c1e1d in the light one: both are read.
    for dark in [true, false] {
        let mut rt = home_in("subtle", false, (1215.0, 1600.0), dark);
        assert_eq!(rt.state.home.cover_lit_count, 1, "the cover lit once the agents answered, in front");
        at_age(&mut rt, Lighting::Power, 0.0);
        assert!(lockup_ink(&rt) < 0.05, "0% {{ opacity: 0 }}: {}", lockup_ink(&rt));
        at_age(&mut rt, Lighting::Power, 0.3 * 2.6);
        let l = crate::look::controls::Look::new(dark);
        let lime = l.c("lime");
        assert_eq!(lime, if dark { color("#f2efe9") } else { color("#1c1e1d") }, "tokens.css --lc-lime");
        let lit = children_of(&rt, "home-cover-power-0");
        assert_eq!(lit.len(), 2, "the mark and the name, again");
        assert!(lit.iter().all(|e| e.text_color == Some(lime)), "color: var(--lc-lime) (dark {dark}): {lit:?}");
        assert!(info(&rt, "home-cover-power-0").opacity > 0.99, "lcLockupGlow 19%-50%: 1");
        // Past 2.6 s it has ended: the copy is gone, the lockup whole, in the glass's ink.
        at_age(&mut rt, Lighting::Power, 3.0);
        settle(&mut rt);
        assert!(!present(&rt, "home-cover-power-0"));
        assert!((lockup_ink(&rt) - 1.0).abs() < 1e-3, "100% {{ opacity: 1 }}");
        assert!(children_of(&rt, "home-cover-lockup").iter().all(|e| e.text_color == Some(l.c("boot-ink"))));
    }
}

// case: relights without ever going dark
#[test]
fn a_relight_on_home_never_goes_dark() {
    let mut rt = home("full", false, (1215.0, 1600.0));
    at_age(&mut rt, Lighting::Power, 3.0);
    settle(&mut rt);
    let due = rt.state.home.cover_relight_at.expect("Full relights: the next one is due");
    rt.update(|s, _| s.home.cover_relight_at = Some(cover_now() - 0.01));
    rt.frame(0.0);
    assert_eq!(rt.state.home.cover_lighting_kind, Some(Lighting::Relight), "relit, due at {due}");
    assert_eq!(rt.state.home.cover_lit_count, 2);
    let mut lowest = 1.0_f32;
    for tenth in 0..22 {
        at_age(&mut rt, Lighting::Relight, f64::from(tenth) / 10.0);
        lowest = lowest.min(lockup_ink(&rt));
    }
    assert!(lowest >= 0.7, "a flicker, never a blackout: {lowest}");
    assert!(lowest < 0.95, "it does flicker: {lowest}");
}

// case: lays the boot screen tube over it: its scanlines and its sweep
#[test]
fn the_tube_lies_over_the_lighting_lockup_scanlines_and_sweep() {
    let mut rt = home("subtle", false, (1215.0, 1600.0));
    at_age(&mut rt, Lighting::Power, 0.6);
    let glass = info(&rt, "home-cover-glass").rect;
    for key in ["home-cover-lockup-tube", "home-cover-lockup-scan"] {
        assert!(inside(glass, info(&rt, key).rect), "{key} on the glass");
    }
    assert!(present(&rt, "home-cover-lockup-sweep"));
    let all = rt.elements();
    let at = |key: &str| all.iter().position(|e| e.key.as_deref() == Some(key)).unwrap();
    assert!(at("home-cover-lockup-tube") > at("home-cover-lockup"), "laid over the lockup");
    let l = crate::look::controls::Look::new(true);
    let drawn = drawn_lockup();
    assert_eq!(drawn.scan, Some(l.c("boot-scan")), "var(--lc-boot-scan)");
    assert_eq!(drawn.sweep, Some(l.c("boot-glow-soft")), "var(--lc-boot-glow-soft)");
    // At rest there is no tube to draw.
    at_age(&mut rt, Lighting::Power, 3.0);
    settle(&mut rt);
    assert!(!present(&rt, "home-cover-lockup-tube"));
}

// case: sets the name the way the cover does: Figtree 700 capitals at the title tracking
#[test]
fn the_name_is_figtree_700_capitals_at_the_title_tracking() {
    let rt = home("off", false, (1215.0, 1600.0));
    let name = children_of(&rt, "home-cover-lockup").into_iter().find(|e| e.text.is_some()).expect("the name in live type");
    assert_eq!(name.text.as_deref(), Some("LOCUST"));
    let style = drawn_lockup().name.unwrap();
    assert_eq!(style.family, rune::Family::Sans, "var(--lc-font-ui): Figtree");
    assert_eq!(style.weight, 700.0);
    assert!((style.tracking - 0.14 * style.size).abs() < 1e-3, "var(--lc-title-tracking): 0.14em");
}

// case: stops for reduced motion
#[test]
fn reduced_motion_draws_the_still_lit_lockup_and_asks_for_no_frames() {
    let mut rt = home("full", true, (1215.0, 1600.0));
    assert_eq!(rt.state.home.cover_lit_count, 0, "nothing lit");
    assert!(!present(&rt, "home-cover-power-0") && !present(&rt, "home-cover-lockup-tube"));
    assert!((lockup_ink(&rt) - 1.0).abs() < 1e-3, "the still lockup is the finished one");
    // A lighting already under way when the setting turns on is not drawn either.
    at_age(&mut rt, Lighting::Power, 0.5);
    assert!(!present(&rt, "home-cover-power-0") && (lockup_ink(&rt) - 1.0).abs() < 1e-3);
}

/// HANDOFF-lockup-1: Home at rest stays cheap. Once the power-on ends the poll stops asking for frames.
#[test]
fn after_the_power_on_ends_no_frame_is_asked_for() {
    let mut rt = home("subtle", false, (1215.0, 1600.0));
    rt.update(|s, _| s.window_focused = false);
    at_age(&mut rt, Lighting::Power, 1.0);
    let clock = rt.wake_deadline();
    assert!(clock.is_some(), "while it lights, the poll asks for the next frame");
    at_age(&mut rt, Lighting::Power, 3.0);
    // The lit copy's own exit fade takes a few frames; then nothing at all is asked for.
    settle(&mut rt);
    settle(&mut rt);
    assert_eq!(rt.wake_deadline(), None, "ended: nothing asked for");
    assert_eq!(rt.state.home.cover_power_frame, None);
}

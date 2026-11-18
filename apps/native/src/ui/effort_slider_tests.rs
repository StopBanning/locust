//! Electron's `the-effort-control-is-claude-codes.test.tsx`, `the-effort-thumb-is-claude-codes-shape.test.ts` and
//! `the-effort-track-takes-the-pointer.test.ts`, drawn headless through `effort_panel` and driven with the pointer and
//! the keys. The cases that need no window (the level's words, the stops' numbers, MOVE) are in
//! `core/src/effort_levels_tests.rs`.
use super::*;
use rune::{Env, Runtime};
use serde_json::Value;

const FIVE: [&str; 5] = ["low", "medium", "high", "xhigh", "max"];
const T: &str = "tm_effort";

fn levels(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

/// The picked level, kept where the picker keeps it: the turn's pick for this conversation.
fn picked(s: &State) -> Option<String> {
    s.picks.get(T).and_then(|p| p.effort.clone())
}

thread_local! {
    /// The levels, runtime and appearance the view draws (a Runtime's view is a plain fn).
    static CASE: std::cell::RefCell<(Vec<String>, &'static str, bool)> = const { std::cell::RefCell::new((Vec::new(), "claude", true)) };
}

fn view(s: &State, _: &mut Cx<State>) -> El {
    let (supported, runtime, dark) = CASE.with(|c| c.borrow().clone());
    let l = crate::look::controls::Look::new(dark);
    let pick: PickLevel = Rc::new(|s: &mut State, level: String| {
        s.picks.insert(T.into(), TurnPick { model: "m".into(), effort: Some(level), runtime: None, account: None });
    });
    let effort = picked(s);
    col().w_full().pt(80.0).px(20.0).child(effort_panel(s, &l, &supported, effort.as_deref(), runtime, pick))
}

fn runtime_on(supported: &[&str], effort: Option<&str>, runtime: &'static str, dark: bool) -> Runtime<State> {
    let mut s = State::default();
    if let Some(e) = effort {
        s.picks.insert(T.into(), TurnPick { model: "m".into(), effort: Some(e.into()), runtime: None, account: None });
    }
    CASE.with(|c| *c.borrow_mut() = (levels(supported), runtime, dark));
    let mut rt = Runtime::new(s, view, (400.0, 400.0), Env::new(dark, 1.0));
    rt.frame(0.0);
    rt
}

fn draw(supported: &[&str], effort: &str) -> Runtime<State> {
    runtime_on(supported, Some(effort), "claude", true)
}

fn texts(rt: &Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter_map(|n| n.text).collect()
}

fn keyed(rt: &Runtime<State>, prefix: &str) -> usize {
    rt.elements().into_iter().filter(|n| n.key.as_deref().is_some_and(|k| k.starts_with(prefix))).count()
}

fn rect(rt: &Runtime<State>, key: &str) -> [f32; 4] {
    rt.presented(key).unwrap_or_else(|| panic!("no {key} drawn")).0
}

fn node<'a>(tree: &'a Value, key: &str) -> Option<&'a Value> {
    if tree["key"] == key {
        return Some(tree);
    }
    tree["children"].as_array()?.iter().find_map(|child| node(child, key))
}

fn a11y(rt: &Runtime<State>, key: &str) -> Value {
    node(&rt.accessibility_json(), key).unwrap_or_else(|| panic!("no {key} in the accessibility tree")).clone()
}

fn has_state(n: &Value, which: &str) -> bool {
    n["states"].as_array().is_some_and(|all| all.iter().any(|s| s == which))
}

// --- the-effort-control-is-claude-codes.test.tsx: the control

#[test]
fn the_control_reads_effort_max_with_a_question_mark_that_says_what_the_level_costs() {
    let mut rt = draw(&FIVE, "max");
    let said = texts(&rt);
    assert!(said.iter().any(|t| t == "Effort"), "{said:?}");
    assert!(said.iter().any(|t| t == "Max"), "{said:?}");
    let (x, y) = rt.find("effort-what").expect("the ?");
    rt.pointer_move(x, y);
    rt.frame(0.1);
    rt.frame(0.6);
    let tip = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("effort-what-tooltip")).and_then(|e| e.label);
    assert_eq!(tip.as_deref(), Some("Max: slowest, costliest"));
}

#[test]
fn the_control_names_the_ends_faster_to_smarter_and_draws_a_stop_for_every_level_the_model_has() {
    let rt = draw(&FIVE, "max");
    let said = texts(&rt);
    let faster = said.iter().position(|t| t == "Faster").expect("Faster");
    assert_eq!(said.get(faster + 1).map(String::as_str), Some("Smarter"), "{said:?}");
    assert_eq!(keyed(&rt, "effort-notch-"), 5);
    // Everything up to the level reads as passed.
    assert_eq!(keyed(&rt, "effort-notch-passed-"), 5);
    assert_eq!(keyed(&draw(&FIVE, "medium"), "effort-notch-passed-"), 2);
}

#[test]
fn the_control_puts_the_thumb_on_its_level() {
    let rt = draw(&FIVE, "high");
    let track = rect(&rt, "effort-scale");
    let thumb = rect(&rt, "effort-thumb");
    let want = stop_position(2, 5) - THUMB_WIDTH / 2.0;
    assert!((thumb[0] - track[0] - want).abs() < 0.01, "thumb at {} of the track, want {want}", thumb[0] - track[0]);
}

#[test]
fn the_control_has_one_thumb_and_no_drop() {
    let rt = draw(&FIVE, "max");
    assert_eq!(keyed(&rt, "effort-thumb"), 1);
    assert_eq!(keyed(&rt, "effort-drop"), 0);
}

#[test]
fn the_control_is_a_real_range_control_over_the_track_with_its_keys_its_snapping_and_its_name() {
    let mut rt = draw(&FIVE, "high");
    let slider = a11y(&rt, "effort-scale");
    assert_eq!(slider["role"], "slider", "{slider}");
    assert_eq!(slider["name"], "Reasoning effort");
    assert_eq!(slider["value"], "High", "{slider}");
    // [value, min, max, step]: min 0, max 4, step 1, value 2.
    assert_eq!(slider["range"], serde_json::json!([2.0, 0.0, 4.0, 1.0]), "{slider}");
    // Its keys: one level a press.
    rt.set_tab_visits_controls(true);
    rt.key(rune::KeyEvent { key: rune::Key::Tab, mods: Default::default() });
    rt.frame(0.016);
    assert!(has_state(&a11y(&rt, "effort-scale"), "focused"), "{}", a11y(&rt, "effort-scale"));
    rt.key(rune::KeyEvent { key: rune::Key::Right, mods: Default::default() });
    rt.frame(0.016);
    assert_eq!(picked(&rt.state).as_deref(), Some("xhigh"));
    rt.key(rune::KeyEvent { key: rune::Key::Left, mods: Default::default() });
    rt.frame(0.016);
    assert_eq!(picked(&rt.state).as_deref(), Some("high"));
    // One level: nothing to move.
    let one = draw(&["high"], "high");
    assert!(has_state(&a11y(&one, "effort-scale"), "disabled"), "{}", a11y(&one, "effort-scale"));
}

#[test]
fn the_control_offers_the_fast_variant_as_a_switch_where_the_model_has_one_and_says_it_in_the_name() {
    let rt = draw(&FIVE, "max");
    assert!(!texts(&rt).iter().any(|t| t == "Fast variant"));
    let cursor = ["low", "low-fast", "medium", "medium-fast", "high", "high-fast"];
    let mut rt = draw(&cursor, "high-fast");
    let switch = a11y(&rt, "effort-fast");
    assert_eq!(switch["role"], "switch", "{switch}");
    assert!(has_state(&switch, "checked"), "{switch}");
    assert!(texts(&rt).iter().any(|t| t == "High · Fast"), "{:?}", texts(&rt));
    let (x, y) = rt.find("effort-fast").unwrap();
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(0.016);
    assert_eq!(picked(&rt.state).as_deref(), Some("high"), "the switch off is the plain level the runtime listed");
}

// --- the-effort-thumb-is-claude-codes-shape.test.ts

#[test]
fn the_thumb_is_claude_codes_a_white_rounded_rectangle_16_by_20_with_4px_corners() {
    for dark in [true, false] {
        let rt = runtime_on(&FIVE, Some("medium"), "claude", dark);
        let thumb = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("effort-thumb")).unwrap();
        assert_eq!((thumb.rect[2], thumb.rect[3]), (16.0, 20.0));
        assert_eq!(thumb.radius, 4.0);
        assert_eq!(thumb.fill, rune::color::hex("#ffffff").unwrap(), "dark {dark}");
    }
}

#[test]
fn the_thumb_fills_the_tracks_height_as_claude_codes_does() {
    let rt = draw(&FIVE, "medium");
    assert_eq!(rect(&rt, "effort-scale")[3], 20.0);
    assert_eq!(rect(&rt, "effort-thumb")[3], rect(&rt, "effort-scale")[3]);
}

#[test]
fn the_thumb_is_one_thing_nothing_else_paints_it() {
    let rt = draw(&FIVE, "medium");
    let thumb = rect(&rt, "effort-thumb");
    assert_eq!(keyed(&rt, "effort-drop"), 0);
    // Nothing else filled is drawn on the thumb's box.
    let others: Vec<_> = rt
        .elements()
        .into_iter()
        .filter(|e| e.key.as_deref() != Some("effort-thumb") && e.fill.a > 0.0 && e.rect == thumb)
        .map(|e| e.key)
        .collect();
    assert!(others.is_empty(), "{others:?}");
}

#[test]
fn the_thumb_sits_on_a_fill_that_shows_as_claude_codes_does() {
    for dark in [true, false] {
        let fill = crate::look::controls::Look::new(dark).c("effort-fill");
        assert!(fill.a >= 0.3, "dark {dark}: {}", fill.a);
        let rt = runtime_on(&FIVE, Some("medium"), "claude", dark);
        let drawn = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("effort-fill")).unwrap();
        assert_eq!(drawn.fill, fill);
    }
}

// --- the-effort-track-takes-the-pointer.test.ts

#[test]
fn the_track_takes_the_pointer_and_picks_as_the_held_thumb_crosses_each_stop() {
    let mut rt = draw(&FIVE, "low");
    let track = rect(&rt, "effort-scale");
    let y = track[1] + 10.0;
    rt.pointer_down(track[0] + stop_position(0, 5), y);
    rt.frame(0.016);
    assert!(rt.state.effort_held.is_some(), "the track holds the thumb");
    rt.pointer_move(track[0] + stop_position(2, 5), y);
    rt.frame(0.016);
    assert_eq!(picked(&rt.state).as_deref(), Some("high"), "picked as it changes, not only on letting go");
    rt.pointer_move(track[0] + stop_position(3, 5) + 4.0, y);
    rt.frame(0.016);
    assert_eq!(picked(&rt.state).as_deref(), Some("xhigh"));
    rt.pointer_up(track[0] + stop_position(3, 5) + 4.0, y);
    rt.frame(0.016);
    assert_eq!(rt.state.effort_held, None, "let go, it settles on the stop");
    // It eases the last of the way (Electron's 320ms at rest); let it arrive.
    for t in [0.2, 0.5, 1.0, 2.0] {
        rt.frame(t);
    }
    let thumb = rect(&rt, "effort-thumb");
    assert!((thumb[0] - track[0] - (stop_position(3, 5) - THUMB_WIDTH / 2.0)).abs() < 0.01);
}

#[test]
fn the_track_does_not_start_a_selection_with_a_drag() {
    let mut rt = draw(&FIVE, "low");
    let track = rect(&rt, "effort-scale");
    let ends = rect(&rt, "effort-ends");
    let y = track[1] + 10.0;
    rt.pointer_down(track[0] + 20.0, y);
    rt.pointer_move(track[0] + 120.0, ends[1] + 4.0);
    rt.pointer_move(track[0] + 180.0, y);
    rt.pointer_up(track[0] + 180.0, y);
    rt.frame(0.016);
    let selected: Vec<_> = rt.elements().into_iter().filter_map(|e| e.selected).filter(|t| !t.is_empty()).collect();
    assert!(selected.is_empty(), "{selected:?}");
    // 180 along the track is nearest the last stop (203), not the fourth (155.5).
    assert_eq!(picked(&rt.state).as_deref(), Some("max"), "the drag went to the track");
}

#[test]
fn the_track_keeps_the_fill_under_a_held_thumb() {
    let mut rt = draw(&FIVE, "low");
    let track = rect(&rt, "effort-scale");
    let y = track[1] + 10.0;
    rt.pointer_down(track[0] + 30.0, y);
    rt.pointer_move(track[0] + 97.0, y);
    rt.frame(0.016);
    let fill = rect(&rt, "effort-fill");
    let thumb = rect(&rt, "effort-thumb");
    assert!((fill[2] - 97.0).abs() < 0.01, "fill {} wide", fill[2]);
    assert!((thumb[0] + THUMB_WIDTH / 2.0 - (fill[0] + fill[2])).abs() < 0.01, "the fill ends at the thumb's centre");
    rt.pointer_up(track[0] + 97.0, y);
}

#[test]
fn the_footer_says_how_cursor_takes_an_effort_and_nothing_elsewhere() {
    let rt = runtime_on(&["low", "high"], Some("low"), "cursor", true);
    assert!(texts(&rt).iter().any(|t| t == "Cursor puts the effort in the model name."));
    let rt = runtime_on(&["low", "high"], Some("low"), "claude", true);
    assert_eq!(keyed(&rt, "effort-footer"), 0);
}

// --- the control where the window draws it: the model picker's foot (Composer.tsx's RoutePicker effort slot)

fn picker_view(s: &State, _: &mut Cx<State>) -> El {
    let mate = locust_core::engine::Teammate {
        id: T.into(),
        name: "Wren".into(),
        role: "Coder".into(),
        runtime: Some("claude".into()),
        model: Some("haiku".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: None,
    };
    let effort = s.picks.get(T).and_then(|p| p.effort.clone());
    let model = s.picks.get(T).map(|p| p.model.clone()).unwrap_or_else(|| "haiku".into());
    col().w_full().child(crate::thread_view::model_picker(s, &crate::look::controls::Look::new(true), T, &mate, None, &model, effort.as_deref(), false))
}

#[test]
fn the_model_pickers_foot_is_the_effort_control_on_the_models_default_level() {
    let mut s = State::default();
    s.models.insert(
        T.into(),
        vec![locust_core::engine::Models {
            runtime: "claude".into(),
            model: Some("haiku".into()),
            effort: None,
            teammate: None,
            models: vec![locust_core::engine::Model { id: "haiku".into(), name: "Haiku 5.5".into(), efforts: levels(&["low", "medium", "high"]) }],
        }],
    );
    let mut rt = Runtime::new(s, picker_view, (600.0, 700.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let said = texts(&rt);
    assert!(!said.iter().any(|t| t == "Level"), "the Level chips are gone: {said:?}");
    assert_eq!(keyed(&rt, "effort-notch-"), 3);
    // No level chosen: the model's default, medium (status.ts defaultEffort).
    assert!(said.iter().any(|t| t == "Medium"), "{said:?}");
    let track = rect(&rt, "effort-scale");
    let y = track[1] + 10.0;
    rt.pointer_down(track[0] + stop_position(2, 3), y);
    rt.pointer_up(track[0] + stop_position(2, 3), y);
    rt.frame(0.016);
    let pick = rt.state.picks.get(T).expect("a pick for this turn");
    assert_eq!((pick.model.as_str(), pick.effort.as_deref()), ("haiku", Some("high")));
}

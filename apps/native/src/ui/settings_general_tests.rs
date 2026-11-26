//! Live hour 1, faults 22 and 23: General is grouped and has no gap before "Your account", and the header says what is
//! running, as Electron's (`ScreenHeader title="Settings" meta="Locust 0.882.0 · local only"`).
use super::*;

fn drawn(s: State) -> rune::Runtime<State> {
    let mut rt = rune::Runtime::new(s, |s, _| view(s, &Palette::new(true)), (1200.0, 5000.0), rune::Env::new(true, 1.0));
    for _ in 0..10 {
        rt.frame(0.05);
    }
    rt
}

fn general() -> State {
    let mut s = State::default();
    door_scene(&mut s);
    s.home.current_version = "0.882.0".into();
    s
}

fn texts(rt: &rune::Runtime<State>) -> Vec<String> {
    rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text).collect()
}

#[test]
fn the_header_says_settings_and_what_is_running() {
    // Electron's words, for an installed build and for a development one.
    assert_eq!(settings_meta("0.882.0", false), "Locust 0.882.0 · local only");
    assert_eq!(settings_meta("0.882.0", true), "Locust 0.882.0 · development build · local only");
    let said = texts(&drawn(general()));
    assert!(said.iter().any(|t| t == "Settings"), "{said:?}");
    let meta = settings_meta("0.882.0", cfg!(debug_assertions));
    assert!(said.iter().any(|t| *t == meta), "{meta:?} in {said:?}");
}

#[test]
fn the_header_names_the_version_this_build_is_when_home_has_not_been_told_one() {
    let mut s = general();
    s.home.current_version.clear();
    let said = texts(&drawn(s));
    let meta = settings_meta(env!("CARGO_PKG_VERSION"), cfg!(debug_assertions));
    assert!(said.iter().any(|t| *t == meta), "{meta:?} in {said:?}");
}

#[test]
fn the_lines_to_add_locust_by_hand_stay_shut_until_pressed() {
    let mut rt = drawn(general());
    let said = texts(&rt).join("\n");
    assert!(said.contains(BY_HAND), "{said}");
    assert!(!said.contains("claude mcp add"), "the commands are not drawn while it is shut: {said}");
    let (x, y) = rt.find(BY_HAND_KEY).expect("the fold's line");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..10 {
        rt.frame(0.05);
    }
    let opened = texts(&rt).join("\n");
    assert!(opened.contains("claude mcp add"), "{opened}");
}

#[test]
fn nothing_but_the_sections_own_spacing_stands_between_a_section_and_the_one_after() {
    // The gap before "Your account" was a whole screen of commands standing open above it.
    let rt = drawn(general());
    let all = rt.elements();
    let heading = all.iter().find(|e| e.text.as_deref() == Some("Your account")).expect("Your account");
    let above = all
        .iter()
        .filter(|e| !e.exiting && e.rect[3] > 0.0 && e.rect[0] > 200.0 && e.rect[1] + e.rect[3] <= heading.rect[1] + 0.5)
        .map(|e| e.rect[1] + e.rect[3])
        .fold(0.0f32, f32::max);
    assert!(heading.rect[1] - above < 40.0, "{}px of nothing before Your account", heading.rect[1] - above);
    let door = all.iter().find(|e| e.key.as_deref() == Some("door-card")).expect("the door's lines are one card");
    assert!(door.rect[1] + door.rect[3] < heading.rect[1], "the card ends above Your account: {:?} {:?}", door.rect, heading.rect);
    assert!(door.rect[3] < 400.0, "the card is the three switches and one line, not the commands: {:?}", door.rect);
}

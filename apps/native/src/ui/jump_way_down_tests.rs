//! the-way-down-says-a-run-is-still-working.test.tsx (3 cases, 0.703): the way back down wears the thinking dots
//! while a run works and says so to a screen reader, is the plain chevron when nothing runs, is absent at the bottom,
//! and brings the chevron back on hover and on keyboard focus, where it is about to be pressed.
use super::*;

type Rt = rune::Runtime<(State, (bool, bool))>;

fn draw(shown: bool, working: bool) -> Rt {
    let mut rt = rune::Runtime::new(
        (State::default(), (shown, working)),
        |(_, (shown, working)), _| {
            let p = Palette::new(true);
            col().w_full().h_full().children(view(*shown, *working, false, &p))
        },
        (400.0, 300.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt
}

fn find(rt: &Rt, key: &str) -> Option<rune::ElementInfo> {
    rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key))
}

// case: wears the thinking dots while the run works, and says so to a screen reader
#[test]
fn wears_the_dots_while_working_and_says_so() {
    let rt = draw(true, true);
    let button = find(&rt, "jump-to-bottom").expect("the way down is drawn");
    assert_eq!(button.role, "button");
    assert_eq!(button.label.as_deref(), Some("Still working. Go to the newest message"));
    assert!(find(&rt, "jump-to-bottom-dots").is_some(), "the thinking dots are worn");
}

// case: is the plain chevron when nothing is running, and absent at the bottom
#[test]
fn is_the_plain_chevron_when_idle_and_absent_at_the_bottom() {
    let rt = draw(true, false);
    assert!(find(&rt, "jump-to-bottom-dots").is_none());
    assert!(find(&rt, "jump-to-bottom-chevron").is_some());
    assert_eq!(find(&rt, "jump-to-bottom").unwrap().label.as_deref(), Some("Go to the newest message"));
    let at_bottom = draw(false, true);
    assert!(find(&at_bottom, "jump-to-bottom").is_none());
    assert!(find(&at_bottom, "jump-to-bottom-dots").is_none());
}

// case: brings the chevron back on hover and keyboard focus, where it is about to be pressed
#[test]
fn tab_reaches_the_way_down_and_its_chevron_comes_back() {
    let mut rt = draw(true, true);
    assert!(find(&rt, "jump-to-bottom").unwrap().focusable, "a keyboard can reach the way down, as Electron's <button> can");
    rt.key(rune::KeyEvent { key: rune::Key::Tab, mods: Default::default() });
    for _ in 0..30 {
        rt.frame(1.0 / 60.0);
    }
    let button = find(&rt, "jump-to-bottom").unwrap();
    assert!(button.focused, "Tab lands on the way down");
    assert!(button.hover > 0.99, "keyboard focus counts as the hover that swaps the glyphs: {}", button.hover);
    let chevron = find(&rt, "jump-to-bottom-chevron").unwrap().opacity;
    let dots = find(&rt, "jump-to-bottom-dots").unwrap().opacity;
    assert!(chevron > 0.99 && dots < 0.01, "focused, the chevron shows and the dots go: chevron {chevron}, dots {dots}");
}

#[test]
fn nothing_hovers_the_way_down_at_rest() {
    let rt = draw(true, true);
    assert!(find(&rt, "jump-to-bottom").unwrap().hover < 0.01);
    let chevron = find(&rt, "jump-to-bottom-chevron").unwrap().opacity;
    let dots = find(&rt, "jump-to-bottom-dots").unwrap().opacity;
    assert!(chevron < 0.01 && dots > 0.29, "at rest the dots show and the chevron waits under them: chevron {chevron}, dots {dots}");
}

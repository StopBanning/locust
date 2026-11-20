//! the-plus-menu-says-what-each-droplet-does.test.tsx (3 cases). Rust draws the + menu as a menu card, not Electron's
//! liquid droplets (triage 18 Nov), so the cases are carried on what they promise: each choice is named in words,
//! outside the + control and after it; the words exist (and can be pressed) only while the menu is open; and they sit
//! on a raised surface of their own in both themes. Drawn headless through the conversation view.
use super::*;
use crate::scenes::composer_base_scene;

type Rt = rune::Runtime<State>;

fn drawn(open: bool, light: bool) -> Rt {
    let mut s = State::default();
    composer_base_scene(&mut s, light);
    s.plus_menu = open;
    let mut rt = rune::Runtime::new(s, |s, _| thread_view(s, &Palette::new(s.appearance.unwrap_or(true))), (1200.0, 1000.0), rune::Env::new(!light, 1.0));
    rt.frame(0.0);
    rt
}

fn find(rt: &Rt, key: &str) -> Option<(usize, rune::ElementInfo)> {
    rt.elements().into_iter().enumerate().find(|(_, e)| e.key.as_deref() == Some(key))
}

fn inside(rt: &Rt, child: usize, ancestor: usize) -> bool {
    let all = rt.elements();
    let mut at = all[child].parent;
    while let Some(i) = at {
        if i == ancestor {
            return true;
        }
        at = all[i].parent;
    }
    false
}

// case: names each droplet in words, after the liquid and outside it
#[test]
fn names_each_choice_in_words_outside_the_plus_control() {
    let rt = drawn(true, false);
    let (plus, _) = find(&rt, "plus").expect("the + control");
    let (attach, a) = find(&rt, "plus-attach").expect("Attach files");
    let (folder, f) = find(&rt, "plus-folder").expect("Choose a folder");
    assert_eq!(a.label.as_deref(), Some("Attach files"));
    assert_eq!(f.label.as_deref(), Some("Choose a folder"));
    let words: Vec<String> = rt.elements().into_iter().filter_map(|e| e.text).collect();
    assert!(words.iter().any(|t| t == "Attach files") && words.iter().any(|t| t == "Choose a folder"), "{words:?}");
    assert!(!inside(&rt, attach, plus) && !inside(&rt, folder, plus), "the words are outside the + control");
}

// case: shows the words only while the menu is open, and lets a press through only then
#[test]
fn the_words_exist_only_while_the_menu_is_open() {
    let closed = drawn(false, false);
    assert!(find(&closed, "plus").is_some());
    assert!(find(&closed, "plus-attach").is_none() && find(&closed, "plus-folder").is_none());
    assert!(!closed.elements().into_iter().filter_map(|e| e.text).any(|t| t == "Attach files"));
    let open = drawn(true, false);
    let (_, a) = find(&open, "plus-attach").unwrap();
    assert_eq!(a.role, "menuitem", "open, a choice takes a press");
}

// case: draws the words on the liquid’s own surface, in both themes
#[test]
fn the_words_sit_on_a_raised_surface_of_their_own_in_both_themes() {
    let fills: Vec<rune::Color> = [false, true]
        .into_iter()
        .map(|light| {
            let rt = drawn(true, light);
            let (menu, m) = find(&rt, "plus-menu").expect("the menu card");
            let (attach, _) = find(&rt, "plus-attach").unwrap();
            assert!(inside(&rt, attach, menu), "the words are on the menu's surface");
            assert!(m.fill.a > 0.99, "an opaque surface, light={light}: {:?}", m.fill);
            m.fill
        })
        .collect();
    assert_ne!(fills[0], fills[1], "each theme draws its own surface");
}

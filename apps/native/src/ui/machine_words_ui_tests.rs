//! a-mac-says-menu-bar-and-log-in.test.tsx (0.843), through the window's own places: the Routines note drawn as a Mac
//! draws it, and no file the window draws a computer from names one machine for every machine. The words themselves are
//! `locust_core::machine_words`'s tests.

use super::*;

fn note_words(view: fn(&State, &mut rune::Cx<State>) -> El) -> Vec<String> {
    let mut rt = rune::Runtime::new(
        State::default(),
        view,
        (900.0, 200.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.elements().iter().filter_map(|e| e.text.clone()).collect()
}

#[test]
fn the_routines_note_says_log_in_on_a_mac_and_sign_in_elsewhere() {
    let mac = note_words(|s, _| col().child(login_note_on(s, &Palette::new(true), "darwin")));
    assert!(mac.iter().any(|w| w == "Routines run while Locust is open. Start Locust when you log in:"), "{mac:?}");
    assert!(!mac.iter().any(|w| w.contains("sign in")), "{mac:?}");
    let pc = note_words(|s, _| col().child(login_note_on(s, &Palette::new(true), "win32")));
    assert!(pc.iter().any(|w| w == "Routines run while Locust is open. Start Locust when you sign in:"), "{pc:?}");
}

// F8: Settings > AI agents said "this PC's sign-in" on a Mac in Electron, and "this Mac's sign-in" on every machine
// here. Each place now asks the machine.
#[test]
fn names_the_computer_by_the_machine_never_this_pc_in_the_words_themselves() {
    let files = [
        ("agents_section.rs", include_str!("agents_section.rs")),
        ("quit_guard_dialog.rs", include_str!("quit_guard_dialog.rs")),
        ("routines_panel.rs", include_str!("routines_panel.rs")),
    ];
    for (name, source) in files {
        for fixed in ["this PC", "This PC", "this Mac's sign-in", "on this Mac…", "Start Locust when you sign in\"", "{ \"log in\" } else { \"sign in\" }"] {
            assert!(!source.contains(fixed), "{name} still says {fixed:?} for every machine");
        }
    }
}

//! the-window-says-when-macos-may-ask.test.tsx (2 cases). The note is drawn only while macOS may be asking, as an
//! alert that names the box and the answer. Electron's second case (the window waits for its listener before asking the
//! host) is Electron's bridge order; Rust's window reads the one flag itself, so the case is carried as that flag alone
//! drawing the note.
use super::*;
use rune::{Env, Runtime};

fn window(asking: bool) -> Runtime<State> {
    let mut s = State::default();
    s.keychain_asking = asking;
    let mut rt = Runtime::new(s, crate::view, (1200.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

// case: is drawn only while macOS may be asking, and names the box and the answer
#[test]
fn drawn_only_while_macos_may_ask_and_names_the_box_and_the_answer() {
    assert!(window(false).elements().into_iter().all(|e| e.key.as_deref() != Some("keychain-note")));
    let rt = window(true);
    let note = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("keychain-note")).expect("the note");
    assert_eq!(note.role, "alert");
    let said = rt.elements().into_iter().filter_map(|e| e.text).collect::<Vec<_>>().join("\n");
    assert!(said.contains("“@teammate/desktop Safe Storage”"), "{said}");
    assert!(said.contains("choose Always Allow"));
    assert!(said.contains("behind this window"));
}

// case: the window takes the host’s word, and asks for it only once its listener is in place
#[test]
fn the_window_draws_the_note_from_the_one_flag() {
    let mut rt = window(false);
    rt.update(|s, _| s.keychain_asking = true);
    rt.frame(0.0);
    assert!(rt.elements().into_iter().any(|e| e.key.as_deref() == Some("keychain-note")));
    rt.update(|s, _| s.keychain_asking = false);
    rt.frame(0.0);
    assert!(!rt.elements().into_iter().any(|e| e.key.as_deref() == Some("keychain-note") && !e.exiting));
}

//! palette-has-no-inner-ring.test.ts through the window: Ctrl K (the window's `on_key`) opens the palette in the whole window view
//! (`crate::view`, which hands `command_palette` whether its field holds the keyboard), and the draw records the
//! head's bottom edge and the field's own ring (`PALETTE_DRAWN`; Rune does not list borders or rings). Expected
//! colours are the tokens Electron's shell.css names: var(--lc-border-hover) lit, var(--lc-border-hairline) at rest.
use super::*;
use rune::{Env, Key, KeyEvent, Runtime};

fn opened_palette() -> Runtime<State> {
    let mut rt = Runtime::new(State::default(), crate::view, (1200.0, 800.0), Env::new(true, 1.0));
    rt.frame(0.0);
    // Ctrl K, as the window's own key handler takes it.
    let ctrl_k = KeyEvent { key: Key::Command('k'), mods: rune::Mods { cmd: true, ..Default::default() } };
    rt.update(move |s, tx| {
        assert!(on_key(s, &ctrl_k, tx));
    });
    rt.frame(0.016);
    assert!(rt.state.palette_open, "Ctrl K opens the palette");
    rt
}

fn drawn_palette() -> ((f32, rune::Color), Option<(f32, f32, rune::Color)>) {
    PALETTE_DRAWN.with(|c| c.get()).expect("the palette was drawn")
}

#[test]
fn turns_the_global_focus_ring_off_at_a_specificity_that_beats_it() {
    // case: turns the global focus ring off at a specificity that beats it
    let rt = opened_palette();
    let field = rt.elements().into_iter().find(|e| e.key.as_deref() == Some("palette-query")).expect("the search field");
    assert!(field.focused, "the field holds the keyboard as the palette opens");
    // .lc-palette__input:focus-visible { outline: none }: the focused field wears no ring of its own.
    assert_eq!(drawn_palette().1, None);
}

#[test]
fn keeps_the_heads_lit_edge_as_the_focus_it_shows() {
    // case: keeps the head’s lit edge as the focus it shows
    let l = crate::look::controls::Look::new(true);
    let mut rt = opened_palette();
    // .lc-palette__head:focus-within { border-bottom-color: var(--lc-border-hover) }: a step brighter, not ivory.
    assert_eq!(drawn_palette().0, (1.0, l.c("border-hover")));
    assert_ne!(l.c("border-hover"), l.c("text-primary"), "not ivory");
    // Focus leaves the field: the edge dims back to the head's hairline (border-bottom: 1px solid var(--lc-border-hairline)).
    rt.update(|s, _| s.palette_query.focused = false);
    rt.frame(0.032);
    assert!(rt.state.palette_open);
    assert_eq!(drawn_palette().0, (1.0, l.c("border-hairline")));
}

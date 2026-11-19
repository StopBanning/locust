//! "How it works" (Screens.tsx `More`): the explanation a setting used to open with, folded under one line. Closed
//! until pressed; a chevron and the words in sentence case, as the Settings pane says them
//! (`.lc-settings__pane .lc-settings__more > summary { text-transform: none }`), then the text. Which folds are open
//! is kept by key, so each setting's fold opens on its own.

use super::*;
use crate::settings_pages::HOW_IT_WORKS;

/// The folds that are open, by key.
#[derive(Default)]
pub(crate) struct Folds {
    open: std::collections::HashSet<String>,
}

impl Folds {
    pub fn is_open(&self, key: &str) -> bool {
        self.open.contains(key)
    }

    pub fn toggle(&mut self, key: &str) {
        if !self.open.remove(key) {
            self.open.insert(key.to_owned());
        }
    }
}

/// The fold for `key`: its summary (`how-it-works-<key>`, a button that says whether it is open) and, once open, its
/// paragraphs (`how-it-works-<key>-text`). `toggle` flips the key in whichever `Folds` the screen keeps.
pub(crate) fn fold(
    l: &crate::look::controls::Look,
    folds: &Folds,
    key: &str,
    paragraphs: &[&str],
    toggle: impl Fn(&mut State) + 'static,
) -> El {
    let open = folds.is_open(key);
    let chevron = if open { &crate::look::icons::CHEVRON_DOWN } else { &crate::look::icons::CHEVRON_RIGHT };
    let summary = row()
        .key(format!("how-it-works-{key}"))
        .role("button")
        .label(HOW_IT_WORKS)
        .expanded(open)
        .focusable(true)
        .focus_ring(2.0, 2.0, l.c("focus-ring"))
        .gap(6.0)
        .center_y()
        .child(crate::look::controls::glyph(l, chevron, 10.0, Some(l.c("text-muted"))))
        .child(text(HOW_IT_WORKS).font(l.ty.body).color(l.c("text-muted")))
        .on_press(toggle);
    let mut body = col().w_full().gap(4.0).px(10.0).pb(10.0).child(summary);
    if open {
        let mut words = col().key(format!("how-it-works-{key}-text")).w_full().gap(12.0).pt(12.0);
        for paragraph in paragraphs {
            words = words.child(text((*paragraph).to_owned()).font(l.ty.body).color(l.c("text-muted")).wrap());
        }
        body = body.child(words);
    }
    body
}

//! the-cover-glass-reads-in-both-themes.test.ts (2 cases): the cover writes its wordmark and its claim in the glass's
//! own ink, and that ink is one colour in both themes, as the glass is. Read from the drawn cover in each theme.
use super::*;
use locust_core::agents::{Auth, Found, Probe};

fn drawn(dark: bool) -> Vec<rune::ElementInfo> {
    let mut s = State::default();
    s.settings_panel.agents = Some(vec![Found { id: "codex".into(), display_name: "Codex CLI".into(), installed: true, version: Some("1.0.0".into()), auth: Auth::Authenticated, ready: true, status: Probe::Ready, unsupported_note: None, through_cli: false, checking: false }]);
    s.appearance = Some(dark);
    let mut rt = rune::Runtime::new(s, |s, _| home_view(s, &Palette::new(s.appearance.unwrap_or(true))), (1215.0, 1600.0), rune::Env::new(dark, 1.0));
    for _ in 0..8 {
        rt.frame(0.25);
    }
    rt.elements()
}

fn inks(all: &[rune::ElementInfo]) -> (Vec<Option<rune::Color>>, Option<rune::Color>) {
    let lockup = all.iter().position(|e| e.key.as_deref() == Some("home-cover-lockup")).expect("the lockup");
    let marks = all.iter().filter(|e| e.parent == Some(lockup)).map(|e| e.text_color).collect();
    let claim = all.iter().find(|e| e.key.as_deref() == Some("home-cover-line")).expect("the claim").text_color;
    (marks, claim)
}

// case: writes the wordmark and the claim in its own ink
#[test]
fn the_wordmark_and_the_claim_are_in_the_glass_ink() {
    let l = crate::look::controls::Look::new(true);
    let (marks, claim) = inks(&drawn(true));
    assert_eq!(marks.len(), 2, "the mark and the wordmark");
    assert!(marks.iter().all(|c| *c == Some(l.c("boot-ink"))), "{marks:?}");
    assert_eq!(claim, Some(l.c("boot-ink-muted")));
}

// case: keeps that ink the same in both themes, as the glass is
#[test]
fn the_ink_is_the_same_in_both_themes() {
    for token in ["--lc-boot-ink:", "--lc-boot-ink-muted:", "--lc-boot-screen-hot:"] {
        assert_eq!(crate::look::TOKENS_CSS.matches(token).count(), 1, "{token} is declared once, for both themes");
    }
    assert_eq!(inks(&drawn(true)), inks(&drawn(false)), "light or dark, the glass reads the same");
}

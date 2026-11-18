//! Electron's `pull-request-news-is-said.test.tsx`, case by case with its values and words, on
//! `pull_request_chip.rs`'s `news_banner_view`, `pull_request_ask` and `watch_toggle`, drawn headless.
//! Electron's `is-plain` class and `aria-pressed` are read as what Rust draws for them: the banner's plain fill
//! (`bg-raised`, where news to fix is `amber-tint`) and the toggle's "Watching" with its lime fill.
use super::*;
use rune::{Env, Runtime};

struct Harness {
    state: State,
    build: Box<dyn Fn(&State) -> El>,
}
fn view(h: &Harness, _: &mut Cx<Harness>) -> El {
    col().w_full().h_full().child((h.build)(&h.state))
}
fn drawn(build: impl Fn(&State) -> El + 'static) -> Runtime<Harness> {
    let mut rt = Runtime::new(Harness { state: State::default(), build: Box::new(build) }, view, (800.0, 200.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}
fn words(rt: &Runtime<Harness>) -> String {
    rt.elements().into_iter().flat_map(|n| [n.text, n.label]).flatten().collect::<Vec<_>>().join("\n")
}
fn fill_of(rt: &Runtime<Harness>, key: &str) -> rune::Color {
    rt.elements().into_iter().find(|n| n.key.as_deref() == Some(key)).expect("drawn").fill
}
fn news() -> PullRequestNews {
    PullRequestNews { number: 12, url: "https://github.com/acme/shop/pull/12".into(), said: "1 of 3 checks did not pass".into(), needs_work: true }
}

#[test]
fn says_what_changed_and_offers_the_teammate_the_fix_as_words_in_the_box() {
    let shown = news();
    let rt = drawn(move |_| news_banner_view(&shown, Some("Wren"), &Palette::new(true)));
    let said = words(&rt);
    assert!(said.contains("Pull request #12: 1 of 3 checks did not pass."), "{said}");
    assert!(said.contains("Ask Wren to fix it"));
    assert!(said.contains("Open on GitHub"));
    assert_eq!(
        pull_request_ask(&news()),
        "Pull request #12 (https://github.com/acme/shop/pull/12): 1 of 3 checks did not pass. Look at what is wrong, fix it on this branch, and push the fix."
    );
}

#[test]
fn offers_no_fix_for_news_with_nothing_to_fix() {
    let approved = PullRequestNews { said: "It was approved".into(), needs_work: false, ..news() };
    let rt = drawn(move |_| news_banner_view(&approved, Some("Wren"), &Palette::new(true)));
    assert!(!words(&rt).contains("to fix it"));
    let look = crate::look::controls::Look::new(true);
    assert_eq!(fill_of(&rt, "pr-news-banner"), look.c("bg-raised"));
    let needs = news();
    let to_fix = drawn(move |_| news_banner_view(&needs, Some("Wren"), &Palette::new(true)));
    assert_ne!(fill_of(&to_fix, "pr-news-banner"), look.c("bg-raised"));
}

#[test]
fn has_a_watch_toggle_that_says_which_it_is() {
    let off = drawn(|_| watch_toggle(false, &Palette::new(true)));
    assert!(words(&off).lines().any(|w| w == "Watch"));
    let on = drawn(|_| watch_toggle(true, &Palette::new(true)));
    assert!(words(&on).lines().any(|w| w == "Watching"));
    let look = crate::look::controls::Look::new(true);
    assert_eq!(fill_of(&on, "pr-watch-toggle"), look.c("lime-tint"));
    assert_ne!(fill_of(&off, "pr-watch-toggle"), look.c("lime-tint"));
}

//! the-runtimes-wear-their-marks.test.tsx, "the marks": Electron reads `RUNTIME_MARKS` and `RuntimeMark`. The Rust app draws
//! the same marks, generated from Electron's own file by `scripts/marks.py` (`icons.rs`), so the same cases read the
//! generated drawings. Cases about where a mark is placed are in the window tests.

use super::icons::{self, RUNTIMES};

/// `EVERY_RUNTIME` in the Electron test.
const EVERY_RUNTIME: [&str; 10] = ["codex", "claude", "cursor", "gemini", "opencode", "copilot", "antigravity", "muse", "kilo", "acp"];

fn body(runtime: &str) -> &'static str {
    icons::mark(runtime).unwrap_or_else(|| panic!("no mark for {runtime}")).body
}

// "cover every runtime a teammate can run on".
#[test]
fn the_marks_cover_every_runtime_a_teammate_can_run_on() {
    for runtime in EVERY_RUNTIME {
        let mark = icons::mark(runtime).unwrap_or_else(|| panic!("no mark for {runtime}"));
        assert!(mark.body.contains("<path"), "{runtime} has a drawing");
    }
    let mut listed: Vec<&str> = RUNTIMES.to_vec();
    listed.sort_unstable();
    let mut every = EVERY_RUNTIME.to_vec();
    every.sort_unstable();
    assert_eq!(listed, every);
}

// "are bundled: nothing in the module is a place to fetch one from".
#[test]
fn the_marks_are_bundled_nothing_in_them_is_a_place_to_fetch_one_from() {
    for runtime in EVERY_RUNTIME {
        let drawn = body(runtime);
        assert!(!drawn.contains("http://") && !drawn.contains("https://"), "{runtime} names an address");
        for bad in ["fetch(", "new Image", "<img", "<image", "xlink:href", "data:image"] {
            assert!(!drawn.contains(bad), "{runtime} draws with {bad}");
        }
    }
}

// "draw Codex as OpenAI's blossom, as Colin asked (0.385), in the words' colour".
#[test]
fn codex_is_openais_blossom_in_the_words_colour() {
    let codex = body("codex");
    assert_eq!(codex.matches("<path").count(), 1, "one path");
    assert!(codex.contains(r#"d="M22.2819 9.8211a5.9847 5.9847 0 0 0-.5157-4.9108"#));
    assert!(!codex.contains("Gradient"), "no gradient");
    assert!(codex.contains(r#"fill="currentColor""#));
}

// "draw Gemini in Google's own gradient, its stops tokens" -- the Rust drawing carries the stops' values, which are the
// tokens' (`--lc-mark-gemini-violet`, `-blue`, `-cyan` in tokens.css, read by `scripts/marks.py`), in a 28 box.
#[test]
fn gemini_is_drawn_in_googles_gradient_with_the_token_stops_in_a_28_box() {
    let gemini = body("gemini");
    assert!(gemini.contains("<radialGradient"));
    for stop in ["#9168c0", "#5684d1", "#1ba1e3"] {
        assert!(gemini.contains(&format!(r#"stop-color="{stop}""#)), "{stop}");
    }
    assert_eq!(icons::mark("gemini").unwrap().view, 28.0);
}

// "draw Antigravity's own arch": Electron's viewBox '14 14.2 83.6 83.6' is a 83.6 box moved by (-14, -14.2).
#[test]
fn antigravity_is_its_own_arch_in_its_own_box() {
    let arch = icons::mark("antigravity").unwrap();
    assert_eq!(arch.view, 83.6);
    assert!(arch.body.contains(r#"translate(-14 -14.2)"#));
    assert!(arch.body.contains("M89.754 92.75"));
}

// "draw nothing for what no teammate runs on".
#[test]
fn nothing_is_drawn_for_what_no_teammate_runs_on() {
    assert!(icons::mark("omniroute").is_none());
    assert!(icons::mark("").is_none());
}

// "the picker's runtime groups carry their marks" (and the rows that name a runtime): each is drawn by `controls::agent_mark`,
// which gives a mark for every runtime that has one and nothing for one that does not.
#[test]
fn a_mark_is_drawn_for_every_runtime_that_has_one_and_nothing_for_one_that_does_not() {
    let l = crate::look::controls::Look::new(true);
    for runtime in EVERY_RUNTIME {
        assert!(crate::look::controls::agent_mark(&l, runtime, 12.0, l.c("text-muted")).is_some(), "{runtime}");
    }
    assert!(crate::look::controls::agent_mark(&l, "omniroute", 12.0, l.c("text-muted")).is_none());
}


// "are decorative beside a name, and named where they stand in for one" and "grey Gemini with its words".

thread_local! {
    static VIEW: std::cell::Cell<Option<fn(&crate::State) -> rune::El>> = const { std::cell::Cell::new(None) };
}

fn a11y_of(el_fn: fn(&crate::State) -> rune::El) -> serde_json::Value {
    VIEW.with(|v| v.set(Some(el_fn)));
    let mut rt = rune::Runtime::new(crate::State::default(), |s, _| VIEW.with(|v| v.get()).unwrap()(s), (60.0, 60.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.accessibility_json()
}

#[test]
fn a_mark_is_decorative_beside_a_name_and_named_where_it_stands_in_for_one() {
    let plain = a11y_of(|_| rune::el().child(crate::look::controls::runtime_mark(&crate::look::controls::Look::new(true), "claude", 14.0, rune::Color::WHITE, None, false).unwrap()));
    assert!(!plain.to_string().contains("\"image\""), "{plain}");
    let named = a11y_of(|_| {
        rune::el().child(crate::look::controls::runtime_mark(&crate::look::controls::Look::new(true), "copilot", 14.0, rune::Color::WHITE, Some("Copilot CLI"), false).unwrap())
    });
    let text = named.to_string();
    assert!(text.contains("\"role\":\"image\",\"name\":\"Copilot CLI\""), "{text}");
}

#[test]
fn gemini_greys_with_its_words_no_gradient_when_muted() {
    let muted = crate::look::controls::mark_drawn("gemini", true).unwrap();
    assert!(!muted.body.contains("radialGradient"));
    assert!(muted.body.contains("fill=\"currentColor\""));
    assert!(crate::look::controls::mark_drawn("gemini", false).unwrap().body.contains("radialGradient"), "not muted: the gradient");
    assert_eq!(crate::look::controls::mark_drawn("codex", true).map(|i| i.name), Some("mark-codex"), "the others keep their drawing");
    assert!(crate::look::controls::mark_drawn("omniroute", true).is_none());
}

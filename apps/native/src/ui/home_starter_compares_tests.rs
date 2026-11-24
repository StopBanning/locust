//! Electron's `a-home-starter-starts-with-a-teammate-or-compares.test.ts`, case by case, on the starter rows Home
//! draws (`home_view`): a row each, the row for the teammate, a quiet Compare beside every row, nothing sent until Send.
use super::*;

fn home() -> rune::Runtime<State> {
    let mut s = State::default();
    home_scene(&mut s);
    let mut rt = rune::Runtime::new(s, crate::view, (1200.0, 3000.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt
}

/// The rows drawn, top to bottom then left to right, as the starter's own button names them.
fn rows(rt: &rune::Runtime<State>) -> Vec<String> {
    let mut drawn: Vec<_> = rt.elements().into_iter()
        .filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("starter-") && !k.starts_with("starter-compare-")))
        .filter_map(|e| {
            let label = rt.elements().into_iter().find(|x| x.path.starts_with(&format!("{}/", e.path)) && x.role == "button")?.label?;
            Some((e.rect[1], e.rect[0], label))
        })
        .collect();
    drawn.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    drawn.into_iter().map(|(_, _, label)| label).collect()
}

fn press(rt: &mut rune::Runtime<State>, key: &str, start: bool) {
    let e = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(key)).unwrap_or_else(|| panic!("no {key}"));
    let [x, y, w, h] = e.rect;
    let x = if start { x + w / 4.0 } else { x + w / 2.0 };
    rt.pointer_down(x, y + h / 2.0);
    rt.pointer_up(x, y + h / 2.0);
    rt.frame(0.05);
}

#[test]
fn home_draws_four_starter_rows_each_saying_what_it_does() {
    // case: are four rows, each saying what it does
    let rt = home();
    assert_eq!(rows(&rt), ["Plan before building", "Debug an issue", "Review my changes", "Build a landing page"]);
    for starter in starters::coder_starters() {
        assert!(starter.line.chars().count() > 10, "{}", starter.id);
        assert!(starter.prompt.chars().count() > 20, "{}", starter.id);
    }
}

#[test]
fn a_starter_sets_the_mode_it_suits_and_compares_work_only_where_work_is_made() {
    // case: suit their mode: a plan changes nothing yet, and a review only reads
    for (id, mode) in [("plan", "plan"), ("review", "ask")] {
        let mut rt = home();
        press(&mut rt, &format!("starter-{id}"), true);
        assert_eq!(rt.state.composer_mode.as_deref(), Some(mode), "{id}");
    }
    let mut rt = home();
    press(&mut rt, "starter-review", true);
    assert!(rt.state.draft.text.ends_with("Change nothing."), "{}", rt.state.draft.text);
    // A fix and a page are compared as work, each model in its own copy; a plan and a review compare answers.
    let compared_as_work: Vec<String> = starters::coder_starters().into_iter().map(|st| st.id).filter(|id| {
        let mut rt = home();
        press(&mut rt, &format!("starter-compare-{id}"), false);
        rt.state.compare.changes
    }).collect();
    assert_eq!(compared_as_work, ["debug", "page"]);
}

#[test]
fn a_starter_leaves_the_words_for_the_person_to_finish() {
    // case: leave the words for the person to finish where the ask needs them
    for id in ["plan", "debug"] {
        let mut rt = home();
        press(&mut rt, &format!("starter-{id}"), true);
        assert!(rt.state.draft.text.ends_with(": "), "{id}: {:?}", rt.state.draft.text);
    }
}

#[test]
fn every_row_has_a_compare_that_fills_the_box_and_sends_nothing() {
    // case: draw a Compare beside every row, and fill the box rather than send
    let rt = home();
    for starter in starters::coder_starters() {
        let compare = rt.elements().into_iter().find(|e| e.key.as_deref() == Some(&format!("starter-compare-{}", starter.id)))
            .unwrap_or_else(|| panic!("no Compare beside {}", starter.label));
        assert_eq!((compare.role, compare.text.as_deref()), ("button", Some("Compare")));
    }
    let mut rt = home();
    press(&mut rt, "starter-compare-debug", false);
    let debug = starters::coder_starters().into_iter().find(|st| st.id == "debug").unwrap();
    assert_eq!(rt.state.draft.text, debug.prompt);
    assert_eq!(rt.state.compare.composer_mode, crate::compare_view::CompareComposerMode::Compare);
    assert!(rt.state.flight.is_none() && rt.state.compare.compares.is_empty(), "nothing is sent until Send");
    // The row itself starts it with a teammate, not a comparison.
    let mut rt = home();
    rt.update(|s, _| s.compare.composer_mode = crate::compare_view::CompareComposerMode::Compare);
    rt.frame(0.1);
    press(&mut rt, "starter-debug", true);
    assert_eq!(rt.state.compare.composer_mode, crate::compare_view::CompareComposerMode::Direct);
    assert_eq!(rt.state.draft.text, debug.prompt);
    assert!(rt.state.flight.is_none());
}

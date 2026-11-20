//! stop-card-says-tools-not-work.test.tsx (1 case): a run stopped before any tool says no tools were used, not that
//! no work was done (it may have written, thought or planned). Read from the drawn card, not the source.
use super::*;
use locust_core::cancellation::Summary;

fn words(summary: &Summary) -> Vec<String> {
    let mut rt = rune::Runtime::new(
        summary.clone(),
        |s, _| col().w_full().child(card(s, true, &Palette::new(true))),
        (600.0, 400.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|e| e.text).collect()
}

// case: says no tools were used, not that no work was done
#[test]
fn says_no_tools_were_used_not_that_no_work_was_done() {
    let drawn = words(&Summary::default());
    assert!(drawn.iter().any(|t| t == "Stopped before it used any tools, so nothing is half-done."), "{drawn:?}");
    assert!(!drawn.iter().any(|t| t.contains("did any work")), "{drawn:?}");
}

#[test]
fn a_run_that_used_a_tool_lists_it_instead() {
    let drawn = words(&Summary { settled: vec!["npm test".into()], ..Default::default() });
    assert!(!drawn.iter().any(|t| t.contains("used any tools")), "{drawn:?}");
    assert!(drawn.iter().any(|t| t == "npm test"), "{drawn:?}");
}

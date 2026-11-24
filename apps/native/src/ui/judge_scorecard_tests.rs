//! Electron's `a-judge-scorecard-sits-under-the-answers.test.tsx`, case by case with its values and words, on
//! `compare_view.rs`'s `render_scorecard`, drawn headless, and where the compare screen puts it. Electron's table cells
//! are read as Rust's rows of words: a verdict and then its why, each its own words (Electron's two spans), and the
//! best score drawn green (Electron's `is-best`).
use super::*;
use locust_core::compare::{CompareRoute, CompareSlot, CompareSlotId, blind_name};
use locust_core::compare_score::{ScoreVerdict, Scorecard};
use rune::{Env, Runtime};
use std::collections::BTreeMap;

struct Harness {
    state: State,
    build: Box<dyn Fn(&State) -> El>,
}
fn view(h: &Harness, _: &mut Cx<Harness>) -> El {
    col().w_full().h_full().child((h.build)(&h.state))
}
fn slot(id: CompareSlotId, runtime: &str, model: &str, label: &str) -> CompareSlot {
    CompareSlot {
        slot: id,
        route: CompareRoute { runtime: runtime.into(), model: model.into(), account: None, effort: None, label: Some(label.into()),
            mode: None, instructions: None, why: None, skill: None },
        mission_ids: vec![format!("m_{}", id.as_str())],
        refused: None,
        retried: vec![],
        folder: None,
    }
}
fn slots() -> Vec<CompareSlot> {
    vec![slot(CompareSlotId::A, "codex", "gpt-6-sol", "GPT-6 Sol"), slot(CompareSlotId::B, "claude", "sonnet", "Sonnet 5.5")]
}
fn verdict(met: bool, why: &str) -> Option<ScoreVerdict> {
    Some(ScoreVerdict { met, why: why.into(), votes: None })
}
fn scorecard() -> Scorecard {
    Scorecard {
        points: vec!["Exactly four lines".into(), "Each pair rhymes".into()],
        by_judge: false,
        verdicts: BTreeMap::from([
            ("a".to_owned(), vec![verdict(true, "Four lines exactly."), verdict(true, "Every pair rhymes.")]),
            ("b".to_owned(), vec![verdict(true, "Four lines."), verdict(false, "Lines 1 and 3 do not rhyme.")]),
        ]),
        weights: None,
    }
}

/// The scorecard's words in order, and the colour each was drawn in.
fn drawn(card: Scorecard, names: [&str; 2], spans: [Option<&str>; 2], costs: [Option<&str>; 2], veiled: bool) -> Vec<(String, Option<rune::Color>)> {
    let names: Vec<String> = names.iter().map(|n| n.to_string()).collect();
    let spans: Vec<Option<String>> = spans.iter().map(|s| s.map(Into::into)).collect();
    let costs: Vec<Option<String>> = costs.iter().map(|s| s.map(Into::into)).collect();
    let build = move |_: &State| {
        let p = Palette::new(true);
        render_scorecard(&card, &slots(), &names, &spans, &costs, &[None, None], veiled, &p, &crate::look::controls::Look::new(true))
    };
    let mut rt = Runtime::new(Harness { state: State::default(), build: Box::new(build) }, view, (900.0, 600.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements().into_iter().filter_map(|n| n.text.map(|t| (t, n.text_color))).collect()
}
fn words(seen: &[(String, Option<rune::Color>)]) -> Vec<&str> {
    seen.iter().map(|(t, _)| t.as_str()).collect()
}
fn follows(seen: &[&str], run: &[&str]) -> bool {
    seen.windows(run.len()).any(|w| w == run)
}
fn green(seen: &[(String, Option<rune::Color>)], word: &str) -> bool {
    let p = Palette::new(true);
    seen.iter().any(|(t, c)| t == word && *c == Some(p.green))
}

/// The compare screen of a judged comparison, as the window draws it: the judge's answer carries its scorecard.
fn judged_screen() -> Vec<(f32, String)> {
    use locust_core::compare::{Compare, CompareJudge};
    let mut s = State::default();
    for (id, said) in [
        ("m_a", "A poem."),
        ("m_b", "A poem."),
        ("m_j", "Answer A keeps the form. I would keep Answer A.\nSCORE A 1: met Four lines exactly.\nSCORE A 2: met Every pair rhymes.\nSCORE B 1: met Four lines.\nSCORE B 2: not met Lines 1 and 3 do not rhyme."),
    ] {
        let items = vec![ledger::Item::Assistant { item_id: format!("{id}_1"), text: said.into(), r#final: true }];
        s.compare.snapshots.insert(id.into(), ledger::Snapshot { tip: id.into(), items, ..Default::default() });
    }
    s.compare.compares = vec![Compare {
        compare_id: "cmp_1".into(),
        teammate_id: None,
        prompt: "Write a four-line poem about a locust swarm at dusk.".into(),
        created_at: "2026-10-12T18:00:00.000Z".into(),
        slots: slots(),
        changes: false,
        changes_in: None,
        blind: false,
        kept: None,
        judge: Some(CompareJudge {
            route: CompareRoute { runtime: "opencode".into(), model: "opencode/nemotron-3-ultra-free".into(), account: None, effort: None, label: None, mode: None, instructions: None, why: None, skill: None },
            mission_ids: vec!["m_j".into()],
            criteria: Some("Exactly four lines\nEach pair rhymes".into()),
            panel: vec![],
            panel_mission_ids: vec![],
        }),
        base: None,
        rerun_of: None,
        reruns: vec![],
        settled: false,
        point_drafts: vec![],
    }];
    s.compare.active_id = Some("cmp_1".into());
    s.compare.loaded = true;
    let mut rt = Runtime::new(s, |s: &State, _: &mut Cx<State>| super::view(s, &Palette::new(true)), (1400.0, 3000.0), Env::new(true, 1.0));
    rt.frame(0.0);
    let mut seen: Vec<(f32, String)> = rt.elements().into_iter().filter(|e| !e.exiting).filter_map(|e| e.text.map(|t| (e.rect[1], t))).collect();
    seen.sort_by(|a, b| a.0.total_cmp(&b.0));
    seen
}

#[test]
fn scores_every_answer_on_every_point_under_its_own_column_met_or_not_and_why_then_score_time_and_what_it_spent() {
    // case: scores every answer on every point under its own column, met or not and why, then score, time and what it spent
    let seen = drawn(scorecard(), ["GPT-6 Sol", "Sonnet 5.5"], [Some("16s"), Some("5s")], [Some("$0.02"), Some("in your plan")], false);
    let w = words(&seen);
    assert!(follows(&w, &["What a good answer does", "GPT-6 Sol", "Sonnet 5.5"]), "{w:?}");
    assert!(w.contains(&"Each pair rhymes"));
    // The verdict, then its why, each its own words.
    assert!(follows(&w, &["not met", "Lines 1 and 3 do not rhyme."]), "{w:?}");
    // The summary comes before the judge's words, on the compare screen itself.
    let screen = judged_screen();
    let at = |words: &str| screen.iter().find(|(_, t)| t.starts_with(words)).map(|(y, _)| *y).unwrap_or_else(|| panic!("{words} not on: {screen:?}"));
    assert!(at("What a good answer does") < at("Answer A keeps the form."), "{screen:?}");
    assert!(at("What a good answer does") < at("It read the answers as Answer A"), "{screen:?}");
    assert!(follows(&w, &["Score", "2 of 2", "1 of 2"]));
    assert!(green(&seen, "2 of 2") && !green(&seen, "1 of 2"));
    assert!(follows(&w, &["Time", "16s", "5s"]));
    assert!(follows(&w, &["Spent", "$0.02", "in your plan"]));
}

#[test]
fn in_a_blind_comparison_scores_model_a_and_model_b_and_keeps_what_they_spent_out_until_one_is_kept() {
    // case: in a blind comparison, scores Model A and Model B and keeps what they spent out until one is kept
    let (a, b) = (blind_name(CompareSlotId::A, false), blind_name(CompareSlotId::B, false));
    let seen = drawn(scorecard(), [&a, &b], [Some("16s"), Some("5s")], [Some("$0.02"), Some("in your plan")], true);
    let w = words(&seen);
    assert!(follows(&w, &["Model A", "Model B"]), "{w:?}");
    assert!(!w.contains(&"Spent"));
    assert!(!w.contains(&"in your plan"));
}

#[test]
fn leaves_out_a_row_no_answer_has_a_figure_for_rather_than_a_line_of_dashes() {
    // case: leaves out a row no answer has a figure for, rather than a line of dashes
    let seen = drawn(scorecard(), ["GPT-6 Sol", "Sonnet 5.5"], [None, None], [None, None], false);
    let w = words(&seen);
    assert!(w.contains(&"Score"));
    assert!(!w.contains(&"Time"));
    assert!(!w.contains(&"Spent"));
}

#[test]
fn names_the_judges_own_points_as_its_own_and_marks_no_winner_in_a_tie() {
    // case: names the judge's own points as its own, and marks no winner in a tie
    let own = Scorecard {
        by_judge: true,
        verdicts: BTreeMap::from([
            ("a".to_owned(), vec![verdict(true, ""), verdict(false, "")]),
            ("b".to_owned(), vec![verdict(false, ""), verdict(true, "")]),
        ]),
        ..scorecard()
    };
    let seen = drawn(own, ["GPT-6 Sol", "Sonnet 5.5"], [Some("16s"), Some("5s")], [Some("$0.02"), Some("in your plan")], false);
    let w = words(&seen);
    assert!(w.contains(&"The judge’s points"));
    assert!(w.contains(&"No points were written, so the judge set these from your request."));
    assert!(!green(&seen, "1 of 2"));
}

//! `thought-folds-to-one-line.test.ts`, drawn headless: a thought reads as one line -- "Thought for 12s" -- and says
//! nothing about time it cannot vouch for (Colin, 2026-09-17).
use super::*;

fn thought_row(duration_ms: Option<u64>) -> String {
    let item = ledger::Item::Thought {
        id: "th1".into(),
        text: "Two reads, then answer.".into(),
        duration_ms,
    };
    let s = State::default();
    let mut rt = rune::Runtime::new(
        (s, item),
        |(s, item), _| activity_item(item, &Palette::new(true), s, "conv"),
        (600.0, 200.0),
        rune::Env::new(true, 1.0),
    );
    rt.frame(0.0);
    rt.elements()
        .into_iter()
        .filter_map(|e| e.text)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn reads_as_one_line_and_says_nothing_about_time_it_cannot_vouch_for() {
    let said = thought_row(Some(12_000));
    assert!(said.contains("Thought for 12s"), "{said}");
    let said = thought_row(Some(90_000));
    assert!(said.contains("Thought for 1m 30s"), "{said}");
    let said = thought_row(None);
    assert!(said.contains("Thought") && !said.contains("Thought for"), "{said}");
    let said = thought_row(Some(200));
    assert!(said.contains("Thought") && !said.contains("Thought for"), "{said}");
}

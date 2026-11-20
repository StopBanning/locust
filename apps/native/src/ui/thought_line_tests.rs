//! a-thought-line-opens-onto-its-words.test.tsx (2 cases): a step line's thought opens straight onto its words, with
//! no "Thought for 7s" and the headline said once (on the line), alone or beside other steps (0.515).

use super::*;

const THOUGHT: &str = "**Considering fallback options**\n\nMaybe the accessibility tree can be read instead.";

fn opened(items: Vec<ledger::Item>) -> (String, Vec<rune::ElementInfo>) {
    let mut s = State::default();
    s.activity_open.insert(format!("conv/group/{}", activity_id(&items[0])), true);
    let mut rt = rune::Runtime::new((s, items), |(s, items), _| activity_group(items, &Palette::new(true), s, "conv"), (700.0, 500.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    let els = rt.elements();
    (els.iter().filter_map(|e| e.text.clone()).collect::<Vec<_>>().join("\n"), els)
}

fn thought(ms: u64) -> ledger::Item {
    ledger::Item::Thought { id: "th1".into(), text: THOUGHT.into(), duration_ms: Some(ms) }
}

fn grep_search() -> ledger::Item {
    ledger::Item::Tool {
        name: "grep_search".into(),
        summary: String::new(),
        detail: Box::new(ledger::ToolDetail { id: "t1".into(), exit_code: Some(0), finished: true, ..Default::default() }),
        status: ledger::Status::Done,
    }
}

// case: opens straight onto its words, without saying the headline again
#[test]
fn a_lone_thought_opens_straight_onto_its_words() {
    let (said, _) = opened(vec![thought(7_000)]);
    assert!(said.contains("Maybe the accessibility tree can be read instead."), "{said}");
    assert!(!said.contains("Thought for 7s"), "{said}");
    assert_eq!(said.matches("Considering fallback options").count(), 1, "{said}");
}

// case: and a thought beside other steps reads the same way (0.515)
#[test]
fn a_thought_beside_other_steps_reads_the_same_way() {
    let (said, els) = opened(vec![thought(3_000), grep_search()]);
    assert!(said.contains("Maybe the accessibility tree can be read instead."), "{said}");
    assert!(!said.contains("Thought for 3s"), "{said}");
    assert!(els.iter().any(|e| e.key.as_deref().is_some_and(|k| k.contains("/steps-thought/"))), "drawn as the step's thought, not a row");
    assert_eq!(said.matches("Considering fallback options").count(), 1, "{said}");
}

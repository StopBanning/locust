//! a-checklist-left-unticked-is-not-a-fault.test.tsx (3 cases): after a run that completed, the plan card says what
//! was left unticked in the header's own voice; amber, and the stopped sentence, only for a run that failed or was
//! stopped; nothing while the run goes on.

use super::*;

const STEPS: [(&str, &str); 2] = [("Ask five questions about the business", "running"), ("Write a one-page introduction", "pending")];

fn card(finished: bool, stopped: bool) -> Vec<rune::ElementInfo> {
    let steps = STEPS.iter().map(|(text, state)| ledger::PlanStep { text: (*text).into(), state: (*state).into() }).collect();
    let item = ledger::Item::Plan { id: "plan".into(), steps, finished, stopped };
    let mut rt = rune::Runtime::new((State::default(), item), |(s, item), _| activity_item(item, &Palette::new(true), s, "conv"), (700.0, 400.0), rune::Env::new(true, 1.0));
    rt.frame(0.0);
    rt.elements()
}

fn count(els: &[rune::ElementInfo]) -> Option<&rune::ElementInfo> {
    els.iter().find(|e| e.text.as_deref() == Some("2 not checked off"))
}

fn has_key(els: &[rune::ElementInfo], part: &str) -> bool {
    els.iter().any(|e| e.key.as_deref().is_some_and(|k| k.contains(part)))
}

// case: says what was left unticked, in its own voice, after a run that completed
#[test]
fn after_a_completed_run_the_count_is_in_the_headers_own_voice() {
    let els = card(true, false);
    let l = crate::look::controls::Look::new(true);
    assert_eq!(count(&els).and_then(|e| e.text_color), Some(l.c("text-muted")), "muted, not amber");
    assert!(has_key(&els, "plan/plan-unreached") && !has_key(&els, "is-stopped"));
    assert!(els.iter().any(|e| e.label.as_deref() == Some(UNTICKED_ENDED)));
}

// case: keeps amber for a run that failed or was stopped
#[test]
fn keeps_amber_for_a_run_that_failed_or_was_stopped() {
    let els = card(true, true);
    let l = crate::look::controls::Look::new(true);
    assert_eq!(count(&els).and_then(|e| e.text_color), Some(l.c("amber-text")));
    assert!(has_key(&els, "plan-unreached/is-stopped"));
    assert!(els.iter().any(|e| e.label.as_deref() == Some("The run stopped before checking these off its list: see what it ran below.")));
}

// case: says nothing of the kind while the run goes on
#[test]
fn says_nothing_of_the_kind_while_the_run_goes_on() {
    assert!(count(&card(false, false)).is_none());
}

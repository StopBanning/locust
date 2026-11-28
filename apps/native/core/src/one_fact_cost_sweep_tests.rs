//! The one-fact-one-surface Electron cases for the inspector and compact cost cells.
use crate::benchmark::{RunCost, cost_cell};
use crate::inspector::cost_label_and_line;

// case: says the same thing wherever a cost was not reported
#[test]
fn one_fact_a_finished_run_without_usage_names_the_missing_fact() {
    assert_eq!(cost_label_and_line(&[], false).1, "not reported by the runtime");
}

// case: keeps the one distinction that is real
#[test]
fn one_fact_an_unfinished_run_is_not_treated_as_unreported() {
    assert_eq!(cost_label_and_line(&[], true).1, "reported when the run ends");
    assert_eq!(cost_label_and_line(&[], false).1, "not reported by the runtime");
}

// case: the short form claims nothing, rather than claiming zero
#[test]
fn one_fact_unknown_cost_does_not_claim_zero_dollars() {
    let drawn = cost_cell(None, "account-default");
    assert!(!drawn.contains('0') && !drawn.contains('$'), "{drawn}");
}

// case: the short form is words, not a bare dash
#[test]
fn one_fact_the_short_cost_cell_uses_words_and_keeps_reported_dollars() {
    assert_eq!(cost_cell(None, "account-default"), "not reported");
    let reported = RunCost { usd: Some(0.42), ..RunCost::default() };
    assert_eq!(cost_cell(Some(&reported), "account-default"), "$0.42");
}

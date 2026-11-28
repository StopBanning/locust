//! one-fact-one-surface.test.ts, "one missing cost, one sentence": the cases whose behaviour is built. Where Rust words a
//! thing differently from Electron, the ledger says so (a missing cost in a benchmark cell is an em dash here, "not
//! reported" there; the inspector's dollars are `$0.4200` here, `$0.42` there).

use crate::benchmark::{RunCost, cost_cell};
use crate::inspector::cost_label_and_line;

fn usd(amount: f64) -> RunCost {
    RunCost { usd: Some(amount), ..Default::default() }
}

// "says the same thing wherever a cost was not reported".
#[test]
fn a_cost_the_runtime_did_not_report_is_said_to_be_not_reported_by_the_runtime() {
    assert_eq!(cost_label_and_line(&[], false), ("Usage", "not reported by the runtime".to_owned()));
}

// "keeps the one distinction that is real".
#[test]
fn a_run_that_has_not_finished_has_not_reported_a_cost_yet() {
    assert_eq!(cost_label_and_line(&[], true), ("Usage", "reported when the run ends".to_owned()));
}

// case: a model whose own id says it is free reads free
#[test]
fn a_model_whose_own_id_says_it_is_free_reads_free_and_a_reported_number_still_wins() {
    assert_eq!(cost_cell(None, "opencode/ling-3.0-flash-fin-free"), "free");
    assert_eq!(cost_cell(Some(&usd(0.01)), "opencode/ling-3.0-flash-fin-free"), "$0.01");
}

// "the short form is words..." -- the dollars part: a reported cost reads as dollars in a benchmark's cell.
#[test]
fn a_reported_cost_reads_as_dollars_in_a_benchmark_cell() {
    assert_eq!(cost_cell(Some(&usd(0.42)), "account-default"), "$0.42");
}

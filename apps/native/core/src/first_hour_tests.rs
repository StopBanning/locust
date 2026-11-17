//! `a-first-hour-starts-plainly.test.ts`: the two cases that are rules rather than Electron's own source text.

use crate::connectors_off::connector_label;
use crate::route_at_start::{PickerRoute, StartAs, as_reviewer};

#[test]
fn a_connector_is_named_for_a_person() {
    assert_eq!(connector_label("plugin:small-business:zoho-projects"), "Zoho Projects");
    assert_eq!(connector_label("plugin:small-business:airwallex-agentos"), "Airwallex Agentos");
    assert_eq!(connector_label("claude.ai Gmail"), "Gmail");
    assert_eq!(connector_label("robinhood"), "robinhood");
}

fn edit(runtime: &str, model: &str, mode: &str) -> StartAs {
    StartAs { teammate_id: "tm_rev".into(), route: PickerRoute { runtime: runtime.into(), model: model.into(), account: None }, mode: mode.into(), effort: None, one_off: false }
}

#[test]
fn a_review_reads_ask_for_that_run_only() {
    let reviewing = as_reviewer(edit("opencode", "opencode/free", "accept-edits"), false);
    assert_eq!(reviewing, StartAs { mode: "ask".into(), one_off: true, ..edit("opencode", "opencode/free", "accept-edits") });
    assert_eq!(as_reviewer(edit("opencode", "opencode/free", "ask"), false), edit("opencode", "opencode/free", "ask"));
    assert_eq!(as_reviewer(edit("antigravity", "x", "accept-edits"), false), edit("antigravity", "x", "accept-edits"));
    assert_eq!(as_reviewer(edit("antigravity", "x", "accept-edits"), true), StartAs { mode: "ask".into(), one_off: true, ..edit("antigravity", "x", "accept-edits") });
}

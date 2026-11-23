//! Electron's `renderer/src/cost.test.ts`, the receipt half: what a run cost off its `run.completed` (cost.ts
//! `runCostOf`, here `relay::mission_cost_from_events`), the one-line cost (`benchmark::cost_line`), adding costs up
//! (`relay::sum_costs`) and how full the context is (`context_usage::Reading`). The conversation half (what it has
//! cost so far, and the newest context reading) is read through a real conversation in `conversation_cost_tests`.
//!
//! A receipt's cache counts are not carried on `RunCost`: Electron keeps `cacheReadTokens` and `cacheWriteTokens` on
//! its cost and draws them nowhere (no line, no sum reads them), and the usage page reads its own
//! (`usage_ledger::UsageTurn`).
use super::*;
use crate::benchmark::cost_line;
use crate::context_usage::Reading;
use serde_json::json;

fn completed(usage: Option<Value>) -> Value {
    let mut payload = json!({ "evidence": { "redacted": false }, "process": {} });
    if let Some(usage) = usage {
        payload["usage"] = usage;
    }
    json!({ "id": "evt_1", "runId": "run_1", "missionId": "mission_1", "sequence": 1,
        "occurredAt": "2026-09-03T10:00:00.000Z", "sourceAdapter": "codex", "type": "run.completed", "payload": payload })
}

fn cost_of(usage: Value) -> Option<RunCost> {
    mission_cost_from_events(&[completed(Some(usage))])
}

fn tokens(input: u64, output: u64) -> RunCost {
    RunCost { input_tokens: Some(input), output_tokens: Some(output), ..Default::default() }
}

// case: reads token counts in either spelling
#[test]
fn reads_token_counts_in_either_spelling() {
    assert_eq!(cost_of(json!({ "inputTokens": 19428, "outputTokens": 161, "cacheReadTokens": 16256 })), Some(tokens(19428, 161)));
    assert_eq!(cost_of(json!({ "input_tokens": 10, "output_tokens": 47 })), Some(tokens(10, 47)));
    // The receipt's own spelling first, when it has both.
    assert_eq!(cost_of(json!({ "inputTokens": 3, "input_tokens": 10, "outputTokens": 4 })), Some(tokens(3, 4)));
    // A count the receipt left empty is read in the other spelling, as `??` does.
    assert_eq!(cost_of(json!({ "inputTokens": null, "input_tokens": 10, "outputTokens": 4 })), Some(tokens(10, 4)));
}

// case: reads Claude Code's dollars and Copilot's premium requests as their own units
#[test]
fn reads_claude_codes_dollars_and_copilots_premium_requests_as_their_own_units() {
    assert_eq!(
        cost_of(json!({ "usd": 0.0297808, "inputTokens": 10, "outputTokens": 47 })),
        Some(RunCost { usd: Some(0.0297808), ..tokens(10, 47) })
    );
    assert_eq!(cost_of(json!({ "premiumRequests": 1, "nanoAiu": 383710000 })), Some(RunCost { premium_requests: Some(1.0), ..Default::default() }));
}

// case: says nothing when the receipt carries nothing, rather than zero
#[test]
fn says_nothing_when_the_receipt_carries_nothing_rather_than_zero() {
    assert_eq!(mission_cost_from_events(&[completed(None)]), None);
    assert_eq!(cost_of(json!({ "nanoAiu": 5 })), None);
    assert_eq!(mission_cost_from_events(&[]), None);
    assert_eq!(cost_line(None), None);
}

// case: never reads a negative or non-numeric count
#[test]
fn never_reads_a_negative_or_non_numeric_count() {
    assert_eq!(cost_of(json!({ "inputTokens": -5, "outputTokens": "lots" })), None);
    assert_eq!(cost_of(json!({ "input_tokens": -5, "output_tokens": "lots" })), None);
}

// case: prefers dollars, then premium requests, then tokens
#[test]
fn prefers_dollars_then_premium_requests_then_tokens() {
    let line = |c: RunCost| cost_line(Some(&c));
    assert_eq!(line(RunCost { usd: Some(0.0297808), ..tokens(10, 47) }).as_deref(), Some("$0.03"));
    assert_eq!(line(RunCost { usd: Some(0.001), ..Default::default() }).as_deref(), Some("< $0.01"));
    assert_eq!(line(RunCost { usd: Some(0.0), ..Default::default() }).as_deref(), Some("$0.00"));
    assert_eq!(line(RunCost { premium_requests: Some(1.0), ..Default::default() }).as_deref(), Some("1 premium request"));
    assert_eq!(line(RunCost { premium_requests: Some(3.0), ..Default::default() }).as_deref(), Some("3 premium requests"));
    assert_eq!(line(tokens(19428, 161)).as_deref(), Some("19k in · 161 out"));
    assert_eq!(line(tokens(1_250_000, 999)).as_deref(), Some("1.3M in · 999 out"));
}

// case: sums each unit it saw and ignores runs that reported nothing
#[test]
fn sums_each_unit_it_saw_and_ignores_runs_that_reported_nothing() {
    let total = sum_costs([
        Some(RunCost { usd: Some(0.02), ..Default::default() }),
        None,
        Some(RunCost { usd: Some(0.03), input_tokens: Some(5), ..Default::default() }),
        Some(RunCost { premium_requests: Some(2.0), ..Default::default() }),
    ])
    .expect("a total");
    assert!((total.usd.unwrap() - 0.05).abs() < 1e-12, "{total:?}");
    assert_eq!((total.input_tokens, total.output_tokens, total.premium_requests), (Some(5), None, Some(2.0)));
    assert_eq!(sum_costs([None, None]), None);
}

// case: counts what the conversation held after its last call against the reported window
#[test]
fn counts_what_the_conversation_held_after_its_last_call_against_the_reported_window() {
    let reading = Reading::from_usage(&json!({ "inputTokens": 2, "outputTokens": 5, "cacheReadTokens": 23997,
        "cacheWriteTokens": 15080, "contextTokens": 39084, "contextWindow": 1_000_000 }));
    assert_eq!(reading, Some(Reading { used: 39084, window: 1_000_000, percent: 4 }));
}

// case: says nothing at all when the runtime did not report a window
#[test]
fn says_nothing_at_all_when_the_runtime_did_not_report_a_window() {
    assert_eq!(Reading::from_usage(&json!({ "inputTokens": 40_000, "outputTokens": 100 })), None);
    assert_eq!(Reading::from_usage(&Value::Null), None);
    assert_eq!(Reading::from_usage(&json!({ "contextWindow": 200_000 })), None);
}

// case: says it in the words the tooltip uses
#[test]
fn says_it_in_the_words_the_tooltip_uses() {
    let reading = Reading::from_usage(&json!({ "contextTokens": 39_079, "contextWindow": 1_000_000 })).unwrap();
    let said = reading.sentence();
    assert!(said.starts_with("Context: ") && said.contains(" of ") && said.ends_with(" used, 4%"), "{said}");
    assert_eq!(said, "Context: 39k of 1.0M used, 4%");
}

// Carries apps/desktop/src/shared/spend.test.ts (Electron 4ce2f37a), case by case. What a run cost is read the way the
// monthly limit reads it (native.rs spend_gate): a finished run's ledger through `usage_ledger::usage_turns`, a
// teammate's month through `spent_by`, then `limit_reached` and `limit_refusal`. A run's money is a UsageTurn's
// dollars and premium requests, with the moment it ended.
use super::*;
use chrono::TimeZone;
use serde_json::{Value, json};

struct Temp(std::path::PathBuf);
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const ENDED: &str = "2026-09-26T06:00:00.000Z";

/// One finished OpenCode run per usage (None: a run that never reached run.completed), as the ledger keeps it.
fn ledgers(usages: &[Option<Value>]) -> (Temp, crate::store::Store) {
    let root = std::env::temp_dir().join(format!("locust-spend-case-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let dir = store.ledger_dir().unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    for (i, usage) in usages.iter().enumerate() {
        let id = format!("mission_{i}");
        let created = json!({"schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": ENDED, "metadata": {"missionId": id, "runId": format!("run_{i}"), "prompt": "Go", "runtime": "opencode", "model": "opencode/some-paid-model", "requestedRouteId": "opencode", "resolvedRouteId": "opencode-account:default", "cliVersion": "test", "workspaceId": "ws_test", "sandbox": "read-only", "executionPolicyVersion": 1, "createdAt": ENDED}});
        let mut body = format!("{created}\n");
        if let Some(usage) = usage {
            let done = json!({"schemaVersion": 27, "recordType": "mission.event", "ledgerSequence": 2, "occurredAt": ENDED, "event": {"id": format!("e_{i}"), "runId": format!("run_{i}"), "missionId": id, "sequence": 1, "type": "run.completed", "occurredAt": ENDED, "sourceAdapter": "opencode", "payload": {"usage": usage}}});
            body.push_str(&format!("{done}\n"));
        }
        std::fs::write(dir.join(format!("{id}.jsonl")), body).unwrap();
    }
    (Temp(root), store)
}

/// What one run cost: (dollars, premium requests, the moment it ended), or None when nothing was money.
fn money_of_one(usage: Option<Value>) -> Option<(Option<f64>, Option<f64>, String)> {
    let (_t, store) = ledgers(&[usage]);
    let turns = crate::usage_ledger::usage_turns(&store).unwrap();
    let turn = turns.first()?;
    (turn.usd.is_some() || turn.premium_requests.is_some()).then(|| (turn.usd, turn.premium_requests, turn.at.clone()))
}

// ---- what a run cost in money ----

// case: is the dollars a runtime priced, with the moment it ended
#[test]
fn a_runs_money_is_the_dollars_its_runtime_priced_when_it_ended() {
    assert_eq!(money_of_one(Some(json!({"usd": 0.42, "inputTokens": 900}))), Some((Some(0.42), None, ENDED.to_owned())));
}

// case: is nothing for a plan's run, whose dollar figure nobody paid
#[test]
fn a_plans_run_costs_no_money() {
    assert_eq!(money_of_one(Some(json!({"usd": 3, "billing": "subscription"}))), None);
}

// case: is nothing for tokens alone, or a free model that priced nothing
#[test]
fn tokens_alone_or_a_run_that_never_finished_cost_no_money() {
    assert_eq!(money_of_one(Some(json!({"inputTokens": 900, "outputTokens": 40}))), None);
    assert_eq!(money_of_one(None), None);
}

// case: is Copilot's premium requests, as their own unit
#[test]
fn copilots_premium_requests_are_money_in_their_own_unit() {
    assert_eq!(money_of_one(Some(json!({"premiumRequests": 2}))), Some((None, Some(2.0), ENDED.to_owned())));
}

// case: adds up by unit, and is nothing when nothing was money
#[test]
fn spending_adds_up_unit_by_unit_and_is_nothing_without_money() {
    let usd = |n| Spend { usd: Some(n), ..Default::default() };
    let both = Spend { usd: Some(0.5), premium_requests: Some(1.0), ..Default::default() };
    assert_eq!(sum_spend(&[usd(1.0), Spend::default(), both]), Some(Spend { usd: Some(1.5), premium_requests: Some(1.0), unpriced_runs: None }));
    assert_eq!(sum_spend(&[Spend::default()]), None);
}

// ---- the month a run counts in ----

// case: is the machine's own month, not UTC's
#[test]
fn a_run_counts_in_the_machines_own_month() {
    // Local noon on the 30th is the 30th everywhere a person lives.
    let noon = chrono::Local.with_ymd_and_hms(2026, 9, 30, 12, 0, 0).unwrap();
    assert_eq!(month_of(&noon.to_rfc3339()).as_deref(), Some("2026-09"));
    let just_after = chrono::Local.with_ymd_and_hms(2026, 10, 1, 0, 5, 0).unwrap();
    assert_eq!(month_of(&just_after.with_timezone(&chrono::Utc).to_rfc3339()).as_deref(), Some("2026-10"));
    assert_eq!(month_of("not a date"), None);
}

// case: starts again on the first of the next month, said as a person says it
#[test]
fn the_month_starts_again_on_the_first_said_in_words() {
    assert_eq!(next_month_starts(chrono::Local.with_ymd_and_hms(2026, 9, 26, 0, 0, 0).unwrap()), "October 1");
    assert_eq!(next_month_starts(chrono::Local.with_ymd_and_hms(2026, 12, 31, 0, 0, 0).unwrap()), "January 1");
}

// ---- a monthly limit ----

// case: is a positive amount no larger than the cap
#[test]
fn a_monthly_limit_is_positive_and_within_the_cap() {
    assert!(is_monthly_limit(5.0));
    assert!(is_monthly_limit(0.01));
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY, 100_001.0] {
        assert!(!is_monthly_limit(bad), "{bad}");
    }
    // Read from the roster as native.rs reads it: text or nothing is no limit.
    for bad in [json!("5"), Value::Null] {
        assert_eq!(bad.as_f64().filter(|l| is_monthly_limit(*l)), None, "{bad}");
    }
}

/// A teammate's spending this month, as spend_gate adds it up from the ledgers.
fn spent_in(usages: &[Option<Value>]) -> Spend {
    let (_t, store) = ledgers(usages);
    let turns = crate::usage_ledger::usage_turns(&store).unwrap();
    spent_by(&turns, month_of(ENDED).as_deref().unwrap(), |_| true)
}

// case: is reached AT the amount as well as past it, and only by dollars
#[test]
fn a_monthly_limit_is_reached_at_its_amount_and_only_by_dollars() {
    assert!(limit_reached(Some(&spent_in(&[Some(json!({"usd": 5}))])), 5.0));
    assert!(limit_reached(Some(&spent_in(&[Some(json!({"usd": 5.01}))])), 5.0));
    assert!(!limit_reached(Some(&spent_in(&[Some(json!({"usd": 4.99}))])), 5.0));
    assert!(!limit_reached(Some(&spent_in(&[Some(json!({"premiumRequests": 500}))])), 5.0));
    assert!(!limit_reached(None, 5.0));
}

// case: says what was spent, against what, and the two ways on
#[test]
fn a_reached_limit_says_spent_against_limit_and_both_ways_on() {
    let spent = Spend { usd: Some(5.02), ..Default::default() };
    assert_eq!(
        limit_refusal("Wren", Some(&spent), 5.0, chrono::Local.with_ymd_and_hms(2026, 9, 26, 0, 0, 0).unwrap()),
        "Wren has reached this month's limit: $5.02 of $5.00. Raise the limit by editing Wren, or it starts again on October 1."
    );
    assert_eq!(dollars(0.004), "< $0.01");
    assert_eq!(dollars(0.0), "$0.00");
}

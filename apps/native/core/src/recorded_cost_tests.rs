//! Electron's own answers, replayed: `parity/recorded/cost.jsonl` holds what Electron's cost.ts `costLine` and
//! `moneyLine` returned for a grid of costs (each field present or not; cents, thousands and millions at their edges).
//! `bash scripts/electron-record/record.sh cost` records it again.
use crate::benchmark::{RunCost, cost_line, money_line};
use serde_json::{Value, json};

const RECORDED: &str = include_str!("../../parity/recorded/cost.jsonl");

fn cost_of(v: &Value) -> Option<RunCost> {
    let o = v.as_object()?;
    Some(RunCost {
        usd: o.get("usd").and_then(Value::as_f64),
        premium_requests: o.get("premiumRequests").and_then(Value::as_f64),
        input_tokens: o.get("inputTokens").and_then(Value::as_u64),
        output_tokens: o.get("outputTokens").and_then(Value::as_u64),
        plan: o.get("plan") == Some(&Value::Bool(true)),
    })
}

#[test]
fn every_recorded_electron_cost_line_is_the_rust_one() {
    let mut mismatches = Vec::new();
    let mut count = 0;
    for line in RECORDED.lines().filter(|l| !l.trim().is_empty()) {
        let case: Value = serde_json::from_str(line).expect("a readable recorded case");
        count += 1;
        let cost = cost_of(&case["args"][0]);
        let rust = match case["fn"].as_str().unwrap() {
            "costLine" => json!(cost_line(cost.as_ref())),
            "moneyLine" => json!(money_line(cost.as_ref())),
            other => panic!("no Rust function for {other}"),
        };
        if rust != case["out"] {
            mismatches.push(format!("{} {}: Electron {} | Rust {rust}", case["fn"], case["args"], case["out"]));
        }
    }
    assert!(count > 1000, "the recording holds {count} cases");
    assert!(mismatches.is_empty(), "{} of {count} differ:\n{}", mismatches.len(), mismatches.join("\n"));
}

#[test]
fn rounds_an_exact_half_cent_up_as_javascripts_to_fixed_does() {
    use crate::benchmark::to_fixed;
    assert_eq!(to_fixed(0.125, 2), "0.13");
    assert_eq!(to_fixed(1.005, 2), "1.00");
    assert_eq!(to_fixed(2.675, 2), "2.67");
    assert_eq!(to_fixed(0.995, 2), "0.99");
    assert_eq!(to_fixed(1234.5, 2), "1234.50");
    assert_eq!(to_fixed(0.015, 2), "0.01");
    assert_eq!(to_fixed(9.995, 2), "9.99");
    assert_eq!(to_fixed(0.5, 0), "1");
}

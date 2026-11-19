//! Electron's own answers, replayed: `parity/recorded/routes.jsonl` holds what Electron's `routeModelName`
//! (routeName.ts) returned for every id-like string in the route-name test files, with every runtime, plus traps.
//! Module state was empty when it was recorded, as the Rust function is stateless.
//! `bash scripts/electron-record/record.sh routes` records it again.
use crate::teammate_tooltip::route_model_name;
use serde_json::{Value, json};

const RECORDED: &str = include_str!("../../parity/recorded/routes.jsonl");

#[test]
fn every_recorded_electron_route_name_is_the_rust_route_name() {
    let mut mismatches = Vec::new();
    let mut count = 0;
    for line in RECORDED.lines().filter(|l| !l.trim().is_empty()) {
        let case: Value = serde_json::from_str(line).expect("a readable recorded case");
        count += 1;
        let (runtime, id) = (case["args"][0].as_str().unwrap(), case["args"][1].as_str().unwrap());
        let rust = json!(route_model_name(runtime, id));
        if rust != case["out"] {
            mismatches.push(format!("{runtime} {id:?}: Electron {} | Rust {rust}", case["out"]));
        }
    }
    assert!(count > 1000, "only {count} cases recorded");
    assert!(mismatches.is_empty(), "{} of {count} recorded route names differ:\n{}", mismatches.len(), mismatches.join("\n"));
}

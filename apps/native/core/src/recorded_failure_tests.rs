//! Electron's own answers, replayed: `parity/recorded/failure.jsonl` holds what Electron says when a run fails --
//! missionView.ts `failureMessage` and `sameSentence`, runtime-adapters `providerErrorSentence` -- for the strings in
//! their own test files, as messages and as stderr, under each runtime.
//! `bash scripts/electron-record/record.sh failure` records it again.
use crate::failure_words::{failure_message, same_sentence};
use crate::provider_error::provider_error_sentence;
use serde_json::{Value, json};

const RECORDED: &str = include_str!("../../parity/recorded/failure.jsonl");

#[test]
fn every_recorded_electron_failure_sentence_is_the_rust_one() {
    let mut mismatches = Vec::new();
    let mut count = 0;
    for line in RECORDED.lines().filter(|l| !l.trim().is_empty()) {
        let case: Value = serde_json::from_str(line).expect("a readable recorded case");
        count += 1;
        let arg = |i: usize| case["args"][i].as_str();
        let rust = match case["fn"].as_str().unwrap() {
            "providerErrorSentence" => json!(provider_error_sentence(arg(0).unwrap(), arg(1))),
            "failureMessage" => json!(failure_message(arg(0).unwrap(), arg(1), arg(2))),
            "sameSentence" => json!(same_sentence(arg(0).unwrap(), arg(1).unwrap())),
            other => panic!("no Rust function for {other}"),
        };
        if rust != case["out"] {
            mismatches.push(format!("{} {}: Electron {} | Rust {rust}", case["fn"], case["args"], case["out"]));
        }
    }
    assert!(count > 2000, "the recording holds {count} cases");
    assert!(mismatches.is_empty(), "{} of {count} differ:\n{}", mismatches.len(), mismatches.join("\n"));
}

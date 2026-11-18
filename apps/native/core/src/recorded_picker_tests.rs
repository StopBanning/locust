//! Electron's own answers, replayed: `parity/recorded/picker.jsonl` holds what Electron's pure picker functions
//! returned (effortLevels.ts, effortScale.ts, routeName.ts freeTagOf, status.ts defaultEffort) on every string and list
//! in their own test files plus generated traps (emoji and other text JavaScript counts in UTF-16 units, unknown words,
//! odd suffixes). `bash scripts/electron-record/record.sh picker` records it again from Electron's source.
use crate::effort_levels::{
    default_effort, effort_description, effort_footer, effort_name, effort_rank, effort_scale, join_effort, levels_line,
    split_effort,
};
use crate::route_rows::free_tag_of;
use serde_json::{Value, json};

const RECORDED: &str = include_str!("../../parity/recorded/picker.jsonl");

fn strings(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_owned()).collect()
}

fn rust_answer(fn_name: &str, args: &[Value]) -> Value {
    let s = |i: usize| args[i].as_str().unwrap();
    match fn_name {
        "effortName" => json!(effort_name(s(0))),
        "effortDescription" => json!(effort_description(s(0))),
        "effortFooter" => json!(effort_footer(s(0))),
        "splitEffort" => {
            let (base, fast) = split_effort(s(0));
            json!({ "base": base, "fast": fast })
        }
        "effortRank" => json!(effort_rank(s(0))),
        "freeTagOf" => {
            let (name, free) = free_tag_of(s(0));
            json!({ "name": name, "free": free })
        }
        "effortScale" => {
            let scale = effort_scale(&strings(&args[0]));
            json!({ "bases": scale.bases, "hasFast": scale.has_fast })
        }
        "levelsLine" => json!(levels_line(&strings(&args[0]))),
        "defaultEffort" => json!(default_effort(&strings(&args[0]), args[1].as_str())),
        "joinEffort" => json!(join_effort(s(0), args[1].as_bool().unwrap(), &strings(&args[2]))),
        other => panic!("no Rust counterpart named for {other}"),
    }
}

#[test]
fn every_recorded_electron_answer_from_the_picker_functions_is_the_rust_answer() {
    let mut mismatches = Vec::new();
    let mut unreadable = Vec::new();
    let mut count = 0;
    for line in RECORDED.lines().filter(|l| !l.trim().is_empty()) {
        let Ok(case) = serde_json::from_str::<Value>(line) else {
            // JavaScript cut a character in half (a lone UTF-16 surrogate), which no Rust string can hold.
            unreadable.push(line.to_owned());
            continue;
        };
        count += 1;
        let args = case["args"].as_array().unwrap();
        let rust = rust_answer(case["fn"].as_str().unwrap(), args);
        if rust != case["out"] {
            mismatches.push(format!("{}({}): Electron {} | Rust {}", case["fn"].as_str().unwrap(), Value::Array(args.clone()), case["out"], rust));
        }
    }
    assert!(count > 1000, "only {count} cases recorded");
    assert!(
        mismatches.is_empty() && unreadable.is_empty(),
        "{} of {count} recorded cases differ, {} unreadable:\n{}\n{}",
        mismatches.len(),
        unreadable.len(),
        mismatches.iter().take(40).cloned().collect::<Vec<_>>().join("\n"),
        unreadable.iter().take(10).cloned().collect::<Vec<_>>().join("\n")
    );
}

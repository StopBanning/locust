//! Electron's own answers, replayed: `parity/recorded/words.jsonl` holds what Electron's missionView.ts
//! `shellCommandText`, `withoutShellWrapper`, `relativePath` and `mcpToolParts` returned for every string in their
//! own test files, plus traps (quoting, Windows hosts, slashes, case, emoji).
//! `bash scripts/electron-record/record.sh words` records it again.
use crate::mcp_names::mcp_tool_parts;
use crate::path_words::relative_path;
use crate::shell_words::{shell_command_text, without_shell_wrapper};
use serde_json::{Value, json};

const RECORDED: &str = include_str!("../../parity/recorded/words.jsonl");

#[test]
fn every_recorded_electron_command_and_path_word_is_the_rust_one() {
    let mut mismatches = Vec::new();
    let mut count = 0;
    for line in RECORDED.lines().filter(|l| !l.trim().is_empty()) {
        let case: Value = serde_json::from_str(line).expect("a readable recorded case");
        count += 1;
        let arg = |i: usize| case["args"][i].as_str();
        let rust = match case["fn"].as_str().unwrap() {
            "shellCommandText" => json!(shell_command_text(arg(0).unwrap())),
            "withoutShellWrapper" => json!(without_shell_wrapper(arg(0).unwrap())),
            "relativePath" => json!(relative_path(arg(0).unwrap(), arg(1))),
            "mcpToolParts" => match mcp_tool_parts(arg(0).unwrap()) {
                Some(parts) => json!({ "server": parts.server, "tool": parts.tool }),
                None => Value::Null,
            },
            other => panic!("no Rust function for {other}"),
        };
        if rust != case["out"] {
            mismatches.push(format!("{} {}: Electron {} | Rust {rust}", case["fn"], case["args"], case["out"]));
        }
    }
    assert!(count > 5000, "the recording holds {count} cases");
    assert!(mismatches.is_empty(), "{} of {count} differ:\n{}", mismatches.len(), mismatches.join("\n"));
}

// Carries packages/runtime-adapters/test/a-denied-read-and-a-file-row-are-said-as-they-are.test.ts (Electron 4ce2f37a).
use super::{agy_events::denied_sentence, events::Normalizer};
use serde_json::{Value, json};

fn said(actions: &[&str]) -> String {
    denied_sentence(&actions.iter().map(|a| (*a).to_owned()).collect::<Vec<_>>()).expect("a sentence")
}

#[test]
fn antigravitys_denied_read_names_a_read_outside_the_folder_not_a_change() {
    let words = said(&["read_file"]);
    assert!(words.contains("not allowed to read a file outside"), "{words}");
    assert!(!words.contains("change files"), "{words}");
}

#[test]
fn antigravity_still_says_a_denied_write_and_command_as_before() {
    assert!(said(&["write_file"]).contains("not allowed to change files"));
    assert!(said(&["command"]).contains("not allowed to run a command"));
    assert!(said(&["read_file", "command"]).contains("(read_file, command)"));
}

fn file_row(changes: Value) -> Value {
    let mut codex = Normalizer::default();
    let events = codex.accept("item/started", &json!({ "item": { "type": "fileChange", "id": "fc_1", "changes": changes } }));
    events.into_iter().find(|e| e.kind == "tool.started").expect("a tool.started").payload["command"].clone()
}

#[test]
fn a_codex_file_row_lists_each_path_one_a_line() {
    assert_eq!(file_row(json!([{ "path": "C:/w/a.ts" }, { "path": "C:/w/b.ts" }])), "C:/w/a.ts\nC:/w/b.ts");
}

#[test]
fn a_codex_file_row_counts_files_when_the_paths_are_not_given() {
    assert_eq!(file_row(json!([1])), "1 file");
    assert_eq!(file_row(json!([1, 2, 3])), "3 files");
}

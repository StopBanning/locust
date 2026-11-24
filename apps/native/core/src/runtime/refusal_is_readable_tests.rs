//! Electron's `packages/runtime-adapters/test/refusal-is-readable.test.ts`, case by case. A refusal is read where the
//! app reads it: a Claude Code `result` record carrying `permission_denials`, handed to `ClaudeNormalizer::accept`
//! (runtime/claude.rs), whose diagnostic is the amber row. Electron's helpers (`brieflyPut`, `namedTool`, `whyRefused`)
//! are checked beside the sentence the caller builds from them.
use super::*;

const NEWLINE: char = '\n';

fn long() -> String {
    [
        "cd \"C:/Users/<home>/.claude/projects/C--Users-<home>--claude/335bab6b-6a87-4507-b0df/tool-results\"",
        "&& python3 -c \" import json",
        "for f in ['mcp-claude_ai_Robinhood-run_scan-1789017927582.txt']:",
        "  d = json.load(open(f, encoding='utf-8'))",
        "\"",
    ]
    .join(&NEWLINE.to_string())
}

/// What the amber row says for these refusals: the caller's own sentence.
fn refused(denials: Value) -> String {
    let mut n = ClaudeNormalizer::default();
    let record = json!({"type":"result","subtype":"success","is_error":false,"result":"ok","permission_denials":denials});
    let events = n.accept(&super::super::records::claude_record(&record.to_string()));
    let said: Vec<String> = events
        .iter()
        .filter(|e| e.kind == "adapter.diagnostic")
        .map(|e| e.payload["message"].as_str().unwrap_or_default().to_owned())
        .collect();
    assert_eq!(said.len(), 1, "{said:?}");
    said[0].clone()
}

/// The part of the sentence between the backticks: how the command was put.
fn put(sentence: &str) -> String {
    let start = sentence.find('`').expect("a quoted command") + 1;
    let end = sentence[start..].find('`').expect("a closing quote") + start;
    sentence[start..end].to_owned()
}

fn bash(command: &str) -> String {
    refused(json!([{"tool_name":"Bash","tool_input":{"command":command}}]))
}

#[test]
fn keeps_the_first_line_and_stops_there() {
    let sentence = bash(&long());
    let said = put(&sentence);
    assert!(said.chars().count() <= REFUSAL_DETAIL_LIMIT + 3, "{said}");
    assert!(said.contains("cd "));
    assert!(!sentence.contains("python3"), "{sentence}");
    assert!(!sentence.contains("json.load"), "{sentence}");
    assert!(!sentence.contains(NEWLINE), "{sentence}");
    assert_eq!(briefly_put(&long()), said);
}

#[test]
fn leaves_an_ordinary_command_exactly_as_it_was() {
    for command in ["pnpm test", "git status", "node _tools/drive-room-many.mjs"] {
        assert_eq!(put(&bash(command)), command);
        assert_eq!(briefly_put(command), command);
    }
}

#[test]
fn never_cuts_a_token_in_half_when_it_can_avoid_it() {
    let words = format!("git commit -m {}", "word ".repeat(60));
    let said = put(&bash(&words));
    assert!(said.ends_with("..."), "{said}");
    assert!(said[..said.len() - 3].ends_with("word"), "{said}");
}

#[test]
fn still_says_something_when_the_whole_thing_is_one_unbroken_token() {
    let blob = "x".repeat(400);
    let said = put(&bash(&blob));
    assert!(said.chars().count() <= REFUSAL_DETAIL_LIMIT + 3, "{said}");
    assert!(said.ends_with("..."), "{said}");
}

#[test]
fn names_a_connector_the_way_the_activity_rows_name_it() {
    assert_eq!(named_tool("mcp__claude_ai_Robinhood__get_accounts"), "get_accounts on Robinhood");
    assert_eq!(named_tool("mcp__linear__create_issue"), "create_issue on linear");
    assert_eq!(named_tool("Bash"), "Bash");
    let sentence = refused(json!([{"tool_name":"mcp__claude_ai_Robinhood__get_accounts","tool_input":{}}]));
    assert!(sentence.starts_with("Claude Code was not permitted to use get_accounts on Robinhood, so it did not."), "{sentence}");
    assert!(!sentence.contains("mcp__"), "{sentence}");
    assert!(bash("git status").starts_with("Claude Code was not permitted to use Bash `git status`, so it did not."));
}

#[test]
fn gives_a_connector_refusal_a_reason_the_person_can_act_on() {
    let connectors = refused(json!([{"tool_name":"mcp__claude_ai_Robinhood__get_accounts","tool_input":{}}]));
    assert!(connectors.contains("Give this teammate the connector in its card, or run it in Auto."), "{connectors}");
    assert!(!connectors.contains("no way to put that question"), "{connectors}");
    assert!(connectors.contains("either you said no when it asked"), "{connectors}");
    assert!(bash("node t.mjs").contains("before running commands"));
    let mixed = refused(json!([{"tool_name":"Bash","tool_input":{"command":"ls"}},{"tool_name":"mcp__x__y","tool_input":{}}]));
    assert!(mixed.contains("before running commands"), "{mixed}");
    assert!(mixed.starts_with("Claude Code was not permitted to use 2 tools, so it did not: Bash `ls`; y on x."), "{mixed}");
}

#[test]
fn says_a_refused_read_reached_outside_the_folder_and_never_claims_a_mode_it_does_not_know() {
    // case: says a refused read reached outside the folder, and never claims a mode it does not know (0.443)
    let read = "Claude Code asks before reading outside this folder, and a printed run has no way to answer. Auto lets it read anywhere.";
    let grep = refused(json!([{"tool_name":"Grep","tool_input":{"description":"find x"}}]));
    assert!(grep.ends_with(&format!("so it did not. {read}")), "{grep}");
    let two = refused(json!([{"tool_name":"Read","tool_input":{"file_path":"/elsewhere/a"}},{"tool_name":"Glob","tool_input":{}}]));
    assert!(two.ends_with(&format!(". {read}")), "{two}");
    assert_eq!(why_refused(&["Grep"]), read);
    assert_eq!(why_refused(&["Read", "Glob"]), read);
    for tools in [json!([{"tool_name":"Grep"}]), json!([{"tool_name":"Bash"}]), json!([{"tool_name":"Bash"},{"tool_name":"Read"}])] {
        assert!(!refused(tools).contains("allows edits"));
    }
}

// My lane-check (24 Nov): the cut is Electron's 96 characters (claude-events.ts REFUSAL_DETAIL_LIMIT), taken at a
// space in the last 24 when there is one, then "...". A long one-line command shows its start, not all of it.
#[test]
fn a_long_refused_command_is_cut_at_96_characters_on_a_space() {
    let words = "word ".repeat(40); // 200 characters, a space every 5
    let said = super::briefly_put(&words);
    assert!(said.ends_with("..."), "{said}");
    let kept = said.trim_end_matches("...");
    assert!(kept.chars().count() <= 96, "{} characters: {said}", kept.chars().count());
    assert!(kept.chars().count() > 72, "cut too early: {said}");
    assert_eq!(kept, "word ".repeat(19).trim_end(), "cut on the last space inside 96");
    // 96 or fewer: left whole.
    let short = "x".repeat(96);
    assert_eq!(super::briefly_put(&short), short);
}

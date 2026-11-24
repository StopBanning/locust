//! Electron's `packages/runtime-adapters/test/antigravity-question.test.ts`, case by case, through the caller: the
//! Antigravity transcript reader (`TranscriptNormalizer::accept_line`) meets an `ask_question` call in a
//! `PLANNER_RESPONSE` step and puts the question on the tool row the mission service raises its card from
//! (antigravity_mission.rs `note_questions`). Electron's `antigravityQuestion` is `question` here.
use super::*;

/// The two payloads, verbatim from real transcripts under `~/.gemini/antigravity/brain/` (captured 2026-09-08), double
/// encoding and all: every value in an Antigravity `args` is a JSON-encoded string.
fn colin_screenshot_args() -> Value {
    json!({
        "questions": "[{\"is_multi_select\":false,\"options\":[\"(Recommended) Run performance benchmark tests\",\"Inspect codebase architecture\",\"Execute automated unit tests\",\"Review system environment settings\"],\"question\":\"Which test option would you like to select?\"}]",
        "toolAction": "\"Prompting user with options\"",
        "toolSummary": "\"Interactive option selection\"",
    })
}

fn folder_args() -> Value {
    json!({
        "questions": "[{\"is_multi_select\":false,\"options\":[\"Organise by file type (e.g., docs/ for markdown and text/ for plain text)\",\"Organise by functional role (e.g., docs/ for documentation and src/ for working/source files)\",\"Organise by topic or workflow (e.g., notes/ and scratch/ folders)\",\"Maintain a flat root layout (keep all files in the root directory with consistent naming prefixes)\"],\"question\":\"How would you like to organise the files in this folder?\"}]",
        "toolAction": "\"Asking folder organisation options\"",
        "toolSummary": "\"Folder organisation options\"",
    })
}

/// The question the tool row carries when the transcript reader meets this `ask_question` call, if any.
fn asked(args: Value) -> Option<Value> {
    let mut reader = TranscriptNormalizer::default();
    let line = json!({"step_index": 4, "type": "PLANNER_RESPONSE", "status": "DONE", "tool_calls": [{"name": "ask_question", "args": args}]});
    let events = reader.accept_line(&line.to_string());
    let started = events.iter().find(|e| e.kind == "tool.started").expect("the call is a tool row");
    started.payload.get("question").cloned()
}

fn options(q: &Value) -> Vec<String> {
    q["options"].as_array().unwrap().iter().map(|o| o.as_str().unwrap().to_owned()).collect()
}

#[test]
fn reads_the_question_colin_was_shown_and_its_four_options() {
    let q = asked(colin_screenshot_args()).expect("a question");
    assert_eq!(q["question"], "Which test option would you like to select?");
    assert_eq!(options(&q).len(), 4);
    assert_eq!(options(&q)[0], "(Recommended) Run performance benchmark tests");
    assert_eq!(q["multiSelect"], false);
}

#[test]
fn reads_a_second_real_payload_the_same_way() {
    let q = asked(folder_args()).expect("a question");
    assert!(q["question"].as_str().unwrap().starts_with("How would you like to organise"), "{q}");
    assert_eq!(options(&q).len(), 4);
}

#[test]
fn sees_through_the_double_encoding_rather_than_around_it() {
    assert!(colin_screenshot_args()["questions"].is_string());
    assert!(asked(colin_screenshot_args()).is_some());
    assert!(question(&colin_screenshot_args()).is_some());
}

#[test]
fn a_call_that_asks_nothing_carries_no_question() {
    // case: says nothing rather than something wrong for a tool that is not asking
    for args in [
        json!({"TargetFile": "\"c:/x/y.txt\""}),
        Value::Null,
        json!({"questions": "not json at all"}),
        json!({"questions": "[]"}),
        json!({"questions": "[{\"options\":[\"a\"]}]"}),
    ] {
        assert_eq!(asked(args.clone()), None, "{args}");
        assert_eq!(question(&args), None, "{args}");
    }
}

#[test]
fn keeps_a_question_that_carries_no_options() {
    let q = asked(json!({"questions": "[{\"question\":\"What should I call it?\"}]"})).expect("a question");
    assert_eq!(q["question"], "What should I call it?");
    assert!(options(&q).is_empty());
}

#[test]
fn carries_multi_select_through_since_it_changes_what_an_answer_means() {
    let q = asked(json!({"questions": "[{\"is_multi_select\":true,\"question\":\"Which ones?\",\"options\":[\"a\",\"b\"]}]"})).expect("a question");
    assert_eq!(q["multiSelect"], true);
}

//! tool-rows-say-what-they-did.test.ts: a command row leads with what the model said it was doing, on the two runtimes
//! that write a sentence (Claude Code and OpenCode), and goes on showing the command when nothing was written.

use super::claude_tool_title;
use crate::runtime::opencode_events::opencode_tool_title;
use serde_json::{Value, json};

// case: takes the model’s own description
#[test]
fn a_bash_row_takes_the_models_own_description() {
    assert_eq!(
        claude_tool_title("Bash", &json!({"command": "grep -rn \"contributor\" apps/", "description": "Check what the app says about the free route"})).as_deref(),
        Some("Check what the app says about the free route")
    );
}

// case: has nothing to say when the model wrote nothing
#[test]
fn a_bash_row_has_nothing_to_say_when_the_model_wrote_nothing() {
    assert_eq!(claude_tool_title("Bash", &json!({"command": "ls"})), None);
    assert_eq!(claude_tool_title("Bash", &json!({"command": "ls", "description": ""})), None);
    assert_eq!(claude_tool_title("Bash", &Value::Null), None);
    // A description of NUL alone is nothing once the ledger's refused character is taken out.
    assert_eq!(claude_tool_title("Bash", &json!({"command": "ls", "description": "\u{0}"})), None);
}

// case: is only for commands
#[test]
fn the_title_is_only_for_commands() {
    assert_eq!(claude_tool_title("Read", &json!({"file_path": "/x", "description": "read it"})), None);
    assert_eq!(claude_tool_title("Task", &json!({"description": "go and look"})), None);
}

// case: reaches OpenCode too, which is the runtime anyone can actually run
#[test]
fn the_title_reaches_opencode_too() {
    assert_eq!(
        opencode_tool_title("bash", &json!({"command": "ls -la", "description": "List the release assets"})).as_deref(),
        Some("List the release assets")
    );
    assert_eq!(opencode_tool_title("shell", &json!({"command": "ls", "description": "List it"})).as_deref(), Some("List it"));
    assert_eq!(opencode_tool_title("bash", &json!({"command": "ls -la"})), None);
    assert_eq!(opencode_tool_title("bash", &json!({"command": "ls -la", "description": ""})), None);
    assert_eq!(opencode_tool_title("read", &json!({"filePath": "/x", "description": "read it"})), None);
    assert_eq!(opencode_tool_title("task", &json!({"description": "go and look"})), None);
}

/// The `tool.started` title OpenCode's normalizer writes for one finished tool record.
fn opencode_row_title(tool: &str, input: Value) -> Option<String> {
    let mut n = crate::runtime::opencode_events::Normalizer::new(Some("opencode/beam-free"));
    let record = json!({"type": "tool_use", "sessionID": "ses_1", "part": {"callID": "call_1", "tool": tool,
        "state": {"status": "completed", "input": input, "output": "", "metadata": {}}}});
    let started = n.accept(&record).into_iter().find(|e| e.kind == "tool.started").expect("a tool.started");
    started.payload["title"].as_str().map(str::to_owned)
}

// case: reaches OpenCode too, which is the runtime anyone can actually run
#[test]
fn an_opencode_row_carries_the_title_only_when_a_command_said_one() {
    assert_eq!(opencode_row_title("bash", json!({"command": "ls -la", "description": "List the release assets"})).as_deref(), Some("List the release assets"));
    assert_eq!(opencode_row_title("bash", json!({"command": "ls -la", "description": ""})), None);
    assert_eq!(opencode_row_title("read", json!({"filePath": "/x", "description": "read it"})), None);
}

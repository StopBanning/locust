//! Electron's `main/todo-list-is-opt-in.test.ts` (6 cases), through `people::turn_brief`, which reads the person's
//! "keep a todo list" switch from the team's settings and hands `standing_formats` the runtime it briefs.
use super::*;

const ASK: &str = "KEEP A TODO LIST";

/// A teammate's brief for `runtime`, with the team's settings as given.
fn brief(runtime: &str, settings: serde_json::Value) -> String {
    let root = std::env::temp_dir().join(format!("locust-todo-opt-in-{}", crate::invariants::unique_nanos()));
    std::fs::create_dir_all(root.join("electron")).unwrap();
    std::fs::create_dir_all(root.join("shop")).unwrap();
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let mut team = serde_json::json!({"schemaVersion":1,"teammates":[{"teammateId":"tm_wren","name":"Wren","role":"builder"}],"missionOwners":{}});
    team["settings"] = settings;
    std::fs::write(store.electron().join("teammates.json"), team.to_string()).unwrap();
    let (briefed, _) = crate::people::turn_brief(&store, "tm_wren", &root.join("shop"), "Do the thing.", None, Some(runtime), &[], 0).unwrap();
    let _ = std::fs::remove_dir_all(&root);
    briefed
}

fn on(runtime: &str) -> String {
    brief(runtime, serde_json::json!({"keepATodoList": true}))
}

// case: says nothing about todo lists unless asked to
#[test]
fn a_brief_says_nothing_about_todo_lists_unless_the_person_asked() {
    assert!(!brief("codex", serde_json::json!({"keepATodoList": false})).contains(ASK));
}

// case: asks once, and asks for the list to be kept current
#[test]
fn a_brief_asks_once_for_a_todo_list_kept_current() {
    let prompt = on("codex");
    assert_eq!(prompt.matches(ASK).count(), 1, "{prompt}");
    assert!(prompt.contains("keep it current"), "{prompt}");
    assert!(prompt.contains("stale list is worse than no list"), "{prompt}");
}

// case: leaves the person words last, so the ask still reads closest to the asking
#[test]
fn a_todo_brief_leaves_the_persons_words_last() {
    assert!(on("codex").trim_end().ends_with("Do the thing."));
}

// case: offers it only to runtimes that have the tool
#[test]
fn the_todo_ask_goes_only_to_runtimes_with_the_tool() {
    for runtime in ["codex", "cursor", "opencode"] {
        assert!(runtime_keeps_a_todo_list(runtime), "{runtime}");
        assert!(on(runtime).contains(ASK), "{runtime}");
    }
}

// case: never offers it to Claude Code, which has no such tool
#[test]
fn the_todo_ask_never_goes_to_claude_code() {
    for runtime in ["claude", "copilot", "gemini", "antigravity"] {
        assert!(!runtime_keeps_a_todo_list(runtime), "{runtime}");
        assert!(!on(runtime).contains(ASK), "{runtime}");
    }
}

// case: describes a tool rather than a format, so it cannot compete with the real one
#[test]
fn the_todo_ask_names_a_tool_not_a_format() {
    let prompt = on("opencode");
    let at = prompt.find(ASK).unwrap();
    let section = &prompt[at..at + todo_section().len()];
    assert!(section.contains("your own todo tool"), "{section}");
    assert!(!section.contains('<'), "{section}");
}

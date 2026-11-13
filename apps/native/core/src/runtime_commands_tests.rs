//! EACH RUNTIME'S OWN SLASH COMMANDS (0.426): main/runtime-commands.test.ts.
use super::*;
use serde_json::json;

fn temp() -> (std::path::PathBuf, Store) {
    let dir = std::env::temp_dir().join(format!("locust-runtime-commands-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(dir.join("electron")).unwrap();
    let store = Store::new(dir.join("native"), dir.join("electron")).unwrap();
    (dir, store)
}

fn info(name: &str, description: &str, hint: &str) -> CommandInfo {
    CommandInfo { name: name.into(), description: description.into(), argument_hint: hint.into() }
}
fn compact() -> CommandInfo {
    info("compact", "Clear conversation history but keep a summary in context", "<optional custom summarization instructions>")
}
fn context() -> CommandInfo {
    info("context", "Show current context usage", "")
}
fn model() -> CommandInfo {
    info("model", "Set the AI model for Claude Code", "[model]")
}
fn mcp() -> CommandInfo {
    info("mcp", "Manage MCP servers", "")
}

fn names(rows: &[CommandInfo]) -> Vec<&str> {
    rows.iter().map(|c| c.name.as_str()).collect()
}

#[test]
fn the_command_a_message_names_is_the_word_after_a_leading_slash() {
    assert_eq!(command_named("/compact").as_deref(), Some("compact"));
    assert_eq!(command_named("  /compact keep the test names").as_deref(), Some("compact"));
    assert_eq!(command_named("/security-review").as_deref(), Some("security-review"));
    assert_eq!(command_named("/plugin:skill now").as_deref(), Some("plugin:skill"));
}

#[test]
fn the_command_a_message_names_is_nothing_when_the_slash_is_not_at_the_start_or_has_no_name() {
    assert_eq!(command_named("please /compact"), None);
    assert_eq!(command_named("/"), None);
    assert_eq!(command_named("/src/main/index.ts is broken"), None);
    assert_eq!(command_named("compact"), None);
    // A name a command could not have: past 64 characters, or glued to other characters.
    assert_eq!(command_named(&format!("/{}", "a".repeat(65))), None);
    assert_eq!(command_named(&format!("/{} go", "a".repeat(64))).as_deref(), Some(&*"a".repeat(64)));
    assert_eq!(command_named("/compact!"), None);
}

#[test]
fn what_follows_the_name_is_the_commands_own_message() {
    assert_eq!(after_name("  /compact  keep the test names  ", "compact"), "keep the test names");
    assert_eq!(after_name("/review", "review"), "");
}

#[test]
fn the_commands_a_runtime_listed_are_offered_without_the_ones_that_would_change_the_persons_own_setup() {
    let (_dir, store) = temp();
    let kept = for_store(&store);
    assert!(kept.set("claude", &[compact(), model(), context(), mcp()]).unwrap());
    // model is the teammate's, chosen in Locust; mcp would rewrite the person's settings.
    assert!(HIDDEN_CLAUDE.contains(&"model"));
    let listed = kept.list();
    assert_eq!(listed["claude"], vec![compact(), context()]);
    assert_eq!(listed["codex"], codex_commands());
    assert_eq!(listed.len(), 2);
}

#[test]
fn a_message_is_a_command_only_for_the_runtime_that_listed_it_and_never_a_hidden_one() {
    let (_dir, store) = temp();
    let kept = for_store(&store);
    kept.set("claude", &[compact(), model(), context()]).unwrap();
    assert!(kept.is_command(Some("claude"), "/compact"));
    assert!(kept.is_command(Some("claude"), "/compact keep the test names"));
    assert!(!kept.is_command(Some("claude"), "/model opus"));
    assert!(!kept.is_command(Some("claude"), "/nonsense"));
    assert!(!kept.is_command(Some("claude"), "please /compact"));
    assert!(!kept.is_command(Some("cursor"), "/compact"));
    assert!(!kept.is_command(None, "/compact"));
}

#[test]
fn commands_are_kept_per_runtime_opencodes_beside_claude_codes_none_of_opencodes_hidden() {
    let (_dir, store) = temp();
    let kept = for_store(&store);
    let init = info("init", "guided AGENTS.md setup", "[arguments]");
    kept.set("claude", &[compact(), model()]).unwrap();
    // OpenCode's list holds a name Claude Code hides: it is not hidden there.
    kept.set("opencode", &[init.clone(), model()]).unwrap();
    let listed = kept.list();
    assert_eq!(listed["claude"], vec![compact()]);
    assert_eq!(listed["opencode"], vec![init.clone(), model()]);
    assert!(kept.is_command(Some("opencode"), "/init keep it short"));
    assert!(kept.is_command(Some("opencode"), "/model"));
    assert!(!kept.is_command(Some("opencode"), "/compact"));
    assert!(!kept.is_command(Some("claude"), "/init"));
    assert_eq!(for_store(&store).list()["opencode"], vec![init, model()]);
}

#[test]
fn commands_are_kept_across_a_restart_and_a_list_that_did_not_change_is_not_written_again() {
    let (dir, store) = temp();
    let first = for_store(&store);
    assert!(first.set("claude", &[compact(), context()]).unwrap());
    let written = std::fs::metadata(store.root().join(FILE)).unwrap().modified().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(20));
    assert!(!first.set("claude", &[compact(), context()]).unwrap());
    assert_eq!(std::fs::metadata(store.root().join(FILE)).unwrap().modified().unwrap(), written);
    // A new store over the same folder: the first message of a session is recognised.
    let again = Store::new(dir.join("native"), dir.join("electron")).unwrap();
    let again = for_store(&again);
    assert_eq!(names(&again.list()["claude"]), vec!["compact", "context"]);
    assert!(again.is_command(Some("claude"), "/context"));
    let file: Value = serde_json::from_str(&std::fs::read_to_string(store.root().join(FILE)).unwrap()).unwrap();
    assert_eq!(file["schemaVersion"], 1);
    assert_eq!(file["byRuntime"]["claude"][0]["argumentHint"], "<optional custom summarization instructions>");
}

#[test]
fn commands_are_nothing_not_an_error_before_any_run_listed_them_or_when_the_file_is_unreadable() {
    let (_dir, store) = temp();
    let kept = for_store(&store);
    let listed = kept.list();
    assert_eq!(listed.keys().map(String::as_str).collect::<Vec<_>>(), vec!["codex"]);
    std::fs::create_dir_all(store.root()).unwrap();
    std::fs::write(store.root().join(FILE), "{ not json").unwrap();
    assert_eq!(kept.list().keys().map(String::as_str).collect::<Vec<_>>(), vec!["codex"]);
    assert!(!kept.is_command(Some("claude"), "/compact"));
    // A runtime that lists none of its own cannot be given a list.
    assert!(kept.set("codex", &[compact()]).is_err());
}

#[test]
fn codexs_own_commands_are_the_ones_its_terminal_sends_as_requests_there_whether_or_not_codex_has_run() {
    let (_dir, store) = temp();
    let kept = for_store(&store);
    assert_eq!(names(&kept.list()["codex"]), vec!["review", "compact", "init"]);
    assert!(kept.is_command(Some("codex"), "/review only the tests"));
    assert!(kept.is_command(Some("codex"), "/compact"));
    assert!(!kept.is_command(Some("codex"), "/model gpt-6-luna"));
    assert!(!kept.is_command(Some("codex"), "/diff"));
}

#[test]
fn a_list_from_a_cli_is_bounded_and_keeps_only_names_a_command_could_have() {
    let rows = json!([
        {"name": "/compact", "description": "  Clear   the\nhistory ", "argumentHint": "<x>"},
        {"name": "bad name"},
        {"name": "-leading"},
        {"description": "no name"},
        "not an object",
        {"name": "x".repeat(65)},
        {"name": "long", "description": "d".repeat(500), "argumentHint": "h".repeat(500)}
    ]);
    let got = commands_from(&rows);
    assert_eq!(names(&got), vec!["compact", "long"]);
    assert_eq!(got[0].description, "Clear the history");
    assert_eq!(got[1].description.len(), 240);
    assert_eq!(got[1].argument_hint.len(), 80);
    let many: Vec<Value> = (0..500).map(|n| json!({"name": format!("c{n}")})).collect();
    assert_eq!(commands_from(&Value::Array(many)).len(), 400);
    assert!(commands_from(&json!({"name": "compact"})).is_empty());
}

#[test]
fn until_native_has_a_list_of_its_own_it_reads_the_one_electron_kept() {
    let (dir, store) = temp();
    std::fs::write(
        dir.join("electron").join(FILE),
        json!({"schemaVersion": 1, "byRuntime": {"claude": [{"name": "compact", "description": "d", "argumentHint": ""}]}}).to_string(),
    )
    .unwrap();
    let kept = for_store(&store);
    assert!(kept.is_command(Some("claude"), "/compact"));
    // The first list native learns is written to its own file, beside what it read; Electron's file is not touched.
    assert!(kept.set("opencode", &[info("init", "", "")]).unwrap());
    assert_eq!(names(&kept.list()["claude"]), vec!["compact"]);
    assert!(std::fs::read_to_string(dir.join("electron").join(FILE)).unwrap().find("opencode").is_none());
}

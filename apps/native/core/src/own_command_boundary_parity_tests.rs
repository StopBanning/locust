//! Native has no Electron renderer IPC. Check the actual entry points reachable from a link,
//! another app, and a routine instead of introducing an IPC or running a shell.
use serde_json::json;

// case: is not a way for a routine, a link, or another app to run a command
#[test]
fn the_native_links_door_and_routine_have_no_own_command_entry() {
    // A link may supply words in a draft, but its parser has no command action.
    assert!(crate::locust_link::parse_locust_link("locust://own-command?prompt=echo+fixture").is_none());
    let draft = crate::locust_link::parse_locust_link("locust://new?prompt=Ordinary+draft").unwrap();
    assert_eq!(draft.prompt, "Ordinary draft");

    let tools = crate::door::tool_list();
    let names: Vec<_> = tools.as_array().unwrap().iter().filter_map(|one| one["name"].as_str()).collect();
    assert!(names.contains(&"start_conversation"), "the actual door catalog is present");
    for name in ["own-commands:run", "own_command", "run_own_command", "run_shell"] {
        assert!(!names.contains(&name), "{name} was publicly offered");
    }
    let root = std::env::temp_dir().join(format!("locust-no-own-command-{}", crate::invariants::unique_nanos()));
    struct Temp(std::path::PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }
    let temp = Temp(root);
    let store = crate::store::Store::new(temp.0.join("native"), temp.0.join("electron")).unwrap();
    let door = crate::door::tools_from_anywhere(store);
    for name in ["own-commands:run", "run_own_command"] {
        let refusal = door(name, &json!({"command":"echo fixture"}));
        assert!(refusal.is_error, "{name}: {refusal:?}");
    }

    // Like Electron's negative source check: the routine owns separate declared steps.
    // It never calls the interactive bash-command and Markdown-command paths.
    let runner = include_str!("routine_runner.rs");
    assert!(runner.contains("impl"), "the routine implementation is present");
    for boundary in ["own-commands:run", "own_command::", "own_commands::"] {
        assert!(!runner.contains(boundary), "routine unexpectedly refers to {boundary}");
    }
}

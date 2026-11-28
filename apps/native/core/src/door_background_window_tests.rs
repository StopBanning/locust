use super::*;

#[test]
fn the_live_door_lists_saved_background_turns_without_folder_session_or_writes() {
    let root = std::env::temp_dir().join(format!("locust-door-background-{}", crate::invariants::unique_nanos()));
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    std::fs::create_dir_all(store.root()).unwrap();
    let file = crate::native::background_runs_path(&store);
    let saved = json!([{
        "id": "aa1234", "account": "work", "prompt": "Review the notes", "startedAt": "2026-10-01T12:00:00Z",
        "state": "blocked", "waitingFor": "answers", "teammateId": "tm_ada",
        "folder": "/private/folder", "sessionId": "bb3456", "extra": "not public"
    }]).to_string();
    std::fs::write(&file, &saved).unwrap();
    let door = Door::for_store(store.clone(), "fixture-locust");
    assert!(door.set_enabled(true).settings.enabled);
    let args = json!({});
    let got = crate::cli_client::LocustClient::new(store.root()).call("list_background_runs", args.clone()).unwrap();
    let want = json!([{
        "id": "aa1234", "account": "work", "prompt": "Review the notes", "startedAt": "2026-10-01T12:00:00Z",
        "state": "blocked", "waitingFor": "answers", "teammateId": "tm_ada"
    }]);
    assert_eq!(got, want);
    let remote = tools_from_anywhere(store.clone());
    assert_eq!(serde_json::from_str::<Value>(&remote("list_background_runs", &args).text).unwrap(), want);
    let scope = Arc::new(crate::door_tools::VoiceScope::new(root.clone(), None));
    let tools = Tools::new(Engine {
        store, settings: Arc::new(Mutex::new(Settings::default())), voice: true,
    }).with_voice(scope.clone());
    assert_eq!(serde_json::from_str::<Value>(&call_live_tools(&tools, "list_background_runs", &args).text).unwrap(), want);
    scope.close();
    assert!(call_live_tools(&tools, "list_background_runs", &args).is_error);
    assert_eq!(std::fs::read_to_string(file).unwrap(), saved);
    door.dispose();
    let _ = std::fs::remove_dir_all(root);
}

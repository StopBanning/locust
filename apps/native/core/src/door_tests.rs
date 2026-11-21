//! The door's listener, files and folder rule (locust-mcp-host.ts, app-folder.ts, the stdio bridge).
use super::*;
use std::io::{Read, Write};

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("locust-door-{name}-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn echo_door(dir: &Path) -> Door {
    let call: Caller = Arc::new(|name: &str, args: &Value| crate::door_tools::ToolResult::ok(format!("{name} {args}")).to_json());
    Door::with(dir.to_path_buf(), "/Applications/Locust Native.app/x", Arc::new(Mutex::new(Settings::default())), call)
}

fn post(url: &str, token: Option<&str>, extra: &str, body: &str) -> String {
    let address = url.trim_start_matches("http://").split('/').next().unwrap().to_owned();
    let mut s = TcpStream::connect(address).unwrap();
    let auth = token.map(|t| format!("authorization: Bearer {t}\r\n")).unwrap_or_default();
    write!(s, "POST /tools/call HTTP/1.1\r\nhost: x\r\n{auth}{extra}content-length: {}\r\n\r\n{body}", body.len()).unwrap();
    let mut out = String::new();
    s.read_to_string(&mut out).unwrap();
    out
}

#[test]
fn the_door_is_off_until_turned_on_and_answers_only_its_current_token() {
    let dir = temp("token");
    let door = echo_door(&dir);
    std::fs::write(door.connection_path(), "{\"stale\":true}").unwrap();
    let state = door.load();
    assert!(!state.settings.enabled);
    assert!(!door.connection_path().exists(), "a crashed process's token is never reused");
    assert!(state.claude_command.is_none());
    assert!(state.openclaw_command.is_none(), "no setup lines while off");

    let on = door.set_enabled(true);
    assert!(on.settings.enabled, "{:?}", on.message);
    assert!(on.claude_command.as_deref().unwrap().contains("--door-bridge '"));
    assert!(on.openclaw_command.as_deref().unwrap().starts_with("openclaw mcp add locust-native --command '"));
    let connection: Value = serde_json::from_str(&std::fs::read_to_string(door.connection_path()).unwrap()).unwrap();
    let (url, token) = (connection["url"].as_str().unwrap(), connection["token"].as_str().unwrap());
    assert_eq!(token.len(), 64);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(door.connection_path()).unwrap().permissions().mode() & 0o777, 0o600);
    }
    let body = r#"{"name":"list_teammates","arguments":{}}"#;
    assert!(post(url, None, "", body).starts_with("HTTP/1.1 401"));
    assert!(post(url, Some(&"0".repeat(64)), "", body).starts_with("HTTP/1.1 401"));
    assert!(post(url, Some(token), "origin: https://evil.example\r\n", body).starts_with("HTTP/1.1 403"));
    assert!(post(url, Some(token), "", "not json").starts_with("HTTP/1.1 400"));
    let ok = post(url, Some(token), "", body);
    assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
    assert!(ok.contains("list_teammates {}"));

    // The bridge reads the same file and gets the same answer.
    let through = bridge_call(&door.connection_path(), "list_teammates", &json!({}));
    assert_eq!(through["content"][0]["text"], "list_teammates {}");

    // Off: the listener and the file are gone, and the bridge says Locust is not running.
    let off = door.set_enabled(false);
    assert!(!off.settings.enabled);
    assert!(!door.connection_path().exists());
    assert_eq!(bridge_call(&door.connection_path(), "list_teammates", &json!({}))["content"][0]["text"], NOT_RUNNING);
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(SETTINGS_FILE)).unwrap()).unwrap();
    assert_eq!(saved["enabled"], false);
}

#[test]
fn a_saved_switch_opens_the_door_at_load_with_a_new_token() {
    let dir = temp("reload");
    let first = echo_door(&dir);
    first.load();
    first.set_enabled(true);
    first.set_own_mode(true);
    let old: Value = serde_json::from_str(&std::fs::read_to_string(first.connection_path()).unwrap()).unwrap();
    first.dispose();
    drop(first);
    let again = echo_door(&dir);
    let state = again.load();
    assert!(state.settings.enabled && state.settings.own_mode);
    let new: Value = serde_json::from_str(&std::fs::read_to_string(again.connection_path()).unwrap()).unwrap();
    assert_ne!(old["token"], new["token"]);
}

#[test]
fn a_connection_file_can_only_point_at_this_machine() {
    let dir = temp("redirect");
    let file = dir.join(CONNECTION_FILE);
    for url in ["http://evil.example:1/tools/call", "https://127.0.0.1:1/tools/call", "http://127.0.0.1/tools/call", "http://127.0.0.1:1/other", "http://u:p@127.0.0.1:1/tools/call"] {
        std::fs::write(&file, json!({"url": url, "token": "a".repeat(64)}).to_string()).unwrap();
        assert!(read_connection(&file).is_none(), "{url}");
    }
    std::fs::write(&file, json!({"url": "http://127.0.0.1:1/tools/call", "token": "short"}).to_string()).unwrap();
    assert!(read_connection(&file).is_none());
    std::fs::write(&file, json!({"url": "http://127.0.0.1:1/tools/call", "token": "a".repeat(64)}).to_string()).unwrap();
    assert!(read_connection(&file).is_some());
}

#[test]
fn an_app_may_name_a_known_folder_one_inside_it_or_a_worktree_of_its_repository() {
    let root = temp("folders");
    let known = root.join("project");
    let inside = known.join("src");
    let worktree = root.join("wt");
    let stranger = root.join("stranger");
    for d in [&inside, &worktree, &stranger] {
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::write(root.join("file.txt"), "x").unwrap();
    let repo = root.join("project/.git");
    let common = |p: &Path| -> Option<PathBuf> {
        let p = p.canonicalize().ok()?;
        (p.ends_with("project") || p.ends_with("wt")).then(|| repo.clone())
    };
    let known_list = [known.clone()];
    assert_eq!(check_app_folder(inside.to_str().unwrap(), &known_list, &common).unwrap(), inside.canonicalize().unwrap());
    assert!(check_app_folder(worktree.to_str().unwrap(), &known_list, &common).is_ok());
    assert!(check_app_folder(stranger.to_str().unwrap(), &known_list, &common).unwrap_err().contains("is not a folder Locust knows"));
    assert_eq!(check_app_folder("relative/path", &known_list, &common).unwrap_err(), "folder is a full path, from the top of the disk.");
    assert!(check_app_folder(root.join("file.txt").to_str().unwrap(), &known_list, &common).unwrap_err().ends_with("is a file, not a folder."));
    assert!(check_app_folder(root.join("gone").to_str().unwrap(), &known_list, &common).unwrap_err().starts_with("There is no folder at"));
}

#[test]
fn the_bridge_lists_the_tools_and_marks_those_that_only_read() {
    let tools = tool_list();
    let names: Vec<&str> = tools.as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    // Electron's bridge order: answer_question after read_reply, carry_on after list_conversations (0.878).
    assert_eq!(
        names,
        ["list_teammates", "start_conversation", "send_message", "read_reply", "answer_question", "stop_turn", "list_conversations", "carry_on", "list_models", "list_background_runs", "list_routines", "run_routine", "routine_status"]
    );
    for t in tools.as_array().unwrap() {
        let reads = t["annotations"]["readOnlyHint"] == true;
        let writes = ["start_conversation", "send_message", "answer_question", "stop_turn", "carry_on", "run_routine"].contains(&t["name"].as_str().unwrap());
        assert_eq!(reads, !writes, "{}", t["name"]);
    }
    // Electron 0.879: start_conversation needs only a message; with no teammate it takes the agent as runtime.
    let start = tools.as_array().unwrap().iter().find(|t| t["name"] == "start_conversation").unwrap();
    assert_eq!(start["inputSchema"]["required"], json!(["message"]));
    assert_eq!(start["inputSchema"]["properties"]["runtime"]["maxLength"], 40);
    assert!(start["description"].as_str().unwrap().contains("or start a conversation with no teammate on an AI agent you name as runtime, from list_models with no teammate"));
}

// locust-mcp-host.ts mcpSetup (0.754): OpenClaw's line names the server (locust-native, beside Electron's), the program and the bridge's arguments one
// --arg each, POSIX-quoted, and carries the connection file's path, never a token.
#[test]
fn the_openclaw_line_adds_the_door_by_command_and_one_arg_each() {
    let (_, _, openclaw) = super::setup("/Apps/Locust Native/locust-native", "/Users/x/it's here/connection.json");
    assert_eq!(
        openclaw,
        "openclaw mcp add locust-native --command '/Apps/Locust Native/locust-native' --arg '--door-bridge' --arg '/Users/x/it'\\''s here/connection.json'"
    );
}

/// every-turn-start-crosses-the-organization-policy.test.ts: "checks voice, phone, MCP and CLI access in the main process,
/// including saved opt-ins". The door asks the organization's `otherApps` at load and at every turn-on: refused, it stays
/// shut, writes nothing, keeps the person's saved switch, and says why.
// case: every way a turn starts crosses the organization policy > checks voice, phone, MCP and CLI access in the main process, including saved opt-ins
#[test]
fn the_organization_keeps_the_door_shut_even_when_the_person_saved_it_open() {
    let dir = temp("policy");
    let first = echo_door(&dir);
    first.load();
    first.set_enabled(true);
    first.set_own_mode(true);
    first.dispose();
    drop(first);
    let said = "Access from other AI apps, the locust command and editors is off. Set by your organization (otherApps in /etc/locust/policy.json). Nothing was started.";
    let refused = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let gate = refused.clone();
    let managed = echo_door(&dir).with_refusal(Arc::new(move || gate.load(Ordering::Relaxed).then(|| said.to_owned())));
    let state = managed.load();
    assert!(!state.settings.enabled, "a saved opt-in does not open it");
    assert!(!state.settings.own_mode, "nothing saved is read while refused (locust-mcp-host.ts load)");
    assert_eq!(state.message.as_deref(), Some(said));
    assert!(state.claude_command.is_none() && !managed.connection_path().exists());
    // Turning it on is refused before anything opens or is saved.
    let saved = std::fs::read_to_string(dir.join(SETTINGS_FILE)).unwrap();
    let state = managed.set_enabled(true);
    assert!(!state.settings.enabled && state.message.as_deref() == Some(said));
    assert!(!managed.connection_path().exists());
    assert_eq!(std::fs::read_to_string(dir.join(SETTINGS_FILE)).unwrap(), saved, "the person's saved switch is kept");
    // Lifted: the next load opens it as the person left it.
    refused.store(false, Ordering::Relaxed);
    let state = managed.load();
    assert!(state.settings.enabled && state.settings.own_mode && state.message.is_none());
    assert!(managed.connection_path().exists());
    // Set again while open: the door's own start refuses too.
    refused.store(true, Ordering::Relaxed);
    managed.dispose();
    assert_eq!(managed.start().unwrap_err(), said);
}

#[test]
fn the_door_reads_the_organizations_other_apps_setting() {
    let dir = temp("policy-real");
    let mut policy = serde_json::Map::new();
    policy.insert("otherApps".into(), json!(false));
    policy.insert("phone".into(), json!(true));
    crate::org_policy::use_policy_on_this_thread(Some(crate::org_policy::PolicyState { path: "/etc/locust/policy.json".into(), present: true, policy, error: None }));
    let door = echo_door(&dir);
    let state = door.set_enabled(true);
    crate::org_policy::use_policy_on_this_thread(None);
    assert!(!state.settings.enabled);
    assert_eq!(
        state.message.as_deref(),
        Some("Access from other AI apps, the locust command and editors is off. Set by your organization (otherApps in /etc/locust/policy.json). Nothing was started.")
    );
    // No policy, no refusal.
    assert!(door.set_enabled(true).settings.enabled);
    door.dispose();
}

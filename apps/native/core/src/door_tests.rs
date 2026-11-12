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

    let on = door.set_enabled(true);
    assert!(on.settings.enabled, "{:?}", on.message);
    assert!(on.claude_command.as_deref().unwrap().contains("--door-bridge '"));
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
    assert_eq!(names.len(), 11);
    for t in tools.as_array().unwrap() {
        let reads = t["annotations"]["readOnlyHint"] == true;
        let writes = ["start_conversation", "send_message", "stop_turn", "run_routine"].contains(&t["name"].as_str().unwrap());
        assert_eq!(reads, !writes, "{}", t["name"]);
    }
}

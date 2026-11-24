//! The door's listener, files and folder rule (locust-mcp-host.ts, app-folder.ts, the stdio bridge).
use super::*;
use std::io::{Read, Write};

pub(super) fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("locust-door-{name}-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub(super) fn echo_door(dir: &Path) -> Door {
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
    // Electron's bridge order: answer_question after read_reply, carry_on, read_usage, set_teammate_model after list_conversations (0.882).
    assert_eq!(
        names,
        ["list_teammates", "start_conversation", "send_message", "read_reply", "answer_question", "stop_turn", "list_conversations", "carry_on", "read_usage", "set_teammate_model", "list_models", "list_background_runs", "list_routines", "run_routine", "routine_status"]
    );
    for t in tools.as_array().unwrap() {
        let reads = t["annotations"]["readOnlyHint"] == true;
        let writes = ["start_conversation", "send_message", "answer_question", "stop_turn", "carry_on", "set_teammate_model", "run_routine"].contains(&t["name"].as_str().unwrap());
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
// case: checks voice, phone, MCP and CLI access in the main process, including saved opt-ins
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

// case: lets an editor answer cards only once the person turns it on, and keeps that and the editor across a restart
#[test]
fn an_editor_answers_cards_only_once_turned_on_and_both_switches_survive_a_restart() {
    let dir = temp("editors-switch");
    // Only a literal true turns it on.
    for odd in [json!("true"), json!(1), Value::Null] {
        std::fs::write(dir.join(SETTINGS_FILE), json!({"enabled": false, "ownMode": false, "editorAnswers": odd, "editorSeen": odd}).to_string()).unwrap();
        let door = echo_door(&dir);
        let state = door.load();
        assert!(!state.settings.editor_answers && !state.settings.editor_seen, "{odd}");
    }
    std::fs::remove_file(dir.join(SETTINGS_FILE)).unwrap();
    let door = echo_door(&dir);
    assert!(!door.load().settings.editor_answers);
    assert!(door.note_editor().settings.editor_seen);
    assert!(door.set_editor_answers(true).settings.editor_answers);
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(SETTINGS_FILE)).unwrap()).unwrap();
    assert_eq!(saved, json!({"enabled": false, "ownMode": false, "editorAnswers": true, "editorSeen": true}));
    let again = echo_door(&dir);
    let state = again.load();
    assert!(state.settings.editor_answers && state.settings.editor_seen);
    assert!(!again.set_editor_answers(false).settings.editor_answers);
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(SETTINGS_FILE)).unwrap()).unwrap();
    assert_eq!(saved, json!({"enabled": false, "ownMode": false, "editorSeen": true}));
}

/// index.ts: `cli/*` names go to the editors' sessions in front of the tools, and close with the door.
#[test]
fn an_editors_names_are_answered_by_its_sessions_and_close_with_the_door() {
    let dir = temp("editors-route");
    let store = Store::new(dir.join("native"), dir.join("electron")).unwrap();
    let door = Door::for_store(store, "/x");
    let answer = (door.call)("cli/keepalive", &json!({"sessionId": "x", "key": "a".repeat(64)}));
    assert!(answer["content"][0]["text"].as_str().unwrap().contains("no longer connected"), "{answer}");
    let tool = (door.call)("keepalive", &json!({}));
    assert_eq!(tool["content"][0]["text"], "That tool is not offered by Locust.");
    door.dispose();
    let closed = (door.call)("cli/status", &json!({}));
    assert!(closed["content"][0]["text"].as_str().unwrap().contains("Locust is closing"), "{closed}");
}

#[test]
fn engine_reads_usage_and_saves_teammate_model_under_org_policy() {
    let dir = temp("engine-usage-model");
    let store = Store::new(dir.join("native"), dir.join("electron")).unwrap();
    store
        .save_json(
            "usage-readings.json",
            &json!({"codex": "5-hour window 78% used · from a run at 2026-11-22T05:13:36.175Z"}),
        )
        .unwrap();
    std::fs::create_dir_all(dir.join("native/mission-ledger")).unwrap();
    std::fs::write(
        dir.join("native/mission-ledger/m_lim.jsonl"),
        format!(
            "{}\n{}\n",
            json!({"recordType": "mission.created", "metadata": {"missionId": "m_lim", "runtime": "codex", "resolvedRouteId": "codex:gpt-5-account:default"}}),
            json!({"recordType": "mission.event", "event": {"type": "route.limit_detected", "occurredAt": "2026-11-22T05:14:00.000Z", "sourceAdapter": "codex", "payload": {"kind": "quota-exhausted", "message": "Codex is at its limit."}}}),
        ),
    )
    .unwrap();
    store
        .save_json(
            "teammates.json",
            &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codec", "name": "Codec", "role": "Custom", "hue": "lime", "createdAt": "2026-11-22T00:00:00.000Z",
                "route": {"runtime": "codex", "model": "gpt-6.1-sol", "mode": "auto", "effort": "high"}}], "missionOwners": {}}),
        )
        .unwrap();
    let engine = Engine { store: store.clone(), settings: Arc::new(Mutex::new(Settings { own_mode: true, conversations: true, ..Default::default() })), voice: false };
    let (windows, at_limit) = engine.usage().unwrap();
    assert_eq!(windows.get("codex").map(String::as_str), Some("5-hour window 78% used · from a run at 2026-11-22T05:13:36.175Z"));
    assert_eq!(at_limit.get("codex").map(String::as_str), Some("Codex is at its limit."));

    // Organization policy refuses a disallowed model before touching teammates.json.
    let mut policy = serde_json::Map::new();
    policy.insert("allowedModels".into(), json!({"codex": ["gpt-6.1-*"]}));
    crate::org_policy::use_policy_on_this_thread(Some(crate::org_policy::PolicyState { path: "/etc/locust/policy.json".into(), present: true, policy, error: None }));
    let denied = engine.save_model("tm_codec", &json!({"runtime": "codex", "model": "gpt-6.2-sol", "mode": "auto", "effort": "high"})).unwrap_err();
    crate::org_policy::use_policy_on_this_thread(None);
    assert!(denied.contains("gpt-6.2-sol") && denied.contains("not allowed"), "{denied}");

    // Without policy refusal, the new model is saved to teammates.json.
    engine.save_model("tm_codec", &json!({"runtime": "codex", "model": "gpt-6.2-sol", "mode": "auto", "effort": "high"})).unwrap();
    let team = crate::people::team(&store).unwrap();
    assert_eq!(team["teammates"][0]["route"]["model"], "gpt-6.2-sol");
    let _ = std::fs::remove_dir_all(&dir);
}

// ---- other-apps-need-a-current-local-token.test.ts (all remaining cases) --------------------------------------------

/// Sends headers only with an unfinished body (`content-length: 9999`), asserting a rejection arrives before any body.
fn headers_only(url: &str, method: &str, token: Option<&str>, extra: &str) -> String {
    let address = url.trim_start_matches("http://").split('/').next().unwrap().to_owned();
    let mut s = TcpStream::connect(address).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let auth = token.map(|t| format!("authorization: Bearer {t}\r\n")).unwrap_or_default();
    write!(
        s,
        "{method} /tools/call HTTP/1.1\r\nhost: 127.0.0.1\r\ncontent-type: application/json\r\ncontent-length: 9999\r\n{auth}{extra}\r\n"
    )
    .unwrap();
    s.flush().unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    out
}

// case: keeps each teammate's own mode off by default, saves the switch, and keeps it across a restart and the server switch
#[test]
fn keeps_each_teammates_own_mode_off_by_default_saves_the_switch_and_keeps_it_across_a_restart_and_the_server_switch()
{
    let dir = temp("own-mode");
    // Only a literal true enables ownMode.
    for odd in [json!("true"), json!(1), Value::Null] {
        std::fs::write(dir.join(SETTINGS_FILE), json!({"enabled": false, "ownMode": odd}).to_string()).unwrap();
        let door = echo_door(&dir);
        assert!(!door.load().settings.own_mode, "{odd}");
    }
    std::fs::remove_file(dir.join(SETTINGS_FILE)).unwrap();
    let door = echo_door(&dir);
    assert!(!door.load().settings.own_mode);
    assert!(door.set_own_mode(true).settings.own_mode);
    door.set_enabled(true);
    door.set_enabled(false);
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(dir.join(SETTINGS_FILE)).unwrap()).unwrap();
    assert_eq!(saved, json!({"enabled": false, "ownMode": true}));
    let again = echo_door(&dir);
    assert!(again.load().settings.own_mode);
    assert!(!again.set_own_mode(false).settings.own_mode);
    let _ = std::fs::remove_dir_all(&dir);
}

// case: is off by default, with no connection file or listening socket
#[test]
fn is_off_by_default_with_no_connection_file_or_listening_socket() {
    let dir = temp("default-off");
    let called = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let c = called.clone();
    let door = Door::with(
        dir.clone(),
        "/Applications/Locust.app/x",
        Arc::new(Mutex::new(Settings::default())),
        Arc::new(move |_, _| {
            c.fetch_add(1, Ordering::SeqCst);
            json!({})
        }),
    );
    let state = door.load();
    assert!(!state.settings.enabled);
    assert!(!state.settings.own_mode);
    assert!(!door.connection_path().exists());
    assert_eq!(called.load(Ordering::SeqCst), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

// case: binds only loopback, rejects no/wrong/stale tokens before an unfinished body, and closes while off
#[test]
fn binds_only_loopback_rejects_no_wrong_stale_tokens_before_an_unfinished_body_and_closes_while_off()
{
    let dir = temp("loopback-headers-only");
    let called = Arc::new(Mutex::new(Vec::<(String, Value)>::new()));
    let c = called.clone();
    let door = Door::with(
        dir.clone(),
        "/Applications/Locust.app/x",
        Arc::new(Mutex::new(Settings::default())),
        Arc::new(move |name, args| {
            c.lock().unwrap().push((name.to_owned(), args.clone()));
            crate::door_tools::ToolResult::ok("hello").to_json()
        }),
    );
    assert!(door.set_enabled(true).settings.enabled);
    let first: Value = serde_json::from_str(&std::fs::read_to_string(door.connection_path()).unwrap()).unwrap();
    let first_url = first["url"].as_str().unwrap().to_owned();
    let first_tok = first["token"].as_str().unwrap().to_owned();
    assert!(first_url.starts_with("http://127.0.0.1:"));

    // Rejects missing and wrong tokens BEFORE an unfinished 9999-byte body.
    assert!(headers_only(&first_url, "POST", None, "").starts_with("HTTP/1.1 401"));
    assert!(headers_only(&first_url, "POST", Some(&"f".repeat(64)), "").starts_with("HTTP/1.1 401"));
    assert!(headers_only(&first_url, "POST", Some("old-token"), "").starts_with("HTTP/1.1 401"));
    assert!(called.lock().unwrap().is_empty());

    // Closes while off: connection refused and connection file removed.
    let off = door.set_enabled(false);
    assert!(!off.settings.enabled && !off.settings.own_mode);
    let addr = first_url.trim_start_matches("http://").split('/').next().unwrap();
    assert!(TcpStream::connect(addr).is_err());
    assert!(!door.connection_path().exists());

    // Re-enabled: fresh token, stale token rejected before unfinished body, valid request succeeds.
    assert!(door.set_enabled(true).settings.enabled);
    let second: Value = serde_json::from_str(&std::fs::read_to_string(door.connection_path()).unwrap()).unwrap();
    let second_url = second["url"].as_str().unwrap().to_owned();
    let second_tok = second["token"].as_str().unwrap().to_owned();
    assert_ne!(second_tok, first_tok);
    assert!(headers_only(&second_url, "POST", Some(&first_tok), "").starts_with("HTTP/1.1 401"));
    assert!(called.lock().unwrap().is_empty());

    let ok = post(&second_url, Some(&second_tok), "", r#"{"name":"list_teammates","arguments":{}}"#);
    assert!(ok.starts_with("HTTP/1.1 200"), "{ok}");
    assert!(ok.contains("hello"));
    assert_eq!(*called.lock().unwrap(), vec![("list_teammates".to_owned(), json!({}))]);
    door.dispose();
    let _ = std::fs::remove_dir_all(&dir);
}

// case: writes no bearer bytes until account-only file permissions succeed, and fails closed if they cannot
#[test]
fn writes_no_bearer_bytes_until_account_only_file_permissions_succeed_and_fails_closed_if_they_cannot()
{
    let dir = temp("protect-fail");
    let protect_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let pc = protect_calls.clone();
    let door = echo_door(&dir).with_protect_file(Arc::new(move |path| {
        pc.fetch_add(1, Ordering::SeqCst);
        let content = std::fs::read_to_string(path).unwrap();
        assert_eq!(content, "", "no bearer bytes may be written before permissions succeed");
        Err("ACL refused".into())
    }));
    let state = door.set_enabled(true);
    assert!(!state.settings.enabled);
    assert_eq!(protect_calls.load(Ordering::SeqCst), 1);
    assert!(!door.connection_path().exists());
    let _ = std::fs::remove_dir_all(&dir);
}

// case: revokes a valid request while its body is still arriving
#[test]
fn revokes_a_valid_request_while_its_body_is_still_arriving() {
    let dir = temp("revoke-mid-body");
    let called = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let c = called.clone();
    let door = Door::with(
        dir.clone(),
        "/Applications/Locust.app/x",
        Arc::new(Mutex::new(Settings::default())),
        Arc::new(move |_, _| {
            c.fetch_add(1, Ordering::SeqCst);
            json!({})
        }),
    );
    assert!(door.set_enabled(true).settings.enabled);
    let conn: Value = serde_json::from_str(&std::fs::read_to_string(door.connection_path()).unwrap()).unwrap();
    let url = conn["url"].as_str().unwrap();
    let tok = conn["token"].as_str().unwrap();
    let addr = url.trim_start_matches("http://").split('/').next().unwrap();

    let body = r#"{"name":"start_conversation","arguments":{}}"#;
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    write!(
        s,
        "POST /tools/call HTTP/1.1\r\nhost: 127.0.0.1\r\nauthorization: Bearer {tok}\r\ncontent-length: {}\r\n\r\n{}",
        body.len(),
        &body[..5]
    )
    .unwrap();
    s.flush().unwrap();
    // Give the handler thread time to read headers and block waiting for the rest of the body.
    std::thread::sleep(Duration::from_millis(60));
    // Revoke the token while the body is still arriving.
    *door.token.lock().unwrap() = None;
    write!(s, "{}", &body[5..]).unwrap();
    s.flush().unwrap();
    let mut out = String::new();
    let _ = s.read_to_string(&mut out);
    assert!(out.starts_with("HTTP/1.1 401"), "{out}");
    assert_eq!(called.load(Ordering::SeqCst), 0);
    door.dispose();
    let _ = std::fs::remove_dir_all(&dir);
}

// case: refuses browser origins and non-tool routes before reading their bodies
#[test]
fn refuses_browser_origins_and_non_tool_routes_before_reading_their_bodies() {
    let dir = temp("origin-route");
    let called = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let c = called.clone();
    let door = Door::with(
        dir.clone(),
        "/Applications/Locust.app/x",
        Arc::new(Mutex::new(Settings::default())),
        Arc::new(move |_, _| {
            c.fetch_add(1, Ordering::SeqCst);
            json!({})
        }),
    );
    assert!(door.set_enabled(true).settings.enabled);
    let conn: Value = serde_json::from_str(&std::fs::read_to_string(door.connection_path()).unwrap()).unwrap();
    let url = conn["url"].as_str().unwrap();
    let tok = conn["token"].as_str().unwrap();

    assert!(headers_only(url, "POST", Some(tok), "origin: https://example.test\r\n").starts_with("HTTP/1.1 403"));
    assert!(headers_only(url, "GET", Some(tok), "").starts_with("HTTP/1.1 403"));
    assert_eq!(called.load(Ordering::SeqCst), 0);
    door.dispose();
    let _ = std::fs::remove_dir_all(&dir);
}

// case: keeps setup text token-free and safely quotes spaces, backslashes and PowerShell expansion
#[test]
fn keeps_setup_text_token_free_and_safely_quotes_spaces_backslashes_and_powershell_expansion() {
    let (claude, codex, openclaw) = super::setup(
        "/Applications/Locust.app/Contents/MacOS/Locust",
        "/Users/ian/Library/Application Support/Locust/it's $HOME`x`\\conn.json",
    );
    assert_eq!(
        claude,
        "claude mcp add --transport stdio --scope user locust-native -- '/Applications/Locust.app/Contents/MacOS/Locust' --door-bridge '/Users/ian/Library/Application Support/Locust/it'\\''s $HOME`x`\\conn.json'"
    );
    assert!(codex.contains("[mcp_servers.locust_native]"));
    assert!(codex.contains("\\\\conn.json"));
    assert_eq!(
        openclaw,
        "openclaw mcp add locust-native --command '/Applications/Locust.app/Contents/MacOS/Locust' --arg '--door-bridge' --arg '/Users/ian/Library/Application Support/Locust/it'\\''s $HOME`x`\\conn.json'"
    );
    for line in [&claude, &codex, &openclaw] {
        assert!(!line.contains("token"));
    }
}

// case: remembers enabling across app restarts but never reuses the old token, and disposes without reopening
#[test]
fn remembers_enabling_across_app_restarts_but_never_reuses_the_old_token_and_disposes_without_reopening()
{
    let dir = temp("restart-dispose");
    let first = echo_door(&dir);
    assert!(first.set_enabled(true).settings.enabled);
    let old: Value = serde_json::from_str(&std::fs::read_to_string(first.connection_path()).unwrap()).unwrap();
    first.dispose();

    let next = echo_door(&dir);
    assert!(next.load().settings.enabled);
    let fresh: Value = serde_json::from_str(&std::fs::read_to_string(next.connection_path()).unwrap()).unwrap();
    assert_ne!(fresh["token"], old["token"]);

    next.dispose();
    assert!(!next.set_enabled(true).settings.enabled);
    assert!(!next.connection_path().exists());
    let _ = std::fs::remove_dir_all(&dir);
}

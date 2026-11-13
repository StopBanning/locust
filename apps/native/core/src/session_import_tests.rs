use super::*;
use crate::approval::Mode;
use crate::runtime::cursor;
use sha2::{Digest, Sha256};
use std::time::Duration;

struct TempRoot(PathBuf);

impl TempRoot {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "locust-import-{tag}-{}",
            crate::native::uuid_v4().unwrap()
        ));
        fs::create_dir_all(&p).unwrap();
        Self(p)
    }

    fn store(&self) -> Store {
        let electron = self.0.join("electron");
        fs::create_dir_all(&electron).unwrap();
        Store::new(self.0.join("native"), electron).unwrap()
    }

    fn places(&self) -> SessionPlaces {
        SessionPlaces {
            claude_home: self.0.join(".claude"),
            codex_home: self.0.join(".codex"),
            cursor_home: Some(self.0.join(".cursor")),
        }
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write_at(path: &Path, text: &str, ms: i64) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, text).unwrap();
    let time = UNIX_EPOCH + Duration::from_millis(ms as u64);
    File::options().write(true).open(path).unwrap().set_modified(time).unwrap();
}

fn lines(records: &[Value]) -> String {
    let mut out = String::new();
    for r in records {
        out.push_str(&r.to_string());
        out.push('\n');
    }
    out
}

fn claude_person(id: &str, cwd: &str) -> String {
    lines(&[
        json!({ "type": "user", "entrypoint": "cli", "sessionId": id, "cwd": cwd, "timestamp": "2026-09-28T10:00:00.000Z", "message": { "role": "user", "content": "Plan the sales dashboard" } }),
        json!({ "type": "assistant", "sessionId": id, "cwd": cwd, "timestamp": "2026-09-28T10:00:05.000Z", "message": { "role": "assistant", "model": "claude-opus-5-5", "stop_reason": "end_turn", "content": [{ "type": "text", "text": "Here is the plan." }] } }),
        json!({ "type": "user", "entrypoint": "cli", "sessionId": id, "cwd": cwd, "timestamp": "2026-09-28T10:01:00.000Z", "message": { "role": "user", "content": "Now add a chart" } }),
        json!({ "type": "assistant", "sessionId": id, "cwd": cwd, "timestamp": "2026-09-28T10:01:02.000Z", "message": { "role": "assistant", "stop_reason": "tool_use", "content": [{ "type": "tool_use", "name": "Write", "input": { "file_path": "chart.ts", "content": "x".repeat(5000) } }] } }),
        json!({ "type": "user", "sessionId": id, "cwd": cwd, "timestamp": "2026-09-28T10:01:03.000Z", "message": { "role": "user", "content": [{ "type": "tool_result", "content": "ok" }] } }),
        json!({ "type": "assistant", "sessionId": id, "cwd": cwd, "timestamp": "2026-09-28T10:01:09.000Z", "message": { "role": "assistant", "stop_reason": "end_turn", "content": [{ "type": "text", "text": "Chart added." }] } }),
        json!({ "type": "ai-title", "sessionId": id, "aiTitle": "Sales dashboard plan" }),
    ])
}

#[test]
fn lists_claude_and_codex_sessions_newest_first_excluding_locust_subagent_and_old() {
    let tmp = TempRoot::new("list");
    let places = tmp.places();
    let now_ms = parse_iso_ms("2026-09-29T20:00:00.000Z").unwrap();

    let shop = tmp.0.join("shop");
    let api = tmp.0.join("api");
    fs::create_dir_all(&shop).unwrap();
    fs::create_dir_all(&api).unwrap();
    let shop_s = shop.to_string_lossy().into_owned();
    let api_s = api.to_string_lossy().into_owned();

    let project = places.claude_home.join("projects").join("C--work-shop");
    write_at(
        &project.join("aaaaaaaa-0000-4000-8000-000000000001.jsonl"),
        &claude_person("aaaaaaaa-0000-4000-8000-000000000001", &shop_s),
        parse_iso_ms("2026-09-28T10:02:00.000Z").unwrap(),
    );
    // Locust's own run: sdk-cli
    write_at(
        &project.join("bbbbbbbb-0000-4000-8000-000000000002.jsonl"),
        &lines(&[json!({
            "type": "user",
            "entrypoint": "sdk-cli",
            "sessionId": "bbbbbbbb-0000-4000-8000-000000000002",
            "cwd": shop_s,
            "timestamp": "2026-09-28T11:00:00.000Z",
            "message": { "content": "Locust asked this" }
        })]),
        parse_iso_ms("2026-09-28T11:00:00.000Z").unwrap(),
    );
    // Older than 30 days
    write_at(
        &project.join("cccccccc-0000-4000-8000-000000000003.jsonl"),
        &claude_person("cccccccc-0000-4000-8000-000000000003", &shop_s),
        parse_iso_ms("2026-08-01T10:00:00.000Z").unwrap(),
    );

    let day = places.codex_home.join("sessions").join("2026").join("09").join("29");
    write_at(
        &day.join("rollout-2026-09-29T09-00-00-dddddddd-0000-4000-8000-000000000004.jsonl"),
        &lines(&[
            json!({ "type": "session_meta", "timestamp": "2026-09-29T09:00:00.000Z", "payload": { "id": "dddddddd-0000-4000-8000-000000000004", "cwd": api_s, "originator": "codex_cli_rs", "source": "cli" } }),
            json!({ "type": "response_item", "timestamp": "2026-09-29T09:00:01.000Z", "payload": { "type": "message", "role": "user", "content": [{ "type": "input_text", "text": "Fix the login bug" }] } }),
        ]),
        parse_iso_ms("2026-09-29T09:05:00.000Z").unwrap(),
    );
    write_at(
        &day.join("rollout-2026-09-29T10-00-00-eeeeeeee-0000-4000-8000-000000000005.jsonl"),
        &lines(&[
            json!({ "type": "session_meta", "timestamp": "2026-09-29T10:00:00.000Z", "payload": { "id": "eeeeeeee-0000-4000-8000-000000000005", "cwd": api_s, "originator": "locust_desktop", "source": "exec" } }),
        ]),
        parse_iso_ms("2026-09-29T10:05:00.000Z").unwrap(),
    );
    write_at(
        &places.codex_home.join("session_index.jsonl"),
        &lines(&[json!({ "id": "dddddddd-0000-4000-8000-000000000004", "thread_name": "Login bug", "updated_at": "2026-09-29T09:05:00Z" })]),
        now_ms,
    );

    let listed = list_importable_sessions(
        &places,
        ListOptions {
            now_ms: Some(now_ms),
            ..Default::default()
        },
    );
    let summary: Vec<(&str, &str, &str)> = listed
        .iter()
        .map(|s| (s.runtime.as_str(), s.title.as_str(), s.cwd.as_str()))
        .collect();
    assert_eq!(
        summary,
        vec![
            ("codex", "Login bug", api_s.as_str()),
            ("claude", "Sales dashboard plan", shop_s.as_str()),
        ]
    );

    let mut skip = HashSet::new();
    skip.insert("dddddddd-0000-4000-8000-000000000004".to_owned());
    let filtered = list_importable_sessions(
        &places,
        ListOptions {
            now_ms: Some(now_ms),
            skip,
            ..Default::default()
        },
    );
    assert_eq!(filtered.len(), 1);
}

#[test]
fn imports_claude_session_into_resumable_turns_and_refuses_duplicate() {
    let tmp = TempRoot::new("claude-import");
    let store = tmp.store();
    let places = tmp.places();
    let now_ms = parse_iso_ms("2026-09-29T20:00:00.000Z").unwrap();

    let work = tmp.0.join("shop");
    fs::create_dir_all(&work).unwrap();
    let work_s = work.to_string_lossy().into_owned();
    let session_id = "aaaaaaaa-0000-4000-8000-000000000001";
    let file = places
        .claude_home
        .join("projects")
        .join("C--work-shop")
        .join(format!("{session_id}.jsonl"));
    write_at(&file, &claude_person(session_id, &work_s), now_ms);

    let session = ImportableSession {
        runtime: ImportRuntime::Claude,
        session_id: session_id.into(),
        title: "Sales dashboard plan".into(),
        cwd: work_s.clone(),
        folder_name: "shop".into(),
        updated_at: "2026-09-29T20:00:00.000Z".into(),
        bytes: 0,
        open_now: false,
        folder_missing: false,
    };

    let mut seq = 0usize;
    let res = import_session_with_id(&store, &places, &session, None, &mut || {
        seq += 1;
        Ok(seq.to_string())
    })
    .unwrap();
    assert_eq!(res.turns, 2);
    assert_eq!(res.skipped, 0);

    let turn1 = fs::read_to_string(store.ledger_dir().unwrap().join("mission_1.jsonl")).unwrap();
    let turn2 = fs::read_to_string(store.ledger_dir().unwrap().join("mission_3.jsonl")).unwrap();
    let meta1: Value = serde_json::from_str(turn1.lines().next().unwrap()).unwrap();
    let meta2: Value = serde_json::from_str(turn2.lines().next().unwrap()).unwrap();

    assert_eq!(meta1["metadata"]["prompt"], "Plan the sales dashboard");
    assert_eq!(meta2["metadata"]["prompt"], "Now add a chart");
    assert!(meta1["metadata"]["continuesFrom"].is_null());
    assert_eq!(meta2["metadata"]["continuesFrom"]["missionId"], "mission_1");
    assert_eq!(meta2["metadata"]["continuesFrom"]["runtimeThreadId"], session_id);
    assert_eq!(meta1["metadata"]["runtime"], "claude");
    assert_eq!(meta1["metadata"]["workspaceId"], people::workspace_id(&work));

    // Only the words: the second answer is after the tool, never the tool's input.
    assert!(turn2.contains("Chart added."));
    assert!(!turn2.contains("xxxxxxxxxx"));
    assert!(turn1.contains(&format!("\"runtimeThreadId\":\"{session_id}\"")));

    // Folder was learned in native store.
    assert_eq!(
        folders::path_of(&store, &people::workspace_id(&work))
            .unwrap()
            .as_deref(),
        Some(work_s.as_str())
    );

    // Watermark saved; importing again is refused.
    let imports = store.seeded_json("terminal-imports.json", json!({})).unwrap();
    assert_eq!(imports[session_id]["through"], "2026-09-28T10:01:09.000Z");

    let dup = import_session(&store, &places, &session, None);
    assert_eq!(dup.unwrap_err(), "That conversation is already in Locust.");

    // Electron folder was never written to.
    assert!(!store.electron().join("terminal-imports.json").exists());
    assert!(!store.electron().join("mission-ledger").exists());
}

// ---------------------------------------------------------------------------
// OpenCode & Cursor tests (from past-opencode-and-cursor-chats-can-be-imported.test.ts)
// ---------------------------------------------------------------------------

const CURSOR_ID: &str = "026f3a83-d3a2-4e27-865f-0f4d492f7d0a";
const OC_ID: &str = "ses_fixture123";

fn opencode_export(id: &str, now_ms: i64) -> String {
    json!({
        "info": { "id": id },
        "messages": [
            {
                "info": { "id": "u1", "sessionID": id, "role": "user", "time": { "created": now_ms - 10_000 } },
                "parts": [
                    { "type": "text", "text": "Plan the chart" },
                    { "type": "text", "text": "injected", "synthetic": true }
                ]
            },
            {
                "info": { "sessionID": id, "parentID": "u1", "role": "assistant", "time": { "completed": now_ms - 7_000 }, "finish": "tool-calls" },
                "parts": [
                    { "type": "tool", "text": "secret tool output" },
                    { "type": "reasoning", "text": "private reasoning" }
                ]
            },
            {
                "info": { "sessionID": id, "parentID": "u1", "role": "assistant", "time": { "completed": now_ms - 5_000 }, "finish": "stop", "providerID": "provider", "modelID": "model" },
                "parts": [{ "type": "text", "text": "The chart is planned." }]
            },
            {
                "info": { "id": "u2", "sessionID": id, "role": "user", "time": { "created": now_ms - 3_000 } },
                "parts": [{ "type": "text", "text": "Unfinished prompt" }]
            },
            {
                "info": { "sessionID": "ses_other", "parentID": "u2", "role": "assistant", "time": { "completed": now_ms }, "finish": "stop" },
                "parts": [{ "type": "text", "text": "Foreign answer" }]
            }
        ]
    })
    .to_string()
}

fn sqlite_exec(db_path: &Path, sql: &str) {
    let out = Command::new("sqlite3")
        .arg(db_path)
        .arg(sql)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "sqlite3 failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn cursor_fixture(home: &Path, cwd: &str, now_ms: i64) -> PathBuf {
    let folder = home.join("chats").join("0".repeat(32)).join(CURSOR_ID);
    fs::create_dir_all(&folder).unwrap();
    fs::write(
        folder.join("meta.json"),
        json!({
            "schemaVersion": 1,
            "hasConversation": true,
            "cwd": cwd,
            "createdAtMs": now_ms - 10_000,
            "updatedAtMs": now_ms
        })
        .to_string(),
    )
    .unwrap();

    let messages = vec![
        json!({ "role": "system", "content": "system instructions" }),
        json!({ "role": "user", "content": "<user_info>injected environment</user_info>" }),
        json!({ "role": "user", "content": [{ "type": "text", "text": "<timestamp>today</timestamp>\n<user_query>Plan the chart</user_query>" }] }),
        json!({ "role": "assistant", "content": [{ "type": "reasoning", "text": "private reasoning" }, { "type": "tool-call", "toolName": "Read" }] }),
        json!({ "role": "tool", "content": [{ "type": "tool-result", "text": "secret tool output" }] }),
        json!({ "role": "assistant", "content": [{ "type": "text", "text": "The chart is planned." }] }),
        json!({ "role": "user", "content": [{ "type": "text", "text": "<user_query>Unfinished prompt</user_query>" }] }),
        json!({ "role": "assistant", "content": [{ "type": "text", "text": "Still working" }, { "type": "tool-call" }] }),
    ];

    let db_path = folder.join("store.db");
    let mut sql = String::from(
        "CREATE TABLE blobs (id TEXT PRIMARY KEY, data BLOB); CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT);",
    );
    let mut index_bytes = Vec::new();
    for msg in &messages {
        let raw = msg.to_string().into_bytes();
        let hash = Sha256::digest(&raw);
        let hex_id = encode_hex(&hash);
        let hex_data = encode_hex(&raw);
        sql.push_str(&format!(
            "INSERT INTO blobs VALUES ('{hex_id}', X'{hex_data}');"
        ));
        index_bytes.push(10);
        index_bytes.push(32);
        index_bytes.extend_from_slice(&hash);
    }
    let index_hash = Sha256::digest(&index_bytes);
    let index_id = encode_hex(&index_hash);
    let index_hex = encode_hex(&index_bytes);
    sql.push_str(&format!(
        "INSERT INTO blobs VALUES ('{index_id}', X'{index_hex}');"
    ));
    let meta_json = json!({
        "agentId": CURSOR_ID,
        "name": "Chart planning",
        "latestRootBlobId": index_id
    })
    .to_string();
    let meta_hex = encode_hex(meta_json.as_bytes());
    sql.push_str(&format!("INSERT INTO meta VALUES ('0', '{meta_hex}');"));
    sqlite_exec(&db_path, &sql);
    folder
}

#[test]
fn reads_opencode_list_and_export_without_tools_reasoning_foreign_or_unfinished() {
    let now_ms = parse_iso_ms("2026-10-19T12:00:00Z").unwrap();
    let list_json = json!([
        { "id": OC_ID, "title": "Chart planning", "directory": "C:/chart", "updated": now_ms },
        { "id": "ses_child", "parentID": OC_ID, "title": "Child", "directory": "C:/chart", "updated": now_ms }
    ])
    .to_string();
    let listed = open_code_sessions(&list_json, Some(now_ms)).unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].runtime, ImportRuntime::Opencode);
    assert_eq!(listed[0].session_id, OC_ID);
    assert_eq!(listed[0].cwd, "C:/chart");
    assert_eq!(listed[0].title, "Chart planning");

    let ex = open_code_exchanges(&opencode_export(OC_ID, now_ms), OC_ID).unwrap();
    assert_eq!(ex.len(), 1);
    assert_eq!(ex[0].prompt, "Plan the chart");
    assert_eq!(ex[0].answer.as_deref(), Some("The chart is planned."));
    assert_eq!(ex[0].started, iso_from_ms(now_ms - 10_000).unwrap());
    assert_eq!(ex[0].finished, iso_from_ms(now_ms - 5_000).unwrap());
    assert_eq!(ex[0].model.as_deref(), Some("provider/model"));

    assert!(
        open_code_exchanges(&opencode_export("ses_foreign", now_ms), OC_ID)
            .unwrap_err()
            .contains("different")
    );
    assert!(open_code_sessions("{}", Some(now_ms)).unwrap_err().contains("session list"));
}

#[test]
fn opencode_reader_uses_documented_commands_and_refuses_failures_and_invalid_ids() {
    use std::cell::RefCell;
    let now_ms = parse_iso_ms("2026-10-19T12:00:00Z").unwrap();

    struct MockRunner {
        calls: RefCell<Vec<Vec<String>>>,
        exit_ok: bool,
        installed: bool,
        now_ms: i64,
    }
    impl OpenCodeRunner for MockRunner {
        fn run_opencode(&self, args: &[&str]) -> Result<Option<String>, String> {
            if !self.installed {
                return Ok(None);
            }
            self.calls
                .borrow_mut()
                .push(args.iter().map(|s| (*s).to_owned()).collect());
            if !self.exit_ok {
                return Err("OpenCode could not read its sessions. Nothing was imported.".into());
            }
            if args.first() == Some(&"export") {
                Ok(Some(opencode_export(OC_ID, self.now_ms)))
            } else {
                Ok(Some("[]".into()))
            }
        }
    }

    let runner = MockRunner {
        calls: RefCell::new(Vec::new()),
        exit_ok: true,
        installed: true,
        now_ms,
    };
    let reader = OpenCodeSessionReader::new(runner);
    assert!(reader.list(Some(now_ms)).unwrap().is_empty());
    assert_eq!(reader.exchanges(OC_ID).unwrap().len(), 1);
    assert_eq!(
        *reader.runner.calls.borrow(),
        vec![
            vec!["session", "list", "--format", "json"],
            vec!["export", OC_ID]
        ]
    );
    assert!(reader.exchanges("../bad").unwrap_err().contains("Invalid"));

    let failed = OpenCodeSessionReader::new(MockRunner {
        calls: RefCell::new(Vec::new()),
        exit_ok: false,
        installed: true,
        now_ms,
    });
    assert!(failed.list(Some(now_ms)).unwrap_err().contains("Nothing was imported"));

    let missing = OpenCodeSessionReader::new(MockRunner {
        calls: RefCell::new(Vec::new()),
        exit_ok: true,
        installed: false,
        now_ms,
    });
    assert!(missing.list(Some(now_ms)).unwrap().is_empty());
}

#[test]
fn reads_cursor_metadata_and_ordered_messages_without_changing_store_or_opening_config() {
    let tmp = TempRoot::new("cursor");
    let home = tmp.0.join(".cursor");
    let now_ms = parse_iso_ms("2026-10-19T12:00:00Z").unwrap();
    let cwd = tmp.0.to_string_lossy().into_owned();
    let folder = cursor_fixture(&home, &cwd, now_ms);
    // A directory named cli-config.json ensures any attempt to read it as a file fails.
    fs::create_dir_all(home.join("cli-config.json")).unwrap();

    let snap = |name: &str| {
        let p = folder.join(name);
        let bytes = fs::read(&p).unwrap();
        let mtime = fs::metadata(&p).unwrap().modified().unwrap();
        (bytes, mtime)
    };
    let before_meta = snap("meta.json");
    let before_db = snap("store.db");

    let sessions = list_cursor_sessions(&home, Some(now_ms));
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].runtime, ImportRuntime::Cursor);
    assert_eq!(sessions[0].session_id, CURSOR_ID);
    assert_eq!(sessions[0].cwd, cwd);
    assert_eq!(sessions[0].title, "Chart planning");

    let ex = cursor_exchanges(&home, &sessions[0].session_id, &sessions[0].cwd).unwrap();
    assert_eq!(ex.len(), 1);
    assert_eq!(ex[0].prompt, "Plan the chart");
    assert_eq!(ex[0].answer.as_deref(), Some("The chart is planned."));
    assert_eq!(ex[0].started, iso_from_ms(now_ms - 10_000).unwrap());
    assert_eq!(ex[0].finished, iso_from_ms(now_ms).unwrap());

    assert_eq!(snap("meta.json"), before_meta);
    assert_eq!(snap("store.db"), before_db);

    assert!(
        cursor_exchanges(&home, CURSOR_ID, "another folder")
            .unwrap_err()
            .contains("could not be found")
    );
    assert!(cursor_message_ids(&[10, 32, 1]).unwrap_err().contains("Truncated"));
    assert!(cursor_message_ids(&[10, 1, 1]).unwrap_err().contains("Unknown"));
}

#[test]
fn lists_huge_cursor_stores_from_metadata_and_keeps_missing_folders_visible() {
    let tmp = TempRoot::new("cursor-huge");
    let home = tmp.0.join(".cursor");
    let now_ms = parse_iso_ms("2026-10-19T12:00:00Z").unwrap();
    let gone = tmp.0.join("gone");
    let folder = cursor_fixture(&home, &gone.to_string_lossy(), now_ms);

    sqlite_exec(
        &folder.join("store.db"),
        "INSERT INTO blobs VALUES ('unused-tool-output', zeroblob(33554432));",
    );

    let sessions = list_cursor_sessions(&home, Some(now_ms));
    assert_eq!(sessions.len(), 1);
    assert!(sessions[0].bytes > 32 * 1024 * 1024);

    let places = tmp.places();
    let listed = list_importable_sessions(
        &places,
        ListOptions {
            now_ms: Some(now_ms),
            other_sessions: sessions.clone(),
            ..Default::default()
        },
    );
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].runtime, ImportRuntime::Cursor);
    assert!(listed[0].folder_missing);

    let mut skip = HashSet::new();
    skip.insert(CURSOR_ID.to_owned());
    assert!(
        list_importable_sessions(
            &places,
            ListOptions {
                now_ms: Some(now_ms),
                skip,
                other_sessions: sessions.clone(),
                ..Default::default()
            }
        )
        .is_empty()
    );
    assert!(
        list_importable_sessions(
            &places,
            ListOptions {
                now_ms: Some(now_ms + 31 * 86_400_000),
                other_sessions: sessions,
                ..Default::default()
            }
        )
        .is_empty()
    );
}

#[test]
fn imports_cursor_and_opencode_as_resumable_on_account_default_and_refuses_duplicates() {
    let now_ms = parse_iso_ms("2026-10-19T12:00:00Z").unwrap();

    for runtime in [ImportRuntime::Cursor, ImportRuntime::Opencode] {
        let tmp = TempRoot::new(runtime.as_str());
        let store = tmp.store();
        let places = tmp.places();
        let cwd = tmp.0.to_string_lossy().into_owned();
        let cursor_home = places.cursor_home.as_ref().unwrap();
        let _folder = cursor_fixture(cursor_home, &cwd, now_ms);

        let session = match runtime {
            ImportRuntime::Cursor => list_cursor_sessions(cursor_home, Some(now_ms))
                .into_iter()
                .next()
                .unwrap(),
            ImportRuntime::Opencode => open_code_sessions(
                &json!([{ "id": OC_ID, "title": "Chart planning", "directory": cwd, "updated": now_ms }]).to_string(),
                Some(now_ms),
            )
            .unwrap()
            .into_iter()
            .next()
            .unwrap(),
            _ => unreachable!(),
        };

        let oc_reader = |_: &str| open_code_exchanges(&opencode_export(OC_ID, now_ms), OC_ID);

        let mut missing = session.clone();
        missing.folder_missing = true;
        let err = import_session(&store, &places, &missing, Some(&oc_reader)).unwrap_err();
        assert!(err.contains("folder"));

        let res = import_session(&store, &places, &session, Some(&oc_reader)).unwrap();
        assert_eq!(res.turns, 1);

        let ledger = fs::read_to_string(
            store
                .ledger_dir()
                .unwrap()
                .join(format!("{}.jsonl", res.mission_id)),
        )
        .unwrap();
        let created: Value = serde_json::from_str(ledger.lines().next().unwrap()).unwrap();
        assert_eq!(created["metadata"]["runtime"], runtime.as_str());
        assert_eq!(created["metadata"]["model"], "account-default");
        assert_eq!(created["metadata"]["requestedRouteId"], runtime.as_str());
        assert_eq!(
            created["metadata"]["resolvedRouteId"],
            format!("{}-account:default", runtime.as_str())
        );
        assert_eq!(created["metadata"]["prompt"], "Plan the chart");
        assert!(ledger.contains(&format!("\"runtimeThreadId\":\"{}\"", session.session_id)));
        assert!(ledger.contains("The chart is planned."));

        let dup = import_session(&store, &places, &session, Some(&oc_reader)).unwrap_err();
        assert!(dup.contains("already"));

        if runtime == ImportRuntime::Cursor {
            let args = cursor::print_args(&tmp.0, Mode::Ask, None, Some(CURSOR_ID), None).unwrap();
            assert!(args.contains(&"--resume".to_owned()));
            assert!(args.contains(&CURSOR_ID.to_owned()));
            assert!(!args.contains(&"--model".to_owned()));
        }
    }
}

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::connectors_off::FREE_MODELS_BEST_FIRST;
use crate::ledger;
use crate::native::{hooks, set_open_folder};
use crate::people::{record_owner, save_teammate_id, team};
use crate::store::Store;
use crate::workspace_settings::{self, workspace_id_for};

use super::host::{ScoutHost, ScoutHostDeps, is_free_route, on_diagnostic};
use super::scheduler::FakeScheduler;
use super::{AnswerResult, ScoutDeps, ScoutRoute};

struct HostTemp {
    root: PathBuf,
    work: PathBuf,
    store: Store,
}

impl HostTemp {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "locust_scout_host_{}_{}_{}",
            tag,
            std::process::id(),
            crate::invariants::unique_nanos()
        ));
        let native = root.join("native");
        let electron = root.join("electron");
        let work = root.join("work");
        fs::create_dir_all(native.join("mission-ledger")).unwrap();
        fs::create_dir_all(electron.join("mission-ledger")).unwrap();
        fs::create_dir_all(&work).unwrap();
        let store = Store::new(native, electron).unwrap();
        Self { root, work, store }
    }

    fn seed_workspace(&self) {
        set_open_folder(&self.store, &self.work).unwrap();
    }
}

impl Drop for HostTemp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn is_free_route_matches_opencode_kilo_and_acp_free_routes() {
    assert!(is_free_route(
        "opencode",
        Some("opencode/deepseek-v4.1-flash-free")
    ));
    assert!(is_free_route(
        "opencode",
        Some("opencode/nemotron-3.5-lightning-free")
    ));
    assert!(!is_free_route("opencode", Some("opencode/x-free-pro")));
    assert!(!is_free_route("opencode", None));

    assert!(is_free_route("kilo", Some("kilo/kilo-auto/free")));
    assert!(is_free_route("kilo", Some("openrouter/qwen:free")));
    assert!(!is_free_route("kilo", Some("kilo/claude-sonnet")));

    assert!(is_free_route(
        "acp",
        Some("acp_1::opencode/glm-5.3-flash-free")
    ));
    assert!(!is_free_route("acp", Some("acp_1::anthropic/claude-sonnet")));
    assert!(!is_free_route("codex", Some("gpt-5.4-mini-free")));
}

#[test]
fn next_free_model_picks_untried_best_first_and_never_leaves_free_routes() {
    let t = HostTemp::new("free_fallback");
    let deps = ScoutHostDeps::new(t.store.clone());

    let paid_id = deps
        .make_teammate(&ScoutRoute {
            runtime: "codex".into(),
            model: "gpt-5.4-mini".into(),
            account: None,
        })
        .unwrap();
    assert_eq!(deps.next_free_model(&paid_id, &[]), None);

    // Swap teammate to free OpenCode model directly via save_teammate_id on a fresh store.
    let t2 = HostTemp::new("free_fallback_2");
    let deps2 = ScoutHostDeps::new(t2.store.clone());
    let first_free = FREE_MODELS_BEST_FIRST[0];
    let second_free = FREE_MODELS_BEST_FIRST[1];
    let third_free = FREE_MODELS_BEST_FIRST[2];
    let free_id = deps2
        .make_teammate(&ScoutRoute {
            runtime: "opencode".into(),
            model: first_free.into(),
            account: None,
        })
        .unwrap();
    assert_eq!(
        deps2.next_free_model(&free_id, &[]),
        Some(second_free.to_owned())
    );
    assert_eq!(
        deps2.next_free_model(&free_id, &[second_free.to_owned()]),
        Some(third_free.to_owned())
    );
    let all: Vec<String> = FREE_MODELS_BEST_FIRST
        .iter()
        .map(|s| (*s).to_owned())
        .collect();
    assert_eq!(deps2.next_free_model(&free_id, &all), None);
}

#[test]
fn make_teammate_and_teammate_exists_create_and_reuse_keeps_watch_teammate() {
    let t = HostTemp::new("make_mate");
    let deps = ScoutHostDeps::new(t.store.clone());

    let first_id = deps
        .make_teammate(&ScoutRoute {
            runtime: "codex".into(),
            model: "account-default".into(),
            account: None,
        })
        .unwrap();
    assert!(deps.teammate_exists(&first_id).unwrap());

    // Calling make_teammate again reuses the existing Custom / Keeps watch teammate.
    let reused_id = deps
        .make_teammate(&ScoutRoute {
            runtime: "claude".into(),
            model: "claude-sonnet-4-6".into(),
            account: None,
        })
        .unwrap();
    assert_eq!(reused_id, first_id);

    let roster = team(&t.store).unwrap();
    assert_eq!(roster["teammates"].as_array().unwrap().len(), 1);
    assert_eq!(roster["teammates"][0]["name"], "Scout");
    assert_eq!(roster["teammates"][0]["hue"], "teal");
    assert_eq!(roster["teammates"][0]["role"], "Custom");
    assert_eq!(roster["teammates"][0]["roleTitle"], "Keeps watch");
    assert_eq!(roster["teammates"][0]["route"]["mode"], "ask");
    assert!(!deps.teammate_exists("tm_missing").unwrap());
}

#[test]
fn make_teammate_skips_taken_names_until_scout_2() {
    let t = HostTemp::new("taken_name");
    save_teammate_id(
        &t.store,
        json!({
            "name": "Scout",
            "hue": "lime",
            "role": "Custom",
            "roleTitle": "Other work",
            "route": {"runtime": "codex", "model": "account-default", "mode": "ask"}
        }),
    )
    .unwrap();
    let deps = ScoutHostDeps::new(t.store.clone());
    let id = deps
        .make_teammate(&ScoutRoute {
            runtime: "codex".into(),
            model: "account-default".into(),
            account: None,
        })
        .unwrap();
    let mate = deps
        .team()
        .unwrap()
        .into_iter()
        .find(|m| m.teammate_id == id)
        .unwrap();
    assert_eq!(mate.name, "Scout 2");
}

#[test]
fn brief_facts_counts_newest_80_excluding_own_looks_and_answer_of_reads_transcript() {
    let t = HostTemp::new("brief_facts");
    let deps = ScoutHostDeps::new(t.store.clone());
    let dir = t.store.ledger_dir().unwrap();

    // 83 non-own missions: 80 completed, 3 failed (the oldest 3 or mix), plus 2 own looks.
    for i in 0..83 {
        let id = format!("m_{i:03}");
        let end_type = if i < 3 {
            "run.failed"
        } else {
            "run.completed"
        };
        let payload = if end_type == "run.failed" {
            json!({"message": "Boom"})
        } else {
            json!({"usage": {}})
        };
        let content = format!(
            "{}\n{}\n",
            json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":id,"prompt":"Task","runtime":"codex","model":"gpt-5.4-mini","createdAt":"2026-01-01T00:00:00Z"}}),
            json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":1,"type":end_type,"payload":payload}})
        );
        fs::write(dir.join(format!("{id}.jsonl")), content).unwrap();
    }
    let own_content = format!(
        "{}\n{}\n{}\n",
        json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":"look_1","prompt":"Look","runtime":"codex","model":"gpt-5.4-mini","createdAt":"2026-01-01T00:00:00Z","startedBy":{"kind":"scout"}}}),
        json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":1,"type":"message.delta","payload":{"itemId":"a1","text":"ITEM 1\nTAKE: Ok."}}}),
        json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":2,"type":"run.completed","payload":{}}})
    );
    fs::write(dir.join("look_1.jsonl"), own_content).unwrap();

    // Sleep 20ms isn't needed if since is in the past; use year 2020.
    let runs = deps
        .brief_facts("2020-01-01T00:00:00.000Z", &["look_1".to_owned()])
        .unwrap();
    assert!(runs.more);
    assert_eq!(runs.finished + runs.failed, 80);

    assert_eq!(
        deps.answer_of("look_1"),
        AnswerResult::Text("ITEM 1\nTAKE: Ok.".into())
    );
    assert_eq!(
        deps.answer_of("m_000"),
        AnswerResult::Failed("Scout’s model did not answer: Boom".into())
    );
    assert_eq!(
        deps.answer_of("missing_id"),
        AnswerResult::Failed("Scout’s look left no record to read.".into())
    );

    // Check failed_turn + continues_from + owner attribution.
    let tm = save_teammate_id(
        &t.store,
        json!({
            "name": "Wren",
            "role": "Custom",
            "roleTitle": "Builder",
            "route": {"runtime": "codex", "model": "gpt-5.4-mini", "mode": "ask"}
        }),
    )
    .unwrap();
    let child = format!(
        "{}\n{}\n",
        json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":"m_child","prompt":"Fix build","runtime":"codex","model":"gpt-5.4-mini","createdAt":"2026-01-01T00:00:01Z","continuesFrom":{"missionId":"m_000"}}}),
        json!({"schemaVersion":27,"recordType":"mission.event","event":{"sequence":1,"type":"run.failed","payload":{"message":"Exit 1"}}})
    );
    fs::write(dir.join("m_child.jsonl"), child).unwrap();
    record_owner(&t.store, "m_child", &tm).unwrap();

    let ft = deps.failed_turn("m_child").unwrap().unwrap();
    assert_eq!(ft.asked, "Fix build");
    assert_eq!(ft.teammate_id.as_deref(), Some(tm.as_str()));
    assert_eq!(ft.name.as_deref(), Some("Wren"));
    assert_eq!(ft.why.as_deref(), Some("Exit 1"));
    assert_eq!(deps.continues_from("m_child").unwrap(), Some("m_000".into()));
}

#[test]
fn run_check_and_has_check_support_run_configured_check_command_and_clip_tail() {
    let t = HostTemp::new("check_cmd");
    t.seed_workspace();
    let deps = ScoutHostDeps::new(t.store.clone());
    assert!(!deps.has_check_support());
    assert_eq!(deps.run_check(), None);

    let ws_id = workspace_id_for(&t.work);
    let mut cmds = BTreeMap::new();
    // Emit 30 lines including one 350-char line, then fail.
    cmds.insert(
        ws_id,
        "python3 -c \"print('x'*350); [print(f'line-{i}') for i in range(30)]; import sys; sys.exit(1)\""
            .to_owned(),
    );
    let mut settings = workspace_settings::load(&t.store).unwrap();
    settings.check_commands = cmds;
    workspace_settings::save(&t.store, &settings).unwrap();

    assert!(deps.has_check_support());
    let ran = deps.run_check().unwrap();
    assert!(!ran.passed);
    assert_eq!(ran.lines.len(), 25);
    assert_eq!(ran.lines[0], "line-5");
    assert_eq!(ran.lines[24], "line-29");
}

#[test]
fn host_end_to_end_turn_on_runs_sealed_look_and_populates_cards_and_notes() {
    let t = HostTemp::new("e2e_sealed");
    t.seed_workspace();

    // Seed a teammate ("Wren") so HAND TO has a roster peer, plus a failing check command so Scout has a fact.
    save_teammate_id(
        &t.store,
        json!({
            "name": "Wren",
            "role": "Custom",
            "roleTitle": "Fixes tests",
            "route": {"runtime": "codex", "model": "gpt-5.4-mini", "mode": "ask"}
        }),
    )
    .unwrap();
    let ws_id = workspace_id_for(&t.work);
    let mut cmds = BTreeMap::new();
    cmds.insert(
        ws_id,
        "python3 -c \"print('check fail 42'); import sys; sys.exit(1)\"".to_owned(),
    );
    let mut settings = workspace_settings::load(&t.store).unwrap();
    settings.check_commands = cmds;
    workspace_settings::save(&t.store, &settings).unwrap();

    // Make workspace a git repo with an initial commit so folder_mark returns Some(...).
    let git_init = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&t.work)
        .status()
        .unwrap();
    assert!(git_init.success());
    fs::write(t.work.join("README.md"), "hello\n").unwrap();
    let git_add = std::process::Command::new("git")
        .args(["add", "README.md"])
        .current_dir(&t.work)
        .status()
        .unwrap();
    assert!(git_add.success());
    let git_commit = std::process::Command::new("git")
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-q",
            "-m",
            "init",
        ])
        .current_dir(&t.work)
        .status()
        .unwrap();
    assert!(git_commit.success());

    // Wrapper around m1-app-server.py that inspects thread/start + turn/start for sealing and replies with ITEM 1 / NOTE.
    let inspect_log = t.root.join("captured_rpc.jsonl");
    let wrapper = t.root.join("mock_app_server.py");
    let py = r#"import json, sys
log_path = sys.argv[1]
answer = (
    "ITEM 1\n"
    "TAKE: Check failed with check fail 42.\n"
    "HAND TO: Wren\n"
    "DRAFT: Fix check fail 42.\n"
    "NOTE: Check fail 42 appeared right after init."
)
def send(v):
    sys.stdout.write(json.dumps(dict(jsonrpc="2.0", **v)) + "\n")
    sys.stdout.flush()
def notify(method, **params):
    send(dict(method=method, params=dict(threadId="th_scout_1", **params)))

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    msg = json.loads(line)
    m = msg.get("method")
    rid = msg.get("id")
    if m == "initialize":
        send(dict(id=rid, result={}))
    elif m == "config/read":
        send(dict(id=rid, result={"config": {"mcp_servers": {"local": {"command": "node"}, "remote": {"url": "https://example.invalid"}}}}))
    elif m == "thread/start":
        with open(log_path, "a", encoding="utf-8") as f:
            f.write(json.dumps({"method": "thread/start", "params": msg.get("params")}) + "\n")
        send(dict(id=rid, result={"thread": {"id": "th_scout_1"}}))
    elif m == "turn/start":
        with open(log_path, "a", encoding="utf-8") as f:
            f.write(json.dumps({"method": "turn/start", "params": msg.get("params")}) + "\n")
        tid = "tu_scout_1"
        send(dict(id=rid, result={"turn": {"id": tid}}))
        notify("turn/started", turn={"id": tid})
        notify("item/agentMessage/delta", itemId="it_1", operation="replace", delta=answer)
        notify("item/completed", item={"id": "it_1", "type": "agentMessage", "text": answer})
        notify("turn/completed", turn={"id": tid, "status": "completed", "usage": {"input_tokens": 10, "output_tokens": 20}})
"#;
    fs::write(&wrapper, py).unwrap();

    let _g = hooks::install(
        t.store.clone(),
        Some((
            PathBuf::from("python3"),
            vec![
                wrapper.to_string_lossy().into_owned(),
                inspect_log.to_string_lossy().into_owned(),
            ],
        )),
    );

    let sched = FakeScheduler::new(1_760_000_000_000);
    let host = ScoutHost::start_with_scheduler(t.store.clone(), sched);
    // Avoid shelling out to real `gh` during the end-to-end test.
    host.deps.set_gh_hook(Some(Arc::new(|_args, _cwd| {
        Ok("[]".to_owned())
    })));

    host.scout.set(&super::types::ScoutChange {
        watches: Some(super::types::ScoutWatchesPatch {
            check: Some(true),
            ..Default::default()
        }),
        ..Default::default()
    });
    let outcome = host.scout.turn_on(Some(&ScoutRoute {
        runtime: "codex".into(),
        model: "gpt-5.4-mini".into(),
        account: None,
    }));
    let super::TurnOnOutcome::Ok(_) = outcome else {
        panic!("expected TurnOnOutcome::Ok, got {outcome:?}");
    };

    // Wait up to 5 seconds for the sealed turn thread to complete and trigger `on_run_ended`.
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let st = host.scout.state();
        if st.cards.iter().any(|c| c.take.is_some()) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for Scout look completion; state={st:?}"
        );
        std::thread::sleep(Duration::from_millis(25));
    }

    let st = host.scout.state();
    assert_eq!(st.cards.len(), 1);
    assert_eq!(
        st.cards[0].take.as_deref(),
        Some("Check failed with check fail 42.")
    );
    let draft = st.cards[0].fact.draft.as_ref().unwrap();
    assert_eq!(draft.name, "Wren");
    assert_eq!(draft.message, "Fix check fail 42.");
    assert_eq!(st.notes.len(), 1);
    assert!(
        st.notes[0].ends_with(": Check fail 42 appeared right after init."),
        "expected dated note, got {:?}",
        st.notes
    );

    // 1. Sealed turn never wrote developerInstructions and applied sealed Codex thread config.
    let rpc_lines: Vec<Value> = fs::read_to_string(&inspect_log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let thread_start = rpc_lines
        .iter()
        .find(|v| v["method"] == "thread/start")
        .unwrap();
    assert!(thread_start["params"].get("developerInstructions").is_none());
    assert_eq!(thread_start["params"]["sandbox"], "read-only");
    assert_eq!(thread_start["params"]["approvalPolicy"], "never");
    assert_eq!(
        thread_start["params"]["config"]["features.shell_tool"],
        false
    );
    assert_eq!(
        thread_start["params"]["config"]["web_search"],
        "disabled"
    );

    // 2. Sealed look was written only to native store, never to electron/, and is hidden from list_recent.
    assert!(t.store.root().join("scout.json").is_file());
    assert!(t.store.root().join("scout-looks.json").is_file());
    assert!(!t.store.electron().join("scout.json").exists());
    assert!(!t.store.electron().join("scout-looks.json").exists());

    let recent = ledger::try_list_recent(&t.store.ledger_dir().unwrap(), 20).unwrap();
    assert!(
        recent.is_empty(),
        "Scout's look should be hidden from recent conversations: {recent:?}"
    );
}

#[test]
fn on_diagnostic_triggers_provider_refused_only_for_matching_codes() {
    let t = HostTemp::new("diag_retry");
    t.seed_workspace();
    let sched = FakeScheduler::new(1_760_000_000_000);
    let host = ScoutHost::start_with_scheduler(t.store.clone(), sched);

    // Unrelated diagnostic code should do nothing cleanly without panicking.
    on_diagnostic(
        &t.store,
        "look_x",
        &json!({"code": "host.folder_moved", "message": "moved"}),
    );
    on_diagnostic(
        &t.store,
        "look_x",
        &json!({"code": "opencode.runtime_error", "message": "429 busy"}),
    );
    on_diagnostic(
        &t.store,
        "look_x",
        &json!({"code": "codex.provider_busy.runtime_error", "detail": "overloaded"}),
    );
    assert!(!host.scout.state().settings.on);
}

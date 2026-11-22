//! Electron `apps/desktop/src/main/opencode-2-runs-over-acp-with-its-permissions.test.ts` (9 of 12 applicable cases).
//! Three cases do not apply to the Rust app: V2 cloud start (no cloud runner in Rust), missing injected `acpSpawn`
//! callback (Rust spawns directly via `Command`), and V1 `opencode run` fallback (Rust always compiles in `serve`).
use super::*;
use crate::{
    approval::{Answer, Channel, Decision, Kind, Mode},
    engine::Teammate,
    ledger_write::LedgerWriter,
    native::{self, Origin, Send as NativeSend},
    runtime::opencode_config::{self, FAIL_V2_GUARD},
    store::Store,
};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);
impl Scratch {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "locust-oc2-acp-{tag}-{}-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed),
            crate::invariants::unique_nanos(),
        ));
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
        .join("scripts/fixtures")
        .join(name)
}

fn env_of(c: &Command, key: &str) -> Option<String> {
    c.get_envs()
        .find(|(k, _)| *k == key)
        .and_then(|(_, v)| v.map(|v| v.to_string_lossy().into_owned()))
}

fn read_acp_log(work: &Path) -> Vec<Value> {
    let text = std::fs::read_to_string(work.join("acp-methods.log")).unwrap_or_default();
    text.lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .collect()
}

fn drive(rt: &mut OpenCode, writer: &mut LedgerWriter, channel: &Channel, mission: &str) -> Vec<Event> {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut events = vec![];
    loop {
        assert!(Instant::now() < deadline, "OpenCode fake timed out");
        if let Some(e) = rt.next_event().unwrap() {
            let terminal = matches!(e.kind.as_str(), "run.completed" | "run.failed" | "run.cancelled");
            for a in channel.take_recorded_for(mission) {
                writer.append_approval(&a.to_json()).unwrap();
            }
            writer.append(&e.kind, e.payload.clone(), e.thread.as_deref()).unwrap();
            events.push(e);
            if terminal {
                break;
            }
        }
    }
    events
}

fn make_store(root: &Path) -> Store {
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    std::fs::create_dir_all(store.electron()).unwrap();
    store
}

fn opencode_teammate(work: &Path, mode: &str) -> Teammate {
    Teammate {
        id: "oc_v2".into(),
        name: "Open".into(),
        role: "Custom".into(),
        runtime: Some("opencode".into()),
        model: Some("opencode/zen-free".into()),
        account: None,
        busy: false,
        runs_in: mode.into(),
        folder: Some(work.to_string_lossy().into_owned()),
    }
}

fn wait_for_turn_end(conversation_id: &str) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while native::owns(conversation_id) {
        assert!(Instant::now() < deadline, "turn {conversation_id} did not finish");
        std::thread::sleep(Duration::from_millis(10));
    }
}

// case: starts ACP instead of serve or run, passes the native guard and sets mode and model before its prompt
#[test]
fn native_starts_acp_instead_of_serve_or_run_passes_the_native_guard_and_sets_mode_and_model_before_its_prompt() {
    let t = Scratch::new("start");
    let store = make_store(&t.0);
    let work = t.0.join("work");
    std::fs::create_dir_all(&work).unwrap();

    // Command built for OpenCode 2 uses `acp` (not `serve` or `run`) and writes the V2 native guard config on spawn.
    let mut cmd = command_for_version(
        PathBuf::from("/fixture/opencode"),
        "opencode 2.0.0",
        &work,
        Mode::Ask,
        &store,
        false,
    )
    .unwrap();
    assert_eq!(cmd.get_program(), "/fixture/opencode");
    let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    assert_eq!(args, vec!["acp"]);
    assert_eq!(cmd.get_current_dir(), Some(work.as_path()));
    assert_eq!(env_of(&cmd, "LOCUST_NATIVE_OPENCODE_VERSION").as_deref(), Some("2.0.0"));

    let _spawned = OpenCode::spawn(&mut cmd).unwrap();
    let config: Value = serde_json::from_str(&env_of(&cmd, "OPENCODE_CONFIG_CONTENT").unwrap()).unwrap();
    assert_eq!(config["default_agent"], "locust");
    assert_eq!(config["agents"]["locust"]["mode"], "primary");
    assert!(config["agents"]["locust"]["permissions"].is_array());
    assert_eq!(config["plugins"].as_array().map(Vec::len), Some(1));
    assert!(config["plugins"][0].as_str().unwrap().starts_with("file://"));
    assert!(config.get("permission").is_none());
    assert!(config.get("experimental").is_none());

    // Live turn over ACP sets mode=locust, model, and effort before sending session/prompt.
    let _guard = native::hooks::install(
        store.clone(),
        Some((
            "python3".into(),
            vec![
                fixture("m4-opencode-acp.py").to_string_lossy().into_owned(),
                "logged".into(),
                "acp".into(),
            ],
        )),
    );
    let started = native::send(NativeSend {
        teammate: opencode_teammate(&work, "ask"),
        conversation: None,
        prompt: "Inspect the project.".into(),
        model: Some("opencode/zen-free".into()),
        effort: Some("high".into()),
    })
    .unwrap();
    wait_for_turn_end(&started.conversation_id);

    let sent = read_acp_log(&work);
    let prompt_at = sent
        .iter()
        .position(|m| m["method"] == "session/prompt")
        .expect("session/prompt sent");
    let config_sets: Vec<&Value> = sent[..prompt_at]
        .iter()
        .filter(|m| m["method"] == "session/set_config_option")
        .map(|m| &m["params"])
        .collect();
    assert_eq!(
        config_sets,
        vec![
            &json!({"sessionId": "s1", "configId": "mode", "value": "locust"}),
            &json!({"sessionId": "s1", "configId": "model", "value": "opencode/zen-free"}),
            &json!({"sessionId": "s1", "configId": "effort", "value": "high"}),
        ]
    );

    let ledger_path = store.ledger_dir().unwrap().join(format!("{}.jsonl", started.conversation_id));
    let ledger = std::fs::read_to_string(&ledger_path).unwrap();
    let created: Value = serde_json::from_str(ledger.lines().next().unwrap()).unwrap();
    assert_eq!(created["metadata"]["runtime"], "opencode");
    assert_eq!(created["metadata"]["cliVersion"], "2.0.0");
    assert_eq!(created["metadata"]["model"], "opencode/zen-free");
    assert!(ledger.contains("OpenCode says hi."));
}

// case: answers an Approve each card with the person's %s decision
#[test]
fn answers_an_approve_each_card_with_the_persons_decision() {
    for (decision, expected_option) in [(Decision::ApproveOnce, "yes"), (Decision::Deny, "no")] {
        let t = Scratch::new("approve-each");
        let store = make_store(&t.0);
        let work = t.0.join("work");
        std::fs::create_dir_all(&work).unwrap();

        let guard_path = opencode_config::write_guard(&store, true).unwrap();
        let acp_cfg = opencode_config::acp_config(Mode::ApproveEach, &guard_path);
        let perms = acp_cfg["agents"]["locust"]["permissions"].as_array().unwrap();
        assert!(perms.contains(&json!({"action": "*", "resource": "*", "effect": "ask"})));

        let mut command = Command::new("python3");
        command
            .arg(fixture("m4-opencode-acp.py"))
            .arg("logged")
            .arg("acp")
            .current_dir(&work);
        let mut rt = OpenCode::spawn(&mut command).unwrap();
        let channel = Channel::new();
        let ch = channel.clone();
        let work_check = work.clone();

        let decider = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                if let Some(card) = ch.waiting().first() {
                    let seen = (card.runtime.clone(), card.kind.clone(), card.detail.clone());
                    // No permission reply has been sent before the person decides.
                    let before = read_acp_log(&work_check);
                    assert!(!before.iter().any(|m| m["id"] == "permission"));
                    ch.decide(Answer::Decision {
                        approval_id: card.approval_id.clone(),
                        decision,
                        reason: None,
                    })
                    .unwrap();
                    return seen;
                }
                assert!(Instant::now() < deadline, "approval card did not appear");
                std::thread::sleep(Duration::from_millis(10));
            }
        });

        let turn = Turn {
            prompt: "Check files".into(),
            folder: work.clone(),
            model: Some("opencode/zen-free".into()),
            effort: None,
            resume: None,
            mode: Mode::ApproveEach,
            approvals: Some(channel.clone()),
            mission_id: "oc2_approve".into(),
            run_id: "run_oc2_approve".into(),
        };
        let mut writer = LedgerWriter::create_runtime(
            &store,
            "oc2_approve",
            &turn.prompt,
            "opencode/zen-free",
            "opencode",
            "opencode",
            &["acp".into()],
            None,
        )
        .unwrap();
        rt.start(&turn).unwrap();
        let events = drive(&mut rt, &mut writer, &channel, "oc2_approve");
        let (runtime, kind, detail) = decider.join().unwrap();
        assert_eq!(runtime, "opencode");
        assert_eq!(kind, Kind::Command);
        assert_eq!(detail, "echo hi");
        assert_eq!(events.last().unwrap().kind, "run.completed");

        let sent = read_acp_log(&work);
        let reply = sent.iter().find(|m| m["id"] == "permission").expect("permission reply logged");
        assert_eq!(
            reply["result"],
            json!({"outcome": {"outcome": "selected", "optionId": expected_option}})
        );
    }
}

// case: uses %s permission policy without raising a card
#[test]
fn uses_mode_permission_policy_without_raising_a_card() {
    for (mode, expected_option) in [(Mode::Auto, "yes"), (Mode::Ask, "no")] {
        let t = Scratch::new("policy");
        let store = make_store(&t.0);
        let work = t.0.join("work");
        std::fs::create_dir_all(&work).unwrap();

        let mut command = Command::new("python3");
        command
            .arg(fixture("m4-opencode-acp.py"))
            .arg("logged")
            .arg("acp")
            .current_dir(&work);
        let mut rt = OpenCode::spawn(&mut command).unwrap();
        let channel = Channel::new();
        let turn = Turn {
            prompt: "Check files".into(),
            folder: work.clone(),
            model: Some("opencode/zen-free".into()),
            effort: None,
            resume: None,
            mode,
            approvals: Some(channel.clone()),
            mission_id: "oc2_policy".into(),
            run_id: "run_oc2_policy".into(),
        };
        let mut writer = LedgerWriter::create_runtime(
            &store,
            "oc2_policy",
            &turn.prompt,
            "opencode/zen-free",
            "opencode",
            "opencode",
            &["acp".into()],
            None,
        )
        .unwrap();
        rt.start(&turn).unwrap();
        let events = drive(&mut rt, &mut writer, &channel, "oc2_policy");
        assert_eq!(events.last().unwrap().kind, "run.completed");
        assert!(channel.waiting().is_empty(), "must not raise an approval card in {mode:?}");

        let sent = read_acp_log(&work);
        let reply = sent.iter().find(|m| m["id"] == "permission").expect("permission reply logged");
        assert_eq!(
            reply["result"],
            json!({"outcome": {"outcome": "selected", "optionId": expected_option}})
        );
    }
}

// case: loads a follow-up ACP session while keeping its replayed history out of the new ledger
#[test]
fn loads_a_follow_up_acp_session_while_keeping_its_replayed_history_out_of_the_new_ledger() {
    let t = Scratch::new("followup");
    let store = make_store(&t.0);
    let work = t.0.join("work");
    std::fs::create_dir_all(&work).unwrap();

    let _guard = native::hooks::install(
        store.clone(),
        Some((
            "python3".into(),
            vec![
                fixture("m4-opencode-acp.py").to_string_lossy().into_owned(),
                "logged".into(),
                "acp".into(),
            ],
        )),
    );
    let first = native::send(NativeSend {
        teammate: opencode_teammate(&work, "ask"),
        conversation: None,
        prompt: "First turn".into(),
        model: Some("opencode/zen-free".into()),
        effort: None,
    })
    .unwrap();
    wait_for_turn_end(&first.conversation_id);

    let _ = std::fs::remove_file(work.join("acp-methods.log"));
    let second = native::send(NativeSend {
        teammate: opencode_teammate(&work, "ask"),
        conversation: Some(first.conversation_id.clone()),
        prompt: "Follow-up".into(),
        model: Some("opencode/zen-free".into()),
        effort: None,
    })
    .unwrap();
    wait_for_turn_end(&second.conversation_id);

    let sent = read_acp_log(&work);
    assert!(!sent.iter().any(|m| m["method"] == "session/new"), "follow-up must load existing session: {sent:?}");
    let load = sent
        .iter()
        .find(|m| m["method"] == "session/load")
        .expect("session/load called");
    assert_eq!(load["params"]["sessionId"], "s1");
    assert_eq!(load["params"]["mcpServers"], json!([]));

    let ledger_path = store.ledger_dir().unwrap().join(format!("{}.jsonl", second.conversation_id));
    let ledger = std::fs::read_to_string(&ledger_path).unwrap();
    assert!(ledger.contains("OpenCode says hi."));
    assert!(!ledger.contains("REPLAY_MUST_NOT_SURVIVE"));
}

// case: briefs a new session for %s instead of loading a foreign permission history
#[test]
fn native_briefs_a_new_session_for_version_change_or_imported_permissions_instead_of_loading_a_foreign_permission_history() {
    for reason in ["version-change", "imported-permissions"] {
        let t = Scratch::new(reason);
        let store = make_store(&t.0);
        let work = t.0.join("work");
        std::fs::create_dir_all(&work).unwrap();

        let _guard = native::hooks::install(
            store.clone(),
            Some((
                "python3".into(),
                vec![
                    fixture("m4-opencode-acp.py").to_string_lossy().into_owned(),
                    "logged".into(),
                    "acp".into(),
                ],
            )),
        );
        let first = native::send(NativeSend {
            teammate: opencode_teammate(&work, "ask"),
            conversation: None,
            prompt: "First turn".into(),
            model: Some("opencode/zen-free".into()),
            effort: None,
        })
        .unwrap();
        wait_for_turn_end(&first.conversation_id);

        // Mutate the prior turn's ledger header to simulate a V1 session or an imported session with no host command.
        let first_path = store.ledger_dir().unwrap().join(format!("{}.jsonl", first.conversation_id));
        let original = std::fs::read_to_string(&first_path).unwrap();
        let mut lines: Vec<String> = original.lines().map(str::to_owned).collect();
        let mut header: Value = serde_json::from_str(&lines[0]).unwrap();
        if reason == "version-change" {
            header["metadata"]["cliVersion"] = json!("1.19.3");
        } else {
            header["metadata"].as_object_mut().unwrap().remove("command");
        }
        lines[0] = header.to_string();
        std::fs::write(&first_path, format!("{}\n", lines.join("\n"))).unwrap();

        let _ = std::fs::remove_file(work.join("acp-methods.log"));
        let second = native::send(NativeSend {
            teammate: opencode_teammate(&work, "ask"),
            conversation: Some(first.conversation_id.clone()),
            prompt: "Follow-up".into(),
            model: Some("opencode/zen-free".into()),
            effort: None,
        })
        .unwrap();
        wait_for_turn_end(&second.conversation_id);

        let sent = read_acp_log(&work);
        assert!(
            !sent.iter().any(|m| m["method"] == "session/load"),
            "{reason}: must not load foreign permission history: {sent:?}"
        );
        assert!(
            sent.iter().any(|m| m["method"] == "session/new"),
            "{reason}: must open a new session"
        );
        let prompt = sent
            .iter()
            .find(|m| m["method"] == "session/prompt")
            .expect("prompt sent")
            .to_string();
        assert!(
            prompt.contains("First turn") && prompt.contains("Done."),
            "{reason}: fresh session must be briefed with prior answer: {prompt}"
        );
    }
}

// case: asks a side question in a freshly briefed ACP session without appending to the parent
#[test]
fn asks_a_side_question_in_a_freshly_briefed_acp_session_without_appending_to_the_parent() {
    let t = Scratch::new("side");
    let store = make_store(&t.0);
    let work = t.0.join("work");
    std::fs::create_dir_all(&work).unwrap();

    let _guard = native::hooks::install(
        store.clone(),
        Some((
            "python3".into(),
            vec![
                fixture("m4-opencode-acp.py").to_string_lossy().into_owned(),
                "logged".into(),
                "acp".into(),
            ],
        )),
    );
    let first = native::send(NativeSend {
        teammate: opencode_teammate(&work, "ask"),
        conversation: None,
        prompt: "Inspect the project.".into(),
        model: Some("opencode/zen-free".into()),
        effort: None,
    })
    .unwrap();
    wait_for_turn_end(&first.conversation_id);

    let parent_path = store.ledger_dir().unwrap().join(format!("{}.jsonl", first.conversation_id));
    let parent_before = std::fs::read_to_string(&parent_path).unwrap().lines().count();

    let _ = std::fs::remove_file(work.join("acp-methods.log"));
    let side = native::send_from(
        NativeSend {
            teammate: opencode_teammate(&work, "ask"),
            conversation: None,
            prompt: "What did that mean?".into(),
            model: Some("opencode/zen-free".into()),
            effort: None,
        },
        Origin {
            mode: Some(Mode::Ask),
            started_by: Some(json!({"kind": "side", "of": first.conversation_id, "question": 0})),
            runtime: Some("opencode".into()),
            ..Default::default()
        },
    )
    .unwrap();
    wait_for_turn_end(&side.conversation_id);

    let sent = read_acp_log(&work);
    assert!(!sent.iter().any(|m| m["method"] == "session/load"));
    assert!(sent.iter().any(|m| m["method"] == "session/new"));
    let prompt = sent
        .iter()
        .find(|m| m["method"] == "session/prompt")
        .expect("prompt sent")
        .to_string();
    assert!(prompt.contains("Inspect the project.") && prompt.contains("Done."), "{prompt}");
    let parent_after = std::fs::read_to_string(&parent_path).unwrap().lines().count();
    assert_eq!(parent_after, parent_before, "side question must not append to parent ledger");
}

// case: cancels a prompt over ACP and bounds an agent that never acknowledges cancellation
#[test]
fn cancels_a_prompt_over_acp_and_bounds_an_agent_that_never_acknowledges_cancellation() {
    assert_eq!(crate::runtime::acp::CANCEL_GRACE, Duration::from_secs(3));
    let t = Scratch::new("cancel");
    let store = make_store(&t.0);
    let work = t.0.join("work");
    std::fs::create_dir_all(&work).unwrap();

    let mut command = Command::new("python3");
    command
        .arg(fixture("m4-opencode-acp.py"))
        .arg("stop-hang")
        .arg("acp")
        .current_dir(&work);
    let mut rt = OpenCode::spawn(&mut command)
        .unwrap()
        .with_cancel_grace(Duration::from_millis(300));
    let turn = Turn {
        prompt: "Hang until cancelled".into(),
        folder: work.clone(),
        model: Some("opencode/zen-free".into()),
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "oc2_cancel".into(),
        run_id: "run_oc2_cancel".into(),
    };
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "oc2_cancel",
        &turn.prompt,
        "opencode/zen-free",
        "opencode",
        "opencode",
        &["acp".into()],
        None,
    )
    .unwrap();
    rt.start(&turn).unwrap();

    // Wait until session/prompt has reached the agent.
    let prompt_deadline = Instant::now() + Duration::from_secs(5);
    while !read_acp_log(&work).iter().any(|m| m["method"] == "session/prompt") {
        assert!(Instant::now() < prompt_deadline, "session/prompt never reached agent");
        std::thread::sleep(Duration::from_millis(10));
    }

    let stopped_at = Instant::now();
    rt.stop().unwrap();
    let events = drive(&mut rt, &mut writer, &Channel::new(), "oc2_cancel");
    let elapsed = stopped_at.elapsed();

    assert!(elapsed >= Duration::from_millis(250), "must wait for cancel grace before killing ({elapsed:?})");
    assert!(elapsed < Duration::from_secs(3), "must bound unresponsive cancellation ({elapsed:?})");
    assert!(events.iter().any(|e| e.kind == "run.cancelled"));

    let sent = read_acp_log(&work);
    let cancel = sent
        .iter()
        .find(|m| m["method"] == "session/cancel")
        .expect("session/cancel sent");
    assert_eq!(cancel["params"], json!({"sessionId": "s1"}));
}

// case: refuses V2 without its command guard instead of starting an unguarded process
#[test]
fn opencode_config_refuses_v2_without_its_command_guard_instead_of_starting_an_unguarded_process() {
    let t = Scratch::new("no-guard");
    let store = make_store(&t.0);
    let work = t.0.join("work");
    std::fs::create_dir_all(&work).unwrap();

    let _guard = native::hooks::install(
        store.clone(),
        Some((
            "python3".into(),
            vec![
                fixture("m4-opencode-acp.py").to_string_lossy().into_owned(),
                "logged".into(),
                "acp".into(),
            ],
        )),
    );
    FAIL_V2_GUARD.with(|f| f.set(true));

    let cmd_err = command_for_version(
        PathBuf::from("/fixture/opencode"),
        "opencode 2.0.0",
        &work,
        Mode::Ask,
        &store,
        false,
    )
    .unwrap_err()
    .to_string();
    assert!(cmd_err.contains("command guard"), "{cmd_err}");

    let send_err = native::send(NativeSend {
        teammate: opencode_teammate(&work, "ask"),
        conversation: None,
        prompt: "Inspect the project.".into(),
        model: Some("opencode/zen-free".into()),
        effort: None,
    })
    .unwrap_err();
    assert!(send_err.contains("command guard"), "{send_err}");

    // Refused before creating any mission ledger or spawning an ACP process.
    let ledgers: Vec<_> = std::fs::read_dir(store.ledger_dir().unwrap())
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .collect();
    assert!(ledgers.is_empty(), "must not create a mission ledger on guard failure");
    assert!(!work.join("acp-methods.log").exists(), "must not spawn an unguarded process");
}

// case: retains the V1 serve route and its original hook when the serve bridge is available
#[test]
fn retains_the_v1_serve_route_and_its_original_hook_when_the_serve_bridge_is_available() {
    let t = Scratch::new("v1-serve");
    let store = make_store(&t.0);
    let work = t.0.join("work");
    std::fs::create_dir_all(&work).unwrap();

    let cmd = command_for_version(
        PathBuf::from("/fixture/opencode"),
        "opencode 1.19.3",
        &work,
        Mode::Ask,
        &store,
        false,
    )
    .unwrap();
    let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().into_owned()).collect();
    assert!(args.iter().any(|a| a == "serve"), "{args:?}");
    assert!(!args.iter().any(|a| a == "acp"), "{args:?}");
    let config: Value = serde_json::from_str(&env_of(&cmd, "OPENCODE_CONFIG_CONTENT").unwrap()).unwrap();
    let plugins = config["plugin"].as_array().expect("V1 plugin hook");
    assert_eq!(plugins.len(), 1);
    assert!(plugins[0].as_str().unwrap().ends_with("/index.js"));
    assert!(config.get("plugins").is_none());

    // Live V1 serve route still runs and cancels cleanly.
    let mut serve = Command::new("python3");
    serve
        .arg(fixture("m4-opencode-serve.py"))
        .arg("serve")
        .arg("success")
        .current_dir(&work)
        .env("LOCUST_NATIVE_OPENCODE_PROTOCOL", "serve");
    let mut rt = OpenCode::spawn(&mut serve).unwrap();
    let turn = Turn {
        prompt: "Serve me".into(),
        folder: work.clone(),
        model: Some("opencode/zen-free".into()),
        effort: None,
        resume: None,
        mode: Mode::Ask,
        approvals: None,
        mission_id: "oc1_serve".into(),
        run_id: "run_oc1_serve".into(),
    };
    let mut writer = LedgerWriter::create_runtime(
        &store,
        "oc1_serve",
        &turn.prompt,
        "opencode/zen-free",
        "opencode",
        "opencode",
        &["serve".into()],
        None,
    )
    .unwrap();
    rt.start(&turn).unwrap();
    let events = drive(&mut rt, &mut writer, &Channel::new(), "oc1_serve");
    assert_eq!(events.last().unwrap().kind, "run.completed");
}

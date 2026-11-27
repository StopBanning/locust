//! what-happens-in-the-terminal-comes-back.test.ts, at the window (App.tsx 0.391): what the person did in the terminal
//! is asked for when a settled Claude Code or Codex conversation is shown, and again whenever the window comes back into
//! focus on it, without a press; a running turn, or another agent's conversation, is not asked about.
use super::*;

fn meta(mission_id: &str, runtime: &str) -> ledger::Metadata {
    ledger::Metadata { mission_id: mission_id.into(), prompt: "Ask".into(), runtime: runtime.into(), ..Default::default() }
}

fn showing(tip: &str, runtime: &str, running: bool) -> State {
    let mut s = State::default();
    s.ledger = Some(LedgerPane {
        cold_tip: None,
        header: Teammate { id: "tm_wren".into(), name: "Wren".into(), role: String::new(), runtime: Some(runtime.into()), model: None,
            account: None, busy: false, runs_in: "ask".into(), folder: None },
        conversation: Conversation { conversation_id: Some(tip.into()), turns: vec![] },
        items: vec![], dir: std::path::PathBuf::new(), worker: None,
        snapshot: ledger::Snapshot { tip: tip.into(), turn_metas: vec![meta(tip, runtime)], running, ..Default::default() },
        due: 0.0, loading: false, after_tip: None, after_tip_deadline: None, backoff: 0.0,
    });
    s
}

/// Polls until the asked catch-up has answered and its answer is taken.
fn settle(s: &mut State) {
    for _ in 0..400 {
        poll(s);
        if !s.terminal.catchup.is_pending() {
            poll(s);
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the catch-up never answered");
}

// Not an Electron case of its own: the window side of the row, the call site that keeps the terminal offer's promise.
#[test]
fn a_settled_conversation_asks_for_the_terminal_when_shown_and_again_on_focus() {
    let mut s = showing("mission_shown", "claude", false);
    poll(&mut s);
    assert_eq!(s.terminal.asks, 1, "asked when the conversation is shown");
    assert_eq!(s.terminal.asked.as_deref(), Some("mission_shown"));
    settle(&mut s);
    poll(&mut s);
    assert_eq!(s.terminal.asks, 1, "not asked again while nothing changed");
    assert_eq!(s.composer_note, None, "asked on its own, it says nothing when nothing came back");
    s.window_focused = false;
    poll(&mut s);
    s.window_focused = true;
    poll(&mut s);
    assert_eq!(s.terminal.asks, 2, "asked again when the window comes back into focus on it");
    settle(&mut s);
    // A new turn on screen (one the terminal brought back, or the next reply) is asked about too.
    s.ledger.as_mut().unwrap().snapshot.tip = "mission_next".into();
    poll(&mut s);
    assert_eq!((s.terminal.asks, s.terminal.asked.as_deref()), (3, Some("mission_next")));
    settle(&mut s);

    let mut codex = showing("mission_codex", "codex", false);
    poll(&mut codex);
    assert_eq!(codex.terminal.asks, 1, "Codex's rollout is read the same way");
    settle(&mut codex);
}

#[test]
fn a_running_turn_or_another_agents_conversation_is_not_asked_about() {
    let mut running = showing("mission_running", "claude", true);
    poll(&mut running);
    assert_eq!(running.terminal.asks, 0, "a running turn is its conversation's newest: nothing continues one");
    let mut other = showing("mission_cursor", "cursor", false);
    poll(&mut other);
    assert_eq!(other.terminal.asks, 0, "only a runtime whose sessions the host can read");
}

// Each case that goes through the app's real store/terminal runs in a fresh test process.
// PATH and the person's shell and home can then be controlled without changing other tests.
#[cfg(unix)]
mod live_boundary {
    use super::*;
    use serde_json::json;
    use std::{fs, path::{Path, PathBuf}, process::Command, time::{Duration, Instant}};

    const MISSION: &str = "mission_first";
    const SESSION: &str = "5f0c2a1e-8d4b-4c6a-9e2f-1b3c4d5e6f70";
    const START: &str = "2026-09-27T09:58:00.000Z";
    const END: &str = "2026-09-27T10:00:00.000Z";

    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }

    fn in_own_profile(test: &str) -> Option<PathBuf> {
        let var = "LOCUST_TERMINAL_TEST_PROFILE";
        if let Some(path) = std::env::var_os(var) {
            return Some(path.into());
        }
        let root = std::env::temp_dir().join(format!("locust-terminal-{}", locust_core::invariants::unique_nanos()));
        fs::create_dir_all(&root).unwrap();
        let _cleanup = Temp(root.clone());
        let shell = root.join("fixture-shell");
        fs::write(&shell, r#"#!/bin/sh
if [ "$2" = "-c" ]; then
    printf '%s\n' "$3" > command-receipt
    printf 'hello\n'
else
    printf '%s\n' "$TERM" "$PTY_PERSON" "$@" > terminal-receipt
fi
"#).unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&shell, fs::Permissions::from_mode(0o700)).unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", test.split_once("::").unwrap().1, "--nocapture"])
            .env(var, &root)
            .env("HOME", root.join("home"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("LOCUST_NATIVE_DATA", root.join("native"))
            .env("SHELL", &shell)
            .env("PTY_PERSON", "person-environment")
            .output().unwrap();
        assert!(output.status.success() && String::from_utf8_lossy(&output.stdout).contains("1 passed"),
            "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        None
    }

    fn record(store: &locust_core::store::Store) {
        let metadata = json!({"missionId": MISSION, "runId": "run_mission_first", "prompt": "Who takes what?",
            "runtime": "claude", "model": "opus", "requestedRouteId": "claude:opus", "resolvedRouteId": "claude:opus",
            "workspaceId": "ws_other", "sandbox": "workspace-write", "executionPolicyVersion": 1, "createdAt": START});
        let event = |at, seq, kind, payload| json!({"schemaVersion": 27, "recordType": "mission.event",
            "ledgerSequence": seq + 1, "occurredAt": at, "event": {"id": format!("run_mission_first:{seq}"),
            "missionId": MISSION, "runId": "run_mission_first", "sourceAdapter": "claude", "runtimeThreadId": SESSION,
            "sequence": seq, "occurredAt": at, "type": kind, "payload": payload}});
        let rows = [
            json!({"schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": START, "metadata": metadata}),
            event(START, 1, "run.started", json!({"runtimeThreadId": SESSION, "evidence": {"redacted": true}})),
            event(END, 2, "run.completed", json!({"runtimeThreadId": SESSION, "process": {"exitCode": 0, "signal": null,
                "stderr": "", "stderrTruncated": false, "recordCount": 2, "inputDeliveryFailed": false,
                "outputLimitExceeded": false, "oversizedRecordsDropped": 0, "forcedTerminationAttempted": false,
                "terminationUnconfirmed": false, "startedAt": START, "finishedAt": END}})),
        ];
        fs::create_dir_all(store.ledger_dir().unwrap()).unwrap();
        fs::write(store.write_path(MISSION).unwrap(), rows.iter().map(|row| format!("{row}\n")).collect::<String>()).unwrap();
        store.save_json("teammates.json", &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_wren", "name": "Wren", "worktree": true}],
            "missionOwners": {MISSION: "tm_wren"}, "settings": {}})).unwrap();
    }

    fn window(state: State) -> rune::Runtime<State> {
        let mut rt = rune::Runtime::new(state, crate::view, (1200., 850.), rune::Env::new(true, 1.));
        for _ in 0..30 { rt.frame(0.05); }
        rt
    }

    fn toggle(rt: &mut rune::Runtime<State>) {
        let (x, y) = rt.find("terminal-toggle").expect("terminal toggle drawn in this conversation");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(0.016);
    }

    fn until(mut f: impl FnMut() -> bool, what: &str) {
        let limit = Instant::now() + Duration::from_secs(3);
        while !f() {
            assert!(Instant::now() < limit, "{what}");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn git(folder: &Path, args: &[&str]) {
        let output = Command::new("git").current_dir(folder).args(args).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    }

    // case: starts only when requested, in the host-resolved conversation folder, with the person’s environment
    #[test]
    fn terminal_starts_on_press_in_the_selected_conversations_worktree() {
        let Some(root) = in_own_profile(concat!(module_path!(), "::terminal_starts_on_press_in_the_selected_conversations_worktree")) else { return; };
        locust_core::org_policy::use_policy_on_this_thread(None);
        let store = native::store().unwrap();
        record(&store);
        let project = root.join("project");
        let conversation = root.join("conversation");
        let tree = conversation.join(".locust/worktrees/tm_wren");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&conversation).unwrap();
        native::set_open_folder(&store, &project).unwrap();
        locust_core::folders::learn(&store, &[("ws_other".into(), conversation.to_string_lossy().into())]).unwrap();
        git(&conversation, &["init", "-q"]);
        git(&conversation, &["-c", "user.name=Test", "-c", "user.email=t@example.com", "commit", "-qm", "fixture", "--allow-empty"]);
        git(&conversation, &["worktree", "add", "-qb", "wren", tree.to_str().unwrap()]);
        let mut s = showing(MISSION, "claude", false);
        s.data = Some(store);
        let mut rt = window(s);
        assert!(rt.state.terminal.tabs.is_empty());
        assert!(!tree.join("terminal-receipt").exists());
        toggle(&mut rt);
        assert!(rt.state.terminal.open);
        assert_eq!(rt.state.terminal.tabs.first().map(|tab| tab.folder.as_path()), Some(tree.as_path()),
            "the open tab uses the conversation's registered worktree, not the current project: {:?}", rt.state.terminal.error);
        let receipt = tree.join("terminal-receipt");
        until(|| fs::read_to_string(&receipt).is_ok_and(|text| text.lines().count() >= 2), "the PTY shell never wrote its receipt");
        let text = fs::read_to_string(receipt).unwrap();
        assert!(text.starts_with("xterm-256color\nperson-environment\n"), "{text:?}");
        assert!(!project.join("terminal-receipt").exists());
        assert!(!conversation.join("terminal-receipt").exists());
    }

    // case: runs in the conversation folder, in the terminal panel's default shell, not a path the window sent
    #[test]
    fn terminal_bang_uses_the_saved_folder_and_the_default_shell() {
        let Some(root) = in_own_profile(concat!(module_path!(), "::terminal_bang_uses_the_saved_folder_and_the_default_shell")) else { return; };
        locust_core::org_policy::use_policy_on_this_thread(None);
        let store = native::store().unwrap();
        record(&store);
        store.save_json("teammates.json", &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_wren", "name": "Wren"}],
            "missionOwners": {MISSION: "tm_wren"}, "settings": {}})).unwrap();
        let project = root.join("project");
        let conversation = root.join("shop");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&conversation).unwrap();
        native::set_open_folder(&store, &project).unwrap();
        locust_core::folders::learn(&store, &[("ws_other".into(), conversation.to_string_lossy().into())]).unwrap();
        let mut s = showing(MISSION, "claude", false);
        s.data = Some(store);
        let mut rt = window(s);
        let (x, y) = rt.find("composer").expect("composer drawn in this conversation");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.paste("!echo hello");
        rt.frame(0.016);
        rt.key(Key::Enter.into());
        rt.frame(0.016);
        assert_eq!(rt.state.terminal.commands.len(), 1, "the typed command starts: {:?}", rt.state.composer_note);
        until(|| {
            poll(&mut rt.state);
            rt.state.terminal.commands[0].result.is_some()
        }, "the command never finished");
        let result = rt.state.terminal.commands[0].result.as_ref().unwrap();
        assert_eq!(result.output.trim_end_matches(['\r', '\n']), "hello");
        assert_eq!(result.exit, Some(0));
        assert!(!result.stopped);
        assert_eq!(fs::read_to_string(conversation.join("command-receipt")).expect("the command receipt belongs in the selected folder"), "echo hello\n");
        assert!(!project.join("command-receipt").exists());
    }

    fn terminal_text(path: &Path) {
        // The prompt and final answer from the first terminal exchange in Electron's CLAUDE_SESSION.
        let typed = json!({"sessionId": SESSION, "type": "user", "entrypoint": "cli", "isSidechain": false,
            "timestamp": "2026-09-27T10:05:00.000Z", "message": {"role": "user", "content": "Now add tests for the migrations."}});
        let answer = json!({"sessionId": SESSION, "type": "assistant", "isSidechain": false, "timestamp": "2026-09-27T10:06:00.000Z",
            "message": {"role": "assistant", "model": "claude-opus-5-5", "stop_reason": "end_turn", "content": [
                {"type": "text", "text": "Added three tests; all pass."}]}});
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("{typed}\n{answer}\n")).unwrap();
    }

    // The focus side of the Electron case in what-happens-in-the-terminal-comes-back.test.ts:210.
    #[test]
    fn terminal_focus_during_a_pending_read_brings_back_the_real_exchange() {
        let Some(root) = in_own_profile(concat!(module_path!(), "::terminal_focus_during_a_pending_read_brings_back_the_real_exchange")) else { return; };
        let store = native::store().unwrap();
        record(&store);
        let mut s = showing(MISSION, "claude", false);
        s.data = Some(store.clone());
        let pane = s.ledger.as_mut().unwrap();
        pane.dir = store.ledger_dir().unwrap();
        pane.worker = Some(native::session_for(pane.dir.clone(), MISSION.into(), None));
        pane.due = f64::INFINITY; // The settled pane isn't polling the disk any more.
        let mut rt = window(s);
        rt.window_focus(true);
        rt.frame(0.016);
        let mut now = 1.;
        let mut app_tick = |rt: &mut rune::Runtime<State>| {
            now += 0.1;
            rt.update(|s, tx| { crate::poll(s, now, tx); });
            rt.frame(0.016);
        };
        app_tick(&mut rt);
        settle(&mut rt.state);
        assert_eq!(rt.state.terminal.asks, 1, "the initial empty read went through the app");
        assert_eq!(native::real_tip(MISSION).unwrap(), MISSION);

        // Hold the worker busy across a real blur/focus. Only the subsequent request reads the transcript.
        let (release, wait) = std::sync::mpsc::channel();
        rt.state.terminal.catchup.request((), move |()| { wait.recv_timeout(Duration::from_secs(10)).unwrap(); Ok(0) });
        rt.window_focus(false);
        rt.frame(0.016);
        app_tick(&mut rt);
        terminal_text(&root.join(format!("home/.claude/projects/C--work/{SESSION}.jsonl")));
        rt.window_focus(true);
        rt.frame(0.016);
        assert!(rt.state.window_focused);
        app_tick(&mut rt);
        assert_eq!(rt.state.terminal.asks, 1, "don't discard the read in flight");
        release.send(()).unwrap();

        // The next poll should still honor the focus change, using the actual local store and runtime reader.
        until(|| {
            app_tick(&mut rt);
            native::real_tip(MISSION).is_ok_and(|id| id != MISSION)
        }, "focus returned while a read was pending, but the terminal turn never reached disk");
        let latest = native::real_tip(MISSION).unwrap();
        let turn = native::turn_records(&latest).unwrap();
        assert!(!turn.incomplete);
        assert_eq!(turn.metadata["prompt"], "Now add tests for the migrations.");
        assert_eq!(turn.metadata["startedBy"]["kind"], "terminal");
        assert_eq!(turn.metadata["continuesFrom"]["missionId"], MISSION);
        assert_eq!(turn.metadata["continuesFrom"]["runtimeThreadId"], SESSION);
        assert!(turn.events.iter().any(|e| e["type"] == "message.delta" && e["payload"]["text"] == "Added three tests; all pass."));
        until(|| {
            app_tick(&mut rt);
            rt.state.ledger.as_ref().is_some_and(|pane| pane.snapshot.tip == latest
                && pane.items.iter().any(|item| matches!(item, ledger::Item::Assistant { text, .. } if text == "Added three tests; all pass.")))
        }, "the settled thread never showed the imported answer");
        assert_eq!(locust_core::people::owner_of(&store, &latest).as_deref(), Some("tm_wren"));
        assert!(rt.elements().into_iter().any(|el| el.text.is_some_and(|text| text.contains("Added three tests; all pass."))),
            "the real window draws the imported answer");
        assert_eq!(rt.state.ledger.as_ref().unwrap().conversation.conversation_id.as_deref(), Some(MISSION));
        assert!(rt.state.terminal.asks >= 2);
    }
}

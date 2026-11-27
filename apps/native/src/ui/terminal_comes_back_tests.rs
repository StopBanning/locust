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
        fs::write(&shell, "#!/bin/sh\nprintf '%s\\n' \"$TERM\" \"$PTY_PERSON\" \"$@\" > terminal-receipt\n").unwrap();
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
}

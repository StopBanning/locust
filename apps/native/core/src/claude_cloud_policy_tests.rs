//! Native Claude Cloud actions against the Electron launch gates. Everything that could
//! open a terminal, start a process, or operate on a Git worktree is a fake.
use super::*;
use crate::org_policy::{PolicyState, use_policy_on_this_thread};
use crate::store::Store;
use std::cell::Cell;

struct Policy;
impl Policy {
    fn set(body: Value) -> Self {
        let policy = crate::org_policy::parse_policy(&body, &mut |_| {}).unwrap();
        use_policy_on_this_thread(Some(PolicyState {
            path: "/test/policy.json".into(),
            present: true,
            policy,
            error: None,
        }));
        Self
    }

    fn broken() -> Self {
        use_policy_on_this_thread(Some(crate::org_policy::read_policy(
            "/test/policy.json",
            &|_| Ok(b"not json".to_vec()),
            &mut |_| {},
        )));
        Self
    }
}
impl Drop for Policy {
    fn drop(&mut self) {
        use_policy_on_this_thread(None);
    }
}

struct Fixture(PathBuf, Store);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "locust-cloud-policy-{}",
            crate::invariants::unique_nanos()
        ));
        let store = Store::new(root.join("native"), root.join("electron")).unwrap();
        Self(root, store)
    }

    fn cloud<'a>(&self, seen: &'a Seen, platform: CloudPlatform) -> ClaudeCloud<'a> {
        // Use the actual constructor, including whoever supplies the engine's spend gate.
        let mut cloud: ClaudeCloud<'a> = on_this_computer_with(&self.1, Arc::default(), || {
            Some(ClaudeLaunch {
                discovered_path: "claude".into(),
                executable_path: "claude".into(),
                prefix_args: Vec::new(),
                env: None,
            })
        });
        cloud.platform = platform;
        let read_discovery = cloud.discover;
        cloud.discover = Box::new(move || {
            seen.discovery.set(seen.discovery.get() + 1);
            read_discovery()
        });
        let fake_git = self.0.join(".git").display().to_string();
        cloud
            .with_claude_home(self.0.join("claude-home"))
            .with_process_env(Vec::new)
            .with_run_window(move |_| {
                seen.windows.set(seen.windows.get() + 1);
                Ok(())
            })
            .with_terminal(move |_| {
                seen.ptys.set(seen.ptys.get() + 1);
                PseudoTerminalResult::Err
            })
            .with_exec(move |_, _, _| {
                seen.execs.set(seen.execs.get() + 1);
                ExecReply {
                    code: 1,
                    output: String::new(),
                    stdout: None,
                }
            })
            .with_git_changes(move |_| {
                seen.gits.set(seen.gits.get() + 1);
                Some(0)
            })
            .with_git(move |args, _, _| {
                seen.gits.set(seen.gits.get() + 1);
                GitRan {
                    code: 0,
                    stdout: if args.contains(&"--git-common-dir") {
                        fake_git.clone()
                    } else {
                        String::new()
                    },
                    stderr: String::new(),
                }
            })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[derive(Default)]
struct Seen {
    discovery: Cell<usize>,
    windows: Cell<usize>,
    ptys: Cell<usize>,
    execs: Cell<usize>,
    gits: Cell<usize>,
}
impl Seen {
    fn assert_nothing_started(&self) {
        assert_eq!(
            (
                self.discovery.get(),
                self.windows.get(),
                self.ptys.get(),
                self.execs.get(),
                self.gits.get()
            ),
            (0, 0, 0, 0, 0),
            "refused before discovery, PTY, terminal window, exec or Git",
        );
    }
}

const ID: &str = "cc_0001";
const FOLDER: &str = "/fake/folder";
const MODEL: &str = "sonnet";
fn known_session(cloud: &ClaudeCloud<'_>) {
    cloud
        .write_sessions(&[ClaudeCloudSession {
            id: ID.into(),
            started_at: "2026-11-01T00:00:00Z".into(),
            prompt: "first task".into(),
            folder: FOLDER.into(),
            teammate_id: None,
            session_id: Some("session_01Jed6HzK3Np9xeyKnSQ7oBG".into()),
            url: None,
            title: None,
            note: None,
        }])
        .unwrap();
}
fn start_refusal(answer: ClaudeCloudAnswer) -> String {
    match answer {
        ClaudeCloudAnswer::Err { message } => message,
        ClaudeCloudAnswer::Ok { .. } => panic!("started a refused cloud run"),
    }
}
fn action_refusal(answer: CloudSendResult) -> String {
    match answer {
        CloudSendResult::Err { message } => message,
        CloudSendResult::Ok => panic!("ran a refused cloud action"),
    }
}

// Electron names the common cloud/background boundary. These are its Claude Cloud launch sites.
// case: gates every separate background and cloud launch before calling its service
#[test]
fn claude_cloud_launches_refuse_before_any_side_effect() {
    let fixture = Fixture::new();
    let seen = Seen::default();
    let windows = fixture.cloud(&seen, CloudPlatform::Win32);
    let mac = fixture.cloud(&seen, CloudPlatform::Darwin);
    known_session(&windows);
    windows
        .remember_environment(FOLDER, Some("ccpool_before"))
        .unwrap();
    let rows = fs::read(&windows.store_path).unwrap();
    let environments = fs::read(windows.environment_path()).unwrap();
    let choice = CloudChoice {
        model: Some(MODEL.into()),
        effort: None,
        environment: Some("ccpool_after".into()),
    };
    for (settings, word) in [
        (json!({"cloud": false}), "cloud"),
        (json!({"allowedAgents": ["codex"]}), "allowedAgents"),
        (json!({"highestMode": "plan"}), "highestMode"),
        (json!({"monthlyLimitUsd": 50}), "monthlyLimitUsd"),
    ] {
        let _policy = Policy::set(settings);
        // Win32 tries a hidden PTY first. macOS opens a window directly.
        for cloud in [&windows, &mac] {
            assert!(
                start_refusal(cloud.start(FOLDER, "second task", None, Some(&choice)))
                    .contains(word),
                "start: {word}"
            );
        }
        assert!(
            action_refusal(mac.home(ID)).contains(word),
            "bring home: {word}"
        );
        assert!(
            action_refusal(mac.send(ID, "follow up")).contains(word),
            "send: {word}"
        );
        assert!(
            matches!(mac.check(ID), ClaudeCloudReading::Err { message } if message.contains(word)),
            "read PTY: {word}"
        );
    }
    {
        let _policy = Policy::broken();
        assert!(
            start_refusal(mac.start(FOLDER, "second task", None, Some(&choice))).contains("policy")
        );
        assert!(action_refusal(mac.home(ID)).contains("policy"));
        assert!(action_refusal(mac.send(ID, "follow up")).contains("policy"));
        assert!(
            matches!(mac.check(ID), ClaudeCloudReading::Err { message } if message.contains("policy"))
        );
    }
    seen.assert_nothing_started();
    assert_eq!(
        fs::read(&windows.store_path).unwrap(),
        rows,
        "no cloud row was changed"
    );
    assert_eq!(
        fs::read(windows.environment_path()).unwrap(),
        environments,
        "no environment was changed"
    );
}

#[test]
fn the_allowed_model_is_the_validated_program_argument_not_just_the_raw_choice() {
    let fixture = Fixture::new();
    let seen = Seen::default();
    let mac = fixture.cloud(&seen, CloudPlatform::Darwin);
    let _policy = Policy::set(json!({"allowedModels": {"claude": ["sonnet", "fake*"]}}));
    for model in [
        None,
        Some("account-default"),
        Some("fake model"),
        Some("opus"),
    ] {
        let choice = CloudChoice {
            model: model.map(str::to_owned),
            ..Default::default()
        };
        let reason = start_refusal(mac.start(FOLDER, "task", None, Some(&choice)));
        assert!(reason.contains("allowedModels"), "{model:?}: {reason}");
    }
    seen.assert_nothing_started();
    let choice = CloudChoice {
        model: Some(MODEL.into()),
        ..Default::default()
    };
    assert!(matches!(
        mac.start(FOLDER, "task", None, Some(&choice)),
        ClaudeCloudAnswer::Ok { .. }
    ));
    assert_eq!(
        seen.windows.get(),
        1,
        "validated Sonnet reaches the fake window"
    );
}

#[test]
fn the_start_checks_this_teammates_budget_before_opening_or_recording_anything() {
    let fixture = Fixture::new();
    fixture.1.save_json("teammates.json", &json!({"schemaVersion": 1,
        "teammates": [{"teammateId": "tmlimited", "name": "Ada", "hue": "lime", "role": "Code & Migrations",
                       "createdAt": "2026-11-01T00:00:00Z", "monthlyLimitUsd": 1.0},
                      {"teammateId": "tmopen", "name": "Bea", "hue": "lime", "role": "Code & Migrations",
                       "createdAt": "2026-11-01T00:00:00Z"}], "missionOwners": {"mission_ada":"tmlimited"}, "settings": {}
    })).unwrap();
    std::fs::create_dir_all(fixture.1.ledger_dir().unwrap()).unwrap();
    let at = chrono::Local::now().to_rfc3339();
    let created = json!({"schemaVersion":27,"recordType":"mission.created","metadata":{"missionId":"mission_ada","runtime":"claude","model":"sonnet","createdAt":at}});
    let completed = json!({"schemaVersion":27,"recordType":"mission.event","event":{"type":"run.completed","occurredAt":at,"sequence":1,"payload":{"usage":{"usd":1.0}}}});
    fs::write(
        fixture.1.ledger_dir().unwrap().join("mission_ada.jsonl"),
        format!("{created}\n{completed}\n"),
    )
    .unwrap();
    let seen = Seen::default();
    let mac = fixture.cloud(&seen, CloudPlatform::Darwin);
    let _policy = Policy::set(json!({}));
    let reason = start_refusal(mac.start(FOLDER, "task", Some("tmlimited"), None));
    assert!(
        reason.contains("Ada has reached this month's limit: $1.00 of $1.00."),
        "{reason}"
    );
    seen.assert_nothing_started();
    assert!(!mac.store_path.exists());
    assert!(matches!(
        mac.start(FOLDER, "task", Some("tmopen"), None),
        ClaudeCloudAnswer::Ok { .. }
    ));
    assert_eq!(seen.windows.get(), 1);
}

#[test]
fn reading_the_saved_rows_does_not_spend_or_open_a_terminal() {
    let fixture = Fixture::new();
    let seen = Seen::default();
    let mac = fixture.cloud(&seen, CloudPlatform::Darwin);
    known_session(&mac);
    let _policy = Policy::set(json!({"cloud": false}));
    assert_eq!(mac.list(FOLDER).len(), 1);
    assert_eq!(mac.all().len(), 1);
    assert_eq!(mac.environment(FOLDER), None);
    assert_eq!(
        (
            seen.discovery.get(),
            seen.windows.get(),
            seen.ptys.get(),
            seen.execs.get()
        ),
        (0, 0, 0, 0)
    );
}

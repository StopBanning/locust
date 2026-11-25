//! Codex's cloud task rows in the window (`CloudTasks.tsx`), drawn through the whole window (`crate::view`) and driven
//! by its poll (`crate::poll`), backed by a fake `CloudTaskService` (never a real Codex).

use super::*;
use locust_core::claude_cloud::{ClaudeCloud, ClaudeLaunch, CloudPlatform, WindowRun};
use locust_core::cloud_tasks::Ran;
use rune::{Env, Runtime};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct World {
    root: PathBuf,
    codex_calls: Arc<Mutex<Vec<Vec<String>>>>,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[derive(Clone)]
struct FakeSpec {
    repo_url: Option<String>,
    branch: Option<String>,
    codex_ready: bool,
    missing_env_repos: Vec<String>,
    other_repos: Vec<(String, String)>,
    status_reply: String,
}

impl Default for FakeSpec {
    fn default() -> Self {
        Self {
            repo_url: Some("https://github.com/acme/widgets.git".into()),
            branch: Some("main".into()),
            codex_ready: true,
            missing_env_repos: Vec::new(),
            other_repos: Vec::new(),
            status_reply: "[READY] Fix checkout total acme/widgets  •  1m ago +2/-2 • 2 files\n".into(),
        }
    }
}

fn fake_codex_service(spec: FakeSpec, calls: Arc<Mutex<Vec<Vec<String>>>>) -> MakeCodex {
    Arc::new(move |store: &Store| {
        let file = cloud_task_service::store_path_of(store);
        let spec_codex = spec.clone();
        let spec_git = spec.clone();
        let calls_codex = calls.clone();
        let ready = spec.codex_ready;
        CloudTaskService::from_runners(
            file,
            move |args: &[&str], _cwd: &Path| {
                calls_codex
                    .lock()
                    .unwrap()
                    .push(args.iter().map(|s| (*s).to_owned()).collect());
                match args.get(1).copied() {
                    Some("list") => {
                        let env_repo = args
                            .windows(2)
                            .find(|w| w[0] == "--env")
                            .map(|w| w[1])
                            .unwrap_or("");
                        if spec_codex.missing_env_repos.iter().any(|r| r == env_repo) {
                            Ok(Ran::new(1, "", format!("environment '{env_repo}' not found")))
                        } else {
                            Ok(Ran::ok(""))
                        }
                    }
                    Some("status") => Ok(Ran::ok(spec_codex.status_reply.clone())),
                    _ => Ok(Ran::ok("")),
                }
            },
            move |args: &[&str], cwd: &Path| {
                let cwd_s = cwd.to_string_lossy();
                let cmd = args.first().copied().unwrap_or("");
                if cmd == "remote" {
                    if let Some((_, url)) = spec_git
                        .other_repos
                        .iter()
                        .find(|(path, _)| cwd_s.as_ref() == path.as_str())
                    {
                        return Ok(Ran::ok(format!("{url}\n")));
                    }
                    return match &spec_git.repo_url {
                        Some(url) => Ok(Ran::ok(format!("{url}\n"))),
                        None => Ok(Ran::new(2, "", "fatal: No such remote 'origin'")),
                    };
                }
                if cmd == "rev-parse" {
                    return match &spec_git.branch {
                        Some(b) => Ok(Ran::ok(format!("{b}\n"))),
                        None => Ok(Ran::new(1, "", "fatal: not a git repository")),
                    };
                }
                Ok(Ran::ok(""))
            },
        )
        .with_ready(move || ready)
    })
}

fn fake_claude() -> crate::claude_cloud_panel::MakeClaude {
    Arc::new(|store: &Store, reads| {
        ClaudeCloud::new(
            locust_core::claude_cloud::store_path_of(store),
            CloudPlatform::Darwin,
            || {
                Some(ClaudeLaunch {
                    discovered_path: "/fake/claude".into(),
                    executable_path: "/fake/claude".into(),
                    prefix_args: Vec::new(),
                    env: None,
                })
            },
        )
        .with_reads(reads)
        .with_run_window(|_: &WindowRun| Ok(()))
    })
}

fn window(
    runtime: &str,
    spec: FakeSpec,
    tasks_json: serde_json::Value,
    claude_sessions: Option<serde_json::Value>,
    extra_folders: &[(&str, &str)],
) -> (World, Runtime<State>) {
    let root = std::env::temp_dir().join(format!(
        "locust-codex-cloud-{}-{}",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let root = root.canonicalize().unwrap();
    let work = root.join("work");
    let here = work.to_string_lossy().into_owned();

    let mut spec = spec;
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    store
        .save_json(
            "teammates.json",
            &serde_json::json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codex", "name": "Chip"}], "missionOwners": {}, "settings": {}}),
        )
        .unwrap();

    for (name, repo_url) in extra_folders {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let dir_s = dir.to_string_lossy().into_owned();
        let now = chrono::Utc::now().to_rfc3339();
        locust_core::folders::use_folder(&store, &dir, &now).unwrap();
        spec.other_repos.push((dir_s, (*repo_url).to_owned()));
    }

    let rendered_tasks = tasks_json.to_string().replace("__FOLDER__", &here);
    std::fs::write(cloud_task_service::store_path_of(&store), rendered_tasks).unwrap();

    if let Some(sessions) = claude_sessions {
        let rendered_sessions = sessions.to_string().replace("__FOLDER__", &here);
        std::fs::write(
            locust_core::claude_cloud::store_path_of(&store),
            rendered_sessions,
        )
        .unwrap();
    }

    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut s = State::default();
    s.engine = Some(native::Kind::Native);
    s.data = Some(store);
    s.cloud.make = Some(fake_claude());
    s.cloud.codex.make = Some(fake_codex_service(spec, calls.clone()));
    let mate = Teammate {
        id: "tm_codex".into(),
        name: "Chip".into(),
        role: "Coder".into(),
        runtime: Some(runtime.into()),
        model: Some(if runtime == "claude" {
            "claude-sonnet-5-5".into()
        } else {
            "gpt-5".into()
        }),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(here),
    };
    crate::apply_team(&mut s, vec![mate]);
    let mut rt = Runtime::new(s, crate::view, (1280.0, 1400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    (
        World {
            root,
            codex_calls: calls,
        },
        rt,
    )
}

fn settle(rt: &mut Runtime<State>) {
    for _ in 0..10 {
        rt.frame(0.05);
    }
}

fn click(rt: &mut Runtime<State>, key: &str) {
    settle(rt);
    let (x, y) = rt
        .find(key)
        .unwrap_or_else(|| panic!("no element {key} on screen"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn pump_by(rt: &mut Runtime<State>, what: &str, step: f64, mut done: impl FnMut(&State) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut now = rt.state.cloud.now_ms as f64 / 1000.0;
    while !done(&rt.state) {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        now += step;
        rt.update(move |s, tx| {
            crate::poll(s, now, tx);
        });
        rt.frame(0.05);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    settle(rt);
}

fn pump(rt: &mut Runtime<State>, what: &str, done: impl FnMut(&State) -> bool) {
    pump_by(rt, what, 0.3, done);
}

fn words(rt: &Runtime<State>) -> String {
    rt.elements()
        .iter()
        .filter_map(|e| e.text.clone().or(e.label.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn codex_task_rows_show_title_status_and_when_changed_newest_first_under_claudes_sessions() {
    let now = chrono::Utc::now();
    let five_min_ago = (now - chrono::Duration::minutes(5)).to_rfc3339();
    let one_hour_ago = (now - chrono::Duration::hours(1)).to_rfc3339();
    let two_hours_ago = (now - chrono::Duration::hours(2)).to_rfc3339();
    let tasks = serde_json::json!([
        {
            "taskId": "task_e_applied",
            "url": "https://chatgpt.com/codex/tasks/task_e_applied",
            "runtime": "codex",
            "repo": "acme/widgets",
            "branch": "main",
            "prompt": "Old prompt",
            "folder": "__FOLDER__",
            "createdAt": two_hours_ago,
            "appliedAt": two_hours_ago,
            "status": { "state": "applied", "title": "Clean up logging", "added": 12, "removed": 3, "files": 2 }
        },
        {
            "taskId": "task_e_pending",
            "url": "https://chatgpt.com/codex/tasks/task_e_pending",
            "runtime": "codex",
            "repo": "acme/widgets",
            "branch": "main",
            "prompt": "Speed up search index",
            "folder": "__FOLDER__",
            "createdAt": five_min_ago,
            "status": { "state": "pending" }
        },
        {
            "taskId": "task_e_ready",
            "url": "https://chatgpt.com/codex/tasks/task_e_ready",
            "runtime": "codex",
            "repo": "acme/widgets",
            "branch": "main",
            "prompt": "Fallback prompt",
            "folder": "__FOLDER__",
            "createdAt": one_hour_ago,
            "status": { "state": "ready", "title": "Fix greeting", "added": 4, "removed": 1, "files": 1 }
        }
    ]);
    let claude_sessions = serde_json::json!({
        "sessions": [{
            "id": "cc_0123456789abcdef0123",
            "startedAt": now.to_rfc3339(),
            "prompt": "Audit Claude session",
            "folder": "__FOLDER__",
            "sessionId": "session_01G8RT9yXr7dbhVM7EUzpvEa",
            "url": "https://claude.ai/code/session_01G8RT9yXr7dbhVM7EUzpvEa",
            "title": "Claude audit"
        }]
    });
    let (w, mut rt) = window("claude", FakeSpec::default(), tasks, Some(claude_sessions), &[]);
    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-cloud");
    pump(&mut rt, "Claude sessions and Codex tasks loaded", |s| {
        s.cloud
            .panel
            .claude
            .as_ref()
            .is_some_and(|c| c.sessions.len() == 1)
            && s.cloud.codex.tasks.len() == 3
    });
    let said = words(&rt);
    assert!(said.contains("Claude audit"), "{said}");
    assert!(said.contains("Speed up search index"), "{said}");
    assert!(
        said.contains("Working in the cloud · started 5 minutes ago"),
        "{said}"
    );
    assert!(said.contains("Fix greeting"), "{said}");
    assert!(
        said.lines().any(|line| line == "Ready · +4 \u{2212}1 in 1 file"),
        "singular file count: {said}"
    );
    assert!(said.contains("Clean up logging"), "{said}");
    assert!(
        said.lines()
            .any(|line| line == "Applied to this folder 2 hours ago · +12 \u{2212}3 in 2 files"),
        "{said}"
    );

    let (_, claude_y) = rt.find("cloud-session-cc_0123456789abcdef0123").unwrap();
    let (_, pending_y) = rt.find("codex-task-task_e_pending").unwrap();
    let (_, ready_y) = rt.find("codex-task-task_e_ready").unwrap();
    let (_, applied_y) = rt.find("codex-task-task_e_applied").unwrap();
    assert!(
        claude_y < pending_y,
        "Codex tasks render under Claude's sessions: claude={claude_y}, codex={pending_y}"
    );
    assert!(
        pending_y < ready_y && ready_y < applied_y,
        "Codex tasks sort newest-first: pending={pending_y}, ready={ready_y}, applied={applied_y}"
    );
    assert!(
        !w.codex_calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.get(1).map(String::as_str) == Some("apply")),
        "listing never applies a task"
    );
}

#[test]
fn pending_codex_tasks_refresh_through_cloud_task_service_on_the_windows_clock() {
    let now = chrono::Utc::now().to_rfc3339();
    let tasks = serde_json::json!([
        {
            "taskId": "task_e_pending",
            "url": "https://chatgpt.com/codex/tasks/task_e_pending",
            "runtime": "codex",
            "repo": "acme/widgets",
            "branch": "main",
            "prompt": "Fix checkout",
            "folder": "__FOLDER__",
            "createdAt": now,
            "status": { "state": "pending" }
        },
        {
            "taskId": "task_e_done",
            "url": "https://chatgpt.com/codex/tasks/task_e_done",
            "runtime": "codex",
            "repo": "acme/widgets",
            "branch": "main",
            "prompt": "Already ready",
            "folder": "__FOLDER__",
            "createdAt": now,
            "status": { "state": "ready", "title": "Already ready", "added": 1, "removed": 0, "files": 1 }
        }
    ]);
    let (w, mut rt) = window("codex", FakeSpec::default(), tasks, None, &[]);
    rt.update(|s, _| s.cloud.open = true);
    pump(&mut rt, "initial Codex tasks loaded", |s| {
        s.cloud.codex.tasks.len() == 2
    });
    assert!(words(&rt).contains("Working in the cloud · started just now"));

    // Advance the window's clock past the 15 s refresh interval.
    pump_by(&mut rt, "pending task refreshed to Ready", 5.0, |s| {
        s.cloud
            .codex
            .tasks
            .iter()
            .any(|t| t.task_id == "task_e_pending" && t.status.state == CloudState::Ready)
    });
    let said = words(&rt);
    assert!(said.contains("Fix checkout total"), "{said}");
    assert!(said.contains("Ready · +2 \u{2212}2 in 2 files"), "{said}");
    let status_calls: Vec<Vec<String>> = w
        .codex_calls
        .lock()
        .unwrap()
        .iter()
        .filter(|c| c.get(1).map(String::as_str) == Some("status"))
        .cloned()
        .collect();
    assert_eq!(status_calls, vec![vec!["cloud", "status", "task_e_pending"]]);
}

#[test]
fn empty_state_says_electrons_words_when_folder_is_on_github_with_environment() {
    let (_w, mut rt) = window("codex", FakeSpec::default(), serde_json::json!([]), None, &[]);
    rt.update(|s, _| s.cloud.open = true);
    pump(&mut rt, "Codex where loaded", |s| {
        s.cloud.codex.codex_ready == Some(true)
            && s.cloud.codex.environment == Some(CloudEnvironment::Ready)
    });
    let said = words(&rt);
    assert!(said.contains("acme/widgets · main"), "{said}");
    assert!(
        said.contains(
            "Pick Cloud in the chat-type menu and describe a task. It runs in Codex Cloud on this repository, as GitHub has it; its change stays there until you apply it here."
        ),
        "{said}"
    );
    assert!(rt.find("codex-cloud-empty").is_some());

    // Control: while Claude is picked with no Codex tasks, Codex's empty state is hidden.
    let (_w2, mut rt2) = window("claude", FakeSpec::default(), serde_json::json!([]), None, &[]);
    click(&mut rt2, "compare-mode-chip");
    click(&mut rt2, "compare-mode-cloud");
    pump(&mut rt2, "Claude panel loaded", |s| {
        s.cloud.codex.codex_ready.is_some()
    });
    assert!(rt2.find("codex-cloud-tasks").is_none());
    assert!(!words(&rt2).contains("Pick Cloud in the chat-type menu"));
}

#[test]
fn codex_not_signed_in_block_uses_electrons_words_and_skips_environment_check() {
    let spec = FakeSpec {
        codex_ready: false,
        ..FakeSpec::default()
    };
    let (w, mut rt) = window("codex", spec, serde_json::json!([]), None, &[]);
    rt.update(|s, _| s.cloud.open = true);
    pump(&mut rt, "Codex not ready loaded", |s| {
        s.cloud.codex.codex_ready == Some(false)
    });
    let said = words(&rt);
    assert!(
        said.contains(
            "Cloud tasks run through Codex CLI, which is not installed or not signed in here. Settings > AI agents shows how."
        ),
        "{said}"
    );
    assert!(
        !said.contains("Pick Cloud in the chat-type menu"),
        "empty prompt hidden while Codex is not signed in: {said}"
    );
    assert!(
        w.codex_calls.lock().unwrap().is_empty(),
        "no codex cloud calls while Codex is not signed in"
    );
}

#[test]
fn missing_codex_environment_block_uses_electrons_words_and_lists_ready_folders() {
    let spec = FakeSpec {
        missing_env_repos: vec!["acme/widgets".into()],
        ..FakeSpec::default()
    };
    let (_w, mut rt) = window(
        "codex",
        spec,
        serde_json::json!([]),
        None,
        &[("other-project", "https://github.com/acme/ready-app.git")],
    );
    rt.update(|s, _| s.cloud.open = true);
    pump(&mut rt, "missing environment loaded", |s| {
        s.cloud.codex.environment == Some(CloudEnvironment::Missing)
            && s.cloud.codex.folders.is_some()
    });
    let said = words(&rt);
    assert!(
        said.contains(
            "acme/widgets has no Codex Cloud environment yet, so a task sent from here would be refused. Make one in the Codex app: Settings > Legacy Codex Cloud, pick this repository, then Save and publish."
        ),
        "{said}"
    );
    assert!(said.contains("Or use a folder that has one:"), "{said}");
    assert!(said.contains("other-project"), "{said}");
    assert!(said.contains("acme/ready-app"), "{said}");
    assert!(said.contains("Choose a folder…"), "{said}");
    assert!(
        !said.contains("Pick Cloud in the chat-type menu"),
        "empty prompt hidden when repository has no Codex Cloud environment: {said}"
    );
}

#[test]
fn this_computers_service_reads_persisted_codex_tasks_from_the_store() {
    let root = std::env::temp_dir().join(format!(
        "locust-codex-real-store-{}-{}",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let root = root.canonicalize().unwrap();
    let work = root.join("work");
    let here = work.to_string_lossy().into_owned();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    store
        .save_json(
            "teammates.json",
            &serde_json::json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codex", "name": "Chip"}], "missionOwners": {}, "settings": {}}),
        )
        .unwrap();
    let tasks = serde_json::json!([{
        "taskId": "task_e_persisted",
        "url": "https://chatgpt.com/codex/tasks/task_e_persisted",
        "runtime": "codex",
        "repo": "acme/widgets",
        "branch": "main",
        "prompt": "Saved from Electron",
        "folder": here,
        "createdAt": chrono::Utc::now().to_rfc3339(),
        "status": { "state": "ready", "title": "Persisted Codex task", "added": 3, "removed": 1, "files": 1 }
    }]);
    std::fs::write(cloud_task_service::store_path_of(&store), tasks.to_string()).unwrap();

    // No fake service here: exercises `cloud_task_service::on_this_computer`. Because `work` has no GitHub remote
    // and the task is already Ready (not Pending), `codex` is never spawned.
    let mut s = State::default();
    s.engine = Some(native::Kind::Native);
    s.data = Some(store);
    let mate = Teammate {
        id: "tm_codex".into(),
        name: "Chip".into(),
        role: "Coder".into(),
        runtime: Some("codex".into()),
        model: Some("gpt-5".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(here),
    };
    crate::apply_team(&mut s, vec![mate]);
    let mut rt = Runtime::new(s, crate::view, (1280.0, 1400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    rt.update(|s, _| s.cloud.open = true);
    pump(&mut rt, "persisted task loaded via on_this_computer", |s| {
        s.cloud.codex.tasks.len() == 1
    });
    let said = words(&rt);
    assert!(said.contains("Persisted Codex task"), "{said}");
    assert!(said.contains("Ready · +3 \u{2212}1 in 1 file"), "{said}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn rechoosing_cloud_while_panel_is_open_refreshes_codex_tasks_from_the_service() {
    let now = chrono::Utc::now().to_rfc3339();
    let first = serde_json::json!([{
        "taskId": "task_e_first",
        "url": "https://chatgpt.com/codex/tasks/task_e_first",
        "runtime": "codex",
        "repo": "acme/widgets",
        "branch": "main",
        "prompt": "First cloud task",
        "folder": "__FOLDER__",
        "createdAt": now,
        "status": { "state": "ready", "title": "First cloud task" }
    }]);
    let (_w, mut rt) = window("claude", FakeSpec::default(), first, None, &[]);
    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-cloud");
    pump(&mut rt, "first Codex task loaded", |s| {
        s.cloud.codex.tasks.len() == 1
    });
    assert!(words(&rt).contains("First cloud task"));

    // Write a second task to the store while the panel stays open, then re-choose Cloud
    // (which calls `claude_cloud_panel::refresh` -> `cloud_tasks::refresh`).
    let store = rt.state.data.clone().unwrap();
    let here = rt.state.teammates[0].folder.clone().unwrap();
    let both = serde_json::json!([
        {
            "taskId": "task_e_first",
            "url": "https://chatgpt.com/codex/tasks/task_e_first",
            "runtime": "codex",
            "repo": "acme/widgets",
            "branch": "main",
            "prompt": "First cloud task",
            "folder": here,
            "createdAt": now,
            "status": { "state": "ready", "title": "First cloud task" }
        },
        {
            "taskId": "task_e_second",
            "url": "https://chatgpt.com/codex/tasks/task_e_second",
            "runtime": "codex",
            "repo": "acme/widgets",
            "branch": "main",
            "prompt": "Second cloud task",
            "folder": here,
            "createdAt": chrono::Utc::now().to_rfc3339(),
            "status": { "state": "ready", "title": "Second cloud task" }
        }
    ]);
    std::fs::write(cloud_task_service::store_path_of(&store), both.to_string()).unwrap();

    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-cloud");
    pump(&mut rt, "second Codex task loaded on refresh", |s| {
        s.cloud.codex.tasks.len() == 2
    });
    assert!(words(&rt).contains("Second cloud task"));
}

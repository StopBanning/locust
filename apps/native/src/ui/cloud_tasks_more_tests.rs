//! Remaining `CloudTasks.tsx` Codex behaviors in the window (row 3): Open in Codex links, Failed/Unknown rows,
//! pre-start notes, folder switching from the missing-environment block, not-on-GitHub environment ordering,
//! closing the panel clearing open diff & stopping pending refresh, multi-hunk footer, and stale diff guard.

use super::*;
use locust_core::cloud_tasks::Ran;
use rune::{Env, Runtime};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;

static TEMP_ID: AtomicU64 = AtomicU64::new(0);

const TWO_HUNK_DIFF: &str = concat!(
    "diff --git a/app.rs b/app.rs\n",
    "--- a/app.rs\n",
    "+++ b/app.rs\n",
    "@@ -1,2 +1,2 @@\n",
    "-let a = 1;\n",
    "+let a = 2;\n",
    " let keep = 0;\n",
    "@@ -20,2 +20,2 @@\n",
    "-let b = 3;\n",
    "+let b = 4;\n",
    " let end = 0;\n",
);

struct World {
    root: PathBuf,
    codex_calls: Arc<Mutex<Vec<Vec<String>>>>,
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[derive(Default)]
struct FakeRig {
    repos_by_dir: Mutex<HashMap<String, Option<String>>>,
    missing_env_repos: Mutex<Vec<String>>,
    diffs_by_id: Mutex<HashMap<String, String>>,
    diff_gates: Mutex<HashMap<String, mpsc::Receiver<()>>>,
}

fn fake_service(rig: Arc<FakeRig>, calls: Arc<Mutex<Vec<Vec<String>>>>) -> MakeCodex {
    Arc::new(move |store: &Store| {
        let file = cloud_task_service::store_path_of(store);
        let rig_codex = rig.clone();
        let rig_git = rig.clone();
        let calls = calls.clone();
        CloudTaskService::from_runners(
            file,
            move |args: &[&str], _cwd: &Path| {
                calls
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
                        if rig_codex
                            .missing_env_repos
                            .lock()
                            .unwrap()
                            .iter()
                            .any(|r| r == env_repo)
                        {
                            Ok(Ran::new(1, "", format!("environment '{env_repo}' not found")))
                        } else {
                            Ok(Ran::ok(""))
                        }
                    }
                    Some("diff") => {
                        let id = args.get(2).copied().unwrap_or("");
                        let gate = rig_codex.diff_gates.lock().unwrap().remove(id);
                        if let Some(rx) = gate {
                            let _ = rx.recv();
                        }
                        let body = rig_codex
                            .diffs_by_id
                            .lock()
                            .unwrap()
                            .get(id)
                            .cloned()
                            .unwrap_or_default();
                        Ok(Ran::ok(body))
                    }
                    _ => Ok(Ran::ok("")),
                }
            },
            move |args: &[&str], cwd: &Path| {
                let cwd_s = cwd.to_string_lossy().into_owned();
                let cmd = args.first().copied().unwrap_or("");
                if cmd == "remote" {
                    let map = rig_git.repos_by_dir.lock().unwrap();
                    return match map.get(&cwd_s).cloned().flatten() {
                        Some(url) => Ok(Ran::ok(format!("{url}\n"))),
                        None => Ok(Ran::new(2, "", "fatal: No such remote 'origin'")),
                    };
                }
                if cmd == "rev-parse" {
                    return Ok(Ran::ok("main\n"));
                }
                Ok(Ran::ok(""))
            },
        )
    })
}

fn setup(
    work_repo: Option<&str>,
    missing_envs: &[&str],
    extra_folders: &[(&str, &str)],
    tasks_json: serde_json::Value,
    bind_teammate_folder: bool,
) -> (World, Arc<FakeRig>, Vec<(String, String)>, Runtime<State>) {
    let root = std::env::temp_dir().join(format!(
        "locust-codex-more-{}-{}",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let root = root.canonicalize().unwrap();
    let here = root.join("work").to_string_lossy().into_owned();

    let rig = Arc::new(FakeRig::default());
    rig.repos_by_dir
        .lock()
        .unwrap()
        .insert(here.clone(), work_repo.map(str::to_owned));
    *rig.missing_env_repos.lock().unwrap() =
        missing_envs.iter().map(|s| (*s).to_owned()).collect();

    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    store
        .save_json(
            "teammates.json",
            &serde_json::json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codex", "name": "Chip"}], "missionOwners": {}, "settings": {}}),
        )
        .unwrap();

    let mut folder_ids = Vec::new();
    for (name, repo_url) in extra_folders {
        let dir = root.join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let dir_s = dir.to_string_lossy().into_owned();
        let now = chrono::Utc::now().to_rfc3339();
        let entry = locust_core::folders::use_folder(&store, &dir, &now).unwrap();
        folder_ids.push(((*name).to_owned(), entry.id));
        rig.repos_by_dir
            .lock()
            .unwrap()
            .insert(dir_s, Some((*repo_url).to_owned()));
    }

    std::fs::write(
        cloud_task_service::store_path_of(&store),
        tasks_json.to_string().replace("__FOLDER__", &here),
    )
    .unwrap();

    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut s = State::default();
    s.engine = Some(native::Kind::Native);
    s.data = Some(store);
    s.home.folder_path = Some(here.clone());
    s.cloud.codex.make = Some(fake_service(rig.clone(), calls.clone()));
    let mate = Teammate {
        id: "tm_codex".into(),
        name: "Chip".into(),
        role: "Coder".into(),
        runtime: Some("codex".into()),
        model: Some("gpt-5".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: bind_teammate_folder.then_some(here),
    };
    crate::apply_team(&mut s, vec![mate]);
    let mut rt = Runtime::new(s, crate::view, (1280.0, 1400.0), Env::new(true, 1.0));
    rt.frame(0.0);
    (
        World {
            root,
            codex_calls: calls,
        },
        rig,
        folder_ids,
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
fn open_in_codex_opens_each_tasks_url_including_failed_and_unknown_rows() {
    let now = chrono::Utc::now();
    let tasks = serde_json::json!([
        {
            "taskId": "task_e_failed",
            "url": "https://chatgpt.com/codex/tasks/task_e_failed",
            "runtime": "codex",
            "repo": "acme/widgets",
            "branch": "main",
            "prompt": "Broken build",
            "folder": "__FOLDER__",
            "createdAt": (now - chrono::Duration::minutes(1)).to_rfc3339(),
            "status": { "state": "failed", "title": "Broken build" }
        },
        {
            "taskId": "task_e_unknown",
            "url": "https://chatgpt.com/codex/tasks/task_e_unknown",
            "runtime": "codex",
            "repo": "acme/widgets",
            "branch": "main",
            "prompt": "Mystery task",
            "folder": "__FOLDER__",
            "createdAt": (now - chrono::Duration::minutes(2)).to_rfc3339(),
            "status": { "state": "unknown" }
        },
        {
            "taskId": "task_e_ready",
            "url": "https://chatgpt.com/codex/tasks/task_e_ready",
            "runtime": "codex",
            "repo": "acme/widgets",
            "branch": "main",
            "prompt": "Good task",
            "folder": "__FOLDER__",
            "createdAt": (now - chrono::Duration::minutes(3)).to_rfc3339(),
            "status": { "state": "ready", "title": "Good task" }
        }
    ]);
    let (_w, _rig, _ids, mut rt) = setup(
        Some("https://github.com/acme/widgets.git"),
        &[],
        &[],
        tasks,
        true,
    );
    rt.update(|s, _| s.cloud.open = true);
    pump(&mut rt, "three tasks loaded", |s| {
        s.cloud.codex.tasks.len() == 3
    });

    let said = words(&rt);
    assert!(
        said.contains("The cloud could not finish it. Open it in Codex to see why."),
        "{said}"
    );
    assert!(
        said.contains("Codex has not said how it is going yet."),
        "{said}"
    );
    // Failed and Unknown rows never offer diff or apply, only Open in Codex.
    assert!(rt.find("codex-task-diff-task_e_failed").is_none());
    assert!(rt.find("codex-task-apply-task_e_failed").is_none());
    assert!(rt.find("codex-task-diff-task_e_unknown").is_none());
    assert!(rt.find("codex-task-apply-task_e_unknown").is_none());

    click(&mut rt, "codex-task-open-task_e_failed");
    click(&mut rt, "codex-task-open-task_e_unknown");
    click(&mut rt, "codex-task-open-task_e_ready");
    assert_eq!(
        rt.state.opened_links,
        [
            "https://chatgpt.com/codex/tasks/task_e_failed",
            "https://chatgpt.com/codex/tasks/task_e_unknown",
            "https://chatgpt.com/codex/tasks/task_e_ready",
        ]
    );
}

#[test]
fn pre_start_notes_render_and_switching_to_a_ready_folder_clears_them_and_reloads() {
    let (_w, _rig, folder_ids, mut rt) = setup(
        Some("https://github.com/acme/missing-env.git"),
        &["acme/missing-env"],
        &[("ready-project", "https://github.com/acme/ready-app.git")],
        serde_json::json!([]),
        false,
    );
    let ready_id = folder_ids
        .iter()
        .find(|(name, _)| name == "ready-project")
        .unwrap()
        .1
        .clone();
    rt.update(|s, _| {
        s.cloud.codex.notes = vec![
            "2 commits on main are not on GitHub, so the cloud works without them.".into(),
            "Uncommitted changes in this folder stay here: the cloud works from what GitHub has."
                .into(),
        ];
        s.cloud.open = true;
    });
    pump(&mut rt, "missing env + folders loaded", |s| {
        s.cloud.codex.environment == Some(CloudEnvironment::Missing)
            && s.cloud.codex.folders.is_some()
    });
    let before = words(&rt);
    assert!(
        before.contains("2 commits on main are not on GitHub, so the cloud works without them."),
        "{before}"
    );
    assert!(
        before.contains(
            "Uncommitted changes in this folder stay here: the cloud works from what GitHub has."
        ),
        "{before}"
    );

    // Click Open on the ready folder offered in the missing-environment block.
    click(&mut rt, &format!("codex-cloud-env-open-{ready_id}"));
    pump(&mut rt, "switched to ready-project and reloaded", |s| {
        s.cloud
            .codex
            .place
            .as_ref()
            .and_then(|p| p.repo.as_deref())
            == Some("acme/ready-app")
            && s.cloud.codex.environment == Some(CloudEnvironment::Ready)
    });
    let after = words(&rt);
    assert!(after.contains("acme/ready-app · main"), "{after}");
    assert!(
        after.contains("Pick Cloud in the chat-type menu and describe a task."),
        "{after}"
    );
    assert!(
        !after.contains("2 commits on main are not on GitHub"),
        "old folder notes cleared on folder switch: {after}"
    );
}

#[test]
fn not_on_github_lists_ready_folders_first_and_marks_missing_environments() {
    let (_w, _rig, folder_ids, mut rt) = setup(
        None,
        &["acme/alpha-missing"],
        &[
            ("alpha-missing", "https://github.com/acme/alpha-missing.git"),
            ("beta-ready", "https://github.com/acme/beta-ready.git"),
        ],
        serde_json::json!([]),
        true,
    );
    let alpha_id = folder_ids
        .iter()
        .find(|(name, _)| name == "alpha-missing")
        .unwrap()
        .1
        .clone();
    let beta_id = folder_ids
        .iter()
        .find(|(name, _)| name == "beta-ready")
        .unwrap()
        .1
        .clone();

    rt.update(|s, _| s.cloud.open = true);
    pump(&mut rt, "not-on-github folders checked", |s| {
        s.cloud
            .codex
            .folders
            .as_ref()
            .is_some_and(|f| f.len() == 2)
    });
    let said = words(&rt);
    assert!(
        said.contains(
            "work is not on GitHub. A cloud task runs on a copy of a GitHub repository in Codex Cloud, not on this computer, so it needs a folder that is on GitHub."
        ),
        "{said}"
    );
    assert!(
        said.contains("acme/alpha-missing · no cloud environment"),
        "{said}"
    );
    assert!(
        !said.contains("acme/beta-ready · no cloud environment"),
        "ready folder must not say no cloud environment: {said}"
    );
    let (_, beta_y) = rt.find(&format!("cloud-tasks-folder-{beta_id}")).unwrap();
    let (_, alpha_y) = rt.find(&format!("cloud-tasks-folder-{alpha_id}")).unwrap();
    assert!(
        beta_y < alpha_y,
        "folders with a ready environment come first: beta={beta_y}, alpha={alpha_y}"
    );
}

#[test]
fn closing_the_panel_clears_open_diff_shows_multi_hunk_footer_and_stops_pending_refresh() {
    let now = chrono::Utc::now().to_rfc3339();
    let tasks = serde_json::json!([
        {
            "taskId": "task_e_ready",
            "url": "https://chatgpt.com/codex/tasks/task_e_ready",
            "runtime": "codex",
            "repo": "acme/widgets",
            "branch": "main",
            "prompt": "Two sections",
            "folder": "__FOLDER__",
            "createdAt": now,
            "status": { "state": "ready", "title": "Two sections", "added": 2, "removed": 2, "files": 1 }
        },
        {
            "taskId": "task_e_pending",
            "url": "https://chatgpt.com/codex/tasks/task_e_pending",
            "runtime": "codex",
            "repo": "acme/widgets",
            "branch": "main",
            "prompt": "Still pending",
            "folder": "__FOLDER__",
            "createdAt": now,
            "status": { "state": "pending" }
        }
    ]);
    let (w, rig, _ids, mut rt) = setup(
        Some("https://github.com/acme/widgets.git"),
        &[],
        &[],
        tasks,
        true,
    );
    rig.diffs_by_id
        .lock()
        .unwrap()
        .insert("task_e_ready".into(), TWO_HUNK_DIFF.into());

    rt.update(|s, _| s.cloud.open = true);
    pump(&mut rt, "tasks loaded", |s| s.cloud.codex.tasks.len() == 2);

    click(&mut rt, "codex-task-diff-task_e_ready");
    pump(&mut rt, "two-hunk diff loaded", |s| {
        s.cloud.codex.open.as_ref().is_some_and(|o| !o.loading)
    });
    let shown = words(&rt);
    assert!(shown.contains("All 2 sections shown"), "{shown}");

    // Close the panel via its Close button.
    click(&mut rt, "cloud-tasks-close");
    assert!(rt.find("cloud-tasks-beside").is_none());
    let mut ticks = 0;
    pump_by(&mut rt, "panel closed for 60s", 15.0, |_| {
        ticks += 1;
        ticks > 4
    });
    assert!(
        !w.codex_calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.get(1).map(String::as_str) == Some("status")),
        "closing the panel stops polling pending tasks"
    );

    // Reopening starts with the diff collapsed.
    rt.update(|s, _| s.cloud.open = true);
    settle(&mut rt);
    assert!(rt.find("codex-task-diff-view-task_e_ready").is_none());
}

#[test]
fn stale_diff_answer_never_overwrites_a_newly_opened_task() {
    let now = chrono::Utc::now();
    let tasks = serde_json::json!([
        {
            "taskId": "task_e_slow",
            "url": "https://chatgpt.com/codex/tasks/task_e_slow",
            "runtime": "codex",
            "repo": "acme/widgets",
            "branch": "main",
            "prompt": "Slow task",
            "folder": "__FOLDER__",
            "createdAt": (now - chrono::Duration::minutes(1)).to_rfc3339(),
            "status": { "state": "ready", "title": "Slow task" }
        },
        {
            "taskId": "task_e_fast",
            "url": "https://chatgpt.com/codex/tasks/task_e_fast",
            "runtime": "codex",
            "repo": "acme/widgets",
            "branch": "main",
            "prompt": "Fast task",
            "folder": "__FOLDER__",
            "createdAt": (now - chrono::Duration::minutes(2)).to_rfc3339(),
            "status": { "state": "ready", "title": "Fast task" }
        }
    ]);
    let (_w, rig, _ids, mut rt) = setup(
        Some("https://github.com/acme/widgets.git"),
        &[],
        &[],
        tasks,
        true,
    );
    let (tx, rx) = mpsc::channel();
    rig.diff_gates
        .lock()
        .unwrap()
        .insert("task_e_slow".into(), rx);
    rig.diffs_by_id.lock().unwrap().insert(
        "task_e_slow".into(),
        "diff --git a/slow.js b/slow.js\n--- a/slow.js\n+++ b/slow.js\n@@ -1 +1 @@\n-old\n+slow\n"
            .into(),
    );
    rig.diffs_by_id.lock().unwrap().insert(
        "task_e_fast".into(),
        "diff --git a/fast.js b/fast.js\n--- a/fast.js\n+++ b/fast.js\n@@ -1 +1 @@\n-old\n+fast\n"
            .into(),
    );

    rt.update(|s, _| s.cloud.open = true);
    pump(&mut rt, "two tasks loaded", |s| {
        s.cloud.codex.tasks.len() == 2
    });

    click(&mut rt, "codex-task-diff-task_e_slow");
    click(&mut rt, "codex-task-diff-task_e_fast");
    pump(&mut rt, "fast diff loaded", |s| {
        s.cloud
            .codex
            .open
            .as_ref()
            .is_some_and(|o| o.task_id == "task_e_fast" && !o.loading)
    });
    assert!(words(&rt).contains("fast.js"));

    // Release slow task's diff and let its answer arrive on poll.
    tx.send(()).unwrap();
    let mut turns = 0;
    pump_by(&mut rt, "slow diff drained", 0.1, |_| {
        turns += 1;
        turns > 10
    });
    let shown = words(&rt);
    assert!(shown.contains("fast.js"), "fast diff must remain: {shown}");
    assert!(
        !shown.contains("slow.js"),
        "stale diff must not overwrite fast diff: {shown}"
    );
}

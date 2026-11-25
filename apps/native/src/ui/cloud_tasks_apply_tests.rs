//! Codex's cloud task diff and apply flows in the window (`CloudTasks.tsx` row 2), drawn through the whole window
//! (`crate::view`) and driven by its poll (`crate::poll`), backed by a fake `CloudTaskService` (never a real Codex).

use super::*;
use locust_core::cloud_tasks::Ran;
use rune::{Env, Runtime};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;

static TEMP_ID: AtomicU64 = AtomicU64::new(0);

const SAMPLE_DIFF: &str = concat!(
    "diff --git a/greet.js b/greet.js\n",
    "--- a/greet.js\n",
    "+++ b/greet.js\n",
    "@@ -1,2 +1,2 @@\n",
    "-export const hi = () => 'hi'\n",
    "+export const hi = () => 'hello!'\n",
    " export const bye = () => 'bye'\n",
    "diff --git a/notes.txt b/notes.txt\n",
    "new file mode 100644\n",
    "--- /dev/null\n",
    "+++ b/notes.txt\n",
    "@@ -0,0 +1,2 @@\n",
    "+first line\n",
    "+second line\n",
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

struct FakeControls {
    diff_reply: Mutex<Ran>,
    diff_gate: Mutex<Option<mpsc::Receiver<()>>>,
    apply_replies: Mutex<VecDeque<Ran>>,
    apply_gate: Mutex<Option<mpsc::Receiver<()>>>,
}

impl FakeControls {
    fn new(diff_stdout: &str, apply_replies: Vec<Ran>) -> Arc<Self> {
        Arc::new(Self {
            diff_reply: Mutex::new(Ran::ok(diff_stdout)),
            diff_gate: Mutex::new(None),
            apply_replies: Mutex::new(apply_replies.into()),
            apply_gate: Mutex::new(None),
        })
    }
}

fn fake_service(
    controls: Arc<FakeControls>,
    calls: Arc<Mutex<Vec<Vec<String>>>>,
) -> MakeCodex {
    Arc::new(move |store: &Store| {
        let file = cloud_task_service::store_path_of(store);
        let controls = controls.clone();
        let calls = calls.clone();
        CloudTaskService::from_runners(
            file,
            move |args: &[&str], _cwd: &Path| {
                calls
                    .lock()
                    .unwrap()
                    .push(args.iter().map(|s| (*s).to_owned()).collect());
                match args.get(1).copied() {
                    Some("list") => Ok(Ran::ok("")),
                    Some("diff") => {
                        let gate = controls.diff_gate.lock().unwrap().take();
                        if let Some(rx) = gate {
                            let _ = rx.recv();
                        }
                        Ok(controls.diff_reply.lock().unwrap().clone())
                    }
                    Some("apply") => {
                        let gate = controls.apply_gate.lock().unwrap().take();
                        if let Some(rx) = gate {
                            let _ = rx.recv();
                        }
                        Ok(controls
                            .apply_replies
                            .lock()
                            .unwrap()
                            .pop_front()
                            .unwrap_or_else(|| Ran::ok("Applied.\n")))
                    }
                    _ => Ok(Ran::ok("")),
                }
            },
            move |args: &[&str], _cwd: &Path| {
                Ok(match args.first().copied() {
                    Some("remote") => Ran::ok("https://github.com/acme/widgets.git\n"),
                    Some("rev-parse") => Ran::ok("main\n"),
                    _ => Ran::ok(""),
                })
            },
        )
    })
}

fn window(
    tasks_json: serde_json::Value,
    controls: Arc<FakeControls>,
) -> (World, Runtime<State>) {
    let root = std::env::temp_dir().join(format!(
        "locust-codex-apply-{}-{}",
        std::process::id(),
        TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("electron/mission-ledger")).unwrap();
    std::fs::create_dir_all(root.join("work")).unwrap();
    let root = root.canonicalize().unwrap();
    let here = root.join("work").to_string_lossy().into_owned();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    store
        .save_json(
            "teammates.json",
            &serde_json::json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_codex", "name": "Chip"}], "missionOwners": {}, "settings": {}}),
        )
        .unwrap();
    std::fs::write(
        cloud_task_service::store_path_of(&store),
        tasks_json.to_string().replace("__FOLDER__", &here),
    )
    .unwrap();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut s = State::default();
    s.engine = Some(native::Kind::Native);
    s.data = Some(store);
    s.cloud.codex.make = Some(fake_service(controls, calls.clone()));
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

fn pump(rt: &mut Runtime<State>, what: &str, mut done: impl FnMut(&State) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let mut now = rt.state.cloud.now_ms as f64 / 1000.0;
    while !done(&rt.state) {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        now += 0.3;
        rt.update(move |s, tx| {
            crate::poll(s, now, tx);
        });
        rt.frame(0.05);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    settle(rt);
}

fn words(rt: &Runtime<State>) -> String {
    rt.elements()
        .iter()
        .filter_map(|e| e.text.clone().or(e.label.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn mixed_tasks() -> serde_json::Value {
    let now = chrono::Utc::now();
    serde_json::json!([
        {
            "taskId": "task_e_ready",
            "url": "https://chatgpt.com/codex/tasks/task_e_ready",
            "runtime": "codex",
            "repo": "acme/widgets",
            "branch": "main",
            "prompt": "Update greeting",
            "folder": "__FOLDER__",
            "createdAt": (now - chrono::Duration::minutes(2)).to_rfc3339(),
            "status": { "state": "ready", "title": "Update greeting", "added": 3, "removed": 1, "files": 2 }
        },
        {
            "taskId": "task_e_applied",
            "url": "https://chatgpt.com/codex/tasks/task_e_applied",
            "runtime": "codex",
            "repo": "acme/widgets",
            "branch": "main",
            "prompt": "Applied earlier",
            "folder": "__FOLDER__",
            "createdAt": (now - chrono::Duration::minutes(10)).to_rfc3339(),
            "appliedAt": (now - chrono::Duration::minutes(8)).to_rfc3339(),
            "status": { "state": "applied", "title": "Applied earlier", "added": 1, "removed": 0, "files": 1 }
        },
        {
            "taskId": "task_e_pending",
            "url": "https://chatgpt.com/codex/tasks/task_e_pending",
            "runtime": "codex",
            "repo": "acme/widgets",
            "branch": "main",
            "prompt": "Still working",
            "folder": "__FOLDER__",
            "createdAt": now.to_rfc3339(),
            "status": { "state": "pending" }
        }
    ])
}

#[test]
fn show_the_change_loads_and_renders_the_tasks_diff_and_hides_on_second_click() {
    let controls = FakeControls::new(SAMPLE_DIFF, vec![]);
    let (tx, rx) = mpsc::channel();
    *controls.diff_gate.lock().unwrap() = Some(rx);

    let (w, mut rt) = window(mixed_tasks(), controls);
    rt.update(|s, _| s.cloud.open = true);
    pump(&mut rt, "tasks loaded", |s| s.cloud.codex.tasks.len() == 3);

    // Offered for Ready and Applied, never Pending.
    assert!(rt.find("codex-task-diff-task_e_ready").is_some());
    assert!(rt.find("codex-task-diff-task_e_applied").is_some());
    assert!(rt.find("codex-task-diff-task_e_pending").is_none());
    assert!(
        !w.codex_calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| matches!(c.get(1).map(String::as_str), Some("diff" | "apply"))),
        "nothing fetched or applied before the person clicks"
    );

    click(&mut rt, "codex-task-diff-task_e_ready");
    let loading = words(&rt);
    assert!(loading.contains("Reading the change…"), "{loading}");
    assert!(loading.contains("Hide the change"), "{loading}");

    tx.send(()).unwrap();
    pump(&mut rt, "diff loaded", |s| {
        s.cloud.codex.open.as_ref().is_some_and(|o| !o.loading)
    });
    let shown = words(&rt);
    assert!(shown.contains("greet.js"), "{shown}");
    assert!(shown.contains("+1 \u{2212}1"), "{shown}");
    assert!(shown.contains("-export const hi = () => 'hi'"), "{shown}");
    assert!(shown.contains("+export const hi = () => 'hello!'"), "{shown}");
    assert!(shown.contains("The whole change is shown"), "{shown}");
    assert!(shown.contains("notes.txt"), "{shown}");
    assert!(shown.lines().any(|line| line == "added"), "non-MODIFIED status shown lowercase: {shown}");
    assert!(shown.contains("+first line"), "{shown}");
    let diff_calls = |calls: &Mutex<Vec<Vec<String>>>| -> Vec<Vec<String>> {
        calls
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.get(1).map(String::as_str) == Some("diff"))
            .cloned()
            .collect()
    };
    assert_eq!(diff_calls(&w.codex_calls), vec![vec!["cloud", "diff", "task_e_ready"]]);
    assert!(
        !w.codex_calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.get(1).map(String::as_str) == Some("apply")),
        "showing the change never applies it"
    );

    // Clicking Hide the change collapses the diff without re-fetching.
    click(&mut rt, "codex-task-diff-task_e_ready");
    assert!(rt.find("codex-task-diff-view-task_e_ready").is_none());
    assert!(!words(&rt).contains("Hide the change"));
    assert_eq!(diff_calls(&w.codex_calls).len(), 1);

    // Applied tasks can also show their change.
    click(&mut rt, "codex-task-diff-task_e_applied");
    pump(&mut rt, "applied task diff loaded", |s| {
        s.cloud
            .codex
            .open
            .as_ref()
            .is_some_and(|o| o.task_id == "task_e_applied" && !o.loading)
    });
    assert!(rt.find("codex-task-diff-view-task_e_applied").is_some());
}

#[test]
fn show_the_change_says_when_codex_returns_no_diff() {
    let controls = FakeControls::new("", vec![]);
    let (_w, mut rt) = window(mixed_tasks(), controls);
    rt.update(|s, _| s.cloud.open = true);
    pump(&mut rt, "tasks loaded", |s| s.cloud.codex.tasks.len() == 3);

    click(&mut rt, "codex-task-diff-task_e_ready");
    pump(&mut rt, "empty diff finished", |s| {
        s.cloud.codex.open.as_ref().is_some_and(|o| !o.loading)
    });
    assert!(
        words(&rt).contains("Codex did not return a change for it."),
        "{}",
        words(&rt)
    );
}

#[test]
fn apply_to_this_folder_runs_only_on_click_guards_reentry_and_updates_row() {
    let controls = FakeControls::new(SAMPLE_DIFF, vec![Ran::ok("Applied.\n")]);
    let (tx, rx) = mpsc::channel();
    *controls.apply_gate.lock().unwrap() = Some(rx);

    let (w, mut rt) = window(mixed_tasks(), controls);
    rt.update(|s, _| s.cloud.open = true);
    pump(&mut rt, "tasks loaded", |s| s.cloud.codex.tasks.len() == 3);

    // Offered only for Ready, never Pending or Applied.
    assert!(rt.find("codex-task-apply-task_e_ready").is_some());
    assert!(rt.find("codex-task-apply-task_e_applied").is_none());
    assert!(rt.find("codex-task-apply-task_e_pending").is_none());

    // Opening the diff does not apply.
    click(&mut rt, "codex-task-diff-task_e_ready");
    pump(&mut rt, "diff loaded", |s| {
        s.cloud.codex.open.as_ref().is_some_and(|o| !o.loading)
    });
    assert!(
        !w.codex_calls
            .lock()
            .unwrap()
            .iter()
            .any(|c| c.get(1).map(String::as_str) == Some("apply")),
        "never applies without clicking Apply to this folder"
    );

    // Click Apply to this folder: shows Applying… and ignores a second click while in flight.
    click(&mut rt, "codex-task-apply-task_e_ready");
    assert!(words(&rt).contains("Applying…"), "{}", words(&rt));
    click(&mut rt, "codex-task-apply-task_e_ready");

    tx.send(()).unwrap();
    pump(&mut rt, "apply finished", |s| {
        s.cloud.codex.applying.is_none()
            && s.cloud
                .codex
                .tasks
                .iter()
                .any(|t| t.task_id == "task_e_ready" && t.status.state == CloudState::Applied)
    });

    let apply_calls: Vec<Vec<String>> = w
        .codex_calls
        .lock()
        .unwrap()
        .iter()
        .filter(|c| c.get(1).map(String::as_str) == Some("apply"))
        .cloned()
        .collect();
    assert_eq!(apply_calls, vec![vec!["cloud", "apply", "task_e_ready"]]);

    let said = words(&rt);
    assert!(
        said.contains("Applied to this folder just now · +3 \u{2212}1 in 2 files"),
        "{said}"
    );
    assert!(
        rt.find("codex-task-apply-task_e_ready").is_none(),
        "Apply button disappears once applied"
    );
    assert!(
        rt.find("codex-task-diff-task_e_ready").is_some(),
        "Show the change remains available after apply"
    );
}

#[test]
fn failed_apply_says_the_problem_in_the_panel_and_a_later_apply_clears_it() {
    let controls = FakeControls::new(
        SAMPLE_DIFF,
        vec![
            Ran::new(1, "", "conflict in greet.js"),
            Ran::ok("Applied.\n"),
        ],
    );
    let (_w, mut rt) = window(mixed_tasks(), controls);
    rt.update(|s, _| s.cloud.open = true);
    pump(&mut rt, "tasks loaded", |s| s.cloud.codex.tasks.len() == 3);

    click(&mut rt, "codex-task-apply-task_e_ready");
    pump(&mut rt, "failed apply finished", |s| {
        s.cloud.codex.applying.is_none() && s.cloud.codex.problem.is_some()
    });
    let failed = words(&rt);
    assert!(
        failed.contains(
            "The change could not be applied here: conflict in greet.js. Nothing in the folder was changed by Locust."
        ),
        "{failed}"
    );
    assert!(rt.find("codex-cloud-problem").is_some());
    assert!(
        rt.find("codex-task-apply-task_e_ready").is_some(),
        "task stays Ready after failed apply"
    );

    // Retry succeeds and clears the problem alert from the panel.
    click(&mut rt, "codex-task-apply-task_e_ready");
    pump(&mut rt, "retry apply succeeded", |s| {
        s.cloud.codex.applying.is_none()
            && s.cloud
                .codex
                .tasks
                .iter()
                .any(|t| t.task_id == "task_e_ready" && t.status.state == CloudState::Applied)
    });
    assert!(rt.find("codex-cloud-problem").is_none());
    assert!(!words(&rt).contains("conflict in greet.js"));
}

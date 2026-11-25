use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::cloud_tasks::{CloudState, Ran};

use super::{CloudApplyResult, CloudStartInput, CloudStartResult, CloudTaskService};

fn ran(stdout: &str, code: i32, stderr: &str) -> Ran {
    Ran::new(code, stdout, stderr)
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "{prefix}-{}-{n}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn run_with_setup<F>(git_overrides: HashMap<&'static str, Ran>, body: F)
where
    F: FnOnce(&CloudTaskService<'_>, &RefCell<Vec<Vec<String>>>),
{
    let dir = TempDir::new("locust-cloud-svc");
    let calls: RefCell<Vec<Vec<String>>> = RefCell::new(Vec::new());
    let codex = |args: &[&str], _cwd: &Path| -> Result<Ran, String> {
        calls
            .borrow_mut()
            .push(args.iter().map(|s| (*s).to_string()).collect());
        Ok(match args.get(1).copied() {
            Some("exec") => ran("https://chatgpt.com/codex/tasks/task_e_abc123\n", 0, ""),
            Some("status") => ran(
                "[READY] Update greeting in greet.js acme/widgets  •  1m ago +2/-2 • 2 files\n",
                0,
                "",
            ),
            Some("diff") => ran(
                "diff --git a/greet.js b/greet.js\n--- a/greet.js\n+++ b/greet.js\n@@ -1 +1 @@\n-a\n+b\n",
                0,
                "",
            ),
            _ => ran("Applied.\n", 0, ""),
        })
    };
    let git = |args: &[&str], _cwd: &Path| -> Result<Ran, String> {
        let cmd = args.first().copied().unwrap_or("");
        if let Some(over) = git_overrides.get(cmd) {
            return Ok(over.clone());
        }
        Ok(match cmd {
            "remote" => ran("https://github.com/acme/widgets.git\n", 0, ""),
            "rev-parse" => ran("main\n", 0, ""),
            "rev-list" => ran("0\n", 0, ""),
            _ => ran("", 0, ""),
        })
    };
    let service = CloudTaskService::new(dir.path.join("cloud-tasks.json"), &codex, &git)
        .with_now(|| "2026-09-30T12:00:00.000Z".into());
    body(&service, &calls);
}

#[test]
// case: starts on the folder's own repository and branch, and is kept
fn starts_on_the_folders_own_repository_and_branch_and_is_kept() {
    run_with_setup(HashMap::new(), |service, calls| {
        let started = service.start(CloudStartInput {
            folder: "/w".into(),
            prompt: "Make the greeting louder.".into(),
            teammate_id: None,
        });
        match started {
            CloudStartResult::Ok { task, notes } => {
                assert_eq!(task.task_id, "task_e_abc123");
                assert_eq!(task.repo, "acme/widgets");
                assert_eq!(task.branch.as_deref(), Some("main"));
                assert!(notes.is_empty());
            }
            CloudStartResult::Err { message } => panic!("expected ok, got {message}"),
        }
        assert_eq!(
            calls.borrow()[0],
            vec![
                "cloud",
                "exec",
                "--env",
                "acme/widgets",
                "--branch",
                "main",
                "Make the greeting louder.",
            ]
        );
        let ids: Vec<String> = service
            .list(Some("/w"))
            .into_iter()
            .map(|t| t.task_id)
            .collect();
        assert_eq!(ids, vec!["task_e_abc123"]);
    });
}

#[test]
// case: says, before it runs, what the cloud will not see
fn says_before_it_runs_what_the_cloud_will_not_see() {
    let mut git = HashMap::new();
    git.insert("rev-list", ran("2\n", 0, ""));
    git.insert("status", ran(" M greet.js\n", 0, ""));
    run_with_setup(git, |service, _calls| {
        let started = service.start(CloudStartInput {
            folder: "/w".into(),
            prompt: "x".into(),
            teammate_id: None,
        });
        match started {
            CloudStartResult::Ok { notes, .. } => {
                assert_eq!(
                    notes,
                    vec![
                        "2 commits on main are not on GitHub, so the cloud works without them.",
                        "Uncommitted changes in this folder stay here: the cloud works from what GitHub has.",
                    ]
                );
            }
            CloudStartResult::Err { message } => panic!("expected ok, got {message}"),
        }
    });
}

#[test]
// case: refuses a folder with no GitHub repository, and starts nothing
fn refuses_a_folder_with_no_github_repository_and_starts_nothing() {
    let mut git = HashMap::new();
    git.insert("remote", ran("", 2, "fatal: No such remote"));
    run_with_setup(git, |service, calls| {
        let started = service.start(CloudStartInput {
            folder: "/w".into(),
            prompt: "x".into(),
            teammate_id: None,
        });
        match started {
            CloudStartResult::Err { message } => {
                assert!(message.contains("GitHub repository, and this folder has none"));
            }
            CloudStartResult::Ok { .. } => panic!("expected refusal"),
        }
        assert!(calls.borrow().is_empty());
    });
}

#[test]
// case: is followed to READY, its change fetched, and applied only when asked
fn is_followed_to_ready_its_change_fetched_and_applied_only_when_asked() {
    run_with_setup(HashMap::new(), |service, calls| {
        let _ = service.start(CloudStartInput {
            folder: "/w".into(),
            prompt: "x".into(),
            teammate_id: None,
        });
        let refreshed = service.refresh("task_e_abc123").expect("refreshed");
        assert_eq!(refreshed.status.state, CloudState::Ready);
        assert_eq!(refreshed.status.added, Some(2));
        assert_eq!(refreshed.status.removed, Some(2));
        assert_eq!(refreshed.status.files, Some(2));

        let diff = service.diff("task_e_abc123").expect("diff");
        assert!(diff.starts_with("diff --git a/greet.js"));
        assert!(!calls.borrow().iter().any(|c| c.get(1).map(String::as_str) == Some("apply")));

        match service.apply("task_e_abc123") {
            CloudApplyResult::Ok { task } => {
                assert_eq!(task.applied_at.as_deref(), Some("2026-09-30T12:00:00.000Z"));
                assert_eq!(task.status.state, CloudState::Applied);
            }
            CloudApplyResult::Err { message } => panic!("expected ok, got {message}"),
        }
        assert_eq!(
            service.apply("task_e_abc123"),
            CloudApplyResult::Err {
                message: "Its change is already in this folder.".into(),
            }
        );
    });
}

#[test]
// case: never applies a task it did not start
fn never_applies_a_task_it_did_not_start() {
    run_with_setup(HashMap::new(), |service, calls| {
        assert!(matches!(
            service.apply("task_e_someone_elses"),
            CloudApplyResult::Err { .. }
        ));
        assert!(calls.borrow().is_empty());
    });
}

#[test]
fn from_runners_reports_codex_readiness_and_environment() {
    use crate::cloud_tasks::CloudEnvironment;
    let dir = TempDir::new("locust-cloud-svc-env");
    let store = crate::store::Store::new(dir.path.join("native"), dir.path.join("electron")).unwrap();
    assert_eq!(
        super::store_path_of(&store),
        store.root().join("cloud-tasks.json")
    );
    let svc = CloudTaskService::from_runners(
        dir.path.join("cloud-tasks.json"),
        |args: &[&str], _cwd: &Path| {
            if args == ["cloud", "list", "--env", "acme/missing", "--limit", "1"] {
                Ok(ran("", 1, "environment 'acme/missing' not found"))
            } else {
                Ok(ran("", 0, ""))
            }
        },
        |_args: &[&str], _cwd: &Path| Ok(ran("", 0, "")),
    )
    .with_ready(|| false);
    assert!(!svc.codex_ready());
    assert_eq!(svc.environment("acme/ready", &dir.path), CloudEnvironment::Ready);
    assert_eq!(svc.environment("acme/missing", &dir.path), CloudEnvironment::Missing);
}

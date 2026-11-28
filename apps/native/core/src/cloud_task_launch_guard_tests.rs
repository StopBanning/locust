use super::*;
use std::cell::Cell;

struct Home(PathBuf);
impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// case: gates every separate background and cloud launch before calling its service
#[test]
fn codex_cloud_prioritizes_organization_refusal_and_never_launches_in_a_free_only_window() {
    let home = Home(std::env::temp_dir().join(format!(
        "locust-cloud-guard-{}",
        crate::invariants::unique_nanos()
    )));
    let path = home.0.join("cloud-tasks.json");
    let git = Cell::new(0);
    let codex = Cell::new(0);
    let task = CloudTaskService::from_runners(
        &path,
        |_, _| {
            codex.set(codex.get() + 1);
            Ok(Ran::new(
                0,
                "https://chatgpt.com/codex/tasks/task_e_abc123\n",
                "",
            ))
        },
        |args, _| {
            git.set(git.get() + 1);
            Ok(match args.first().copied() {
                Some("remote") => Ran::new(0, "https://github.com/acme/widgets.git\n", ""),
                Some("rev-parse") => Ran::new(0, "main\n", ""),
                Some("rev-list") => Ran::new(0, "0\n", ""),
                _ => Ran::new(0, "", ""),
            })
        },
    );
    let input = || CloudStartInput {
        folder: "/project".into(),
        prompt: "Review the workspace.".into(),
        teammate_id: None,
    };
    let install = |policy: Value| {
        crate::org_policy::use_policy_on_this_thread(Some(crate::org_policy::PolicyState {
            path: "/test/organization-policy.json".into(),
            present: true,
            policy: policy.as_object().unwrap().clone(),
            error: None,
        }));
    };
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            crate::org_policy::use_policy_on_this_thread(None);
        }
    }
    let _reset = Reset;
    {
        let _held = crate::model_lists::hold_free_routes_for_test(true);
        install(json!({"cloud":false}));
        assert_eq!(task.start(input()), CloudStartResult::Err {
            message: "Cloud computers and cloud tasks are off. Set by your organization (cloud in /test/organization-policy.json). Nothing was started.".into(),
        });
        install(json!({}));
        assert_eq!(task.start(input()), CloudStartResult::Err {
            message: "This window was opened by a test drive that may only use free routes, and this route is not one. Nothing was started.".into(),
        });
        assert_eq!((git.get(), codex.get()), (0, 0));
        assert!(!path.exists());
    }
    let _allowed = crate::model_lists::hold_free_routes_for_test(false);
    assert!(matches!(task.start(input()), CloudStartResult::Ok { .. }));
    assert_eq!(codex.get(), 1);
    assert!(git.get() > 0);
    assert!(path.exists());
}

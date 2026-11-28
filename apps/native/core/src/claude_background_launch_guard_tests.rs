use super::*;
use std::cell::Cell;

struct Home(PathBuf);
impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[derive(Default)]
struct FakeHost {
    looked_up: Cell<usize>,
    ran: Cell<usize>,
}
impl BackgroundHost for &FakeHost {
    fn run_claude(&self, args: &[String], _cwd: &Path, _account: Option<&str>) -> ClaudeCliOutput {
        self.ran.set(self.ran.get() + 1);
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--permission-mode", "bypassPermissions"])
        );
        ClaudeCliOutput {
            code: 0,
            stdout: "backgrounded · abc123456\n".into(),
            stderr: String::new(),
        }
    }
    fn session_of(&self, _conversation: &str, _account: Option<&str>) -> Option<String> {
        self.looked_up.set(self.looked_up.get() + 1);
        None
    }
    fn bring_in(&self, _run: &BackgroundRun) -> BringInOutcome {
        panic!("starting a run must not import anything");
    }
}

#[test]
fn background_refuses_free_only_and_requires_the_saved_auto_opt_in_before_touching_claude() {
    let home = Home(std::env::temp_dir().join(format!(
        "locust-background-guard-{}",
        crate::invariants::unique_nanos()
    )));
    let store = crate::store::Store::new(home.0.join("native"), home.0.join("electron")).unwrap();
    fs::create_dir_all(store.root()).unwrap();
    let _engine = crate::native::hooks::install(store.clone(), None);
    crate::org_policy::use_policy_on_this_thread(Some(crate::org_policy::PolicyState {
        path: "/test/organization-policy.json".into(),
        present: true,
        policy: Default::default(),
        error: None,
    }));
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            crate::org_policy::use_policy_on_this_thread(None);
        }
    }
    let _reset = Reset;
    let path = store.root().join("claude-background.json");
    let host = FakeHost::default();
    let mut service = BackgroundRuns::new(path.clone(), &host);
    let request = || StartBackgroundRequest {
        prompt: "Review the workspace.".into(),
        folder: home.0.to_string_lossy().into_owned(),
        sandbox: BackgroundSandbox::FullAccess,
        ask_each: false,
        conversation: Some("conversation".into()),
        teammate_id: None,
        model: Some("haiku".into()),
        effort: None,
        account: None,
    };
    let mut assert_refusal = |why: &str| {
        match service.start(request()) {
            StartBackgroundOutcome::Refused {
                message,
                needs_trust,
            } => {
                assert_eq!(message, why);
                assert!(!needs_trust);
            }
            other => panic!("expected refusal, got {other:?}"),
        }
        assert_eq!(host.looked_up.get(), 0);
        assert_eq!(host.ran.get(), 0);
        assert!(!path.exists());
        assert!(service.watching().is_empty());
    };
    {
        let _held = crate::model_lists::hold_free_routes_for_test(true);
        assert_refusal(
            "This window spends nothing, and a background turn runs Claude Code on your plan.",
        );
    }
    let _unheld = crate::model_lists::hold_free_routes_for_test(false);
    assert_refusal("Auto is off in Settings, so a background turn cannot use it.");

    store
        .save_json(
            "teammates.json",
            &json!({"schemaVersion":1, "teammates":[], "settings":{"autoMode":true}}),
        )
        .unwrap();
    let started = service.start(request());
    assert!(
        matches!(started, StartBackgroundOutcome::Started(_)),
        "{started:?}"
    );
    assert_eq!(host.looked_up.get(), 1);
    assert_eq!(host.ran.get(), 1);
    assert!(path.exists());
}

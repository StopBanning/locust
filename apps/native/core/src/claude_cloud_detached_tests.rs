use super::*;
use std::cell::RefCell;

const ID: &str = "cc_abcdef";
const SESSION: &str = "session_0123456789";
const HERE: &str = "locust-cloud-abcdef-here";

struct World {
    root: PathBuf,
    repo: PathBuf,
    store: PathBuf,
}
impl World {
    fn new(session: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "locust-cloud-detached-{}-{}",
            std::process::id(),
            crate::invariants::unique_nanos()
        ));
        let repo = root.join("checkout");
        fs::create_dir_all(&repo).unwrap();
        let repo = repo.canonicalize().unwrap();
        let store = root.join("sessions.json");
        fs::write(
            &store,
            json!({"sessions":[{"id":ID, "folder":repo, "prompt":"Test",
            "startedAt":"2026-10-03T00:00:00Z", "sessionId":session}]})
            .to_string(),
        )
        .unwrap();
        Self { root, repo, store }
    }
    fn git(&self, args: &[&str]) -> GitRan {
        git_run(args, self.repo.to_str().unwrap(), None)
    }
    fn init(&self) {
        for args in [
            &["init", "-q", "-b", "main"][..],
            &["config", "user.email", "test@example.com"],
            &["config", "user.name", "Test"],
        ] {
            assert_eq!(self.git(args).code, 0);
        }
        fs::write(self.repo.join("cart.txt"), "committed").unwrap();
        assert_eq!(self.git(&["add", "cart.txt"]).code, 0);
        assert_eq!(self.git(&["commit", "-qm", "Fixture"]).code, 0);
    }
    fn place(&self) -> PathBuf {
        self.repo.join(".claude/worktrees").join(HERE)
    }
    fn cloud(&self, platform: CloudPlatform) -> ClaudeCloud<'_> {
        ClaudeCloud::new(&self.store, platform, || {
            Some(ClaudeLaunch {
                discovered_path: "C:/fake/claude.exe".into(),
                executable_path: "C:/fake/claude.exe".into(),
                prefix_args: vec![],
                env: None,
            })
        })
        .with_process_env(Vec::new)
    }
    fn fake_git(&self, args: &[&str]) -> GitRan {
        GitRan {
            code: 0,
            stdout: if args[0] == "rev-parse" {
                self.repo.join(".git").to_string_lossy().into_owned()
            } else {
                String::new()
            },
            stderr: String::new(),
        }
    }
}
impl Drop for World {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

// case: A cloud terminal teleports the validated id instead of resuming a local session.
#[test]
fn claude_cloud_copy_teleports_the_valid_id() {
    let w = World::new(SESSION);
    let windows = RefCell::new(Vec::new());
    let cloud = w
        .cloud(CloudPlatform::Darwin)
        .with_git(|args, _, _| w.fake_git(args))
        .with_run_window(|run| {
            windows.borrow_mut().push(run.clone());
            Ok(())
        });
    assert_eq!(cloud.continue_in_terminal(ID), CloudSendResult::Ok);
    let got = windows.borrow();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].args, ["--teleport", SESSION]);
}

// case: A cloud terminal refuses the invalid session id %j.
#[test]
fn claude_cloud_copy_rejects_each_malformed_id() {
    for session in ["", "--cloud", "a & calc", "%PATH%", "a;b", "a\nb"] {
        let w = World::new(session);
        let cloud = w
            .cloud(CloudPlatform::Win32)
            .with_git(|_, _, _| panic!("invalid id reached Git"))
            .with_run_window(|_| panic!("invalid id reached terminal"));
        assert!(matches!(
            cloud.continue_in_terminal(ID),
            CloudSendResult::Err { .. }
        ));
        assert!(!w.place().exists());
    }
}

// case: A cloud terminal uses Windows Terminal when available (%s) and otherwise a console.
#[test]
fn claude_cloud_copy_prefers_windows_terminal_and_falls_back() {
    for available in [true, false] {
        let w = World::new(SESSION);
        let seen = RefCell::new(Vec::new());
        let cloud = w
            .cloud(CloudPlatform::Win32)
            .with_git(|args, _, _| w.fake_git(args))
            .with_run_window(|run| {
                let mut seen = seen.borrow_mut();
                seen.push(run.clone());
                if seen.len() == 1 && !available {
                    Err("missing".into())
                } else {
                    Ok(())
                }
            });
        assert_eq!(cloud.continue_in_terminal(ID), CloudSendResult::Ok);
        let got = seen.borrow();
        assert_eq!(got.len(), if available { 1 } else { 2 });
        assert!(got[0].file.ends_with("wt.exe"));
        assert_eq!(
            &got[0].args[got[0].args.len() - 3..],
            ["C:/fake/claude.exe", "--teleport", SESSION]
        );
        if !available {
            assert_eq!(got[1].file, "cmd.exe");
            assert_eq!(got[1].cwd, w.place().to_str().unwrap());
            assert_eq!(
                got[1].args,
                [format!(
                    "/d /k \"\"C:/fake/claude.exe\" --teleport {SESSION}\""
                )]
            );
        }
    }
}

// case: Continuing a cloud session creates a detached worktree without touching its folder or reading worktree.
#[test]
fn claude_cloud_copy_uses_a_distinct_detached_git_destination() {
    let w = World::new(SESSION);
    let calls = RefCell::new(Vec::new());
    let windows = RefCell::new(Vec::new());
    let folder = w.repo.to_str().unwrap();
    let cloud = w
        .cloud(CloudPlatform::Win32)
        .with_git(|args, from, _| {
            calls.borrow_mut().push((
                args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
                from.to_owned(),
            ));
            w.fake_git(args)
        })
        .with_run_window(|run| {
            windows.borrow_mut().push(run.clone());
            Ok(())
        });
    assert_eq!(cloud.continue_in_terminal(ID), CloudSendResult::Ok);
    let destination = w.place();
    assert_eq!(
        calls.borrow()[1],
        (
            vec![
                "worktree".to_owned(),
                "add".into(),
                "--detach".into(),
                destination.to_str().unwrap().into(),
                "HEAD".into()
            ],
            folder.into()
        )
    );
    assert_eq!(windows.borrow()[0].cwd, destination.to_str().unwrap());
}

// case: An invalid stored cloud id creates no worktree and opens nothing.
#[test]
fn claude_cloud_bad_stored_id_opens_nothing() {
    let w = World::new("a & calc");
    let cloud = w
        .cloud(CloudPlatform::Win32)
        .with_git(|_, _, _| panic!("bad stored id reached Git"))
        .with_run_window(|_| panic!("bad stored id reached terminal"));
    assert_eq!(
        cloud.continue_in_terminal(ID),
        CloudSendResult::Err {
            message:
                "Locust does not know a valid id for this cloud session. Open it on claude.ai."
                    .into()
        }
    );
    assert!(!w.repo.join(".claude").exists());
}

// case: A worktree creation failure opens no terminal and preserves any earlier copy.
#[test]
fn claude_cloud_add_failure_never_opens_or_removes_an_earlier_copy() {
    let w = World::new(SESSION);
    let cloud = w
        .cloud(CloudPlatform::Win32)
        .with_git(|args, _, _| {
            assert!(!args.contains(&"remove"));
            let mut ran = w.fake_git(args);
            if args[0] == "worktree" {
                ran.code = 128;
            }
            ran
        })
        .with_run_window(|_| panic!("failed worktree reached terminal"));
    assert!(matches!(
        cloud.continue_in_terminal(ID),
        CloudSendResult::Err { .. }
    ));
    fs::create_dir_all(w.place()).unwrap();
    fs::write(w.place().join("my-work.txt"), "the earlier copy").unwrap();
    assert!(matches!(
        cloud.continue_in_terminal(ID),
        CloudSendResult::Err { .. }
    ));
    assert_eq!(
        fs::read_to_string(w.place().join("my-work.txt")).unwrap(),
        "the earlier copy"
    );
}

// case: A terminal launch failure removes only the newly created clean worktree without force.
#[test]
fn claude_cloud_terminal_failure_only_removes_the_clean_new_copy() {
    for dirty in [false, true] {
        let w = World::new(SESSION);
        w.init();
        let calls = RefCell::new(Vec::new());
        let cloud = w
            .cloud(CloudPlatform::Darwin)
            .with_git(|args, cwd, input| {
                calls
                    .borrow_mut()
                    .push(args.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>());
                git_run(args, cwd, input)
            })
            .with_run_window(|run| {
                if dirty {
                    fs::write(Path::new(&run.cwd).join("notes.txt"), "keep this").unwrap();
                }
                Err("Not opened".into())
            });
        assert!(matches!(
            cloud.continue_in_terminal(ID),
            CloudSendResult::Err { .. }
        ));
        assert_eq!(
            calls.borrow().last().unwrap(),
            &["worktree", "remove", w.place().to_str().unwrap()]
        );
        if dirty {
            assert_eq!(
                fs::read_to_string(w.place().join("notes.txt")).unwrap(),
                "keep this"
            );
        } else {
            assert!(!w.place().exists());
        }
    }
}

// case: Real git makes a fresh terminal checkout while the original folder keeps its uncommitted work.
#[test]
fn claude_cloud_real_git_preserves_dirty_original_and_reading_copy() {
    let w = World::new(SESSION);
    w.init();
    let read = w.repo.join(".claude/worktrees/locust-cloud-abcdef");
    assert_eq!(
        w.git(&[
            "worktree",
            "add",
            "--detach",
            read.to_str().unwrap(),
            "HEAD"
        ])
        .code,
        0
    );
    fs::write(read.join("cart.txt"), "the separate reading").unwrap();
    fs::write(w.repo.join("cart.txt"), "uncommitted").unwrap();
    let cloud = w.cloud(CloudPlatform::Darwin).with_run_window(|run| {
        assert_eq!(run.cwd, w.place().to_str().unwrap());
        assert_eq!(
            fs::read_to_string(Path::new(&run.cwd).join("cart.txt")).unwrap(),
            "committed"
        );
        assert_ne!(
            git_run(&["symbolic-ref", "-q", "HEAD"], &run.cwd, None).code,
            0
        );
        Ok(())
    });
    assert_eq!(cloud.continue_in_terminal(ID), CloudSendResult::Ok);
    assert_eq!(
        fs::read_to_string(w.repo.join("cart.txt")).unwrap(),
        "uncommitted"
    );
    assert_eq!(
        fs::read_to_string(read.join("cart.txt")).unwrap(),
        "the separate reading"
    );
    assert!(w.git(&["worktree", "list"]).stdout.contains(HERE));
}

// An empty directory may also belong to the person. Git would adopt it if asked.
#[test]
fn claude_cloud_copy_refuses_an_existing_empty_path_before_git_add() {
    let w = World::new(SESSION);
    fs::create_dir_all(w.place()).unwrap();
    let cloud = w
        .cloud(CloudPlatform::Darwin)
        .with_git(|args, _, _| {
            assert_ne!(
                args[0], "worktree",
                "Git must not adopt an existing directory"
            );
            w.fake_git(args)
        })
        .with_run_window(|_| panic!("existing path reached the terminal"));
    assert!(matches!(
        cloud.continue_in_terminal(ID),
        CloudSendResult::Err { .. }
    ));
    assert!(w.place().is_dir());
}

// A refused terminal must not create a checkout even when the session ID is valid.
#[test]
fn claude_cloud_copy_checks_terminal_policy_before_git() {
    let w = World::new(SESSION);
    for (key, value) in [
        ("terminal", json!(false)),
        ("cloud", json!(false)),
        ("monthlyLimitUsd", json!(5)),
        ("allowedAgents", json!(["codex"])),
    ] {
        let mut cloud = w
            .cloud(CloudPlatform::Darwin)
            .with_git(|_, _, _| panic!("refused terminal reached Git"))
            .with_run_window(|_| panic!("refused terminal was opened"));
        cloud.detached_policy = Some(Box::new(move || crate::org_policy::PolicyState {
            path: "/company/policy.json".into(),
            present: true,
            policy: [(key.to_owned(), value.clone())].into_iter().collect(),
            error: None,
        }));
        let CloudSendResult::Err { message } = cloud.continue_in_terminal(ID) else {
            panic!("the policy must refuse");
        };
        assert!(
            message.contains(key),
            "{message:?} must name the refused setting {key}"
        );
        assert!(!w.repo.join(".claude").exists());
    }
}

#[cfg(unix)]
#[test]
fn claude_cloud_copy_will_not_follow_a_redirected_worktrees_directory() {
    let w = World::new(SESSION);
    let elsewhere = w.root.join("different");
    fs::create_dir(&elsewhere).unwrap();
    fs::create_dir(w.repo.join(".claude")).unwrap();
    std::os::unix::fs::symlink(&elsewhere, w.repo.join(".claude/worktrees")).unwrap();
    let cloud = w
        .cloud(CloudPlatform::Darwin)
        .with_git(|args, _, _| {
            assert_ne!(args[0], "worktree");
            w.fake_git(args)
        })
        .with_run_window(|_| panic!("redirected path reached the terminal"));
    assert!(matches!(
        cloud.continue_in_terminal(ID),
        CloudSendResult::Err { .. }
    ));
    assert_eq!(fs::read_dir(elsewhere).unwrap().count(), 0);
}

// Native local IDs are padded to 20 bytes. Their first 12 characters are often identical.
#[test]
fn claude_cloud_long_local_ids_keep_distinct_copies_and_ambiguous_ids_never_launch() {
    let w = World::new(SESSION);
    let a = "cc_00000000000000000001";
    let b = "cc_00000000000000000002";
    let saved = |second: &str| {
        fs::write(&w.store, json!({"sessions":[
            {"id":a,"folder":w.repo,"prompt":"A","startedAt":"2026-10-03T00:00:00Z","sessionId":SESSION},
            {"id":second,"folder":w.repo,"prompt":"B","startedAt":"2026-10-03T00:00:00Z","sessionId":"session_0987654321"}
        ]}).to_string()).unwrap();
    };
    saved(b);
    let windows = RefCell::new(Vec::new());
    let cloud = w
        .cloud(CloudPlatform::Darwin)
        .with_git(|args, _, _| w.fake_git(args))
        .with_run_window(|run| {
            windows.borrow_mut().push(run.clone());
            Ok(())
        });
    assert_eq!(cloud.continue_in_terminal(a), CloudSendResult::Ok);
    assert_eq!(cloud.continue_in_terminal(b), CloudSendResult::Ok);
    let got = windows.borrow();
    assert!(
        got[0]
            .cwd
            .ends_with("locust-cloud-00000000000000000001-here")
    );
    assert!(
        got[1]
            .cwd
            .ends_with("locust-cloud-00000000000000000002-here")
    );
    saved(a);
    assert!(matches!(
        cloud.continue_in_terminal(a),
        CloudSendResult::Err { .. }
    ));
    assert_eq!(got.len(), 2);
}

use super::*;

#[test]
fn headless_run_refuses_a_paid_route_before_looking_for_its_folder() {
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
    let _held = crate::model_lists::hold_free_routes_for_test(true);
    // This missing folder also guarantees that neither branch can start a real provider.
    let folder = std::env::temp_dir().join(format!(
        "locust-absent-folder-{}",
        crate::invariants::unique_nanos()
    ));
    assert!(!folder.exists());
    let run_in_missing_folder = |runtime: &str, model: &str| {
        run(&[
            runtime.into(),
            "Review the workspace.".into(),
            "--folder".into(),
            folder.to_string_lossy().into_owned(),
            "--model".into(),
            model.into(),
        ])
        .unwrap_err()
        .to_string()
    };
    let paid = "This window was opened by a test drive that may only use free routes, and this route is not one. Nothing was started.";
    assert_eq!(run_in_missing_folder("codex", "gpt-5.4"), paid);
    assert_eq!(
        run_in_missing_folder("acp", "acp_fixture::opencode/genuinely-free"),
        paid
    );
    assert_eq!(
        run_in_missing_folder("opencode", "fixture/paid-free-pro"),
        paid
    );
    // A missing provider half is not a model the driver can select.
    assert_eq!(run_in_missing_folder("opencode", "/genuinely-free"), paid);
    assert_ne!(
        run_in_missing_folder("opencode", "fixture/genuinely-free"),
        paid
    );
    assert_ne!(run_in_missing_folder("kilo", "kilo/kilo-auto/free"), paid);
}

#[cfg(unix)]
#[test]
fn headless_acp_refuses_a_named_model_that_the_saved_agent_cannot_select() {
    use std::{fs, os::unix::fs::PermissionsExt, process::Command};
    const CHILD: &str = "LOCUST_B21_ACP_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        match run(&[
            "acp".into(),
            "Review the workspace.".into(),
            "--model".into(),
            "acp_fixture::allowed-model".into(),
        ]) {
            Ok(code) => println!("ACP_EXIT: {code}"),
            Err(error) => println!("ACP_REFUSAL: {error}"),
        }
        return;
    }
    struct Home(PathBuf);
    impl Drop for Home {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let home = Home(std::env::temp_dir().join(format!(
        "locust-cli-acp-guard-{}",
        crate::invariants::unique_nanos()
    )));
    let config = home.0.join("config");
    let profile = if cfg!(target_os = "macos") {
        home.0.join("Library/Application Support/@teammate/desktop")
    } else {
        config.join("@teammate/desktop")
    };
    let work = home.0.join("work");
    let native = home.0.join("native");
    for folder in [&profile, &work, &native] {
        fs::create_dir_all(folder).unwrap();
    }
    let agent = work.join("fixture-agent");
    fs::write(&agent, "#!/bin/sh\nprintf 'started\\n' > invoked\n").unwrap();
    fs::set_permissions(&agent, fs::Permissions::from_mode(0o700)).unwrap();
    // A saved agent without answer.modelOption has no way to select a model.
    fs::write(profile.join("acp-agents.json"), json!({
        "agents": [{"id": "acp_fixture", "name": "Fixture", "commandLine": format!("\"{}\"", agent.display())}]
    }).to_string()).unwrap();
    let policy = home.0.join("policy.json");
    fs::write(&policy, r#"{"allowedModels":{"acp":["allowed-model"]}}"#).unwrap();
    let launch = || {
        Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "run_cli::free_route_tests::headless_acp_refuses_a_named_model_that_the_saved_agent_cannot_select", "--nocapture"])
        .env_clear()
        .env(CHILD, "1")
        .env("HOME", &home.0)
        .env("XDG_CONFIG_HOME", &config)
        .env("LOCUST_NATIVE_DATA", &native)
        .env("LOCUST_POLICY_FILE", &policy)
        .env("LOCUST_TEST_ROOT", &home.0)
        .env("TMPDIR", &home.0)
        .current_dir(&work)
        .output().unwrap()
    };
    let child = launch();
    let stdout = String::from_utf8_lossy(&child.stdout);
    let stderr = String::from_utf8_lossy(&child.stderr);
    assert!(child.status.success(), "{stdout}\n{stderr}");
    // Electron's finite lists require an explicit model. This driver cannot set one.
    assert!(
        stdout.contains(&format!("ACP_REFUSAL: The acp model account default is not allowed. Pick an allowed, explicit model. Set by your organization (allowedModels in {}). Nothing was started.", policy.display())),
        "{stdout}\n{stderr}"
    );
    assert!(!work.join("invoked").exists());
    assert!(!native.join("mission-ledger").exists());

    let python = Command::new("python3")
        .args(["-c", "import sys; print(sys.executable)"])
        .output()
        .unwrap();
    assert!(python.status.success());
    let python = String::from_utf8(python.stdout).unwrap();
    let script = r#"
import json, pathlib, sys
for line in sys.stdin:
    call = json.loads(line)
    method = call['method']
    if method == 'initialize':
        result = {'protocolVersion': 1, 'agentCapabilities': {}}
    elif method == 'session/new':
        result = {'sessionId': 'session-1', 'configOptions': []}
        if pathlib.Path('offer-model').exists():
            result['configOptions'] = [{'id': 'model', 'currentValue': 'allowed-model'}]
    elif method == 'session/prompt':
        pathlib.Path('prompted').touch()
        result = {'stopReason': 'end_turn'}
    else:
        raise AssertionError(method)
    print(json.dumps({'jsonrpc': '2.0', 'id': call['id'], 'result': result}), flush=True)
"#;
    fs::write(&agent, format!("#!{}\n{script}", python.trim())).unwrap();
    fs::write(profile.join("acp-agents.json"), json!({
        "agents": [{"id": "acp_fixture", "name": "Fixture", "commandLine": format!("\"{}\"", agent.display()),
            "answer": {"modelOption": {"id": "model"}}}]
    }).to_string()).unwrap();

    // A saved selector is still not enough if the current ACP session does not offer it.
    let child = launch();
    let stdout = String::from_utf8_lossy(&child.stdout);
    let stderr = String::from_utf8_lossy(&child.stderr);
    assert!(child.status.success(), "{stdout}\n{stderr}");
    assert!(stdout.contains("ACP_EXIT: 1"), "{stdout}\n{stderr}");
    assert!(
        stderr
            .contains("The agent did not offer the required model setting, so it was not started."),
        "{stdout}\n{stderr}"
    );
    assert!(!work.join("prompted").exists());

    fs::write(work.join("offer-model"), "").unwrap();
    let child = launch();
    let stdout = String::from_utf8_lossy(&child.stdout);
    let stderr = String::from_utf8_lossy(&child.stderr);
    assert!(child.status.success(), "{stdout}\n{stderr}");
    assert!(stdout.contains("ACP_EXIT: 0"), "{stdout}\n{stderr}");
    assert!(work.join("prompted").exists());
}

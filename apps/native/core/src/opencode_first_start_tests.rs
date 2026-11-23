//! Electron `apps/desktop/src/main/opencode-sets-itself-up-before-the-first-message.test.ts` (7 cases).
use super::*;
use crate::start_timing::{Phase, shown_start_phase, start_phase_label};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    sync::mpsc,
};

fn folder_path() -> PathBuf {
    PathBuf::from("C:").join("home").join(".config").join("opencode")
}

fn marker_path() -> PathBuf {
    folder_path().join("node_modules").join("@opencode-ai").join("plugin")
}

fn launch() -> SetupLaunch {
    let mut env = BTreeMap::new();
    env.insert("ELECTRON_RUN_AS_NODE".into(), "1".into());
    SetupLaunch {
        executable_path: "C:\\Locust\\Locust.exe".into(),
        prefix_args: vec!["C:\\profile\\npm\\node_modules\\opencode-ai\\bin\\opencode".into()],
        env,
    }
}

struct Harness {
    first_start: Arc<OpenCodeFirstStart>,
    ran: Arc<Mutex<Vec<SetupCommand>>>,
    started_rx: mpsc::Receiver<()>,
    release_tx: mpsc::Sender<()>,
}

fn harness(set_up: bool, installs: bool, found: bool) -> Harness {
    let files = Arc::new(Mutex::new(HashSet::<PathBuf>::new()));
    if set_up {
        files.lock().unwrap().insert(marker_path());
    }
    let ran = Arc::new(Mutex::new(Vec::<SetupCommand>::new()));
    let (started_tx, started_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let release_rx = Arc::new(Mutex::new(release_rx));

    let files_run = files.clone();
    let files_exists = files.clone();
    let ran_run = ran.clone();

    let first_start = Arc::new(OpenCodeFirstStart::new(
        move || found.then(launch),
        || Some("file:///C:/profile/opencode-command-guard.js".into()),
        move |cmd| {
            ran_run.lock().unwrap().push(cmd);
            let _ = started_tx.send(());
            let _ = release_rx.lock().unwrap().recv_timeout(Duration::from_secs(5));
            if installs {
                files_run.lock().unwrap().insert(marker_path());
            }
            Some(SetupResult {
                exit_code: Some(0),
                timed_out: false,
                stdout: "{}".into(),
                stderr: String::new(),
            })
        },
        folder_path,
        move |p| files_exists.lock().unwrap().contains(p),
    ));

    Harness {
        first_start,
        ran,
        started_rx,
        release_tx,
    }
}

// case: runs debug config once, naming the guard plugin, through the launch it was found with
#[test]
fn runs_debug_config_once_naming_the_guard_plugin_through_the_launch_it_was_found_with() {
    let h = harness(false, true, true);
    assert!(h.first_start.will_set_up());
    let fs = h.first_start.clone();
    let handle = std::thread::spawn(move || fs.ensure());
    h.started_rx.recv_timeout(Duration::from_secs(5)).expect("setup started");
    {
        let ran = h.ran.lock().unwrap();
        assert_eq!(ran.len(), 1);
        assert_eq!(ran[0].executable_path, "C:\\Locust\\Locust.exe");
        assert_eq!(
            ran[0].args,
            vec![
                "C:\\profile\\npm\\node_modules\\opencode-ai\\bin\\opencode",
                "debug",
                "config",
            ]
        );
        assert_eq!(ran[0].env.get("ELECTRON_RUN_AS_NODE").map(String::as_str), Some("1"));
        let config: Value =
            serde_json::from_str(ran[0].env.get("OPENCODE_CONFIG_CONTENT").unwrap()).unwrap();
        assert_eq!(
            config,
            json!({"plugin": ["file:///C:/profile/opencode-command-guard.js"]})
        );
        assert_eq!(ran[0].timeout, OPENCODE_SETUP_TIMEOUT);
    }
    let _ = h.release_tx.send(());
    assert_eq!(handle.join().unwrap(), OpenCodeSetUpOutcome::SetUp);
    assert!(!h.first_start.will_set_up());
}

// case: does nothing where OpenCode is already set up
#[test]
fn does_nothing_where_opencode_is_already_set_up() {
    let h = harness(true, true, true);
    assert_eq!(h.first_start.ensure(), OpenCodeSetUpOutcome::Already);
    assert!(!h.first_start.will_set_up());
    assert!(h.ran.lock().unwrap().is_empty());
}

// case: shares one run between calls made while it is going
#[test]
fn shares_one_run_between_calls_made_while_it_is_going() {
    let h = harness(false, true, true);
    let fs1 = h.first_start.clone();
    let fs2 = h.first_start.clone();
    let first = std::thread::spawn(move || fs1.ensure());
    h.started_rx.recv_timeout(Duration::from_secs(5)).expect("first call running");
    let second = std::thread::spawn(move || fs2.ensure());
    // Give the second caller a moment to attach to the in-flight run before releasing.
    std::thread::sleep(Duration::from_millis(30));
    assert_eq!(h.ran.lock().unwrap().len(), 1);
    let _ = h.release_tx.send(());
    assert_eq!(first.join().unwrap(), OpenCodeSetUpOutcome::SetUp);
    assert_eq!(second.join().unwrap(), OpenCodeSetUpOutcome::SetUp);
    assert_eq!(h.ran.lock().unwrap().len(), 1);
}

// case: tries once a launch when the install does not land
#[test]
fn tries_once_a_launch_when_the_install_does_not_land() {
    let h = harness(false, false, true);
    let fs = h.first_start.clone();
    let first = std::thread::spawn(move || fs.ensure());
    h.started_rx.recv_timeout(Duration::from_secs(5)).expect("setup started");
    let _ = h.release_tx.send(());
    assert_eq!(first.join().unwrap(), OpenCodeSetUpOutcome::Failed);
    assert_eq!(h.first_start.ensure(), OpenCodeSetUpOutcome::Tried);
    assert_eq!(h.ran.lock().unwrap().len(), 1);
    assert!(h.first_start.will_set_up());
}

// case: runs nothing when OpenCode is not found
#[test]
fn runs_nothing_when_opencode_is_not_found() {
    let h = harness(false, true, false);
    assert_eq!(h.first_start.ensure(), OpenCodeSetUpOutcome::NotFound);
    assert!(h.ran.lock().unwrap().is_empty());
}

// case: is XDG_CONFIG_HOME when set, else .config under home
#[test]
fn is_xdg_config_home_when_set_else_dot_config_under_home() {
    let xdg = PathBuf::from("D:").join("xdg");
    let home = PathBuf::from("C:").join("home");
    assert_eq!(
        opencode_config_folder(Some(xdg.to_str().unwrap()), &home),
        xdg.join("opencode")
    );
    assert_eq!(
        opencode_config_folder(None, &home),
        home.join(".config").join("opencode")
    );
    assert_eq!(
        opencode_config_folder(Some(""), &home),
        home.join(".config").join("opencode")
    );
    let marker = marker_path();
    assert!(opencode_is_set_up(&folder_path(), |p| p == marker));
    assert!(!opencode_is_set_up(&folder_path(), |_| false));
}

// case: says the agent is setting itself up, and roughly how long
#[test]
fn says_the_agent_is_setting_itself_up_and_roughly_how_long() {
    assert_eq!(
        start_phase_label(Some(Phase::RuntimeFirstSetup), "OpenCode"),
        "Setting up OpenCode for its first start, about a minute and a half"
    );
    assert_eq!(
        start_phase_label(Some(Phase::StartingRuntime), "OpenCode"),
        "Starting OpenCode"
    );
    assert_eq!(
        shown_start_phase(Phase::Looking, true),
        Phase::RuntimeFirstSetup
    );
    assert_eq!(
        shown_start_phase(Phase::StartingRuntime, true),
        Phase::RuntimeFirstSetup
    );
    assert_eq!(
        shown_start_phase(Phase::Briefing, true),
        Phase::Briefing
    );
    assert_eq!(
        shown_start_phase(Phase::StartingRuntime, false),
        Phase::StartingRuntime
    );
}

fn found(id: &str, version: &str, ready: bool) -> crate::agents::Found {
    crate::agents::Found {
        id: id.into(),
        display_name: id.into(),
        installed: true,
        version: Some(version.into()),
        auth: crate::agents::Auth::NotApplicable,
        ready,
        status: if ready { crate::agents::Probe::Ready } else { crate::agents::Probe::ProbeFailed },
        unsupported_note: None,
        through_cli: false,
        checking: false,
    }
}

#[test]
fn a_discovery_answer_sets_up_only_a_ready_opencode_1_when_agents_are_shown() {
    assert!(wants_set_up(&[found("codex", "0.5.0", true), found("opencode", "1.19.2", true)], false));
    assert!(!wants_set_up(&[found("opencode", "1.19.2", true)], true), "hidden agents set nothing up");
    assert!(!wants_set_up(&[found("opencode", "2.0.1", true)], false), "OpenCode 2 has no plugin install");
    assert!(!wants_set_up(&[found("opencode", "1.19.2", false)], false), "not ready");
    assert!(!wants_set_up(&[found("cursor", "1.0.0", true)], false), "another agent");
}

#[test]
fn after_discovery_runs_the_set_up_in_the_background() {
    let ran = Arc::new(Mutex::new(Vec::<SetupCommand>::new()));
    let kept = ran.clone();
    let first_start = Arc::new(OpenCodeFirstStart::new(
        launch_some,
        || Some("file:///guard.js".into()),
        move |command| {
            kept.lock().unwrap().push(command);
            None
        },
        folder_path,
        |_| false,
    ));
    assert!(!after_discovery(&[found("opencode", "2.0.1", true)], &first_start));
    assert!(after_discovery(&[found("opencode", "1.19.2", true)], &first_start));
    let until = std::time::Instant::now() + Duration::from_secs(10);
    while ran.lock().unwrap().is_empty() && std::time::Instant::now() < until {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(ran.lock().unwrap().len(), 1);
}

fn launch_some() -> Option<SetupLaunch> {
    Some(launch())
}

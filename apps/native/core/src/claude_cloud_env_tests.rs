use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;

use super::{
    ClaudeCloud, ClaudeCloudAnswer, ClaudeLaunch, CloudAction, CloudChoice, CloudPlatform,
    PseudoTerminalResult, PseudoTerminalRun, WindowRun, WindowsCommandOptions, choice_args,
    windows_command_line,
};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("{prefix}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create temp dir");
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn make_cloud<'a>(
    store_path: PathBuf,
    terminal_lines: &'a RefCell<Vec<String>>,
    window_calls: &'a RefCell<usize>,
) -> ClaudeCloud<'a> {
    ClaudeCloud::new(store_path, CloudPlatform::Win32, || {
        Some(ClaudeLaunch {
            discovered_path: "fake-claude".into(),
            executable_path: "fake-claude".into(),
            prefix_args: Vec::new(),
            env: None,
        })
    })
    .with_terminal(move |run: &PseudoTerminalRun| {
        terminal_lines.borrow_mut().push(run.line.clone());
        PseudoTerminalResult::Ok {
            drawn: "View: https://claude.ai/code/session_0123456789".into(),
        }
    })
    .with_run_window(move |_run: &WindowRun| {
        *window_calls.borrow_mut() += 1;
        Err("A window must not open".into())
    })
}

#[test]
// case: A task carries an optional organization environment alongside the selected model and effort.
fn a_task_carries_an_optional_organization_environment_alongside_the_selected_model_and_effort() {
    let choice = CloudChoice {
        model: Some("haiku".into()),
        effort: Some("low".into()),
        environment: Some("ccpool_team-1".into()),
    };
    assert_eq!(
        choice_args(Some(&choice)).unwrap(),
        vec![
            "--model",
            "haiku",
            "--effort",
            "low",
            "--environment",
            "ccpool_team-1"
        ]
    );
    assert_eq!(choice_args(None).unwrap(), Vec::<String>::new());
    let line = windows_command_line(
        "fake",
        &[],
        CloudAction::Start,
        &WindowsCommandOptions {
            choice: Some(&choice),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(line.contains("--environment ccpool_team-1 --cloud"));

    let dir = TempDir::new("locust-cloud-env");
    let root_str = dir.path.to_string_lossy().to_string();
    let terminal_lines = RefCell::new(Vec::new());
    let window_calls = RefCell::new(0);
    let cloud = make_cloud(dir.path.join("sessions.json"), &terminal_lines, &window_calls);
    let answer = cloud.start(&root_str, "Test", None, Some(&choice));
    assert!(matches!(answer, ClaudeCloudAnswer::Ok { .. }));
    assert!(
        terminal_lines.borrow()[0]
            .contains("--model haiku --effort low --environment ccpool_team-1 --cloud")
    );
    assert_eq!(*window_calls.borrow(), 0);
}

#[test]
// case: An invalid environment id %j is refused before any launch.
fn an_invalid_environment_id_j_is_refused_before_any_launch() {
    let too_long = format!("ccpool_{}", "x".repeat(81));
    let invalid = [
        "",
        "ccpool_",
        "pool_team",
        "ccpool_a & calc",
        "ccpool_a;b",
        "ccpool_a\nb",
        &too_long,
    ];
    for env in invalid {
        let choice = CloudChoice {
            environment: Some(env.into()),
            ..Default::default()
        };
        let err = choice_args(Some(&choice)).unwrap_err();
        assert!(err.contains("environment ID"), "unexpected error: {err}");

        let dir = TempDir::new("locust-cloud-env-invalid");
        let root_str = dir.path.to_string_lossy().to_string();
        let terminal_lines = RefCell::new(Vec::new());
        let window_calls = RefCell::new(0);
        let store_path = dir.path.join("sessions.json");
        let cloud = make_cloud(store_path.clone(), &terminal_lines, &window_calls);
        let answer = cloud.start(&root_str, "Test", None, Some(&choice));
        assert!(matches!(answer, ClaudeCloudAnswer::Err { .. }));
        assert!(terminal_lines.borrow().is_empty());
        assert_eq!(*window_calls.borrow(), 0);
        assert_eq!(cloud.environment(&root_str), None);
        assert!(!Path::new(&format!("{}.environments.json", store_path.display())).exists());
    }
}

#[test]
// case: A folder remembers its environment across reloads while other folders keep their own choices.
fn a_folder_remembers_its_environment_across_reloads_while_other_folders_keep_their_own_choices() {
    let dir = TempDir::new("locust-cloud-env-reload");
    let store_path = dir.path.join("sessions.json");
    let terminal_lines = RefCell::new(Vec::new());
    let window_calls = RefCell::new(0);
    let cloud = make_cloud(store_path.clone(), &terminal_lines, &window_calls);
    let _ = cloud.start(
        "folder-one",
        "Test",
        None,
        Some(&CloudChoice {
            environment: Some("ccpool_one".into()),
            ..Default::default()
        }),
    );
    let _ = cloud.start(
        "folder-two",
        "Test",
        None,
        Some(&CloudChoice {
            environment: Some("ccpool_two".into()),
            ..Default::default()
        }),
    );

    let reloaded = make_cloud(store_path, &terminal_lines, &window_calls);
    assert_eq!(reloaded.environment("folder-one").as_deref(), Some("ccpool_one"));
    assert_eq!(reloaded.environment("folder-two").as_deref(), Some("ccpool_two"));
    assert_eq!(reloaded.environment("other-folder"), None);

    let _ = reloaded.start("folder-one", "Test", None, None);
    assert_eq!(reloaded.environment("folder-one"), None);
    assert_eq!(reloaded.environment("folder-two").as_deref(), Some("ccpool_two"));
}

#[test]
// case: A task without an environment keeps the default cloud arguments and opens no window.
fn a_task_without_an_environment_keeps_the_default_cloud_arguments_and_opens_no_window() {
    let dir = TempDir::new("locust-cloud-env-default");
    let root_str = dir.path.to_string_lossy().to_string();
    let terminal_lines = RefCell::new(Vec::new());
    let window_calls = RefCell::new(0);
    let cloud = make_cloud(dir.path.join("sessions.json"), &terminal_lines, &window_calls);
    assert!(matches!(
        cloud.start(&root_str, "Test", None, None),
        ClaudeCloudAnswer::Ok { .. }
    ));
    assert_eq!(*window_calls.borrow(), 0);
    assert!(!terminal_lines.borrow()[0].contains("--environment"));
    let line = windows_command_line(
        "fake",
        &[],
        CloudAction::Start,
        &WindowsCommandOptions::default(),
    )
    .unwrap();
    assert!(!line.contains("--environment"));
}

#[test]
// case: An invalid saved environment is ignored instead of becoming a command argument.
fn an_invalid_saved_environment_is_ignored_instead_of_becoming_a_command_argument() {
    let dir = TempDir::new("locust-cloud-env-saved-bad");
    let store_path = dir.path.join("sessions.json");
    let root_str = dir.path.to_string_lossy().to_string();
    let env_file = format!("{}.environments.json", store_path.display());
    std::fs::write(
        &env_file,
        json!([{ "folder": root_str, "id": "ccpool_a & calc" }]).to_string(),
    )
    .expect("write bad env");
    let terminal_lines = RefCell::new(Vec::new());
    let window_calls = RefCell::new(0);
    let cloud = make_cloud(store_path, &terminal_lines, &window_calls);
    assert_eq!(cloud.environment(&root_str), None);
}

//! Codex Cloud send from the composer (`Composer.tsx` CHAT_MODES `cloud`, 0.503; `App.tsx` `startCloudTask`):
//! driven through the whole window (`crate::view` + `crate::poll`) with a fake `CloudTaskService`, never a real Codex.

use super::*;
use locust_core::cloud_tasks::Ran;
use rune::{Env, Key, Runtime};
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
struct SendSpec {
    repo_url: Option<String>,
    branch: Option<String>,
    dirty: bool,
    codex_ready: bool,
    exec_reply: Ran,
    /// `codex cloud list --env` says the repository has no environment (the panel's check before a send).
    env_missing: bool,
}

impl Default for SendSpec {
    fn default() -> Self {
        Self {
            repo_url: Some("https://github.com/acme/widgets.git".into()),
            branch: Some("main".into()),
            dirty: false,
            codex_ready: true,
            exec_reply: Ran::ok("https://chatgpt.com/codex/tasks/task_e_9911\n"),
            env_missing: false,
        }
    }
}

fn fake_service(spec: SendSpec, calls: Arc<Mutex<Vec<Vec<String>>>>) -> MakeCodex {
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
                    Some("exec") => Ok(spec_codex.exec_reply.clone()),
                    Some("list") if spec_codex.env_missing => Ok(Ran::new(1, "", "Error: environment 'acme/widgets' not found; run `codex cloud` to list available environments")),
                    Some("list") => Ok(Ran::ok("")),
                    Some("status") => Ok(Ran::ok("")),
                    _ => Ok(Ran::ok("")),
                }
            },
            move |args: &[&str], _cwd: &Path| {
                let cmd = args.first().copied().unwrap_or("");
                if cmd == "remote" {
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
                if cmd == "status" {
                    return if spec_git.dirty {
                        Ok(Ran::ok(" M src/lib.rs\n"))
                    } else {
                        Ok(Ran::ok(""))
                    };
                }
                Ok(Ran::ok(""))
            },
        )
        .with_ready(move || ready)
    })
}

fn mate(id: &str, name: &str, runtime: &str, folder: &str) -> Teammate {
    Teammate {
        id: id.into(),
        name: name.into(),
        role: "Coder".into(),
        runtime: Some(runtime.into()),
        model: Some("gpt-5-codex".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(folder.into()),
    }
}

fn window(spec: SendSpec) -> (World, Runtime<State>) {
    let root = std::env::temp_dir().join(format!(
        "locust-cloud5-send-{}-{}",
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
            &serde_json::json!({
                "schemaVersion": 1,
                "teammates": [
                    {"teammateId": "tm_codex", "name": "Chip"},
                    {"teammateId": "tm_other", "name": "Wren"}
                ],
                "missionOwners": {},
                "settings": {}
            }),
        )
        .unwrap();

    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut s = State::default();
    s.engine = Some(native::Kind::Native);
    s.data = Some(store);
    s.home.folder_path = Some(here.clone());
    s.cloud.codex.make = Some(fake_service(spec, calls.clone()));
    crate::apply_team(
        &mut s,
        vec![
            mate("tm_codex", "Chip", "codex", &here),
            mate("tm_other", "Wren", "claude", &here),
        ],
    );
    let mut rt = Runtime::new(s, crate::view, (1280.0, 820.0), Env::new(true, 1.0));
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
        .unwrap_or_else(|| panic!("no element keyed {key}"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    settle(rt);
}

fn type_and_send(rt: &mut Runtime<State>, prompt: &str) {
    click(rt, "composer");
    rt.paste(prompt);
    rt.frame(0.016);
    rt.key(Key::Enter.into());
    settle(rt);
}

fn pump(rt: &mut Runtime<State>, what: &str, mut done: impl FnMut(&State) -> bool) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let mut now = 0.0;
    while !done(&rt.state) {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        rt.update(move |s, tx| {
            crate::poll(s, now, tx);
        });
        rt.frame(0.05);
        now += 0.3;
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

#[test]
fn chat_type_menu_offers_codex_cloud_and_choosing_it_opens_panel_without_org_env_field() {
    let (_world, mut rt) = window(SendSpec::default());
    assert!(rt.find("cloud-tasks-beside").is_none());

    click(&mut rt, "compare-mode-chip");
    let menu = words(&rt);
    assert!(
        menu.contains("Runs in Codex Cloud on this repository, on the model Codex Cloud chooses"),
        "expected Codex Cloud description in chat-type menu: {menu}"
    );

    click(&mut rt, "compare-mode-cloud");
    assert!(rt.state.cloud.codex.on, "Codex Cloud mode should be on");
    assert!(!rt.state.cloud.on, "Claude Cloud mode should stay off");
    assert!(
        rt.find("cloud-tasks-beside").is_some(),
        "choosing Cloud opens the panel beside the conversation"
    );
    assert!(
        words(&rt).contains("Comparison: Cloud"),
        "chip should say Cloud"
    );
    assert_eq!(
        crate::composer_wiring::composer_placeholder(&rt.state),
        "Describe a task for Codex Cloud…"
    );
    assert!(
        rt.find("cloud-environment").is_none(),
        "Codex Cloud must not show Claude's organization environment field"
    );

    // Switching back to Direct turns Codex Cloud off.
    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-direct");
    assert!(!rt.state.cloud.codex.on);
    assert!(words(&rt).contains("Comparison: Direct"));
}

#[test]
fn send_with_codex_cloud_on_starts_a_codex_cloud_task_instead_of_a_turn() {
    let (world, mut rt) = window(SendSpec {
        dirty: true,
        ..SendSpec::default()
    });
    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-cloud");
    pump(&mut rt, "codex panel loaded", |s| {
        s.cloud.codex.place.is_some()
    });

    type_and_send(&mut rt, "Tighten checkout validation");

    let calls = world.codex_calls.lock().unwrap().clone();
    let exec_call = calls
        .iter()
        .find(|args| args.get(1).map(String::as_str) == Some("exec"))
        .expect("expected a `codex cloud exec` call");
    assert_eq!(
        exec_call,
        &vec![
            "cloud".to_string(),
            "exec".to_string(),
            "--env".to_string(),
            "acme/widgets".to_string(),
            "--branch".to_string(),
            "main".to_string(),
            "Tighten checkout validation".to_string(),
        ]
    );

    let s = &rt.state;
    assert!(s.flight.is_none(), "no local turn should start");
    assert!(s.draft.text.is_empty(), "draft should clear after send");
    assert!(!s.cloud.codex.on, "Cloud is one message's choice");
    assert!(s.cloud.open, "cloud panel stays open");
    assert_eq!(s.cloud.codex.tasks.len(), 1);
    assert_eq!(s.cloud.codex.tasks[0].task_id, "task_e_9911");
    assert_eq!(
        s.cloud.codex.tasks[0].teammate_id.as_deref(),
        Some("tm_codex")
    );
    assert_eq!(s.cloud.panel.codex_tasks, 1);

    let screen = words(&rt);
    assert!(
        rt.find("codex-task-task_e_9911").is_some(),
        "started task row should be drawn in panel"
    );
    assert!(
        screen.contains("Uncommitted changes in this folder stay here"),
        "pre-start dirty note should be shown in panel: {screen}"
    );
}

#[test]
fn refused_codex_cloud_send_shows_error_and_keeps_draft() {
    let (world, mut rt) = window(SendSpec {
        repo_url: None,
        ..SendSpec::default()
    });
    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-cloud");
    pump(&mut rt, "codex panel loaded", |s| {
        s.cloud.codex.place.is_some()
    });

    type_and_send(&mut rt, "Try without a GitHub remote");

    let calls = world.codex_calls.lock().unwrap().clone();
    assert!(
        !calls
            .iter()
            .any(|args| args.get(1).map(String::as_str) == Some("exec")),
        "refused send must not invoke codex cloud exec: {calls:?}"
    );
    assert!(
        rt.state
            .composer_note
            .as_deref()
            .is_some_and(|n| n.contains("GitHub repository")),
        "composer note should explain the refusal: {:?}",
        rt.state.composer_note
    );
    assert_eq!(
        rt.state.draft.text, "Try without a GitHub remote",
        "draft must be preserved on refusal"
    );
    assert!(rt.state.cloud.codex.on, "Cloud stays on so person can adjust");
    assert!(rt.state.cloud.codex.tasks.is_empty());
}

#[test]
fn unready_codex_refuses_cloud_menu_choice_and_explains_why() {
    let (_world, mut rt) = window(SendSpec {
        codex_ready: false,
        ..SendSpec::default()
    });
    rt.update(|s, _| {
        s.cloud.codex.codex_ready = Some(false);
    });
    settle(&mut rt);

    click(&mut rt, "compare-mode-chip");
    let menu = words(&rt);
    assert!(
        menu.contains("Codex CLI is not installed or not signed in here."),
        "refusal should appear in menu description: {menu}"
    );

    click(&mut rt, "compare-mode-cloud");
    assert!(
        !rt.state.cloud.codex.on,
        "refused Cloud choice must not turn Cloud on"
    );
    assert_eq!(
        rt.state.composer_note.as_deref(),
        Some("Codex CLI is not installed or not signed in here.")
    );
}

#[test]
fn switching_teammate_or_model_clears_codex_cloud_choice() {
    let (_world, mut rt) = window(SendSpec::default());
    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-cloud");
    assert!(rt.state.cloud.codex.on);

    // Switching to another teammate clears Cloud on poll.
    rt.update(|s, _| {
        s.selected = 1;
    });
    pump(&mut rt, "codex cloud cleared on teammate switch", |s| {
        !s.cloud.codex.on
    });

    // Turn back on for Codex teammate, then switch route pick to Claude -> poll clears Cloud.
    rt.update(|s, _| {
        s.selected = 0;
    });
    settle(&mut rt);
    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-cloud");
    assert!(rt.state.cloud.codex.on);

    rt.update(|s, _| {
        s.picks.insert(
            "tm_codex".into(),
            TurnPick {
                runtime: Some("claude".into()),
                model: "claude-sonnet-5-5".into(),
                effort: None,
                account: None,
            },
        );
    });
    pump(&mut rt, "codex cloud cleared on model switch", |s| {
        !s.cloud.codex.on
    });
}

// Cloud on is Codex Cloud only while a Codex model is picked: with the teammate switched to another agent, a send is not
// taken for a Codex cloud task (it goes on as that agent's own turn).
#[test]
fn a_send_with_another_agent_picked_is_never_taken_for_a_codex_cloud_task() {
    let (world, mut rt) = window(SendSpec { dirty: true, ..SendSpec::default() });
    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-cloud");
    pump(&mut rt, "codex panel loaded", |s| s.cloud.codex.place.is_some());
    assert!(rt.state.cloud.codex.on, "Cloud is on for the Codex teammate");
    let mut taken = None;
    rt.update(|s, _| {
        let at = s.selected;
        s.teammates[at].runtime = Some("claude".into());
        s.picks.clear();
        taken = Some(crate::cloud_tasks::send(s, "Tighten checkout validation"));
    });
    assert_eq!(taken, Some(false), "not a Codex cloud send");
    let calls = world.codex_calls.lock().unwrap().clone();
    assert!(!calls.iter().any(|a| a.get(1).map(String::as_str) == Some("exec")), "{calls:?}");
}

fn exec_calls(world: &World) -> usize {
    world
        .codex_calls
        .lock()
        .unwrap()
        .iter()
        .filter(|args| args.get(1).map(String::as_str) == Some("exec"))
        .count()
}

fn no_environment_words() -> String {
    locust_core::cloud_tasks::refusal_for(&locust_core::cloud_tasks::CloudStartProblem::NoEnvironment {
        repo: "acme/widgets".into(),
    })
}

// CODEX CLOUD WITH NO ENVIRONMENT (27 Nov, Electron `sendOrCompare`): known before the send, nothing goes to Codex Cloud;
// the words stay in the box under what to do instead, and Cloud stays picked.
#[test]
fn a_repository_known_to_have_no_codex_environment_holds_the_send_before_it_goes() {
    let (world, mut rt) = window(SendSpec { env_missing: true, ..SendSpec::default() });
    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-cloud");
    pump(&mut rt, "missing environment known", |s| {
        s.cloud.codex.environment == Some(locust_core::cloud_tasks::CloudEnvironment::Missing)
    });

    type_and_send(&mut rt, "Tighten checkout validation");

    assert_eq!(exec_calls(&world), 0, "a doomed send never reaches codex cloud exec");
    assert_eq!(rt.state.composer_note.as_deref(), Some(no_environment_words().as_str()));
    assert_eq!(rt.state.draft.text, "Tighten checkout validation", "the words stay in the box");
    assert!(rt.state.cloud.codex.on, "nothing switches by itself: Cloud stays picked");
    assert!(rt.state.cloud.codex.tasks.is_empty());
}

// Learned from the refusal instead (the check had not said so): the refusal is said over the box, the repository is
// marked as having no environment, and the next send is held back before it goes.
#[test]
fn a_no_environment_refusal_marks_the_repository_and_holds_the_next_send() {
    let (world, mut rt) = window(SendSpec {
        exec_reply: Ran::new(1, "", "Error: environment 'acme/widgets' not found; run `codex cloud` to list available environments"),
        ..SendSpec::default()
    });
    click(&mut rt, "compare-mode-chip");
    click(&mut rt, "compare-mode-cloud");
    pump(&mut rt, "codex panel loaded", |s| s.cloud.codex.place.is_some() && s.cloud.codex.environment.is_some());
    assert_ne!(rt.state.cloud.codex.environment, Some(locust_core::cloud_tasks::CloudEnvironment::Missing));

    type_and_send(&mut rt, "Tighten checkout validation");
    assert_eq!(exec_calls(&world), 1);
    assert_eq!(rt.state.composer_note.as_deref(), Some(no_environment_words().as_str()));
    assert_eq!(rt.state.cloud.codex.environment, Some(locust_core::cloud_tasks::CloudEnvironment::Missing));
    assert_eq!(rt.state.draft.text, "Tighten checkout validation");

    rt.update(|s, _| s.composer_note = None);
    type_and_send(&mut rt, "Tighten checkout validation");
    assert_eq!(exec_calls(&world), 1, "the second send is held back, never sent again");
    assert_eq!(rt.state.composer_note.as_deref(), Some(no_environment_words().as_str()));
}

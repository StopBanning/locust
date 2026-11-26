//! Claude's cloud through the door, on the real engine with Claude Code and git stubbed: no test runs the real `claude`
//! or touches the network. Windows and terminals are recorded, never opened.
use super::*;
use crate::claude_cloud::{ClaudeLaunch, CloudPlatform, PseudoTerminalResult, PseudoTerminalRun, WindowRun};
use std::cell::RefCell;

const CREATED: &str = "Created cloud session: Fix the cart\r\nhttps://claude.ai/code/session_0123456789abcdef\r\nResume with: claude --teleport session_0123456789abcdef\r\n";

struct Dir(PathBuf);

impl Dir {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("locust-door-cloud-{}", crate::invariants::unique_nanos()));
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
    fn sessions(&self) -> PathBuf {
        self.0.join("claude-cloud.json")
    }
    fn marks(&self) -> PathBuf {
        self.0.join("door-cloud.json")
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn launch() -> Option<ClaudeLaunch> {
    Some(ClaudeLaunch { discovered_path: "fake-claude".into(), executable_path: "fake-claude".into(), prefix_args: Vec::new(), env: None })
}

/// A Claude Code that draws what `claude --cloud` draws on Windows, recording the line it was run with.
fn cloud_on<'a>(dir: &Dir, platform: CloudPlatform, ran: &'a RefCell<Vec<PseudoTerminalRun>>, windows: &'a RefCell<Vec<WindowRun>>) -> ClaudeCloud<'a> {
    ClaudeCloud::new(dir.sessions(), platform, launch)
        .with_terminal(move |run: &PseudoTerminalRun| {
            ran.borrow_mut().push(run.clone());
            PseudoTerminalResult::Ok { drawn: CREATED.into() }
        })
        .with_run_window(move |run: &WindowRun| {
            windows.borrow_mut().push(run.clone());
            Ok(())
        })
}

/// The door's seams are the test's own until the guard drops; tests that use them take turns.
struct Installed(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn install(seams: Seams) -> Installed {
    static TURN: Mutex<()> = Mutex::new(());
    let turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    *crate::invariants::lock(&TESTING) = Some(seams);
    Installed(turn)
}

impl Drop for Installed {
    fn drop(&mut self) {
        *crate::invariants::lock(&TESTING) = None;
    }
}

fn begin(folder: &str) -> CloudStart {
    CloudStart { folder: folder.into(), prompt: "Fix the cart".into(), teammate_id: "tm_ivy".into(), model: None, effort: None }
}

#[test]
fn a_task_the_door_starts_goes_to_claudes_cloud_and_is_recorded_as_an_apps() {
    let dir = Dir::new();
    let (ran, windows) = (RefCell::new(Vec::new()), RefCell::new(Vec::new()));
    let claude = cloud_on(&dir, CloudPlatform::Win32, &ran, &windows);
    let ClaudeCloudAnswer::Ok { session } = start_with(&claude, &dir.marks(), &begin("C:/work/app")) else { panic!("not started") };
    assert_eq!(ran.borrow().len(), 1, "Claude Code was run once, in a terminal nobody sees");
    assert_eq!(ran.borrow()[0].cwd, "C:/work/app");
    assert_eq!(ran.borrow()[0].env.get(crate::claude_cloud::TASK_VARIABLE).map(String::as_str), Some("Fix the cart"));
    assert!(windows.borrow().is_empty(), "no window of Claude Code's own opens when the session was created unseen");
    assert_eq!(session.session_id.as_deref(), Some("session_0123456789abcdef"));
    assert_eq!(session.url.as_deref(), Some("https://claude.ai/code/session_0123456789abcdef"));
    assert_eq!(session.teammate_id.as_deref(), Some("tm_ivy"));
    let chats = chats_with(&claude, &dir.marks());
    assert_eq!(chats.len(), 1);
    assert!(chats[0].from_app);
    assert_eq!(chats[0].session, session);
}

#[test]
fn a_session_the_window_started_is_listed_as_the_persons_not_an_apps() {
    let dir = Dir::new();
    let (ran, windows) = (RefCell::new(Vec::new()), RefCell::new(Vec::new()));
    let claude = cloud_on(&dir, CloudPlatform::Win32, &ran, &windows);
    // The window's own start goes straight to the engine; the door never saw it.
    let ClaudeCloudAnswer::Ok { session } = claude.start("C:/work/app", "By hand", None, None) else { panic!("not started") };
    let chats = chats_with(&claude, &dir.marks());
    assert_eq!(chats.len(), 1);
    assert!(!chats[0].from_app);
    assert_eq!(chat_with(&claude, &dir.marks(), &session.id).map(|c| c.session.prompt), Some("By hand".into()));
    assert_eq!(chat_with(&claude, &dir.marks(), "cc_00000000000000000099"), None);
}

#[test]
fn an_id_the_engine_counted_again_after_a_restart_is_not_taken_for_the_session_the_door_started() {
    let dir = Dir::new();
    let (ran, windows) = (RefCell::new(Vec::new()), RefCell::new(Vec::new()));
    let claude = cloud_on(&dir, CloudPlatform::Win32, &ran, &windows);
    let ClaudeCloudAnswer::Ok { session } = start_with(&claude, &dir.marks(), &begin("C:/work/app")) else { panic!("not started") };
    // The engine counts ids from 1 in every run: another session now has the id the door recorded, started later.
    let mut body: Value = serde_json::from_str(&std::fs::read_to_string(dir.sessions()).unwrap()).unwrap();
    body["sessions"][0]["startedAt"] = json!("2030-01-01T00:00:00.000Z");
    std::fs::write(dir.sessions(), body.to_string()).unwrap();
    let chats = chats_with(&claude, &dir.marks());
    assert_eq!(chats[0].session.id, session.id);
    assert!(!chats[0].from_app, "the record names the time it was started, not the id alone");
}

#[test]
fn the_folders_saved_environment_goes_with_the_task_and_is_not_erased() {
    let dir = Dir::new();
    let (ran, windows) = (RefCell::new(Vec::new()), RefCell::new(Vec::new()));
    let claude = cloud_on(&dir, CloudPlatform::Win32, &ran, &windows);
    std::fs::write(dir.0.join("claude-cloud.json.environments.json"), json!([{"folder": "C:/work/app", "id": "ccpool_team"}]).to_string()).unwrap();
    let start = CloudStart { model: Some("claude-opus-5-5".into()), effort: Some("high".into()), ..begin("C:/work/app") };
    assert!(matches!(start_with(&claude, &dir.marks(), &start), ClaudeCloudAnswer::Ok { .. }));
    let line = ran.borrow()[0].line.clone();
    assert!(line.contains("--model claude-opus-5-5 --effort high --environment ccpool_team --cloud"), "{line}");
    assert_eq!(claude.environment("C:/work/app").as_deref(), Some("ccpool_team"), "a start with none would have erased it");
}

#[test]
fn on_a_mac_claude_codes_own_window_opens_and_the_session_is_recorded_without_its_id() {
    let dir = Dir::new();
    let (ran, windows) = (RefCell::new(Vec::new()), RefCell::new(Vec::new()));
    let claude = cloud_on(&dir, CloudPlatform::Darwin, &ran, &windows);
    let ClaudeCloudAnswer::Ok { session } = start_with(&claude, &dir.marks(), &begin("/Users/a/app")) else { panic!("not started") };
    assert!(ran.borrow().is_empty(), "a hidden start is Windows only so far");
    assert_eq!(windows.borrow().len(), 1);
    assert_eq!(windows.borrow()[0].args, vec!["--cloud".to_owned(), "Fix the cart".to_owned()]);
    assert_eq!((session.session_id.clone(), session.url.clone()), (None, None));
    assert_eq!(link_of(&session), "https://claude.ai/code", "the list of sessions, when it has no page of its own");
    assert!(chats_with(&claude, &dir.marks())[0].from_app);
}

#[test]
fn a_refusal_from_claude_code_is_passed_on_and_nothing_is_recorded() {
    let dir = Dir::new();
    let claude = ClaudeCloud::new(dir.sessions(), CloudPlatform::Win32, || None);
    let ClaudeCloudAnswer::Err { message } = start_with(&claude, &dir.marks(), &begin("C:/work/app")) else { panic!("started") };
    assert_eq!(message, "Claude Code is not installed here. Settings > AI agents shows how to add it.");
    assert!(chats_with(&claude, &dir.marks()).is_empty());
    assert!(!dir.marks().exists());
}

#[test]
fn a_start_that_cannot_be_recorded_says_so_instead_of_handing_back_an_id_apps_may_not_read() {
    let dir = Dir::new();
    let (ran, windows) = (RefCell::new(Vec::new()), RefCell::new(Vec::new()));
    let claude = cloud_on(&dir, CloudPlatform::Win32, &ran, &windows);
    // The record's folder is a file: it cannot be written.
    std::fs::write(dir.0.join("blocked"), "x").unwrap();
    let ClaudeCloudAnswer::Ok { session } = start_with(&claude, &dir.0.join("blocked").join("door-cloud.json"), &begin("C:/work/app")) else {
        panic!("not started")
    };
    assert!(session.note.as_deref().is_some_and(|n| n.contains("could not record that this app started it")));
}

#[test]
fn the_live_door_starts_lists_and_records_through_its_own_claude_code() {
    let root = std::env::temp_dir().join(format!("locust-door-cloud-live-{}", crate::invariants::unique_nanos()));
    let electron = root.join("electron");
    std::fs::create_dir_all(root.join("work")).unwrap();
    std::fs::create_dir_all(&electron).unwrap();
    std::fs::write(
        electron.join("teammates.json"),
        json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_ivy", "name": "Ivy", "hue": "blue", "role": "Custom",
            "createdAt": "2026-11-01T00:00:00Z", "folder": root.join("work").to_string_lossy(),
            "route": {"runtime": "claude", "model": "claude-opus-5-5", "mode": "ask"}}]})
        .to_string(),
    )
    .unwrap();
    let store = Store::new(root.join("native"), electron).unwrap();
    let ran: Arc<Mutex<Vec<PseudoTerminalRun>>> = Arc::default();
    let seen = ran.clone();
    let seams = Seams {
        claude: Arc::new(move |store: &Store| {
            let seen = seen.clone();
            ClaudeCloud::new(crate::claude_cloud::store_path_of(store), CloudPlatform::Win32, launch).with_terminal(move |run: &PseudoTerminalRun| {
                seen.lock().unwrap().push(run.clone());
                PseudoTerminalResult::Ok { drawn: CREATED.into() }
            })
        }),
        github: Arc::new(|_| FolderOnGitHub { repo: Some("locust/app".into()), branch: Some("main".into()), unpushed: Some(0), dirty: false }),
    };
    let _own = install(seams);
    let tools = crate::door::tools_over_seeing(store.clone());
    let started = tools.call("start_conversation", &json!({"teammate": "Ivy", "message": "Fix the cart", "cloud": true}));
    assert!(!started.is_error, "{}", started.text);
    let reply: Value = serde_json::from_str(&started.text).unwrap();
    assert_eq!(reply["runtime"], "claude");
    assert_eq!(reply["cloud"], true);
    assert_eq!(reply["repo"], "locust/app");
    assert_eq!(reply["url"], "https://claude.ai/code/session_0123456789abcdef");
    assert_eq!(ran.lock().unwrap().len(), 1, "the engine's Claude Code ran once, the test's own");
    assert_eq!(ran.lock().unwrap()[0].cwd, root.join("work").to_string_lossy());
    assert!(crate::claude_cloud::store_path_of(&store).exists(), "recorded where the window's panel reads");
    let listed: Value = serde_json::from_str(&tools.call("list_conversations", &json!({})).text).unwrap();
    let row = listed.as_array().unwrap().iter().find(|r| r["conversation_id"] == reply["conversation_id"]).expect("listed").clone();
    assert_eq!((row["cloud"].clone(), row["started_by"].clone(), row["teammate"].clone()), (json!(true), json!("an app"), json!("Ivy")));
    assert_eq!(row["url"], "https://claude.ai/code/session_0123456789abcdef");
    let _ = std::fs::remove_dir_all(root);
}

//! Claude's cloud through the door, on the real engine with Claude Code and git stubbed: no test runs the real `claude`
//! or touches the network. Windows and terminals are recorded, never opened.
use super::*;
use crate::claude_cloud::{ClaudeLaunch, CloudPlatform, ExecOptions, ExecReply, PseudoTerminalResult, PseudoTerminalRun, WindowRun};
use std::cell::RefCell;
use std::process::Command;

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
fn every_session_the_door_starts_in_a_row_stays_recorded_as_an_apps_newest_first() {
    let dir = Dir::new();
    let (ran, windows) = (RefCell::new(Vec::new()), RefCell::new(Vec::new()));
    let claude = cloud_on(&dir, CloudPlatform::Win32, &ran, &windows);
    let mut ids = Vec::new();
    for n in 0..3 {
        let ClaudeCloudAnswer::Ok { session } = start_with(&claude, &dir.marks(), &begin("C:/work/app")) else { panic!("start {n} failed") };
        ids.push(session.id);
    }
    let chats = chats_with(&claude, &dir.marks());
    assert_eq!(chats.len(), 3);
    assert!(chats.iter().all(|c| c.from_app), "an app that started three can read and follow up all three");
    assert_eq!(read_marks(&dir.marks()).iter().map(|m| m.id.clone()).collect::<Vec<_>>(), ids.iter().rev().cloned().collect::<Vec<_>>());
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

// ---- follow up and read ----------------------------------------------------------------------------------------------

const SESSION: &str = "session_0123456789abcdef";

fn git(cwd: &Path, args: &[&str]) {
    let out = Command::new("git").args(args).current_dir(cwd).output().expect("git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

fn slug(path: &Path) -> String {
    path.to_string_lossy().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect()
}

/// The transcript a session of one exchange has, and of two once a follow-up was answered; and one whose follow-up is
/// not answered yet.
fn transcript(exchanges: &[(&str, Option<&str>)]) -> String {
    let mut lines = vec![r#"{"type":"permission-mode","permissionMode":"default","sessionId":"local"}"#.to_owned()];
    for (n, (prompt, answer)) in exchanges.iter().enumerate() {
        lines.push(
            json!({"type": "user", "entrypoint": "cli", "promptSource": "sdk", "remoteSourced": true,
                "timestamp": format!("2026-11-25T10:0{n}:01.000Z"), "message": {"role": "user", "content": prompt}})
            .to_string(),
        );
        if let Some(answer) = answer {
            lines.push(
                json!({"type": "assistant", "entrypoint": "cli", "timestamp": format!("2026-11-25T10:0{n}:09.000Z"),
                    "message": {"role": "assistant", "model": "claude-opus-5-5", "stop_reason": "end_turn", "content": [{"type": "text", "text": answer}]}})
                .to_string(),
            );
        }
    }
    lines.join("\n")
}

/// A real repository with one commit, and a store that knows the session the door started from it. Claude Code
/// never runs: the terminal the engine reads through plays what its teleport did.
struct World {
    repo: PathBuf,
    home: PathBuf,
    dir: Dir,
}

impl World {
    fn new() -> Self {
        let dir = Dir::new();
        let repo = dir.0.join("repo").canonicalize().unwrap_or_else(|_| {
            std::fs::create_dir_all(dir.0.join("repo")).unwrap();
            dir.0.join("repo").canonicalize().unwrap()
        });
        git(&repo, &["init", "-q", "-b", "main"]);
        git(&repo, &["config", "user.email", "test@example.com"]);
        git(&repo, &["config", "user.name", "Test"]);
        std::fs::write(repo.join("cart.js"), "export const total = (items) => items.length\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "first"]);
        let home = dir.0.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let sessions = json!({"sessions": [{"id": "cc_00000000000000000001", "startedAt": "2026-11-25T10:00:00.000Z",
            "prompt": "Fix the cart total", "folder": repo.to_string_lossy(), "sessionId": SESSION,
            "url": format!("https://claude.ai/code/{SESSION}"), "teammateId": "tm_ivy"}]});
        std::fs::write(dir.sessions(), sessions.to_string()).unwrap();
        // The door started it.
        write_marks(&dir.marks(), &[Mark { id: "cc_00000000000000000001".into(), started_at: "2026-11-25T10:00:00.000Z".into(), seen: 1, pending: 0 }]).unwrap();
        Self { repo, home, dir }
    }

    /// Claude Code, its teleport played: the worktree made on its branch, the change committed, the transcript written.
    fn claude<'a>(&'a self, said: &'a str, teleports: &'a RefCell<u32>, sends: &'a RefCell<Vec<(Vec<String>, Option<String>)>>, receipt: &'a str) -> ClaudeCloud<'a> {
        let name = crate::claude_cloud::reading_worktree_name("cc_00000000000000000001");
        ClaudeCloud::new(self.dir.sessions(), CloudPlatform::Win32, launch)
            .with_claude_home(&self.home)
            .with_process_env(Vec::new)
            .with_terminal(move |_run: &PseudoTerminalRun| {
                *teleports.borrow_mut() += 1;
                let place = self.repo.join(".claude").join("worktrees").join(&name);
                let place_s = place.to_string_lossy().into_owned();
                git(&self.repo, &["worktree", "add", "-q", "-b", &format!("worktree-{name}"), &place_s]);
                git(&place, &["checkout", "-q", "-B", "claude/fix-cart-total"]);
                std::fs::write(place.join("cart.js"), "export const total = (items) => items.reduce((sum, item) => sum + item.price, 0)\n").unwrap();
                git(&place, &["commit", "-q", "-am", "Sum prices"]);
                let dir = self.home.join("projects").join(slug(&place));
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(dir.join("a.jsonl"), said).unwrap();
                PseudoTerminalResult::Ok { drawn: " \u{25cf} Session resumed\r\n".into() }
            })
            .with_exec(move |_file: &str, args: &[String], options: &ExecOptions| {
                sends.borrow_mut().push((args.to_vec(), options.env.get(crate::claude_cloud::TASK_VARIABLE).cloned()));
                ExecReply { code: 0, output: receipt.into(), stdout: Some(receipt.into()) }
            })
    }
}

const RECEIPT: &str = r#"{"ok":true}"#;
const ID: &str = "cc_00000000000000000001";

#[test]
fn a_follow_up_goes_to_the_session_itself_and_is_counted() {
    let w = World::new();
    let (teleports, sends) = (RefCell::new(0), RefCell::new(Vec::new()));
    let claude = w.claude("", &teleports, &sends, RECEIPT);
    assert_eq!(send_with(&claude, &w.dir.marks(), ID, "Also add a test"), CloudSendResult::Ok);
    assert_eq!(sends.borrow().len(), 1);
    let (args, task) = sends.borrow()[0].clone();
    assert!(args[0].contains(&format!("-p --cloud {SESSION} --output-format json")), "{args:?}");
    assert_eq!(task.as_deref(), Some("Also add a test"), "the words go as the task variable, never in the command line");
    let mark = read_marks(&w.dir.marks()).remove(0);
    assert_eq!((mark.seen, mark.pending), (1, 1));
    // Claude Code's own refusal is passed on, and counts nothing.
    let refused = w.claude("", &teleports, &sends, r#"{"ok":false,"error":"Session is archived"}"#);
    assert_eq!(send_with(&refused, &w.dir.marks(), ID, "More"), CloudSendResult::Err { message: "Session is archived".into() });
    assert_eq!(read_marks(&w.dir.marks())[0].pending, 1);
    assert_eq!(send_with(&refused, &w.dir.marks(), "cc_00000000000000000042", "More"), CloudSendResult::Err {
        message: "Locust does not know this session's id, so it cannot send to it. Send from claude.ai or the Claude app.".into()
    });
}

#[test]
fn a_read_gives_the_answer_and_the_files_the_session_changed_in_a_worktree_of_its_own() {
    let w = World::new();
    let (teleports, sends) = (RefCell::new(0), RefCell::new(Vec::new()));
    let said = transcript(&[("Fix the cart total", Some("Fixed: total now sums prices."))]);
    let claude = w.claude(&said, &teleports, &sends, RECEIPT);
    let CloudRead::Done { answer, model, changed, change_too_big, note } = read_with(&claude, &w.dir.marks(), ID) else { panic!("not done") };
    assert_eq!(*teleports.borrow(), 1, "one look per read");
    assert_eq!(answer, "Fixed: total now sums prices.");
    assert_eq!(model.as_deref(), Some("claude-opus-5-5"));
    assert_eq!(changed, vec![("cart.js".to_owned(), 1, 1)]);
    assert!(!change_too_big && note.is_none());
    // Read into a worktree beside the folder; the person's folder itself is untouched.
    assert_eq!(std::fs::read_to_string(w.repo.join("cart.js")).unwrap(), "export const total = (items) => items.length\n");
    assert!(w.repo.join(".claude/worktrees").is_dir());
}

#[test]
fn a_read_straight_after_a_follow_up_is_still_working_until_the_reply_to_it_is_there() {
    let w = World::new();
    let (teleports, sends) = (RefCell::new(0), RefCell::new(Vec::new()));
    let first = transcript(&[("Fix the cart total", Some("Fixed."))]);
    let claude = w.claude(&first, &teleports, &sends, RECEIPT);
    assert!(matches!(read_with(&claude, &w.dir.marks(), ID), CloudRead::Done { .. }));
    assert_eq!(send_with(&claude, &w.dir.marks(), ID, "Also add a test"), CloudSendResult::Ok);
    // The session still shows the first answer only: that is not the reply to the follow-up.
    let teleported = |said: &str| read_with(&w.claude(said, &teleports, &sends, RECEIPT), &w.dir.marks(), ID);
    assert!(matches!(teleported(&first), CloudRead::Working { .. }), "the old answer is not the new reply");
    // The follow-up arrived but is not answered yet.
    let asked = transcript(&[("Fix the cart total", Some("Fixed.")), ("Also add a test", None)]);
    assert!(matches!(teleported(&asked), CloudRead::Working { .. }));
    // Answered: now it is the reply, and the count is settled for the next follow-up.
    let answered = transcript(&[("Fix the cart total", Some("Fixed.")), ("Also add a test", Some("Added cart.test.js."))]);
    let CloudRead::Done { answer, .. } = teleported(&answered) else { panic!("not done") };
    assert_eq!(answer, "Added cart.test.js.");
    let mark = read_marks(&w.dir.marks()).remove(0);
    assert_eq!((mark.seen, mark.pending), (2, 0));
}

#[test]
fn a_session_with_no_id_cannot_be_read_and_says_so() {
    let dir = Dir::new();
    let (ran, windows) = (RefCell::new(Vec::new()), RefCell::new(Vec::new()));
    let claude = cloud_on(&dir, CloudPlatform::Darwin, &ran, &windows);
    let ClaudeCloudAnswer::Ok { session } = start_with(&claude, &dir.marks(), &begin("/Users/a/app")) else { panic!("not started") };
    let CloudRead::Failed(message) = read_with(&claude, &dir.marks(), &session.id) else { panic!("read") };
    assert!(message.contains("does not know this session\u{2019}s id"), "{message}");
    assert!(ran.borrow().is_empty(), "nothing was run to read it");
}

#[test]
fn the_live_door_follows_up_reads_and_stops_through_its_own_claude_code() {
    let root = std::env::temp_dir().join(format!("locust-door-cloud-live2-{}", crate::invariants::unique_nanos()));
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
    let sent: Arc<Mutex<Vec<Vec<String>>>> = Arc::default();
    let seen = sent.clone();
    let seams = Seams {
        claude: Arc::new(move |store: &Store| {
            let seen = seen.clone();
            ClaudeCloud::new(crate::claude_cloud::store_path_of(store), CloudPlatform::Win32, launch)
                .with_terminal(|_: &PseudoTerminalRun| PseudoTerminalResult::Ok { drawn: CREATED.into() })
                .with_exec(move |_: &str, args: &[String], _: &ExecOptions| {
                    seen.lock().unwrap().push(args.to_vec());
                    ExecReply { code: 0, output: RECEIPT.into(), stdout: Some(RECEIPT.into()) }
                })
        }),
        github: Arc::new(|_| FolderOnGitHub { repo: Some("locust/app".into()), branch: Some("main".into()), unpushed: Some(0), dirty: false }),
    };
    let _own = install(seams);
    let tools = crate::door::tools_over_seeing(store);
    let started = tools.call("start_conversation", &json!({"teammate": "Ivy", "message": "Fix the cart", "cloud": true}));
    assert!(!started.is_error, "{}", started.text);
    let id = serde_json::from_str::<Value>(&started.text).unwrap()["conversation_id"].as_str().unwrap().to_owned();
    // The follow-up reaches Claude Code's send, for this session.
    let followed = tools.call("send_message", &json!({"conversation_id": id, "message": "Also add a test"}));
    assert!(!followed.is_error, "{}", followed.text);
    assert_eq!(sent.lock().unwrap().len(), 1);
    assert!(sent.lock().unwrap()[0][0].contains(&format!("-p --cloud {SESSION} --output-format json")));
    // A read is the engine's own: the folder here is no git checkout, so Claude Code cannot bring the session into it.
    let read = tools.call("read_reply", &json!({"conversation_id": id}));
    assert!(read.is_error && read.text.contains("not a git checkout"), "{}", read.text);
    assert!(read.text.ends_with("Its page: https://claude.ai/code/session_0123456789abcdef"));
    // It cannot be stopped from here.
    let stopped = tools.call("stop_turn", &json!({"conversation_id": id}));
    assert!(stopped.is_error && stopped.text.contains("cannot be stopped from here"), "{}", stopped.text);
    let _ = std::fs::remove_dir_all(root);
}

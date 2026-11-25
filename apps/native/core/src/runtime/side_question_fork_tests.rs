// Carries packages/runtime-adapters/test/a-side-question-forks-the-session.test.ts (Electron 4ce2f37a): a question on
// the side continues a copy of the session and leaves the session itself as it was. Claude Code's cases are here.
// Rust has no `opencode run` command line to give `-s … --fork`: it forks an OpenCode session through the served
// session's own `/session/{id}/fork` (opencode_serve.rs), and the OpenCode cases are carried on that, below.
use super::claude::{PrintOptions, print_args};
use crate::approval::Mode;

#[test]
fn a_claude_code_session_is_forked_with_resume_then_fork_session() {
    let args = print_args(&PrintOptions { mode: Mode::Auto, resume: Some("sess-1"), fork: true, ..Default::default() }).unwrap();
    let at = args.iter().position(|a| a == "--resume").expect("--resume");
    assert_eq!(args[at..at + 3], ["--resume", "sess-1", "--fork-session"]);
}

#[test]
fn a_claude_code_run_with_no_session_to_fork_adds_no_fork_flag() {
    let args = print_args(&PrintOptions { mode: Mode::Auto, fork: true, ..Default::default() }).unwrap();
    assert!(!args.iter().any(|a| a == "--fork-session"), "{args:?}");
}

// Rust's own: an ordinary follow-up resumes the session itself.
#[test]
fn an_ordinary_claude_code_follow_up_is_not_a_fork() {
    let args = print_args(&PrintOptions { mode: Mode::Auto, resume: Some("sess-1"), ..Default::default() }).unwrap();
    assert!(args.iter().any(|a| a == "--resume") && !args.iter().any(|a| a == "--fork-session"), "{args:?}");
}

// OpenCode 1 is driven through `opencode serve`, so where Electron gives `opencode run` `-s <id> --fork`, Rust asks the
// served session for a copy (`POST /session/<id>/fork`) and prompts the copy. A stand-in server writes down every POST.
const SERVER: &str = r#"
import json, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
class H(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    def log_message(self, *a): pass
    def answer(self, body):
        data = b'' if body is None else json.dumps(body).encode()
        self.send_response(200)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def do_GET(self):
        if self.path.startswith('/event'):
            self.send_response(200)
            self.send_header('Content-Type', 'text/event-stream')
            self.send_header('Connection', 'close')
            self.end_headers()
            try:
                while True:
                    self.wfile.write(b': ping\n\n'); self.wfile.flush(); time.sleep(0.2)
            except Exception:
                return
        self.answer({})
    def do_POST(self):
        n = int(self.headers.get('Content-Length', '0'))
        if n: self.rfile.read(n)
        with open('posts.txt', 'a') as f: f.write(self.path + '\n')
        if self.path == '/session': return self.answer({'id': 'ses_new'})
        if self.path.endswith('/fork'): return self.answer({'id': 'ses_fork'})
        self.answer(None)
httpd = ThreadingHTTPServer(('127.0.0.1', 0), H)
print(f'listening on http://127.0.0.1:{httpd.server_address[1]}', flush=True)
httpd.serve_forever()
"#;

/// The POSTs an OpenCode 1 run made to its server, up to its prompt, for a run resuming `resume` (a side question when
/// `fork`), driven through `OpenCode` as native.rs drives it (`OpenCode::spawn(..).with_fork(fork)`).
fn served_posts(resume: Option<&str>, fork: bool) -> Vec<String> {
    use super::{Runtime, Turn, opencode::OpenCode};
    use std::time::{Duration, Instant};
    let dir = std::env::temp_dir().join(format!(
        "locust-side-fork-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("server.py"), SERVER).unwrap();
    let mut command = std::process::Command::new("python3");
    command.arg(dir.join("server.py")).arg("serve").current_dir(&dir).env("LOCUST_NATIVE_OPENCODE_PROTOCOL", "serve");
    let mut rt = OpenCode::spawn(&mut command).unwrap().with_fork(fork);
    let turn = Turn {
        prompt: "why?".into(),
        folder: dir.clone(),
        model: None,
        effort: None,
        resume: resume.map(str::to_owned),
        mode: Mode::Ask,
        approvals: None,
        mission_id: "side_fork".into(),
        run_id: "run_side_fork".into(),
    };
    rt.start(&turn).unwrap();
    let posts = || std::fs::read_to_string(dir.join("posts.txt")).unwrap_or_default();
    crate::load_waits::wait_for("the prompt to reach the server", || posts().contains("/prompt_async"));
    let seen: Vec<String> = posts().lines().map(str::to_owned).collect();
    rt.stop().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && !matches!(rt.next_event(), Ok(None) | Err(_)) {}
    drop(rt);
    let _ = std::fs::remove_dir_all(&dir);
    seen
}

#[test]
fn an_opencode_side_question_forks_the_served_session_and_prompts_the_copy() {
    // case: forks an OpenCode session: -s, then --fork
    assert_eq!(served_posts(Some("ses_1"), true), ["/session/ses_1/fork", "/session/ses_fork/prompt_async"]);
}

#[test]
fn no_fork_without_a_session_to_fork_or_for_an_ordinary_follow_up() {
    // case: adds no fork flag without a session, or to an ordinary follow-up
    let fresh = print_args(&PrintOptions { mode: Mode::Auto, fork: true, ..Default::default() }).unwrap();
    assert!(!fresh.iter().any(|a| a == "--fork-session"), "{fresh:?}");
    // An ordinary OpenCode follow-up prompts the session itself; a side question with no session starts a new one.
    assert_eq!(served_posts(Some("ses_1"), false), ["/session/ses_1/prompt_async"]);
    assert_eq!(served_posts(None, true), ["/session", "/session/ses_new/prompt_async"]);
}

/// The POSTs a side question on an OpenCode 1 conversation (whose session is `ses_1`) made to its server, through the
/// app's own path (`native::send_from`, which hands the run `with_fork` at native.rs), stopped once it prompts.
fn side_question_through_the_app() -> Vec<String> {
    use crate::{engine::Teammate, native, store::Store};
    use serde_json::json;
    use std::time::{Duration, Instant};
    let root = std::env::temp_dir().join(format!("locust-side-fork-app-{}", crate::invariants::unique_nanos()));
    let work = root.join("work");
    std::fs::create_dir_all(&work).unwrap();
    std::fs::write(root.join("server.py"), SERVER).unwrap();
    let store = Store::new(root.join("native"), root.join("electron")).unwrap();
    store
        .save_json("teammates.json", &json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_open", "name": "Open"}], "missionOwners": {}, "settings": {}}))
        .unwrap();
    let ledger = [
        json!({"schemaVersion": 27, "recordType": "mission.created", "ledgerSequence": 1, "occurredAt": "2026-09-29T15:00:00.000Z", "metadata": {"missionId": "mission_convo", "runId": "run_mission_convo", "runtime": "opencode", "model": "opencode/beam-free", "mode": "accept-edits", "createdAt": "2026-09-29T15:00:00.000Z", "prompt": "Add a rename command."}}),
        json!({"schemaVersion": 27, "recordType": "mission.event", "ledgerSequence": 2, "occurredAt": "2026-09-29T15:00:01.000Z", "event": {"sequence": 1, "type": "run.started", "payload": {"runtimeThreadId": "ses_1"}}}),
    ];
    std::fs::create_dir_all(store.ledger_dir().unwrap()).unwrap();
    std::fs::write(store.write_path("mission_convo").unwrap(), ledger.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
    let _hook = native::hooks::install(store, Some(("python3".into(), vec![root.join("server.py").to_string_lossy().into_owned(), "serve".into()])));
    let teammate = Teammate {
        id: "tm_open".into(),
        name: "Open".into(),
        role: "Custom".into(),
        runtime: Some("opencode".into()),
        model: Some("opencode/beam-free".into()),
        account: None,
        busy: false,
        runs_in: "ask".into(),
        folder: Some(work.to_string_lossy().into_owned()),
    };
    let side = native::send_from(
        native::Send { teammate, conversation: None, prompt: "Why?".into(), model: None, effort: None },
        native::Origin {
            mode: Some(Mode::Ask),
            started_by: Some(json!({"kind": "side", "of": "mission_convo", "question": 1})),
            runtime: Some("opencode".into()),
            ..Default::default()
        },
    )
    .unwrap();
    let posts = || std::fs::read_to_string(work.join("posts.txt")).unwrap_or_default();
    crate::load_waits::wait_for("the side question's prompt to reach the server", || posts().contains("/prompt_async"));
    let seen: Vec<String> = posts().lines().map(str::to_owned).collect();
    let _ = native::stop(&side.conversation_id);
    let deadline = Instant::now() + Duration::from_secs(15);
    while native::owns(&side.conversation_id) {
        assert!(Instant::now() < deadline, "the side question did not end once stopped");
        std::thread::sleep(Duration::from_millis(10));
    }
    let _ = std::fs::remove_dir_all(&root);
    seen
}

#[test]
fn an_opencode_side_question_from_the_app_prompts_a_copy_of_the_conversations_session() {
    // case: forks an OpenCode session: -s, then --fork
    assert_eq!(side_question_through_the_app(), ["/session/ses_1/fork", "/session/ses_fork/prompt_async"]);
}

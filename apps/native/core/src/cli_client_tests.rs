//! The locust command talks only to Locust (packages/cli/test/the-command-talks-only-to-locust.test.ts), against a
//! fake app of the test's own on 127.0.0.1 (fake-app.ts): it checks the bearer token, records each `{name,
//! arguments}`, and answers as the test says. The last test starts a real door with the editors' sessions behind it.
use super::*;
use crate::cli_sessions::{CliBegun, CliCard, CliHost, CliOptions, CliSessions, CliSink, CliStart, CliUpdate};
use crate::door::{Caller, Door, Settings};
use std::io::BufRead;
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};

/// What the fake app does with one call.
enum Answer {
    /// `{content: [{type: "text", text: JSON.stringify(value)}]}`.
    Json(Value),
    /// These bytes, exactly, then the connection closes.
    Raw(String),
    /// These bytes, then nothing more: the reply never finishes.
    Never(String),
}

type Requests = Arc<Mutex<Vec<(String, Value)>>>;

struct FakeApp {
    profile: PathBuf,
    url: String,
    requests: Requests,
    client: LocustClient,
}

impl FakeApp {
    fn names(&self) -> Vec<String> {
        self.requests.lock().unwrap().iter().map(|(n, _)| n.clone()).collect()
    }
    fn requests(&self) -> Vec<(String, Value)> {
        self.requests.lock().unwrap().clone()
    }
}

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("locust-cli-test-{name}-{}-{}", std::process::id(), crate::invariants::unique_nanos()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn token() -> String {
    crate::door::random_hex(32).unwrap()
}

fn one_request(mut stream: TcpStream, token: &str, requests: &Requests, handler: &(dyn Fn(&str, &Value) -> Answer + Send + Sync)) {
    let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
    let (mut length, mut authorization) = (0usize, String::new());
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    loop {
        line.clear();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
            break;
        }
        let (name, value) = line.split_once(':').unwrap_or((&line, ""));
        match name.to_ascii_lowercase().as_str() {
            "content-length" => length = value.trim().parse().unwrap_or(0),
            "authorization" => authorization = value.trim().to_owned(),
            _ => {}
        }
    }
    if authorization != format!("Bearer {token}") {
        let _ = stream.write_all(b"HTTP/1.1 401 Unauthorized\r\ncontent-length: 0\r\nconnection: close\r\n\r\n");
        // As the real door does (door.rs `refuse`): read out the body still arriving before closing, so the close is
        // not a reset that loses the refusal (the release-only flake of the test below).
        let _ = stream.shutdown(std::net::Shutdown::Write);
        let mut rest = vec![0u8; length];
        let _ = reader.read_exact(&mut rest);
        return;
    }
    let mut body = vec![0u8; length];
    reader.read_exact(&mut body).unwrap();
    let call: Value = serde_json::from_slice(&body).unwrap();
    let (name, args) = (call["name"].as_str().unwrap().to_owned(), call["arguments"].clone());
    requests.lock().unwrap().push((name.clone(), args.clone()));
    match handler(&name, &args) {
        Answer::Json(value) => {
            let body = json!({"content": [{"type": "text", "text": value.to_string()}]}).to_string();
            let _ = write!(stream, "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
        }
        Answer::Raw(bytes) => {
            let _ = stream.write_all(bytes.as_bytes());
        }
        Answer::Never(bytes) => {
            let _ = stream.write_all(bytes.as_bytes());
            let _ = stream.flush();
            std::thread::sleep(Duration::from_secs(5));
        }
    }
}

fn fake_app(handler: impl Fn(&str, &Value) -> Answer + Send + Sync + 'static, timeout_ms: u64) -> FakeApp {
    let profile = temp("profile");
    let token = token();
    let requests: Requests = Arc::default();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://127.0.0.1:{}/tools/call", listener.local_addr().unwrap().port());
    let handler: Arc<dyn Fn(&str, &Value) -> Answer + Send + Sync> = Arc::new(handler);
    let (seen, key) = (requests.clone(), token.clone());
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let (seen, key, handler) = (seen.clone(), key.clone(), handler.clone());
            std::thread::spawn(move || one_request(stream, &key, &seen, &*handler));
        }
    });
    std::fs::write(profile.join(CONNECTION_FILE), json!({"url": url, "token": token}).to_string()).unwrap();
    let client = LocustClient::with_timeout(&profile, Duration::from_millis(timeout_ms));
    FakeApp { profile, url, requests, client }
}

fn team_json() -> Value {
    json!([
        {"id": "mate-one", "name": "One", "runtime": "codex", "model": "test-model", "runs_in": "ask"},
        {"id": "mate-two", "name": "Two", "runtime": "claude", "model": "other-model", "runs_in": "ask"}
    ])
}

fn mate(id: &str, name: &str, runtime: &str, model: &str) -> CliTeammate {
    CliTeammate { id: id.into(), name: name.into(), runtime: Some(runtime.into()), model: Some(model.into()), runs_in: "ask".into() }
}

fn session() -> CliSession {
    CliSession {
        session_id: "session-one".into(),
        key: token(),
        teammate: mate("mate-one", "One", "codex", "test-model"),
        workspace: std::env::current_dir().unwrap().to_string_lossy().into_owned(),
    }
}

fn ok() -> Answer {
    Answer::Json(json!({"ok": true}))
}

fn nothing(_: &CliText) -> Result<(), LocalError> {
    Ok(())
}

// case: uses Electron’s actual profile name on each operating system
#[test]
fn uses_electrons_actual_profile_name_on_each_operating_system() {
    let app_data = |name: &str| (name == "APPDATA").then(|| "C:\\profile data".to_owned());
    assert_eq!(profile_directory("win32", &app_data, "C:\\home").unwrap(), "C:\\profile data\\@teammate\\desktop");
    assert_eq!(profile_directory("darwin", &|_| None, "/home/test").unwrap(), "/home/test/Library/Application Support/@teammate/desktop");
    assert_eq!(profile_directory("linux", &|_| None, "/home/test").unwrap(), "/home/test/.config/@teammate/desktop");
    let xdg = |name: &str| (name == "XDG_CONFIG_HOME").then(|| "/custom config".to_owned());
    assert_eq!(profile_directory("linux", &xdg, "/home/test").unwrap(), "/custom config/@teammate/desktop");
    let missing = profile_directory("win32", &|_| None, "C:\\home").unwrap_err();
    assert!(missing.0.contains("--profile"), "{missing}");
}

// case: authenticates against a fake app and reads teammates and status
#[test]
fn authenticates_against_a_fake_app_and_reads_teammates_and_status() {
    let app = fake_app(
        |name, _| {
            Answer::Json(if name == "list_teammates" {
                team_json()
            } else {
                json!({"workspace": "/project", "turns": [{"missionId": "m1", "teammate": "One", "state": "waiting"}]})
            })
        },
        2_000,
    );
    assert_eq!(app.client.teammates().unwrap(), vec![mate("mate-one", "One", "codex", "test-model"), mate("mate-two", "Two", "claude", "other-model")]);
    assert_eq!(app.client.status().unwrap().turns[0].state, "waiting");
    assert_eq!(app.names(), ["list_teammates", "cli/status"]);
    // Another app's token is refused before anything is read.
    std::fs::write(app.profile.join(CONNECTION_FILE), json!({"url": app.url, "token": "b".repeat(64)}).to_string()).unwrap();
    let refused = app.client.teammates().unwrap_err();
    assert!(refused.0.starts_with("Locust refused this connection."), "{refused}");
    assert_eq!(app.names(), ["list_teammates", "cli/status"]);
}

// case: refuses a connection file naming %s
#[test]
fn refuses_a_connection_file_naming_s() {
    let app = fake_app(|_, _| Answer::Json(json!([])), 2_000);
    let port = app.url.trim_start_matches("http://127.0.0.1:").trim_end_matches("/tools/call").to_owned();
    for url in [
        "https://127.0.0.1:1234/tools/call",
        "http://localhost:1234/tools/call",
        "http://example.test:1234/tools/call",
        "http://127.0.0.1:1234/elsewhere",
        "http://person:secret@127.0.0.1:1234/tools/call",
        "http://127.0.0.1:1234/tools/call?token=other",
    ] {
        // Electron's six, and each again at the fake app's own port, where a request would have been seen.
        for url in [url.to_owned(), url.replace("1234", &port)] {
            std::fs::write(app.profile.join(CONNECTION_FILE), json!({"url": url, "token": "a".repeat(64)}).to_string()).unwrap();
            let refused = app.client.teammates().unwrap_err();
            assert!(refused.0.contains("Settings > General"), "{url}: {refused}");
            assert!(refused.0.starts_with("Locust is not open, or its local server is off."), "{url}: {refused}");
            assert_eq!(app.requests(), [], "{url}");
        }
    }
}

// case: explains a missing connection file without reading any other app
#[test]
fn explains_a_missing_connection_file_without_reading_any_other_app() {
    let missing = LocustClient::new(temp("empty").join("missing-test-profile")).teammates().unwrap_err();
    assert!(missing.0.contains("not open") && missing.0.contains("server is off"), "{missing}");
    assert!(missing.0.find("not open") < missing.0.find("server is off"));
}

// case: does not follow a redirect with its token
#[test]
fn does_not_follow_a_redirect_with_its_token() {
    let elsewhere = fake_app(|_, _| ok(), 2_000);
    for location in ["https://example.test/".to_owned(), elsewhere.url.clone()] {
        let app = fake_app(
            move |_, _| Answer::Raw(format!("HTTP/1.1 302 Found\r\nlocation: {location}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")),
            2_000,
        );
        let refused = app.client.teammates().unwrap_err();
        assert!(refused.0.contains("could not complete"), "{refused}");
        assert_eq!(app.requests().len(), 1);
    }
    assert_eq!(elsewhere.requests(), [], "the redirect's target never heard from the command");
}

// case: has an absolute deadline even when a server never finishes its reply
#[test]
fn has_an_absolute_deadline_even_when_a_server_never_finishes_its_reply() {
    let app = fake_app(|_, _| Answer::Never("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 100\r\n\r\n{".into()), 50);
    let began = Instant::now();
    let late = app.client.teammates().unwrap_err();
    assert!(late.0.contains("time") || late.0.contains("interrupted"), "{late}");
    // Well under the client's own 15 s default, so it is this call's 50 ms that ended it; 3 s was too tight for a
    // machine under a full gate's load (3.3 s seen, 25 Nov), and a server that never finishes would hang forever.
    assert!(began.elapsed() < Duration::from_secs(10), "the deadline held: {:?}", began.elapsed());
    assert_eq!(app.requests().len(), 1);
}

// case: passes the server’s refusal through rather than retrying a start
#[test]
fn passes_the_servers_refusal_through_rather_than_retrying_a_start() {
    let app = fake_app(
        |_, _| {
            let body = json!({"isError": true, "content": [{"type": "text", "text": "Choose One or Two. Nothing was started."}]}).to_string();
            Answer::Raw(format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()))
        },
        2_000,
    );
    let refused = app.client.open(Some("missing"), None).unwrap_err();
    assert!(refused.0.contains("Choose One or Two"), "{refused}");
    assert_eq!(app.requests().len(), 1);
}

// case: prints incremental text once, not again for the final replacement
#[test]
fn prints_incremental_text_once_not_again_for_the_final_replacement() {
    let connection = session();
    let polls = Arc::new(AtomicUsize::new(0));
    let app = fake_app(
        move |name, _| {
            if name != "cli/poll" {
                return ok();
            }
            let poll = polls.fetch_add(1, Ordering::SeqCst) + 1;
            Answer::Json(json!({
                "turn": {"requestId": "p", "phase": if poll == 1 { "running" } else { "completed" }},
                "events": if poll == 1 {
                    json!([{"sequence": 1, "itemId": "a", "operation": "append", "text": "Hello"}])
                } else {
                    json!([{"sequence": 2, "itemId": "a", "operation": "replace", "text": "Hello world"}])
                },
                "cursor": poll,
                "approvals": []
            }))
        },
        2_000,
    );
    let printed = Mutex::new(Vec::<String>::new());
    let mut stream = TextStream::new(|text: &str| {
        printed.lock().unwrap().push(text.to_owned());
        Ok(())
    });
    let ended = app.client.prompt(&connection, "Say hello", None, &mut |event: &CliText| stream.accept(event), None, Duration::from_millis(1));
    assert_eq!(ended, Ok(Ended::EndTurn));
    drop(stream);
    assert_eq!(*printed.lock().unwrap(), ["Hello", " world"]);
    assert_eq!(app.names().iter().filter(|n| *n == "cli/prompt").count(), 1);
    let last_poll = app.requests().into_iter().rev().find(|(n, _)| n == "cli/poll").unwrap();
    assert_eq!(last_poll.1["after"], 1);
}

// case: cancels only this prompt when interrupted while its start request is pending
#[test]
fn cancels_only_this_prompt_when_interrupted_while_its_start_request_is_pending() {
    let (controller, connection) = (Abort::new(), session());
    let interrupt = controller.clone();
    let app = fake_app(
        move |name, _| {
            if name == "cli/prompt" {
                interrupt.abort();
                std::thread::sleep(Duration::from_millis(20));
            }
            ok()
        },
        2_000,
    );
    let ended = app.client.prompt(&connection, "Start", Some(&controller), &mut nothing, None, Duration::from_millis(250));
    assert_eq!(ended, Ok(Ended::Cancelled));
    let requests = app.requests();
    let start = &requests.iter().find(|(n, _)| n == "cli/prompt").unwrap().1;
    let cancel = &requests.iter().find(|(n, _)| n == "cli/cancel").expect("a cancel was sent").1;
    assert_eq!(cancel["requestId"], start["requestId"]);
    assert_eq!(cancel["sessionId"], json!(connection.session_id));
}

// case: does not start an already-cancelled prompt
#[test]
fn does_not_start_an_already_cancelled_prompt() {
    let app = fake_app(|_, _| Answer::Json(json!({})), 2_000);
    let controller = Abort::new();
    controller.abort();
    let ended = app.client.prompt(&session(), "Do not send", Some(&controller), &mut nothing, None, Duration::from_millis(250));
    assert_eq!(ended, Ok(Ended::Cancelled));
    assert_eq!(app.requests(), []);
}

// case: cancels an uncertain turn when reading its reply fails
#[test]
fn cancels_an_uncertain_turn_when_reading_its_reply_fails() {
    let app = fake_app(
        |name, _| {
            if name == "cli/poll" {
                Answer::Raw("HTTP/1.1 200 OK\r\ncontent-length: 8\r\nconnection: close\r\n\r\nnot json".into())
            } else {
                ok()
            }
        },
        2_000,
    );
    let failed = app.client.prompt(&session(), "Start", None, &mut nothing, None, Duration::from_millis(250)).unwrap_err();
    assert!(failed.0.contains("unreadable"), "{failed}");
    assert_eq!(app.names(), ["cli/prompt", "cli/poll", "cli/cancel"]);
}

// case: does not silently duplicate text that an agent rewrote
#[test]
fn does_not_silently_duplicate_text_that_an_agent_rewrote() {
    let mut stream = TextStream::new(|_: &str| Ok(()));
    let text = |sequence, operation: &str, text: &str| CliText { sequence, item_id: "a".into(), operation: operation.into(), text: text.into() };
    stream.accept(&text(1, "append", "old")).unwrap();
    let revised = stream.accept(&text(2, "replace", "new")).unwrap_err();
    assert!(revised.0.contains("revised"), "{revised}");
}

fn command(args: &[&str]) -> (i32, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
    let code = run(&args, &mut out, &mut err, &Abort::new());
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

// case: runs the built teammates command without a live app or a global install
#[test]
fn runs_the_built_teammates_command_without_a_live_app_or_a_global_install() {
    let app = fake_app(|_, _| Answer::Json(team_json()), 2_000);
    let (code, text, errors) = command(&["--profile", app.profile.to_str().unwrap(), "teammates"]);
    assert_eq!(code, 0, "{errors}");
    assert!(text.contains("One\tmate-one\tcodex\ttest-model"), "{text}");
    assert_eq!(errors, "");
}

// ---- a real door, with the editors' sessions behind it ---------------------------------------------------------

/// Locust as the editors' sessions see it: One on the team, a folder open, and a turn that waits on a card for two
/// polls, then streams "Hello", corrects it to "Hello world" and ends.
#[derive(Default)]
struct Host {
    starts: Mutex<Vec<CliStart>>,
    cancels: Mutex<Vec<String>>,
    card_reads: Arc<AtomicUsize>,
}

fn host_folder() -> String {
    std::env::temp_dir().to_string_lossy().into_owned()
}

impl CliHost for Host {
    fn teammates(&self) -> Result<Vec<crate::cli_sessions::CliTeammate>, String> {
        Ok(vec![crate::cli_sessions::CliTeammate { id: "mate-one".into(), name: "One".into(), runtime: Some("codex".into()), model: Some("test-model".into()), runs_in: "ask".into() }])
    }
    fn workspace(&self) -> Option<String> {
        Some(host_folder())
    }
    fn start(&self, request: CliStart, update: CliSink) -> Result<CliBegun, String> {
        self.starts.lock().unwrap().push(request);
        let reads = self.card_reads.clone();
        std::thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(10);
            while reads.load(Ordering::SeqCst) < 2 && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(5));
            }
            update(CliUpdate::Event(json!({"type": "message.delta", "payload": {"itemId": "a", "text": "Hello"}})));
            update(CliUpdate::Event(json!({"type": "message.delta", "payload": {"itemId": "a", "operation": "replace", "text": "Hello world"}})));
            update(CliUpdate::Event(json!({"type": "run.completed", "payload": {}})));
        });
        Ok(CliBegun { mission_id: "mission-one".into(), run_id: "run-one".into() })
    }
    fn cancel(&self, run_id: &str) {
        self.cancels.lock().unwrap().push(run_id.to_owned());
    }
    fn approvals(&self) -> Vec<CliCard> {
        self.card_reads.fetch_add(1, Ordering::SeqCst);
        vec![CliCard {
            approval_id: "card-one".into(),
            mission_id: "mission-one".into(),
            run_id: "run-one".into(),
            kind: "command".into(),
            summary: "Run the build".into(),
            detail: "cargo build".into(),
            cwd: None,
            answer_in: None,
        }]
    }
    fn answer(&self, _: &str, _: bool) -> bool {
        false
    }
    fn editor_answers(&self) -> bool {
        false
    }
    fn status(&self) -> Result<Value, String> {
        Ok(json!({"workspace": host_folder(), "turns": [{"missionId": "m1", "teammate": "One", "state": "running"}]}))
    }
}

#[test]
fn the_command_reads_status_and_sends_a_prompt_through_a_door_started_in_the_test() {
    let dir = temp("door");
    let host = Arc::new(Host::default());
    let sessions = Arc::new(CliSessions::new(host.clone(), CliOptions::default()));
    let behind = sessions.clone();
    // As `Door::for_store` routes them: the editors' names in front of the tools.
    let call: Caller = Arc::new(move |name: &str, args: &Value| {
        if name.starts_with("cli/") { behind.call(name, args).to_json() } else { crate::door_tools::ToolResult::refused("No such tool.").to_json() }
    });
    let door = Door::with(dir.clone(), "/x/locust-native", Arc::new(Mutex::new(Settings::default())), call).with_refusal(Arc::new(|| None));
    let state = door.set_enabled(true);
    assert!(state.settings.enabled, "{:?}", state.message);
    let profile = dir.to_str().unwrap();

    let (code, text, errors) = command(&["--profile", profile, "status"]);
    assert_eq!((code, text.as_str(), errors.as_str()), (0, "running\tOne\tm1\n", ""));

    let (code, text, errors) = command(&["--profile", profile, "send", "One", "Say hello"]);
    assert_eq!(code, 0, "{errors}");
    assert_eq!(text, "Hello world\n");
    assert_eq!(errors, "Waiting for you in Locust: Run the build\n", "said once, though two polls saw it");
    assert_eq!(
        *host.starts.lock().unwrap(),
        [CliStart { prompt: "Say hello".into(), teammate_id: "mate-one".into(), mode: "ask".into(), follow_up_of: None }]
    );
    assert_eq!(*host.cancels.lock().unwrap(), Vec::<String>::new(), "a finished turn is not stopped by closing its session");

    // A teammate Locust doesn't have is refused in Locust's own words, and nothing starts.
    let (code, _, errors) = command(&["--profile", profile, "send", "Nobody", "Hi"]);
    assert_eq!(code, 1);
    assert!(errors.starts_with("That teammate could not be identified. Choose one from locust teammates: One (mate-one)."), "{errors}");
    assert_eq!(host.starts.lock().unwrap().len(), 1);

    // Off, the command says how to turn the door on.
    door.set_enabled(false);
    let (code, _, errors) = command(&["--profile", profile, "status"]);
    assert_eq!(code, 1);
    assert_eq!(errors, format!("Locust is not open, or its local server is off. {SWITCH}\n"));
    door.dispose();
    sessions.dispose();
}

// My lane-check (24 Nov): the token's shape is checked too. A file at this computer's own address whose token isn't 64
// lowercase hex is refused before any request, like a file naming somewhere else.
#[test]
fn refuses_a_connection_file_whose_token_is_not_64_hex() {
    let app = fake_app(|_, _| Answer::Json(json!([])), 2_000);
    for token in ["a".repeat(63), "a".repeat(65), "A".repeat(64), "g".repeat(64), String::new()] {
        std::fs::write(app.profile.join(CONNECTION_FILE), json!({"url": app.url, "token": token}).to_string()).unwrap();
        let refused = app.client.teammates().unwrap_err();
        assert!(refused.0.starts_with("Locust is not open, or its local server is off."), "{token:?}: {refused}");
        assert_eq!(app.requests(), [], "{token:?}");
    }
}

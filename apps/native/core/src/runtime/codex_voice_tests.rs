use super::*;
use std::{fs, io::Write, path::Path, sync::mpsc::SyncSender};

const WAIT: Duration = crate::load_waits::UNDER_LOAD;
const OFFER: &str = "v=0\r\noffer";
const ANSWER: &str = "v=0\r\nanswer";

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Arc<Self> {
        let path = std::env::temp_dir().join(format!(
            "locust-codex-voice-{}",
            crate::invariants::unique_nanos()
        ));
        fs::create_dir_all(path.join("acme")).unwrap();
        fs::write(path.join("notifications"), b"").unwrap();
        Arc::new(Self(path))
    }

    fn wire(&self) -> Vec<Value> {
        fs::read_to_string(self.0.join("wire"))
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }

    fn params(&self, method: &str) -> Value {
        self.wire()
            .into_iter()
            .find(|wire| wire["method"] == method)
            .unwrap_or_else(|| panic!("No {method} on wire"))["params"]
            .clone()
    }

    fn reply(&self, id: u64) -> Value {
        self.wire()
            .into_iter()
            .find(|wire| wire["method"].is_null() && wire["id"] == id)
            .map_or(Value::Null, |wire| wire["result"].clone())
    }

    fn send(&self, wire: Value) {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(self.0.join("notifications"))
            .unwrap();
        writeln!(file, "{wire}").unwrap();
    }

    fn pids(&self) -> Vec<u32> {
        fs::read_to_string(self.0.join("pids"))
            .unwrap_or_default()
            .lines()
            .map(|line| line.parse().unwrap())
            .collect()
    }

    fn assert_gone(&self) {
        for pid in self.pids() {
            // A real child was started and reaped by Codex::drop.
            assert_eq!(unsafe { libc::kill(pid as i32, 0) }, -1);
            assert_eq!(io::Error::last_os_error().raw_os_error(), Some(libc::ESRCH));
        }
    }

    fn ask(&self, id: u64, server: &str, kind: &str, mode: &str) {
        self.send(json!({
            "id":id,"method":"mcpServer/elicitation/request","params":{
                "threadId":"thread_1","serverName":server,"mode":mode,
                "_meta":{"codex_approval_kind":kind,"tool_params_display":[
                    {"display_name":"subject","value":"Status"},{"display_name":"priority","value":1}
                ]},
                "message":format!("Allow the {server} MCP server to run tool \"send_note\"?")
            }
        }));
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Lease(Arc<Fixture>, AddOn);
impl CallAddOn for Lease {
    fn configuration(&self) -> &AddOn {
        &self.1
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        fs::write(self.0.0.join("addon-closed"), b"").unwrap();
    }
}

struct Hold {
    entered: SyncSender<()>,
    resume: Mutex<Receiver<()>>,
}
impl Hold {
    fn wait(&self) {
        // A replacement may look for Codex again after the first hold.
        if self.entered.send(()).is_ok() {
            let _ = crate::invariants::lock(&self.resume).recv_timeout(WAIT);
        }
    }
}
fn hold() -> (Hold, Receiver<()>, SyncSender<()>) {
    let (entered, notice) = mpsc::sync_channel(1);
    let (resume, waiter) = mpsc::sync_channel(1);
    (
        Hold {
            entered,
            resume: Mutex::new(waiter),
        },
        notice,
        resume,
    )
}

struct Host {
    fixture: Arc<Fixture>,
    scenario: &'static str,
    discover_hold: Option<Hold>,
    add_on_hold: Option<Hold>,
    add_on: bool,
    policy: Option<&'static str>,
}

impl CallHost for Host {
    fn policy_refusal(&self, _: &CallOptions) -> Option<String> {
        self.policy.map(str::to_owned)
    }

    fn discover(&self) -> Option<Command> {
        if let Some(hold) = &self.discover_hold {
            hold.wait();
        }
        if self.scenario == "missing" {
            return None;
        }
        let mut command = Command::new("python3");
        command
            .env_clear()
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../scripts/fixtures/voice-app-server.py"
            ))
            .arg(&self.fixture.0)
            .arg(self.scenario);
        Some(command)
    }

    fn open_add_on(&self, _: &CallOptions) -> Option<Box<dyn CallAddOn>> {
        if !self.add_on {
            return None;
        }
        fs::write(self.fixture.0.join("addon-opened"), b"").unwrap();
        if let Some(hold) = &self.add_on_hold {
            hold.wait();
        }
        Some(Box::new(Lease(
            self.fixture.clone(),
            AddOn {
                command: "Locust.exe".into(),
                args: vec!["bridge.mjs".into(), "connection.json".into()],
                env: [("ELECTRON_RUN_AS_NODE".into(), "1".into())].into(),
            },
        )))
    }
}

fn options(folder: &Path) -> CallOptions {
    CallOptions {
        folder: folder.join("acme"),
        offer: OFFER.into(),
        teammate: None,
        voice: None,
        preview: None,
        connectors: false,
        anchor_turn: None,
    }
}

fn setup(scenario: &'static str, changes: impl FnOnce(&mut Host)) -> (Arc<Fixture>, VoiceCalls) {
    let fixture = Fixture::new();
    let mut host = Host {
        fixture: fixture.clone(),
        scenario,
        discover_hold: None,
        add_on_hold: None,
        add_on: false,
        policy: None,
    };
    changes(&mut host);
    let mut voice = VoiceCalls::new(Arc::new(host));
    voice.times.request = WAIT;
    voice.times.answer = WAIT;
    voice.times.stop = Duration::from_millis(150);
    (fixture, voice)
}

fn ready(window: &CallWindow) {
    assert_eq!(
        window.started.recv_timeout(WAIT).unwrap(),
        Ok(ANSWER.into())
    );
}

fn event(window: &CallWindow) -> CallEvent {
    window.events.recv_timeout(WAIT).unwrap()
}

fn done(window: &CallWindow) {
    for _ in 0..40 {
        if matches!(event(window), CallEvent::Ended(_)) {
            return;
        }
    }
    panic!("Call never ended");
}

fn stop(voice: &VoiceCalls, window: &CallWindow, fixture: &Fixture) {
    voice.stop();
    done(window);
    fixture.assert_gone();
}

fn until(what: &str, condition: impl FnMut() -> bool) {
    crate::load_waits::wait_for(what, condition);
}

fn with_call(
    scenario: &'static str,
    changes: impl FnOnce(&mut CallOptions),
) -> (Arc<Fixture>, VoiceCalls, CallWindow) {
    let (fake, voice) = setup(scenario, |_| {});
    let mut opts = options(&fake.0);
    changes(&mut opts);
    let window = voice.start(opts).unwrap();
    ready(&window);
    (fake, voice, window)
}

// case: is asked of Codex on a read-only thread with every add-on off, and the answer comes back
#[test]
fn sends_the_offer_to_the_existing_codex_transport() {
    let (fake, voice, window) = with_call("", |_| {});
    let init = fake.params("initialize");
    assert_eq!(init["capabilities"]["experimentalApi"], true);
    let thread = fake.params("thread/start");
    assert_eq!(thread["cwd"], fake.0.join("acme").to_str().unwrap());
    assert_eq!(thread["sandbox"], "read-only");
    assert_eq!(thread["approvalPolicy"], "untrusted");
    assert_eq!(thread["ephemeral"], true);
    let mut expected = rules::thread_config(&json!({}), None, false).unwrap();
    expected["mcp_servers.robinhood.enabled"] = json!(false);
    expected["mcp_servers.remote_notes.enabled"] = json!(false);
    assert_eq!(thread["config"], expected);
    let start = fake.params("thread/realtime/start");
    assert_eq!(start["threadId"], "thread_1");
    assert_eq!(start["outputModality"], "audio");
    assert_eq!(start["version"], "v3");
    assert_eq!(start["transport"], json!({"type":"webrtc","sdp":OFFER}));
    assert!(
        start["realtimeStartInstructions"]
            .as_str()
            .unwrap()
            .contains("\"acme\"")
    );
    let wire = fake.wire();
    let methods: Vec<_> = wire
        .iter()
        .filter_map(|wire| wire["method"].as_str())
        .collect();
    assert_eq!(
        &methods[..5],
        &[
            "initialize",
            "initialized",
            "account/read",
            "config/read",
            "thread/start"
        ]
    );
    assert!(!methods.contains(&"mcpServerStatus/list"));
    assert_eq!(fake.pids().len(), 1);
    stop(&voice, &window, &fake);
}

// case: says what is said, refuses whatever Codex asks to do, and ends with End
#[test]
fn relays_words_and_declines_commands() {
    let (fake, voice, window) = with_call("", |_| {});
    for (method, role, key, words) in [
        ("delta", "user", "delta", "What is two"),
        ("done", "user", "text", "What is two plus two"),
        ("done", "assistant", "text", "That’s four."),
    ] {
        fake.send(
            json!({"method":format!("thread/realtime/transcript/{method}"),"params":{
                "threadId":"thread_1","role":role,(key):words
            }}),
        );
    }
    fake.send(json!({"method":"turn/started","params":{"threadId":"other"}}));
    fake.send(json!({"method":"turn/started","params":{"threadId":"thread_1"}}));
    fake.send(json!({"id":99,"method":"item/commandExecution/requestApproval","params":{"threadId":"thread_1"}}));
    assert_eq!(
        event(&window),
        CallEvent::Partial("you", "What is two".into())
    );
    assert_eq!(
        event(&window),
        CallEvent::Said("you", "What is two plus two".into())
    );
    assert_eq!(
        event(&window),
        CallEvent::Said("voice", "That’s four.".into())
    );
    assert_eq!(event(&window), CallEvent::Working(true));
    until("declines the command", || !fake.reply(99).is_null());
    assert_eq!(fake.reply(99), json!({"decision":"decline"}));
    stop(&voice, &window, &fake);
    assert_eq!(fake.params("thread/realtime/stop")["threadId"], "thread_1");
}

fn refused(scenario: &'static str, message: &str) {
    let (fake, voice) = setup(scenario, |_| {});
    let window = voice.start(options(&fake.0)).unwrap();
    assert!(
        window
            .started
            .recv_timeout(WAIT)
            .unwrap()
            .unwrap_err()
            .contains(message)
    );
    done(&window);
    fake.assert_gone();
    if scenario != "service-error" && scenario != "closed-on-start" {
        assert!(
            !fake
                .wire()
                .iter()
                .any(|wire| wire["method"] == "thread/start")
        );
    }
}

// case: is not placed on an API key, and says how to fix it
#[test]
fn requires_the_chatgpt_account() {
    refused("key", "Codex is signed in with an API key.");
}

// case: is not placed when the settings cannot be read: nothing is left on by accident (Sol’s #2)
#[test]
fn fails_closed_without_config() {
    refused("no-config", rules::SETTINGS_UNREAD);
}

// case: says the voice service’s refusal in words, and stops the server
#[test]
fn explains_service_errors() {
    refused("service-error", "sign Codex in with ChatGPT");
}

// case: needs a folder, and Codex
#[test]
fn needs_folder_and_codex_before_starting() {
    refused("missing", MISSING_CODEX);
    let (fake, voice) = setup("", |_| {});
    let opts = CallOptions {
        folder: PathBuf::new(),
        ..options(&fake.0)
    };
    let window = voice.start(opts).unwrap();
    assert_eq!(
        window.started.recv_timeout(WAIT).unwrap(),
        Err("Pick a folder first.".into())
    );
    done(&window);
    assert!(fake.pids().is_empty());
}

// case: runs one start at a time: a second press ends the first before it spawns anything (#6)
#[test]
fn replacement_takes_the_start_ticket() {
    let (held, entered, resume) = hold();
    let (fake, voice) = setup("", |host| host.discover_hold = Some(held));
    let first = voice.start(options(&fake.0)).unwrap();
    entered.recv_timeout(WAIT).unwrap();
    let second = voice.start(options(&fake.0)).unwrap();
    resume.send(()).unwrap();
    assert_eq!(
        first.started.recv_timeout(WAIT).unwrap(),
        Err(ENDED_EARLY.into())
    );
    entered.recv_timeout(WAIT).unwrap();
    resume.send(()).unwrap();
    ready(&second);
    assert_eq!(fake.pids().len(), 1);
    stop(&voice, &second, &fake);
}

// case: reaches a start still finding Codex: End then spawns nothing (#6)
#[test]
fn end_cancels_discovery_before_spawn() {
    let (held, entered, resume) = hold();
    let (fake, voice) = setup("", |host| host.discover_hold = Some(held));
    let window = voice.start(options(&fake.0)).unwrap();
    entered.recv_timeout(WAIT).unwrap();
    voice.stop();
    resume.send(()).unwrap();
    assert_eq!(
        window.started.recv_timeout(WAIT).unwrap(),
        Err(ENDED_EARLY.into())
    );
    done(&window);
    assert!(fake.pids().is_empty());
}

// case: reaches a start already talking to Codex: End stops it there and the server goes (#6)
#[test]
fn end_interrupts_a_wait_for_initialize() {
    let (fake, voice) = setup("no-initialize", |_| {});
    let window = voice.start(options(&fake.0)).unwrap();
    until("fake received initialize", || !fake.wire().is_empty());
    stop(&voice, &window, &fake);
    assert_eq!(
        window.started.recv_timeout(WAIT).unwrap(),
        Err(ENDED_EARLY.into())
    );
}

// case: hands back no answer for a call that closed as it came (#11)
#[test]
fn a_closed_call_has_no_answer() {
    refused("closed-on-start", "The voice service closed the call");
}

// case: does not wait on a Codex that never says it stopped (#12)
#[test]
fn end_reaps_a_server_that_does_not_acknowledge_stop() {
    let (fake, voice, window) = with_call("no-stop", |_| {});
    voice.stop();
    assert_eq!(
        window.events.recv_timeout(Duration::from_secs(5)).unwrap(),
        CallEvent::Ended(None)
    );
    fake.assert_gone();
}

// case: ends at once when the app quits
#[test]
fn quit_skips_the_stop_handshake() {
    let (fake, voice, window) = with_call("no-stop", |_| {});
    voice.dispose();
    done(&window);
    assert!(
        !fake
            .wire()
            .iter()
            .any(|wire| wire["method"] == "thread/realtime/stop")
    );
    fake.assert_gone();
}

// case: gives the call’s thread Locust’s own add-on, under the call’s own name, and tells Codex how to hand over
#[test]
fn add_on_uses_its_own_name_and_instructions() {
    let (fake, voice) = setup("", |host| host.add_on = true);
    let window = voice.start(options(&fake.0)).unwrap();
    ready(&window);
    let cfg = fake.params("thread/start")["config"].clone();
    assert_eq!(cfg["features.plugins"], false);
    assert_eq!(cfg["mcp_servers.locust_call.command"], "Locust.exe");
    assert_eq!(
        cfg["mcp_servers.locust_call.args"],
        json!(["bridge.mjs", "connection.json"])
    );
    assert_eq!(
        cfg["mcp_servers.locust_call.env"],
        json!({"ELECTRON_RUN_AS_NODE":"1"})
    );
    assert_eq!(cfg["mcp_servers.locust_call.enabled"], true);
    let start = fake.params("thread/realtime/start");
    assert!(
        start["realtimeStartInstructions"]
            .as_str()
            .unwrap()
            .contains("locust_call tools")
    );
    assert!(
        start["realtimeStartInstructions"]
            .as_str()
            .unwrap()
            .contains("wait for them to say yes")
    );
    stop(&voice, &window, &fake);
}

// case: lets Codex use that add-on and nothing else, and says a hand-off in the strip
#[test]
fn allows_own_tools_only_and_reports_handoffs() {
    let (fake, voice) = setup("", |host| host.add_on = true);
    let window = voice.start(options(&fake.0)).unwrap();
    ready(&window);
    fake.ask(71, "locust_call", "mcp_tool_call", "form");
    fake.ask(72, "robinhood", "mcp_tool_call", "form");
    fake.ask(73, "locust_call", "something_else", "form");
    fake.send(
        json!({"method":"item/started","params":{"threadId":"thread_1","item":{
            "type":"mcpToolCall","server":"locust_call","tool":"start_conversation",
            "arguments":{"teammate":"Wren","message":"Fix the login redirect loop"}
        }}}),
    );
    assert_eq!(
        event(&window),
        CallEvent::Handed {
            to: "Wren".into(),
            task: "Fix the login redirect loop".into()
        }
    );
    until("all tool replies", || !fake.reply(73).is_null());
    assert_eq!(fake.reply(71), json!({"action":"accept","content":{}}));
    assert_eq!(fake.reply(72), json!({"action":"decline"}));
    assert_eq!(fake.reply(73), json!({"action":"decline"}));
    stop(&voice, &window, &fake);
}

// case: closes the add-on with the call
#[test]
fn the_private_server_has_the_lifetime_of_the_call() {
    let (fake, voice) = setup("", |host| host.add_on = true);
    let window = voice.start(options(&fake.0)).unwrap();
    ready(&window);
    assert!(!fake.0.join("addon-closed").exists());
    stop(&voice, &window, &fake);
    assert!(fake.0.join("addon-closed").exists());
}

// case: still talks without it, and then lets Codex use nothing
#[test]
fn without_an_add_on_the_call_still_starts_sealed() {
    let (fake, voice, window) = with_call("", |_| {});
    assert!(
        fake.params("thread/start")["config"]
            .get("mcp_servers.locust_call.enabled")
            .is_none()
    );
    assert!(
        !fake.params("thread/realtime/start")["realtimeStartInstructions"]
            .as_str()
            .unwrap()
            .contains("locust_call")
    );
    fake.ask(81, "locust_call", "mcp_tool_call", "form");
    until("declines unopened add-on", || !fake.reply(81).is_null());
    assert_eq!(fake.reply(81), json!({"action":"decline"}));
    stop(&voice, &window, &fake);
    assert!(!fake.0.join("addon-closed").exists());
}

// case: keeps the person’s connectors on for the call’s thread, and its thinking quick
#[test]
fn connector_mode_keeps_only_remote_servers_and_uses_low_effort() {
    let (fake, voice, window) = with_call("", |options| options.connectors = true);
    let config = fake.params("thread/start")["config"].clone();
    assert!(config.get("features.apps").is_none());
    assert!(config.get("mcp_servers.remote_notes.enabled").is_none());
    assert_eq!(config["mcp_servers.robinhood.enabled"], false);
    assert_eq!(config["model_reasoning_effort"], "low");
    assert!(
        fake.params("thread/realtime/start")["realtimeStartInstructions"]
            .as_str()
            .unwrap()
            .contains("Allow and Deny in the call’s strip")
    );
    stop(&voice, &window, &fake);
}

fn question(window: &CallWindow) -> (String, String, Vec<(String, String)>) {
    let CallEvent::Asks { id, what, fields } = event(window) else {
        panic!("expected a connector question");
    };
    (id, what, fields)
}

// case: asks the person before a connector changes anything, and does it only on Allow
#[test]
fn connector_waits_for_exactly_one_explicit_allow() {
    let (fake, voice, window) = with_call("", |options| options.connectors = true);
    fake.ask(50, "codex_apps", "mcp_tool_call", "form");
    let (id, what, fields) = question(&window);
    assert_eq!(what, "ChatGPT: send note");
    assert_eq!(
        fields,
        vec![
            ("subject".into(), "Status".into()),
            ("priority".into(), "1".into())
        ]
    );
    assert!(fake.reply(50).is_null());
    voice.answer(id.clone(), true);
    assert_eq!(
        event(&window),
        CallEvent::Asked {
            id: id.clone(),
            allowed: true
        }
    );
    voice.answer(id, false);
    voice.answer("nobody".into(), true);
    stop(&voice, &window, &fake);
    assert_eq!(fake.reply(50), json!({"action":"accept","content":{}}));
    assert_eq!(
        fake.wire().iter().filter(|wire| wire["id"] == 50).count(),
        1
    );
}

// case: is a Deny on Deny, when nobody answers in time, and when the call ends
#[test]
fn refusal_timeout_and_end_all_decline_the_connector() {
    let (fake, mut voice) = setup("", |_| {});
    voice.times.ask = Duration::from_secs(1);
    let window = voice
        .start(CallOptions {
            connectors: true,
            ..options(&fake.0)
        })
        .unwrap();
    ready(&window);
    for id in [51, 52, 53] {
        fake.ask(id, "robinhood", "mcp_tool_call", "form");
        let (question, _, _) = question(&window);
        match id {
            51 => voice.answer(question.clone(), false),
            53 => voice.stop(),
            _ => {}
        }
        assert_eq!(
            event(&window),
            CallEvent::Asked {
                id: question,
                allowed: false
            }
        );
    }
    done(&window);
    for id in 51..=53 {
        assert_eq!(fake.reply(id), json!({"action":"decline"}));
    }
}

// case: declines unasked what is not a connector’s tool call, and every connector when they are off
#[test]
fn connector_gate_declines_other_questions_and_disabled_connectors() {
    let (fake, voice, window) = with_call("", |options| options.connectors = true);
    fake.ask(60, "robinhood", "mcp_tool_call", "url");
    fake.send(json!({"id":61,"method":"item/fileChange/requestApproval","params":{"threadId":"thread_1"}}));
    until("unsupported requests settled", || !fake.reply(61).is_null());
    assert_eq!(fake.reply(60), json!({"action":"decline"}));
    assert_eq!(fake.reply(61), json!({"decision":"decline"}));
    assert!(window.events.try_recv().is_err());
    stop(&voice, &window, &fake);

    let (fake, voice, window) = with_call("", |_| {});
    fake.ask(62, "robinhood", "mcp_tool_call", "form");
    until("disabled connector refused", || !fake.reply(62).is_null());
    assert_eq!(fake.reply(62), json!({"action":"decline"}));
    assert!(window.events.try_recv().is_err());
    stop(&voice, &window, &fake);
}

// case: is the one chosen, and Codex’s own when none is or it cannot be read
#[test]
fn chosen_voice_is_optional() {
    let (fake, voice, window) = with_call("", |options| options.voice = Some("sol".into()));
    assert_eq!(fake.params("thread/realtime/start")["voice"], "sol");
    stop(&voice, &window, &fake);
    let (fake, voice, window) = with_call("", |_| {});
    assert!(fake.params("thread/realtime/start").get("voice").is_none());
    stop(&voice, &window, &fake);
}

// case: is an ordinary call when the teammate cannot be found
#[test]
fn no_resolved_teammate_uses_ordinary_instructions() {
    let (fake, voice, window) = with_call("", |_| {});
    let start = fake.params("thread/realtime/start");
    assert!(
        !start["realtimeStartInstructions"]
            .as_str()
            .unwrap()
            .contains("You are")
    );
    assert!(
        start["initialItems"][0]["text"]
            .as_str()
            .unwrap()
            .contains("the voice of a call the person started")
    );
    stop(&voice, &window, &fake);
}

// case: is a call with that voice alone, told only to greet
#[test]
fn preview_has_no_connectors_or_context() {
    let (fake, voice) = setup("", |host| host.add_on = true);
    let window = voice
        .start(CallOptions {
            connectors: true,
            preview: Some("sol".into()),
            ..options(&fake.0)
        })
        .unwrap();
    ready(&window);
    assert!(!fake.0.join("addon-opened").exists());
    let config = fake.params("thread/start")["config"].clone();
    assert_eq!(config["features.apps"], false);
    assert_eq!(config["mcp_servers.remote_notes.enabled"], false);
    let params = fake.params("thread/realtime/start");
    assert_eq!(params["voice"], "sol");
    assert_eq!(params["version"], "v3");
    assert_eq!(params["includeStartupContext"], false);
    assert!(params.get("realtimeStartInstructions").is_none());
    assert_eq!(
        params["initialItems"],
        json!([{"role":"developer","text":rules::preview_rules("sol")}])
    );
    assert!(
        params["initialItems"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Your voice is called Sol")
    );
    stop(&voice, &window, &fake);
}

// case: never ends a call that is on, and one preview may take the place of another
#[test]
fn preview_cannot_evict_a_call_but_may_replace_a_preview() {
    let (fake, voice, window) = with_call("", |_| {});
    let preview = |name: &str| CallOptions {
        preview: Some(name.into()),
        ..options(&fake.0)
    };
    assert!(
        matches!(voice.start(preview("sol")), Err(message) if message == "A call is on. End it to hear a voice.")
    );
    assert_eq!(fake.pids().len(), 1);
    stop(&voice, &window, &fake);
    let previous = voice.start(preview("sol")).unwrap();
    ready(&previous);
    let next = voice.start(preview("cove")).unwrap();
    ready(&next);
    done(&previous);
    assert_eq!(fake.pids().len(), 3);
    stop(&voice, &next, &fake);
}

// case: closes the add-on once it has opened
#[test]
fn cancellation_reaps_an_add_on_that_finishes_opening_late() {
    let (held, entered, resume) = hold();
    let (fake, voice) = setup("", |host| {
        host.add_on = true;
        host.add_on_hold = Some(held);
    });
    let window = voice.start(options(&fake.0)).unwrap();
    entered.recv_timeout(WAIT).unwrap();
    voice.stop();
    assert!(!fake.0.join("addon-closed").exists());
    resume.send(()).unwrap();
    assert_eq!(
        window.started.recv_timeout(WAIT).unwrap(),
        Err(ENDED_EARLY.into())
    );
    done(&window);
    assert!(fake.0.join("addon-closed").exists());
    fake.assert_gone();
}

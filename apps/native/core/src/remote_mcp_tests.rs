//! Electron's an-app-uses-the-team-from-anywhere.test.ts, on this computer's side of it: the relay is a fake that
//! records what it is told, the tools are a fake that records what they are asked, and the clock is the test's own.
//! Nothing here reaches a relay, a keychain or a real token.
use super::*;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

const START: i64 = 1_800_000_000_000;
const REDIRECT: &str = "https://claude.ai/api/mcp/auth_callback";

struct FakeRelay {
    address: Mutex<Option<String>>,
    opened: Mutex<Vec<bool>>,
    reachable: AtomicBool,
}
impl McpRelay for FakeRelay {
    fn address(&self) -> Option<String> {
        self.address.lock().unwrap().clone()
    }
    fn open(&self, on: bool) -> bool {
        self.opened.lock().unwrap().push(on);
        self.reachable.load(Ordering::SeqCst)
    }
}
#[derive(Default)]
struct FakeTools {
    calls: Mutex<Vec<(String, Value)>>,
    fail: AtomicBool,
    huge: AtomicBool,
}
impl McpTools for FakeTools {
    fn call(&self, name: &str, arguments: &Value) -> Result<ToolResult, String> {
        self.calls.lock().unwrap().push((name.into(), arguments.clone()));
        if self.fail.load(Ordering::SeqCst) {
            return Err("boom".into());
        }
        if self.huge.load(Ordering::SeqCst) {
            return Ok(ToolResult::ok("x".repeat(MAX_ANSWER_CHARS)));
        }
        Ok(ToolResult::ok(json!([{"id": "tm_wren", "name": "Wren"}]).to_string()))
    }
}

struct World {
    remote: Arc<RemoteMcp>,
    relay: Arc<FakeRelay>,
    tools: Arc<FakeTools>,
    now: Arc<AtomicI64>,
    folder: PathBuf,
    told: Arc<Mutex<Vec<RemoteMcpState>>>,
    said: Arc<Mutex<Vec<String>>>,
    refused: Arc<Mutex<Option<String>>>,
}

fn folder(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("locust-remote-mcp-{label}-{}-{}", std::process::id(), random_base64url(6).unwrap()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
fn world(label: &str) -> World {
    world_waiting(label, Duration::from_secs(20))
}
fn world_waiting(label: &str, wait: Duration) -> World {
    let folder = folder(label);
    let relay = Arc::new(FakeRelay { address: Mutex::new(Some("https://app.locust.lol/v1/room/r/mcp".into())), opened: Mutex::new(vec![]), reachable: AtomicBool::new(true) });
    let tools = Arc::new(FakeTools::default());
    let now = Arc::new(AtomicI64::new(START));
    let told = Arc::new(Mutex::new(Vec::new()));
    let said = Arc::new(Mutex::new(Vec::new()));
    let refused = Arc::new(Mutex::new(None));
    let mut options = Options::new(folder.join(REMOTE_MCP_FILE), relay.clone(), tools.clone());
    let clock = now.clone();
    options.now = Arc::new(move || clock.load(Ordering::SeqCst));
    let t = told.clone();
    options.changed = Arc::new(move |state| t.lock().unwrap().push(state.clone()));
    let s = said.clone();
    options.asked = Arc::new(move |name| s.lock().unwrap().push(name.into()));
    let r = refused.clone();
    options.refusal = Arc::new(move || r.lock().unwrap().clone());
    options.asking_wait = wait;
    World { remote: Arc::new(RemoteMcp::new(options)), relay, tools, now, folder, told, said, refused }
}
impl World {
    fn tick(&self, ms: i64) {
        self.now.fetch_add(ms, Ordering::SeqCst);
    }
    fn file(&self) -> String {
        std::fs::read_to_string(self.folder.join(REMOTE_MCP_FILE)).unwrap_or_default()
    }
    fn token(&self, body: &str) -> RemoteMcpReply {
        self.remote.answer(&RemoteMcpAsk { ask: "t".into(), kind: AskKind::Token, body: body.into(), auth: None })
    }
    fn rpc(&self, key: &str, method: &str, params: Option<Value>, id: Option<i64>) -> (u16, Option<Value>) {
        let mut body = json!({"jsonrpc": "2.0", "method": method});
        if let Some(id) = id {
            body["id"] = json!(id);
        }
        if let Some(params) = params {
            body["params"] = params;
        }
        self.raw(key, &body.to_string())
    }
    fn raw(&self, key: &str, body: &str) -> (u16, Option<Value>) {
        let answer = self.remote.answer(&RemoteMcpAsk { ask: "c".into(), kind: AskKind::Call, body: body.into(), auth: Some(key.into()) });
        let json = if answer.body.is_empty() { None } else { Some(serde_json::from_str(&answer.body).unwrap()) };
        (answer.status, json)
    }
    /// An app's sign-in as the relay passes it: this blocks until the person answers, so it runs on its own thread.
    fn start_sign_in(&self, over: Value) -> std::thread::JoinHandle<RemoteMcpReply> {
        let remote = self.remote.clone();
        let mut body = json!({"client_id": "client_1", "client": "Claude", "redirect": REDIRECT, "challenge": challenge_of(&verifier()), "match": "123456"});
        for (k, v) in over.as_object().unwrap() {
            body[k] = v.clone();
        }
        std::thread::spawn(move || remote.answer(&RemoteMcpAsk { ask: "a".into(), kind: AskKind::Connect, body: body.to_string(), auth: None }))
    }
    fn card(&self) -> RemoteMcpAsking {
        for _ in 0..400 {
            if let Some(card) = self.remote.settings().asking.first() {
                return card.clone();
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("no card");
    }
    /// The whole sign-in, allowed: the code from the card, traded for keys.
    fn connected(&self) -> (Value, String) {
        let verifier = verifier();
        let sign_in = self.start_sign_in(json!({"challenge": challenge_of(&verifier)}));
        let card = self.card();
        self.remote.answer_sign_in(&card.request, true);
        let code: Value = serde_json::from_str(&sign_in.join().unwrap().body).unwrap();
        let code = code["code"].as_str().unwrap().to_owned();
        let keys = self.token(&form(&[("grant_type", "authorization_code"), ("code", &code), ("code_verifier", &verifier), ("client_id", "client_1"), ("redirect_uri", REDIRECT)]));
        assert_eq!(keys.status, 200, "{}", keys.body);
        (serde_json::from_str(&keys.body).unwrap(), code)
    }
}
impl Drop for World {
    fn drop(&mut self) {
        self.remote.dispose();
        let _ = std::fs::remove_dir_all(&self.folder);
    }
}
fn verifier() -> String {
    random_base64url(32).unwrap()
}
fn challenge_of(verifier: &str) -> String {
    sha256_base64url(verifier)
}
fn form(pairs: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new()).extend_pairs(pairs).finish()
}
fn body(reply: &RemoteMcpReply) -> Value {
    serde_json::from_str(&reply.body).unwrap()
}

#[test]
fn signs_in_only_when_the_person_allows_its_card_which_shows_the_six_digits_its_page_does() {
    let room = world("allow");
    // Off: no card is made, nothing is decided by asking.
    let off = room.start_sign_in(json!({})).join().unwrap();
    assert_eq!((off.status, body(&off)), (403, json!({"error": "denied"})));
    room.remote.set_enabled(&json!(true));
    let sign_in = room.start_sign_in(json!({"match": "482915"}));
    let card = room.card();
    assert_eq!((card.name.as_str(), card.host.as_str(), card.match_digits.as_str()), ("Claude", "claude.ai", "482915"));
    assert!(card.match_digits.len() == 6 && card.match_digits.bytes().all(|b| b.is_ascii_digit()));
    assert_eq!(card.until, START + ASKING_MS);
    assert_eq!(*room.said.lock().unwrap(), vec!["Claude".to_string()]);
    // Nothing is decided by waiting: the sign-in is still held.
    std::thread::sleep(Duration::from_millis(30));
    assert!(!sign_in.is_finished());
    room.remote.answer_sign_in(&card.request, true);
    let made = sign_in.join().unwrap();
    assert_eq!(made.status, 200);
    assert!(is_key(body(&made)["code"].as_str().unwrap()), "{}", made.body);
    assert!(room.remote.settings().asking.is_empty());
}

#[test]
fn sends_the_app_its_own_word_for_no_when_the_person_denies_it_and_makes_no_code() {
    let room = world("deny");
    room.remote.set_enabled(&json!(true));
    let sign_in = room.start_sign_in(json!({}));
    let card = room.card();
    room.remote.answer_sign_in(&card.request, false);
    let denied = sign_in.join().unwrap();
    assert_eq!((denied.status, body(&denied)), (403, json!({"error": "denied"})));
    assert!(room.remote.settings().asking.is_empty());
    // A code never made cannot be traded.
    let tried = room.token(&form(&[("grant_type", "authorization_code"), ("code", &verifier()), ("code_verifier", &verifier()), ("client_id", "client_1")]));
    assert_eq!(body(&tried)["error"], "invalid_grant");
}

#[test]
fn trades_the_code_for_keys_once_and_only_with_the_proof_the_app_started_with() {
    let room = world("proof");
    room.remote.set_enabled(&json!(true));
    let proof = verifier();
    let sign_in = room.start_sign_in(json!({"challenge": challenge_of(&proof)}));
    room.remote.answer_sign_in(&room.card().request, true);
    let code = body(&sign_in.join().unwrap())["code"].as_str().unwrap().to_owned();
    // A wrong proof spends the code: it is good once, right or wrong.
    let wrong = room.token(&form(&[("grant_type", "authorization_code"), ("code", &code), ("code_verifier", &verifier()), ("client_id", "client_1"), ("redirect_uri", REDIRECT)]));
    assert_eq!(wrong.status, 400);
    assert_eq!(body(&wrong), json!({"error": "invalid_grant", "error_description": "The app could not prove it started this sign-in."}));
    let again = room.token(&form(&[("grant_type", "authorization_code"), ("code", &code), ("code_verifier", &proof), ("client_id", "client_1"), ("redirect_uri", REDIRECT)]));
    assert_eq!(again.status, 400);
    assert_eq!(body(&again)["error_description"], "That code is not one this Locust made, or it was used already.");
}

#[test]
fn a_code_made_for_another_app_or_another_address_or_one_that_ran_out_gives_no_keys() {
    let room = world("code-rules");
    room.remote.set_enabled(&json!(true));
    let trade = |room: &World, client: &str, redirect: Option<&str>, wait: i64| {
        let proof = verifier();
        let sign_in = room.start_sign_in(json!({"challenge": challenge_of(&proof)}));
        room.remote.answer_sign_in(&room.card().request, true);
        let code = body(&sign_in.join().unwrap())["code"].as_str().unwrap().to_owned();
        room.tick(wait);
        let mut pairs = vec![("grant_type", "authorization_code"), ("code", code.as_str()), ("code_verifier", proof.as_str()), ("client_id", client)];
        if let Some(redirect) = redirect {
            pairs.push(("redirect_uri", redirect));
        }
        room.token(&form(&pairs))
    };
    let other_app = trade(&room, "client_2", Some(REDIRECT), 0);
    assert_eq!(body(&other_app)["error_description"], "That code was made for another app.");
    let other_address = trade(&room, "client_1", Some("https://evil.example/cb"), 0);
    assert_eq!(body(&other_address)["error_description"], "That code was made for another app.");
    let late = trade(&room, "client_1", Some(REDIRECT), CODE_MS);
    assert_eq!(body(&late)["error_description"], "That code ran out. Connect again from the app.");
    // The same code a moment sooner, with no redirect named (the code's own is meant): keys.
    let fine = trade(&room, "client_1", None, CODE_MS - 1);
    assert_eq!(fine.status, 200, "{}", fine.body);
}

#[test]
fn a_token_request_is_read_as_a_form_or_as_json_and_anything_else_is_not_read() {
    let room = world("shapes");
    room.remote.set_enabled(&json!(true));
    let (keys, _) = room.connected();
    let refresh = keys["refresh_token"].as_str().unwrap();
    let as_json = room.token(&json!({"grant_type": "refresh_token", "refresh_token": refresh}).to_string());
    assert_eq!(as_json.status, 200, "{}", as_json.body);
    assert_eq!(room.token("{not json").status, 400);
    assert_eq!(body(&room.token("{not json")), json!({"error": "invalid_request", "error_description": "The request could not be read."}));
    assert_eq!(body(&room.token(&form(&[("grant_type", "password")]))), json!({"error": "unsupported_grant_type"}));
    assert_eq!(body(&room.token("")), json!({"error": "unsupported_grant_type"}));
}

#[test]
fn answers_mcp_with_the_same_tools_and_rules_as_an_app_on_this_computer_for_a_good_key_only() {
    let room = world("mcp");
    room.remote.set_enabled(&json!(true));
    let (keys, _) = room.connected();
    assert_eq!((keys["token_type"].as_str(), keys["expires_in"].as_i64(), keys["scope"].as_str()), (Some("Bearer"), Some(86_400), Some("locust")));
    let access = keys["access_token"].as_str().unwrap().to_owned();
    let (status, init) = room.rpc(&access, "initialize", Some(json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "claude"}})), Some(1));
    assert_eq!(status, 200);
    let result = &init.unwrap()["result"];
    assert_eq!((&result["protocolVersion"], &result["capabilities"], &result["serverInfo"]["name"]), (&json!("2025-06-18"), &json!({"tools": {}}), &json!("locust")));
    assert_eq!(room.rpc(&access, "notifications/initialized", None, None), (202, None));
    let (_, listed) = room.rpc(&access, "tools/list", None, Some(2));
    let names: Vec<String> = listed.unwrap()["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_owned()).collect();
    assert_eq!(names, ["list_teammates", "start_conversation", "send_message", "read_reply", "stop_turn", "list_conversations", "carry_on", "list_models", "list_background_runs", "list_routines", "run_routine", "routine_status"]);
    let (_, called) = room.rpc(&access, "tools/call", Some(json!({"name": "list_teammates", "arguments": {}})), Some(3));
    assert!(called.unwrap()["result"]["content"][0]["text"].as_str().unwrap().contains("Wren"));
    assert_eq!(*room.tools.calls.lock().unwrap(), vec![("list_teammates".to_string(), json!({}))]);
    // Only the listed tools: never Locust's own command-line ones.
    let (_, cli) = room.rpc(&access, "tools/call", Some(json!({"name": "cli/start", "arguments": {}})), Some(4));
    assert_eq!(cli.unwrap()["error"]["code"], -32602);
    assert_eq!(room.tools.calls.lock().unwrap().len(), 1);
    assert_eq!(room.rpc(&verifier(), "tools/list", None, Some(5)).0, 401);
    assert_eq!(room.rpc("short", "tools/list", None, Some(5)).0, 401);
    // A day on, the key is spent; the refresh key makes new ones, and the old refresh key is gone.
    room.tick(ACCESS_MS + 1);
    assert_eq!(room.rpc(&access, "tools/list", None, Some(6)).0, 401);
    let refresh = keys["refresh_token"].as_str().unwrap();
    let renewed = room.token(&form(&[("grant_type", "refresh_token"), ("refresh_token", refresh)]));
    assert_eq!(renewed.status, 200, "{}", renewed.body);
    let fresh = body(&renewed);
    assert_eq!(room.rpc(fresh["access_token"].as_str().unwrap(), "tools/list", None, Some(7)).0, 200);
    assert_eq!(room.token(&form(&[("grant_type", "refresh_token"), ("refresh_token", refresh)])).status, 400);
}

#[test]
fn the_tools_are_the_twelve_word_for_word_that_an_app_on_this_computer_is_offered() {
    let bridge_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../desktop/resources/locust-mcp-bridge.mjs");
    let bridge = std::fs::read_to_string(&bridge_path).unwrap_or_else(|e| panic!("{}: {e}", bridge_path.display()));
    let reads = {
        let start = bridge.find("READS = new Set([").expect("READS in the bridge") + "READS = new Set([".len();
        bridge[start..start + bridge[start..].find(']').unwrap()].to_owned()
    };
    let tools = remote_mcp_tools();
    let tools = tools.as_array().unwrap();
    for tool in tools {
        let (name, description) = (tool["name"].as_str().unwrap(), tool["description"].as_str().unwrap());
        let line = format!("{{ name: '{name}', description: '{}'", description.replace('\'', "\\'"));
        assert!(bridge.contains(&line), "the bridge words {name} differently");
        assert_eq!(reads.contains(&format!("'{name}'")), tool.get("annotations").is_some(), "{name}: reads");
    }
    let start = bridge.find("const TOOLS = [").unwrap();
    let listed = &bridge[start..start + bridge[start..].find("\n]").unwrap()];
    let in_bridge: Vec<&str> = listed.lines().filter_map(|l| l.strip_prefix("  { name: '")).filter_map(|l| l.split('\'').next()).collect();
    let ours: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(in_bridge, ours);
    assert_eq!(ours.len(), 12);
}

#[test]
fn keeps_only_hashes_of_the_keys_and_remove_or_off_takes_them_away_at_once() {
    let room = world("hashes");
    room.remote.set_enabled(&json!(true));
    let (keys, code) = room.connected();
    let (access, refresh) = (keys["access_token"].as_str().unwrap().to_owned(), keys["refresh_token"].as_str().unwrap().to_owned());
    let file = room.file();
    assert!(!file.contains(&access) && !file.contains(&refresh) && !file.contains(&code));
    assert!(file.contains(&sha256_base64url(&access)) && file.contains(&sha256_base64url(&refresh)));
    // Nothing the window is told carries a key either.
    for state in room.told.lock().unwrap().iter() {
        let text = state.to_json().to_string();
        assert!(!text.contains(&access) && !text.contains(&refresh) && !text.contains(&code));
    }
    let app = room.remote.settings().apps[0].clone();
    assert_eq!((app.name.as_str(), app.host.as_str()), ("Claude", "claude.ai"));
    room.remote.remove(&app.id);
    assert_eq!(room.rpc(&access, "tools/list", None, Some(1)).0, 401);
    assert!(!room.file().contains("claude.ai"));
    let (again, _) = room.connected();
    let again_access = again["access_token"].as_str().unwrap().to_owned();
    assert_eq!(room.rpc(&again_access, "tools/list", None, Some(1)).0, 200);
    room.remote.set_enabled(&json!(false));
    let state = room.remote.settings();
    assert!(!state.enabled && state.apps.is_empty());
    assert_eq!(room.rpc(&again_access, "tools/list", None, Some(1)).0, 401);
    assert_eq!(room.relay.opened.lock().unwrap().last(), Some(&false));
    assert!(room.file().contains("\"grants\":[]") && room.file().contains("\"enabled\":false"));
}

#[test]
fn asks_nothing_of_a_computer_that_is_off_and_refuses_a_sign_in_that_is_not_one() {
    let room = world("refuse");
    room.remote.set_enabled(&json!(true));
    let denied = |over: Value| {
        let held = room.start_sign_in(over);
        // Refused at once: a card that was made would hold the sign-in for the person's answer.
        for _ in 0..200 {
            if held.is_finished() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(held.is_finished(), "a card was made for a sign-in that is not one");
        let one = held.join().unwrap();
        (one.status, body(&one))
    };
    let no = (403, json!({"error": "denied"}));
    assert_eq!(denied(json!({"client_id": ""})), no);
    assert_eq!(denied(json!({"redirect": ""})), no);
    assert_eq!(denied(json!({"challenge": "short"})), no);
    assert_eq!(denied(json!({"challenge": "a".repeat(44)})), no);
    assert_eq!(denied(json!({"match": "12345"})), no);
    assert_eq!(denied(json!({"match": "12345a"})), no);
    assert_eq!(denied(json!({"match": 123456})), no);
    assert!(room.remote.settings().asking.is_empty());
    let unread = room.remote.answer(&RemoteMcpAsk { ask: "a".into(), kind: AskKind::Connect, body: "not json".into(), auth: None });
    assert_eq!((unread.status, body(&unread)), (400, json!({"error": "bad-ask"})));
    // A rule against it refuses the card, the keys and the calls.
    let (keys, _) = room.connected();
    let access = keys["access_token"].as_str().unwrap().to_owned();
    *room.refused.lock().unwrap() = Some("Your organisation does not allow it.".into());
    assert_eq!(denied(json!({})), no);
    assert_eq!(room.rpc(&access, "tools/list", None, Some(1)).0, 401);
    assert_eq!(room.token(&form(&[("grant_type", "refresh_token"), ("refresh_token", keys["refresh_token"].as_str().unwrap())])).status, 400);
    *room.refused.lock().unwrap() = None;
    room.remote.set_enabled(&json!(false));
    assert_eq!(denied(json!({})), no);
    assert_eq!(body(&room.token(&form(&[("grant_type", "refresh_token"), ("refresh_token", "x")])))["error_description"], "This Locust no longer takes apps from anywhere.");
    assert_eq!(room.rpc(&access, "tools/list", None, Some(1)).0, 401);
}

#[test]
fn a_card_nobody_answers_goes_after_its_time_and_makes_no_code() {
    let room = world_waiting("timeout", Duration::from_millis(80));
    room.remote.set_enabled(&json!(true));
    let sign_in = room.start_sign_in(json!({}));
    room.card();
    let denied = sign_in.join().unwrap();
    assert_eq!((denied.status, body(&denied)), (403, json!({"error": "denied"})));
    assert!(room.remote.settings().asking.is_empty());
    assert!(room.told.lock().unwrap().last().unwrap().asking.is_empty());
    // A card past its own time is no longer shown, though still held.
    let late = world_waiting("late-card", Duration::from_secs(20));
    late.remote.set_enabled(&json!(true));
    let held = late.start_sign_in(json!({}));
    let card = late.card();
    late.tick(ASKING_MS + 1);
    assert!(late.remote.settings().asking.is_empty());
    late.remote.answer_sign_in(&card.request, false);
    held.join().unwrap();
}

#[test]
fn turning_it_off_ends_a_card_waiting_and_a_code_not_yet_traded() {
    let room = world("off-with-card");
    room.remote.set_enabled(&json!(true));
    let proof = verifier();
    let first = room.start_sign_in(json!({"challenge": challenge_of(&proof)}));
    room.remote.answer_sign_in(&room.card().request, true);
    let code = body(&first.join().unwrap())["code"].as_str().unwrap().to_owned();
    let second = room.start_sign_in(json!({}));
    room.card();
    room.remote.set_enabled(&json!(false));
    assert_eq!(second.join().unwrap().status, 403);
    assert!(room.remote.settings().asking.is_empty());
    room.remote.set_enabled(&json!(true));
    let traded = room.token(&form(&[("grant_type", "authorization_code"), ("code", &code), ("code_verifier", &proof), ("client_id", "client_1"), ("redirect_uri", REDIRECT)]));
    assert_eq!(body(&traded)["error_description"], "That code is not one this Locust made, or it was used already.");
    // An answer to a card that is gone changes nothing.
    assert!(room.remote.answer_sign_in("nobody", true).asking.is_empty());
}

#[test]
fn an_allow_that_arrives_after_it_was_turned_off_makes_no_code() {
    let room = world("allow-after-off");
    room.remote.set_enabled(&json!(true));
    let sign_in = room.start_sign_in(json!({}));
    let card = room.card();
    room.remote.set_enabled(&json!(false));
    room.remote.answer_sign_in(&card.request, true);
    assert_eq!(sign_in.join().unwrap().status, 403);
}

#[test]
fn it_is_turned_on_only_with_the_relay_reached_and_from_anywhere_on_and_no_rule_against_it() {
    let room = world("enable");
    *room.relay.address.lock().unwrap() = None;
    let state = room.remote.set_enabled(&json!(true));
    assert_eq!((state.enabled, state.message.as_deref()), (false, Some(NEEDS_FROM_ANYWHERE)));
    assert!(room.relay.opened.lock().unwrap().is_empty());
    *room.relay.address.lock().unwrap() = Some("https://app.locust.lol/v1/room/r/mcp".into());
    room.relay.reachable.store(false, Ordering::SeqCst);
    let state = room.remote.set_enabled(&json!(true));
    assert_eq!((state.enabled, state.message.as_deref()), (false, Some(RELAY_UNREACHED)));
    assert!(!room.file().contains("\"enabled\":true"));
    *room.refused.lock().unwrap() = Some("Not here.".into());
    room.relay.reachable.store(true, Ordering::SeqCst);
    let state = room.remote.set_enabled(&json!(true));
    assert_eq!((state.enabled, state.message.as_deref()), (false, Some("Not here.")));
    *room.refused.lock().unwrap() = None;
    let state = room.remote.set_enabled(&json!(true));
    assert_eq!((state.enabled, state.address.as_deref(), state.message), (true, Some("https://app.locust.lol/v1/room/r/mcp"), None));
    assert!(room.file().contains("\"enabled\":true"));
    // Not a boolean: nothing changes.
    assert!(room.remote.set_enabled(&json!("no")).enabled);
    // Off, no address is given.
    assert_eq!(room.remote.set_enabled(&json!(false)).address, None);
    // The relay is told again as the PC connects, in whatever state it is.
    room.remote.sync();
    assert_eq!(room.relay.opened.lock().unwrap().last(), Some(&false));
    room.remote.set_enabled(&json!(true));
    room.remote.sync();
    assert_eq!(room.relay.opened.lock().unwrap().last(), Some(&true));
}

#[test]
fn the_stub_relay_has_no_address_so_nothing_can_be_turned_on_without_a_real_one() {
    let folder = folder("stub");
    let remote = RemoteMcp::new(Options::new(folder.join(REMOTE_MCP_FILE), Arc::new(StubMcpRelay), Arc::new(FakeTools::default())));
    let state = remote.set_enabled(&json!(true));
    assert_eq!((state.enabled, state.message.as_deref()), (false, Some(NEEDS_FROM_ANYWHERE)));
    let _ = std::fs::remove_dir_all(folder);
}

#[test]
fn an_unsaveable_setting_is_said_and_keys_that_could_not_be_kept_are_not_given() {
    let room = world("unsaveable");
    room.remote.set_enabled(&json!(true));
    let proof = verifier();
    let sign_in = room.start_sign_in(json!({"challenge": challenge_of(&proof)}));
    room.remote.answer_sign_in(&room.card().request, true);
    let code = body(&sign_in.join().unwrap())["code"].as_str().unwrap().to_owned();
    // The file's place is taken by a folder: nothing can be written there.
    let place = room.folder.join(REMOTE_MCP_FILE);
    let _ = std::fs::remove_file(&place);
    std::fs::create_dir_all(&place).unwrap();
    let traded = room.token(&form(&[("grant_type", "authorization_code"), ("code", &code), ("code_verifier", &proof), ("client_id", "client_1"), ("redirect_uri", REDIRECT)]));
    assert_eq!((traded.status, body(&traded)), (500, json!({"error": "server_error"})));
    assert!(room.remote.settings().apps.is_empty());
    let state = room.remote.set_enabled(&json!(false));
    assert_eq!(state.message.as_deref(), Some(NOT_SAVED));
}

#[test]
fn the_ten_apps_most_lately_used_are_kept_and_the_one_used_longest_ago_goes() {
    let room = world("max-apps");
    room.remote.set_enabled(&json!(true));
    let mut ids = vec![];
    for _ in 0..MAX_APPS {
        room.tick(1000);
        room.connected();
        ids.push(room.remote.settings().apps.last().unwrap().id.clone());
    }
    assert_eq!(room.remote.settings().apps.len(), MAX_APPS);
    room.tick(1000);
    room.connected();
    let apps = room.remote.settings().apps;
    assert_eq!(apps.len(), MAX_APPS);
    assert!(!apps.iter().any(|a| a.id == ids[0]), "the first, used longest ago, is gone");
    assert!(apps.iter().any(|a| a.id == ids[1]) && apps.iter().any(|a| a.id == ids[MAX_APPS - 1]));
}

#[test]
fn a_refresh_key_lasts_ninety_days_unused_is_for_its_own_app_and_each_use_replaces_it() {
    let room = world("refresh");
    room.remote.set_enabled(&json!(true));
    let (keys, _) = room.connected();
    let refresh = keys["refresh_token"].as_str().unwrap().to_owned();
    let other = room.token(&form(&[("grant_type", "refresh_token"), ("refresh_token", &refresh), ("client_id", "client_2")]));
    assert_eq!(body(&other)["error_description"], "That refresh key was made for another app.");
    // Wrong app does not spend it.
    room.tick(REFRESH_MS - 1);
    let renewed = room.token(&form(&[("grant_type", "refresh_token"), ("refresh_token", &refresh), ("client_id", "client_1")]));
    assert_eq!(renewed.status, 200, "{}", renewed.body);
    let again = body(&renewed)["refresh_token"].as_str().unwrap().to_owned();
    assert_ne!(again, refresh);
    assert_eq!(room.remote.settings().apps.len(), 1);
    assert!(room.remote.settings().apps[0].used.is_some());
    room.tick(REFRESH_MS);
    let late = room.token(&form(&[("grant_type", "refresh_token"), ("refresh_token", &again)]));
    assert_eq!(body(&late)["error_description"], "That refresh key is not one this Locust holds. Connect again from the app.");
}

#[test]
fn an_apps_last_use_is_written_down_at_most_once_a_minute_not_with_every_call() {
    let room = world("used");
    room.remote.set_enabled(&json!(true));
    let (keys, _) = room.connected();
    let access = keys["access_token"].as_str().unwrap().to_owned();
    assert!(!room.file().contains("\"used\""));
    room.tick(5_000);
    room.rpc(&access, "ping", None, Some(1));
    room.tick(5_000);
    room.rpc(&access, "ping", None, Some(2));
    room.remote.tick();
    assert!(!room.file().contains("\"used\""), "a call is not a disk write");
    assert!(room.remote.settings().apps[0].used.is_some(), "but the window knows");
    room.tick(USED_SAVE_MS);
    room.remote.tick();
    assert!(room.file().contains(&format!("\"used\":{}", START + 5_000 + 5_000)));
}

#[test]
fn the_windows_look_writes_a_due_last_use_before_it_says_the_settings() {
    let room = world("look");
    room.remote.set_enabled(&json!(true));
    let (keys, _) = room.connected();
    let access = keys["access_token"].as_str().unwrap().to_owned();
    room.tick(5_000);
    room.rpc(&access, "ping", None, Some(1));
    let seen = room.remote.look();
    assert!(seen.apps[0].used.is_some());
    assert!(!room.file().contains("\"used\""), "not due yet: no disk write");
    room.tick(USED_SAVE_MS);
    room.remote.look();
    assert!(room.file().contains(&format!("\"used\":{}", START + 5_000)), "the window's look is what writes it down");
}

#[test]
fn an_unreadable_request_is_answered_in_json_rpcs_words_and_an_unknown_method_is_not_found() {
    let room = world("rpc-errors");
    room.remote.set_enabled(&json!(true));
    let (keys, _) = room.connected();
    let access = keys["access_token"].as_str().unwrap().to_owned();
    let code_of = |(status, json): (u16, Option<Value>)| {
        assert_eq!(status, 200);
        let error = json.unwrap()["error"].clone();
        (error["code"].as_i64().unwrap(), error["message"].as_str().unwrap().to_owned())
    };
    assert_eq!(code_of(room.raw(&access, "nope")), (-32700, "The request is not JSON.".into()));
    assert_eq!(code_of(room.raw(&access, "[]")), (-32600, "Send one JSON-RPC request at a time.".into()));
    assert_eq!(code_of(room.raw(&access, "7")), (-32600, "Send one JSON-RPC request at a time.".into()));
    assert_eq!(code_of(room.raw(&access, r#"{"id":4,"method":"ping"}"#)), (-32600, "Invalid request.".into()));
    assert_eq!(code_of(room.raw(&access, r#"{"jsonrpc":"2.0","id":4,"method":7}"#)), (-32600, "Invalid request.".into()));
    assert_eq!(code_of(room.rpc(&access, "resources/list", None, Some(5))), (-32601, "Method not found.".into()));
    // An unknown protocol is answered with the newest this Locust speaks; ping is answered empty.
    let (_, init) = room.rpc(&access, "initialize", Some(json!({"protocolVersion": "1999-01-01"})), Some(6));
    assert_eq!(init.unwrap()["result"]["protocolVersion"], "2025-06-18");
    let (_, ping) = room.rpc(&access, "ping", None, Some(7));
    assert_eq!(ping.unwrap(), json!({"jsonrpc": "2.0", "id": 7, "result": {}}));
    // A tool call with no arguments runs with none; one that fails says so and is not retried.
    room.rpc(&access, "tools/call", Some(json!({"name": "list_models"})), Some(8));
    assert_eq!(room.tools.calls.lock().unwrap().last(), Some(&("list_models".to_string(), json!({}))));
    room.tools.fail.store(true, Ordering::SeqCst);
    let (_, failed) = room.rpc(&access, "tools/call", Some(json!({"name": "list_models", "arguments": {}})), Some(9));
    let failed = failed.unwrap();
    assert_eq!((failed["result"]["isError"].clone(), failed["result"]["content"][0]["text"].as_str().unwrap()), (json!(true), NOT_COMPLETED));
    assert_eq!(room.tools.calls.lock().unwrap().len(), 2);
}

#[test]
fn an_answer_too_long_for_the_relay_is_said_to_be_too_long() {
    let room = world("long");
    room.remote.set_enabled(&json!(true));
    let (keys, _) = room.connected();
    let access = keys["access_token"].as_str().unwrap().to_owned();
    room.tools.huge.store(true, Ordering::SeqCst);
    let (status, long) = room.rpc(&access, "tools/call", Some(json!({"name": "list_teammates", "arguments": {}})), Some(1));
    assert_eq!(status, 200);
    let long = long.unwrap();
    assert_eq!((long["id"].clone(), long["result"]["isError"].clone(), long["result"]["content"][0]["text"].as_str().unwrap()), (json!(1), json!(true), TOO_LONG));
}

#[test]
fn saved_apps_are_read_back_and_an_expired_one_or_one_a_rule_forbids_is_not_kept() {
    let room = world("load");
    room.remote.set_enabled(&json!(true));
    let (keys, _) = room.connected();
    let access = keys["access_token"].as_str().unwrap().to_owned();
    let reopen = |room: &World| {
        let mut options = Options::new(room.folder.join(REMOTE_MCP_FILE), room.relay.clone(), room.tools.clone());
        let clock = room.now.clone();
        options.now = Arc::new(move || clock.load(Ordering::SeqCst));
        let r = room.refused.clone();
        options.refusal = Arc::new(move || r.lock().unwrap().clone());
        RemoteMcp::new(options)
    };
    let again = reopen(&room);
    let state = again.load();
    assert!(state.enabled);
    assert_eq!(state.apps.len(), 1);
    assert_eq!(state.apps[0].name, "Claude");
    let answer = again.answer(&RemoteMcpAsk { ask: "c".into(), kind: AskKind::Call, body: json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}).to_string(), auth: Some(access.clone()) });
    assert_eq!(answer.status, 200, "keys made before a restart still open the door after it");
    *room.refused.lock().unwrap() = Some("No.".into());
    let ruled = reopen(&room).load();
    assert!(!ruled.enabled);
    *room.refused.lock().unwrap() = None;
    room.tick(REFRESH_MS + 1);
    assert!(reopen(&room).load().apps.is_empty());
    // A file that is not there, or not ours, is off with no apps.
    std::fs::write(room.folder.join(REMOTE_MCP_FILE), "garbage").unwrap();
    let off = reopen(&room).load();
    assert!(!off.enabled && off.apps.is_empty());
}

#[test]
fn the_state_the_window_reads_is_electrons_shape_with_dates_and_no_key() {
    let room = world("shape");
    room.remote.set_enabled(&json!(true));
    let json = room.remote.settings().to_json();
    assert_eq!(json, json!({"enabled": true, "address": "https://app.locust.lol/v1/room/r/mcp", "apps": [], "asking": []}));
    let (_, _) = room.connected();
    let sign_in = room.start_sign_in(json!({"client": "  ", "redirect": "http://localhost:6274/cb"}));
    let card = room.card();
    let json = room.remote.settings().to_json();
    assert_eq!(json["apps"][0]["added"], "2027-01-15T08:00:00.000Z");
    assert_eq!(json["apps"][0]["name"], "Claude");
    assert!(json["apps"][0].get("used").is_none());
    assert_eq!(json["asking"][0], json!({"request": card.request, "name": "An app", "host": "an app on the computer it was started on", "match": "123456", "until": START + ASKING_MS}));
    room.remote.answer_sign_in(&card.request, false);
    sign_in.join().unwrap();
}

#[test]
fn the_host_a_card_names_is_where_the_app_sends_the_person_never_a_whole_address() {
    assert_eq!(host_of("https://claude.ai/api/mcp/auth_callback?x=1"), "claude.ai");
    assert_eq!(host_of("https://example.org:8443/cb"), "example.org:8443");
    assert_eq!(host_of("http://localhost:6274/cb"), "an app on the computer it was started on");
    assert_eq!(host_of("not an address"), "an address it did not make clear");
    assert_eq!(host_of(""), "an address it did not make clear");
}

#[test]
fn an_app_with_a_long_name_is_shown_by_its_first_eighty_letters_and_one_with_none_as_an_app() {
    let room = world("names");
    room.remote.set_enabled(&json!(true));
    let long = "é".repeat(120);
    let sign_in = room.start_sign_in(json!({"client": long}));
    let card = room.card();
    assert_eq!(card.name.chars().count(), 80);
    room.remote.answer_sign_in(&card.request, false);
    sign_in.join().unwrap();
    let sign_in = room.start_sign_in(json!({"client": null}));
    let card = room.card();
    assert_eq!(card.name, "An app");
    room.remote.answer_sign_in(&card.request, false);
    sign_in.join().unwrap();
}

#[test]
fn the_doors_own_tools_answer_an_app_from_anywhere_in_ask_unless_each_teammates_own_mode_is_on() {
    let root = folder("door-tools");
    for part in ["native", "electron", "work"] {
        std::fs::create_dir_all(root.join(part)).unwrap();
    }
    std::fs::write(
        root.join("electron").join("teammates.json"),
        json!({"schemaVersion": 1, "teammates": [{"teammateId": "tm_a", "name": "Reader", "hue": "blue", "role": "Custom", "createdAt": "2026-11-01T00:00:00Z", "folder": root.join("work").to_string_lossy(), "route": {"runtime": "codex", "model": "account-default", "mode": "plan"}}]}).to_string(),
    )
    .unwrap();
    let store = crate::store::Store::new(root.join("native"), root.join("electron")).unwrap();
    let tools: Arc<dyn McpTools> = Arc::new(crate::door::tools_from_anywhere(store.clone()));
    let runs_in = |tools: &Arc<dyn McpTools>| {
        let listed: Value = serde_json::from_str(&tools.call("list_teammates", &json!({})).unwrap().text).unwrap();
        (listed[0]["name"].clone(), listed[0]["runs_in"].clone())
    };
    // locust-mcp-tools.ts: `runs_in: own ? t.route?.mode ?? 'ask' : 'ask'`, the switch read as each call comes.
    assert_eq!(runs_in(&tools), (json!("Reader"), json!("ask")));
    let off = tools.call("list_conversations", &json!({})).unwrap();
    assert_eq!(off.text, "Off: this app sees only the conversations it started. The person can turn on \"Let apps see your conversations\" in Locust's Settings.");
    std::fs::write(store.root().join(crate::door::SETTINGS_FILE), json!({"enabled": true, "ownMode": true}).to_string()).unwrap();
    assert_eq!(runs_in(&tools), (json!("Reader"), json!("plan")));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn sync_and_settings_hold_the_same_transition_until_the_relay_answers() {
    use std::sync::{OnceLock, TryLockError, Weak};

    #[derive(Default)]
    struct Witness {
        service: OnceLock<Weak<RemoteMcp>>,
        wire: Mutex<Vec<(bool, bool)>>,
        during_notification: Mutex<Vec<bool>>,
    }

    impl Witness {
        fn transition_is_held(&self) -> bool {
            let service = self.service.get().unwrap().upgrade().unwrap();
            matches!(service.relay_transition.try_lock(), Err(TryLockError::WouldBlock))
        }
    }

    impl McpRelay for Witness {
        fn address(&self) -> Option<String> {
            Some("https://relay.test/v1/room/r/mcp".into())
        }
        fn open(&self, on: bool) -> bool {
            self.wire.lock().unwrap().push((on, self.transition_is_held()));
            true
        }
    }

    let dir = folder("transition");
    let witness = Arc::new(Witness::default());
    let mut options = Options::new(dir.join(REMOTE_MCP_FILE), witness.clone(), Arc::new(FakeTools::default()));
    let changed = witness.clone();
    options.changed = Arc::new(move |_| {
        changed.during_notification.lock().unwrap().push(changed.transition_is_held());
    });
    let service = Arc::new(RemoteMcp::new(options));
    assert!(witness.service.set(Arc::downgrade(&service)).is_ok());

    service.sync();
    assert!(service.set_enabled(&json!(true)).enabled);
    service.sync();
    assert!(!service.set_enabled(&json!(false)).enabled);
    let _ = std::fs::remove_dir_all(dir);

    assert_eq!(*witness.wire.lock().unwrap(), [(false, true), (true, true), (true, true), (false, true)]);
    assert_eq!(*witness.during_notification.lock().unwrap(), [false, false]);
}
